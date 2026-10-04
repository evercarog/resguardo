//! Certificado público automático para una consola en internet (ACME, RFC 8555;
//! Let's Encrypt): `--dominio consola.ejemplo.com --acme-correo tu@correo`.
//!
//! - Reto HTTP-01 en el puerto 80. Ese mismo oyente solo responde a los retos y
//!   redirige todo lo demás a `https://<dominio>/…`.
//! - El certificado, su clave y la clave de la cuenta ACME van en `<datos>/acme/`
//!   (privados). Se renueva solo cuando queda un tercio de su vigencia (con
//!   Let's Encrypt, unos 30 días de 90), con reintentos espaciados si falla.
//! - Solo se sirve a quien pide el nombre del dominio (SNI). A los agentes y a
//!   las otras consolas se les da otra dirección, `agentes.<dominio>` (o la IP),
//!   donde se sirve el certificado de la autoridad propia que fijan al
//!   vincularse: el SAS v3 sigue cubriendo esa autoridad, y una renovación o un
//!   cambio de raíz de Let's Encrypt no les afecta. Ver docs/consola-en-linea.md.
//!
//! Sin dependencias nuevas: HTTP con `ureq` (rustls y las raíces de webpki),
//! firmas ES256 con `ring` y la petición de certificado (CSR) con `rcgen`.

use base64::Engine;
use ring::rand::SystemRandom;
use ring::signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_FIXED_SIGNING};
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

const B64U: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// Let's Encrypt (certificados de verdad).
pub const LETS_ENCRYPT: &str = "https://acme-v02.api.letsencrypt.org/directory";
/// Let's Encrypt de pruebas (`--acme-pruebas`): sin sus límites, pero los navegadores no confían en él.
pub const LETS_ENCRYPT_PRUEBAS: &str = "https://acme-staging-v02.api.letsencrypt.org/directory";

/// Lo que hace falta para pedir el certificado.
#[derive(Clone, Debug)]
pub struct ConfigAcme {
    /// El nombre público de la consola (`consola.ejemplo.com`), ya validado.
    pub dominio: String,
    /// Correo para los avisos de la autoridad (caducidad, cambios). Opcional.
    pub correo: Option<String>,
    /// URL del directorio ACME.
    pub directorio: String,
    /// Pausa entre consultas mientras la autoridad comprueba (las pruebas la acortan).
    pub pausa: Duration,
}

impl ConfigAcme {
    pub fn nueva(dominio: &str, correo: Option<String>, pruebas: bool) -> Self {
        Self {
            dominio: dominio.to_string(),
            correo,
            directorio: if pruebas { LETS_ENCRYPT_PRUEBAS } else { LETS_ENCRYPT }.to_string(),
            pausa: Duration::from_secs(2),
        }
    }
}

/// Un nombre de dominio público válido (en minúsculas): letras, cifras, guiones
/// y puntos, con al menos un punto, sin ser una IP.
pub fn valida_dominio(d: &str) -> Result<String, String> {
    let d = d.trim().trim_end_matches('.').to_ascii_lowercase();
    let etiquetas_ok = d.split('.').all(|e| !e.is_empty() && e.len() <= 63 && !e.starts_with('-') && !e.ends_with('-'));
    let ok = d.len() <= 253
        && d.contains('.')
        && etiquetas_ok
        && d.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
        && d.parse::<std::net::IpAddr>().is_err()
        && !d.rsplit('.').next().is_some_and(|tld| tld.chars().all(|c| c.is_ascii_digit()));
    if ok {
        Ok(d)
    } else {
        Err(format!("«{d}» no es un nombre de dominio válido (p. ej. consola.ejemplo.com; una IP no sirve para un certificado público)."))
    }
}

/// Un correo con forma de correo (para `mailto:`).
pub fn valida_correo(c: &str) -> Result<String, String> {
    let c = c.trim().to_string();
    let ok = c.len() <= 254
        && c.split_once('@').is_some_and(|(u, d)| !u.is_empty() && d.contains('.'))
        && !c.chars().any(|ch| ch.is_whitespace() || ch.is_control() || matches!(ch, '<' | '>' | '"' | ',' | ';'));
    if ok {
        Ok(c)
    } else {
        Err(format!("«{c}» no es un correo válido."))
    }
}

// ---------- Retos HTTP-01 ----------

/// Retos HTTP-01 en curso: token → autorización (`token.huella`).
#[derive(Clone, Default)]
pub struct Retos(Arc<Mutex<HashMap<String, String>>>);

impl Retos {
    pub fn respuesta(&self, token: &str) -> Option<String> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).get(token).cloned()
    }
    fn poner(&self, token: &str, autorizacion: &str) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).insert(token.to_string(), autorizacion.to_string());
    }
    fn quitar(&self, token: &str) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).remove(token);
    }
}

/// Quita el reto al terminar (bien o mal).
struct RetoPuesto<'a>(&'a Retos, String);
impl Drop for RetoPuesto<'_> {
    fn drop(&mut self) {
        self.0.quitar(&self.1);
    }
}

/// El oyente del puerto 80: los retos y, lo demás, a HTTPS en el dominio (nunca a
/// otro nombre: la cabecera Host no decide adónde se redirige).
pub fn app_http(retos: Retos, dominio: String, puerto_https: u16) -> axum::Router {
    use axum::extract::{Path as Ruta, State};
    use axum::http::{header, StatusCode, Uri};
    use axum::response::{IntoResponse, Response};
    #[derive(Clone)]
    struct E {
        retos: Retos,
        destino: String,
    }
    async fn reto(State(e): State<E>, Ruta(token): Ruta<String>) -> Response {
        match e.retos.respuesta(&token) {
            Some(a) => ([(header::CONTENT_TYPE, "text/plain")], a).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        }
    }
    async fn redirigir(State(e): State<E>, uri: Uri) -> Response {
        let ruta = uri.path_and_query().map(|p| p.as_str()).unwrap_or("/");
        let ruta = if ruta.starts_with('/') && !ruta.starts_with("//") { ruta } else { "/" };
        (StatusCode::MOVED_PERMANENTLY, [(header::LOCATION, format!("{}{ruta}", e.destino))]).into_response()
    }
    let destino = if puerto_https == 443 { format!("https://{dominio}") } else { format!("https://{dominio}:{puerto_https}") };
    axum::Router::new().route("/.well-known/acme-challenge/{token}", axum::routing::get(reto)).fallback(redirigir).with_state(E { retos, destino })
}

// ---------- Certificados que sirve el servidor ----------

/// Lo que elige rustls en cada conexión: el certificado público para el dominio
/// (si ya lo hay) y el de la autoridad propia para todo lo demás (agentes,
/// otras consolas, la IP, la red local).
pub struct Certificados {
    dominio: String,
    publico: RwLock<Option<Arc<CertifiedKey>>>,
    propio: Arc<CertifiedKey>,
}

impl std::fmt::Debug for Certificados {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Certificados").field("dominio", &self.dominio).finish_non_exhaustive()
    }
}

impl Certificados {
    pub fn nuevos(dominio: &str, propio: Arc<CertifiedKey>) -> Self {
        Self { dominio: dominio.to_ascii_lowercase(), publico: RwLock::new(None), propio }
    }
    pub fn poner_publico(&self, c: Arc<CertifiedKey>) {
        *self.publico.write().unwrap_or_else(|e| e.into_inner()) = Some(c);
    }
    pub fn tiene_publico(&self) -> bool {
        self.publico.read().unwrap_or_else(|e| e.into_inner()).is_some()
    }
    /// El certificado para este nombre (SNI); sin nombre (una IP), el propio.
    pub fn para(&self, sni: Option<&str>) -> Arc<CertifiedKey> {
        if sni.is_some_and(|n| n.eq_ignore_ascii_case(&self.dominio)) {
            if let Some(c) = self.publico.read().unwrap_or_else(|e| e.into_inner()).clone() {
                return c;
            }
        }
        self.propio.clone()
    }
}

impl ResolvesServerCert for Certificados {
    fn resolve(&self, hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        Some(self.para(hello.server_name()))
    }
}

/// Certificado (cadena PEM) y clave (PEM) listos para rustls.
pub fn certificado_rustls(cert_pem: &str, key_pem: &str) -> Result<Arc<CertifiedKey>, String> {
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};
    let certs: Vec<CertificateDer<'static>> =
        CertificateDer::pem_slice_iter(cert_pem.as_bytes()).collect::<Result<_, _>>().map_err(|e| format!("Certificado no válido: {e}"))?;
    if certs.is_empty() {
        return Err("No hay certificados.".into());
    }
    let key = PrivateKeyDer::from_pem_slice(key_pem.as_bytes()).map_err(|e| format!("Clave no válida: {e}"))?;
    let firma = rustls::crypto::ring::sign::any_supported_type(&key).map_err(|e| e.to_string())?;
    Ok(Arc::new(CertifiedKey::new(certs, firma)))
}

/// Configuración de rustls que elige el certificado con [`Certificados`].
pub fn config_rustls(certs: Arc<Certificados>) -> Result<Arc<rustls::ServerConfig>, String> {
    let mut cfg = rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_no_client_auth()
        .with_cert_resolver(certs);
    cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::new(cfg))
}

// ---------- Vigencia de un certificado (DER, sin dependencias) ----------

/// Lee un TLV de DER: (etiqueta, contenido, resto).
fn tlv(d: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let (&etiqueta, d) = d.split_first()?;
    let (&l0, d) = d.split_first()?;
    let (largo, d) = if l0 < 0x80 {
        (l0 as usize, d)
    } else {
        let n = (l0 & 0x7f) as usize;
        if n == 0 || n > 4 || d.len() < n {
            return None;
        }
        (d[..n].iter().fold(0usize, |a, b| (a << 8) | *b as usize), &d[n..])
    };
    (d.len() >= largo).then(|| (etiqueta, &d[..largo], &d[largo..]))
}

fn hora_der(etiqueta: u8, v: &[u8]) -> Option<i64> {
    let t = std::str::from_utf8(v).ok()?.strip_suffix('Z')?;
    let t = match etiqueta {
        // UTCTime: AAMMDDhhmmss (50-99 → 19xx).
        0x17 if t.len() == 12 => {
            let aa: i32 = t[..2].parse().ok()?;
            format!("{}{}", if aa >= 50 { 1900 + aa } else { 2000 + aa }, &t[2..])
        }
        // GeneralizedTime: AAAAMMDDhhmmss.
        0x18 if t.len() == 14 => t.to_string(),
        _ => return None,
    };
    chrono::NaiveDateTime::parse_from_str(&t, "%Y%m%d%H%M%S").ok().map(|d| d.and_utc().timestamp())
}

/// (desde, hasta) del certificado, en segundos Unix.
pub fn vigencia_der(der: &[u8]) -> Option<(i64, i64)> {
    let (0x30, cert, _) = tlv(der)? else { return None };
    let (0x30, tbs, _) = tlv(cert)? else { return None };
    let mut resto = tbs;
    // [0] versión (opcional).
    if resto.first() == Some(&0xa0) {
        resto = tlv(resto)?.2;
    }
    // Número de serie, algoritmo y emisor.
    for _ in 0..3 {
        resto = tlv(resto)?.2;
    }
    let (0x30, validez, _) = tlv(resto)? else { return None };
    let (e1, desde, r) = tlv(validez)?;
    let (e2, hasta, _) = tlv(r)?;
    Some((hora_der(e1, desde)?, hora_der(e2, hasta)?))
}

/// La vigencia del primer certificado (el del servidor) de una cadena PEM.
pub fn vigencia_pem(pem: &str) -> Option<(i64, i64)> {
    use rustls::pki_types::pem::PemObject;
    let cert = rustls::pki_types::CertificateDer::pem_slice_iter(pem.as_bytes()).next()?.ok()?;
    vigencia_der(cert.as_ref())
}

/// ¿Toca pedir otro? Cuando queda un tercio de su vigencia (o ya no vale).
pub fn toca_renovar(pem: &str, ahora: i64) -> bool {
    match vigencia_pem(pem) {
        Some((desde, hasta)) if hasta > desde => ahora >= hasta - (hasta - desde) / 3,
        _ => true,
    }
}

// ---------- Archivos ----------

/// `<datos>/acme`.
pub fn carpeta(datos: &Path) -> PathBuf {
    datos.join("acme")
}

/// Cada autoridad ACME con su carpeta (cuenta y certificados): el Let's Encrypt de
/// verdad en `<datos>/acme`; el de pruebas u otra, en una subcarpeta. Así, al quitar
/// `--acme-pruebas`, no se sigue sirviendo el certificado de pruebas.
pub fn carpeta_de_autoridad(carpeta: &Path, directorio: &str) -> PathBuf {
    if directorio == LETS_ENCRYPT {
        carpeta.to_path_buf()
    } else if directorio == LETS_ENCRYPT_PRUEBAS {
        carpeta.join("pruebas")
    } else {
        let h = Sha256::digest(directorio.as_bytes());
        carpeta.join(format!("otra-{}", B64U.encode(&h[..6])))
    }
}

fn archivos(carpeta: &Path, dominio: &str) -> (PathBuf, PathBuf) {
    (carpeta.join(format!("{dominio}.crt")), carpeta.join(format!("{dominio}.key")))
}

/// El certificado guardado para ese dominio, si lo hay: (cadena PEM, clave PEM).
pub fn guardado(carpeta: &Path, dominio: &str) -> Option<(String, String)> {
    let (c, k) = archivos(carpeta, dominio);
    Some((std::fs::read_to_string(c).ok()?, std::fs::read_to_string(k).ok()?))
}

fn guardar(carpeta: &Path, dominio: &str, cert: &str, clave: &str) -> Result<(), String> {
    std::fs::create_dir_all(carpeta).map_err(|e| e.to_string())?;
    let (c, k) = archivos(carpeta, dominio);
    crate::identidad::escribir_privado(&k, clave.as_bytes())?;
    crate::identidad::escribir_privado(&c, cert.as_bytes())
}

/// La clave de la cuenta ACME (P-256, PKCS#8): la crea la primera vez.
fn clave_cuenta(carpeta: &Path) -> Result<EcdsaKeyPair, String> {
    use rustls::pki_types::pem::PemObject;
    let ruta = carpeta.join("cuenta.key");
    let rng = SystemRandom::new();
    let pkcs8 = match std::fs::read_to_string(&ruta) {
        Ok(pem) => rustls::pki_types::PrivatePkcs8KeyDer::from_pem_slice(pem.as_bytes())
            .map_err(|_| format!("{} dañada.", ruta.display()))?
            .secret_pkcs8_der()
            .to_vec(),
        Err(_) => {
            let doc = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).map_err(|_| "No se pudo crear la clave de la cuenta ACME.")?;
            std::fs::create_dir_all(carpeta).map_err(|e| e.to_string())?;
            let pem = format!("-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n", base64::engine::general_purpose::STANDARD.encode(doc.as_ref()));
            crate::identidad::escribir_privado(&ruta, pem.as_bytes())?;
            doc.as_ref().to_vec()
        }
    };
    EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &pkcs8, &rng).map_err(|_| format!("{} no es una clave P-256.", ruta.display()))
}

// ---------- Cliente ACME (RFC 8555) ----------

/// La clave pública como JWK (RFC 7517), con los campos en el orden de RFC 7638.
pub fn jwk(clave: &EcdsaKeyPair) -> String {
    let p = clave.public_key().as_ref();
    // Punto sin comprimir: 0x04 || X || Y.
    format!(r#"{{"crv":"P-256","kty":"EC","x":"{}","y":"{}"}}"#, B64U.encode(&p[1..33]), B64U.encode(&p[33..65]))
}

/// Huella de la JWK (RFC 7638): la segunda mitad de la autorización de un reto.
pub fn huella_jwk(clave: &EcdsaKeyPair) -> String {
    B64U.encode(Sha256::digest(jwk(clave).as_bytes()))
}

struct Respuesta {
    estado: u16,
    location: Option<String>,
    nonce: Option<String>,
    retry_after: Option<u64>,
    cuerpo: Vec<u8>,
}

impl Respuesta {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.cuerpo).unwrap_or(Value::Null)
    }
}

/// El problema que explica la autoridad (RFC 7807), en una línea.
fn problema(r: &Respuesta) -> String {
    let v = r.json();
    let detalle = v["detail"].as_str().unwrap_or("");
    let tipo = v["type"].as_str().unwrap_or("").trim_start_matches("urn:ietf:params:acme:error:");
    format!("la autoridad respondió {} {tipo}: {detalle}", r.estado)
}

struct Cliente {
    http: ureq::Agent,
    clave: EcdsaKeyPair,
    kid: Option<String>,
    nonce: Option<String>,
    url_nonce: String,
    pausa: Duration,
    rng: SystemRandom,
}

/// HTTPS siempre; `http://` solo a este mismo equipo (un servidor ACME de pruebas).
fn url_permitida(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://127.0.0.1:") || url.starts_with("http://localhost:") || url.starts_with("http://[::1]:")
}

fn leer(r: Result<axum::http::Response<ureq::Body>, ureq::Error>) -> Result<Respuesta, String> {
    let mut r = r.map_err(|e| format!("no se pudo hablar con la autoridad: {e}"))?;
    let cab = |k: &str| r.headers().get(k).and_then(|v| v.to_str().ok()).map(str::to_string);
    let (location, nonce) = (cab("location"), cab("replay-nonce"));
    let retry_after = cab("retry-after").and_then(|v| v.trim().parse().ok());
    let estado = r.status().as_u16();
    let cuerpo = r.body_mut().with_config().limit(1024 * 1024).read_to_vec().map_err(|e| format!("respuesta de la autoridad incompleta: {e}"))?;
    Ok(Respuesta { estado, location, nonce, retry_after, cuerpo })
}

impl Cliente {
    fn nonce(&mut self) -> Result<String, String> {
        if let Some(n) = self.nonce.take() {
            return Ok(n);
        }
        let r = leer(self.http.head(&self.url_nonce).call())?;
        r.nonce.ok_or_else(|| "la autoridad no dio un nonce.".to_string())
    }

    fn jws(&self, url: &str, nonce: &str, carga: Option<&Value>) -> Result<String, String> {
        let mut protegido = json!({ "alg": "ES256", "nonce": nonce, "url": url });
        match &self.kid {
            Some(k) => protegido["kid"] = json!(k),
            None => protegido["jwk"] = serde_json::from_str(&jwk(&self.clave)).map_err(|e| e.to_string())?,
        }
        let p = B64U.encode(protegido.to_string());
        // POST-as-GET: carga vacía.
        let c = carga.map(|v| B64U.encode(v.to_string())).unwrap_or_default();
        let firma = self.clave.sign(&self.rng, format!("{p}.{c}").as_bytes()).map_err(|_| "No se pudo firmar la petición ACME.")?;
        Ok(json!({ "protected": p, "payload": c, "signature": B64U.encode(firma.as_ref()) }).to_string())
    }

    /// POST firmado; con `badNonce`, otra vez con el nonce nuevo (hasta 3 veces).
    fn post(&mut self, url: &str, carga: Option<&Value>) -> Result<Respuesta, String> {
        if !url_permitida(url) {
            return Err(format!("la autoridad dio una dirección no segura: {url}"));
        }
        let mut intento = 0;
        loop {
            intento += 1;
            let nonce = self.nonce()?;
            let cuerpo = self.jws(url, &nonce, carga)?;
            let r = leer(self.http.post(url).header("content-type", "application/jose+json").send(cuerpo.as_bytes()))?;
            self.nonce = r.nonce.clone();
            if r.estado < 400 {
                return Ok(r);
            }
            if intento < 3 && r.json()["type"] == "urn:ietf:params:acme:error:badNonce" {
                continue;
            }
            return Err(problema(&r));
        }
    }

    /// Consulta `url` (POST-as-GET) hasta que su `status` sea uno de `listo`.
    fn esperar(&mut self, url: &str, listo: &[&str], que: &str) -> Result<Value, String> {
        for _ in 0..60 {
            let r = self.post(url, None)?;
            let v = r.json();
            let estado = v["status"].as_str().unwrap_or("");
            if listo.contains(&estado) {
                return Ok(v);
            }
            if estado == "invalid" {
                let detalle = v["challenges"]
                    .as_array()
                    .and_then(|l| l.iter().find_map(|c| c["error"]["detail"].as_str()))
                    .or(v["error"]["detail"].as_str())
                    .unwrap_or("sin detalle");
                return Err(format!("{que}: no válido ({detalle})"));
            }
            // Lo que pida la autoridad (Retry-After), como mucho 5 pausas.
            let pausa = r.retry_after.map(|s| Duration::from_secs(s.clamp(1, 10))).unwrap_or(self.pausa).min(self.pausa * 5);
            std::thread::sleep(pausa);
        }
        Err(format!("{que}: la autoridad no terminó a tiempo"))
    }
}

/// Pide un certificado para `cfg.dominio` (reto HTTP-01 en `retos`) y lo
/// devuelve sin guardarlo: (cadena PEM, clave PEM). Bloquea (hilo aparte).
pub fn obtener(cfg: &ConfigAcme, carpeta: &Path, retos: &Retos) -> Result<(String, String), String> {
    if !url_permitida(&cfg.directorio) {
        return Err(format!("El directorio ACME tiene que ser https:// ({}).", cfg.directorio));
    }
    let http = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .user_agent(format!("Resguardo-Server/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .new_agent();
    let dir = leer(http.get(&cfg.directorio).call())?;
    if dir.estado != 200 {
        return Err(format!("el directorio ACME respondió {}", dir.estado));
    }
    let dir = dir.json();
    let url = |k: &str| dir[k].as_str().map(str::to_string).ok_or_else(|| format!("al directorio ACME le falta «{k}»"));
    let (nueva_cuenta, nuevo_pedido) = (url("newAccount")?, url("newOrder")?);
    let mut c =
        Cliente { http, clave: clave_cuenta(carpeta)?, kid: None, nonce: None, url_nonce: url("newNonce")?, pausa: cfg.pausa, rng: SystemRandom::new() };
    // Cuenta (la misma clave devuelve la misma cuenta).
    let mut cuenta = json!({ "termsOfServiceAgreed": true });
    if let Some(correo) = &cfg.correo {
        cuenta["contact"] = json!([format!("mailto:{correo}")]);
    }
    let r = c.post(&nueva_cuenta, Some(&cuenta))?;
    c.kid = Some(r.location.ok_or("la autoridad no dio la dirección de la cuenta")?);
    // Pedido.
    let r = c.post(&nuevo_pedido, Some(&json!({ "identifiers": [{ "type": "dns", "value": cfg.dominio }] })))?;
    let url_pedido = r.location.clone().ok_or("la autoridad no dio la dirección del pedido")?;
    let pedido = r.json();
    let autorizaciones: Vec<String> =
        pedido["authorizations"].as_array().map(|l| l.iter().filter_map(|a| a.as_str().map(str::to_string)).collect()).unwrap_or_default();
    let huella = huella_jwk(&c.clave);
    for url_autorizacion in &autorizaciones {
        let a = c.post(url_autorizacion, None)?.json();
        if a["status"] == "valid" {
            continue;
        }
        let reto = a["challenges"].as_array().and_then(|l| l.iter().find(|x| x["type"] == "http-01")).ok_or("la autoridad no ofrece el reto http-01")?.clone();
        let token = reto["token"].as_str().unwrap_or("");
        if token.is_empty() || token.len() > 256 || !token.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_') {
            return Err("la autoridad dio un reto no válido".into());
        }
        let url_reto = reto["url"].as_str().ok_or("al reto le falta su dirección")?.to_string();
        retos.poner(token, &format!("{token}.{huella}"));
        let _quitar = RetoPuesto(retos, token.to_string());
        c.post(&url_reto, Some(&json!({})))?;
        c.esperar(url_autorizacion, &["valid"], &format!("el reto de {}", cfg.dominio))?;
    }
    let pedido = c.esperar(&url_pedido, &["ready", "valid"], "el pedido")?;
    // Clave nueva en cada certificado (como hacen certbot y Caddy).
    let clave = rcgen::KeyPair::generate().map_err(|e| e.to_string())?;
    let csr = csr_de(&cfg.dominio, &clave)?;
    if pedido["status"] == "ready" {
        let fin = pedido["finalize"].as_str().ok_or("al pedido le falta «finalize»")?.to_string();
        c.post(&fin, Some(&json!({ "csr": B64U.encode(csr.der()) })))?;
    }
    let pedido = c.esperar(&url_pedido, &["valid"], "el certificado")?;
    let url_cert = pedido["certificate"].as_str().ok_or("al pedido le falta el certificado")?.to_string();
    let pem = String::from_utf8(c.post(&url_cert, None)?.cuerpo).map_err(|_| "certificado no válido")?;
    // Que sirva de verdad: la cadena se lee y la clave es la suya.
    certificado_rustls(&pem, &clave.serialize_pem())?.keys_match().map_err(|e| format!("el certificado no es de la clave pedida ({e})"))?;
    Ok((pem, clave.serialize_pem()))
}

/// Pone el certificado guardado (si lo hay) y arranca la renovación: pide uno
/// nuevo cuando falta o toca, lo guarda y lo pone en `certs` sin reiniciar.
pub fn arrancar(cfg: ConfigAcme, carpeta: PathBuf, retos: Retos, certs: Arc<Certificados>) {
    let carpeta = carpeta_de_autoridad(&carpeta, &cfg.directorio);
    if let Some((pem, clave)) = guardado(&carpeta, &cfg.dominio) {
        match certificado_rustls(&pem, &clave) {
            Ok(c) => certs.poner_publico(c),
            Err(e) => eprintln!("Certificado público guardado de {} no válido ({e}): se pide otro.", cfg.dominio),
        }
    }
    tokio::spawn(async move {
        let mut tras_fallo = Duration::from_secs(60);
        loop {
            let actual = guardado(&carpeta, &cfg.dominio).map(|(p, _)| p);
            if !actual.as_deref().is_none_or(|p| toca_renovar(p, crate::almacen::ahora())) && certs.tiene_publico() {
                tokio::time::sleep(Duration::from_secs(12 * 3600)).await;
                continue;
            }
            println!("Certificado público: pidiéndolo para {} ({})…", cfg.dominio, cfg.directorio);
            let (c2, d2, r2) = (cfg.clone(), carpeta.clone(), retos.clone());
            let r = tokio::task::spawn_blocking(move || obtener(&c2, &d2, &r2)).await.unwrap_or_else(|e| Err(e.to_string()));
            let r = r.and_then(|(pem, clave)| {
                let listo = certificado_rustls(&pem, &clave)?;
                guardar(&carpeta, &cfg.dominio, &pem, &clave)?;
                Ok((listo, vigencia_pem(&pem)))
            });
            match r {
                Ok((listo, vigencia)) => {
                    certs.poner_publico(listo);
                    let hasta =
                        vigencia.and_then(|(_, h)| chrono::DateTime::from_timestamp(h, 0)).map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default();
                    println!("Certificado público de {} listo (vale hasta el {hasta}).", cfg.dominio);
                    tras_fallo = Duration::from_secs(60);
                }
                Err(e) => {
                    eprintln!(
                        "Certificado público de {}: no se pudo obtener ({e}). Comprueba que el nombre apunta a este servidor y que el puerto 80 está abierto. Otro intento en {} min.",
                        cfg.dominio,
                        tras_fallo.as_secs() / 60
                    );
                    tokio::time::sleep(tras_fallo).await;
                    tras_fallo = (tras_fallo * 4).min(Duration::from_secs(6 * 3600));
                }
            }
        }
    });
}

/// La petición de certificado: el nombre en el SAN y también como CN. Sin esto, rcgen pone
/// de CN «rcgen self signed cert» y Let's Encrypt la rechaza («rejectedIdentifier»).
fn csr_de(dominio: &str, clave: &rcgen::KeyPair) -> Result<rcgen::CertificateSigningRequest, String> {
    let mut p = rcgen::CertificateParams::new(vec![dominio.to_string()]).map_err(|e| e.to_string())?;
    let mut dn = rcgen::DistinguishedName::new();
    dn.push(rcgen::DnType::CommonName, dominio);
    p.distinguished_name = dn;
    p.serialize_request(clave).map_err(|e| e.to_string())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cada_autoridad_en_su_carpeta() {
        let base = Path::new("acme");
        assert_eq!(carpeta_de_autoridad(base, LETS_ENCRYPT), base);
        assert_eq!(carpeta_de_autoridad(base, LETS_ENCRYPT_PRUEBAS), base.join("pruebas"));
        let otra = carpeta_de_autoridad(base, "https://127.0.0.1:14000/dir");
        assert!(otra.starts_with(base) && otra != base && otra != base.join("pruebas"));
    }

    #[test]
    fn la_peticion_solo_lleva_el_dominio() {
        let k = rcgen::KeyPair::generate().unwrap();
        let der = csr_de("consola.ejemplo.com", &k).unwrap().der().to_vec();
        let texto = String::from_utf8_lossy(&der);
        assert!(texto.contains("consola.ejemplo.com"));
        assert!(!texto.contains("rcgen"), "la petición lleva el nombre por defecto de rcgen");
    }

    #[test]
    fn dominios_y_correos() {
        assert_eq!(valida_dominio(" Consola.Ejemplo.COM. ").unwrap(), "consola.ejemplo.com");
        assert!(valida_dominio("mi-oficina.duckdns.org").is_ok());
        for malo in ["localhost", "203.0.113.5", "-a.ejemplo.com", "a..b", "con espacio.com", "x.123", "a_b.ejemplo.com", ""] {
            assert!(valida_dominio(malo).is_err(), "{malo}");
        }
        assert!(valida_correo("ana@ejemplo.com").is_ok());
        assert!(valida_correo("ana").is_err());
        assert!(valida_correo("a@b.com>,x@y.com").is_err());
    }

    #[test]
    fn vigencia_de_un_certificado() {
        let clave = rcgen::KeyPair::generate().unwrap();
        let mut p = rcgen::CertificateParams::new(vec!["consola.ejemplo.com".into()]).unwrap();
        p.not_before = rcgen::date_time_ymd(2026, 1, 1);
        p.not_after = rcgen::date_time_ymd(2026, 4, 1);
        let cert = p.clone().self_signed(&clave).unwrap();
        let (desde, hasta) = vigencia_der(cert.der()).unwrap();
        assert_eq!(desde, chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp());
        assert_eq!(hasta - desde, 90 * 86_400);
        // Renovar con un tercio por delante: no a los 50 días, sí a los 61.
        assert!(!toca_renovar(&cert.pem(), desde + 50 * 86_400));
        assert!(toca_renovar(&cert.pem(), desde + 61 * 86_400));
        assert!(toca_renovar(&cert.pem(), hasta + 1));
        assert!(toca_renovar("no es un certificado", desde));
        // Fechas de después de 2049 van como GeneralizedTime.
        p.not_after = rcgen::date_time_ymd(2051, 6, 1);
        let (_, hasta) = vigencia_der(p.self_signed(&clave).unwrap().der()).unwrap();
        assert_eq!(hasta, chrono::NaiveDate::from_ymd_opt(2051, 6, 1).unwrap().and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp());
        assert!(vigencia_der(&[0x30, 0x81]).is_none());
    }

    #[test]
    fn jwk_y_su_huella() {
        let rng = SystemRandom::new();
        let doc = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).unwrap();
        let clave = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, doc.as_ref(), &rng).unwrap();
        let j: Value = serde_json::from_str(&jwk(&clave)).unwrap();
        assert_eq!(j["kty"], "EC");
        assert_eq!(B64U.decode(j["x"].as_str().unwrap()).unwrap().len(), 32);
        // RFC 7638: campos en orden y sin espacios.
        assert!(jwk(&clave).starts_with(r#"{"crv":"P-256","kty":"EC","x":""#));
        assert_eq!(B64U.decode(huella_jwk(&clave)).unwrap().len(), 32);
    }

    #[test]
    fn el_puerto_80_solo_responde_retos_y_redirige_al_dominio() {
        use tower::ServiceExt;
        let retos = Retos::default();
        retos.poner("abc", "abc.huella");
        let app = app_http(retos.clone(), "consola.ejemplo.com".into(), 443);
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        rt.block_on(async {
            let pedir = |ruta: &str, host: &str| {
                let req = axum::http::Request::builder().uri(ruta).header("host", host).body(axum::body::Body::empty()).unwrap();
                app.clone().oneshot(req)
            };
            let r = pedir("/.well-known/acme-challenge/abc", "consola.ejemplo.com").await.unwrap();
            assert_eq!(r.status(), 200);
            let cuerpo = axum::body::to_bytes(r.into_body(), 1024).await.unwrap();
            assert_eq!(&cuerpo[..], b"abc.huella");
            assert_eq!(pedir("/.well-known/acme-challenge/otro", "x").await.unwrap().status(), 404);
            // A HTTPS en el dominio, venga con el Host que venga.
            let r = pedir("/entrar?x=1", "malo.ejemplo.net").await.unwrap();
            assert_eq!(r.status(), 301);
            assert_eq!(r.headers()["location"], "https://consola.ejemplo.com/entrar?x=1");
        });
        let app = app_http(retos, "consola.ejemplo.com".into(), 8443);
        let r = rt.block_on(app.oneshot(axum::http::Request::builder().uri("/").body(axum::body::Body::empty()).unwrap())).unwrap();
        assert_eq!(r.headers()["location"], "https://consola.ejemplo.com:8443/");
    }

    #[test]
    fn el_dominio_lleva_el_publico_y_lo_demas_el_propio() {
        let hacer = |n: &str| {
            let k = rcgen::KeyPair::generate().unwrap();
            let c = rcgen::CertificateParams::new(vec![n.into()]).unwrap().self_signed(&k).unwrap();
            certificado_rustls(&c.pem(), &k.serialize_pem()).unwrap()
        };
        let (propio, publico) = (hacer("agentes.consola.ejemplo.com"), hacer("consola.ejemplo.com"));
        let certs = Certificados::nuevos("consola.ejemplo.com", propio.clone());
        // Sin el público todavía: el propio para todos.
        assert!(Arc::ptr_eq(&certs.para(Some("consola.ejemplo.com")), &propio));
        certs.poner_publico(publico.clone());
        assert!(Arc::ptr_eq(&certs.para(Some("Consola.Ejemplo.com")), &publico));
        assert!(Arc::ptr_eq(&certs.para(Some("agentes.consola.ejemplo.com")), &propio));
        assert!(Arc::ptr_eq(&certs.para(None), &propio));
    }
}
