//! Aislamiento entre clientes de un servidor compartido (v1.34), con las rutas
//! sacadas de la propia tabla de rutas (src/api/mod.rs y src/agentes.rs): una
//! ruta nueva entra sola en la prueba.
//!
//! - Quien es miembro (incluso propietario) del cliente A, o el propietario del
//!   servidor que no es miembro de B, no lee ni toca nada de B por ninguna ruta
//!   ni con ningún método: «no existe».
//! - Con su propio cliente en la ruta y los ids de B en el resto, tampoco llega
//!   a nada de B (cada cliente tiene su base de datos).
//! - Un equipo de A no llega a las sesiones ni a los relés de B.
//! - «Clientes del servidor» da cifras, sin nada de dentro de los clientes.
//! - Las cuotas de cada cliente (equipos, historial, relé y órdenes por minuto).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use base64::Engine;
use ed25519_dalek::SigningKey;
use http_body_util::BodyExt;
use resguardo_protocolo::{claves, mensajes};
use resguardo_servidor::estado::{Opciones, St};
use resguardo_servidor::{api, auth, preparar};
use serde_json::{json, Value};
use tower::ServiceExt;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
/// Lo que nunca puede aparecer en una respuesta a quien no es de B.
const NOMBRE_EQUIPO_B: &str = "EQUIPO-SECRETO-DE-B";
const CORREO_B: &str = "bea@cliente-b.ejemplo.com";

struct Prueba {
    _dir: tempfile::TempDir,
    st: St,
    app: Router,
}

fn servidor() -> Prueba {
    let dir = tempfile::tempdir().unwrap();
    let st = preparar(dir.path(), Opciones { https: false, publico: true, ..Default::default() }).unwrap();
    let app = api::router(st.clone());
    Prueba { _dir: dir, st, app }
}

struct Resp {
    estado: StatusCode,
    json: Value,
    texto: String,
    cookie: Option<String>,
}

async fn pedir_bytes(app: &Router, metodo: &str, ruta: &str, cuerpo: Body, tipo: &str, cookie: Option<&str>, auth: Option<&str>) -> Resp {
    let mut r = Request::builder().method(metodo).uri(ruta).header("host", "localhost").header("content-type", tipo);
    if metodo != "GET" {
        r = r.header("x-resguardo", "1");
    }
    if let Some(c) = cookie {
        r = r.header("cookie", c);
    }
    if let Some(a) = auth {
        r = r.header("authorization", a);
    }
    let res = app.clone().oneshot(r.body(cuerpo).unwrap()).await.unwrap();
    let estado = res.status();
    let cookie = res.headers().get("set-cookie").and_then(|v| v.to_str().ok()).and_then(|v| v.split(';').next()).map(str::to_string);
    let bytes = res.into_body().collect().await.unwrap().to_bytes().to_vec();
    let texto = String::from_utf8_lossy(&bytes).to_string();
    Resp { estado, json: serde_json::from_slice(&bytes).unwrap_or(Value::Null), texto, cookie }
}

async fn pedir(app: &Router, metodo: &str, ruta: &str, cuerpo: Option<Value>, cookie: Option<&str>) -> Resp {
    let body = cuerpo.map(|v| Body::from(v.to_string())).unwrap_or_else(Body::empty);
    pedir_bytes(app, metodo, ruta, body, "application/json", cookie, None).await
}

fn totp(secreto: &str) -> String {
    let paso = chrono::Utc::now().timestamp() / 30;
    format!("{:06}", auth::hotp(&auth::de_base32(secreto).unwrap(), paso as u64))
}

/// Primer arranque: el propietario del servidor.
async fn propietario_servidor(p: &Prueba) -> String {
    let codigo = api::preparar_codigo_arranque(&p.st).unwrap().unwrap();
    let cuerpo = json!({ "codigo_arranque": codigo, "correo": "ana@servidor.ejemplo.com", "nombre": "Ana", "contrasena": "una contraseña bien larga" });
    let r = pedir(&p.app, "POST", "/api/inicio", Some(cuerpo), None).await;
    let (cookie, secreto) = (r.cookie.unwrap(), r.json["totp"]["secreto"].as_str().unwrap().to_string());
    let r = pedir(&p.app, "POST", "/api/sesion/totp", Some(json!({ "codigo": totp(&secreto) })), Some(&cookie)).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    r.cookie.unwrap()
}

/// Acepta una invitación con una cuenta nueva y devuelve su sesión completa.
async fn aceptar(p: &Prueba, enlace: &str, correo: &str) -> String {
    let token = enlace.split('#').nth(1).unwrap();
    let cuerpo = json!({ "token": token, "correo": correo, "nombre": correo, "contrasena": "contraseña de la invitada" });
    let r = pedir(&p.app, "POST", "/api/invitaciones/aceptar", Some(cuerpo), None).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let (cookie, secreto) = (r.cookie.unwrap(), r.json["totp"]["secreto"].as_str().unwrap().to_string());
    let r = pedir(&p.app, "POST", "/api/sesion/totp", Some(json!({ "codigo": totp(&secreto) })), Some(&cookie)).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    r.cookie.unwrap()
}

struct Equipo {
    id: String,
    secreto: String,
}

impl Equipo {
    fn auth(&self) -> String {
        format!("Equipo {}:{}", self.id, self.secreto)
    }
}

/// Empareja y confirma un equipo con ese nombre. Devuelve el equipo y el id del emparejamiento.
async fn equipo(p: &Prueba, cookie: &str, c: &str, nombre: &str) -> (Equipo, String) {
    let r = pedir(&p.app, "POST", &format!("/api/clientes/{c}/emparejamientos"), None, Some(cookie)).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let (emp, codigo) = (r.json["id"].as_str().unwrap().to_string(), r.json["codigo"].as_str().unwrap().to_string());
    let box_pub = claves::public_of(&claves::new_key()).unwrap();
    let sign_pub = B64.encode(SigningKey::from_bytes(&[7u8; 32]).verifying_key().to_bytes());
    let cuerpo = json!({ "codigo_hash": mensajes::code_hash(&codigo), "nombre": nombre, "so": "windows", "version": "0.7.14", "box_pub": box_pub, "sign_pub": sign_pub, "sal_equipo": B64.encode([3u8; 16]) });
    let r = pedir(&p.app, "POST", "/api/agente/unirse", Some(cuerpo), None).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let e = Equipo { id: r.json["equipo_id"].as_str().unwrap().into(), secreto: r.json["secreto"].as_str().unwrap().into() };
    let r = pedir(&p.app, "POST", &format!("/api/clientes/{c}/emparejamientos/{emp}/confirmar"), Some(json!({ "etiqueta": "ETIQUETA" })), Some(cookie)).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    (e, emp)
}

fn caduca() -> String {
    (chrono::Local::now() + chrono::Duration::hours(2)).to_rfc3339()
}

/// Las rutas de un archivo de rutas: (ruta, métodos).
fn rutas(fuente: &str) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    let trozos: Vec<&str> = fuente.split(".route(").skip(1).collect();
    for t in trozos {
        let Some(ruta) = t.trim_start().strip_prefix('"').and_then(|r| r.split('"').next()) else { continue };
        // Hasta el paréntesis que cierra este `.route(`.
        let mut nivel = 0i32;
        let fin = t
            .char_indices()
            .find(|(_, ch)| {
                match ch {
                    '(' => nivel += 1,
                    ')' if nivel == 0 => return true,
                    ')' => nivel -= 1,
                    _ => {}
                }
                false
            })
            .map_or(t.len(), |(i, _)| i);
        let cuerpo = &t[..fin];
        let mut metodos = Vec::new();
        for m in ["get", "post", "put", "patch", "delete"] {
            let con_punto = format!(".{m}(");
            let sin = format!("{m}(");
            if cuerpo.contains(&con_punto) || cuerpo.trim_start().split_once(',').is_some_and(|(_, r)| r.trim_start().starts_with(&sin)) {
                metodos.push(m.to_uppercase());
            }
        }
        out.push((ruta.to_string(), metodos));
    }
    out
}

#[test]
fn la_tabla_de_rutas_se_lee() {
    let r = rutas(include_str!("../src/api/mod.rs"));
    let buscar = |ruta: &str| r.iter().find(|(x, _)| x == ruta).map(|(_, m)| m.clone()).unwrap_or_default();
    assert_eq!(buscar("/api/clientes/{c}/equipos/{e}/ordenes"), ["GET", "POST"]);
    assert_eq!(buscar("/api/clientes/{c}/paquete"), ["GET", "PUT", "DELETE"]);
    assert_eq!(buscar("/api/clientes/{c}/miembros/{cuenta}"), ["PUT", "DELETE"]);
    assert!(r.iter().filter(|(x, _)| x.starts_with("/api/clientes/{c}")).count() > 40, "{r:?}");
    let a = rutas(include_str!("../src/agentes.rs"));
    assert!(a.iter().any(|(x, m)| x == "/api/agente/relevos/{r}/trozos/{n}" && m == &["PUT"]), "{a:?}");
}

/// Lo de B que Bea ve (para comprobar que nada cambió).
async fn foto_de_b(p: &Prueba, bea: &str, b: &str) -> Value {
    let mut foto = json!({});
    for que in ["equipos", "miembros", "plantillas", "ordenes", "emparejamientos"] {
        let r = pedir(&p.app, "GET", &format!("/api/clientes/{b}/{que}"), None, Some(bea)).await;
        assert_eq!(r.estado, StatusCode::OK, "{que}: {}", r.json);
        let mut v = r.json;
        // Lo que cambia solo con el tiempo o con el canal.
        if let Some(l) = v.as_array_mut() {
            for x in l.iter_mut() {
                if let Some(o) = x.as_object_mut() {
                    for k in ["ultimo_contacto", "conectado", "caduca", "estado"] {
                        if que != "ordenes" || k != "estado" {
                            o.remove(k);
                        }
                    }
                }
            }
        }
        foto[que] = v;
    }
    foto
}

#[tokio::test]
async fn nadie_de_fuera_llega_a_otro_cliente() {
    let p = servidor();
    let ana = propietario_servidor(&p).await;
    // A: de Ana (la crea ella y es su propietaria), con Luis también propietario y un equipo.
    let r = pedir(&p.app, "POST", "/api/clientes", Some(json!({ "nombre": "Cliente A" })), Some(&ana)).await;
    let a = r.json["id"].as_str().unwrap().to_string();
    let r = pedir(&p.app, "POST", &format!("/api/clientes/{a}/invitaciones"), Some(json!({ "rol": "propietario" })), Some(&ana)).await;
    let luis = aceptar(&p, r.json["enlace"].as_str().unwrap(), "luis@cliente-a.ejemplo.com").await;
    let (equipo_a, _) = equipo(&p, &ana, &a, "EQUIPO-DE-A").await;

    // B: para otra persona. Ana no es miembro: ni lo ve.
    let r = pedir(&p.app, "POST", "/api/servidor/clientes", Some(json!({ "nombre": "Cliente B", "cuotas": { "equipos": 10 } })), Some(&ana)).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let b = r.json["id"].as_str().unwrap().to_string();
    let bea = aceptar(&p, r.json["invitacion"]["enlace"].as_str().unwrap(), CORREO_B).await;
    let r = pedir(&p.app, "GET", "/api/cuenta", None, Some(&ana)).await;
    assert!(!r.texto.contains(&b), "Ana no es miembro de B");
    let r = pedir(&p.app, "GET", "/api/cuenta", None, Some(&bea)).await;
    assert_eq!(r.json["clientes"][0]["rol"], "propietario");
    let bea_id = r.json["id"].as_str().unwrap().to_string();
    // Lo de B: un equipo, una sesión, un relé, una orden, una plantilla y un emparejamiento abierto.
    let (equipo_b, emp_b) = equipo(&p, &bea, &b, NOMBRE_EQUIPO_B).await;
    let sobre = B64.encode([9u8; 64]);
    let sesion_b = uuid::Uuid::new_v4().to_string();
    let r = pedir(
        &p.app,
        "POST",
        &format!("/api/clientes/{b}/equipos/{}/ordenes", equipo_b.id),
        Some(json!({ "tipo": "explorar", "seq": 1, "sellado": sobre, "caduca": caduca(), "sesion": sesion_b })),
        Some(&bea),
    )
    .await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let orden_b = r.json["id"].as_str().unwrap().to_string();
    let relevo_b = uuid::Uuid::new_v4().to_string();
    let r = pedir(
        &p.app,
        "POST",
        &format!("/api/clientes/{b}/equipos/{}/ordenes", equipo_b.id),
        Some(json!({ "tipo": "descargar", "seq": 2, "sellado": sobre, "caduca": caduca(), "relevo": { "id": relevo_b, "max_bytes": 1024 } })),
        Some(&bea),
    )
    .await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let plantilla_b = uuid::Uuid::new_v4().to_string();
    let r = pedir(&p.app, "PUT", &format!("/api/clientes/{b}/plantillas/{plantilla_b}"), Some(json!({ "cifrado": B64.encode([1u8; 64]) })), Some(&bea)).await;
    assert!(r.estado.is_success(), "{}", r.json);
    let r = pedir(&p.app, "POST", &format!("/api/clientes/{b}/emparejamientos"), None, Some(&bea)).await;
    let emp_abierto_b = r.json["id"].as_str().unwrap().to_string();
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{b}"), None, Some(&bea)).await;
    let sal_b = r.json["sal_cliente"].as_str().unwrap().to_string();
    let antes = foto_de_b(&p, &bea, &b).await;
    assert!(antes.to_string().contains(NOMBRE_EQUIPO_B));

    let secretos = [NOMBRE_EQUIPO_B, CORREO_B, sal_b.as_str(), equipo_b.secreto.as_str()];
    let sin_secretos = |r: &Resp, que: &str| {
        for s in secretos {
            assert!(!r.texto.contains(s), "{que} dejó ver «{s}»: {}", r.texto);
        }
    };
    let ids_b = |ruta: &str, c: &str| {
        ruta.replace("{c}", c)
            .replace("{e}", &equipo_b.id)
            .replace(
                "{p}",
                if ruta.contains("plantillas") {
                    &plantilla_b
                } else if ruta.contains("confirmar") {
                    &emp_abierto_b
                } else {
                    &emp_b
                },
            )
            .replace("{o}", &orden_b)
            .replace("{s}", &sesion_b)
            .replace("{r}", &relevo_b)
            .replace("{n}", "0")
            .replace("{a}", &uuid::Uuid::new_v4().to_string())
            .replace("{k}", "canal-b")
            .replace("{d}", "destino-b")
            .replace("{cuenta}", &bea_id)
    };

    // 1. Con B en la ruta: ni el propietario del servidor ni la propietaria de A llegan.
    let tabla = rutas(include_str!("../src/api/mod.rs"));
    let mut probadas = 0;
    for (ruta, metodos) in tabla.iter().filter(|(r, _)| r.starts_with("/api/clientes/{c}")) {
        for m in metodos {
            for (quien, cookie) in [("el propietario del servidor", &ana), ("la propietaria de A", &luis)] {
                let url = ids_b(ruta, &b);
                let r = pedir(&p.app, m, &url, Some(json!({})), Some(cookie)).await;
                assert_eq!(r.estado, StatusCode::NOT_FOUND, "{quien}: {m} {ruta} → {} {}", r.estado, r.texto);
                assert_eq!(r.json["error"], "no_existe", "{m} {ruta}");
                sin_secretos(&r, &format!("{m} {ruta}"));
                probadas += 1;
            }
        }
    }
    assert!(probadas > 100, "{probadas} peticiones");

    // 2. Con A en la ruta y los ids de B en lo demás: nada de B.
    for (ruta, metodos) in tabla.iter().filter(|(r, _)| r.starts_with("/api/clientes/{c}/")) {
        for m in metodos {
            let url = ids_b(ruta, &a);
            let r = pedir(&p.app, m, &url, Some(json!({})), Some(&luis)).await;
            assert!(!r.estado.is_server_error(), "{m} {ruta} → {}", r.texto);
            sin_secretos(&r, &format!("{m} {ruta} (con A)"));
        }
    }

    // 3. Las del propietario del servidor: quien no lo es, nada.
    for (ruta, metodos) in tabla.iter().filter(|(r, _)| r.starts_with("/api/servidor/clientes") || r == "/api/servidor/cuotas") {
        for m in metodos {
            let r = pedir(&p.app, m, &ids_b(ruta, &b), Some(json!({ "nombre": "x" })), Some(&luis)).await;
            assert_eq!(r.estado, StatusCode::FORBIDDEN, "{m} {ruta}: {}", r.texto);
        }
    }

    // 4. Un equipo de A con las sesiones y los relés de B.
    let agentes = rutas(include_str!("../src/agentes.rs"));
    let mut con_ids = 0;
    for (ruta, metodos) in agentes.iter().filter(|(r, _)| r.contains('{')) {
        for m in metodos {
            let url = ids_b(ruta, &a);
            let (cuerpo, tipo) = if ruta.contains("/trozos/") {
                (Body::from(vec![1u8; 16]), "application/octet-stream")
            } else {
                (Body::from(json!({ "trozos": 0, "bytes": 0, "cifrado": B64.encode([5u8; 40]) }).to_string()), "application/json")
            };
            let r = pedir_bytes(&p.app, m, &url, cuerpo, tipo, None, Some(&equipo_a.auth())).await;
            assert_eq!(r.estado, StatusCode::NOT_FOUND, "equipo de A: {m} {ruta} → {}", r.texto);
            con_ids += 1;
        }
    }
    assert!(con_ids >= 4, "{agentes:?}");

    // Nada de B cambió.
    assert_eq!(foto_de_b(&p, &bea, &b).await, antes);

    // 5. «Clientes del servidor»: cifras de B, nada de dentro.
    let r = pedir(&p.app, "GET", "/api/servidor/clientes", None, Some(&ana)).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    sin_secretos(&r, "Clientes del servidor");
    assert!(!r.texto.contains(&equipo_b.id), "ni los ids de sus equipos");
    let fila = r.json["clientes"].as_array().unwrap().iter().find(|c| c["id"] == b.as_str()).unwrap().clone();
    assert_eq!((fila["equipos"].as_i64(), fila["personas"].as_i64(), fila["soy_miembro"].as_bool()), (Some(1), Some(1), Some(false)));
    assert_eq!(fila["cuotas"]["equipos"], 10);
    assert_eq!(fila["efectivas"]["equipos"], 10);
    assert_eq!(r.json["predeterminadas"]["equipos"], 50, "en internet, 50 equipos por defecto");
    assert!(fila["ultima_actividad"].is_string() && fila["bytes"].as_u64().unwrap() > 0);
    // Con propietario dentro, el del servidor no puede invitarse como propietario.
    let r = pedir(&p.app, "POST", &format!("/api/servidor/clientes/{b}/invitacion"), None, Some(&ana)).await;
    assert_eq!(r.estado, StatusCode::CONFLICT, "{}", r.json);
}

#[tokio::test]
async fn cuotas_de_cada_cliente() {
    let p = servidor();
    let ana = propietario_servidor(&p).await;
    let r = pedir(&p.app, "POST", "/api/clientes", Some(json!({ "nombre": "Pequeño" })), Some(&ana)).await;
    let c = r.json["id"].as_str().unwrap().to_string();
    // Valores fuera de rango, no.
    let r = pedir(&p.app, "PUT", &format!("/api/servidor/clientes/{c}/cuotas"), Some(json!({ "equipos": -1 })), Some(&ana)).await;
    assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY);
    let r = pedir(
        &p.app,
        "PUT",
        &format!("/api/servidor/clientes/{c}/cuotas"),
        Some(json!({ "equipos": 1, "ordenes_min": 2, "relevo_mb_mes": 1, "historial": 3 })),
        Some(&ana),
    )
    .await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["efectivas"]["ordenes_min"], 2);

    // Equipos: uno sí; el segundo, ni código.
    let (eq, _) = equipo(&p, &ana, &c, "UNO").await;
    let r = pedir(&p.app, "POST", &format!("/api/clientes/{c}/emparejamientos"), None, Some(&ana)).await;
    assert_eq!((r.estado, r.json["error"].as_str()), (StatusCode::FORBIDDEN, Some("cuota")), "{}", r.json);
    assert!(r.json["mensaje"].as_str().unwrap().contains("1 equipos"));

    // Órdenes por minuto: dos, y la tercera espera.
    let sobre = B64.encode([9u8; 64]);
    let relevo = uuid::Uuid::new_v4().to_string();
    for seq in 1..=2 {
        let cuerpo = if seq == 1 {
            json!({ "tipo": "descargar", "seq": seq, "sellado": sobre, "caduca": caduca(), "relevo": { "id": relevo, "max_bytes": 3 * 1024 * 1024 } })
        } else {
            json!({ "tipo": "copiar_ahora", "seq": seq, "sellado": sobre, "caduca": caduca() })
        };
        let r = pedir(&p.app, "POST", &format!("/api/clientes/{c}/equipos/{}/ordenes", eq.id), Some(cuerpo), Some(&ana)).await;
        assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    }
    let r = pedir(
        &p.app,
        "POST",
        &format!("/api/clientes/{c}/equipos/{}/ordenes", eq.id),
        Some(json!({ "tipo": "copiar_ahora", "seq": 3, "sellado": sobre, "caduca": caduca() })),
        Some(&ana),
    )
    .await;
    assert_eq!((r.estado, r.json["error"].as_str()), (StatusCode::TOO_MANY_REQUESTS, Some("cuota")), "{}", r.json);

    // El relé: 1 MB al mes. 600 KB sí; otros 600 KB, no.
    let trozo = |n: u32| {
        let ruta = format!("/api/agente/relevos/{relevo}/trozos/{n}");
        let auth = eq.auth();
        let app = p.app.clone();
        async move { pedir_bytes(&app, "PUT", &ruta, Body::from(vec![7u8; 600 * 1024]), "application/octet-stream", None, Some(&auth)).await }
    };
    assert_eq!(trozo(0).await.estado, StatusCode::NO_CONTENT);
    let r = trozo(1).await;
    assert_eq!((r.estado, r.json["error"].as_str()), (StatusCode::FORBIDDEN, Some("cuota")), "{}", r.texto);

    // Historial: como mucho 3 entradas del equipo.
    let entradas: Vec<Value> = (0..5)
        .map(|i| json!({ "id": format!("h{i}"), "hora": (chrono::Local::now() - chrono::Duration::hours(i)).to_rfc3339(), "tipo": "copia", "resultado": "ok" }))
        .collect();
    let r = pedir_bytes(
        &p.app,
        "POST",
        "/api/agente/historial",
        Body::from(json!({ "entradas": entradas }).to_string()),
        "application/json",
        None,
        Some(&eq.auth()),
    )
    .await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.texto);
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/equipos/{}/historial", eq.id), None, Some(&ana)).await;
    assert_eq!(r.json.as_array().map(Vec::len), Some(3), "{}", r.json);

    // Las predeterminadas: las cambia solo el propietario del servidor, y valen para los demás clientes.
    let r = pedir(&p.app, "PUT", "/api/servidor/cuotas", Some(json!({ "equipos": 0 })), Some(&ana)).await;
    assert_eq!(r.json["predeterminadas"]["equipos"], 0, "0 = sin límite");
    let r = pedir(&p.app, "GET", "/api/servidor/clientes", None, Some(&ana)).await;
    let fila = &r.json["clientes"][0];
    assert_eq!((fila["efectivas"]["equipos"].as_i64(), fila["relevo_mes_bytes"].as_u64()), (Some(1), Some(600 * 1024)));
    // GET /api/servidor dice que es una consola en internet.
    let r = pedir(&p.app, "GET", "/api/servidor", None, None).await;
    assert_eq!(r.json["publico"], true);
    assert!(r.json["url_agentes"].is_null());
}
