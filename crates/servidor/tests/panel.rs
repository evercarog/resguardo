//! «Todos los clientes» (`GET /api/panel` y `GET /api/panel/progreso`, v1.3x):
//! cada cuenta ve juntos solo los clientes de los que es miembro.
//!
//! - Ana, propietaria del servidor y miembro de A y C: ve A y C; nunca B (de
//!   Bea), aunque B tenga equipos, informes y algo en marcha.
//! - Luis, solo en A (técnico): ve A y su papel; nada de C ni de B.
//! - Bea, solo en B: ve B con su equipo, su informe resumido y su progreso.
//! - Sin sesión completa, nada.

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
const EQUIPO_B: &str = "EQUIPO-SECRETO-DE-B";
const REPO_B: &str = "Nóminas secretas de B";

struct Prueba {
    _dir: tempfile::TempDir,
    st: St,
    app: Router,
}

struct Resp {
    estado: StatusCode,
    json: Value,
    texto: String,
    cookie: Option<String>,
}

async fn pedir(app: &Router, metodo: &str, ruta: &str, cuerpo: Option<Value>, cookie: Option<&str>, auth: Option<&str>) -> Resp {
    let mut r = Request::builder().method(metodo).uri(ruta).header("host", "localhost").header("content-type", "application/json");
    if metodo != "GET" {
        r = r.header("x-resguardo", "1");
    }
    if let Some(c) = cookie {
        r = r.header("cookie", c);
    }
    if let Some(a) = auth {
        r = r.header("authorization", a);
    }
    let body = cuerpo.map(|v| Body::from(v.to_string())).unwrap_or_else(Body::empty);
    let res = app.clone().oneshot(r.body(body).unwrap()).await.unwrap();
    let estado = res.status();
    let cookie = res.headers().get("set-cookie").and_then(|v| v.to_str().ok()).and_then(|v| v.split(';').next()).map(str::to_string);
    let bytes = res.into_body().collect().await.unwrap().to_bytes().to_vec();
    Resp { estado, json: serde_json::from_slice(&bytes).unwrap_or(Value::Null), texto: String::from_utf8_lossy(&bytes).to_string(), cookie }
}

fn totp(secreto: &str) -> String {
    let paso = chrono::Utc::now().timestamp() / 30;
    format!("{:06}", auth::hotp(&auth::de_base32(secreto).unwrap(), paso as u64))
}

async fn propietario_servidor(p: &Prueba) -> String {
    let codigo = api::preparar_codigo_arranque(&p.st).unwrap().unwrap();
    let cuerpo = json!({ "codigo_arranque": codigo, "correo": "ana@servidor.ejemplo.com", "nombre": "Ana", "contrasena": "una contraseña bien larga" });
    let r = pedir(&p.app, "POST", "/api/inicio", Some(cuerpo), None, None).await;
    let (cookie, secreto) = (r.cookie.unwrap(), r.json["totp"]["secreto"].as_str().unwrap().to_string());
    let r = pedir(&p.app, "POST", "/api/sesion/totp", Some(json!({ "codigo": totp(&secreto) })), Some(&cookie), None).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    r.cookie.unwrap()
}

async fn aceptar(p: &Prueba, enlace: &str, correo: &str) -> String {
    let token = enlace.split('#').nth(1).unwrap();
    let cuerpo = json!({ "token": token, "correo": correo, "nombre": correo, "contrasena": "contraseña de la invitada" });
    let r = pedir(&p.app, "POST", "/api/invitaciones/aceptar", Some(cuerpo), None, None).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let (cookie, secreto) = (r.cookie.unwrap(), r.json["totp"]["secreto"].as_str().unwrap().to_string());
    let r = pedir(&p.app, "POST", "/api/sesion/totp", Some(json!({ "codigo": totp(&secreto) })), Some(&cookie), None).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    r.cookie.unwrap()
}

/// Empareja y confirma un equipo; devuelve su cabecera `Authorization`.
async fn equipo(p: &Prueba, cookie: &str, c: &str, nombre: &str) -> (String, String) {
    let r = pedir(&p.app, "POST", &format!("/api/clientes/{c}/emparejamientos"), None, Some(cookie), None).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let (emp, codigo) = (r.json["id"].as_str().unwrap().to_string(), r.json["codigo"].as_str().unwrap().to_string());
    let box_pub = claves::public_of(&claves::new_key()).unwrap();
    let sign_pub = B64.encode(SigningKey::from_bytes(&[7u8; 32]).verifying_key().to_bytes());
    let cuerpo = json!({ "codigo_hash": mensajes::code_hash(&codigo), "nombre": nombre, "so": "windows", "version": "0.7.14", "box_pub": box_pub, "sign_pub": sign_pub, "sal_equipo": B64.encode([3u8; 16]) });
    let r = pedir(&p.app, "POST", "/api/agente/unirse", Some(cuerpo), None, None).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let (id, secreto) = (r.json["equipo_id"].as_str().unwrap().to_string(), r.json["secreto"].as_str().unwrap().to_string());
    let r =
        pedir(&p.app, "POST", &format!("/api/clientes/{c}/emparejamientos/{emp}/confirmar"), Some(json!({ "etiqueta": "ETIQUETA" })), Some(cookie), None).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    (id.clone(), format!("Equipo {id}:{secreto}"))
}

fn ids(r: &Resp) -> Vec<String> {
    r.json["clientes"].as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap().to_string()).collect()
}

#[tokio::test]
async fn cada_cuenta_ve_solo_sus_clientes() {
    let dir = tempfile::tempdir().unwrap();
    let st = preparar(dir.path(), Opciones { https: false, publico: true, ..Default::default() }).unwrap();
    let p = Prueba { app: api::router(st.clone()), st, _dir: dir };
    let ana = propietario_servidor(&p).await;

    // A y C, de Ana; Luis, técnico en A.
    let a = pedir(&p.app, "POST", "/api/clientes", Some(json!({ "nombre": "Cliente A" })), Some(&ana), None).await.json["id"].as_str().unwrap().to_string();
    let c = pedir(&p.app, "POST", "/api/clientes", Some(json!({ "nombre": "Cliente C" })), Some(&ana), None).await.json["id"].as_str().unwrap().to_string();
    let r = pedir(&p.app, "POST", &format!("/api/clientes/{a}/invitaciones"), Some(json!({ "rol": "tecnico" })), Some(&ana), None).await;
    let luis = aceptar(&p, r.json["enlace"].as_str().unwrap(), "luis@cliente-a.ejemplo.com").await;
    let (equipo_a, auth_a) = equipo(&p, &ana, &a, "EQUIPO-DE-A").await;

    // B, para Bea (Ana no es miembro), con un equipo que informa y copia ahora.
    let r = pedir(&p.app, "POST", "/api/servidor/clientes", Some(json!({ "nombre": "Cliente B" })), Some(&ana), None).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let b = r.json["id"].as_str().unwrap().to_string();
    let bea = aceptar(&p, r.json["invitacion"]["enlace"].as_str().unwrap(), "bea@cliente-b.ejemplo.com").await;
    let (equipo_b, auth_b) = equipo(&p, &bea, &b, EQUIPO_B).await;
    let hora = chrono::Local::now().to_rfc3339();
    let informe = json!({ "datos": { "repos": [{ "id": "rb", "nombre": REPO_B, "versiones": [{ "id": "0000000b", "hora": hora, "copia": "k", "total_bytes": 9 }], "ejecuciones": [] }] } });
    let r = pedir(&p.app, "POST", "/api/agente/informe", Some(informe.clone()), None, Some(&auth_b)).await;
    assert_eq!(r.estado, StatusCode::NO_CONTENT, "{}", r.texto);
    let tareas = json!({ "tareas": [{ "tipo": "copia", "fase": "subiendo", "repo": "rb", "nombre": REPO_B, "porcentaje": 0.4 }] });
    let r = pedir(&p.app, "POST", "/api/agente/progreso", Some(tareas.clone()), None, Some(&auth_b)).await;
    assert_eq!(r.estado, StatusCode::NO_CONTENT, "{}", r.texto);
    // Y en A, también un informe y una copia en marcha.
    let informe_a = json!({ "datos": { "repos": [{ "id": "ra", "nombre": "Docs A", "versiones": [], "ejecuciones": [] }] } });
    assert_eq!(pedir(&p.app, "POST", "/api/agente/informe", Some(informe_a), None, Some(&auth_a)).await.estado, StatusCode::NO_CONTENT);
    let tareas_a = json!({ "tareas": [{ "tipo": "copia", "fase": "subiendo", "repo": "ra", "porcentaje": 0.1 }] });
    assert_eq!(pedir(&p.app, "POST", "/api/agente/progreso", Some(tareas_a), None, Some(&auth_a)).await.estado, StatusCode::NO_CONTENT);

    let secretos = [EQUIPO_B, REPO_B, equipo_b.as_str(), b.as_str(), "bea@cliente-b"];
    let sin_b = |r: &Resp, quien: &str| {
        for s in secretos {
            assert!(!r.texto.contains(s), "{quien} vio «{s}»: {}", r.texto);
        }
    };

    // Ana: A y C (por nombre), con su papel; nada de B.
    let r = pedir(&p.app, "GET", "/api/panel", None, Some(&ana), None).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(ids(&r), [a.clone(), c.clone()]);
    assert_eq!(r.json["clientes"][0]["rol"], "propietario");
    assert_eq!(r.json["clientes"][0]["equipos"][0]["nombre"], "EQUIPO-DE-A");
    assert_eq!(r.json["clientes"][0]["informes"][0]["equipo"], equipo_a.as_str());
    assert_eq!(r.json["clientes"][0]["informes_completos"], true);
    assert!(r.json["clientes"][1]["equipos"].as_array().unwrap().is_empty());
    assert_eq!(r.json["omitidos"], 0);
    assert_eq!(r.json["progreso"].as_array().unwrap().len(), 1, "{}", r.json);
    assert_eq!((r.json["progreso"][0]["cliente"].as_str(), r.json["progreso"][0]["equipo"].as_str()), (Some(a.as_str()), Some(equipo_a.as_str())));
    sin_b(&r, "Ana (propietaria del servidor, no miembro de B)");
    let r = pedir(&p.app, "GET", "/api/panel/progreso", None, Some(&ana), None).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json.as_array().unwrap().len(), 1);
    sin_b(&r, "el progreso de Ana");

    // Luis: solo A, como técnico.
    let r = pedir(&p.app, "GET", "/api/panel", None, Some(&luis), None).await;
    assert_eq!(ids(&r), [a.clone()]);
    assert_eq!(r.json["clientes"][0]["rol"], "tecnico");
    assert!(!r.texto.contains(&c), "ni C");
    sin_b(&r, "Luis");

    // Bea: solo B, con su informe resumido y lo que está en marcha.
    let r = pedir(&p.app, "GET", "/api/panel", None, Some(&bea), None).await;
    assert_eq!(ids(&r), [b.clone()]);
    let cb = &r.json["clientes"][0];
    assert_eq!(cb["equipos"][0]["nombre"], EQUIPO_B);
    assert_eq!(cb["informes"][0]["datos"]["repos"][0]["nombre"], REPO_B);
    assert_eq!(r.json["progreso"][0]["tareas"][0]["porcentaje"], 0.4);
    assert!(!r.texto.contains(&a) && !r.texto.contains("EQUIPO-DE-A"), "nada de A");
    let r = pedir(&p.app, "GET", "/api/panel/progreso", None, Some(&bea), None).await;
    assert_eq!(r.json[0]["equipo"], equipo_b.as_str());

    // Al dejar de ser miembro, deja de verlo al momento (aunque lo de A siga en la memoria).
    let r = pedir(&p.app, "GET", "/api/cuenta", None, Some(&luis), None).await;
    let luis_id = r.json["id"].as_str().unwrap().to_string();
    let r = pedir(&p.app, "DELETE", &format!("/api/clientes/{a}/miembros/{luis_id}"), None, Some(&ana), None).await;
    assert_eq!(r.estado, StatusCode::NO_CONTENT, "{}", r.texto);
    let r = pedir(&p.app, "GET", "/api/panel", None, Some(&luis), None).await;
    assert_eq!(r.estado, StatusCode::OK);
    assert!(ids(&r).is_empty(), "{}", r.json);
    assert!(pedir(&p.app, "GET", "/api/panel/progreso", None, Some(&luis), None).await.json.as_array().unwrap().is_empty());

    // Sin sesión (o a medias), nada.
    for ruta in ["/api/panel", "/api/panel/progreso"] {
        let r = pedir(&p.app, "GET", ruta, None, None, None).await;
        assert_eq!(r.estado, StatusCode::UNAUTHORIZED, "{ruta}");
    }
}
