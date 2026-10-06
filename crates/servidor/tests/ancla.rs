//! v1.4x: ancla externa de la auditoría (docs/plan-mejoras.md, 9b; docs/plataforma.md
//! §7.3.1). El servidor manda la cabeza de la cadena de cada cliente, firmada, a sus
//! equipos; si alguien rehace la cadena entera (la comprobación de siempre dice que
//! está bien), la cabeza de antes ya no cuadra. Y un equipo que lo nota lo cuenta en
//! su historial (`auditoria_rehecha`): aviso crítico, una sola vez.
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use base64::Engine;
use http_body_util::BodyExt;
use resguardo_protocolo::{claves, derivaciones, mensajes};
use resguardo_servidor::estado::{Opciones, St};
use resguardo_servidor::{api, auth, preparar};
use serde_json::{json, Value};
use sha2::Digest;
use tower::ServiceExt;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

struct Prueba {
    dir: tempfile::TempDir,
    st: St,
    app: Router,
}

struct Resp {
    estado: StatusCode,
    json: Value,
    cookie: Option<String>,
}

fn servidor() -> Prueba {
    let dir = tempfile::tempdir().unwrap();
    let st = preparar(dir.path(), Opciones { https: false, ..Default::default() }).unwrap();
    Prueba { app: api::router(st.clone()), st, dir }
}

impl Prueba {
    async fn pedir(&self, metodo: &str, ruta: &str, cuerpo: Option<Value>, cookie: Option<&str>, extra: &[(&str, &str)]) -> Resp {
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
        let res = self.app.clone().oneshot(r.body(body).unwrap()).await.unwrap();
        let estado = res.status();
        let cookie = res.headers().get("set-cookie").and_then(|v| v.to_str().ok()).and_then(|v| v.split(';').next()).map(str::to_string);
        let bytes = res.into_body().collect().await.unwrap().to_bytes().to_vec();
        Resp { estado, json: serde_json::from_slice(&bytes).unwrap_or(Value::Null), cookie }
    }

    /// Propietario con la sesión completa, un cliente y un equipo confirmado: (cookie, cliente, auth del equipo).
    async fn cliente_con_equipo(&self) -> (String, String, String) {
        let codigo = api::preparar_codigo_arranque(&self.st).unwrap().unwrap();
        let cuerpo = json!({ "codigo_arranque": codigo, "correo": "ana@ejemplo.com", "nombre": "Ana", "contrasena": "una contraseña bien larga" });
        let r = self.pedir("POST", "/api/inicio", Some(cuerpo), None, &[]).await;
        let cookie = r.cookie.unwrap();
        let secreto = r.json["totp"]["secreto"].as_str().unwrap().to_string();
        let paso = chrono::Utc::now().timestamp() / 30;
        let totp = format!("{:06}", auth::hotp(&auth::de_base32(&secreto).unwrap(), paso as u64));
        let r = self.pedir("POST", "/api/sesion/totp", Some(json!({ "codigo": totp })), Some(&cookie), &[]).await;
        assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
        let cookie = r.cookie.unwrap();
        let r = self.pedir("POST", "/api/clientes", Some(json!({ "nombre": "Ferretería Rambla", "espera_min_horas": 24 })), Some(&cookie), &[]).await;
        let c = r.json["id"].as_str().unwrap().to_string();
        let r = self.pedir("POST", &format!("/api/clientes/{c}/emparejamientos"), None, Some(&cookie), &[]).await;
        let (emp, codigo) = (r.json["id"].as_str().unwrap().to_string(), r.json["codigo"].as_str().unwrap().to_string());
        let box_pub = claves::public_of(&claves::new_key()).unwrap();
        let sign_pub = B64.encode(ed25519_dalek::SigningKey::from_bytes(&[11u8; 32]).verifying_key().to_bytes());
        let r = self
            .pedir(
                "POST",
                "/api/agente/unirse",
                Some(json!({ "codigo_hash": mensajes::code_hash(&codigo), "nombre": "MOSTRADOR", "so": "windows", "version": "0.7.0", "box_pub": box_pub, "sign_pub": sign_pub, "sal_equipo": B64.encode([3u8; 16]) })),
                None,
                &[],
            )
            .await;
        assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
        let auth = format!("Equipo {}:{}", r.json["equipo_id"].as_str().unwrap(), r.json["secreto"].as_str().unwrap());
        let r = self
            .pedir("POST", &format!("/api/clientes/{c}/emparejamientos/{emp}/confirmar"), Some(json!({ "etiqueta": "ETIQUETA" })), Some(&cookie), &[])
            .await;
        assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
        (cookie, c, auth)
    }

    async fn subir(&self, auth: &str, e: Value) -> Resp {
        self.pedir("POST", "/api/agente/historial", Some(json!({ "entradas": [e] })), None, &[("authorization", auth)]).await
    }

    async fn avisos(&self, c: &str, cookie: &str) -> Value {
        self.pedir("GET", &format!("/api/clientes/{c}/avisos"), None, Some(cookie), &[]).await.json
    }

    async fn tomar(&self, auth: &str) -> Value {
        let r = self.pedir("POST", "/api/agente/tomar", Some(json!({ "reto": B64.encode([42u8; 32]) })), None, &[("authorization", auth)]).await;
        assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
        r.json
    }
}

/// Comprueba la firma del ancla con la identidad del servidor, como el agente.
fn firma_valida(p: &Prueba, a: &Value) -> bool {
    let texto = derivaciones::texto_ancla_auditoria(
        a["cliente"].as_str().unwrap(),
        a["n"].as_u64().unwrap(),
        a["creado"].as_i64().unwrap(),
        a["hash"].as_str().unwrap(),
    );
    let firma: [u8; 64] = B64.decode(a["firma"].as_str().unwrap()).unwrap().try_into().unwrap();
    let identidad: [u8; 32] = B64.decode(&p.st.identidad_pub).unwrap().try_into().unwrap();
    ed25519_dalek::VerifyingKey::from_bytes(&identidad).unwrap().verify_strict(texto.as_bytes(), &ed25519_dalek::Signature::from_bytes(&firma)).is_ok()
}

/// Lo que haría un servidor malicioso: rehacer la cadena entera (quitando una entrada
/// incómoda) con huellas nuevas que cuadran entre sí, saltándose los disparadores.
fn rehacer_la_cadena(p: &Prueba, cliente: &str, quitar: i64) {
    let db = rusqlite::Connection::open(p.dir.path().join("clientes").join(format!("{cliente}.db"))).unwrap();
    db.execute_batch("PRAGMA busy_timeout=5000; DROP TRIGGER auditoria_sin_cambios; DROP TRIGGER auditoria_sin_borrar;").unwrap();
    let mut filas: Vec<(i64, String, String, String, String)> = db
        .prepare("SELECT creado, actor, accion, objetivo, datos FROM auditoria ORDER BY n")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    filas.remove(quitar as usize - 1);
    db.execute("DELETE FROM auditoria", []).unwrap();
    let mut prev = "0".repeat(64);
    for (i, (creado, actor, accion, objetivo, datos)) in filas.iter().enumerate() {
        let n = i as i64 + 1;
        let hash: String =
            sha2::Sha256::digest(format!("{prev}|{n}|{creado}|{actor}|{accion}|{objetivo}|{datos}").as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
        db.execute(
            "INSERT INTO auditoria (n, creado, actor, accion, objetivo, datos, prev_hash, hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![n, creado, actor, accion, objetivo, datos, prev, hash],
        )
        .unwrap();
        prev = hash;
    }
}

#[tokio::test]
async fn el_ancla_delata_una_cadena_rehecha() {
    let p = servidor();
    let (cookie, c, auth) = p.cliente_con_equipo().await;
    // Unas cuantas cosas más en la actividad.
    for nombre in ["Ferretería Rambla Norte", "Ferretería Rambla"] {
        let r = p.pedir("PATCH", &format!("/api/clientes/{c}"), Some(json!({ "nombre": nombre })), Some(&cookie), &[]).await;
        assert!(r.estado.is_success(), "{}", r.json);
    }
    let cabeza = p.pedir("GET", &format!("/api/clientes/{c}/auditoria?orden=desc&limite=1"), None, Some(&cookie), &[]).await.json[0].clone();
    let n = cabeza["n"].as_u64().unwrap();
    assert!(n >= 4, "{cabeza}");

    // El ancla llega firmada con cada sondeo: la cabeza de la cadena de este cliente.
    let a = p.tomar(&auth).await["ancla"].clone();
    assert_eq!((a["cliente"].as_str(), a["n"].as_u64(), a["hash"].as_str()), (Some(c.as_str()), Some(n), cabeza["hash"].as_str()));
    assert!(firma_valida(&p, &a), "firmada con la identidad del servidor");
    let mut falsa = a.clone();
    falsa["n"] = json!(n + 1);
    assert!(!firma_valida(&p, &falsa), "la firma cubre el número");
    let linea = derivaciones::linea_ancla(&c, n, a["creado"].as_i64().unwrap(), a["hash"].as_str().unwrap());
    assert_eq!(linea, format!("resguardo-ancla:1:{c}:{n}:{}:{}", a["creado"], cabeza["hash"].as_str().unwrap()));

    // Alguien rehace la cadena entera sin la segunda entrada: la comprobación de siempre
    // no lo ve (la cadena nueva cuadra consigo misma)…
    rehacer_la_cadena(&p, &c, 2);
    let v = p.pedir("GET", &format!("/api/clientes/{c}/auditoria/verificar"), None, Some(&cookie), &[]).await;
    assert_eq!(v.json["ok"], true, "{}", v.json);
    // …pero el ancla de antes ya no cuadra: la entrada n.º n ya no existe (retrocede)…
    let b = p.tomar(&auth).await["ancla"].clone();
    assert!(firma_valida(&p, &b));
    assert_eq!(b["n"].as_u64(), Some(n - 1), "la cadena rehecha es más corta");
    // …y en cuanto vuelve a tener n entradas, la n.º n tiene otra huella.
    let r = p.pedir("PATCH", &format!("/api/clientes/{c}"), Some(json!({ "nombre": "Ferretería Rambla Sur" })), Some(&cookie), &[]).await;
    assert!(r.estado.is_success());
    let b = p.tomar(&auth).await["ancla"].clone();
    assert_eq!(b["n"].as_u64(), Some(n));
    assert_ne!(b["hash"], a["hash"], "misma entrada, otra huella: rehecha");
}

#[tokio::test]
async fn una_consola_rehecha_avisa_una_vez() {
    let p = servidor();
    let (cookie, c, auth) = p.cliente_con_equipo().await;
    let ahora = chrono::Utc::now();
    let entrada = |id: &str, hora: chrono::DateTime<chrono::Utc>, identidad: &str| {
        json!({
            "id": id, "tipo": "auditoria_rehecha", "hora": hora.to_rfc3339(),
            "consola": "Consola en línea", "identidad": identidad,
            "antes": { "n": 41, "creado": 1_790_000_000, "hash": "a".repeat(64) },
            "ahora": { "n": 39, "creado": 1_790_000_500, "hash": "b".repeat(64) },
            "motivo": "retrocede",
        })
    };

    let r = p.subir(&auth, entrada("rehecha-1", ahora, "b3RyYQ==")).await;
    assert_eq!((r.estado, r.json["nuevas"].as_u64()), (StatusCode::OK, Some(1)), "{}", r.json);
    let v = p.avisos(&c, &cookie).await;
    let de_rehecha: Vec<&Value> = v.as_array().unwrap().iter().filter(|a| a["tipo"] == "auditoria_rehecha").collect();
    assert_eq!(de_rehecha.len(), 1, "{v}");
    assert!(de_rehecha[0]["visto_por"].is_null(), "un aviso nuevo, sin ver");
    let m = de_rehecha[0]["mensaje"].as_str().unwrap();
    assert!(m.contains("La consola «Consola en línea»") && m.contains("n.º 41") && m.contains("la 39"), "{m}");
    // La misma entrada otra vez (el equipo repite el margen): ni otra entrada ni otro aviso.
    let r = p.subir(&auth, entrada("rehecha-1", ahora, "b3RyYQ==")).await;
    assert_eq!(r.json["nuevas"].as_u64(), Some(0));
    assert_eq!(p.avisos(&c, &cookie).await.as_array().unwrap().iter().filter(|a| a["tipo"] == "auditoria_rehecha").count(), 1);
    // De esta misma consola: «Este servidor».
    let r = p.subir(&auth, entrada("rehecha-2", ahora, &p.st.identidad_pub)).await;
    assert_eq!(r.json["nuevas"].as_u64(), Some(1));
    assert!(p
        .avisos(&c, &cookie)
        .await
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["tipo"] == "auditoria_rehecha" && a["mensaje"].as_str().unwrap().starts_with("Este servidor")));
    // Una de hace semanas (una consola nueva recibe la bitácora entera): a la lista, ya vista.
    let r = p.subir(&auth, entrada("rehecha-3", ahora - chrono::Duration::days(30), "b3RyYQ==")).await;
    assert_eq!(r.json["nuevas"].as_u64(), Some(1));
    let v = p.avisos(&c, &cookie).await;
    let vieja = v.as_array().unwrap().iter().filter(|a| a["tipo"] == "auditoria_rehecha").find(|a| !a["visto_por"].is_null());
    assert!(vieja.is_some(), "{v}");
    // Sin su forma (huella que no es tal): se descarta.
    let mut mala = entrada("rehecha-4", ahora, "b3RyYQ==");
    mala["antes"]["hash"] = json!("no-es-una-huella");
    assert_eq!(p.subir(&auth, mala).await.json["nuevas"].as_u64(), Some(0));
    // Y queda en el historial del equipo, para todas las consolas.
    let equipo = auth.trim_start_matches("Equipo ").split(':').next().unwrap();
    let h = p.pedir("GET", &format!("/api/clientes/{c}/equipos/{equipo}/historial"), None, Some(&cookie), &[]).await;
    assert_eq!(h.json.as_array().map(|x| x.iter().filter(|e| e["tipo"] == "auditoria_rehecha").count()), Some(3), "{}", h.json);
}
