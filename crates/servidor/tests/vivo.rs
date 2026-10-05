//! Canal en vivo de la consola (v1.39, `GET /api/clientes/{c}/vivo`): quién lo
//! abre (sesión completa, miembro del cliente, `Origin` de este servidor), que
//! cada cliente oye solo lo suyo, los topes de conexiones y lo que llega cuando
//! un equipo empieza y termina una copia, manda su informe o recibe una orden.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use base64::Engine;
use ed25519_dalek::SigningKey;
use futures_util::StreamExt;
use http_body_util::BodyExt;
use resguardo_protocolo::{claves, mensajes};
use resguardo_servidor::estado::{Opciones, St};
use resguardo_servidor::{api, auth, preparar, vivo};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::time::Duration;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tower::ServiceExt;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

struct Prueba {
    _dir: tempfile::TempDir,
    st: St,
    app: Router,
    addr: SocketAddr,
}

/// Servidor de verdad en un puerto libre (sin TLS: solo para la prueba).
async fn servidor() -> Prueba {
    let dir = tempfile::tempdir().unwrap();
    let st = preparar(dir.path(), Opciones { https: false, ..Default::default() }).unwrap();
    let app = api::router(st.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app2 = app.clone();
    tokio::spawn(async move { axum::serve(listener, app2).await.unwrap() });
    Prueba { _dir: dir, st, app, addr }
}

struct Resp {
    estado: StatusCode,
    json: Value,
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
    Resp { estado, json: serde_json::from_slice(&bytes).unwrap_or(Value::Null), cookie }
}

fn totp(secreto: &str) -> String {
    let paso = chrono::Utc::now().timestamp() / 30;
    format!("{:06}", auth::hotp(&auth::de_base32(secreto).unwrap(), paso as u64))
}

async fn propietario(p: &Prueba) -> String {
    let codigo = api::preparar_codigo_arranque(&p.st).unwrap().unwrap();
    let cuerpo = json!({ "codigo_arranque": codigo, "correo": "ana@ejemplo.com", "nombre": "Ana", "contrasena": "una contraseña bien larga" });
    let r = pedir(&p.app, "POST", "/api/inicio", Some(cuerpo), None, None).await;
    let (cookie, secreto) = (r.cookie.unwrap(), r.json["totp"]["secreto"].as_str().unwrap().to_string());
    let r = pedir(&p.app, "POST", "/api/sesion/totp", Some(json!({ "codigo": totp(&secreto) })), Some(&cookie), None).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    r.cookie.unwrap()
}

/// Invita con ese rol y devuelve (sesión completa, id de la cuenta).
async fn invitado(p: &Prueba, cookie: &str, c: &str, rol: &str, correo: &str) -> (String, String) {
    let r = pedir(&p.app, "POST", &format!("/api/clientes/{c}/invitaciones"), Some(json!({ "rol": rol })), Some(cookie), None).await;
    let token = r.json["enlace"].as_str().unwrap().split('#').nth(1).unwrap().to_string();
    let cuerpo = json!({ "token": token, "correo": correo, "nombre": correo, "contrasena": "contraseña de la invitada" });
    let r = pedir(&p.app, "POST", "/api/invitaciones/aceptar", Some(cuerpo), None, None).await;
    let (cookie2, secreto) = (r.cookie.unwrap(), r.json["totp"]["secreto"].as_str().unwrap().to_string());
    let r = pedir(&p.app, "POST", "/api/sesion/totp", Some(json!({ "codigo": totp(&secreto) })), Some(&cookie2), None).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let cookie2 = r.cookie.unwrap();
    let id = pedir(&p.app, "GET", "/api/cuenta", None, Some(&cookie2), None).await.json["id"].as_str().unwrap().to_string();
    (cookie2, id)
}

async fn cliente(p: &Prueba, cookie: &str, nombre: &str) -> String {
    let r = pedir(&p.app, "POST", "/api/clientes", Some(json!({ "nombre": nombre })), Some(cookie), None).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    r.json["id"].as_str().unwrap().to_string()
}

/// Un equipo emparejado y confirmado: (id, cabecera de autorización).
async fn equipo(p: &Prueba, cookie: &str, c: &str) -> (String, String) {
    let r = pedir(&p.app, "POST", &format!("/api/clientes/{c}/emparejamientos"), None, Some(cookie), None).await;
    let (emp, codigo) = (r.json["id"].as_str().unwrap().to_string(), r.json["codigo"].as_str().unwrap().to_string());
    let box_pub = claves::public_of(&claves::new_key()).unwrap();
    let sign_pub = B64.encode(SigningKey::from_bytes(&[7u8; 32]).verifying_key().to_bytes());
    let cuerpo = json!({ "codigo_hash": mensajes::code_hash(&codigo), "nombre": "RECEPCION", "so": "windows", "version": "0.7.16", "box_pub": box_pub, "sign_pub": sign_pub, "sal_equipo": B64.encode([3u8; 16]) });
    let r = pedir(&p.app, "POST", "/api/agente/unirse", Some(cuerpo), None, None).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let (id, secreto) = (r.json["equipo_id"].as_str().unwrap().to_string(), r.json["secreto"].as_str().unwrap().to_string());
    let r =
        pedir(&p.app, "POST", &format!("/api/clientes/{c}/emparejamientos/{emp}/confirmar"), Some(json!({ "etiqueta": "ETIQUETA" })), Some(cookie), None).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    (id.clone(), format!("Equipo {id}:{secreto}"))
}

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Abre el canal; `Err(estado HTTP)` si el servidor no lo deja abrir.
async fn abrir(p: &Prueba, c: &str, cookie: Option<&str>, origen: Option<&str>) -> Result<Ws, u16> {
    let mut req = format!("ws://{}/api/clientes/{c}/vivo", p.addr).into_client_request().unwrap();
    if let Some(k) = cookie {
        req.headers_mut().insert("cookie", k.parse().unwrap());
    }
    if let Some(o) = origen {
        req.headers_mut().insert("origin", o.parse().unwrap());
    }
    match tokio_tungstenite::connect_async(req).await {
        Ok((ws, _)) => Ok(ws),
        Err(tokio_tungstenite::tungstenite::Error::Http(r)) => Err(r.status().as_u16()),
        Err(e) => panic!("{e}"),
    }
}

fn origen(p: &Prueba) -> String {
    format!("http://{}", p.addr)
}

/// El siguiente mensaje de texto (sin latidos), con plazo.
async fn siguiente(ws: &mut Ws) -> Value {
    loop {
        let m = tokio::time::timeout(Duration::from_secs(5), ws.next()).await.expect("llega a tiempo").expect("abierto").expect("sin error");
        if let Message::Text(t) = m {
            let v: Value = serde_json::from_str(&t).unwrap();
            if v["t"] != "latido" {
                return v;
            }
        }
    }
}

/// Espera un mensaje que cumpla `f` (descartando los demás).
async fn esperar(ws: &mut Ws, f: impl Fn(&Value) -> bool) -> Value {
    for _ in 0..20 {
        let v = siguiente(ws).await;
        if f(&v) {
            return v;
        }
    }
    panic!("no llegó el mensaje esperado");
}

/// Nada más en un rato (lo de otros clientes no llega).
async fn nada(ws: &mut Ws) {
    match tokio::time::timeout(Duration::from_millis(400), ws.next()).await {
        Err(_) => {}
        Ok(Some(Ok(Message::Text(t)))) => panic!("no tenía que llegar nada: {t}"),
        Ok(otro) => panic!("no tenía que llegar nada: {otro:?}"),
    }
}

#[tokio::test]
async fn quien_puede_abrirlo() {
    let p = servidor().await;
    let ana = propietario(&p).await;
    let a = cliente(&p, &ana, "Cliente A").await;
    let o = origen(&p);
    // El servidor dice que lo tiene.
    assert_eq!(pedir(&p.app, "GET", "/api/servidor", None, None, None).await.json["vivo"], true);
    // Sin sesión, no.
    assert_eq!(abrir(&p, &a, None, Some(&o)).await.err(), Some(401));
    // Desde otra web (o sin Origin), no: un WebSocket no pasa por CORS.
    assert_eq!(abrir(&p, &a, Some(&ana), Some("https://otra.example")).await.err(), Some(403));
    assert_eq!(abrir(&p, &a, Some(&ana), None).await.err(), Some(403));
    // Un cliente del que no es miembro, «no existe».
    let r = pedir(&p.app, "POST", "/api/servidor/clientes", Some(json!({ "nombre": "Cliente B" })), Some(&ana), None).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let b = r.json["id"].as_str().unwrap().to_string();
    assert_eq!(abrir(&p, &b, Some(&ana), Some(&o)).await.err(), Some(404));
    let (bea, _) = invitado(&p, &ana, &a, "lectura", "bea@ejemplo.com").await;
    // Cualquier papel lo abre (también solo lectura), y lo primero es el saludo.
    let mut ws = abrir(&p, &a, Some(&bea), Some(&o)).await.expect("abre");
    let hola = siguiente(&mut ws).await;
    assert_eq!((hola["t"].as_str(), hola["v"].as_u64()), (Some("hola"), Some(1)));
    assert_eq!(p.st.vivo.abiertas(), 1);
    drop(ws);
    for _ in 0..50 {
        if p.st.vivo.abiertas() == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(p.st.vivo.abiertas(), 0, "al cerrarse, suelta su hueco");
}

#[tokio::test]
async fn cada_cliente_oye_lo_suyo() {
    let p = servidor().await;
    let ana = propietario(&p).await;
    let (a, b) = (cliente(&p, &ana, "Cliente A").await, cliente(&p, &ana, "Cliente B").await);
    let ((ea, auth_a), (eb, auth_b)) = (equipo(&p, &ana, &a).await, equipo(&p, &ana, &b).await);
    let o = origen(&p);
    let mut wa = abrir(&p, &a, Some(&ana), Some(&o)).await.unwrap();
    let mut wb = abrir(&p, &b, Some(&ana), Some(&o)).await.unwrap();
    assert_eq!(siguiente(&mut wa).await["t"], "hola");
    assert_eq!(siguiente(&mut wb).await["t"], "hola");

    // B empieza una copia: lo oye B, no A.
    let tarea = json!({ "tipo": "copia", "fase": "subiendo", "repo": "r1", "copia": "k1", "porcentaje": 0.1 });
    let r = pedir(&p.app, "POST", "/api/agente/progreso", Some(json!({ "tareas": [tarea] })), None, Some(&auth_b)).await;
    assert_eq!(r.estado, StatusCode::NO_CONTENT);
    assert_eq!(siguiente(&mut wb).await, json!({ "t": "progreso", "equipo": eb, "estado": "empieza" }));
    nada(&mut wa).await;

    // A: empieza, cambia, y al terminar llega el informe (las cifras ya guardadas) y el fin.
    let auth = Some(auth_a.as_str());
    pedir(&p.app, "POST", "/api/agente/progreso", Some(json!({ "tareas": [tarea] })), None, auth).await;
    assert_eq!(siguiente(&mut wa).await["estado"], "empieza");
    let mut t2 = tarea.clone();
    t2["porcentaje"] = json!(0.5);
    pedir(&p.app, "POST", "/api/agente/progreso", Some(json!({ "tareas": [t2] })), None, auth).await;
    assert_eq!(siguiente(&mut wa).await["estado"], "cambia");
    let r =
        pedir(&p.app, "POST", "/api/agente/informe", Some(json!({ "datos": { "version": "0.7.16", "servicio": "en_marcha", "progreso": [] } })), None, auth)
            .await;
    assert_eq!(r.estado, StatusCode::NO_CONTENT);
    assert_eq!(siguiente(&mut wa).await, json!({ "t": "informe", "equipo": ea }));
    assert_eq!(siguiente(&mut wa).await, json!({ "t": "progreso", "equipo": ea, "estado": "termina" }));
    // Lo que ya había terminado no termina otra vez.
    pedir(&p.app, "POST", "/api/agente/progreso", Some(json!({ "tareas": [] })), None, auth).await;
    nada(&mut wa).await;

    // Una orden de la consola: su estado (y lo de B, nada).
    let cuerpo = json!({ "tipo": "copiar_ahora", "seq": 1, "sellado": B64.encode([9u8; 64]), "caduca": (chrono::Local::now() + chrono::Duration::hours(1)).to_rfc3339() });
    let r = pedir(&p.app, "POST", &format!("/api/clientes/{a}/equipos/{ea}/ordenes"), Some(cuerpo), Some(&ana), None).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let orden = r.json["id"].as_str().unwrap().to_string();
    let m = siguiente(&mut wa).await;
    assert_eq!((m["t"].as_str(), m["orden"].as_str(), m["estado"].as_str()), (Some("orden"), Some(orden.as_str()), Some("pendiente")));
    // El equipo la toma por sondeo: entregada.
    let r = pedir(&p.app, "POST", "/api/agente/tomar", Some(json!({ "reto": B64.encode([1u8; 32]) })), None, auth).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(esperar(&mut wa, |m| m["t"] == "orden").await["estado"], "entregada");

    // Un aviso y su historial.
    let r = pedir(&p.app, "POST", "/api/agente/aviso", Some(json!({ "tipo": "copia_fallida", "mensaje": "Falló" })), None, auth).await;
    assert_eq!(r.estado, StatusCode::NO_CONTENT);
    assert_eq!(esperar(&mut wa, |m| m["t"] == "avisos").await["equipo"], ea.as_str());
    let entrada = json!({ "id": "h1", "hora": chrono::Local::now().to_rfc3339(), "tipo": "copia", "resultado": "ok" });
    let r = pedir(&p.app, "POST", "/api/agente/historial", Some(json!({ "entradas": [entrada] })), None, auth).await;
    assert_eq!(r.json["nuevas"], 1, "{}", r.json);
    assert_eq!(esperar(&mut wa, |m| m["t"] == "historial").await["equipo"], ea.as_str());
    // La misma entrada otra vez no es nada nuevo.
    pedir(&p.app, "POST", "/api/agente/historial", Some(json!({ "entradas": [entrada] })), None, auth).await;

    // Renombrar el equipo, desde la consola: `equipo`.
    let r = pedir(&p.app, "PATCH", &format!("/api/clientes/{a}/equipos/{ea}"), Some(json!({ "nombre": "CAJA" })), Some(&ana), None).await;
    assert_eq!(r.estado, StatusCode::OK);
    assert_eq!(esperar(&mut wa, |m| m["t"] == "equipo").await["equipo"], ea.as_str());
    // B no oyó nada de esto (los avisos de la tarea de fondo de las notificaciones son de A).
    loop {
        match tokio::time::timeout(Duration::from_millis(400), wb.next()).await {
            Err(_) => break,
            Ok(Some(Ok(Message::Text(t)))) => {
                let v: Value = serde_json::from_str(&t).unwrap();
                assert!(v["t"] == "latido" || (v["equipo"] != ea.as_str() && v["t"] != "orden" && v["t"] != "historial"), "B oyó algo de A: {t}");
            }
            Ok(_) => {}
        }
    }
}

#[tokio::test]
async fn topes_de_conexiones() {
    let p = servidor().await;
    let ana = propietario(&p).await;
    let a = cliente(&p, &ana, "Cliente A").await;
    let o = origen(&p);
    let mut abiertos = Vec::new();
    for _ in 0..vivo::MAX_POR_CUENTA {
        abiertos.push(abrir(&p, &a, Some(&ana), Some(&o)).await.expect("cabe"));
    }
    assert_eq!(abrir(&p, &a, Some(&ana), Some(&o)).await.err(), Some(429), "una cuenta no pasa de su tope");
    // Otra persona del mismo cliente sí.
    let (bea, _) = invitado(&p, &ana, &a, "tecnico", "bea@ejemplo.com").await;
    assert!(abrir(&p, &a, Some(&bea), Some(&o)).await.is_ok());
    // Al cerrar una, cabe otra.
    abiertos.pop();
    let mut ok = false;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if let Ok(ws) = abrir(&p, &a, Some(&ana), Some(&o)).await {
            abiertos.push(ws);
            ok = true;
            break;
        }
    }
    assert!(ok, "al cerrarse una conexión, su hueco queda libre");
}

#[tokio::test]
async fn se_cierra_si_ya_no_vale() {
    let p = servidor().await;
    let ana = propietario(&p).await;
    let a = cliente(&p, &ana, "Cliente A").await;
    let (bea, bea_id) = invitado(&p, &ana, &a, "lectura", "bea@ejemplo.com").await;
    let ana_id = pedir(&p.app, "GET", "/api/cuenta", None, Some(&ana), None).await.json["id"].as_str().unwrap().to_string();
    let token = |cookie: &str| auth::hash_ficha(cookie.split_once('=').unwrap().1);
    // Lo que mira cada minuto: sesión y rol.
    assert_eq!(vivo::revisar(&p.st, &token(&bea), &bea_id, &a).await, None);
    assert_eq!(vivo::revisar(&p.st, &token(&bea), &ana_id, &a).await, Some(4401), "la sesión es de otra cuenta");
    // Quitada del cliente: 4403.
    let r = pedir(&p.app, "DELETE", &format!("/api/clientes/{a}/miembros/{bea_id}"), None, Some(&ana), None).await;
    assert!(r.estado.is_success(), "{}", r.json);
    assert_eq!(vivo::revisar(&p.st, &token(&bea), &bea_id, &a).await, Some(4403));
    // Al salir, la sesión ya no vale: 4401.
    let r = pedir(&p.app, "DELETE", "/api/sesion", None, Some(&ana), None).await;
    assert!(r.estado.is_success());
    assert_eq!(vivo::revisar(&p.st, &token(&ana), &ana_id, &a).await, Some(4401));
}
