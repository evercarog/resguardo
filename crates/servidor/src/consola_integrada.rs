//! La consola web dentro del binario (feature `consola-integrada`): los
//! archivos de `consola/build` se incluyen al compilar. `--consola DIR` tiene
//! prioridad (para probar otra versión sin recompilar).
//!
//! 16x (0): la compilación de la consola deja junto a cada archivo de texto su
//! versión en brotli (`.br`) y en gzip (`.gz`); aquí se manda la que acepte el
//! navegador (`Accept-Encoding`) con `Content-Encoding` y `Vary`, sin comprimir
//! nada al servir. Cada archivo lleva su `ETag` (la huella que guarda la
//! compilación), así `index.html` (sin caché) vuelve con un 304 si no cambió.

use crate::comprimir::acepta;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};

#[derive(rust_embed::Embed)]
#[folder = "../../consola/build"]
struct Archivos;

/// ¿Lleva este binario la consola? (Sin `index.html`, la compilación no la encontró.)
pub fn disponible() -> bool {
    Archivos::get("index.html").is_some()
}

/// Las codificaciones que se pueden mandar, de la preferida a la menos.
const CODIFICACIONES: [(&str, &str); 2] = [("br", "br"), ("gzip", "gz")];

/// ¿Ya tiene el navegador esta versión? (`If-None-Match` con la misma ETag, o `*`.)
fn ya_la_tiene(cabeceras: &HeaderMap, etag: &str) -> bool {
    cabeceras
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(|e| e.trim().trim_start_matches("W/"))
        .any(|e| e == etag || e == "*")
}

/// Sirve un archivo de la consola; las rutas que no son archivos van a `index.html` (SPA).
pub async fn servir(uri: Uri, cabeceras: HeaderMap) -> Response {
    let ruta = uri.path().trim_start_matches('/');
    // Las versiones comprimidas no se piden por su nombre: solo con `Accept-Encoding`.
    let es_comprimida = ruta.ends_with(".br") || ruta.ends_with(".gz");
    let (archivo, nombre) = match Archivos::get(ruta).filter(|_| !ruta.is_empty() && !es_comprimida) {
        Some(a) => (a, ruta),
        None if ruta.starts_with("_app/") => return StatusCode::NOT_FOUND.into_response(),
        None => match Archivos::get("index.html") {
            Some(a) => (a, "index.html"),
            None => return StatusCode::NOT_FOUND.into_response(),
        },
    };
    let tipo = archivo.metadata.mimetype().to_string();
    // Lo de `_app/immutable` lleva su hash en el nombre: se puede guardar para siempre.
    let cache = if nombre.starts_with("_app/immutable/") { "public, max-age=31536000, immutable" } else { "no-cache" };
    let huella: String = archivo.metadata.sha256_hash().iter().take(12).map(|b| format!("{b:02x}")).collect();

    // La versión comprimida que acepte el navegador, si la compilación la dejó.
    let comprimida = CODIFICACIONES.iter().find_map(|(cod, ext)| {
        if !acepta(&cabeceras, cod) {
            return None;
        }
        Archivos::get(&format!("{nombre}.{ext}")).map(|a| (*cod, a))
    });
    let hay_variantes = CODIFICACIONES.iter().any(|(_, ext)| Archivos::get(&format!("{nombre}.{ext}")).is_some());
    let etag = match &comprimida {
        Some((cod, _)) => format!("\"{huella}-{cod}\""),
        None => format!("\"{huella}\""),
    };

    let mut res = if ya_la_tiene(&cabeceras, &etag) {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        match comprimida {
            Some((cod, a)) => {
                let mut r = a.data.into_owned().into_response();
                r.headers_mut().insert(header::CONTENT_ENCODING, HeaderValue::from_static(cod));
                r
            }
            None => archivo.data.into_owned().into_response(),
        }
    };
    let h = res.headers_mut();
    if let Ok(v) = HeaderValue::from_str(&tipo) {
        h.insert(header::CONTENT_TYPE, v);
    }
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    if let Ok(v) = HeaderValue::from_str(&etag) {
        h.insert(header::ETAG, v);
    }
    if hay_variantes {
        h.insert(header::VARY, HeaderValue::from_static("accept-encoding"));
    }
    res
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn con(nombre: header::HeaderName, valor: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(nombre, HeaderValue::from_str(valor).unwrap());
        h
    }

    #[test]
    fn lee_if_none_match() {
        assert!(ya_la_tiene(&con(header::IF_NONE_MATCH, "\"abc\""), "\"abc\""));
        assert!(ya_la_tiene(&con(header::IF_NONE_MATCH, "\"x\", W/\"abc\""), "\"abc\""));
        assert!(ya_la_tiene(&con(header::IF_NONE_MATCH, "*"), "\"abc\""));
        assert!(!ya_la_tiene(&con(header::IF_NONE_MATCH, "\"abc-br\""), "\"abc\""));
        assert!(!ya_la_tiene(&HeaderMap::new(), "\"abc\""));
    }
}
