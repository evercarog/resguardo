//! 16x (7): las respuestas grandes de **lectura** de la consola, en gzip si el
//! navegador lo acepta. Con 200 equipos, `GET …/informes` son ~12 MB y
//! `GET /api/panel` ~5 MB de JSON que se comprime a una décima parte
//! (`docs/rendimiento.md`).
//!
//! Solo unas rutas concretas, de solo lectura, que no llevan secretos ni nada
//! que escriba quien pide (así no hay ataque tipo BREACH: no hay un secreto que
//! adivinar por el tamaño), solo con estado 200, JSON y más de [`MIN_BYTES`].
//! Comprimir cuesta algo de procesador: se hace fuera del hilo de la red
//! (`spawn_blocking`) y con un nivel rápido.

use axum::body::{Body, Bytes};
use axum::extract::Request;
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

/// Por debajo de esto no merece la pena.
pub const MIN_BYTES: usize = 32 * 1024;
/// Lo más que se lee de una respuesta para comprimirla (más grande: se manda tal cual).
pub const MAX_BYTES: usize = 128 * 1024 * 1024;
/// Nivel de deflate (0–10): rápido; el JSON se queda en ~1/9 igualmente.
const NIVEL: u8 = 3;

/// ¿Acepta el navegador esta codificación? (`Accept-Encoding`, sin las de `q=0`.)
pub fn acepta(cabeceras: &HeaderMap, codificacion: &str) -> bool {
    cabeceras.get_all(header::ACCEPT_ENCODING).iter().filter_map(|v| v.to_str().ok()).flat_map(|v| v.split(',')).any(|parte| {
        let mut trozos = parte.split(';');
        let nombre = trozos.next().unwrap_or("").trim();
        let q_cero = trozos.any(|p| {
            let p = p.trim();
            p.strip_prefix("q=").or_else(|| p.strip_prefix("Q=")).and_then(|q| q.trim().parse::<f32>().ok()).is_some_and(|q| q <= 0.0)
        });
        !q_cero && (nombre.eq_ignore_ascii_case(codificacion) || nombre == "*")
    })
}

/// Las rutas que se comprimen: grandes, de solo lectura y sin secretos.
pub fn se_comprime(ruta: &str) -> bool {
    if ruta == "/api/panel" {
        return true;
    }
    let Some(resto) = ruta.strip_prefix("/api/clientes/") else { return false };
    let partes: Vec<&str> = resto.split('/').collect();
    matches!(partes.as_slice(), [_, "informes"] | [_, "resumen"] | [_, "equipos", _, "informes"] | [_, "equipos", _, "historial"] | [_, "restauraciones"])
}

/// CRC-32 (IEEE), el del pie de un gzip.
fn crc32(datos: &[u8]) -> u32 {
    static TABLA: std::sync::LazyLock<[u32; 256]> = std::sync::LazyLock::new(|| {
        let mut t = [0u32; 256];
        for (i, v) in t.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *v = c;
        }
        t
    });
    !datos.iter().fold(!0u32, |c, &b| TABLA[((c ^ u32::from(b)) & 0xFF) as usize] ^ (c >> 8))
}

/// Un gzip (RFC 1952) con deflate de `miniz_oxide`, que ya está en el árbol.
pub fn gzip(datos: &[u8]) -> Vec<u8> {
    let cuerpo = miniz_oxide::deflate::compress_to_vec(datos, NIVEL);
    let mut out = Vec::with_capacity(cuerpo.len() + 18);
    // Cabecera: firma, deflate, sin opciones, sin fecha, sin pistas, sistema «desconocido».
    out.extend_from_slice(&[0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 0xff]);
    out.extend_from_slice(&cuerpo);
    out.extend_from_slice(&crc32(datos).to_le_bytes());
    out.extend_from_slice(&(datos.len() as u32).to_le_bytes());
    out
}

/// La capa: comprime la respuesta si toca (ver arriba).
pub async fn respuestas(req: Request, next: Next) -> Response {
    let toca = req.method() == Method::GET && se_comprime(req.uri().path()) && acepta(req.headers(), "gzip");
    let res = next.run(req).await;
    let json = res.headers().get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).is_some_and(|t| t.starts_with("application/json"));
    if !toca || !json || res.status() != StatusCode::OK || res.headers().contains_key(header::CONTENT_ENCODING) {
        return res;
    }
    // Una respuesta que pasa del tope va tal cual (leerla entera daría error).
    if axum::body::HttpBody::size_hint(res.body()).upper().is_none_or(|n| n > MAX_BYTES as u64) {
        return res;
    }
    let (mut partes, cuerpo) = res.into_parts();
    let Ok(bytes): Result<Bytes, _> = axum::body::to_bytes(cuerpo, MAX_BYTES).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    if bytes.len() < MIN_BYTES {
        return Response::from_parts(partes, Body::from(bytes));
    }
    let copia = bytes.clone();
    match tokio::task::spawn_blocking(move || gzip(&copia)).await {
        Ok(comprimido) => {
            partes.headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
            partes.headers.insert(header::VARY, HeaderValue::from_static("accept-encoding"));
            partes.headers.remove(header::CONTENT_LENGTH);
            Response::from_parts(partes, Body::from(comprimido))
        }
        Err(_) => Response::from_parts(partes, Body::from(bytes)),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn con(valor: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(header::ACCEPT_ENCODING, HeaderValue::from_str(valor).unwrap());
        h
    }

    #[test]
    fn lee_accept_encoding() {
        assert!(acepta(&con("gzip, deflate, br, zstd"), "br") && acepta(&con("gzip, deflate, br"), "gzip"));
        assert!(acepta(&con("gzip;q=1.0, br;q=0"), "gzip") && !acepta(&con("gzip;q=1.0, br;q=0"), "br"));
        assert!(!acepta(&con("identity"), "gzip"));
        assert!(!acepta(&HeaderMap::new(), "br"));
        assert!(acepta(&con("*"), "br"));
        // Basura: sin pánico.
        assert!(acepta(&con(";;q=x,,br;q=abc"), "br") && !acepta(&con(";;q=x,,br;q=abc"), "gzip"));
    }

    #[test]
    fn solo_las_rutas_de_lectura_grandes() {
        for si in [
            "/api/panel",
            "/api/clientes/c1/informes",
            "/api/clientes/c1/resumen",
            "/api/clientes/c1/equipos/e1/historial",
            "/api/clientes/c1/equipos/e1/informes",
            "/api/clientes/c1/restauraciones",
        ] {
            assert!(se_comprime(si), "{si}");
        }
        for no in [
            "/api/sesion",
            "/api/cuenta",
            "/api/clientes/c1",
            "/api/clientes/c1/equipos/e1",
            "/api/clientes/c1/informes/x",
            "/api/panel/progreso",
            "/api/clientes/c1/emparejamientos",
            "/api/panelx",
        ] {
            assert!(!se_comprime(no), "{no}");
        }
    }

    #[test]
    fn crc32_conocido() {
        // El de «123456789» (valor de comprobación de CRC-32/IEEE).
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    /// La capa entera, con un router de prueba: grande y aceptado, en gzip; si no, tal cual.
    #[tokio::test]
    async fn la_capa() {
        use axum::routing::get;
        use tower::ServiceExt;
        let grande = serde_json::json!({ "datos": vec!["una versión"; 20_000] });
        let g2 = grande.clone();
        let app = axum::Router::new()
            .route("/api/panel", get(move || async move { axum::Json(g2) }))
            .route("/api/clientes/c1/resumen", get(|| async { axum::Json(serde_json::json!({ "poco": 1 })) }))
            .route("/api/sesion", get(|| async { axum::Json(serde_json::json!({ "datos": vec!["x"; 50_000] })) }))
            .layer(axum::middleware::from_fn(respuestas));
        let pedir = |ruta: &str, ae: Option<&str>| {
            let mut r = axum::http::Request::builder().uri(ruta);
            if let Some(v) = ae {
                r = r.header(header::ACCEPT_ENCODING, v);
            }
            app.clone().oneshot(r.body(Body::empty()).unwrap())
        };
        let r = pedir("/api/panel", Some("gzip, br")).await.unwrap();
        assert_eq!(r.headers().get(header::CONTENT_ENCODING).unwrap(), "gzip");
        assert_eq!(r.headers().get(header::VARY).unwrap(), "accept-encoding");
        let b = axum::body::to_bytes(r.into_body(), usize::MAX).await.unwrap();
        let crudo = miniz_oxide::inflate::decompress_to_vec(&b[10..b.len() - 8]).unwrap();
        assert_eq!(serde_json::from_slice::<serde_json::Value>(&crudo).unwrap(), grande);
        // Sin aceptarlo, pequeña o de otra ruta: tal cual.
        assert!(pedir("/api/panel", None).await.unwrap().headers().get(header::CONTENT_ENCODING).is_none());
        assert!(pedir("/api/panel", Some("gzip;q=0")).await.unwrap().headers().get(header::CONTENT_ENCODING).is_none());
        assert!(pedir("/api/clientes/c1/resumen", Some("gzip")).await.unwrap().headers().get(header::CONTENT_ENCODING).is_none());
        assert!(pedir("/api/sesion", Some("gzip")).await.unwrap().headers().get(header::CONTENT_ENCODING).is_none());
    }

    /// Para medir (docs/rendimiento.md): `RESGUARDO_MEDIR_JSON=<archivo> cargo test -p
    /// resguardo-servidor --release --lib medir_con_un_archivo -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn medir_con_un_archivo() {
        let Ok(ruta) = std::env::var("RESGUARDO_MEDIR_JSON") else { return };
        let datos = std::fs::read(ruta).unwrap();
        let t = std::time::Instant::now();
        let g = gzip(&datos);
        println!("{} bytes → {} en gzip ({:.1} %) en {} ms", datos.len(), g.len(), g.len() as f64 * 100.0 / datos.len() as f64, t.elapsed().as_millis());
    }

    #[test]
    fn gzip_se_abre() {
        let datos = serde_json::to_vec(&serde_json::json!({ "repos": vec!["versión con tildes y eñes"; 5000] })).unwrap();
        let g = gzip(&datos);
        assert!(g.len() < datos.len() / 5);
        assert_eq!(&g[..3], &[0x1f, 0x8b, 8]);
        // El cuerpo deflate, de vuelta; y el pie dice su CRC y su tamaño.
        let crudo = miniz_oxide::inflate::decompress_to_vec(&g[10..g.len() - 8]).unwrap();
        assert_eq!(crudo, datos);
        assert_eq!(&g[g.len() - 8..g.len() - 4], &crc32(&datos).to_le_bytes());
        assert_eq!(&g[g.len() - 4..], &(datos.len() as u32).to_le_bytes());
    }
}
