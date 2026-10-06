//! v1.4x: el código de «Añadir equipo» lo genera el navegador y al servidor solo le llega su
//! hash (docs/plan-mejoras.md, 9a). Convive con la forma de antes (el servidor lo genera).
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use base64::Engine;
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
    /// Todo lo que el servidor contestó (para comprobar que nunca da el código).
    respuestas: std::sync::Mutex<Vec<u8>>,
}

struct Resp {
    estado: StatusCode,
    json: Value,
    cookie: Option<String>,
    bytes: Vec<u8>,
}

/// Un servidor con instalador del agente (o sin él) y la huella de su autoridad TLS.
fn servidor(con_instalador: bool) -> Prueba {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("tls")).unwrap();
    std::fs::write(dir.path().join("tls").join("ca.huella"), vec!["5A"; 32].join(":")).unwrap();
    let instalador_agente = con_instalador.then(|| {
        let exe = dir.path().join("Resguardo-Agente-setup.exe");
        std::fs::write(&exe, b"MZ instalador de prueba").unwrap();
        exe
    });
    let st = preparar(dir.path(), Opciones { https: false, instalador_agente, ..Default::default() }).unwrap();
    Prueba { _dir: dir, st: st.clone(), app: api::router(st), respuestas: Default::default() }
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

    /// Propietario del servidor con la sesión completa y un cliente: (cookie, cliente).
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

    /// Un técnico del cliente (no puede añadir equipos).
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

    /// `POST /api/agente/unirse` con ese código (como el agente: solo su hash).
    async fn unirse(&self, codigo: &str) -> Resp {
        let box_pub = claves::public_of(&claves::new_key()).unwrap();
        let cuerpo = json!({
            "codigo_hash": mensajes::code_hash(codigo), "nombre": "DESKTOP-4F2K", "so": "windows", "version": "0.7.21",
            "box_pub": box_pub, "sign_pub": B64.encode([11u8; 32]), "sal_equipo": B64.encode([3u8; 16]), "sas_version": 3,
        });
        self.pedir("POST", "/api/agente/unirse", Some(cuerpo), None).await
    }

    /// ¿Apareció el código en alguna respuesta del servidor (normalizado o tal cual)?
    fn lo_vio(&self, codigo: &str) -> bool {
        let todo = String::from_utf8_lossy(&self.respuestas.lock().unwrap()).to_uppercase();
        let norm: String = codigo.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_uppercase();
        todo.contains(&codigo.to_uppercase()) || todo.contains(&norm)
    }
}

/// Lo que haría el navegador: un código de 16 letras y cifras y su hash.
fn codigo_del_navegador(n: u8) -> (String, String) {
    let a = mensajes::ALFABETO_CODIGO;
    let s: String = (0..16).map(|i| a[(usize::from(n) * 7 + i * 13) % a.len()] as char).collect();
    let codigo = format!("{}-{}-{}-{}", &s[..4], &s[4..8], &s[8..12], &s[12..]);
    let hash = mensajes::code_hash(&codigo);
    (codigo, hash)
}

#[tokio::test]
async fn codigo_de_15_min_generado_en_el_navegador() {
    let p = servidor(false);
    let (cookie, c) = p.propietario_y_cliente().await;
    assert_eq!(p.pedir("GET", "/api/servidor", None, None).await.json["codigo_navegador"], true);
    let ruta = format!("/api/clientes/{c}/emparejamientos");
    let (codigo, hash) = codigo_del_navegador(1);

    // Solo llega el hash; la respuesta no trae código.
    let r = p.pedir("POST", &ruta, Some(json!({ "codigo_hash": hash.to_uppercase() })), Some(&cookie)).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!((r.json.get("codigo"), r.json["codigo_navegador"].as_bool(), r.json["reutilizado"].as_bool()), (None, Some(true), Some(false)));
    let emp = r.json["id"].as_str().unwrap().to_string();
    // El mismo hash otra vez (o el de un código que ya existe): 409, nada se pisa.
    let r = p.pedir("POST", &ruta, Some(json!({ "codigo_hash": hash })), Some(&cookie)).await;
    assert_eq!((r.estado, r.json["error"].as_str()), (StatusCode::CONFLICT, Some("codigo_repetido")));
    // Un hash que no lo es: 422.
    for malo in [json!({ "codigo_hash": "abc" }), json!({ "codigo_hash": "g".repeat(64) }), json!({ "codigo_hash": 7 })] {
        let r = p.pedir("POST", &ruta, Some(malo.clone()), Some(&cookie)).await;
        assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY, "{malo}: {}", r.json);
    }

    // Lo que ve la consola: el hash, nunca el código.
    let v = p.pedir("GET", &format!("{ruta}/{emp}"), None, Some(&cookie)).await.json;
    assert_eq!(
        (v["estado"].as_str(), v["codigo_hash"].as_str(), v["codigo_navegador"].as_bool(), v.get("codigo")),
        (Some("abierto"), Some(hash.as_str()), Some(true), None)
    );
    // `codigo-abierto`: una consola anterior (sin `?navegador=1`) no lo ve (no sabría enseñarlo).
    let abierto = format!("/api/clientes/{c}/codigo-abierto");
    assert_eq!(p.pedir("GET", &abierto, None, Some(&cookie)).await.json, Value::Null);
    let v = p.pedir("GET", &format!("{abierto}?navegador=1"), None, Some(&cookie)).await.json;
    assert_eq!((v["id"].as_str(), v["codigo"].clone(), v["codigo_hash"].as_str()), (Some(emp.as_str()), Value::Null, Some(hash.as_str())));
    // Una consola anterior que pide un código no recibe la marca del navegador: uno suyo.
    let viejo = p.pedir("POST", &ruta, None, Some(&cookie)).await;
    assert_eq!(viejo.json["reutilizado"], false);
    let codigo_viejo = viejo.json["codigo"].as_str().unwrap().to_string();
    assert!(!codigo_viejo.contains("sha256") && codigo_viejo.len() == 12, "{codigo_viejo}");
    assert_eq!(p.pedir("POST", &ruta, None, Some(&cookie)).await.json["codigo"].as_str(), Some(codigo_viejo.as_str()), "el de antes se sigue reutilizando");

    // El equipo se une con el código (manda su hash, como siempre): los agentes no cambian.
    let r = p.unirse(&codigo).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["sas_version"], 3);
    let eq = r.json["equipo_id"].as_str().unwrap().to_string();
    assert_eq!(p.unirse(&codigo).await.estado, StatusCode::NOT_FOUND, "un solo uso");
    let v = p.pedir("GET", &format!("{ruta}/{emp}"), None, Some(&cookie)).await.json;
    assert_eq!((v["estado"].as_str(), v["codigo_hash"].as_str(), v["equipo"]["id"].as_str()), (Some("unido"), Some(hash.as_str()), Some(eq.as_str())));
    assert!(v["sas"].as_str().is_some());
    // Confirmado (sin el alta del equipo): sigue a medias y se puede anular.
    let r = p.pedir("POST", &format!("{ruta}/{emp}/confirmar"), Some(json!({ "etiqueta": "ETIQUETA" })), Some(&cookie)).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let v = p.pedir("GET", &format!("{ruta}/{emp}"), None, Some(&cookie)).await.json;
    assert_eq!((v["estado"].as_str(), v["codigo_hash"].as_str()), (Some("confirmado"), Some(hash.as_str())));
    let medias = p.pedir("GET", &format!("/api/clientes/{c}/a-medias"), None, Some(&cookie)).await.json;
    assert_eq!((medias[0]["id"].as_str(), medias[0]["estado"].as_str()), (Some(emp.as_str()), Some("confirmado")));
    assert_eq!(p.pedir("DELETE", &format!("{ruta}/{emp}"), None, Some(&cookie)).await.estado, StatusCode::NO_CONTENT);
    assert_eq!(p.pedir("GET", &format!("/api/clientes/{c}/equipos/{eq}"), None, Some(&cookie)).await.estado, StatusCode::NOT_FOUND);

    assert!(!p.lo_vio(&codigo), "el servidor nunca dio el código del navegador");
}

#[tokio::test]
async fn preparados_con_el_codigo_del_navegador() {
    let p = servidor(true);
    let (cookie, c) = p.propietario_y_cliente().await;
    let ruta = format!("/api/clientes/{c}/instaladores");
    let (codigo_w, hash_w) = codigo_del_navegador(2);
    let (codigo_l, hash_l) = codigo_del_navegador(3);

    // Windows: JSON (no el instalador), con lo que la consola necesita para la cola.
    let cuerpo = json!({ "nombre": "PC-01", "so": "windows", "servidor": "https://192.168.1.20:8443/", "codigo_hash": hash_w });
    let r = p.pedir("POST", &ruta, Some(cuerpo), Some(&cookie)).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", String::from_utf8_lossy(&r.bytes));
    assert_eq!(
        (
            r.json["nombre"].as_str(),
            r.json["so"].as_str(),
            r.json["servidor"].as_str(),
            r.json["cliente"].as_str(),
            r.json["huella_ca"].as_str(),
            r.json.get("codigo")
        ),
        (Some("PC-01"), Some("windows"), Some("https://192.168.1.20:8443"), Some(c.as_str()), Some(p.st.huella_ca.as_str()), None)
    );
    let emp_w = r.json["id"].as_str().unwrap().to_string();
    // El instalador genérico, sin cola (la añade el navegador); solo administradores.
    let exe = p.pedir("GET", &format!("/api/clientes/{c}/instalador-agente"), None, Some(&cookie)).await;
    assert_eq!((exe.estado, exe.bytes.as_slice()), (StatusCode::OK, b"MZ instalador de prueba".as_slice()));
    let tecnico = p.tecnico(&cookie, &c).await;
    assert_eq!(p.pedir("GET", &format!("/api/clientes/{c}/instalador-agente"), None, Some(&tecnico)).await.estado, StatusCode::FORBIDDEN);
    let cuerpo = json!({ "nombre": "X", "so": "linux", "servidor": "https://srv:8443", "codigo_hash": hash_l });
    assert_eq!(p.pedir("POST", &ruta, Some(cuerpo), Some(&tecnico)).await.estado, StatusCode::FORBIDDEN);
    // La consola arma la cola con su código; el agente la lee y se une.
    let mut listo = exe.bytes.clone();
    let datos = resguardo_protocolo::instalador::DatosInstalador {
        v: 1,
        servidor: r.json["servidor"].as_str().unwrap().into(),
        huella_ca: r.json["huella_ca"].as_str().unwrap().into(),
        cliente: c.clone(),
        nombre: "PC-01".into(),
        codigo: codigo_w.clone(),
    };
    listo.extend(resguardo_protocolo::instalador::cola(&datos).unwrap());
    let d = resguardo_protocolo::instalador::leer_cola(&listo).unwrap().unwrap();
    let r = p.unirse(&d.codigo).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let eq = r.json["equipo_id"].as_str().unwrap();
    assert_eq!(p.pedir("GET", &format!("/api/clientes/{c}/equipos/{eq}"), None, Some(&cookie)).await.json["nombre"], "PC-01");
    let v = p.pedir("GET", &format!("/api/clientes/{c}/emparejamientos/{emp_w}"), None, Some(&cookie)).await.json;
    assert_eq!(
        (v["estado"].as_str(), v["nombre"].as_str(), v["codigo_hash"].as_str(), v.get("codigo")),
        (Some("unido"), Some("PC-01"), Some(hash_w.as_str()), None)
    );

    // Linux: igual, la línea la arma la consola.
    let cuerpo = json!({ "nombre": "srv-datos", "so": "linux", "servidor": "https://192.168.1.20:8443", "codigo_hash": hash_l });
    let r = p.pedir("POST", &ruta, Some(cuerpo.clone()), Some(&cookie)).await;
    assert_eq!((r.estado, r.json.get("codigo"), r.json["codigo_navegador"].as_bool()), (StatusCode::OK, None, Some(true)), "{}", r.json);
    let emp_l = r.json["id"].as_str().unwrap().to_string();
    // El mismo nombre otra vez con el mismo hash: 409 (no se reutiliza en el servidor; la consola
    // reutiliza el suyo sin pedir).
    assert_eq!(p.pedir("POST", &ruta, Some(cuerpo), Some(&cookie)).await.estado, StatusCode::CONFLICT);
    // Una consola anterior que prepara el mismo equipo no recibe la marca: uno suyo, nuevo.
    let r = p.pedir("POST", &ruta, Some(json!({ "nombre": "srv-datos", "so": "linux", "servidor": "https://192.168.1.20:8443" })), Some(&cookie)).await;
    assert_eq!(r.json["reutilizado"], false);
    assert!(r.json["codigo"].as_str().is_some_and(|x| !x.contains("sha256")), "{}", r.json);
    // Datos que no valen: nada.
    let malo = json!({ "nombre": "X", "so": "linux", "servidor": "http://srv:8443", "codigo_hash": codigo_del_navegador(4).1 });
    assert_eq!(p.pedir("POST", &ruta, Some(malo), Some(&cookie)).await.estado, StatusCode::UNPROCESSABLE_ENTITY);
    let malo = json!({ "nombre": "X", "so": "linux", "servidor": "https://srv:8443", "codigo_hash": "123" });
    assert_eq!(p.pedir("POST", &ruta, Some(malo), Some(&cookie)).await.estado, StatusCode::UNPROCESSABLE_ENTITY);
    // Anulado, ya no sirve.
    assert_eq!(p.pedir("DELETE", &format!("/api/clientes/{c}/emparejamientos/{emp_l}"), None, Some(&cookie)).await.estado, StatusCode::NO_CONTENT);
    assert_eq!(p.unirse(&codigo_l).await.estado, StatusCode::NOT_FOUND);

    assert!(!p.lo_vio(&codigo_w) && !p.lo_vio(&codigo_l), "el servidor nunca dio los códigos del navegador");
}

/// Sin instalador en el servidor: 404 antes de gastar nada, también con el código del navegador.
#[tokio::test]
async fn sin_instalador_no_se_gasta_el_codigo_del_navegador() {
    let p = servidor(false);
    let (cookie, c) = p.propietario_y_cliente().await;
    let cuerpo = json!({ "nombre": "PC", "so": "windows", "servidor": "https://srv:8443", "codigo_hash": codigo_del_navegador(5).1 });
    let r = p.pedir("POST", &format!("/api/clientes/{c}/instaladores"), Some(cuerpo), Some(&cookie)).await;
    assert_eq!((r.estado, r.json["error"].as_str()), (StatusCode::NOT_FOUND, Some("sin_instalador")));
    let r = p.pedir("GET", &format!("/api/clientes/{c}/instalador-agente"), None, Some(&cookie)).await;
    assert_eq!((r.estado, r.json["error"].as_str()), (StatusCode::NOT_FOUND, Some("sin_instalador")));
    assert_eq!(p.pedir("GET", &format!("/api/clientes/{c}/emparejamientos"), None, Some(&cookie)).await.json, json!([]));
    assert_eq!(p.unirse(&codigo_del_navegador(5).0).await.estado, StatusCode::NOT_FOUND);
}

/// `unirse`: el límite por IP cuenta los fallos (probar códigos), no los equipos que se unen bien.
/// Antes, 20 por hora en total: una oficina tras una IP no vinculaba el 21.º («Demasiados intentos»).
#[tokio::test]
async fn unirse_limita_los_fallos_no_los_aciertos() {
    let p = servidor(false);
    let (cookie, c) = p.propietario_y_cliente().await;
    let ruta = format!("/api/clientes/{c}/emparejamientos");
    for n in 0..25u8 {
        let (codigo, hash) = codigo_del_navegador(10 + n);
        let r = p.pedir("POST", &ruta, Some(json!({ "codigo_hash": hash })), Some(&cookie)).await;
        assert_eq!(r.estado, StatusCode::OK, "{n}: {}", r.json);
        let r = p.unirse(&codigo).await;
        assert_eq!(r.estado, StatusCode::OK, "equipo {n}: {}", r.json);
    }
    // Probar códigos sí se frena: tras 20 fallos, ni uno bueno pasa (desde esa IP, esa hora).
    let (bueno, hash) = codigo_del_navegador(99);
    assert_eq!(p.pedir("POST", &ruta, Some(json!({ "codigo_hash": hash })), Some(&cookie)).await.estado, StatusCode::OK);
    let mut fallos = 0;
    for n in 0..30u8 {
        let r = p.unirse(&format!("MALO-{n:04}-XXXX")).await;
        if r.estado == StatusCode::TOO_MANY_REQUESTS {
            break;
        }
        assert_eq!(r.estado, StatusCode::NOT_FOUND);
        fallos += 1;
    }
    assert!((20..=21).contains(&fallos), "{fallos}");
    let r = p.unirse(&bueno).await;
    assert_eq!((r.estado, r.json["error"].as_str()), (StatusCode::TOO_MANY_REQUESTS, Some("demasiados_intentos")));
}
