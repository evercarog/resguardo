//! Pruebas de integración de Resguardo Server: el servidor completo con un
//! SQLite temporal, la consola por HTTP (sin red, `oneshot`) y un agente de
//! prueba por HTTP y por WebSocket (red local real).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey, Verifier};
use http_body_util::BodyExt;
use resguardo_protocolo::{claves, derivaciones, mensajes};
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

struct Resp {
    estado: StatusCode,
    json: Value,
    cookie: Option<String>,
    bytes: Vec<u8>,
}

async fn pedir(app: &Router, metodo: &str, ruta: &str, cuerpo: Option<Value>, cookie: Option<&str>, extra: &[(&str, &str)]) -> Resp {
    let mut r = Request::builder().method(metodo).uri(ruta).header("host", "localhost");
    if metodo != "GET" {
        r = r.header("x-resguardo", "1");
    }
    if let Some(c) = cookie {
        r = r.header("cookie", c);
    }
    for (k, v) in extra {
        r = r.header(*k, *v);
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
    let bytes = res.into_body().collect().await.unwrap().to_bytes().to_vec();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    Resp { estado, json, cookie, bytes }
}

/// Una petición con el cuerpo tal cual (sin pasar por `serde_json`) y la respuesta entera.
async fn pedir_crudo(app: &Router, metodo: &str, ruta: &str, cuerpo: Vec<u8>, cabeceras: &[(&str, &str)]) -> axum::http::Response<Body> {
    let mut r = Request::builder().method(metodo).uri(ruta).header("host", "localhost").header("content-type", "application/json");
    for (k, v) in cabeceras {
        r = r.header(*k, *v);
    }
    app.clone().oneshot(r.body(Body::from(cuerpo)).unwrap()).await.unwrap()
}

fn codigo_totp(secreto: &str, desfase: i64) -> String {
    let paso = chrono::Utc::now().timestamp() / 30 + desfase;
    format!("{:06}", auth::hotp(&auth::de_base32(secreto).unwrap(), paso as u64))
}

/// Primer arranque: propietario del servidor con su sesión completa.
async fn propietario(p: &Prueba) -> String {
    propietario_y_totp(p).await.0
}

/// Lo mismo, con el secreto TOTP (para las pruebas que vuelven a autenticar).
async fn propietario_y_totp(p: &Prueba) -> (String, String) {
    let codigo = api::preparar_codigo_arranque(&p.st).unwrap().unwrap();
    let r = pedir(
        &p.app,
        "POST",
        "/api/inicio",
        Some(json!({ "codigo_arranque": codigo, "correo": "ana@ejemplo.com", "nombre": "Ana", "contrasena": "una contraseña bien larga" })),
        None,
        &[],
    )
    .await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let cookie = r.cookie.unwrap();
    let secreto = r.json["totp"]["secreto"].as_str().unwrap().to_string();
    // Sin el TOTP, la sesión no sirve.
    assert_eq!(pedir(&p.app, "GET", "/api/cuenta", None, Some(&cookie), &[]).await.estado, StatusCode::UNAUTHORIZED);
    let r = pedir(&p.app, "POST", "/api/sesion/totp", Some(json!({ "codigo": codigo_totp(&secreto, 0) })), Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["codigos_recuperacion"].as_array().unwrap().len(), 10);
    // La sesión completa lleva otra ficha: la de la contraseña sola ya no vale.
    let completa = r.cookie.unwrap();
    assert_ne!(completa, cookie);
    assert_eq!(pedir(&p.app, "GET", "/api/cuenta", None, Some(&cookie), &[]).await.estado, StatusCode::UNAUTHORIZED);
    (completa, secreto)
}

/// Invita a alguien con ese rol y devuelve su sesión completa.
async fn invitado(p: &Prueba, cookie: &str, c: &str, rol: &str) -> String {
    invitado_y_totp(p, cookie, c, rol).await.0
}

/// Lo mismo, con el secreto TOTP. La cuenta es `<rol>@ejemplo.com`.
async fn invitado_y_totp(p: &Prueba, cookie: &str, c: &str, rol: &str) -> (String, String) {
    let r = pedir(&p.app, "POST", &format!("/api/clientes/{c}/invitaciones"), Some(json!({ "rol": rol })), Some(cookie), &[]).await;
    let token = r.json["enlace"].as_str().unwrap().split('#').nth(1).unwrap().to_string();
    let r = pedir(
        &p.app,
        "POST",
        "/api/invitaciones/aceptar",
        Some(json!({ "token": token, "correo": format!("{rol}@ejemplo.com"), "nombre": rol, "contrasena": "contraseña del invitado" })),
        None,
        &[],
    )
    .await;
    let cookie2 = r.cookie.unwrap();
    let secreto = r.json["totp"]["secreto"].as_str().unwrap().to_string();
    let r = pedir(&p.app, "POST", "/api/sesion/totp", Some(json!({ "codigo": codigo_totp(&secreto, 0) })), Some(&cookie2), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    (r.cookie.unwrap(), secreto)
}

/// Un agente de prueba con sus claves.
struct Agente {
    id: String,
    secreto: String,
    box_secret: String,
    box_pub: String,
    firma: SigningKey,
}

impl Agente {
    fn nuevo() -> Self {
        let box_secret = claves::new_key();
        let box_pub = claves::public_of(&box_secret).unwrap();
        Agente { id: String::new(), secreto: String::new(), box_secret, box_pub, firma: SigningKey::from_bytes(&[11u8; 32]) }
    }
    fn sign_pub(&self) -> String {
        B64.encode(self.firma.verifying_key().to_bytes())
    }
    fn auth(&self) -> String {
        format!("Equipo {}:{}", self.id, self.secreto)
    }
    fn firmar_resultado(&self, orden: &str, seq: u64, estado: &str, mensaje: Option<&str>) -> String {
        B64.encode(self.firma.sign(derivaciones::texto_resultado(orden, seq, estado, mensaje, None).as_bytes()).to_bytes())
    }
}

/// Cliente «Café del Sur» con un equipo emparejado y confirmado. Devuelve (cliente, agente).
async fn cliente_con_equipo(p: &Prueba, cookie: &str) -> (String, Agente) {
    let r = pedir(&p.app, "POST", "/api/clientes", Some(json!({ "nombre": "Café del Sur", "espera_min_horas": 24 })), Some(cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let c = r.json["id"].as_str().unwrap().to_string();
    let r = pedir(&p.app, "POST", &format!("/api/clientes/{c}/emparejamientos"), None, Some(cookie), &[]).await;
    let (emp, codigo) = (r.json["id"].as_str().unwrap().to_string(), r.json["codigo"].as_str().unwrap().to_string());
    let mut ag = Agente::nuevo();
    let r = pedir(
        &p.app,
        "POST",
        "/api/agente/unirse",
        Some(json!({ "codigo_hash": mensajes::code_hash(&codigo), "nombre": "RECEPCION", "so": "windows", "version": "0.7.0", "box_pub": ag.box_pub, "sign_pub": ag.sign_pub(), "sal_equipo": B64.encode([3u8; 16]) })),
        None,
        &[],
    )
    .await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    ag.id = r.json["equipo_id"].as_str().unwrap().into();
    ag.secreto = r.json["secreto"].as_str().unwrap().into();
    let sas_agente = r.json["sas"].as_str().unwrap().to_string();
    // El mismo código sirve una sola vez.
    let otra = pedir(&p.app, "POST", "/api/agente/unirse", Some(json!({ "codigo_hash": mensajes::code_hash(&codigo), "nombre": "X", "so": "w", "version": "1", "box_pub": ag.box_pub, "sign_pub": ag.sign_pub(), "sal_equipo": B64.encode([3u8; 16]) })), None, &[]).await;
    assert_eq!(otra.estado, StatusCode::NOT_FOUND);
    // La consola ve el mismo SAS.
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/emparejamientos/{emp}"), None, Some(cookie), &[]).await;
    assert_eq!(r.json["estado"], "unido");
    assert_eq!(r.json["sas"], sas_agente);
    assert_eq!(sas_agente, derivaciones::sas_v2(&p.st.identidad_pub, &ag.box_pub, &ag.sign_pub()));
    // Un agente anterior a 0.7.10 no anuncia la versión: SAS v2 (la consola avisa de que es antiguo).
    assert_eq!(r.json["sas_version"], 2);
    let r =
        pedir(&p.app, "POST", &format!("/api/clientes/{c}/emparejamientos/{emp}/confirmar"), Some(json!({ "etiqueta": "ETIQUETA" })), Some(cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["confirmado"], true);
    (c, ag)
}

/// v1.26: un agente que anuncia `sas_version: 3` y el SAS que incluye la huella de la autoridad TLS.
#[tokio::test]
async fn emparejamiento_con_sas_v3() {
    let dir = tempfile::tempdir().unwrap();
    let huella = "3F:A1:09:7C:52:E4:8B:D0:16:2A:9E:C3:75:0B:F8:44:D9:61:2C:A7:30:5E:B2:8F:E1:47:0A:96:CD:13:7B:58";
    std::fs::create_dir_all(dir.path().join("tls")).unwrap();
    std::fs::write(
        dir.path().join("tls").join("ca.huella"),
        format!(
            "{huella}
"
        ),
    )
    .unwrap();
    let st = preparar(dir.path(), Opciones { https: false, ..Default::default() }).unwrap();
    let p = Prueba { app: api::router(st.clone()), st, _dir: dir };
    assert_eq!(p.st.huella_ca, huella);
    let cookie = propietario(&p).await;
    let r = pedir(&p.app, "POST", "/api/clientes", Some(json!({ "nombre": "Café del Sur", "espera_min_horas": 24 })), Some(&cookie), &[]).await;
    let c = r.json["id"].as_str().unwrap().to_string();
    let r = pedir(&p.app, "POST", &format!("/api/clientes/{c}/emparejamientos"), None, Some(&cookie), &[]).await;
    let (emp, codigo) = (r.json["id"].as_str().unwrap().to_string(), r.json["codigo"].as_str().unwrap().to_string());
    let ag = Agente::nuevo();
    let cuerpo = json!({ "codigo_hash": mensajes::code_hash(&codigo), "nombre": "RECEPCION", "so": "windows", "version": "0.7.10", "box_pub": ag.box_pub, "sign_pub": ag.sign_pub(), "sal_equipo": B64.encode([3u8; 16]), "sas_version": 3 });
    let r = pedir(&p.app, "POST", "/api/agente/unirse", Some(cuerpo), None, &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let v3 = derivaciones::sas_v3(&p.st.identidad_pub, &ag.box_pub, &ag.sign_pub(), huella);
    assert_eq!(r.json["sas"], v3.as_str());
    assert_eq!(r.json["sas_version"], 3);
    assert_ne!(v3, derivaciones::sas_v2(&p.st.identidad_pub, &ag.box_pub, &ag.sign_pub()));
    // La consola ve el mismo número y que es v3 (lo calcula con `huella_ca` de GET /api/servidor).
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/emparejamientos/{emp}"), None, Some(&cookie), &[]).await;
    assert_eq!((r.json["sas"].as_str(), r.json["sas_version"].as_i64()), (Some(v3.as_str()), Some(3)));
    let r = pedir(&p.app, "GET", "/api/servidor", None, None, &[]).await;
    assert_eq!(r.json["huella_ca"], huella);
    // Un agente que fijó otra autoridad (alguien en medio) mostraría otro número.
    let otra = format!("{}9", &huella[..huella.len() - 1]);
    assert_ne!(derivaciones::sas_v3(&p.st.identidad_pub, &ag.box_pub, &ag.sign_pub(), &otra), v3);
}

fn caduca(horas: i64) -> String {
    (chrono::Local::now() + chrono::Duration::hours(horas)).to_rfc3339()
}

fn sobre(ag: &Agente) -> String {
    claves::seal_bytes(&ag.box_pub, br#"{"v":2}"#).unwrap()
}

#[tokio::test]
async fn cuentas_csrf_y_errores() {
    let p = servidor();
    // Sin cuentas: el servidor dice que no está inicializado.
    let r = pedir(&p.app, "GET", "/api/servidor", None, None, &[]).await;
    assert_eq!(r.json["inicializado"], false);
    // La app key pública de Dropbox: la de la app «Resguardo» salvo que se compile con otra (o vacía).
    match option_env!("RESGUARDO_DROPBOX_APP_KEY") {
        None => assert_eq!(r.json["dropbox_app_key"], "beobf3c13cvlrup"),
        Some(k) if k.trim().is_empty() => assert!(r.json.get("dropbox_app_key").is_none()),
        Some(k) => assert_eq!(r.json["dropbox_app_key"], k.trim()),
    }
    assert_eq!(r.json["identidad"], p.st.identidad_pub.as_str());
    let cookie = propietario(&p).await;
    let r = pedir(&p.app, "GET", "/api/cuenta", None, Some(&cookie), &[]).await;
    assert_eq!(r.json["correo"], "ana@ejemplo.com");
    assert_eq!(r.json["superusuario"], true);
    // Un segundo /api/inicio no vale.
    let r = pedir(
        &p.app,
        "POST",
        "/api/inicio",
        Some(json!({ "codigo_arranque": "X", "correo": "b@b.co", "nombre": "B", "contrasena": "otra contraseña larga" })),
        None,
        &[],
    )
    .await;
    assert_eq!(r.estado, StatusCode::CONFLICT);
    // CSRF: sin la cabecera, rechazado.
    let req = Request::builder()
        .method("POST")
        .uri("/api/clientes")
        .header("cookie", &cookie)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"nombre":"X"}"#))
        .unwrap();
    assert_eq!(p.app.clone().oneshot(req).await.unwrap().status(), StatusCode::FORBIDDEN);
    // Origen ajeno, también.
    let r = pedir(&p.app, "POST", "/api/clientes", Some(json!({ "nombre": "X" })), Some(&cookie), &[("origin", "https://malo.example")]).await;
    assert_eq!(r.estado, StatusCode::FORBIDDEN);
    // Contraseña incorrecta: mismo mensaje que un correo inexistente.
    let a = pedir(&p.app, "POST", "/api/sesion", Some(json!({ "correo": "ana@ejemplo.com", "contrasena": "mala" })), None, &[]).await;
    let b = pedir(&p.app, "POST", "/api/sesion", Some(json!({ "correo": "nadie@ejemplo.com", "contrasena": "mala" })), None, &[]).await;
    assert_eq!((a.estado, b.estado), (StatusCode::UNAUTHORIZED, StatusCode::UNAUTHORIZED));
    assert_eq!(a.json["mensaje"], b.json["mensaje"]);
    // Ruta desconocida de la API: JSON 404.
    let r = pedir(&p.app, "GET", "/api/no-existe", None, None, &[]).await;
    assert_eq!((r.estado, r.json["error"].as_str()), (StatusCode::NOT_FOUND, Some("no_existe")));
    // Cabeceras de seguridad.
    let res = p.app.clone().oneshot(Request::builder().uri("/").body(Body::empty()).unwrap()).await.unwrap();
    let csp = res.headers().get("content-security-policy").unwrap().to_str().unwrap().to_string();
    assert!(csp.contains("frame-ancestors 'none'"));
    // La consola cambia el código de Dropbox por el token desde el navegador: solo ese origen de fuera.
    assert!(csp.contains("connect-src 'self' https://api.dropboxapi.com;"), "{csp}");
    assert!(csp.contains("default-src 'self';") && csp.contains("script-src 'self' 'wasm-unsafe-eval';"), "{csp}");
}

#[tokio::test]
async fn emparejamiento_ordenes_y_roles() {
    let p = servidor();
    let cookie = propietario(&p).await;
    let (c, ag) = cliente_con_equipo(&p, &cookie).await;
    let ordenes = format!("/api/clientes/{c}/equipos/{}/ordenes", ag.id);

    // Una orden inofensiva.
    let r = pedir(&p.app, "POST", &ordenes, Some(json!({ "tipo": "copiar_ahora", "seq": 1, "sellado": sobre(&ag), "caduca": caduca(1) })), Some(&cookie), &[])
        .await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let orden = r.json["id"].as_str().unwrap().to_string();
    // seq repetido: conflicto con el siguiente.
    let r = pedir(&p.app, "POST", &ordenes, Some(json!({ "tipo": "copiar_ahora", "seq": 1, "sellado": sobre(&ag), "caduca": caduca(1) })), Some(&cookie), &[])
        .await;
    assert_eq!((r.estado, r.json["siguiente_seq"].as_u64()), (StatusCode::CONFLICT, Some(2)));
    // Tipo desconocido.
    let r =
        pedir(&p.app, "POST", &ordenes, Some(json!({ "tipo": "formatear", "seq": 2, "sellado": sobre(&ag), "caduca": caduca(1) })), Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY);

    // El agente la recoge (sondeo) y comprueba la identidad del servidor.
    let reto = B64.encode([42u8; 32]);
    let r = pedir(&p.app, "POST", "/api/agente/tomar", Some(json!({ "reto": reto })), None, &[("authorization", &ag.auth())]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let firma: [u8; 64] = B64.decode(r.json["firma"].as_str().unwrap()).unwrap().try_into().unwrap();
    let identidad: [u8; 32] = B64.decode(&p.st.identidad_pub).unwrap().try_into().unwrap();
    ed25519_dalek::VerifyingKey::from_bytes(&identidad)
        .unwrap()
        .verify(derivaciones::texto_identidad_servidor(&reto, &ag.id).as_bytes(), &ed25519_dalek::Signature::from_bytes(&firma))
        .expect("firma de identidad del servidor");
    let recibidas = r.json["ordenes"].as_array().unwrap();
    assert_eq!(recibidas.len(), 1);
    // Y el sobre que le llega es el que se selló para él.
    assert_eq!(claves::open_bytes(&ag.box_secret, recibidas[0]["sellado"].as_str().unwrap()).unwrap(), br#"{"v":2}"#);
    // Un secreto equivocado no entra.
    let r = pedir(&p.app, "POST", "/api/agente/tomar", Some(json!({ "reto": reto })), None, &[("authorization", &format!("Equipo {}:mal", ag.id))]).await;
    assert_eq!(r.estado, StatusCode::UNAUTHORIZED);

    // Resultado firmado (con una firma falsa se rechaza).
    let r = pedir(
        &p.app,
        "POST",
        "/api/agente/resultado",
        Some(json!({ "orden": orden, "estado": "hecha", "mensaje": "Copia hecha.", "firma": B64.encode([0u8; 64]) })),
        None,
        &[("authorization", &ag.auth())],
    )
    .await;
    assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY);
    let firma = ag.firmar_resultado(&orden, 1, "hecha", Some("Copia hecha."));
    let r = pedir(
        &p.app,
        "POST",
        "/api/agente/resultado",
        Some(json!({ "orden": orden, "estado": "hecha", "mensaje": "Copia hecha.", "firma": firma })),
        None,
        &[("authorization", &ag.auth())],
    )
    .await;
    assert_eq!(r.estado, StatusCode::NO_CONTENT, "{}", r.json);
    let r = pedir(&p.app, "GET", &ordenes, None, Some(&cookie), &[]).await;
    assert_eq!(r.json[0]["estado"], "hecha");
    assert_eq!(r.json[0]["emitida_por"]["nombre"], "Ana");

    // Destructiva: sin espera, rechazada; con ella, queda pendiente y se puede cancelar.
    let r = pedir(&p.app, "POST", &ordenes, Some(json!({ "tipo": "pausar", "seq": 2, "sellado": sobre(&ag), "caduca": caduca(48) })), Some(&cookie), &[]).await;
    assert_eq!((r.estado, r.json["espera_min_horas"].as_i64()), (StatusCode::UNPROCESSABLE_ENTITY, Some(24)));
    let r = pedir(
        &p.app,
        "POST",
        &ordenes,
        Some(json!({ "tipo": "pausar", "seq": 2, "sellado": sobre(&ag), "caduca": caduca(48), "not_before": caduca(25) })),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let destructiva = r.json["id"].as_str().unwrap().to_string();
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/ordenes?pendientes=1"), None, Some(&cookie), &[]).await;
    assert_eq!(r.json.as_array().unwrap().len(), 1);
    // No se entrega antes de su hora.
    let r = pedir(&p.app, "POST", "/api/agente/tomar", Some(json!({ "reto": reto })), None, &[("authorization", &ag.auth())]).await;
    assert!(r.json["ordenes"].as_array().unwrap().is_empty());
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/avisos?abiertos=1"), None, Some(&cookie), &[]).await;
    assert!(r.json.as_array().unwrap().iter().any(|a| a["tipo"] == "orden_destructiva"));
    let r = pedir(&p.app, "POST", &format!("/api/clientes/{c}/ordenes/{destructiva}/cancelar"), None, Some(&cookie), &[]).await;
    assert_eq!((r.estado, r.json["estado"].as_str()), (StatusCode::OK, Some("cancelada")));
    // Destructiva según el cuerpo: si declara not_before, tiene que respetar la espera desde ahora
    // (si no, un `emitida` atrasado dentro del sobre la haría aplicarse al momento).
    for tipo in ["guarda_copias", "restaurar", "cambiar_espera"] {
        let r = pedir(
            &p.app,
            "POST",
            &ordenes,
            Some(json!({ "tipo": tipo, "seq": 3, "sellado": sobre(&ag), "caduca": caduca(48), "not_before": caduca(1) })),
            Some(&cookie),
            &[],
        )
        .await;
        assert_eq!((r.estado, r.json["espera_min_horas"].as_i64()), (StatusCode::UNPROCESSABLE_ENTITY, Some(24)), "{tipo}");
    }

    // Roles: un técnico invitado no puede dar de baja equipos; uno de lectura, nada.
    for (rol, tipo, esperado) in [("tecnico", "baja_equipo", StatusCode::FORBIDDEN), ("lectura", "copiar_ahora", StatusCode::FORBIDDEN)] {
        let r = pedir(&p.app, "POST", &format!("/api/clientes/{c}/invitaciones"), Some(json!({ "rol": rol })), Some(&cookie), &[]).await;
        let token = r.json["enlace"].as_str().unwrap().split('#').nth(1).unwrap().to_string();
        let correo = format!("{rol}@ejemplo.com");
        let r = pedir(
            &p.app,
            "POST",
            "/api/invitaciones/aceptar",
            Some(json!({ "token": token, "correo": correo, "nombre": rol, "contrasena": "contraseña del invitado" })),
            None,
            &[],
        )
        .await;
        assert_eq!(r.json["necesita"], "alta_totp", "{}", r.json);
        let cookie2 = r.cookie.unwrap();
        let secreto = r.json["totp"]["secreto"].as_str().unwrap().to_string();
        let r = pedir(&p.app, "POST", "/api/sesion/totp", Some(json!({ "codigo": codigo_totp(&secreto, 0) })), Some(&cookie2), &[]).await;
        assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
        let cookie2 = r.cookie.unwrap();
        let r = pedir(
            &p.app,
            "POST",
            &ordenes,
            Some(json!({ "tipo": tipo, "seq": 3, "sellado": sobre(&ag), "caduca": caduca(48), "not_before": caduca(25) })),
            Some(&cookie2),
            &[],
        )
        .await;
        assert_eq!(r.estado, esperado, "{rol}: {}", r.json);
        // Uno de lectura no puede dar por visto un aviso (p. ej. el de una orden destructiva).
        if rol == "lectura" {
            let avisos = pedir(&p.app, "GET", &format!("/api/clientes/{c}/avisos?abiertos=1"), None, Some(&cookie), &[]).await;
            let a = avisos.json[0]["id"].as_str().unwrap().to_string();
            let r = pedir(&p.app, "POST", &format!("/api/clientes/{c}/avisos/{a}/visto"), None, Some(&cookie2), &[]).await;
            assert_eq!(r.estado, StatusCode::FORBIDDEN, "{}", r.json);
        }
        // Y no ve otros clientes.
        let r = pedir(&p.app, "GET", "/api/clientes/otro-id/equipos", None, Some(&cookie2), &[]).await;
        assert_eq!(r.estado, StatusCode::NOT_FOUND);
    }

    // La auditoría está entera.
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/auditoria/verificar"), None, Some(&cookie), &[]).await;
    assert_eq!(r.json["ok"], true, "{}", r.json);
    assert!(r.json["entradas"].as_i64().unwrap() >= 6);
}

#[tokio::test]
async fn sesion_interactiva() {
    let p = servidor();
    let cookie = propietario(&p).await;
    let (c, ag) = cliente_con_equipo(&p, &cookie).await;
    let ordenes = format!("/api/clientes/{c}/equipos/{}/ordenes", ag.id);
    let sesion = uuid::Uuid::new_v4().to_string();
    let r = pedir(
        &p.app,
        "POST",
        &ordenes,
        Some(json!({ "tipo": "explorar", "seq": 1, "sellado": sobre(&ag), "caduca": caduca(1), "sesion": sesion })),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    // El agente sabe que hay una sesión abierta y escribe en ella.
    let r = pedir(&p.app, "POST", "/api/agente/tomar", Some(json!({ "reto": B64.encode([1u8; 32]) })), None, &[("authorization", &ag.auth())]).await;
    assert_eq!(r.json["sesiones"][0], sesion.as_str());
    assert_eq!(r.json["atencion"], true);
    let r =
        pedir(&p.app, "POST", &format!("/api/agente/sesiones/{sesion}/mensajes"), Some(json!({ "cifrado": "AAAA" })), None, &[("authorization", &ag.auth())])
            .await;
    assert_eq!(r.json["n"], 1);
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/sesiones/{sesion}/mensajes?desde=0"), None, Some(&cookie), &[]).await;
    assert_eq!(r.json[0]["cifrado"], "AAAA");
    assert_eq!(r.json[0]["de"], "equipo");
    // La espera larga devuelve en cuanto escribe el otro lado.
    let (app, ck, s2) = (p.app.clone(), cookie.clone(), sesion.clone());
    let lector = tokio::spawn(async move { pedir(&app, "GET", &format!("/api/clientes/{c}/sesiones/{s2}/mensajes?desde=1"), None, Some(&ck), &[]).await });
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    pedir(&p.app, "POST", &format!("/api/agente/sesiones/{sesion}/mensajes"), Some(json!({ "cifrado": "BBBB" })), None, &[("authorization", &ag.auth())]).await;
    let r = tokio::time::timeout(std::time::Duration::from_secs(5), lector).await.expect("la espera larga no despertó").unwrap();
    assert_eq!(r.json[0]["cifrado"], "BBBB");
}

#[tokio::test]
async fn rele_de_descarga() {
    let p = servidor();
    let cookie = propietario(&p).await;
    let (c, ag) = cliente_con_equipo(&p, &cookie).await;
    let ordenes = format!("/api/clientes/{c}/equipos/{}/ordenes", ag.id);
    let (sesion, relevo) = (uuid::Uuid::new_v4().to_string(), uuid::Uuid::new_v4().to_string());
    // Más grande de lo permitido: rechazada.
    let r = pedir(&p.app, "POST", &ordenes, Some(json!({ "tipo": "descargar", "seq": 1, "sellado": sobre(&ag), "caduca": caduca(1), "sesion": sesion, "relevo": { "id": relevo, "max_bytes": 10_000_000_000u64 } })), Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY);
    let r = pedir(&p.app, "POST", &ordenes, Some(json!({ "tipo": "descargar", "seq": 1, "sellado": sobre(&ag), "caduca": caduca(1), "sesion": sesion, "relevo": { "id": relevo, "max_bytes": 1000 } })), Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let subir = |n: u64, datos: &'static [u8]| {
        let (app, auth, relevo) = (p.app.clone(), ag.auth(), relevo.clone());
        async move {
            let req = Request::builder()
                .method("PUT")
                .uri(format!("/api/agente/relevos/{relevo}/trozos/{n}"))
                .header("authorization", auth)
                .body(Body::from(datos))
                .unwrap();
            app.oneshot(req).await.unwrap().status()
        }
    };
    assert_eq!(subir(1, b"fuera de orden").await, StatusCode::CONFLICT);
    assert_eq!(subir(0, b"trozo-0").await, StatusCode::NO_CONTENT);
    assert_eq!(subir(1, b"trozo-1").await, StatusCode::NO_CONTENT);
    let r =
        pedir(&p.app, "POST", &format!("/api/agente/relevos/{relevo}/fin"), Some(json!({ "trozos": 2, "bytes": 14 })), None, &[("authorization", &ag.auth())])
            .await;
    assert_eq!(r.estado, StatusCode::NO_CONTENT, "{}", r.json);
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/relevos/{relevo}"), None, Some(&cookie), &[]).await;
    assert_eq!((r.json["estado"].as_str(), r.json["trozos"].as_u64()), (Some("listo"), Some(2)));
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/relevos/{relevo}/trozos/1"), None, Some(&cookie), &[]).await;
    assert_eq!(r.bytes, b"trozo-1");
    let r = pedir(&p.app, "DELETE", &format!("/api/clientes/{c}/relevos/{relevo}"), None, Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::NO_CONTENT);
    assert!(!p.st.datos.join("relevos").join(&c).join(&relevo).exists());
}

#[tokio::test]
async fn canal_websocket() {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

    let p = servidor();
    let cookie = propietario(&p).await;
    let (c, ag) = cliente_con_equipo(&p, &cookie).await;
    // Servidor real en un puerto libre (sin TLS: solo para la prueba).
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = p.app.clone();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let reto = B64.encode([7u8; 32]);
    let mut req = format!("ws://{addr}/api/agente/canal?reto={}", urlenc(&reto)).into_client_request().unwrap();
    req.headers_mut().insert("authorization", ag.auth().parse().unwrap());
    let (mut ws, _) = tokio_tungstenite::connect_async(req).await.expect("conecta");
    let hola: Value = serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
    assert_eq!(hola["t"], "hola");
    let firma: [u8; 64] = B64.decode(hola["firma"].as_str().unwrap()).unwrap().try_into().unwrap();
    let identidad: [u8; 32] = B64.decode(&p.st.identidad_pub).unwrap().try_into().unwrap();
    assert!(ed25519_dalek::VerifyingKey::from_bytes(&identidad)
        .unwrap()
        .verify(derivaciones::texto_identidad_servidor(&reto, &ag.id).as_bytes(), &ed25519_dalek::Signature::from_bytes(&firma))
        .is_ok());

    // Una orden de la consola le llega al momento.
    let r = pedir(
        &p.app,
        "POST",
        &format!("/api/clientes/{c}/equipos/{}/ordenes", ag.id),
        Some(json!({ "tipo": "copiar_ahora", "seq": 1, "sellado": sobre(&ag), "caduca": caduca(1) })),
        Some(&cookie),
        &[],
    )
    .await;
    let orden = r.json["id"].as_str().unwrap().to_string();
    let msg: Value =
        serde_json::from_str(tokio::time::timeout(std::time::Duration::from_secs(5), ws.next()).await.unwrap().unwrap().unwrap().to_text().unwrap()).unwrap();
    assert_eq!((msg["t"].as_str(), msg["orden"]["id"].as_str()), (Some("orden"), Some(orden.as_str())));

    // Resultado e informe por el mismo canal.
    let firma = ag.firmar_resultado(&orden, 1, "hecha", None);
    ws.send(Message::Text(json!({ "t": "resultado", "orden": orden, "estado": "hecha", "mensaje": null, "detalle": null, "firma": firma }).to_string().into()))
        .await
        .unwrap();
    ws.send(Message::Text(json!({ "t": "informe", "datos": { "version": "0.7.0", "servicio": "en_marcha" } }).to_string().into())).await.unwrap();
    // El progreso en vivo (v1.25), por el mismo canal.
    let tarea = json!({ "tipo": "copia", "repo": "r1", "fase": "escaneando", "porcentaje": 0.1 });
    ws.send(Message::Text(json!({ "t": "progreso", "tareas": [tarea] }).to_string().into())).await.unwrap();
    // Se procesa en segundo plano: se espera un poco.
    let mut hecha = false;
    for _ in 0..50 {
        let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/equipos/{}", ag.id), None, Some(&cookie), &[]).await;
        let o = pedir(&p.app, "GET", &format!("/api/clientes/{c}/equipos/{}/ordenes", ag.id), None, Some(&cookie), &[]).await;
        if o.json[0]["estado"] == "hecha" && r.json["estado_servicio"] == "en_marcha" {
            assert_eq!(r.json["conectado"], true);
            hecha = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(hecha, "el resultado o el informe no llegaron por el WebSocket");
    // El último informe de cada equipo, de una vez.
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/informes"), None, Some(&cookie), &[]).await;
    assert_eq!((r.json[0]["equipo"].as_str(), r.json[0]["datos"]["version"].as_str()), (Some(ag.id.as_str()), Some("0.7.0")));
    let mut visto = false;
    for _ in 0..50 {
        let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/progreso"), None, Some(&cookie), &[]).await;
        if r.json[0]["tareas"][0]["fase"] == "escaneando" {
            visto = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(visto, "el progreso no llegó por el WebSocket");
    ws.close(None).await.unwrap();
}

#[tokio::test]
async fn cuenta_nombre_autenticador_y_recuperacion() {
    // La prueba usa «el código de este paso de 30 s» varias veces: si cruzara al paso
    // siguiente a mitad, el código «ya usado» sería otro y fallaría sin motivo.
    let s = chrono::Utc::now().timestamp() % 30;
    if s >= 15 {
        tokio::time::sleep(std::time::Duration::from_secs((31 - s) as u64)).await;
    }
    let p = servidor();
    let (cookie, secreto) = propietario_y_totp(&p).await;
    // Nombre.
    let r = pedir(&p.app, "PATCH", "/api/cuenta", Some(json!({ "nombre": "Ana María" })), Some(&cookie), &[]).await;
    assert_eq!((r.estado, r.json["nombre"].as_str()), (StatusCode::OK, Some("Ana María")), "{}", r.json);
    assert_eq!(pedir(&p.app, "PATCH", "/api/cuenta", Some(json!({ "nombre": "  " })), Some(&cookie), &[]).await.estado, StatusCode::UNPROCESSABLE_ENTITY);
    // Autenticador nuevo: hace falta la contraseña y un código del actual.
    let mal = pedir(&p.app, "POST", "/api/cuenta/totp", Some(json!({ "contrasena": "no es", "codigo": codigo_totp(&secreto, 1) })), Some(&cookie), &[]).await;
    assert_eq!(mal.estado, StatusCode::UNAUTHORIZED);
    // El código ya usado al entrar no vale otra vez.
    let r = pedir(
        &p.app,
        "POST",
        "/api/cuenta/totp",
        Some(json!({ "contrasena": "una contraseña bien larga", "codigo": codigo_totp(&secreto, 0) })),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(r.estado, StatusCode::UNAUTHORIZED);
    let r = pedir(
        &p.app,
        "POST",
        "/api/cuenta/totp",
        Some(json!({ "contrasena": "una contraseña bien larga", "codigo": codigo_totp(&secreto, 1) })),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let nuevo = r.json["totp"]["secreto"].as_str().unwrap().to_string();
    assert_ne!(nuevo, secreto);
    let r = pedir(&p.app, "POST", "/api/cuenta/totp/confirmar", Some(json!({ "codigo": "000000" })), Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::UNAUTHORIZED);
    let r = pedir(&p.app, "POST", "/api/cuenta/totp/confirmar", Some(json!({ "codigo": codigo_totp(&nuevo, 0) })), Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    // Autenticador nuevo, sesión nueva: la de antes ya no vale.
    let antes = cookie;
    let cookie = r.cookie.unwrap();
    assert_eq!(pedir(&p.app, "GET", "/api/cuenta", None, Some(&antes), &[]).await.estado, StatusCode::UNAUTHORIZED);
    // Ya no hay nada pendiente.
    let r = pedir(&p.app, "POST", "/api/cuenta/totp/confirmar", Some(json!({ "codigo": codigo_totp(&nuevo, 1) })), Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY);
    // Códigos de recuperación nuevos, con el autenticador nuevo (el viejo ya no vale).
    let r = pedir(
        &p.app,
        "POST",
        "/api/cuenta/recuperacion",
        Some(json!({ "contrasena": "una contraseña bien larga", "codigo": codigo_totp(&secreto, 1) })),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(r.estado, StatusCode::UNAUTHORIZED);
    let r = pedir(
        &p.app,
        "POST",
        "/api/cuenta/recuperacion",
        Some(json!({ "contrasena": "una contraseña bien larga", "codigo": codigo_totp(&nuevo, 1) })),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["codigos_recuperacion"].as_array().unwrap().len(), 10);
}

#[tokio::test]
async fn listados_auditoria_y_espera_confirmada() {
    let p = servidor();
    let cookie = propietario(&p).await;
    let (c, ag) = cliente_con_equipo(&p, &cookie).await;
    let ordenes = format!("/api/clientes/{c}/equipos/{}/ordenes", ag.id);
    let mut ids = Vec::new();
    for seq in 1..=3 {
        let r = pedir(
            &p.app,
            "POST",
            &ordenes,
            Some(json!({ "tipo": "copiar_ahora", "seq": seq, "sellado": sobre(&ag), "caduca": caduca(1) })),
            Some(&cookie),
            &[],
        )
        .await;
        assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
        ids.push(r.json["id"].as_str().unwrap().to_string());
    }
    // Órdenes de todo el cliente, por páginas (de la más reciente a la más antigua).
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/ordenes?limite=2"), None, Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let pagina1: Vec<String> = r.json["ordenes"].as_array().unwrap().iter().map(|o| o["id"].as_str().unwrap().to_string()).collect();
    assert_eq!(pagina1.len(), 2);
    assert_eq!(r.json["ordenes"][0]["equipo"], ag.id.as_str());
    let cursor = r.json["siguiente"].as_str().unwrap().to_string();
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/ordenes?limite=2&antes={cursor}"), None, Some(&cookie), &[]).await;
    let pagina2: Vec<String> = r.json["ordenes"].as_array().unwrap().iter().map(|o| o["id"].as_str().unwrap().to_string()).collect();
    assert_eq!(pagina2.len(), 1);
    assert!(r.json["siguiente"].is_null());
    let mut todas = [pagina1, pagina2].concat();
    todas.sort();
    let mut esperadas = ids.clone();
    esperadas.sort();
    assert_eq!(todas, esperadas, "cada orden una vez");
    // Filtros.
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/ordenes?estado=hecha"), None, Some(&cookie), &[]).await;
    assert!(r.json["ordenes"].as_array().unwrap().is_empty());
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/ordenes?equipo={}", ag.id), None, Some(&cookie), &[]).await;
    assert_eq!(r.json["ordenes"].as_array().unwrap().len(), 3);
    assert_eq!(
        pedir(&p.app, "GET", &format!("/api/clientes/{c}/ordenes?estado=rara"), None, Some(&cookie), &[]).await.estado,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // Auditoría de la más reciente hacia atrás, con cursor.
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/auditoria?orden=desc&limite=2"), None, Some(&cookie), &[]).await;
    let ns: Vec<i64> = r.json.as_array().unwrap().iter().map(|e| e["n"].as_i64().unwrap()).collect();
    assert_eq!(ns.len(), 2);
    assert!(ns[0] > ns[1]);
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/auditoria?orden=desc&antes={}&limite=100", ns[1]), None, Some(&cookie), &[]).await;
    assert!(r.json.as_array().unwrap().iter().all(|e| e["n"].as_i64().unwrap() < ns[1]));
    // Los técnicos leen la actividad; los de solo lectura, no.
    let tecnico = invitado(&p, &cookie, &c, "tecnico").await;
    assert_eq!(pedir(&p.app, "GET", &format!("/api/clientes/{c}/auditoria?orden=desc"), None, Some(&tecnico), &[]).await.estado, StatusCode::OK);
    assert_eq!(pedir(&p.app, "GET", &format!("/api/clientes/{c}/auditoria/verificar"), None, Some(&tecnico), &[]).await.estado, StatusCode::OK);
    let lectura = invitado(&p, &cookie, &c, "lectura").await;
    assert_eq!(pedir(&p.app, "GET", &format!("/api/clientes/{c}/auditoria"), None, Some(&lectura), &[]).await.estado, StatusCode::FORBIDDEN);

    // cambiar_espera: el servidor solo cambia la espera con el resultado firmado del equipo.
    let r =
        pedir(&p.app, "POST", &ordenes, Some(json!({ "tipo": "cambiar_espera", "seq": 4, "sellado": sobre(&ag), "caduca": caduca(1) })), Some(&cookie), &[])
            .await;
    let orden = r.json["id"].as_str().unwrap().to_string();
    pedir(&p.app, "POST", "/api/agente/tomar", Some(json!({ "reto": B64.encode([1u8; 32]) })), None, &[("authorization", &ag.auth())]).await;
    let detalle = r#"{"espera_min_horas":2}"#;
    let firma = B64.encode(ag.firma.sign(derivaciones::texto_resultado(&orden, 4, "hecha", Some("Espera: 2 h."), Some(detalle)).as_bytes()).to_bytes());
    let r = pedir(
        &p.app,
        "POST",
        "/api/agente/resultado",
        Some(json!({ "orden": orden, "estado": "hecha", "mensaje": "Espera: 2 h.", "detalle": detalle, "firma": firma })),
        None,
        &[("authorization", &ag.auth())],
    )
    .await;
    assert_eq!(r.estado, StatusCode::NO_CONTENT, "{}", r.json);
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/equipos/{}", ag.id), None, Some(&cookie), &[]).await;
    assert_eq!(r.json["espera_min_horas"], 2);
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}"), None, Some(&cookie), &[]).await;
    assert_eq!(r.json["espera_min_horas"], 2, "todos los equipos coinciden: también el cliente");
    // Ahora una destructiva con 3 h de espera vale (antes hacían falta 24).
    let r = pedir(
        &p.app,
        "POST",
        &ordenes,
        Some(json!({ "tipo": "pausar", "seq": 5, "sellado": sobre(&ag), "caduca": caduca(48), "not_before": caduca(3) })),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let r = pedir(
        &p.app,
        "POST",
        &ordenes,
        Some(json!({ "tipo": "pausar", "seq": 6, "sellado": sobre(&ag), "caduca": caduca(48), "not_before": caduca(1) })),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!((r.estado, r.json["espera_min_horas"].as_i64()), (StatusCode::UNPROCESSABLE_ENTITY, Some(2)));
}

fn urlenc(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

#[tokio::test]
async fn recibir_cliente_paquete_e_importacion() {
    // Servidor anterior (A) con un cliente, un equipo y su auditoría.
    let a = servidor();
    let ck_a = propietario(&a).await;
    let (c_a, ag) = cliente_con_equipo(&a, &ck_a).await;
    let r = pedir(&a.app, "GET", &format!("/api/clientes/{c_a}"), None, Some(&ck_a), &[]).await;
    let sal = r.json["sal_cliente"].as_str().unwrap().to_string();
    let auditoria_a = pedir(&a.app, "GET", &format!("/api/clientes/{c_a}/auditoria?limite=1000"), None, Some(&ck_a), &[]).await.json;
    assert!(auditoria_a.as_array().unwrap().len() >= 3);

    // Servidor nuevo (B): «Recibir un cliente» con la misma sal y una ficha de un uso.
    let b = servidor();
    let ck_b = propietario(&b).await;
    let r = pedir(&b.app, "POST", "/api/clientes/recibir", Some(json!({ "nombre": "Café del Sur", "sal_cliente": sal, "usos": 1 })), Some(&ck_b), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let c_b = r.json["cliente"]["id"].as_str().unwrap().to_string();
    assert_eq!(r.json["cliente"]["sal_cliente"], sal.as_str());
    let ficha = r.json["ficha"].as_str().unwrap().to_string();

    // El equipo entra con la ficha, ya confirmado y con su mismo id.
    let cuerpo = json!({ "ficha": ficha, "equipo_id": ag.id, "nombre": "RECEPCION", "so": "windows", "version": "0.7.0",
        "box_pub": ag.box_pub, "sign_pub": ag.sign_pub(), "sal_equipo": B64.encode([3u8; 16]), "etiqueta": B64.encode([7u8; 32]), "espera_min_horas": 12 });
    let r = pedir(&b.app, "POST", "/api/agente/recibir", Some(cuerpo.clone()), None, &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["equipo_id"], ag.id.as_str());
    assert_eq!(r.json["cliente_id"], c_b.as_str());
    let r = pedir(&b.app, "GET", &format!("/api/clientes/{c_b}/equipos/{}", ag.id), None, Some(&ck_b), &[]).await;
    assert_eq!((r.json["confirmado"].as_bool(), r.json["espera_min_horas"].as_i64()), (Some(true), Some(12)));
    // La ficha era de un uso.
    assert_eq!(pedir(&b.app, "POST", "/api/agente/recibir", Some(cuerpo), None, &[]).await.estado, StatusCode::NOT_FOUND);

    // Paquete: solo el de este cliente (por su sal), y se sirve tal cual.
    let k = [9u8; 32];
    let otro = resguardo_protocolo::paquete::cifrar(&k, "b3RyYSBzYWwgZGUgb3RybyBjbGllbnRl", b"{}");
    let poner = |cuerpo: Vec<u8>| {
        let (app, ck, c) = (b.app.clone(), ck_b.clone(), c_b.clone());
        async move {
            let req = Request::builder()
                .method("PUT")
                .uri(format!("/api/clientes/{c}/paquete"))
                .header("host", "localhost")
                .header("x-resguardo", "1")
                .header("cookie", ck)
                .body(Body::from(cuerpo))
                .unwrap();
            app.oneshot(req).await.unwrap().status()
        }
    };
    assert_eq!(poner(otro).await, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(poner(b"no es un paquete".to_vec()).await, StatusCode::UNPROCESSABLE_ENTITY);
    let bueno = resguardo_protocolo::paquete::cifrar(&k, &sal, br#"{"v":1}"#);
    assert_eq!(poner(bueno.clone()).await, StatusCode::NO_CONTENT);
    let r = pedir(&b.app, "GET", &format!("/api/clientes/{c_b}/paquete"), None, Some(&ck_b), &[]).await;
    assert_eq!(r.bytes, bueno);

    // Importar la auditoría de A: con la cadena rota, no; entera, sí (una vez).
    let mut rota = auditoria_a.clone();
    rota[1]["datos"] = json!("{\"cambiado\":true}");
    let r = pedir(&b.app, "POST", &format!("/api/clientes/{c_b}/importar"), Some(json!({ "origen": "A", "auditoria": rota })), Some(&ck_b), &[]).await;
    assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.json);
    let cuerpo = json!({ "origen": "A", "auditoria": auditoria_a, "informes": [{ "equipo": ag.id, "recibido": 1_700_000_000, "datos": { "version": "0.6" } }],
        "avisos": [{ "equipo": ag.id, "tipo": "copia_fallida", "mensaje": "Antigua", "creado": "2026-01-01T10:00:00+00:00" }] });
    let r = pedir(&b.app, "POST", &format!("/api/clientes/{c_b}/importar"), Some(cuerpo.clone()), Some(&ck_b), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(pedir(&b.app, "POST", &format!("/api/clientes/{c_b}/importar"), Some(cuerpo), Some(&ck_b), &[]).await.estado, StatusCode::CONFLICT);
    let r = pedir(&b.app, "GET", &format!("/api/clientes/{c_b}/auditoria/importada"), None, Some(&ck_b), &[]).await;
    assert_eq!(r.json.as_array().unwrap().len(), auditoria_a.as_array().unwrap().len());
    let r = pedir(&b.app, "GET", &format!("/api/clientes/{c_b}/auditoria/verificar"), None, Some(&ck_b), &[]).await;
    assert_eq!(r.json["ok"], true);

    // En A, el resultado firmado de cambiar_servidor deja el equipo como «trasladado».
    let r = pedir(
        &a.app,
        "POST",
        &format!("/api/clientes/{c_a}/equipos/{}/ordenes", ag.id),
        Some(json!({ "tipo": "cambiar_servidor", "seq": 1, "sellado": sobre(&ag), "caduca": caduca(48) })),
        Some(&ck_a),
        &[],
    )
    .await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let orden = r.json["id"].as_str().unwrap().to_string();
    pedir(&a.app, "POST", "/api/agente/tomar", Some(json!({ "reto": B64.encode([1u8; 32]) })), None, &[("authorization", &ag.auth())]).await;
    let firma = ag.firmar_resultado(&orden, 1, "hecha", Some("En el servidor nuevo."));
    let r = pedir(
        &a.app,
        "POST",
        "/api/agente/resultado",
        Some(json!({ "orden": orden, "estado": "hecha", "mensaje": "En el servidor nuevo.", "firma": firma })),
        None,
        &[("authorization", &ag.auth())],
    )
    .await;
    assert_eq!(r.estado, StatusCode::NO_CONTENT, "{}", r.json);
    let r = pedir(&a.app, "GET", &format!("/api/clientes/{c_a}/equipos/{}", ag.id), None, Some(&ck_a), &[]).await;
    assert_eq!(r.json["modo"], "trasladado");
}

/// La consola dentro del binario (`--features consola-integrada`, con consola/build compilada).
#[cfg(feature = "consola-integrada")]
#[tokio::test]
async fn consola_integrada() {
    let p = servidor();
    let r = pedir(&p.app, "GET", "/", None, None, &[]).await;
    assert_eq!(r.estado, StatusCode::OK);
    let html = String::from_utf8_lossy(&r.bytes).to_lowercase();
    assert!(html.contains("<!doctype html") && html.contains("_app/arranque-"), "no es la consola");
    // Las rutas de la SPA dan index.html; lo que falta en _app/, 404; la API sigue siendo la API.
    assert_eq!(pedir(&p.app, "GET", "/c/abc/equipos", None, None, &[]).await.bytes, r.bytes);
    assert_eq!(pedir(&p.app, "GET", "/_app/no-existe.js", None, None, &[]).await.estado, StatusCode::NOT_FOUND);
    assert_eq!(pedir(&p.app, "GET", "/api/no-existe", None, None, &[]).await.estado, StatusCode::NOT_FOUND);
    let res = p.app.clone().oneshot(Request::builder().uri("/").body(Body::empty()).unwrap()).await.unwrap();
    assert!(res.headers().get("content-security-policy").unwrap().to_str().unwrap().contains("script-src 'self'"));
}

/// Instalador listo (v1.17): el servidor añade la cola con el código; el equipo entra con
/// el nombre de la consola, el código sirve una vez y deja de verse al confirmar o anular.
#[tokio::test]
async fn equipos_preparados_instalador_y_linux() {
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join("Resguardo-Agente-setup.exe");
    std::fs::write(&exe, b"MZ instalador de prueba").unwrap();
    // La huella de la autoridad TLS (la escribe el arranque de verdad; aquí no hay TLS).
    std::fs::create_dir_all(dir.path().join("tls")).unwrap();
    std::fs::write(dir.path().join("tls").join("ca.huella"), vec!["5A"; 32].join(":")).unwrap();
    let st = preparar(dir.path(), Opciones { https: false, instalador_agente: Some(exe), ..Default::default() }).unwrap();
    let p = Prueba { _dir: dir, st: st.clone(), app: api::router(st) };
    let cookie = propietario(&p).await;
    assert_eq!(pedir(&p.app, "GET", "/api/servidor", None, None, &[]).await.json["instalador_agente"], true);
    let r = pedir(&p.app, "POST", "/api/clientes", Some(json!({ "nombre": "Ferretería Altamar", "espera_min_horas": 24 })), Some(&cookie), &[]).await;
    let c = r.json["id"].as_str().unwrap().to_string();
    let ruta = format!("/api/clientes/{c}/instaladores");

    // Windows: el instalador genérico con la cola al final.
    let r =
        pedir(&p.app, "POST", &ruta, Some(json!({ "nombre": "PC-01", "so": "windows", "servidor": "https://192.168.1.20:8443/" })), Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", String::from_utf8_lossy(&r.bytes));
    assert!(r.bytes.starts_with(b"MZ instalador de prueba"));
    let d = resguardo_protocolo::instalador::leer_cola(&r.bytes).unwrap().unwrap();
    assert_eq!(
        (d.servidor.as_str(), d.cliente.as_str(), d.nombre.as_str(), d.huella_ca.as_str()),
        ("https://192.168.1.20:8443", c.as_str(), "PC-01", p.st.huella_ca.as_str())
    );
    // Datos que no valen: nada.
    for malo in [
        json!({ "nombre": "X", "so": "windows", "servidor": "http://srv:8443" }),
        json!({ "nombre": "", "so": "linux", "servidor": "https://srv:8443" }),
        json!({ "nombre": "X", "so": "mac", "servidor": "https://srv:8443" }),
    ] {
        assert_eq!(pedir(&p.app, "POST", &ruta, Some(malo), Some(&cookie), &[]).await.estado, StatusCode::UNPROCESSABLE_ENTITY);
    }
    // Solo administradores.
    let tecnico = invitado(&p, &cookie, &c, "tecnico").await;
    let linux = json!({ "nombre": "X", "so": "linux", "servidor": "https://srv:8443" });
    assert_eq!(pedir(&p.app, "POST", &ruta, Some(linux), Some(&tecnico), &[]).await.estado, StatusCode::FORBIDDEN);
    assert_eq!(pedir(&p.app, "GET", &format!("/api/clientes/{c}/emparejamientos"), None, Some(&tecnico), &[]).await.estado, StatusCode::FORBIDDEN);

    // Sale en la lista de preparados (sin el código).
    let l = pedir(&p.app, "GET", &format!("/api/clientes/{c}/emparejamientos"), None, Some(&cookie), &[]).await.json;
    let emp = l[0]["id"].as_str().unwrap().to_string();
    assert_eq!((l[0]["nombre"].as_str(), l[0]["estado"].as_str(), l[0].get("codigo")), (Some("PC-01"), Some("abierto"), None));

    // El equipo se une con el código (y el nombre de la consola, no el suyo); una sola vez.
    let ag = Agente::nuevo();
    let unirse = |codigo: &str| json!({ "codigo_hash": mensajes::code_hash(codigo), "nombre": "DESKTOP-4F2K", "so": "windows", "version": "0.7.7", "box_pub": ag.box_pub, "sign_pub": ag.sign_pub(), "sal_equipo": B64.encode([3u8; 16]) });
    let r = pedir(&p.app, "POST", "/api/agente/unirse", Some(unirse(&d.codigo)), None, &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let eq = r.json["equipo_id"].as_str().unwrap().to_string();
    assert_eq!(pedir(&p.app, "POST", "/api/agente/unirse", Some(unirse(&d.codigo)), None, &[]).await.estado, StatusCode::NOT_FOUND);
    let e = pedir(&p.app, "GET", &format!("/api/clientes/{c}/equipos/{eq}"), None, Some(&cookie), &[]).await.json;
    assert_eq!(e["nombre"], "PC-01");
    // La consola necesita el código para el alta mientras está «unido»; al confirmar, ya no.
    let v = pedir(&p.app, "GET", &format!("/api/clientes/{c}/emparejamientos/{emp}"), None, Some(&cookie), &[]).await.json;
    assert_eq!((v["estado"].as_str(), v["codigo"].as_str()), (Some("unido"), Some(d.codigo.as_str())));
    let r =
        pedir(&p.app, "POST", &format!("/api/clientes/{c}/emparejamientos/{emp}/confirmar"), Some(json!({ "etiqueta": "ETIQUETA" })), Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let v = pedir(&p.app, "GET", &format!("/api/clientes/{c}/emparejamientos/{emp}"), None, Some(&cookie), &[]).await.json;
    assert_eq!((v["estado"].as_str(), v.get("codigo")), (Some("confirmado"), None));

    // Linux: la línea con el código; anulado, ya no sirve.
    let r =
        pedir(&p.app, "POST", &ruta, Some(json!({ "nombre": "srv-datos", "so": "linux", "servidor": "https://192.168.1.20:8443" })), Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let (emp2, codigo2) = (r.json["id"].as_str().unwrap().to_string(), r.json["codigo"].as_str().unwrap().to_string());
    assert_eq!(r.json["huella_ca"].as_str(), Some(p.st.huella_ca.as_str()));
    assert_eq!(pedir(&p.app, "DELETE", &format!("/api/clientes/{c}/emparejamientos/{emp2}"), None, Some(&cookie), &[]).await.estado, StatusCode::NO_CONTENT);
    assert_eq!(pedir(&p.app, "POST", "/api/agente/unirse", Some(unirse(&codigo2)), None, &[]).await.estado, StatusCode::NOT_FOUND);
    assert_eq!(pedir(&p.app, "GET", &format!("/api/clientes/{c}/emparejamientos"), None, Some(&cookie), &[]).await.json, json!([]));
}

/// Sin instalador en el servidor (Linux, desarrollo): se dice, y no se gasta ningún código.
#[tokio::test]
async fn sin_instalador_del_agente() {
    let p = servidor();
    let cookie = propietario(&p).await;
    let r = pedir(&p.app, "POST", "/api/clientes", Some(json!({ "nombre": "Ferretería Altamar", "espera_min_horas": 24 })), Some(&cookie), &[]).await;
    let c = r.json["id"].as_str().unwrap().to_string();
    let cuerpo = json!({ "nombre": "PC", "so": "windows", "servidor": "https://srv:8443" });
    let r = pedir(&p.app, "POST", &format!("/api/clientes/{c}/instaladores"), Some(cuerpo), Some(&cookie), &[]).await;
    assert_eq!((r.estado, r.json["error"].as_str()), (StatusCode::NOT_FOUND, Some("sin_instalador")));
    // «Vincular este servidor» (v1.19): sin puerto conocido ni agente en la máquina, tampoco.
    let r = pedir(&p.app, "POST", &format!("/api/clientes/{c}/equipo-local"), None, Some(&cookie), &[]).await;
    assert_eq!((r.estado, r.json["error"].as_str()), (StatusCode::NOT_FOUND, Some("sin_agente_local")));
    assert_eq!(pedir(&p.app, "GET", &format!("/api/clientes/{c}/emparejamientos"), None, Some(&cookie), &[]).await.json, json!([]));
}

/// Etiquetas de los equipos (v1.18): técnicos o más, limpias y auditadas; las ven todos.
#[tokio::test]
async fn etiquetas_de_equipos() {
    let p = servidor();
    let cookie = propietario(&p).await;
    let (c, ag) = cliente_con_equipo(&p, &cookie).await;
    let ruta = format!("/api/clientes/{c}/equipos/{}/etiquetas", ag.id);
    let tecnico = invitado(&p, &cookie, &c, "tecnico").await;
    let lectura = invitado(&p, &cookie, &c, "lectura").await;
    let r = pedir(&p.app, "PUT", &ruta, Some(json!({ "etiquetas": [" Contabilidad ", "contabilidad", "Sede norte"] })), Some(&tecnico), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["etiquetas"], json!(["Contabilidad", "Sede norte"]));
    assert_eq!(pedir(&p.app, "PUT", &ruta, Some(json!({ "etiquetas": ["X"] })), Some(&lectura), &[]).await.estado, StatusCode::FORBIDDEN);
    assert_eq!(pedir(&p.app, "PUT", &ruta, Some(json!({ "etiquetas": ["a,b"] })), Some(&tecnico), &[]).await.estado, StatusCode::UNPROCESSABLE_ENTITY);
    let l = pedir(&p.app, "GET", &format!("/api/clientes/{c}/equipos"), None, Some(&lectura), &[]).await.json;
    assert_eq!(l[0]["etiquetas"], json!(["Contabilidad", "Sede norte"]));
    let a = pedir(&p.app, "GET", &format!("/api/clientes/{c}/auditoria?orden=desc&limite=5"), None, Some(&cookie), &[]).await.json;
    assert!(a.as_array().unwrap().iter().any(|x| x["accion"] == "etiquetas_equipo"));
    let otro = format!("/api/clientes/{c}/equipos/no-existe/etiquetas");
    assert_eq!(pedir(&p.app, "PUT", &otro, Some(json!({ "etiquetas": [] })), Some(&cookie), &[]).await.estado, StatusCode::NOT_FOUND);
}

/// Progreso en vivo (v1.25): del agente a la consola, solo en memoria y limpio.
#[tokio::test]
async fn progreso_en_vivo() {
    let p = servidor();
    let cookie = propietario(&p).await;
    let (c, ag) = cliente_con_equipo(&p, &cookie).await;
    let lectura = invitado(&p, &cookie, &c, "lectura").await;
    let ruta = format!("/api/clientes/{c}/progreso");
    let auth = ag.auth();
    let agente = [("authorization", auth.as_str())];
    assert_eq!(pedir(&p.app, "GET", &ruta, None, Some(&lectura), &[]).await.json, json!([]));
    let tarea = json!({ "tipo": "copia", "repo": "r1", "copia": "k1", "nombre": "Documentos", "fase": "subiendo", "porcentaje": 0.42,
                        "archivos": 120, "archivos_total": 300, "bytes": 4000, "bytes_total": 10000, "velocidad": 2000, "quedan_s": 90,
                        "empezo": "2026-10-03T10:00:00-05:00", "ruta": r"C:\Users\Ana" });
    let r = pedir(&p.app, "POST", "/api/agente/progreso", Some(json!({ "tareas": [tarea] })), None, &agente).await;
    assert_eq!(r.estado, StatusCode::NO_CONTENT, "{}", r.json);
    // Cualquier papel lo lee (como los informes); sin campos desconocidos.
    let r = pedir(&p.app, "GET", &ruta, None, Some(&lectura), &[]).await;
    assert_eq!(r.estado, StatusCode::OK);
    assert_eq!(r.json[0]["equipo"], ag.id.as_str());
    assert_eq!(r.json[0]["tareas"][0]["porcentaje"], 0.42);
    assert_eq!(r.json[0]["tareas"][0]["nombre"], "Documentos");
    assert!(r.json[0]["tareas"][0].get("ruta").is_none());
    assert!(r.json[0]["recibido"].is_string());
    // Sin sesión, nada; un mensaje que no es una lista, rechazado.
    assert_eq!(pedir(&p.app, "GET", &ruta, None, None, &[]).await.estado, StatusCode::UNAUTHORIZED);
    let r = pedir(&p.app, "POST", "/api/agente/progreso", Some(json!({ "tareas": { "x": 1 } })), None, &agente).await;
    assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY);
    // Al terminar, el informe sin `progreso` lo quita (y uno con `progreso` lo pone: el sondeo).
    let r = pedir(&p.app, "POST", "/api/agente/informe", Some(json!({ "datos": { "version": "0.7.9", "servicio": "en_marcha" } })), None, &agente).await;
    assert_eq!(r.estado, StatusCode::NO_CONTENT);
    assert_eq!(pedir(&p.app, "GET", &ruta, None, Some(&cookie), &[]).await.json, json!([]));
    let datos = json!({ "version": "0.7.9", "servicio": "en_marcha", "progreso": [{ "tipo": "verificar", "repo": "r1", "fase": "en_marcha", "etapa": "Leyendo el 5 % de los datos…" }] });
    pedir(&p.app, "POST", "/api/agente/informe", Some(json!({ "datos": datos })), None, &agente).await;
    let r = pedir(&p.app, "GET", &ruta, None, Some(&cookie), &[]).await;
    assert_eq!(r.json[0]["tareas"][0]["tipo"], "verificar");
    let r = pedir(&p.app, "POST", "/api/agente/progreso", Some(json!({ "tareas": [] })), None, &agente).await;
    assert_eq!(r.estado, StatusCode::NO_CONTENT);
    assert_eq!(pedir(&p.app, "GET", &ruta, None, Some(&cookie), &[]).await.json, json!([]));
}

/// Tras restaurar una copia de la consola, el servidor recuerda un número de orden
/// anterior al último que aceptó el equipo: con su informe (`ultimo_seq`) se pone al día.
#[tokio::test]
async fn el_informe_adelanta_el_numero_de_orden() {
    let p = servidor();
    let cookie = propietario(&p).await;
    let (c, ag) = cliente_con_equipo(&p, &cookie).await;
    let auth = ag.auth();
    let agente = [("authorization", auth.as_str())];
    let seq = |p: &Prueba| {
        let (app, cookie, ruta) = (p.app.clone(), cookie.clone(), format!("/api/clientes/{c}/equipos/{}", ag.id));
        async move { pedir(&app, "GET", &ruta, None, Some(&cookie), &[]).await.json["siguiente_seq"].as_u64() }
    };
    assert_eq!(seq(&p).await, Some(1));
    let r = pedir(&p.app, "POST", "/api/agente/informe", Some(json!({ "datos": { "version": "0.7.13", "ultimo_seq": 12 } })), None, &agente).await;
    assert_eq!(r.estado, StatusCode::NO_CONTENT);
    assert_eq!(seq(&p).await, Some(13), "el siguiente, después del último que aceptó el equipo");
    // Nunca lo baja (un informe viejo o de un agente anterior, sin el campo).
    pedir(&p.app, "POST", "/api/agente/informe", Some(json!({ "datos": { "version": "0.7.13", "ultimo_seq": 3 } })), None, &agente).await;
    pedir(&p.app, "POST", "/api/agente/informe", Some(json!({ "datos": { "version": "0.7.12" } })), None, &agente).await;
    assert_eq!(seq(&p).await, Some(13));
}

/// v1.35: el equipo dice su último número de orden ya al abrir el canal y en cada
/// sondeo, y el servidor se pone al día **antes** de contarlo como conectado: la
/// primera orden que firma la consola al verlo conectado (tras restaurar su copia)
/// ya no sale «repetida» (lo encontró la prueba de extremo a extremo).
#[tokio::test]
async fn el_canal_y_el_sondeo_adelantan_el_numero_de_orden() {
    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    let p = servidor();
    let cookie = propietario(&p).await;
    let (c, ag) = cliente_con_equipo(&p, &cookie).await;
    let auth = ag.auth();
    let ruta = format!("/api/clientes/{c}/equipos/{}", ag.id);
    let (pr, ck, rt) = (&p, &cookie, &ruta);
    let seq = move || async move { pedir(&pr.app, "GET", rt, None, Some(ck), &[]).await.json["siguiente_seq"].as_u64() };
    let tomar = |n: Value| {
        let (app, auth) = (p.app.clone(), auth.clone());
        async move {
            let r =
                pedir(&app, "POST", "/api/agente/tomar", Some(json!({ "reto": B64.encode([3u8; 32]), "ultimo_seq": n })), None, &[("authorization", &auth)])
                    .await;
            assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
        }
    };
    // Sondeo: sube, nunca baja; sin el campo (un agente anterior), nada.
    tomar(json!(5)).await;
    assert_eq!(seq().await, Some(6));
    tomar(json!(2)).await;
    tomar(Value::Null).await;
    assert_eq!(seq().await, Some(6));

    // Canal: en cuanto el equipo cuenta como conectado, el número ya está al día (sin informe).
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = p.app.clone();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut req = format!("ws://{addr}/api/agente/canal?reto={}&ultimo_seq=12", urlenc(&B64.encode([7u8; 32]))).into_client_request().unwrap();
    req.headers_mut().insert("authorization", auth.parse().unwrap());
    let (mut ws, _) = tokio_tungstenite::connect_async(req).await.expect("conecta");
    assert_eq!(seq().await, Some(13), "al día antes de abrir el canal");
    let hola: Value = serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
    assert_eq!(hola["t"], "hola");
    let e = pedir(&p.app, "GET", &ruta, None, Some(&cookie), &[]).await.json;
    assert_eq!((e["conectado"].as_bool(), e["siguiente_seq"].as_u64()), (Some(true), Some(13)));
    drop(ws);
    // Uno anterior (sin `ultimo_seq` en el canal) sigue entrando.
    let mut req = format!("ws://{addr}/api/agente/canal?reto={}", urlenc(&B64.encode([8u8; 32]))).into_client_request().unwrap();
    req.headers_mut().insert("authorization", auth.parse().unwrap());
    let (_ws, _) = tokio_tungstenite::connect_async(req).await.expect("conecta sin ultimo_seq");
    assert_eq!(seq().await, Some(13));
}

/// Plantillas de copia (v1.20): el servidor guarda bytes opacos de la consola; solo administradores.
#[tokio::test]
async fn plantillas_cifradas() {
    let p = servidor();
    let cookie = propietario(&p).await;
    let (c, _ag) = cliente_con_equipo(&p, &cookie).await;
    let tecnico = invitado(&p, &cookie, &c, "tecnico").await;
    let cifrado = B64.encode([7u8; 120]);
    let ruta = format!("/api/clientes/{c}/plantillas/pla-1");
    assert_eq!(pedir(&p.app, "PUT", &ruta, Some(json!({ "cifrado": cifrado })), Some(&cookie), &[]).await.estado, StatusCode::NO_CONTENT);
    // Sustituir la misma: sigue habiendo una.
    assert_eq!(pedir(&p.app, "PUT", &ruta, Some(json!({ "cifrado": cifrado })), Some(&cookie), &[]).await.estado, StatusCode::NO_CONTENT);
    let l = pedir(&p.app, "GET", &format!("/api/clientes/{c}/plantillas"), None, Some(&cookie), &[]).await.json;
    assert_eq!((l.as_array().unwrap().len(), l[0]["id"].as_str(), l[0]["cifrado"].as_str()), (1, Some("pla-1"), Some(cifrado.as_str())));
    // Técnicos no; ids raros, base64 malo o tamaños fuera de rango tampoco.
    assert_eq!(pedir(&p.app, "GET", &format!("/api/clientes/{c}/plantillas"), None, Some(&tecnico), &[]).await.estado, StatusCode::FORBIDDEN);
    assert_eq!(pedir(&p.app, "PUT", &ruta, Some(json!({ "cifrado": cifrado })), Some(&tecnico), &[]).await.estado, StatusCode::FORBIDDEN);
    for (id, cuerpo) in [("a.b", cifrado.clone()), ("pla-2", "no es base64".into()), ("pla-2", B64.encode([1u8; 10])), ("pla-2", B64.encode(vec![1u8; 70_000]))]
    {
        let r = pedir(&p.app, "PUT", &format!("/api/clientes/{c}/plantillas/{id}"), Some(json!({ "cifrado": cuerpo })), Some(&cookie), &[]).await;
        assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY, "{id}");
    }
    assert_eq!(pedir(&p.app, "DELETE", &ruta, None, Some(&cookie), &[]).await.estado, StatusCode::NO_CONTENT);
    assert_eq!(pedir(&p.app, "DELETE", &ruta, None, Some(&cookie), &[]).await.estado, StatusCode::NOT_FOUND);
    let a = pedir(&p.app, "GET", &format!("/api/clientes/{c}/auditoria?orden=desc&limite=5"), None, Some(&cookie), &[]).await.json;
    assert!(a.as_array().unwrap().iter().any(|x| x["accion"] == "borrar_plantilla"));
}

/// v1.23: «Copia de la consola». El propietario del servidor pone la clave pública
/// (la clave de respaldo no llega nunca al servidor), la hace al momento y ve cómo
/// fue; nadie más puede, y no hay forma de descargarla por la API.
#[tokio::test]
async fn copia_de_la_consola() {
    let p = servidor();
    let cookie = propietario(&p).await;
    let (c, _) = cliente_con_equipo(&p, &cookie).await;
    let r = pedir(&p.app, "GET", "/api/servidor/respaldo", None, Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!((r.json["activo"].as_bool(), r.json["clave_puesta"].is_null()), (Some(false), true));
    // Sin clave no se puede encender ni hacer.
    let r = pedir(&p.app, "PUT", "/api/servidor/respaldo", Some(json!({ "activo": true })), Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY);
    let r = pedir(&p.app, "POST", "/api/servidor/respaldo/ahora", None, Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY);
    // Pública mala, o sin su sal: no.
    let sal = B64.encode([6u8; 16]);
    let publica = resguardo_protocolo::respaldo_consola::publica("una clave de respaldo larga", &sal).unwrap();
    for cuerpo in [json!({ "publica": "corta", "sal": sal }), json!({ "publica": publica }), json!({ "hora": "25:00" }), json!({ "conservar": 0 })] {
        let r = pedir(&p.app, "PUT", "/api/servidor/respaldo", Some(cuerpo.clone()), Some(&cookie), &[]).await;
        assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY, "{cuerpo}");
    }
    let r = pedir(
        &p.app,
        "PUT",
        "/api/servidor/respaldo",
        Some(json!({ "publica": publica, "sal": sal, "activo": true, "hora": "02:15", "conservar": 3 })),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!((r.json["activo"].as_bool(), r.json["hora"].as_str(), r.json["conservar"].as_u64()), (Some(true), Some("02:15"), Some(3)));
    assert!(r.json["clave_puesta"].is_string() && r.json["proxima"].is_string());
    assert!(!r.json.to_string().contains(&publica), "la pública tampoco se enseña");
    let r = pedir(&p.app, "POST", "/api/servidor/respaldo/ahora", None, Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["ultima"]["ok"], true, "{}", r.json);
    assert_eq!(r.json["copias"].as_array().unwrap().len(), 1);
    let archivo = p.st.datos.join("respaldos").join(r.json["copias"][0]["archivo"].as_str().unwrap());
    let (cab, _) = resguardo_protocolo::respaldo_consola::leer_cabecera(&mut std::fs::File::open(&archivo).unwrap()).unwrap();
    assert_eq!(cab.identidad, p.st.identidad_pub);
    // Otros (aunque sean propietarios de un cliente) no.
    let otro = invitado(&p, &cookie, &c, "propietario").await;
    assert_eq!(pedir(&p.app, "GET", "/api/servidor/respaldo", None, Some(&otro), &[]).await.estado, StatusCode::FORBIDDEN);
    assert_eq!(pedir(&p.app, "POST", "/api/servidor/respaldo/ahora", None, Some(&otro), &[]).await.estado, StatusCode::FORBIDDEN);
    assert_eq!(pedir(&p.app, "GET", "/api/servidor/respaldo", None, None, &[]).await.estado, StatusCode::UNAUTHORIZED);
}

/// v1.23: el equipo sube su historial (al llegar a un servidor nuevo y después,
/// lo que falte). Se guarda una vez por id, los avisos entran ya vistos (sin
/// repetir el que llegó en su momento) y lo que no vale se ignora.
#[tokio::test]
async fn historial_del_equipo_sin_repetir() {
    let p = servidor();
    let cookie = propietario(&p).await;
    let (c, ag) = cliente_con_equipo(&p, &cookie).await;
    let auth = ag.auth();
    let cab = [("authorization", auth.as_str())];
    // Sin nada aún: el sondeo dice que no tiene historial de este equipo.
    let r = pedir(&p.app, "POST", "/api/agente/tomar", Some(json!({ "reto": B64.encode([1u8; 32]) })), None, &cab).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["historial"]["ultima"], Value::Null);
    // Un aviso que llegó en su momento no se repite al subir el historial.
    let r = pedir(&p.app, "POST", "/api/agente/aviso", Some(json!({ "tipo": "intentos_fallidos", "mensaje": "Clave mal 5 veces." })), None, &cab).await;
    assert!(r.estado.is_success(), "{}", r.json);
    let hace = |h: i64| (chrono::Local::now() - chrono::Duration::hours(h)).to_rfc3339();
    let entradas = json!([
        { "id": "a1", "hora": hace(48), "tipo": "copia", "repo": "r1", "copia": "docs", "resultado": "ok", "duracion_s": 90, "anadido": 1000,
          "ganchos": [{ "tipo": "sqlserver", "estado": "ok", "mensaje": "Volcado de 2 bases." }] },
        { "id": "a2", "hora": hace(30), "tipo": "verificacion", "repo": "r1", "resultado": "fallo", "mensaje": "Faltan paquetes." },
        { "id": "a3", "hora": hace(20), "tipo": "aviso", "aviso": "cambio_inusual", "mensaje": "Cambio inusual en «Docs»." },
        { "id": "a4", "hora": chrono::Local::now().to_rfc3339(), "tipo": "aviso", "aviso": "intentos_fallidos", "mensaje": "Clave mal 5 veces." },
        // De hace dos años (lo antiguo, resumido por día en el equipo): también vale.
        { "id": "dia-r1-docs-x", "hora": hace(24 * 730), "tipo": "resumen_dia", "repo": "r1", "copia": "docs", "ok": 3, "fallidas": 1 },
        // No valen: tipo desconocido, de antes del 2000, id raro, aviso de un tipo desconocido.
        { "id": "x1", "hora": hace(1), "tipo": "otra_cosa" },
        { "id": "x2", "hora": "1999-12-31T10:00:00+00:00", "tipo": "copia" },
        { "id": "../x3", "hora": hace(1), "tipo": "copia" },
        { "id": "x4", "hora": hace(1), "tipo": "aviso", "aviso": "inventado", "mensaje": "?" },
    ]);
    let r = pedir(&p.app, "POST", "/api/agente/historial", Some(json!({ "entradas": entradas })), None, &cab).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["nuevas"], 5);
    assert!(r.json["ultima"].is_string());
    // Otra vez lo mismo (otra consola lo pidió, o se cortó a medias): nada nuevo.
    let r = pedir(&p.app, "POST", "/api/agente/historial", Some(json!({ "entradas": entradas })), None, &cab).await;
    assert_eq!(r.json["nuevas"], 0);
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/equipos/{}/historial", ag.id), None, Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let l = r.json.as_array().unwrap();
    assert_eq!(l.len(), 5);
    assert_eq!(l[0]["id"], "a4", "la más reciente primero");
    assert_eq!(l[3]["ganchos"][0]["tipo"], "sqlserver");
    assert_eq!(l[4]["tipo"], "resumen_dia");
    // Con `desde`, solo lo posterior.
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/equipos/{}/historial?desde={}", ag.id, urlenc(&hace(25))), None, Some(&cookie), &[]).await;
    assert_eq!(r.json.as_array().unwrap().len(), 2);
    // v1.26: por páginas, con cursor (`antes`), `hasta` y `tipo`.
    let ruta = format!("/api/clientes/{c}/equipos/{}/historial", ag.id);
    let ids = |v: &Value| v.as_array().unwrap().iter().map(|e| e["id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    let r = pedir(&p.app, "GET", &format!("{ruta}?limite=2"), None, Some(&cookie), &[]).await;
    assert_eq!(ids(&r.json), ["a4", "a3"]);
    let r = pedir(&p.app, "GET", &format!("{ruta}?limite=2&antes=a3"), None, Some(&cookie), &[]).await;
    assert_eq!(ids(&r.json), ["a2", "a1"]);
    let r = pedir(&p.app, "GET", &format!("{ruta}?limite=2&antes=a1"), None, Some(&cookie), &[]).await;
    assert_eq!(ids(&r.json), ["dia-r1-docs-x"], "la última página, más corta");
    let r = pedir(&p.app, "GET", &format!("{ruta}?antes=no-existe"), None, Some(&cookie), &[]).await;
    assert_eq!(ids(&r.json), Vec::<String>::new());
    let r = pedir(&p.app, "GET", &format!("{ruta}?tipo=copia,verificacion"), None, Some(&cookie), &[]).await;
    assert_eq!(ids(&r.json), ["a2", "a1"]);
    let r = pedir(&p.app, "GET", &format!("{ruta}?hasta={}&desde={}", urlenc(&hace(25)), urlenc(&hace(40))), None, Some(&cookie), &[]).await;
    assert_eq!(ids(&r.json), ["a2"]);
    let r = pedir(&p.app, "GET", &format!("{ruta}?tipo=inventado"), None, Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY);
    let r = pedir(&p.app, "GET", &format!("{ruta}?hasta=ayer"), None, Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY);
    // Avisos: el de antes entra ya visto; el que llegó en su momento no se repite.
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/avisos"), None, Some(&cookie), &[]).await;
    let avisos = r.json.as_array().unwrap();
    assert_eq!(avisos.iter().filter(|a| a["tipo"] == "intentos_fallidos").count(), 1, "{avisos:?}");
    let cambio = avisos.iter().find(|a| a["tipo"] == "cambio_inusual").unwrap();
    assert_eq!(cambio["visto_por"], "la consola anterior");
    // Y el sondeo ya sabe hasta dónde tiene.
    let r = pedir(&p.app, "POST", "/api/agente/tomar", Some(json!({ "reto": B64.encode([2u8; 32]) })), None, &cab).await;
    assert!(r.json["historial"]["ultima"].is_string());
    // Demasiadas de una vez: se rechaza.
    let muchas: Vec<Value> = (0..501).map(|i| json!({ "id": format!("m{i}"), "hora": hace(1), "tipo": "copia" })).collect();
    let r = pedir(&p.app, "POST", "/api/agente/historial", Some(json!({ "entradas": muchas })), None, &cab).await;
    assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY);
    // Muchas a la misma hora: las páginas siguen por id, sin saltarse ni repetir ninguna.
    let misma = hace(2);
    let lote: Vec<Value> = (0..300).map(|i| json!({ "id": format!("m{i:03}"), "hora": misma, "tipo": "copia" })).collect();
    let r = pedir(&p.app, "POST", "/api/agente/historial", Some(json!({ "entradas": lote })), None, &cab).await;
    assert_eq!(r.json["nuevas"], 300, "{}", r.json);
    let (mut vistas, mut antes) = (Vec::<String>::new(), String::new());
    loop {
        let r = pedir(&p.app, "GET", &format!("{ruta}?limite=7&antes={antes}"), None, Some(&cookie), &[]).await;
        let pagina = ids(&r.json);
        vistas.extend(pagina.iter().cloned());
        if pagina.len() < 7 {
            break;
        }
        antes = pagina.last().unwrap().clone();
    }
    assert_eq!(vistas.len(), 305);
    assert_eq!(vistas.iter().collect::<std::collections::HashSet<_>>().len(), 305, "sin repetir");
    assert_eq!((vistas[0].as_str(), vistas[1].as_str(), vistas[2].as_str(), vistas[302].as_str()), ("a4", "m000", "m001", "a2"));
    // Sin `limite`, 500 como mucho (antes, 5000: hasta 80 MB); uno mayor que 2000 se queda en 2000.
    let lote: Vec<Value> = (0..300).map(|i| json!({ "id": format!("n{i:03}"), "hora": hace(3), "tipo": "espejo" })).collect();
    let r = pedir(&p.app, "POST", "/api/agente/historial", Some(json!({ "entradas": lote })), None, &cab).await;
    assert_eq!(r.json["nuevas"], 300, "{}", r.json);
    let r = pedir(&p.app, "GET", &ruta, None, Some(&cookie), &[]).await;
    assert_eq!(r.json.as_array().unwrap().len(), 500);
    let r = pedir(&p.app, "GET", &format!("{ruta}?limite=99999"), None, Some(&cookie), &[]).await;
    assert_eq!(r.json.as_array().unwrap().len(), 605);
    // Sin credenciales de equipo, nada.
    let r = pedir(&p.app, "POST", "/api/agente/historial", Some(json!({ "entradas": [] })), None, &[]).await;
    assert_eq!(r.estado, StatusCode::UNAUTHORIZED);
}

/// Los avisos que llegan con el historial tienen su propio tope por día (el de
/// `/api/agente/aviso` es de 60 por hora: el historial no debe saltárselo) y
/// nunca quedan con fecha futura (encima de los demás en la lista).
#[tokio::test]
async fn avisos_del_historial_con_tope_y_sin_fecha_futura() {
    use resguardo_servidor::agentes::MAX_AVISOS_HISTORIAL_DIA as MAX;
    let p = servidor();
    let cookie = propietario(&p).await;
    let (c, ag) = cliente_con_equipo(&p, &cookie).await;
    let auth = ag.auth();
    let cab = [("authorization", auth.as_str())];
    // Ya pasó casi todos los de hoy.
    for _ in 0..MAX - 1 {
        assert!(p.st.limites.intento(&format!("aviso-historial:{}", ag.id), MAX, std::time::Duration::from_secs(86_400)));
    }
    let manana = (chrono::Local::now() + chrono::Duration::hours(20)).to_rfc3339();
    let entradas: Vec<Value> = (0..3)
        .map(|i| json!({ "id": format!("av{i}"), "hora": manana, "tipo": "aviso", "aviso": "cambio_inusual", "mensaje": format!("Aviso {i}") }))
        .collect();
    let r = pedir(&p.app, "POST", "/api/agente/historial", Some(json!({ "entradas": entradas })), None, &cab).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["nuevas"], 3, "el historial entra entero");
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/avisos"), None, Some(&cookie), &[]).await;
    let avisos: Vec<&Value> = r.json.as_array().unwrap().iter().filter(|a| a["mensaje"].as_str().is_some_and(|m| m.starts_with("Aviso "))).collect();
    assert_eq!(avisos.len(), 1, "solo cabía uno más hoy: {avisos:?}");
    let creado = chrono::DateTime::parse_from_rfc3339(avisos[0]["creado"].as_str().unwrap()).unwrap();
    assert!(creado <= chrono::Local::now() + chrono::Duration::seconds(5), "sin fecha futura: {creado}");
}

/// v1.27: el propietario restablece la verificación en dos pasos de alguien que perdió el móvil.
#[tokio::test]
async fn restablecer_verificacion_en_dos_pasos() {
    let p = servidor();
    let (ana, _) = propietario_y_totp(&p).await;
    let r = pedir(&p.app, "POST", "/api/clientes", Some(json!({ "nombre": "Café del Sur", "espera_min_horas": 24 })), Some(&ana), &[]).await;
    let c = r.json["id"].as_str().unwrap().to_string();
    let (dueno, secreto_dueno) = invitado_y_totp(&p, &ana, &c, "propietario").await;
    let (tecnico, _) = invitado_y_totp(&p, &ana, &c, "tecnico").await;
    let (lectura, _) = invitado_y_totp(&p, &ana, &c, "lectura").await;
    let id_de = |correo: &str| {
        let m = p.st.db.cuenta_por_correo(correo).unwrap().unwrap();
        m.id
    };
    let (id_ana, id_dueno, id_tecnico, id_lectura) =
        (id_de("ana@ejemplo.com"), id_de("propietario@ejemplo.com"), id_de("tecnico@ejemplo.com"), id_de("lectura@ejemplo.com"));
    let restablecer = |quien: String, cuenta: String, codigo: String| {
        let (app, c) = (p.app.clone(), c.clone());
        async move {
            pedir(&app, "POST", &format!("/api/clientes/{c}/miembros/{cuenta}/restablecer-totp"), Some(json!({ "codigo": codigo })), Some(&quien), &[]).await
        }
    };
    // El técnico está también en otro cliente («Otro», de Ana): el propietario de Café del Sur no manda en él.
    let r = pedir(&p.app, "POST", "/api/clientes", Some(json!({ "nombre": "Otro", "espera_min_horas": 24 })), Some(&ana), &[]).await;
    let otro = r.json["id"].as_str().unwrap().to_string();
    let r = pedir(&p.app, "POST", &format!("/api/clientes/{otro}/invitaciones"), Some(json!({ "rol": "lectura" })), Some(&ana), &[]).await;
    let token = r.json["enlace"].as_str().unwrap().split('#').nth(1).unwrap().to_string();
    assert_eq!(pedir(&p.app, "POST", "/api/invitaciones/aceptar", Some(json!({ "token": token })), Some(&tecnico), &[]).await.estado, StatusCode::OK);

    // Permisos (antes de pedir el código: no se gasta).
    let nada = "000000".to_string();
    assert_eq!(restablecer(tecnico.clone(), id_lectura.clone(), nada.clone()).await.estado, StatusCode::FORBIDDEN, "un técnico no");
    assert_eq!(restablecer(dueno.clone(), id_dueno.clone(), nada.clone()).await.estado, StatusCode::UNPROCESSABLE_ENTITY, "la suya, no");
    let r = restablecer(dueno.clone(), id_ana.clone(), nada.clone()).await;
    assert_eq!(r.estado, StatusCode::FORBIDDEN, "nunca la del propietario del servidor: {}", r.json);
    let r = restablecer(dueno.clone(), id_tecnico.clone(), nada.clone()).await;
    assert_eq!(r.estado, StatusCode::FORBIDDEN, "ni la de quien está en clientes que no son suyos: {}", r.json);
    assert_eq!(restablecer(dueno.clone(), "no-existe".into(), nada.clone()).await.estado, StatusCode::NOT_FOUND);
    // Paso de más: sin un código bueno del autenticador de quien lo hace, nada.
    let r = restablecer(dueno.clone(), id_lectura.clone(), nada.clone()).await;
    assert_eq!(r.estado, StatusCode::UNAUTHORIZED, "{}", r.json);
    assert_eq!(pedir(&p.app, "GET", "/api/cuenta", None, Some(&lectura), &[]).await.estado, StatusCode::OK, "sin el código no se toca nada");

    // El propietario del cliente, con su código: a la de solo lectura.
    let r = restablecer(dueno.clone(), id_lectura.clone(), codigo_totp(&secreto_dueno, 1)).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let codigo = r.json["codigo"].as_str().unwrap().to_string();
    assert_eq!(codigo.len(), 19, "XXXX-XXXX-XXXX-XXXX");
    let caduca = chrono::DateTime::parse_from_rfc3339(r.json["caduca"].as_str().unwrap()).unwrap().timestamp();
    assert!((caduca - chrono::Utc::now().timestamp() - 24 * 3600).abs() < 60, "24 h");
    // El mismo código de quien lo hace no vale dos veces.
    assert_eq!(restablecer(dueno.clone(), id_lectura.clone(), codigo_totp(&secreto_dueno, 1)).await.estado, StatusCode::UNAUTHORIZED);

    // Fuera sus sesiones, su autenticador y sus códigos de recuperación.
    assert_eq!(pedir(&p.app, "GET", "/api/cuenta", None, Some(&lectura), &[]).await.estado, StatusCode::UNAUTHORIZED);
    let cuenta = p.st.db.cuenta(&id_lectura).unwrap().unwrap();
    assert!(!cuenta.totp_activo && cuenta.totp_secreto.is_none());
    let entrar = || {
        let app = p.app.clone();
        async move {
            let r =
                pedir(&app, "POST", "/api/sesion", Some(json!({ "correo": "lectura@ejemplo.com", "contrasena": "contraseña del invitado" })), None, &[]).await;
            assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
            r
        }
    };
    // Con la contraseña sola no se da de alta otro autenticador: pide el código del propietario.
    let r = entrar().await;
    assert_eq!(r.json["necesita"], "restablecimiento", "{}", r.json);
    assert_eq!(r.json["restablecida"]["por"], "propietario");
    assert!(r.json["restablecida"]["cuando"].is_string());
    assert_eq!(r.json["caducado"], false);
    assert!(r.json.get("totp").is_none());
    let pendiente = r.cookie.unwrap();
    // Ni un código cualquiera ni uno de recuperación.
    assert_eq!(pedir(&p.app, "POST", "/api/sesion/totp", Some(json!({ "codigo": "123456" })), Some(&pendiente), &[]).await.estado, StatusCode::FORBIDDEN);
    assert_eq!(
        pedir(&p.app, "POST", "/api/sesion/totp", Some(json!({ "recuperacion": "abcd-efgh" })), Some(&pendiente), &[]).await.estado,
        StatusCode::FORBIDDEN
    );
    let r = pedir(&p.app, "POST", "/api/sesion/restablecimiento", Some(json!({ "codigo": "AAAA-BBBB-CCCC-DDDD" })), Some(&pendiente), &[]).await;
    assert_eq!(r.estado, StatusCode::UNAUTHORIZED);
    // El del propietario (da igual mayúsculas y guiones): alta de un autenticador nuevo.
    let r =
        pedir(&p.app, "POST", "/api/sesion/restablecimiento", Some(json!({ "codigo": codigo.to_lowercase().replace('-', " ") })), Some(&pendiente), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["necesita"], "alta_totp");
    assert_eq!(r.json["restablecida"]["por"], "propietario");
    let nuevo = r.json["totp"]["secreto"].as_str().unwrap().to_string();
    // Otra sesión a medias (solo con la contraseña) no puede terminar ese alta.
    let otra = entrar().await;
    assert_eq!(otra.json["necesita"], "restablecimiento");
    let otra = otra.cookie.unwrap();
    assert_eq!(
        pedir(&p.app, "POST", "/api/sesion/totp", Some(json!({ "codigo": codigo_totp(&nuevo, 0) })), Some(&otra), &[]).await.estado,
        StatusCode::FORBIDDEN
    );
    // La que dio el código, sí: códigos de recuperación nuevos y el aviso.
    let r = pedir(&p.app, "POST", "/api/sesion/totp", Some(json!({ "codigo": codigo_totp(&nuevo, 0) })), Some(&pendiente), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["codigos_recuperacion"].as_array().unwrap().len(), 10);
    assert_eq!(r.json["restablecida"]["por"], "propietario");
    assert_eq!(pedir(&p.app, "GET", "/api/cuenta", None, Some(&r.cookie.unwrap()), &[]).await.estado, StatusCode::OK);
    assert_eq!(pedir(&p.app, "GET", "/api/cuenta", None, Some(&pendiente), &[]).await.estado, StatusCode::UNAUTHORIZED, "la ficha a medias ya no vale");
    // El código ya se usó: la próxima vez, el autenticador nuevo como siempre.
    let r = entrar().await;
    assert_eq!(r.json["necesita"], "totp", "{}", r.json);
    let r = pedir(&p.app, "POST", "/api/sesion/restablecimiento", Some(json!({ "codigo": codigo })), Some(&r.cookie.unwrap()), &[]).await;
    assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY);

    // El propietario del servidor sí puede con un propietario del cliente.
    let secreto_ana = p.st.db.cuenta(&id_ana).unwrap().unwrap().totp_secreto.unwrap();
    let r = restablecer(ana.clone(), id_dueno.clone(), codigo_totp(&secreto_ana, 1)).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(pedir(&p.app, "GET", "/api/cuenta", None, Some(&dueno), &[]).await.estado, StatusCode::UNAUTHORIZED);
    // Pasadas 24 h, el código ya no sirve (y al entrar se dice).
    let codigo_dueno = r.json["codigo"].as_str().unwrap().to_string();
    let clave = format!("totp_restablecido:{id_dueno}");
    let mut v: Value = serde_json::from_str(&p.st.db.valor(&clave).unwrap().unwrap()).unwrap();
    v["caduca"] = json!(chrono::Utc::now().timestamp() - 1);
    p.st.db.poner_valor(&clave, &v.to_string()).unwrap();
    let r =
        pedir(&p.app, "POST", "/api/sesion", Some(json!({ "correo": "propietario@ejemplo.com", "contrasena": "contraseña del invitado" })), None, &[]).await;
    assert_eq!((r.json["necesita"].as_str(), r.json["caducado"].as_bool()), (Some("restablecimiento"), Some(true)), "{}", r.json);
    let r = pedir(&p.app, "POST", "/api/sesion/restablecimiento", Some(json!({ "codigo": codigo_dueno })), Some(&r.cookie.unwrap()), &[]).await;
    assert_eq!(r.estado, StatusCode::GONE);

    // Queda en la auditoría del cliente (y la cadena sigue entera).
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/auditoria?orden=desc&limite=100"), None, Some(&ana), &[]).await;
    let hechas: Vec<&Value> = r.json.as_array().unwrap().iter().filter(|e| e["accion"] == "restablecer_totp").collect();
    assert_eq!(hechas.len(), 2, "{}", r.json);
    assert!(hechas.iter().any(|e| e["objetivo"] == id_lectura.as_str() && e["actor"] == "cuenta:propietario@ejemplo.com"));
    assert!(hechas.iter().any(|e| e["objetivo"] == id_dueno.as_str() && e["actor"] == "cuenta:ana@ejemplo.com"));
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/auditoria/verificar"), None, Some(&ana), &[]).await;
    assert_eq!(r.json["ok"], true, "{}", r.json);
}
// ---------- Endurecimiento HTTP (ronda 2) ----------

/// Las cabeceras de seguridad van en todas las respuestas: la consola, la API,
/// los errores y la API de los agentes. HSTS solo con HTTPS.
#[tokio::test]
async fn cabeceras_de_seguridad_en_todas_las_respuestas() {
    let p = servidor();
    for (metodo, ruta) in [("GET", "/"), ("GET", "/api/servidor"), ("GET", "/api/no-existe"), ("POST", "/api/agente/informe"), ("POST", "/api/sesion")] {
        let res = pedir_crudo(&p.app, metodo, ruta, Vec::new(), &[]).await;
        let h = res.headers();
        for (k, v) in [
            ("x-content-type-options", "nosniff"),
            ("referrer-policy", "no-referrer"),
            ("x-frame-options", "DENY"),
            ("cross-origin-opener-policy", "same-origin"),
            ("cross-origin-resource-policy", "same-origin"),
            ("x-permitted-cross-domain-policies", "none"),
        ] {
            assert_eq!(h.get(k).and_then(|v| v.to_str().ok()), Some(v), "{metodo} {ruta}: {k}");
        }
        assert!(h.get("permissions-policy").unwrap().to_str().unwrap().contains("camera=()"), "{ruta}");
        let csp = h.get("content-security-policy").unwrap().to_str().unwrap();
        assert!(csp.contains("default-src 'self'") && csp.contains("frame-ancestors 'none'") && csp.contains("object-src 'none'"), "{ruta}");
        assert!(h.get("strict-transport-security").is_none(), "sin HTTPS no hay HSTS");
    }
    let dir = tempfile::tempdir().unwrap();
    let app = api::router(preparar(dir.path(), Opciones { https: true, ..Default::default() }).unwrap());
    let res = pedir_crudo(&app, "GET", "/api/servidor", Vec::new(), &[]).await;
    assert!(res.headers().get("strict-transport-security").unwrap().to_str().unwrap().starts_with("max-age="));
}

/// Lo que cambia algo solo vale desde la propia consola: con `X-Resguardo`,
/// sin `Origin` de otro sitio y, si el navegador lo dice, desde la misma página.
#[tokio::test]
async fn csrf_rechaza_lo_que_viene_de_otro_sitio() {
    let p = servidor();
    let cookie = propietario(&p).await;
    let crear = |extra: Vec<(&'static str, &'static str)>| {
        let (app, cookie) = (p.app.clone(), cookie.clone());
        async move { pedir(&app, "POST", "/api/clientes", Some(json!({ "nombre": "X", "espera_min_horas": 24 })), Some(&cookie), &extra).await.estado }
    };
    assert_eq!(crear(vec![("sec-fetch-site", "cross-site")]).await, StatusCode::FORBIDDEN);
    assert_eq!(crear(vec![("sec-fetch-site", "same-site")]).await, StatusCode::FORBIDDEN);
    assert_eq!(crear(vec![("origin", "https://otro.example")]).await, StatusCode::FORBIDDEN);
    assert_eq!(crear(vec![("origin", "null")]).await, StatusCode::FORBIDDEN);
    assert_eq!(crear(vec![("sec-fetch-site", "same-origin"), ("origin", "http://localhost")]).await, StatusCode::OK);
    // Sin la cabecera propia, nada (aunque lleve la cookie).
    let res = pedir_crudo(&p.app, "POST", "/api/clientes", br#"{"nombre":"Y","espera_min_horas":24}"#.to_vec(), &[("cookie", &cookie)]).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

/// Con HTTPS la cookie de sesión lleva `__Host-`, `Secure`, `HttpOnly`,
/// `SameSite=Strict` y `Path=/`, sin `Domain`; y es la que vale para entrar.
#[tokio::test]
async fn cookie_de_sesion_con_https() {
    let dir = tempfile::tempdir().unwrap();
    let st = preparar(dir.path(), Opciones { https: true, ..Default::default() }).unwrap();
    let app = api::router(st.clone());
    let codigo = api::preparar_codigo_arranque(&st).unwrap().unwrap();
    let cuerpo = json!({ "codigo_arranque": codigo, "correo": "ana@ejemplo.com", "nombre": "Ana", "contrasena": "una contraseña bien larga" });
    let res = pedir_crudo(&app, "POST", "/api/inicio", cuerpo.to_string().into_bytes(), &[("x-resguardo", "1")]).await;
    assert_eq!(res.status(), StatusCode::OK);
    let sc = res.headers().get("set-cookie").unwrap().to_str().unwrap().to_string();
    assert!(sc.starts_with("__Host-resguardo_sesion_"), "{sc}");
    for a in ["; Path=/", "; HttpOnly", "; SameSite=Strict", "; Secure"] {
        assert!(sc.contains(a), "{sc}: falta {a}");
    }
    assert!(!sc.to_lowercase().contains("domain="), "{sc}");
    let cookie = sc.split(';').next().unwrap().to_string();
    let secreto =
        serde_json::from_slice::<Value>(&res.into_body().collect().await.unwrap().to_bytes()).unwrap()["totp"]["secreto"].as_str().unwrap().to_string();
    let r = pedir(&app, "POST", "/api/sesion/totp", Some(json!({ "codigo": codigo_totp(&secreto, 0) })), Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let completa = r.cookie.unwrap();
    assert!(completa.starts_with("__Host-"));
    assert_eq!(pedir(&app, "GET", "/api/cuenta", None, Some(&completa), &[]).await.estado, StatusCode::OK);
    // La misma ficha con el nombre sin prefijo (la pondría una página por HTTP): no.
    let sin_prefijo = completa.trim_start_matches("__Host-").to_string();
    assert_eq!(pedir(&app, "GET", "/api/cuenta", None, Some(&sin_prefijo), &[]).await.estado, StatusCode::UNAUTHORIZED);
}

/// Al cambiar la contraseña, todas las sesiones de la cuenta caen y esta sigue
/// con una ficha nueva.
#[tokio::test]
async fn cambiar_la_contrasena_da_otra_ficha() {
    let p = servidor();
    let cookie = propietario(&p).await;
    let r = pedir(
        &p.app,
        "PUT",
        "/api/cuenta/contrasena",
        Some(json!({ "actual": "una contraseña bien larga", "nueva": "otra contraseña bien larga" })),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(r.estado, StatusCode::NO_CONTENT, "{}", r.json);
    let nueva = r.cookie.unwrap();
    assert_ne!(nueva, cookie);
    assert_eq!(pedir(&p.app, "GET", "/api/cuenta", None, Some(&cookie), &[]).await.estado, StatusCode::UNAUTHORIZED);
    assert_eq!(pedir(&p.app, "GET", "/api/cuenta", None, Some(&nueva), &[]).await.estado, StatusCode::OK);
}

/// Cuerpos de más de 1 MiB (salvo las rutas que lo dicen) o JSON anidado sin
/// fin: un error, sin que el servidor se caiga.
#[tokio::test]
async fn cuerpos_grandes_o_muy_anidados() {
    let p = servidor();
    let cookie = propietario(&p).await;
    let grande = json!({ "nombre": "x".repeat(2 * 1024 * 1024), "espera_min_horas": 24 });
    assert_eq!(pedir(&p.app, "POST", "/api/clientes", Some(grande), Some(&cookie), &[]).await.estado, StatusCode::PAYLOAD_TOO_LARGE);
    let (_, ag) = cliente_con_equipo(&p, &cookie).await;
    let profundo = format!("{{\"datos\":{}{}}}", "[".repeat(200_000), "]".repeat(200_000));
    let res = pedir_crudo(&p.app, "POST", "/api/agente/informe", profundo.into_bytes(), &[("authorization", &ag.auth())]).await;
    assert!(res.status().is_client_error(), "{}", res.status());
    // Y el servidor sigue atendiendo.
    let r = pedir(&p.app, "POST", "/api/agente/informe", Some(json!({ "datos": { "version": "0.7.11" } })), None, &[("authorization", &ag.auth())]).await;
    assert_eq!(r.estado, StatusCode::NO_CONTENT, "{}", r.json);
}

/// Un cuerpo que no termina de llegar no deja la petición colgada (slowloris).
#[tokio::test(start_paused = true)]
async fn un_cuerpo_que_no_llega_corta_la_peticion() {
    let p = servidor();
    let cuerpo = Body::from_stream(futures_util::stream::pending::<Result<axum::body::Bytes, std::io::Error>>());
    let r = Request::builder()
        .method("POST")
        .uri("/api/sesion")
        .header("host", "localhost")
        .header("x-resguardo", "1")
        .header("content-type", "application/json")
        .body(cuerpo)
        .unwrap();
    let res = tokio::time::timeout(api::ESPERA_CUERPO * 2, p.app.clone().oneshot(r)).await.expect("la petición termina").unwrap();
    assert!(res.status().is_client_error(), "{}", res.status());
}

/// Sin TLS (detrás de un proxy o en 127.0.0.1) atiende el mismo bucle que con
/// TLS: con su límite de tiempo para las cabeceras y sus cabeceras de seguridad.
#[tokio::test]
async fn sin_tls_atiende_con_el_mismo_bucle() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let dir = tempfile::tempdir().unwrap();
    let st = preparar(dir.path(), Opciones { https: false, ..Default::default() }).unwrap();
    let puerto = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let addr: std::net::SocketAddr = format!("127.0.0.1:{puerto}").parse().unwrap();
    tokio::spawn(resguardo_servidor::servir(st, addr, resguardo_servidor::Tls::Ninguno));
    let mut s = loop {
        if let Ok(s) = tokio::net::TcpStream::connect(addr).await {
            break s;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    };
    s.write_all(b"GET /api/servidor HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").await.unwrap();
    let mut r = String::new();
    s.read_to_string(&mut r).await.unwrap();
    assert!(r.starts_with("HTTP/1.1 200"), "{r}");
    assert!(r.to_lowercase().contains("x-content-type-options: nosniff"), "{r}");
}

/// Notificaciones (canales, secretos, «Enviar prueba», registro y preferencias): los
/// secretos nunca vuelven por la API ni quedan en claro en la base de datos, lo que cambia a
/// dónde va un canal pide un código del autenticador recién sacado, y cada uno solo toca lo suyo.
#[tokio::test]
async fn notificaciones_canales_secretos_y_preferencias() {
    use resguardo_servidor::notificaciones::transporte::{Enviado, Fallo, Falso};
    let p = servidor();
    let falso = std::sync::Arc::new(Falso::default());
    p.st.notif.poner_transporte(falso.clone());
    let (ana, secreto_ana) = propietario_y_totp(&p).await;
    let r = pedir(&p.app, "POST", "/api/clientes", Some(json!({ "nombre": "Café del Sur", "espera_min_horas": 24 })), Some(&ana), &[]).await;
    let c = r.json["id"].as_str().unwrap().to_string();
    let (dueno, secreto_dueno) = invitado_y_totp(&p, &ana, &c, "propietario").await;
    let (tecnico, _) = invitado_y_totp(&p, &ana, &c, "tecnico").await;
    let id_de = |correo: &str| p.st.db.cuenta_por_correo(correo).unwrap().unwrap().id;
    let (id_ana, id_tecnico) = (id_de("ana@ejemplo.com"), id_de("tecnico@ejemplo.com"));
    // Un código «recién sacado» otra vez (como si pasaran 30 s).
    let otro_codigo = |cuenta: &str, secreto: &str| {
        p.st.db.poner_totp_ultimo(cuenta, 0).unwrap();
        codigo_totp(secreto, 0)
    };

    // Ajustes del servidor: solo su propietario.
    let r = pedir(
        &p.app,
        "PUT",
        "/api/servidor/notificaciones",
        Some(json!({ "url_consola": "https://copias.ejemplo.com:8443/", "max_por_hora": 5 })),
        Some(&ana),
        &[],
    )
    .await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["url_consola"], "https://copias.ejemplo.com:8443");
    assert_eq!(r.json["max_por_hora"], 5);
    let r = pedir(&p.app, "PUT", "/api/servidor/notificaciones", Some(json!({ "url_consola": "javascript:alert(1)" })), Some(&ana), &[]).await;
    assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(pedir(&p.app, "GET", "/api/servidor/notificaciones", None, Some(&dueno), &[]).await.estado, StatusCode::FORBIDDEN);

    // Un canal de correo: sin el código, no.
    let correo = json!({
        "tipo": "correo", "nombre": "Oficina",
        "config": { "host": "smtp.ejemplo.com", "seguridad": "starttls", "usuario": "avisos@ejemplo.com", "remitente": "Resguardo <avisos@ejemplo.com>" },
        "secretos": { "contrasena": "CLAVE-SMTP-SECRETA" },
    });
    let r = pedir(&p.app, "POST", "/api/servidor/notificaciones/canales", Some(correo.clone()), Some(&ana), &[]).await;
    assert_eq!((r.estado, r.json["error"].as_str()), (StatusCode::UNAUTHORIZED, Some("codigo")), "{}", r.json);
    let mut con_codigo = correo.clone();
    con_codigo["codigo"] = json!(codigo_totp(&secreto_ana, 1));
    let r = pedir(&p.app, "POST", "/api/servidor/notificaciones/canales", Some(con_codigo.clone()), Some(&ana), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["secretos"]["contrasena"], "configurado");
    assert_eq!(r.json["completo"], true);
    assert!(!r.json.to_string().contains("CLAVE-SMTP"), "{}", r.json);
    let k = r.json["id"].as_str().unwrap().to_string();
    // El mismo código no vale dos veces.
    let r = pedir(&p.app, "POST", "/api/servidor/notificaciones/canales", Some(con_codigo), Some(&ana), &[]).await;
    assert_eq!(r.estado, StatusCode::UNAUTHORIZED);
    // Ni la API ni la base de datos lo tienen en claro.
    let r = pedir(&p.app, "GET", "/api/servidor/notificaciones", None, Some(&ana), &[]).await;
    assert!(!String::from_utf8_lossy(&r.bytes).contains("CLAVE-SMTP") && !String::from_utf8_lossy(&r.bytes).contains("n1:"), "{}", r.json);
    assert_eq!(r.json["canales"][0]["config"]["host"], "smtp.ejemplo.com");
    assert!(!p.st.db.valor("notif:ajustes").unwrap().unwrap().contains("CLAVE-SMTP"));
    // Nombre o apagar: sin código. Otro servidor de correo o la contraseña: con código.
    let ruta = format!("/api/servidor/notificaciones/canales/{k}");
    let r = pedir(&p.app, "PATCH", &ruta, Some(json!({ "nombre": "Correo de la oficina" })), Some(&ana), &[]).await;
    assert_eq!((r.estado, r.json["nombre"].as_str()), (StatusCode::OK, Some("Correo de la oficina")), "{}", r.json);
    let mut otro_host = correo["config"].clone();
    otro_host["host"] = json!("smtp.de-otro.com");
    for cambio in [json!({ "config": otro_host }), json!({ "secretos": { "contrasena": "OTRA-CLAVE" } })] {
        let r = pedir(&p.app, "PATCH", &ruta, Some(cambio), Some(&ana), &[]).await;
        assert_eq!(r.estado, StatusCode::UNAUTHORIZED, "{}", r.json);
    }
    let r = pedir(
        &p.app,
        "PATCH",
        &ruta,
        Some(json!({ "secretos": { "contrasena": "OTRA-CLAVE" }, "codigo": otro_codigo(&id_ana, &secreto_ana) })),
        Some(&ana),
        &[],
    )
    .await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert!(!r.json.to_string().contains("OTRA-CLAVE"));

    // «Enviar prueba»: al correo de quien la pide; queda en el registro.
    let r = pedir(&p.app, "POST", &format!("{ruta}/prueba"), None, Some(&ana), &[]).await;
    assert_eq!((r.estado, r.json["ok"].as_bool()), (StatusCode::OK, Some(true)), "{}", r.json);
    match &falso.enviados()[..] {
        [Enviado::Correo { para, asunto, crudo, .. }] => {
            assert_eq!(para, "ana@ejemplo.com");
            assert_eq!(asunto, "Prueba de notificaciones de Resguardo");
            assert!(!crudo.contains("OTRA-CLAVE"));
        }
        otro => panic!("{otro:?}"),
    }
    falso.fallar(Fallo { permanente: true, texto: "El servidor de correo no aceptó el usuario o la contraseña (535).".into() });
    let r = pedir(&p.app, "POST", &format!("{ruta}/prueba"), None, Some(&ana), &[]).await;
    assert_eq!(r.json["ok"], false);
    assert!(r.json["mensaje"].as_str().unwrap().contains("535"));
    let r = pedir(&p.app, "GET", "/api/servidor/notificaciones/registro", None, Some(&ana), &[]).await;
    let estados: Vec<&str> = r.json.as_array().unwrap().iter().map(|e| e["estado"].as_str().unwrap()).collect();
    assert_eq!(estados, vec!["fallido", "enviado"], "{}", r.json);
    assert_eq!(r.json[0]["canal"]["nombre"], "Correo de la oficina");
    assert_eq!(r.json[0]["destino"], "ana@ejemplo.com");
    assert!(!r.json.to_string().contains("OTRA-CLAVE"));
    assert_eq!(pedir(&p.app, "GET", "/api/servidor/notificaciones/registro", None, Some(&dueno), &[]).await.estado, StatusCode::FORBIDDEN);

    // Un webhook del cliente, de su propietario (con su código); el técnico, no.
    let webhook = json!({
        "tipo": "webhook", "nombre": "Avisos de Café del Sur",
        "secretos": { "url": "https://hooks.ejemplo.com/servicios/TOKEN-EN-LA-URL", "secreto": "secreto-para-firmar-1234" },
        "reglas": { "severidades": ["critico"] },
        "codigo": codigo_totp(&secreto_dueno, 1),
    });
    let rc = format!("/api/clientes/{c}/notificaciones");
    assert_eq!(pedir(&p.app, "POST", &format!("{rc}/canales"), Some(webhook.clone()), Some(&tecnico), &[]).await.estado, StatusCode::FORBIDDEN);
    let r = pedir(&p.app, "POST", &format!("{rc}/canales"), Some(webhook), Some(&dueno), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["config"]["servidor"], "hooks.ejemplo.com");
    assert_eq!(r.json["secretos"], json!({ "secreto": "configurado", "url": "configurado" }));
    let kw = r.json["id"].as_str().unwrap().to_string();
    let r = pedir(&p.app, "GET", &rc, None, Some(&dueno), &[]).await;
    assert_eq!(r.estado, StatusCode::OK);
    assert!(!r.json.to_string().contains("TOKEN-EN-LA-URL") && !r.json.to_string().contains("secreto-para"), "{}", r.json);
    assert_eq!(r.json["correo"]["de"], "servidor");
    // La prueba del webhook va firmada.
    let r = pedir(&p.app, "POST", &format!("{rc}/canales/{kw}/prueba"), None, Some(&dueno), &[]).await;
    assert_eq!(r.json["ok"], true, "{}", r.json);
    match falso.enviados().last().unwrap() {
        Enviado::Http(h) => {
            assert_eq!(h.url, "https://hooks.ejemplo.com/servicios/TOKEN-EN-LA-URL");
            assert!(h.cabeceras.iter().any(|(k, v)| k == "X-Resguardo-Firma" && v.starts_with("t=") && v.contains(",v1=")));
        }
        otro => panic!("{otro:?}"),
    }
    let r = pedir(&p.app, "GET", &format!("{rc}/registro"), None, Some(&dueno), &[]).await;
    assert_eq!(r.json.as_array().unwrap().len(), 1, "solo lo de este cliente: {}", r.json);
    assert_eq!(r.json[0]["destino"], "hooks.ejemplo.com");

    // Preferencias de las personas: por defecto según el papel; el propietario las cambia y cada uno las suyas.
    let r = pedir(&p.app, "GET", &format!("{rc}/personas"), None, Some(&dueno), &[]).await;
    let de = |cuenta: &str| r.json.as_array().unwrap().iter().find(|x| x["cuenta"] == cuenta).unwrap()["preferencias"].clone();
    assert_eq!(de(&id_ana), json!({ "inmediatos": ["critico", "importante"], "resumen": true, "propias": false }));
    assert_eq!(de(&id_tecnico), json!({ "inmediatos": ["critico"], "resumen": false, "propias": false }));
    assert_eq!(pedir(&p.app, "GET", &format!("{rc}/personas"), None, Some(&tecnico), &[]).await.estado, StatusCode::FORBIDDEN);
    let prefs = json!({ "inmediatos": ["critico", "importante", "informativo"], "resumen": true });
    let r = pedir(&p.app, "PUT", &format!("{rc}/personas/{id_tecnico}"), Some(prefs.clone()), Some(&dueno), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(pedir(&p.app, "PUT", &format!("{rc}/personas/{id_ana}"), Some(prefs.clone()), Some(&tecnico), &[]).await.estado, StatusCode::FORBIDDEN);
    let r = pedir(&p.app, "PUT", &format!("{rc}/personas/{id_tecnico}"), Some(json!({ "inmediatos": [], "resumen": false })), Some(&tecnico), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "cada uno las suyas: {}", r.json);
    // Las mías: horas de silencio y resúmenes.
    let r = pedir(
        &p.app,
        "PUT",
        "/api/cuenta/notificaciones",
        Some(json!({ "silencio": { "desde": "22:00", "hasta": "07:00" }, "resumen_diario": true })),
        Some(&tecnico),
        &[],
    )
    .await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["silencio"], json!({ "desde": "22:00", "hasta": "07:00", "salvo_criticos": true }));
    assert_eq!(r.json["clientes"][0]["preferencias"]["inmediatos"], json!([]));
    assert_eq!(r.json["clientes"][0]["correo"], true);
    let r = pedir(&p.app, "PUT", "/api/cuenta/notificaciones", Some(json!({ "silencio": { "desde": "25:00", "hasta": "07:00" } })), Some(&tecnico), &[]).await;
    assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY);
    let r = pedir(&p.app, "PUT", "/api/cuenta/notificaciones", Some(json!({ "silencio": null })), Some(&tecnico), &[]).await;
    assert_eq!((r.json["silencio"].is_null(), r.json["resumen_diario"].as_bool()), (true, Some(true)), "{}", r.json);

    // Quitar un canal.
    assert_eq!(pedir(&p.app, "DELETE", &format!("{rc}/canales/{kw}"), None, Some(&dueno), &[]).await.estado, StatusCode::NO_CONTENT);
    assert_eq!(pedir(&p.app, "DELETE", &format!("{rc}/canales/{kw}"), None, Some(&dueno), &[]).await.estado, StatusCode::NOT_FOUND);
    // Todo en la auditoría del cliente, sin secretos.
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/auditoria?orden=desc&limite=20"), None, Some(&dueno), &[]).await;
    let acciones: Vec<&str> = r.json.as_array().unwrap().iter().map(|e| e["accion"].as_str().unwrap()).collect();
    assert!(acciones.contains(&"notificaciones_canal") && acciones.contains(&"notificaciones_preferencias"), "{acciones:?}");
    assert!(!r.json.to_string().contains("TOKEN-EN-LA-URL") && !r.json.to_string().contains("secreto-para"));
}

/// Un PNG con la forma justa (firma, IHDR y IEND) para las pruebas de la marca.
fn png_de_prueba(ancho: u32, alto: u32) -> Vec<u8> {
    let mut b = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
    b.extend_from_slice(b"IHDR");
    b.extend_from_slice(&ancho.to_be_bytes());
    b.extend_from_slice(&alto.to_be_bytes());
    b.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    b.extend_from_slice(&[0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82]);
    b
}

/// v1.32: la marca del cliente (logo PNG y acento). La ven todos sus miembros;
/// la cambian propietarios y administradores; solo PNG de verdad, de hasta
/// 200 KB, y un acento de los de la consola. Queda en la auditoría.
#[tokio::test]
async fn marca_del_cliente() {
    let p = servidor();
    let cookie = propietario(&p).await;
    let (c, _ag) = cliente_con_equipo(&p, &cookie).await;
    let tecnico = invitado(&p, &cookie, &c, "tecnico").await;
    let admin = invitado(&p, &cookie, &c, "administrador").await;
    let ruta = format!("/api/clientes/{c}/marca");

    // Sin marca: todo a null, también en el cliente.
    let r = pedir(&p.app, "GET", &ruta, None, Some(&tecnico), &[]).await;
    assert_eq!((r.estado, r.json["acento"].is_null(), r.json["logo"].is_null()), (StatusCode::OK, true, true), "{}", r.json);
    assert!(pedir(&p.app, "GET", &format!("/api/clientes/{c}"), None, Some(&cookie), &[]).await.json["marca"]["logo"].is_null());

    // Un técnico no la cambia; un administrador sí.
    let png = png_de_prueba(120, 40);
    let cuerpo = json!({ "acento": "violet", "logo": B64.encode(&png) });
    assert_eq!(pedir(&p.app, "PUT", &ruta, Some(cuerpo.clone()), Some(&tecnico), &[]).await.estado, StatusCode::FORBIDDEN);
    let r = pedir(&p.app, "PUT", &ruta, Some(cuerpo), Some(&admin), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["acento"], "violet");
    let url = r.json["logo"].as_str().unwrap().to_string();
    assert!(url.starts_with(&format!("/api/clientes/{c}/marca/logo?v=")), "{url}");

    // El logo, tal cual y como imagen PNG (con nosniff), para cualquier miembro.
    let res = pedir_crudo(&p.app, "GET", &url, Vec::new(), &[("cookie", tecnico.as_str())]).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers().get("content-type").unwrap(), "image/png");
    assert_eq!(res.headers().get("x-content-type-options").unwrap(), "nosniff");
    assert!(res.headers().get("cache-control").unwrap().to_str().unwrap().starts_with("private"));
    assert_eq!(res.into_body().collect().await.unwrap().to_bytes().to_vec(), png);
    // Sin sesión, nada.
    assert_eq!(pedir(&p.app, "GET", &url, None, None, &[]).await.estado, StatusCode::UNAUTHORIZED);

    // En la lista de clientes (el selector) y en el cliente.
    let l = pedir(&p.app, "GET", "/api/clientes", None, Some(&cookie), &[]).await.json;
    let x = l.as_array().unwrap().iter().find(|x| x["id"] == c.as_str()).unwrap();
    assert_eq!((x["marca"]["acento"].as_str(), x["marca"]["logo"].as_str()), (Some("violet"), Some(url.as_str())));

    // Ni colores fuera de la lista, ni SVG, ni PNG dañados, enormes o de más de 200 KB.
    let svg = B64.encode(b"<svg xmlns=\"http://www.w3.org/2000/svg\" onload=\"alert(1)\"><script>alert(1)</script></svg>");
    let mut grande = png_de_prueba(100, 100);
    grande.splice(33..33, vec![0u8; 210 * 1024]);
    let mut cortado = png_de_prueba(10, 10);
    cortado.truncate(30);
    for malo in [
        json!({ "acento": "#ff0000" }),
        json!({ "acento": null, "logo": svg }),
        json!({ "acento": null, "logo": "no es base64" }),
        json!({ "acento": null, "logo": B64.encode(&cortado) }),
        json!({ "acento": null, "logo": B64.encode(png_de_prueba(5000, 10)) }),
        json!({ "acento": null, "logo": B64.encode(&grande) }),
    ] {
        let r = pedir(&p.app, "PUT", &ruta, Some(malo.clone()), Some(&cookie), &[]).await;
        assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY, "{malo}");
    }
    // Lo rechazado no cambió nada.
    assert_eq!(pedir(&p.app, "GET", &ruta, None, Some(&cookie), &[]).await.json["logo"].as_str(), Some(url.as_str()));

    // Cambiar solo el acento deja el logo; quitarlo lo quita.
    let r = pedir(&p.app, "PUT", &ruta, Some(json!({ "acento": "amber" })), Some(&cookie), &[]).await;
    assert_eq!((r.json["acento"].as_str(), r.json["logo"].as_str()), (Some("amber"), Some(url.as_str())));
    let r = pedir(&p.app, "PUT", &ruta, Some(json!({ "acento": null, "quitar_logo": true })), Some(&cookie), &[]).await;
    assert!(r.json["acento"].is_null() && r.json["logo"].is_null(), "{}", r.json);
    assert_eq!(pedir(&p.app, "GET", &url, None, Some(&cookie), &[]).await.estado, StatusCode::NOT_FOUND);

    let a = pedir(&p.app, "GET", &format!("/api/clientes/{c}/auditoria?orden=desc&limite=10"), None, Some(&cookie), &[]).await.json;
    assert_eq!(a.as_array().unwrap().iter().filter(|x| x["accion"] == "cambiar_marca").count(), 3);
}
