//! Bloque 7 (0.7.26): código de alta para varios equipos. Lo genera el navegador (al servidor
//! solo le llega su hash), vale para N equipos y X días, se anula, cada uso queda en la
//! auditoría y cada equipo que se une espera su confirmación (un emparejamiento por equipo).
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use base64::Engine;
use http_body_util::BodyExt;
use resguardo_protocolo::{claves, mensajes};
use resguardo_servidor::almacen::Lote;
use resguardo_servidor::estado::{Opciones, St};
use resguardo_servidor::{api, auth, preparar};
use serde_json::{json, Value};
use tower::ServiceExt;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
const INSTALADOR: &[u8] = b"MZ instalador de prueba para la linea de PowerShell";

struct Prueba {
    dir: tempfile::TempDir,
    st: St,
    app: Router,
    /// Todo lo que el servidor contestó (para comprobar que nunca da el código).
    respuestas: std::sync::Mutex<Vec<u8>>,
}

struct Resp {
    estado: StatusCode,
    json: Value,
    cookie: Option<String>,
    bytes: Vec<u8>,
}

fn servidor() -> Prueba {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("tls")).unwrap();
    std::fs::write(dir.path().join("tls").join("ca.huella"), vec!["5A"; 32].join(":")).unwrap();
    let exe = dir.path().join("Resguardo-Agente-setup.exe");
    std::fs::write(&exe, INSTALADOR).unwrap();
    let st = preparar(dir.path(), Opciones { https: false, instalador_agente: Some(exe), ..Default::default() }).unwrap();
    Prueba { dir, st: st.clone(), app: api::router(st), respuestas: Default::default() }
}

impl Prueba {
    async fn pedir(&self, metodo: &str, ruta: &str, cuerpo: Option<Value>, cookie: Option<&str>) -> Resp {
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
        let res = self.app.clone().oneshot(r.body(body).unwrap()).await.unwrap();
        let estado = res.status();
        let cookie = res.headers().get("set-cookie").and_then(|v| v.to_str().ok()).and_then(|v| v.split(';').next()).map(str::to_string);
        let bytes = res.into_body().collect().await.unwrap().to_bytes().to_vec();
        self.respuestas.lock().unwrap().extend_from_slice(&bytes);
        let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        Resp { estado, json, cookie, bytes }
    }

    async fn propietario_y_cliente(&self) -> (String, String) {
        let codigo = api::preparar_codigo_arranque(&self.st).unwrap().unwrap();
        let cuerpo = json!({ "codigo_arranque": codigo, "correo": "ana@ejemplo.com", "nombre": "Ana", "contrasena": "una contraseña bien larga" });
        let r = self.pedir("POST", "/api/inicio", Some(cuerpo), None).await;
        let cookie = r.cookie.unwrap();
        let secreto = r.json["totp"]["secreto"].as_str().unwrap().to_string();
        let paso = chrono::Utc::now().timestamp() / 30;
        let totp = format!("{:06}", auth::hotp(&auth::de_base32(&secreto).unwrap(), paso as u64));
        let r = self.pedir("POST", "/api/sesion/totp", Some(json!({ "codigo": totp })), Some(&cookie)).await;
        assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
        let cookie = r.cookie.unwrap();
        let r = self.pedir("POST", "/api/clientes", Some(json!({ "nombre": "Ferretería Altamar", "espera_min_horas": 24 })), Some(&cookie)).await;
        (cookie, r.json["id"].as_str().unwrap().to_string())
    }

    async fn tecnico(&self, cookie: &str, c: &str) -> String {
        let r = self.pedir("POST", &format!("/api/clientes/{c}/invitaciones"), Some(json!({ "rol": "tecnico" })), Some(cookie)).await;
        let token = r.json["enlace"].as_str().unwrap().split('#').nth(1).unwrap().to_string();
        let cuerpo = json!({ "token": token, "correo": "tecnico@ejemplo.com", "nombre": "Técnico", "contrasena": "contraseña del invitado" });
        let r = self.pedir("POST", "/api/invitaciones/aceptar", Some(cuerpo), None).await;
        let cookie2 = r.cookie.unwrap();
        let secreto = r.json["totp"]["secreto"].as_str().unwrap().to_string();
        let paso = chrono::Utc::now().timestamp() / 30;
        let totp = format!("{:06}", auth::hotp(&auth::de_base32(&secreto).unwrap(), paso as u64));
        self.pedir("POST", "/api/sesion/totp", Some(json!({ "codigo": totp })), Some(&cookie2)).await.cookie.unwrap()
    }

    /// `POST /api/agente/unirse` como un agente (solo el hash). `sas_version: None`, como uno anterior a 0.7.10.
    async fn unirse(&self, codigo: &str, nombre: &str, sas_version: Option<i64>) -> Resp {
        let box_pub = claves::public_of(&claves::new_key()).unwrap();
        let mut cuerpo = json!({
            "codigo_hash": mensajes::code_hash(codigo), "nombre": nombre, "so": "windows", "version": "0.7.21",
            "box_pub": box_pub, "sign_pub": B64.encode(rand_32()), "sal_equipo": B64.encode([3u8; 16]),
        });
        if let Some(v) = sas_version {
            cuerpo["sas_version"] = json!(v);
        }
        self.pedir("POST", "/api/agente/unirse", Some(cuerpo), None).await
    }

    fn lo_vio(&self, codigo: &str) -> bool {
        let todo = String::from_utf8_lossy(&self.respuestas.lock().unwrap()).to_uppercase();
        todo.contains(&codigo.to_uppercase()) || todo.contains(&normalizado(codigo))
    }

    async fn auditoria(&self, cookie: &str, c: &str) -> Vec<Value> {
        self.pedir("GET", &format!("/api/clientes/{c}/auditoria?limite=1000"), None, Some(cookie)).await.json.as_array().unwrap().clone()
    }
}

fn rand_32() -> [u8; 32] {
    let mut b = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rng(), &mut b);
    b
}

fn normalizado(codigo: &str) -> String {
    codigo.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_uppercase()
}

/// Lo que haría el navegador: un código de 16 letras y cifras y su hash.
fn codigo_del_navegador(n: u8) -> (String, String) {
    let a = mensajes::ALFABETO_CODIGO;
    let s: String = (0..16).map(|i| a[(usize::from(n) * 11 + i * 7) % a.len()] as char).collect();
    let codigo = format!("{}-{}-{}-{}", &s[..4], &s[4..8], &s[8..12], &s[12..]);
    let hash = mensajes::code_hash(&codigo);
    (codigo, hash)
}

/// Todos los bytes de la carpeta de datos del servidor (bases, WAL…).
fn todo_lo_guardado(dir: &std::path::Path) -> Vec<u8> {
    let mut out = Vec::new();
    let mut pila = vec![dir.to_path_buf()];
    while let Some(d) = pila.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                pila.push(p);
            } else if let Ok(b) = std::fs::read(&p) {
                out.extend(b);
            }
        }
    }
    out
}

#[tokio::test]
async fn un_codigo_para_varios_equipos_cada_uno_espera_su_confirmacion() {
    let p = servidor();
    let (cookie, c) = p.propietario_y_cliente().await;
    assert_eq!(p.pedir("GET", "/api/servidor", None, None).await.json["codigo_varios"], true);
    let ruta = format!("/api/clientes/{c}/codigos-varios");
    let (codigo, hash) = codigo_del_navegador(1);

    let r = p.pedir("POST", &ruta, Some(json!({ "codigo_hash": hash, "usos": 2, "dias": 7, "nombre": "Planta 2" })), Some(&cookie)).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let lote = r.json["id"].as_str().unwrap().to_string();
    assert_eq!(
        (r.json["usos"].as_i64(), r.json["usados"].as_i64(), r.json["quedan"].as_i64(), r.json["estado"].as_str(), r.json["nombre"].as_str()),
        (Some(2), Some(0), Some(2), Some("activo"), Some("Planta 2"))
    );
    assert!(r.json.get("codigo").is_none(), "el servidor no tiene el código");
    // El mismo hash otra vez: 409; y no puede pisar un código de un solo uso (ni al revés).
    let r = p.pedir("POST", &ruta, Some(json!({ "codigo_hash": hash, "usos": 2, "dias": 7 })), Some(&cookie)).await;
    assert_eq!((r.estado, r.json["error"].as_str()), (StatusCode::CONFLICT, Some("codigo_repetido")));
    let r = p.pedir("POST", &format!("/api/clientes/{c}/emparejamientos"), Some(json!({ "codigo_hash": hash })), Some(&cookie)).await;
    assert_eq!((r.estado, r.json["error"].as_str()), (StatusCode::CONFLICT, Some("codigo_repetido")));

    // Dos equipos con el mismo código (uno de ellos, un agente anterior sin `sas_version`).
    let a = p.unirse(&codigo, "RECEPCION-01", Some(3)).await;
    assert_eq!(a.estado, StatusCode::OK, "{}", a.json);
    let b = p.unirse(&codigo, "RECEPCION-02", None).await;
    assert_eq!(b.estado, StatusCode::OK, "{}", b.json);
    assert_eq!((a.json["sas_version"].as_i64(), b.json["sas_version"].as_i64()), (Some(3), Some(2)));
    assert_ne!(a.json["equipo_id"], b.json["equipo_id"]);
    // El tercero: agotado (404 «codigo», como uno que no vale).
    let tercero = p.unirse(&codigo, "RECEPCION-03", Some(3)).await;
    assert_eq!((tercero.estado, tercero.json["error"].as_str()), (StatusCode::NOT_FOUND, Some("codigo")));

    // La consola ve los dos esperando, cada uno con su emparejamiento, su número y sus llaves.
    let v = p.pedir("GET", &format!("{ruta}/{lote}"), None, Some(&cookie)).await.json;
    assert_eq!((v["usados"].as_i64(), v["quedan"].as_i64(), v["estado"].as_str(), v["pendientes"].as_u64()), (Some(2), Some(0), Some("agotado"), Some(2)));
    assert_eq!(v["rechazos"].as_i64(), Some(1));
    let equipos = v["equipos"].as_array().unwrap();
    assert_eq!(equipos.len(), 2);
    let ids: Vec<&str> = equipos.iter().map(|e| e["id"].as_str().unwrap()).collect();
    assert_ne!(ids[0], ids[1], "un emparejamiento por equipo");
    for e in equipos {
        assert_eq!(e["estado"], "unido");
        assert!(e["equipo"]["box_pub"].is_string() && e["equipo"]["sal_equipo"].is_string());
        assert!(e["sas"].as_str().is_some_and(|s| s.len() == 7));
        assert!(e.get("ip").is_some());
    }
    // Ninguno está confirmado: nada entra solo.
    let lista = p.pedir("GET", &format!("/api/clientes/{c}/equipos"), None, Some(&cookie)).await.json;
    assert!(lista.as_array().unwrap().iter().all(|e| e["confirmado"] == false), "{lista}");
    // Cada emparejamiento, como uno del navegador: el hash, nunca el código (para la `prueba_codigo`).
    let emp = p.pedir("GET", &format!("/api/clientes/{c}/emparejamientos/{}", ids[0]), None, Some(&cookie)).await.json;
    assert_eq!((emp["codigo_hash"].as_str(), emp["codigo_navegador"].as_bool(), emp["lote"].as_str()), (Some(hash.as_str()), Some(true), Some(lote.as_str())));
    // «A medias» los marca con su lote (la consola los enseña en la página del código).
    let medias = p.pedir("GET", &format!("/api/clientes/{c}/a-medias"), None, Some(&cookie)).await.json;
    assert_eq!(medias.as_array().unwrap().iter().filter(|m| m["lote"] == json!(lote)).count(), 2);
    // No salen como «el código de 15 min de esta cuenta».
    let abierto = p.pedir("GET", &format!("/api/clientes/{c}/codigo-abierto?navegador=1"), None, Some(&cookie)).await.json;
    assert!(abierto.is_null(), "{abierto}");

    // Confirmar uno (como siempre) y rechazar el otro.
    let r = p
        .pedir("POST", &format!("/api/clientes/{c}/emparejamientos/{}/confirmar", ids[0]), Some(json!({ "etiqueta": B64.encode([5u8; 32]) })), Some(&cookie))
        .await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let r = p.pedir("DELETE", &format!("/api/clientes/{c}/emparejamientos/{}", ids[1]), None, Some(&cookie)).await;
    assert_eq!(r.estado, StatusCode::NO_CONTENT);
    let v = p.pedir("GET", &format!("{ruta}/{lote}"), None, Some(&cookie)).await.json;
    let estados: Vec<&str> = v["equipos"].as_array().unwrap().iter().map(|e| e["estado"].as_str().unwrap()).collect();
    assert!(estados.contains(&"confirmado") && estados.contains(&"cancelado"), "{estados:?}");
    assert_eq!(v["pendientes"].as_u64(), Some(0));
    assert_eq!(p.pedir("GET", &format!("/api/clientes/{c}/equipos"), None, Some(&cookie)).await.json.as_array().unwrap().len(), 1, "el rechazado se quita");

    // Auditoría: la creación, cada uso (con su lote) y el rechazo.
    let aud = p.auditoria(&cookie, &c).await;
    let acciones = |a: &str| aud.iter().filter(|e| e["accion"] == a).count();
    assert_eq!(acciones("crear_codigo_varios"), 1);
    assert_eq!(aud.iter().filter(|e| e["accion"] == "unirse" && e["datos"].as_str().is_some_and(|d| d.contains(&lote))).count(), 2);
    assert_eq!(acciones("codigo_varios_rechazado"), 1);

    // El servidor nunca tuvo el código: ni en sus respuestas ni en lo que guarda.
    assert!(!p.lo_vio(&codigo));
    let guardado = String::from_utf8_lossy(&todo_lo_guardado(p.dir.path())).to_uppercase();
    assert!(!guardado.contains(&normalizado(&codigo)) && !guardado.contains(&codigo.to_uppercase()), "el código no está en la carpeta de datos");
}

#[tokio::test]
async fn anular_caducar_limites_y_permisos() {
    let p = servidor();
    let (cookie, c) = p.propietario_y_cliente().await;
    let ruta = format!("/api/clientes/{c}/codigos-varios");
    let (codigo, hash) = codigo_del_navegador(2);

    // Datos que no valen.
    for malo in [
        json!({ "codigo_hash": hash, "usos": 0, "dias": 7 }),
        json!({ "codigo_hash": hash, "usos": 101, "dias": 7 }),
        json!({ "codigo_hash": hash, "usos": 10, "dias": 0 }),
        json!({ "codigo_hash": hash, "usos": 10, "dias": 31 }),
        json!({ "codigo_hash": "abc", "usos": 10, "dias": 7 }),
        json!({ "codigo_hash": hash, "usos": 10, "dias": 7, "nombre": "a\"b" }),
    ] {
        let r = p.pedir("POST", &ruta, Some(malo.clone()), Some(&cookie)).await;
        assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY, "{malo}: {}", r.json);
    }
    // Un técnico no puede crearlos, verlos ni anularlos.
    let tecnico = p.tecnico(&cookie, &c).await;
    assert_eq!(p.pedir("POST", &ruta, Some(json!({ "codigo_hash": hash, "usos": 10, "dias": 7 })), Some(&tecnico)).await.estado, StatusCode::FORBIDDEN);
    assert_eq!(p.pedir("GET", &ruta, None, Some(&tecnico)).await.estado, StatusCode::FORBIDDEN);

    let r = p.pedir("POST", &ruta, Some(json!({ "codigo_hash": hash, "usos": 10, "dias": 7 })), Some(&cookie)).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let lote = r.json["id"].as_str().unwrap().to_string();
    assert_eq!(p.pedir("DELETE", &format!("{ruta}/{lote}"), None, Some(&tecnico)).await.estado, StatusCode::FORBIDDEN);

    // La descarga del instalador sin sesión, solo con el lote activo; su huella, con sesión.
    let h = p.pedir("GET", &format!("/api/clientes/{c}/instalador-agente/huella"), None, Some(&cookie)).await;
    assert_eq!(h.estado, StatusCode::OK, "{}", h.json);
    assert_eq!(h.json["sha256"].as_str().unwrap(), resguardo_servidor::instalador_agente::sha256_hex(INSTALADOR));
    assert_eq!(h.json["bytes"].as_u64(), Some(INSTALADOR.len() as u64));
    assert_eq!(p.pedir("GET", &format!("/api/clientes/{c}/instalador-agente/huella"), None, Some(&tecnico)).await.estado, StatusCode::FORBIDDEN);
    let exe = p.pedir("GET", &format!("/api/agente/instalador/{lote}"), None, None).await;
    assert_eq!((exe.estado, exe.bytes.as_slice()), (StatusCode::OK, INSTALADOR));
    for otro in [uuid::Uuid::new_v4().to_string(), "../ca".to_string(), "x".to_string()] {
        assert_eq!(p.pedir("GET", &format!("/api/agente/instalador/{otro}"), None, None).await.estado, StatusCode::NOT_FOUND, "{otro}");
    }

    // Uno se une; se anula; el siguiente ya no entra (y el que se unió sigue esperando).
    let unido = p.unirse(&codigo, "ALMACEN-01", Some(3)).await;
    assert_eq!(unido.estado, StatusCode::OK);
    let equipo_unido = unido.json["equipo_id"].as_str().unwrap().to_string();
    assert_eq!(p.pedir("DELETE", &format!("{ruta}/{lote}"), None, Some(&cookie)).await.estado, StatusCode::NO_CONTENT);
    let r = p.unirse(&codigo, "ALMACEN-02", Some(3)).await;
    assert_eq!((r.estado, r.json["error"].as_str()), (StatusCode::NOT_FOUND, Some("codigo")));
    let v = p.pedir("GET", &format!("{ruta}/{lote}"), None, Some(&cookie)).await.json;
    assert_eq!((v["estado"].as_str(), v["usados"].as_i64(), v["pendientes"].as_u64()), (Some("anulado"), Some(1), Some(1)));
    // El intento con el código anulado se reconoce y queda en el lote y en la auditoría.
    assert_eq!(v["rechazos"].as_i64(), Some(1));
    assert!(p
        .auditoria(&cookie, &c)
        .await
        .iter()
        .any(|e| e["accion"] == "codigo_varios_rechazado" && e["datos"].as_str().is_some_and(|d| d.contains("anulado"))));
    assert!(v["anulado"].is_string());
    assert_eq!(p.pedir("GET", &format!("/api/agente/instalador/{lote}"), None, None).await.estado, StatusCode::NOT_FOUND, "anulado: sin descarga");
    assert_eq!(p.pedir("DELETE", &format!("{ruta}/{uuid}", uuid = uuid::Uuid::new_v4()), None, Some(&cookie)).await.estado, StatusCode::NOT_FOUND);
    assert_eq!(p.auditoria(&cookie, &c).await.iter().filter(|e| e["accion"] == "anular_codigo_varios").count(), 1);

    // Caducado: un lote cuya fecha ya pasó no admite a nadie (aunque le queden usos).
    let (codigo2, hash2) = codigo_del_navegador(3);
    // (El contexto del cliente, por el equipo que se unió: las pruebas no lo crean a mano.)
    let ctx = p.st.db.cliente_de_equipo(&equipo_unido).unwrap().unwrap();
    let viejo = Lote {
        id: uuid::Uuid::new_v4().to_string(),
        codigo_hash: hash2.clone(),
        nombre: None,
        usos: 10,
        usados: 0,
        caduca: chrono::Utc::now().timestamp() - 1,
        creado: chrono::Utc::now().timestamp() - 86_400,
        creado_por: "ana".into(),
        anulado: None,
        rechazos: 0,
    };
    p.st.db.crear_lote(&ctx, &viejo).unwrap();
    // En el índice con una caducidad futura: es el lote el que manda.
    p.st.db.indexar_codigo_varios(&hash2, &c, &viejo.id, chrono::Utc::now().timestamp() + 3600).unwrap();
    let r = p.unirse(&codigo2, "ALMACEN-03", Some(3)).await;
    assert_eq!(r.estado, StatusCode::NOT_FOUND);
    let lista = p.pedir("GET", &ruta, None, Some(&cookie)).await.json;
    let est = |id: &str| lista.as_array().unwrap().iter().find(|l| l["id"] == id).map(|l| l["estado"].as_str().unwrap().to_string());
    assert_eq!((est(&lote).as_deref(), est(&viejo.id).as_deref()), (Some("anulado"), Some("caducado")));

    // Como mucho 5 activos a la vez por cliente.
    for n in 0..5u8 {
        let (_, h) = codigo_del_navegador(10 + n);
        let r = p.pedir("POST", &ruta, Some(json!({ "codigo_hash": h, "usos": 3, "dias": 1 })), Some(&cookie)).await;
        let esperado = if n < 5 { StatusCode::OK } else { StatusCode::CONFLICT };
        assert_eq!(r.estado, esperado, "{n}: {}", r.json);
    }
    let (_, h) = codigo_del_navegador(20);
    let r = p.pedir("POST", &ruta, Some(json!({ "codigo_hash": h, "usos": 3, "dias": 1 })), Some(&cookie)).await;
    assert_eq!((r.estado, r.json["error"].as_str()), (StatusCode::CONFLICT, Some("demasiados_codigos_varios")));
}

#[tokio::test]
async fn probar_codigos_bloquea_la_ip_tambien_con_un_codigo_bueno() {
    let p = servidor();
    let (cookie, c) = p.propietario_y_cliente().await;
    let (codigo, hash) = codigo_del_navegador(4);
    let r = p.pedir("POST", &format!("/api/clientes/{c}/codigos-varios"), Some(json!({ "codigo_hash": hash, "usos": 50, "dias": 7 })), Some(&cookie)).await;
    assert_eq!(r.estado, StatusCode::OK);
    // Más de 20 códigos que no valen desde la misma IP (`MAX_FALLOS_UNIRSE_H`)…
    for n in 0..=20u8 {
        let (malo, _) = codigo_del_navegador(100 + n);
        assert_eq!(p.unirse(&malo, "INTRUSO", Some(3)).await.estado, StatusCode::NOT_FOUND);
    }
    // …y esa IP espera aunque luego acierte: probar no sale a cuenta.
    let r = p.unirse(&codigo, "INTRUSO", Some(3)).await;
    assert_eq!(r.estado, StatusCode::TOO_MANY_REQUESTS, "{}", r.json);
    let v = p.pedir("GET", &format!("/api/clientes/{c}/codigos-varios"), None, Some(&cookie)).await.json;
    assert_eq!(v[0]["usados"].as_i64(), Some(0), "no gastó ningún uso");
}
