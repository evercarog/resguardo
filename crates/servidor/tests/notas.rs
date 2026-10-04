//! Observaciones y comentarios (v1.3x): quién lee y quién escribe, los 15
//! minutos para cambiar un comentario propio, que un propietario puede borrar
//! cualquiera, la auditoría (sin el texto) y que viajan con el paquete de
//! exportación (`POST …/importar`, campo `notas`).

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

struct Prueba {
    _dir: tempfile::TempDir,
    st: St,
    app: Router,
}

fn servidor() -> Prueba {
    let dir = tempfile::tempdir().unwrap();
    let st = preparar(dir.path(), Opciones { https: false, ..Default::default() }).unwrap();
    let app = api::router(st.clone());
    Prueba { _dir: dir, st, app }
}

async fn pedir(app: &Router, metodo: &str, ruta: &str, cuerpo: Option<Value>, cookie: Option<&str>) -> (StatusCode, Value, Option<String>) {
    let mut r = Request::builder().method(metodo).uri(ruta).header("host", "localhost");
    if metodo != "GET" {
        r = r.header("x-resguardo", "1");
    }
    if let Some(c) = cookie {
        r = r.header("cookie", c);
    }
    let body = match cuerpo {
        Some(v) => {
            r = r.header("content-type", "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    let res = app.clone().oneshot(r.body(body).unwrap()).await.unwrap();
    let estado = res.status();
    let cookie = res.headers().get("set-cookie").and_then(|v| v.to_str().ok()).and_then(|v| v.split(';').next()).map(str::to_string);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (estado, serde_json::from_slice(&bytes).unwrap_or(Value::Null), cookie)
}

fn totp(secreto: &str) -> String {
    let paso = chrono::Utc::now().timestamp() / 30;
    format!("{:06}", auth::hotp(&auth::de_base32(secreto).unwrap(), paso as u64))
}

async fn propietario(p: &Prueba) -> String {
    let codigo = api::preparar_codigo_arranque(&p.st).unwrap().unwrap();
    let cuerpo = json!({ "codigo_arranque": codigo, "correo": "ana@ejemplo.com", "nombre": "Ana", "contrasena": "una contraseña bien larga" });
    let (_, j, cookie) = pedir(&p.app, "POST", "/api/inicio", Some(cuerpo), None).await;
    let (e, j, cookie) =
        pedir(&p.app, "POST", "/api/sesion/totp", Some(json!({ "codigo": totp(j["totp"]["secreto"].as_str().unwrap()) })), cookie.as_deref()).await;
    assert_eq!(e, StatusCode::OK, "{j}");
    cookie.unwrap()
}

async fn invitado(p: &Prueba, cookie: &str, c: &str, rol: &str, correo: &str) -> String {
    let (_, j, _) = pedir(&p.app, "POST", &format!("/api/clientes/{c}/invitaciones"), Some(json!({ "rol": rol })), Some(cookie)).await;
    let token = j["enlace"].as_str().unwrap().split('#').nth(1).unwrap().to_string();
    let cuerpo = json!({ "token": token, "correo": correo, "nombre": correo.split('@').next(), "contrasena": "contraseña del invitado" });
    let (_, j, c2) = pedir(&p.app, "POST", "/api/invitaciones/aceptar", Some(cuerpo), None).await;
    let (e, j, c3) = pedir(&p.app, "POST", "/api/sesion/totp", Some(json!({ "codigo": totp(j["totp"]["secreto"].as_str().unwrap()) })), c2.as_deref()).await;
    assert_eq!(e, StatusCode::OK, "{j}");
    c3.unwrap()
}

/// Un cliente con un equipo emparejado y confirmado: (cliente, equipo).
async fn cliente_con_equipo(p: &Prueba, cookie: &str) -> (String, String) {
    let (_, j, _) = pedir(&p.app, "POST", "/api/clientes", Some(json!({ "nombre": "Panadería Ejemplo", "espera_min_horas": 24 })), Some(cookie)).await;
    let c = j["id"].as_str().unwrap().to_string();
    let (_, j, _) = pedir(&p.app, "POST", &format!("/api/clientes/{c}/emparejamientos"), None, Some(cookie)).await;
    let (emp, codigo) = (j["id"].as_str().unwrap().to_string(), j["codigo"].as_str().unwrap().to_string());
    let box_pub = claves::public_of(&claves::new_key()).unwrap();
    let sign_pub = B64.encode(SigningKey::from_bytes(&[7u8; 32]).verifying_key().to_bytes());
    let cuerpo = json!({ "codigo_hash": mensajes::code_hash(&codigo), "nombre": "CAJA-1", "so": "windows", "version": "0.7.0", "box_pub": box_pub, "sign_pub": sign_pub, "sal_equipo": B64.encode([3u8; 16]) });
    let (e, j, _) = pedir(&p.app, "POST", "/api/agente/unirse", Some(cuerpo), None).await;
    assert_eq!(e, StatusCode::OK, "{j}");
    let equipo = j["equipo_id"].as_str().unwrap().to_string();
    let (e, j, _) =
        pedir(&p.app, "POST", &format!("/api/clientes/{c}/emparejamientos/{emp}/confirmar"), Some(json!({ "etiqueta": "ETIQUETA" })), Some(cookie)).await;
    assert_eq!(e, StatusCode::OK, "{j}");
    (c, equipo)
}

#[tokio::test]
async fn observaciones_y_comentarios_con_papeles() {
    let p = servidor();
    let ana = propietario(&p).await;
    let (c, eq) = cliente_con_equipo(&p, &ana).await;
    let lectura = invitado(&p, &ana, &c, "lectura", "lola@ejemplo.com").await;
    let tecnico = invitado(&p, &ana, &c, "tecnico", "tomas@ejemplo.com").await;
    let tecnica2 = invitado(&p, &ana, &c, "tecnico", "teresa@ejemplo.com").await;
    let ruta = |r: &str| format!("/api/clientes/{c}/notas{r}");

    // Observación de una copia del equipo (técnico); la de lectura no puede escribir.
    let obs = json!({ "tipo": "copia", "objeto": format!("{eq}/diaria"), "texto": "  **Ojo**: la base va aparte\r\n- llamar a Luis si falla  " });
    let (e, _, _) = pedir(&p.app, "PUT", &ruta("/observacion"), Some(obs.clone()), Some(&lectura)).await;
    assert_eq!(e, StatusCode::FORBIDDEN);
    let (e, j, _) = pedir(&p.app, "PUT", &ruta("/observacion"), Some(obs), Some(&tecnico)).await;
    assert_eq!(e, StatusCode::OK, "{j}");
    assert_eq!(j["texto"], "**Ojo**: la base va aparte\n- llamar a Luis si falla");
    assert_eq!(j["por"], "tomas");

    // Objetos que no valen: otro cliente, un equipo que no existe, sin «<equipo>/», un tipo raro, demasiado largo.
    for (tipo, objeto) in [("cliente", "otro"), ("equipo", "no-existe"), ("repositorio", "sin-barra"), ("raro", "x")] {
        let (e, j, _) = pedir(&p.app, "PUT", &ruta("/observacion"), Some(json!({ "tipo": tipo, "objeto": objeto, "texto": "x" })), Some(&tecnico)).await;
        assert!(e == StatusCode::UNPROCESSABLE_ENTITY || e == StatusCode::NOT_FOUND, "{tipo}/{objeto}: {e} {j}");
    }
    let largo = json!({ "tipo": "cliente", "objeto": c, "texto": "a".repeat(2001) });
    assert_eq!(pedir(&p.app, "PUT", &ruta("/observacion"), Some(largo), Some(&tecnico)).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let (e, _, _) =
        pedir(&p.app, "PUT", &ruta("/observacion"), Some(json!({ "tipo": "cliente", "objeto": c, "texto": "Contrato hasta 2027" })), Some(&ana)).await;
    assert_eq!(e, StatusCode::OK);

    // Comentarios: los escribe un técnico; la de lectura los lee.
    let (e, k1, _) =
        pedir(&p.app, "POST", &ruta("/comentarios"), Some(json!({ "tipo": "equipo", "objeto": eq, "texto": "Cambié el disco el 3/10" })), Some(&tecnico)).await;
    assert_eq!(e, StatusCode::OK, "{k1}");
    assert_eq!((k1["editable"].as_bool(), k1["borrable"].as_bool()), (Some(true), Some(true)));
    let id1 = k1["id"].as_str().unwrap().to_string();
    let (e, _, _) = pedir(&p.app, "POST", &ruta("/comentarios"), Some(json!({ "tipo": "equipo", "objeto": eq, "texto": "   " })), Some(&tecnico)).await;
    assert_eq!(e, StatusCode::UNPROCESSABLE_ENTITY, "vacío no");
    let (e, _, _) = pedir(&p.app, "POST", &ruta("/comentarios"), Some(json!({ "tipo": "equipo", "objeto": eq, "texto": "hola" })), Some(&lectura)).await;
    assert_eq!(e, StatusCode::FORBIDDEN);
    let (_, k2, _) = pedir(&p.app, "POST", &ruta("/comentarios"), Some(json!({ "tipo": "equipo", "objeto": eq, "texto": "Lo vi" })), Some(&tecnica2)).await;
    let id2 = k2["id"].as_str().unwrap().to_string();

    let ver = format!("/api/clientes/{c}/notas/objeto?tipo=equipo&objeto={eq}");
    let (e, j, _) = pedir(&p.app, "GET", &ver, None, Some(&lectura)).await;
    assert_eq!(e, StatusCode::OK, "{j}");
    let coms = j["comentarios"].as_array().unwrap();
    assert_eq!(coms.len(), 2);
    assert_eq!(coms[0]["texto"], "Cambié el disco el 3/10");
    assert_eq!(coms[0]["autor"]["nombre"], "tomas");
    assert!(coms.iter().all(|k| k["editable"] == false && k["borrable"] == false), "la de lectura no toca nada");
    assert_eq!(j["minutos_edicion"], 15);

    // Solo su autor lo cambia; otra técnica no; un propietario lo borra.
    let (e, _, _) = pedir(&p.app, "PATCH", &ruta(&format!("/comentarios/{id1}")), Some(json!({ "texto": "otro" })), Some(&tecnica2)).await;
    assert_eq!(e, StatusCode::FORBIDDEN);
    let (e, j, _) =
        pedir(&p.app, "PATCH", &ruta(&format!("/comentarios/{id1}")), Some(json!({ "texto": "Cambié el disco el 3/10 (Samsung)" })), Some(&tecnico)).await;
    assert_eq!(e, StatusCode::OK, "{j}");
    assert!(j["editado"].is_string());
    assert_eq!(pedir(&p.app, "DELETE", &ruta(&format!("/comentarios/{id2}")), None, Some(&tecnico)).await.0, StatusCode::FORBIDDEN);
    assert_eq!(pedir(&p.app, "DELETE", &ruta(&format!("/comentarios/{id2}")), None, Some(&ana)).await.0, StatusCode::NO_CONTENT);
    assert_eq!(pedir(&p.app, "DELETE", &ruta(&format!("/comentarios/{id2}")), None, Some(&ana)).await.0, StatusCode::NOT_FOUND);

    // El índice: contadores y el título de la observación (sin las marcas).
    let (_, j, _) = pedir(&p.app, "GET", &ruta(""), None, Some(&lectura)).await;
    let objs = j["objetos"].as_array().unwrap();
    let de = |t: &str| objs.iter().find(|o| o["tipo"] == t).cloned().unwrap();
    assert_eq!(de("copia")["titulo"], "Ojo: la base va aparte");
    assert_eq!(de("equipo")["comentarios"], 1);
    assert_eq!(de("equipo")["titulo"], Value::Null);
    assert_eq!(de("cliente")["titulo"], "Contrato hasta 2027");

    // Para exportar hace falta ser administrador.
    assert_eq!(pedir(&p.app, "GET", &ruta("/todas"), None, Some(&tecnico)).await.0, StatusCode::FORBIDDEN);
    let (e, todas, _) = pedir(&p.app, "GET", &ruta("/todas"), None, Some(&ana)).await;
    assert_eq!(e, StatusCode::OK);
    assert_eq!((todas["observaciones"].as_array().unwrap().len(), todas["comentarios"].as_array().unwrap().len()), (2, 1));

    // La auditoría lo cuenta, sin el texto.
    let (_, aud, _) = pedir(&p.app, "GET", &format!("/api/clientes/{c}/auditoria?limite=200"), None, Some(&ana)).await;
    let texto = aud.to_string();
    for accion in ["poner_observacion", "comentar", "editar_comentario", "borrar_comentario"] {
        assert!(texto.contains(accion), "falta {accion}");
    }
    assert!(!texto.contains("Samsung") && !texto.contains("Luis"), "el texto no va a la auditoría");

    // Vacía: se borra.
    let (e, j, _) = pedir(&p.app, "PUT", &ruta("/observacion"), Some(json!({ "tipo": "cliente", "objeto": c, "texto": "" })), Some(&ana)).await;
    assert_eq!((e, j), (StatusCode::OK, Value::Null));

    // Con el paquete de exportación, a otro cliente (importar: solo el propietario, una vez).
    let (_, j, _) = pedir(&p.app, "POST", "/api/clientes", Some(json!({ "nombre": "Panadería Nueva", "espera_min_horas": 24 })), Some(&ana)).await;
    let c2 = j["id"].as_str().unwrap();
    let mut notas = todas.clone();
    notas["observaciones"].as_array_mut().unwrap().push(json!({ "tipo": "cliente", "objeto": c2, "texto": "a\u{0007}b", "actualizada": 1 }));
    let cuerpo = json!({ "origen": "https://antigua.ejemplo.com", "auditoria": [], "notas": notas });
    let (e, j, _) = pedir(&p.app, "POST", &format!("/api/clientes/{c2}/importar"), Some(cuerpo), Some(&ana)).await;
    assert_eq!(e, StatusCode::OK, "{j}");
    assert_eq!((j["observaciones"].as_u64(), j["comentarios"].as_u64()), (Some(2), Some(1)), "la que lleva un carácter de control se salta");
    let (_, j, _) = pedir(&p.app, "GET", &format!("/api/clientes/{c2}/notas/objeto?tipo=equipo&objeto={eq}"), None, Some(&ana)).await;
    let k = &j["comentarios"][0];
    assert_eq!((k["autor"]["nombre"].as_str(), k["editable"].as_bool(), k["borrable"].as_bool()), (Some("tomas"), Some(false), Some(true)));
}
