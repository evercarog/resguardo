//! Certificado público automático (crate::acme) contra una autoridad ACME de
//! prueba en este mismo equipo: comprueba de verdad las firmas ES256 (JWS), los
//! nonces de un solo uso y el reto HTTP-01 (lo pide al oyente del «puerto 80»),
//! y firma el certificado con la clave de la petición (CSR). Después, el
//! servidor entero: el certificado público para el dominio y el propio (el que
//! fijan los agentes) para `agentes.<dominio>`.

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use resguardo_servidor::acme::{self, ConfigAcme, Retos};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const B64U: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;
const DOMINIO: &str = "consola.test";

// ---------- La autoridad ACME de prueba ----------

#[derive(Default)]
struct Estado {
    base: String,
    /// Dónde está el «puerto 80» del servidor (en la realidad, http://<dominio>:80).
    http: String,
    nonces: HashSet<String>,
    /// kid → punto público (0x04 || X || Y).
    cuentas: HashMap<String, Vec<u8>>,
    /// Huella JWK de cada cuenta.
    huellas: HashMap<String, String>,
    pedidos: HashMap<String, Value>,
    autorizaciones: HashMap<String, Value>,
    certificados: HashMap<String, String>,
    /// Un `badNonce` en el primer pedido, para ver que el cliente reintenta.
    nonce_malo_dado: bool,
    retos_comprobados: usize,
    ca: Option<(rcgen::Certificate, rcgen::KeyPair)>,
}

type E = Arc<Mutex<Estado>>;

fn nonce(e: &E) -> String {
    let n = uuid::Uuid::new_v4().simple().to_string();
    e.lock().unwrap().nonces.insert(n.clone());
    n
}

fn con_nonce(e: &E, estado: StatusCode, location: Option<String>, cuerpo: Value) -> Response {
    let mut r = (estado, Json(cuerpo)).into_response();
    r.headers_mut().insert("replay-nonce", nonce(e).parse().unwrap());
    if let Some(l) = location {
        r.headers_mut().insert("location", l.parse().unwrap());
    }
    r
}

fn problema(e: &E, tipo: &str, detalle: &str) -> Response {
    con_nonce(e, StatusCode::BAD_REQUEST, None, json!({ "type": format!("urn:ietf:params:acme:error:{tipo}"), "detail": detalle }))
}

/// Comprueba el JWS: firma ES256 (con la jwk o la cuenta del kid), nonce de un solo uso y url.
/// Devuelve (kid, carga, jwk si es una cuenta nueva).
type Jws = (Option<String>, Vec<u8>, Option<Value>);

fn abrir_jws(e: &E, url: &str, cuerpo: &[u8]) -> Result<Jws, String> {
    let v: Value = serde_json::from_slice(cuerpo).map_err(|_| "no es JSON")?;
    let (p, c, f) = (v["protected"].as_str().unwrap_or(""), v["payload"].as_str().unwrap_or(""), v["signature"].as_str().unwrap_or(""));
    let protegido: Value = serde_json::from_slice(&B64U.decode(p).map_err(|_| "protected")?).map_err(|_| "protected")?;
    if protegido["alg"] != "ES256" {
        return Err("alg".into());
    }
    if protegido["url"] != url {
        return Err(format!("url: {} != {url}", protegido["url"]));
    }
    let n = protegido["nonce"].as_str().unwrap_or("");
    if !e.lock().unwrap().nonces.remove(n) {
        return Err("badNonce".into());
    }
    let (kid, punto, jwk) = match (protegido["kid"].as_str(), protegido.get("jwk")) {
        (Some(kid), None) => (Some(kid.to_string()), e.lock().unwrap().cuentas.get(kid).cloned().ok_or("cuenta desconocida")?, None),
        (None, Some(jwk)) => {
            let mut punto = vec![4u8];
            punto.extend(B64U.decode(jwk["x"].as_str().unwrap_or("")).map_err(|_| "x")?);
            punto.extend(B64U.decode(jwk["y"].as_str().unwrap_or("")).map_err(|_| "y")?);
            (None, punto, Some(jwk.clone()))
        }
        _ => return Err("ni kid ni jwk".into()),
    };
    let firma = B64U.decode(f).map_err(|_| "firma")?;
    ring::signature::UnparsedPublicKey::new(&ring::signature::ECDSA_P256_SHA256_FIXED, &punto)
        .verify(format!("{p}.{c}").as_bytes(), &firma)
        .map_err(|_| "la firma no es válida")?;
    Ok((kid, B64U.decode(c).map_err(|_| "payload")?, jwk))
}

macro_rules! jws {
    ($e:expr, $ruta:expr, $cuerpo:expr) => {{
        let url = format!("{}{}", $e.lock().unwrap().base, $ruta);
        match abrir_jws(&$e, &url, &$cuerpo) {
            Ok(x) => x,
            Err(m) if m == "badNonce" => return problema(&$e, "badNonce", "nonce gastado"),
            Err(m) => return problema(&$e, "malformed", &m),
        }
    }};
}

async fn directorio(State(e): State<E>) -> Json<Value> {
    let b = e.lock().unwrap().base.clone();
    Json(
        json!({ "newNonce": format!("{b}/nonce"), "newAccount": format!("{b}/cuenta"), "newOrder": format!("{b}/pedido-nuevo"), "meta": { "termsOfService": format!("{b}/condiciones") } }),
    )
}

async fn dar_nonce(State(e): State<E>) -> Response {
    let mut r = StatusCode::OK.into_response();
    r.headers_mut().insert("replay-nonce", nonce(&e).parse().unwrap());
    r
}

async fn cuenta(State(e): State<E>, cuerpo: Bytes) -> Response {
    let (_, carga, jwk) = jws!(e, "/cuenta", cuerpo);
    let carga: Value = serde_json::from_slice(&carga).unwrap_or_default();
    assert_eq!(carga["termsOfServiceAgreed"], true);
    let jwk = jwk.expect("newAccount va con jwk");
    let canonica = format!(r#"{{"crv":"P-256","kty":"EC","x":"{}","y":"{}"}}"#, jwk["x"].as_str().unwrap(), jwk["y"].as_str().unwrap());
    let huella = B64U.encode(<sha2::Sha256 as sha2::Digest>::digest(canonica.as_bytes()));
    let mut punto = vec![4u8];
    punto.extend(B64U.decode(jwk["x"].as_str().unwrap()).unwrap());
    punto.extend(B64U.decode(jwk["y"].as_str().unwrap()).unwrap());
    let mut g = e.lock().unwrap();
    // La misma clave, la misma cuenta.
    let existente = g.cuentas.iter().find(|(_, p)| **p == punto).map(|(k, _)| k.clone());
    let (kid, nueva) = match existente {
        Some(k) => (k, false),
        None => (format!("{}/cuenta/{}", g.base, g.cuentas.len() + 1), true),
    };
    g.cuentas.insert(kid.clone(), punto);
    g.huellas.insert(kid.clone(), huella);
    drop(g);
    con_nonce(&e, if nueva { StatusCode::CREATED } else { StatusCode::OK }, Some(kid), json!({ "status": "valid", "contact": carga["contact"] }))
}

async fn pedido_nuevo(State(e): State<E>, cuerpo: Bytes) -> Response {
    let (kid, carga, _) = jws!(e, "/pedido-nuevo", cuerpo);
    assert!(kid.is_some(), "el pedido va con kid");
    {
        let mut g = e.lock().unwrap();
        if !g.nonce_malo_dado {
            g.nonce_malo_dado = true;
            drop(g);
            return problema(&e, "badNonce", "otra vez, por favor");
        }
    }
    let carga: Value = serde_json::from_slice(&carga).unwrap();
    assert_eq!(carga["identifiers"][0]["value"], DOMINIO);
    let mut g = e.lock().unwrap();
    let n = g.pedidos.len() + 1;
    let b = g.base.clone();
    let token = format!("token-{n}_{}", uuid::Uuid::new_v4().simple());
    g.autorizaciones.insert(
        n.to_string(),
        json!({ "status": "pending", "identifier": { "type": "dns", "value": DOMINIO }, "kid": kid,
                "challenges": [{ "type": "dns-01", "url": format!("{b}/otro/{n}"), "token": "x" }, { "type": "http-01", "url": format!("{b}/reto/{n}"), "token": token, "status": "pending" }] }),
    );
    let pedido = json!({ "status": "pending", "authorizations": [format!("{b}/authz/{n}")], "finalize": format!("{b}/fin/{n}") });
    g.pedidos.insert(n.to_string(), pedido.clone());
    drop(g);
    con_nonce(&e, StatusCode::CREATED, Some(format!("{b}/pedido/{n}")), pedido)
}

async fn autorizacion(State(e): State<E>, Path(n): Path<String>, cuerpo: Bytes) -> Response {
    let (_, carga, _) = jws!(e, &format!("/authz/{n}"), cuerpo);
    assert!(carga.is_empty(), "POST-as-GET");
    let a = e.lock().unwrap().autorizaciones[&n].clone();
    con_nonce(&e, StatusCode::OK, None, a)
}

async fn reto(State(e): State<E>, Path(n): Path<String>, cuerpo: Bytes) -> Response {
    let (kid, carga, _) = jws!(e, &format!("/reto/{n}"), cuerpo);
    assert_eq!(carga, b"{}");
    let (token, http, huella) = {
        let g = e.lock().unwrap();
        let token = g.autorizaciones[&n]["challenges"][1]["token"].as_str().unwrap().to_string();
        (token, g.http.clone(), g.huellas[kid.as_deref().unwrap()].clone())
    };
    // Como Let's Encrypt: pide http://<dominio>/.well-known/acme-challenge/<token>.
    let url = format!("{http}/.well-known/acme-challenge/{token}");
    let respuesta = tokio::task::spawn_blocking(move || ureq::get(&url).call().ok().and_then(|mut r| r.body_mut().read_to_string().ok())).await.unwrap();
    let ok = respuesta.as_deref() == Some(format!("{token}.{huella}").as_str());
    let mut g = e.lock().unwrap();
    g.retos_comprobados += 1;
    let a = g.autorizaciones.get_mut(&n).unwrap();
    a["status"] = json!(if ok { "valid" } else { "invalid" });
    a["challenges"][1]["status"] = json!(if ok { "valid" } else { "invalid" });
    if !ok {
        a["challenges"][1]["error"] = json!({ "detail": format!("respuesta inesperada: {respuesta:?}") });
    }
    let p = g.pedidos.get_mut(&n).unwrap();
    p["status"] = json!(if ok { "ready" } else { "invalid" });
    let r = g.autorizaciones[&n]["challenges"][1].clone();
    drop(g);
    con_nonce(&e, StatusCode::OK, None, r)
}

/// La clave pública de una CSR (PKCS#10): el punto de su SubjectPublicKeyInfo.
fn punto_de_csr(der: &[u8]) -> Vec<u8> {
    fn tlv(d: &[u8]) -> (u8, &[u8], &[u8]) {
        let (t, l0) = (d[0], d[1]);
        let (largo, ini) = if l0 < 0x80 {
            (l0 as usize, 2)
        } else {
            (d[2..2 + (l0 & 0x7f) as usize].iter().fold(0usize, |a, b| (a << 8) | *b as usize), 2 + (l0 & 0x7f) as usize)
        };
        (t, &d[ini..ini + largo], &d[ini + largo..])
    }
    let (_, csr, _) = tlv(der);
    let (_, info, _) = tlv(csr);
    let (_, _, r) = tlv(info); // versión
    let (_, _, r) = tlv(r); // sujeto
    let (_, spki, _) = tlv(r);
    let (_, _, r) = tlv(spki); // algoritmo
    let (t, bits, _) = tlv(r);
    assert_eq!(t, 0x03);
    bits[1..].to_vec()
}

struct ClavePublica(Vec<u8>);
impl rcgen::PublicKeyData for ClavePublica {
    fn der_bytes(&self) -> &[u8] {
        &self.0
    }
    fn algorithm(&self) -> &rcgen::SignatureAlgorithm {
        &rcgen::PKCS_ECDSA_P256_SHA256
    }
}

async fn finalizar(State(e): State<E>, Path(n): Path<String>, cuerpo: Bytes) -> Response {
    let (_, carga, _) = jws!(e, &format!("/fin/{n}"), cuerpo);
    let carga: Value = serde_json::from_slice(&carga).unwrap();
    let csr = B64U.decode(carga["csr"].as_str().unwrap()).unwrap();
    let mut g = e.lock().unwrap();
    assert_eq!(g.pedidos[&n]["status"], "ready", "se finaliza con el pedido listo");
    let (ca, ca_key) = g.ca.as_ref().unwrap();
    let mut p = rcgen::CertificateParams::new(vec![DOMINIO.to_string()]).unwrap();
    // Desde ayer, 90 días (como Let's Encrypt).
    let fecha = |d: chrono::NaiveDate| rcgen::date_time_ymd(chrono::Datelike::year(&d), chrono::Datelike::month(&d) as u8, chrono::Datelike::day(&d) as u8);
    let ayer = chrono::Utc::now().date_naive() - chrono::Duration::days(1);
    p.not_before = fecha(ayer);
    p.not_after = fecha(ayer + chrono::Duration::days(90));
    let cert = p.signed_by(&ClavePublica(punto_de_csr(&csr)), ca, ca_key).unwrap();
    let cadena = format!("{}{}", cert.pem(), ca.pem());
    let b = g.base.clone();
    g.certificados.insert(n.clone(), cadena);
    let pedido = g.pedidos.get_mut(&n).unwrap();
    pedido["status"] = json!("processing");
    pedido["certificate"] = json!(format!("{b}/cert/{n}"));
    let r = pedido.clone();
    drop(g);
    con_nonce(&e, StatusCode::OK, None, r)
}

async fn ver_pedido(State(e): State<E>, Path(n): Path<String>, cuerpo: Bytes) -> Response {
    let _ = jws!(e, &format!("/pedido/{n}"), cuerpo);
    let mut g = e.lock().unwrap();
    let p = g.pedidos.get_mut(&n).unwrap();
    // «processing» una vez y luego «valid»: el cliente tiene que esperar.
    let r = p.clone();
    if p["status"] == "processing" {
        p["status"] = json!("valid");
    }
    drop(g);
    con_nonce(&e, StatusCode::OK, None, r)
}

async fn certificado(State(e): State<E>, Path(n): Path<String>, cuerpo: Bytes) -> Response {
    let _ = jws!(e, &format!("/cert/{n}"), cuerpo);
    let pem = e.lock().unwrap().certificados[&n].clone();
    let mut r = pem.into_response();
    r.headers_mut().insert("replay-nonce", nonce(&e).parse().unwrap());
    r.headers_mut().insert("content-type", "application/pem-certificate-chain".parse().unwrap());
    r
}

/// Arranca la autoridad de prueba; `http` es la base del «puerto 80» del servidor.
async fn autoridad(http: &str) -> (String, E, String) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let ca_key = rcgen::KeyPair::generate().unwrap();
    let mut p = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
    p.distinguished_name.push(rcgen::DnType::CommonName, "Autoridad ACME de prueba");
    p.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    p.key_usages = vec![rcgen::KeyUsagePurpose::KeyCertSign, rcgen::KeyUsagePurpose::DigitalSignature];
    let ca = p.self_signed(&ca_key).unwrap();
    let ca_pem = ca.pem();
    let e: E = Arc::new(Mutex::new(Estado { base: base.clone(), http: http.to_string(), ca: Some((ca, ca_key)), ..Default::default() }));
    let app = Router::new()
        .route("/dir", get(directorio))
        .route("/nonce", get(dar_nonce).head(dar_nonce))
        .route("/cuenta", post(cuenta))
        .route("/pedido-nuevo", post(pedido_nuevo))
        .route("/authz/{n}", post(autorizacion))
        .route("/reto/{n}", post(reto))
        .route("/fin/{n}", post(finalizar))
        .route("/pedido/{n}", post(ver_pedido))
        .route("/cert/{n}", post(certificado))
        .with_state(e.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("{base}/dir"), e, ca_pem)
}

fn config(directorio: &str) -> ConfigAcme {
    let mut c = ConfigAcme::nueva(DOMINIO, Some("ana@ejemplo.com".into()), false);
    c.directorio = directorio.to_string();
    c.pausa = Duration::from_millis(20);
    c
}

#[tokio::test(flavor = "multi_thread")]
async fn obtiene_el_certificado_con_el_reto_http01() {
    // El «puerto 80» del servidor, con sus retos.
    let retos = Retos::default();
    let l80 = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let http = format!("http://{}", l80.local_addr().unwrap());
    let app80 = acme::app_http(retos.clone(), DOMINIO.into(), 443);
    tokio::spawn(async move { axum::serve(l80, app80).await.unwrap() });
    let (dir, e, ca_pem) = autoridad(&http).await;
    let carpeta = tempfile::tempdir().unwrap();

    let (cfg, d, r) = (config(&dir), carpeta.path().to_path_buf(), retos.clone());
    let (pem, clave) = tokio::task::spawn_blocking(move || acme::obtener(&cfg, &d, &r)).await.unwrap().expect("certificado");
    assert!(pem.matches("BEGIN CERTIFICATE").count() == 2, "la cadena entera");
    assert!(clave.contains("PRIVATE KEY"));
    let (desde, hasta) = acme::vigencia_pem(&pem).unwrap();
    assert_eq!(hasta - desde, 90 * 86_400);
    assert!(!acme::toca_renovar(&pem, chrono::Utc::now().timestamp()));
    assert!(acme::certificado_rustls(&pem, &clave).unwrap().keys_match().is_ok());
    assert!(retos.respuesta("x").is_none(), "los retos se quitan al terminar");
    assert!(carpeta.path().join("cuenta.key").is_file(), "la clave de la cuenta se guarda");
    // El certificado es de la autoridad para el dominio (como lo comprueba un navegador).
    verificar_cadena(&pem, &ca_pem, DOMINIO);
    {
        let g = e.lock().unwrap();
        assert_eq!(g.retos_comprobados, 1);
        assert!(g.nonce_malo_dado, "reintentó tras badNonce");
    }

    // Otra vez (renovar): la misma cuenta.
    let (cfg, d, r) = (config(&dir), carpeta.path().to_path_buf(), retos.clone());
    tokio::task::spawn_blocking(move || acme::obtener(&cfg, &d, &r)).await.unwrap().expect("renovado");
    assert_eq!(e.lock().unwrap().cuentas.len(), 1, "la misma clave de cuenta");

    // Si el reto no responde (el puerto 80 no llega a este servidor), falla con el motivo.
    let (dir2, _e2, _) = autoridad("http://127.0.0.1:9").await;
    let (cfg, d) = (config(&dir2), carpeta.path().to_path_buf());
    let err = tokio::task::spawn_blocking(move || acme::obtener(&cfg, &d, &Retos::default())).await.unwrap().unwrap_err();
    assert!(err.contains("no válido"), "{err}");
    // Y una autoridad por http:// que no es este equipo, ni se intenta.
    let mut cfg = config("http://acme.ejemplo.com/dir");
    cfg.pausa = Duration::from_millis(1);
    assert!(acme::obtener(&cfg, carpeta.path(), &Retos::default()).unwrap_err().contains("https://"));
}

/// Verifica la cadena como un navegador que confía en `ca_pem`.
fn verificar_cadena(cadena: &str, ca_pem: &str, nombre: &str) {
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
    let certs: Vec<CertificateDer> = CertificateDer::pem_slice_iter(cadena.as_bytes()).map(Result::unwrap).collect();
    let mut raices = rustls::RootCertStore::empty();
    raices.add(CertificateDer::from_pem_slice(ca_pem.as_bytes()).unwrap()).unwrap();
    let v = rustls::client::WebPkiServerVerifier::builder_with_provider(Arc::new(raices), Arc::new(rustls::crypto::ring::default_provider())).build().unwrap();
    use rustls::client::danger::ServerCertVerifier;
    v.verify_server_cert(&certs[0], &certs[1..], &ServerName::try_from(nombre.to_string()).unwrap(), &[], UnixTime::now()).expect("cadena válida");
}

/// Abre TLS con `sni` confiando solo en `ca_pem` (como un agente con su autoridad fijada).
async fn saludo_tls(direccion: SocketAddr, sni: &str, ca_pem: &str) -> Result<(), String> {
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::{CertificateDer, ServerName};
    let mut raices = rustls::RootCertStore::empty();
    for c in CertificateDer::pem_slice_iter(ca_pem.as_bytes()) {
        raices.add(c.unwrap()).unwrap();
    }
    let cfg = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(raices)
        .with_no_client_auth();
    let tcp = tokio::net::TcpStream::connect(direccion).await.map_err(|e| e.to_string())?;
    tokio_rustls::TlsConnector::from(Arc::new(cfg)).connect(ServerName::try_from(sni.to_string()).unwrap(), tcp).await.map(|_| ()).map_err(|e| e.to_string())
}

fn puerto_libre() -> SocketAddr {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn el_servidor_da_el_publico_en_el_dominio_y_el_propio_a_los_agentes() {
    let datos = tempfile::tempdir().unwrap();
    let agentes = format!("agentes.{DOMINIO}");
    let huella = resguardo_servidor::identidad::preparar_tls(datos.path(), &[DOMINIO.to_string(), agentes.clone()]).unwrap();
    let propia_ca = std::fs::read_to_string(datos.path().join("tls").join("ca.crt")).unwrap();
    let st = resguardo_servidor::preparar(datos.path(), resguardo_servidor::estado::Opciones { publico: true, ..Default::default() }).unwrap();
    assert_eq!(st.huella_ca, huella);
    let (https, http) = (puerto_libre(), puerto_libre());
    let (dir, _e, ca_publica) = autoridad(&format!("http://{http}")).await;
    let tls = resguardo_servidor::Tls::Publico {
        cert: datos.path().join("tls").join("servidor.crt"),
        clave: datos.path().join("tls").join("servidor.key"),
        acme: config(&dir),
        http,
    };
    tokio::spawn(resguardo_servidor::servir(st, https, tls));
    // Hasta que lo tenga (pide el certificado al arrancar).
    let guardado = datos.path().join("acme").join(format!("{DOMINIO}.crt"));
    for _ in 0..200 {
        if guardado.is_file() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(guardado.is_file(), "el certificado público se guarda en <datos>/acme");
    tokio::time::sleep(Duration::from_millis(100)).await;
    // El dominio, con el público (el navegador confía en su autoridad)…
    saludo_tls(https, DOMINIO, &ca_publica).await.expect("el dominio lleva el certificado público");
    assert!(saludo_tls(https, DOMINIO, &propia_ca).await.is_err(), "y no el propio");
    // …y agentes.<dominio>, el de la autoridad propia: lo que fijó el agente al vincularse.
    saludo_tls(https, &agentes, &propia_ca).await.expect("los agentes siguen con la autoridad propia");
    assert!(saludo_tls(https, &agentes, &ca_publica).await.is_err());
    // El puerto 80 lleva a HTTPS en el dominio.
    let r = ureq::Agent::config_builder().max_redirects(0).build().new_agent().get(&format!("http://{http}/entrar")).call().unwrap();
    assert_eq!(r.status(), 301);
    assert_eq!(r.headers()["location"], format!("https://{DOMINIO}:{}/entrar", https.port()));
}
