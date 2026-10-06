//! Prueba de integración de F6 con **dos Resguardo Server reales** (TLS) en
//! el mismo proceso: cambio de servidor que no llega a hacerse (se queda en
//! el antiguo), cambio de verdad y paso automático a un servidor de respaldo.

use crate::servidor_v2::{self as s, cargar, guardar, ronda, vincular};
use base64::Engine;
use resguardo_protocolo::derivaciones;
use resguardo_protocolo::orden_v2::{self, Autorizacion, OrdenV2};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
pub(crate) const CLAVE: &str = "caballo bateria grapa correcta";

pub(crate) struct Srv {
    pub dir: std::path::PathBuf,
    pub url: String,
    pub ca: String,
    pub identidad: String,
    /// Mientras viva, el servidor sigue en marcha; al soltarlo (`matar`), se para.
    vida: std::sync::mpsc::Sender<()>,
}

impl Srv {
    /// Para el servidor de verdad (el puerto deja de responder), como si se apagara la máquina.
    pub fn matar(self) -> std::path::PathBuf {
        let direccion = self.url.trim_start_matches("https://").to_string();
        drop(self.vida);
        for _ in 0..100 {
            if std::net::TcpStream::connect(&direccion).is_err() {
                return self.dir;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("el servidor {direccion} no se paró");
    }
}

fn arrancar(nombre: &str) -> (Srv, String) {
    arrancar_en(std::env::temp_dir().join(format!("resguardo-f6-{nombre}-{}", std::process::id())))
}

/// Un Resguardo Server real (TLS propio) con sus datos en `dir`, y su código de arranque.
pub(crate) fn arrancar_en(dir: std::path::PathBuf) -> (Srv, String) {
    arrancar_con(dir, resguardo_servidor::estado::Opciones::default())
}

/// Lo mismo con otras opciones (p. ej. la llave de publicación de pruebas).
pub(crate) fn arrancar_con(dir: std::path::PathBuf, opciones: resguardo_servidor::estado::Opciones) -> (Srv, String) {
    let _ = std::fs::remove_dir_all(&dir);
    resguardo_servidor::identidad::preparar_tls(&dir, &[]).unwrap();
    let st = resguardo_servidor::preparar(&dir, opciones).unwrap();
    let codigo = resguardo_servidor::api::preparar_codigo_arranque(&st).unwrap().unwrap();
    let puerto = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    (servir_en(st, dir, puerto), codigo)
}

/// Arranca otra vez un servidor con los datos que haya en `dir` (p. ej. una copia
/// restaurada) en la misma dirección que tenía (`url`): los equipos vuelven solos.
pub(crate) fn arrancar_sobre(dir: std::path::PathBuf, url: &str) -> Srv {
    let st = resguardo_servidor::preparar(&dir, resguardo_servidor::estado::Opciones::default()).unwrap();
    let puerto = url.rsplit(':').next().unwrap().parse().unwrap();
    servir_en(st, dir, puerto)
}

fn servir_en(st: resguardo_servidor::estado::St, dir: std::path::PathBuf, puerto: u16) -> Srv {
    let identidad = st.identidad_pub.clone();
    let addr: std::net::SocketAddr = format!("127.0.0.1:{puerto}").parse().unwrap();
    let tls = dir.join("tls");
    let (vida, fin) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let servir =
                resguardo_servidor::servir(st, addr, resguardo_servidor::Tls::Propio { cert: tls.join("servidor.crt"), clave: tls.join("servidor.key") });
            tokio::select! {
                _ = servir => {}
                _ = tokio::task::spawn_blocking(move || fin.recv()) => {}
            }
        });
        // Fuera todo: escucha, conexiones abiertas y tareas del servidor.
        rt.shutdown_background();
    });
    std::thread::sleep(Duration::from_millis(400));
    let ca = std::fs::read_to_string(dir.join("tls").join("ca.crt")).unwrap();
    Srv { dir, url: format!("https://127.0.0.1:{puerto}"), ca, identidad, vida }
}

pub(crate) fn agente_tls(ca: &str) -> ureq::Agent {
    use ureq::tls::{Certificate, RootCerts, TlsConfig, TlsProvider};
    let cert = Certificate::from_pem(ca.as_bytes()).unwrap().to_owned();
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(60)))
        .http_status_as_error(false)
        .tls_config(TlsConfig::builder().provider(TlsProvider::Rustls).root_certs(RootCerts::Specific(Arc::new(vec![cert]))).build())
        .build()
        .new_agent()
}

pub(crate) struct Consola {
    url: String,
    agente: ureq::Agent,
    cookie: String,
}

impl Consola {
    pub fn entrar(srv: &Srv, codigo: &str) -> Self {
        let agente = agente_tls(&srv.ca);
        let mut r = agente
            .post(&format!("{}/api/inicio", srv.url))
            .header("x-resguardo", "1")
            .send_json(json!({ "codigo_arranque": codigo, "correo": "ana@ejemplo.com", "nombre": "Ana", "contrasena": "una contraseña bien larga" }))
            .unwrap();
        let cookie = r.headers().get("set-cookie").unwrap().to_str().unwrap().split(';').next().unwrap().to_string();
        let v: Value = r.body_mut().read_json().unwrap();
        let paso = chrono::Utc::now().timestamp() / 30;
        let totp = format!(
            "{:06}",
            resguardo_servidor::auth::hotp(&resguardo_servidor::auth::de_base32(v["totp"]["secreto"].as_str().unwrap()).unwrap(), paso as u64)
        );
        let r = agente
            .post(&format!("{}/api/sesion/totp", srv.url))
            .header("cookie", &cookie)
            .header("x-resguardo", "1")
            .send_json(json!({ "codigo": totp }))
            .unwrap();
        assert_eq!(r.status().as_u16(), 200);
        // La sesión completa lleva otra ficha.
        let cookie = r.headers().get("set-cookie").unwrap().to_str().unwrap().split(';').next().unwrap().to_string();
        Consola { url: srv.url.clone(), agente, cookie }
    }
    pub fn pedir(&self, metodo: &str, ruta: &str, cuerpo: Option<Value>) -> (u16, Value) {
        let url = format!("{}{ruta}", self.url);
        let r = match (metodo, cuerpo) {
            ("GET", _) => self.agente.get(&url).header("cookie", &self.cookie).call(),
            ("PATCH", Some(c)) => self.agente.patch(&url).header("cookie", &self.cookie).header("x-resguardo", "1").send_json(&c),
            ("PUT", Some(c)) => self.agente.put(&url).header("cookie", &self.cookie).header("x-resguardo", "1").send_json(&c),
            (_, Some(c)) => self.agente.post(&url).header("cookie", &self.cookie).header("x-resguardo", "1").send_json(&c),
            (_, None) => self.agente.post(&url).header("cookie", &self.cookie).header("x-resguardo", "1").send_empty(),
        };
        let mut r = r.unwrap();
        (r.status().as_u16(), r.body_mut().read_json().unwrap_or(Value::Null))
    }
}

/// El equipo, visto por la consola de un servidor.
pub(crate) struct Equipo {
    pub c: String,
    pub id: String,
    pub box_pub: String,
    pub prueba: [u8; 32],
}

pub(crate) fn orden(e: &Equipo, seq: u64, tipo: &str, cuerpo: Value) -> OrdenV2 {
    let ahora = chrono::Local::now();
    OrdenV2 {
        v: 2,
        cliente: e.c.clone(),
        equipo: e.id.clone(),
        seq,
        nonce: B64.encode(uuid::Uuid::new_v4().as_bytes()),
        emitida: ahora.to_rfc3339(),
        caduca: (ahora + chrono::Duration::hours(48)).to_rfc3339(),
        not_before: None,
        tipo: tipo.into(),
        cuerpo,
        autorizacion: Autorizacion { prueba_admin: Some(B64.encode(e.prueba)), ..Default::default() },
        responder_a: None,
        por: None,
    }
}

pub(crate) fn enviar(consola: &Consola, e: &Equipo, o: &OrdenV2) {
    let sobre = orden_v2::sellar(o, &e.box_pub).unwrap();
    let (st, r) = consola.pedir(
        "POST",
        &format!("/api/clientes/{}/equipos/{}/ordenes", e.c, e.id),
        Some(json!({ "tipo": o.tipo, "seq": o.seq, "sellado": sobre, "caduca": o.caduca, "not_before": o.not_before })),
    );
    assert_eq!(st, 200, "{r}");
}

pub(crate) fn estado(consola: &Consola, e: &Equipo, seq: u64) -> Value {
    let (_, l) = consola.pedir("GET", &format!("/api/clientes/{}/equipos/{}/ordenes", e.c, e.id), None);
    l.as_array().unwrap().iter().find(|o| o["seq"] == seq).cloned().unwrap_or(Value::Null)
}

#[test]
fn cambio_de_servidor_y_respaldo_con_dos_servidores() {
    let _real = crate::restic::tests::real_repo_lock();
    let dir_agente = std::env::temp_dir().join(format!("resguardo-f6-agente-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir_agente);
    std::env::set_var("RESGUARDO_AGENT_DIR", &dir_agente);

    let (a, codigo_a) = arrancar("a");
    let (b, codigo_b) = arrancar("b");
    let ca = Consola::entrar(&a, &codigo_a);
    let cb = Consola::entrar(&b, &codigo_b);

    // En A: cliente, emparejamiento y alta.
    let (_, cliente) = ca.pedir("POST", "/api/clientes", Some(json!({ "nombre": "Café del Sur", "espera_min_horas": 1 })));
    let (c_a, sal_cliente) = (cliente["id"].as_str().unwrap().to_string(), cliente["sal_cliente"].as_str().unwrap().to_string());
    let (_, emp) = ca.pedir("POST", &format!("/api/clientes/{c_a}/emparejamientos"), None);
    let (codigo, emp_id) = (emp["codigo"].as_str().unwrap().to_string(), emp["id"].as_str().unwrap().to_string());
    vincular(&a.url, &codigo, "PRUEBA-F6").unwrap();
    let (_, est) = ca.pedir("GET", &format!("/api/clientes/{c_a}/emparejamientos/{emp_id}"), None);
    let eq = &est["equipo"];
    let kcfg = derivaciones::k_cfg(CLAVE, &sal_cliente).unwrap();
    let (id, box_pub, sign_pub) =
        (eq["id"].as_str().unwrap().to_string(), eq["box_pub"].as_str().unwrap().to_string(), eq["sign_pub"].as_str().unwrap().to_string());
    let etiqueta = derivaciones::etiqueta_equipo(&kcfg, &id, &box_pub, &sign_pub);
    assert_eq!(ca.pedir("POST", &format!("/api/clientes/{c_a}/emparejamientos/{emp_id}/confirmar"), Some(json!({ "etiqueta": etiqueta }))).0, 200);
    let prueba = derivaciones::prueba_admin(CLAVE, eq["sal_equipo"].as_str().unwrap()).unwrap();
    let verificador = B64.encode(derivaciones::verificador(&prueba));
    let ea = Equipo { c: c_a.clone(), id: id.clone(), box_pub: box_pub.clone(), prueba };
    let mut alta = orden(&ea, 1, "alta", json!({ "verificador": verificador, "k_cfg": B64.encode(kcfg), "espera_min_horas": 1 }));
    alta.autorizacion.prueba_codigo = Some(orden_v2::prueba_codigo(&codigo, &id, &verificador));
    enviar(&ca, &ea, &alta);
    ronda().unwrap();
    assert_eq!(estado(&ca, &ea, 1)["estado"], "hecha");

    // En B: «Recibir un cliente» con la misma sal.
    let (st, rec) = cb.pedir("POST", "/api/clientes/recibir", Some(json!({ "nombre": "Café del Sur", "sal_cliente": sal_cliente, "usos": 5 })));
    assert_eq!(st, 200, "{rec}");
    let c_b = rec["cliente"]["id"].as_str().unwrap().to_string();
    let ficha_b = rec["ficha"].as_str().unwrap().to_string();
    assert_eq!(rec["servidor"]["identidad"], b.identidad.as_str());

    // 1. Un cambio a un servidor que no responde: en marcha y, pasado el plazo, fallida; sigue en A.
    let muerto = format!("https://127.0.0.1:{}", std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port());
    enviar(&ca, &ea, &orden(&ea, 2, "cambiar_servidor", json!({ "url": muerto, "identidad": b.identidad, "ca_pem": b.ca, "ficha": ficha_b })));
    ronda().unwrap();
    assert_eq!(estado(&ca, &ea, 2)["estado"], "en_marcha");
    let mut v = cargar().unwrap();
    v.cambio.as_mut().unwrap().hasta = chrono::Utc::now().timestamp() - 1;
    guardar(&v).unwrap();
    ronda().unwrap();
    let o2 = estado(&ca, &ea, 2);
    assert_eq!(o2["estado"], "fallida", "{o2}");
    assert_eq!(cargar().unwrap().url, a.url, "sigue en el antiguo");
    assert!(cargar().unwrap().cambio.is_none());

    // Una identidad que no es la del servidor nuevo: no se cambia (se reintenta hasta el plazo).
    enviar(&ca, &ea, &orden(&ea, 3, "cambiar_servidor", json!({ "url": b.url, "identidad": a.identidad, "ca_pem": b.ca, "ficha": ficha_b })));
    ronda().unwrap();
    assert_eq!(cargar().unwrap().url, a.url);
    let mut v = cargar().unwrap();
    v.cambio = None;
    guardar(&v).unwrap();

    // 2. El cambio de verdad.
    enviar(&ca, &ea, &orden(&ea, 4, "cambiar_servidor", json!({ "url": b.url, "identidad": b.identidad, "ca_pem": b.ca, "ficha": ficha_b })));
    ronda().unwrap();
    let o4 = estado(&ca, &ea, 4);
    assert_eq!(o4["estado"], "hecha", "{o4}");
    let v = cargar().unwrap();
    assert_eq!((v.url.as_str(), v.cliente_id.as_str(), v.equipo_id.as_str()), (b.url.as_str(), c_b.as_str(), id.as_str()));
    let (_, e_a) = ca.pedir("GET", &format!("/api/clientes/{c_a}/equipos/{id}"), None);
    assert_eq!(e_a["modo"], "trasladado");
    // En B: el mismo equipo, confirmado, con una etiqueta que la consola comprueba con su clave, y su configuración cifrada.
    let (_, e_b) = cb.pedir("GET", &format!("/api/clientes/{c_b}/equipos/{id}"), None);
    assert_eq!(e_b["confirmado"], true);
    assert_eq!(e_b["etiqueta"], etiqueta.as_str());
    let (st, cfg) = cb.pedir("GET", &format!("/api/clientes/{c_b}/equipos/{id}/config"), None);
    assert_eq!(st, 200, "{cfg}");
    // Y ya obedece a B (seq desde 1 otra vez).
    let eb = Equipo { c: c_b.clone(), id: id.clone(), box_pub: box_pub.clone(), prueba };
    enviar(&cb, &eb, &orden(&eb, 1, "cambiar_espera", json!({ "horas": 6 })));
    ronda().unwrap();
    assert_eq!(estado(&cb, &eb, 1)["estado"], "hecha");

    // 3. Respaldo: A (donde el cliente sigue) con una ficha nueva. Si B deja de responder, pasa a A.
    let (_, fa) = ca.pedir("POST", &format!("/api/clientes/{c_a}/fichas"), Some(json!({ "usos": 1, "dias": 365 })));
    let ficha_a = fa["ficha"].as_str().unwrap().to_string();
    enviar(
        &cb,
        &eb,
        &orden(
            &eb,
            2,
            "servidores_respaldo",
            json!({ "servidores": [{ "url": a.url, "identidad": a.identidad, "ca_pem": a.ca, "ficha": ficha_a }], "dias": 1 }),
        ),
    );
    ronda().unwrap();
    assert_eq!(estado(&cb, &eb, 2)["estado"], "hecha");
    // B «muere»: su dirección deja de responder y hace dos días que no contesta.
    let mut v = cargar().unwrap();
    v.url = muerto.clone();
    v.ultimo_ok = chrono::Utc::now().timestamp() - 2 * 86_400;
    guardar(&v).unwrap();
    assert!(ronda().is_err());
    let v = cargar().unwrap();
    assert_eq!(v.url, a.url, "pasó al respaldo");
    assert_eq!(v.equipo_id, id, "el mismo equipo (A lo reconoce por sus claves)");
    assert!(v.respaldo.is_empty());
    assert!(ronda().is_ok());
    let (_, e_a) = ca.pedir("GET", &format!("/api/clientes/{c_a}/equipos/{id}"), None);
    assert_eq!(e_a["modo"], "gestionado", "{e_a}");
    // La espera confirmada sigue en el equipo.
    assert_eq!(v.espera_min_horas, 6);
    let _ = s::informe();

    std::env::remove_var("RESGUARDO_AGENT_DIR");
    let _ = std::fs::remove_dir_all(dir_agente);
    let _ = std::fs::remove_dir_all(a.dir);
    let _ = std::fs::remove_dir_all(b.dir);
}
