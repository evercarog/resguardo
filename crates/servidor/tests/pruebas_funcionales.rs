//! Pruebas funcionales (plan 0.8.0, E3, 14k): quién lee, quién registra, cambia y
//! borra, la política (solo administradores), lo que se rechaza (campos de más,
//! fechas futuras, equipos de otro cliente) y los adjuntos (solo PDF, PNG o JPG por
//! su contenido, siempre como descarga y con su tope).

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
async fn pruebas_funcionales_con_papeles_y_adjuntos() {
    let p = servidor();
    let ana = propietario(&p).await;
    let (c, eq) = cliente_con_equipo(&p, &ana).await;
    let lectura = invitado(&p, &ana, &c, "lectura", "lola@ejemplo.com").await;
    let tecnico = invitado(&p, &ana, &c, "tecnico", "tomas@ejemplo.com").await;
    let admin = invitado(&p, &ana, &c, "administrador", "alba@ejemplo.com").await;
    let ruta = |r: &str| format!("/api/clientes/{c}/pruebas-funcionales{r}");

    // Sin nada: la política por defecto (cada 6 meses) y al día (cuenta desde hoy).
    let (e, j, _) = pedir(&p.app, "GET", &ruta(""), None, Some(&lectura)).await;
    assert_eq!(e, StatusCode::OK, "{j}");
    assert_eq!((j["ajustes"]["activo"].as_bool(), j["ajustes"]["meses"].as_u64()), (Some(true), Some(6)));
    assert_eq!(j["estado"]["cliente"]["fase"], "al_dia");
    assert_eq!(j["pruebas"].as_array().map(Vec::len), Some(0));

    let hoy = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let prueba = json!({
        "fecha": hoy, "motivo": "programada", "alcance": "repos", "repos": [{ "equipo": eq, "repo": "docs" }],
        "que": "La carpeta de facturas del último mes", "donde": "Máquina virtual de pruebas",
        "desde": { "tipo": "espejo", "destino": "nube:almacen:Nube Sur", "nombre": "Nube Sur" },
        "resultado": "ok", "rto_min": 95, "rpo_min": 600, "incidencias": "", "notas": "Sin sorpresas.",
        "restauracion": { "equipo": eq, "id": "restauracion-0a1b2c3d" }
    });
    // La de lectura no registra; un técnico sí (y «quién» es él si no se dice).
    assert_eq!(pedir(&p.app, "POST", &ruta(""), Some(prueba.clone()), Some(&lectura)).await.0, StatusCode::FORBIDDEN);
    let (e, j, _) = pedir(&p.app, "POST", &ruta(""), Some(prueba.clone()), Some(&tecnico)).await;
    assert_eq!(e, StatusCode::OK, "{j}");
    let id = j["id"].as_str().unwrap().to_string();
    let pf = j["pruebas"].as_array().unwrap().iter().find(|x| x["id"] == id.as_str()).cloned().unwrap();
    assert_eq!((pf["quien"].as_str(), pf["por"].as_str(), pf["rto_min"].as_u64(), pf["incidencias"].as_str()), (Some("tomas"), Some("tomas"), Some(95), None));
    assert_eq!(pf["desde"]["nombre"], "Nube Sur");
    assert!(j["estado"]["cliente"]["ultima"].is_string());

    // Lo que no vale: campos de más, fecha futura, un equipo de otro cliente, una restauración rara.
    let mut mala = prueba.clone();
    mala["contrasena"] = json!("x");
    assert_eq!(pedir(&p.app, "POST", &ruta(""), Some(mala), Some(&tecnico)).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let mut futura = prueba.clone();
    futura["fecha"] = json!("2999-01-01");
    assert_eq!(pedir(&p.app, "POST", &ruta(""), Some(futura), Some(&tecnico)).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let mut ajena = prueba.clone();
    ajena["repos"] = json!([{ "equipo": "otro-equipo", "repo": "docs" }]);
    assert_eq!(pedir(&p.app, "POST", &ruta(""), Some(ajena), Some(&tecnico)).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let mut rara = prueba.clone();
    rara["restauracion"] = json!({ "equipo": eq, "id": "../x" });
    assert_eq!(pedir(&p.app, "POST", &ruta(""), Some(rara), Some(&tecnico)).await.0, StatusCode::UNPROCESSABLE_ENTITY);

    // Cambiarla: técnico; una que no existe, 404.
    let mut cambio = prueba.clone();
    cambio["resultado"] = json!("parcial");
    cambio["incidencias"] = json!("Faltó un permiso en la carpeta restaurada.");
    let (e, j, _) = pedir(&p.app, "PUT", &ruta(&format!("/{id}")), Some(cambio.clone()), Some(&tecnico)).await;
    assert_eq!(e, StatusCode::OK, "{j}");
    assert_eq!(j["pruebas"][0]["resultado"], "parcial");
    assert_eq!(pedir(&p.app, "PUT", &ruta("/no-existe"), Some(cambio), Some(&tecnico)).await.0, StatusCode::NOT_FOUND);

    // La política: solo administradores; con objetivos, cada prueba dice si los cumple.
    let pol = json!({ "activo": true, "meses": 3, "aviso_dias": 15, "rto_objetivo_min": 60, "rpo_objetivo_min": 1440, "repos": [{ "equipo": eq, "repo": "docs", "meses": 1 }] });
    assert_eq!(pedir(&p.app, "PUT", &ruta("/ajustes"), Some(pol.clone()), Some(&tecnico)).await.0, StatusCode::FORBIDDEN);
    let (e, j, _) = pedir(&p.app, "PUT", &ruta("/ajustes"), Some(pol), Some(&admin)).await;
    assert_eq!(e, StatusCode::OK, "{j}");
    assert_eq!((j["pruebas"][0]["cumple_rto"].as_bool(), j["pruebas"][0]["cumple_rpo"].as_bool()), (Some(false), Some(true)));
    assert_eq!(j["estado"]["repos"][0]["plazo"]["meses"], 1);
    assert_eq!(pedir(&p.app, "PUT", &ruta("/ajustes"), Some(json!({ "meses": 0 })), Some(&admin)).await.0, StatusCode::UNPROCESSABLE_ENTITY);

    // Adjuntos: un PDF sí (el nombre, limpio y con la extensión de lo que es); un SVG no.
    let pdf = b"%PDF-1.7\n1 0 obj\n<<>>\nendobj\ntrailer\n<<>>\n%%EOF\n".to_vec();
    let adj = |nombre: &str, b: &[u8]| json!({ "nombre": nombre, "datos": B64.encode(b) });
    assert_eq!(pedir(&p.app, "POST", &ruta(&format!("/{id}/adjuntos")), Some(adj("acta.pdf", &pdf)), Some(&lectura)).await.0, StatusCode::FORBIDDEN);
    let (e, j, _) = pedir(&p.app, "POST", &ruta(&format!("/{id}/adjuntos")), Some(adj("..\\Acta \"final\".exe", &pdf)), Some(&tecnico)).await;
    assert_eq!(e, StatusCode::OK, "{j}");
    let a = &j["pruebas"][0]["adjuntos"][0];
    assert_eq!((a["nombre"].as_str(), a["tipo"].as_str(), a["n"].as_i64()), (Some("Acta final.pdf"), Some("application/pdf"), Some(1)));
    let svg = b"<svg xmlns='http://www.w3.org/2000/svg' onload='alert(1)'/>";
    assert_eq!(pedir(&p.app, "POST", &ruta(&format!("/{id}/adjuntos")), Some(adj("x.png", svg)), Some(&tecnico)).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let grande = [b"%PDF-".as_slice(), &vec![b'x'; 2 * 1024 * 1024]].concat();
    let (e, _, _) = pedir(&p.app, "POST", &ruta(&format!("/{id}/adjuntos")), Some(adj("grande.pdf", &grande)), Some(&tecnico)).await;
    assert!(e == StatusCode::UNPROCESSABLE_ENTITY || e == StatusCode::PAYLOAD_TOO_LARGE, "{e}");
    assert_eq!(pedir(&p.app, "POST", &ruta("/no-existe/adjuntos"), Some(adj("a.pdf", &pdf)), Some(&tecnico)).await.0, StatusCode::NOT_FOUND);

    // Se descarga (la de lectura también), siempre como adjunto y sin que el navegador lo interprete.
    let res = p
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(ruta(&format!("/{id}/adjuntos/1")))
                .header("host", "localhost")
                .header("cookie", &lectura)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let h = res.headers().clone();
    assert_eq!(h.get("content-type").unwrap(), "application/pdf");
    assert!(h.get("content-disposition").unwrap().to_str().unwrap().starts_with("attachment;"));
    assert_eq!(h.get("x-content-type-options").unwrap(), "nosniff");
    assert_eq!(res.into_body().collect().await.unwrap().to_bytes().to_vec(), pdf);
    assert_eq!(pedir(&p.app, "GET", &ruta(&format!("/{id}/adjuntos/9")), None, Some(&lectura)).await.0, StatusCode::NOT_FOUND);

    // Quitar el adjunto: técnico. Borrar la prueba: administradores (no técnicos).
    assert_eq!(pedir(&p.app, "DELETE", &ruta(&format!("/{id}/adjuntos/1")), None, Some(&lectura)).await.0, StatusCode::FORBIDDEN);
    assert_eq!(pedir(&p.app, "DELETE", &ruta(&format!("/{id}/adjuntos/1")), None, Some(&tecnico)).await.0, StatusCode::OK);
    assert_eq!(pedir(&p.app, "DELETE", &ruta(&format!("/{id}")), None, Some(&tecnico)).await.0, StatusCode::FORBIDDEN);
    let (e, j, _) = pedir(&p.app, "DELETE", &ruta(&format!("/{id}")), None, Some(&admin)).await;
    assert_eq!(e, StatusCode::OK, "{j}");
    assert_eq!(j["pruebas"].as_array().map(Vec::len), Some(0));

    // Todo queda en la auditoría, sin el texto.
    let (_, j, _) = pedir(&p.app, "GET", &format!("/api/clientes/{c}/auditoria"), None, Some(&ana)).await;
    let texto = j.to_string();
    for accion in
        ["registrar_prueba_funcional", "cambiar_prueba_funcional", "ajustes_pruebas_funcionales", "adjunto_prueba_funcional", "borrar_prueba_funcional"]
    {
        assert!(texto.contains(accion), "falta {accion} en la auditoría");
    }
    assert!(!texto.contains("Faltó un permiso"), "sin el texto");

    // Quien no es del cliente no ve nada.
    let (_, j2, _) = pedir(&p.app, "POST", "/api/clientes", Some(json!({ "nombre": "Otro Ejemplo", "espera_min_horas": 24 })), Some(&ana)).await;
    let c2 = j2["id"].as_str().unwrap();
    assert_eq!(pedir(&p.app, "GET", &format!("/api/clientes/{c2}/pruebas-funcionales"), None, Some(&lectura)).await.0, StatusCode::NOT_FOUND);
}
