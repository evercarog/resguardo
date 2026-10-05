//! Agente v2: el equipo gestionado por Resguardo Server (docs/plataforma.md,
//! docs/api-servidor.md). Se vincula con un código y el SAS (v3), mantiene un
//! WebSocket con el servidor (o consulta cada poco si no pasa), y solo
//! obedece órdenes selladas para él que traigan su autorización: la prueba
//! de la clave de administración del cliente o la contraseña del
//! repositorio. Responde con resultados firmados.
//!
//! Convive con el agente de la fase 5 (`endpoint.rs`, web de Supabase): cada
//! equipo usa uno u otro hasta su migración.

use base64::Engine;
use ed25519_dalek::{Signer, SigningKey, Verifier};
use resguardo_protocolo::{claves, derivaciones, orden_v2, ordenes};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
const ARCHIVO: &str = "servidor.bin";
/// Intentos fallidos de una misma contraseña antes de bloquear ese tipo de orden.
const MAX_FALLOS: u32 = 5;

/// Fallos de autorización de una clave («admin» o «repo:<id>»).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Fallos {
    pub n: u32,
    pub desde: i64,
    pub bloqueado_hasta: i64,
    /// Rachas seguidas: el bloqueo se duplica en cada una (15 min, 30, 60… hasta 24 h).
    pub rachas: u32,
}

/// Lo que el equipo guarda de su vínculo con un servidor (cifrado con DPAPI
/// en la carpeta privada del agente).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Vinculo {
    pub url: String,
    /// Autoridad TLS del servidor, fijada al vincular.
    pub ca_pem: String,
    /// Identidad Ed25519 del servidor (base64), fijada al vincular.
    pub identidad: String,
    pub cliente_id: String,
    pub equipo_id: String,
    pub secreto: String,
    pub box_secret: String,
    /// Semilla Ed25519 del equipo (base64).
    pub sign_seed: String,
    pub sal_equipo: String,
    /// El código de emparejamiento, solo hasta recibir el `alta`.
    #[serde(default)]
    pub codigo: Option<String>,
    /// `SHA-256(prueba_e)` de la clave de administración (base64).
    #[serde(default)]
    pub verificador: Option<String>,
    #[serde(default)]
    pub k_cfg: Option<String>,
    #[serde(default = "espera_por_defecto")]
    pub espera_min_horas: i64,
    #[serde(default)]
    pub ultimo_seq: u64,
    /// Nonces ya vistos con su caducidad.
    #[serde(default)]
    pub nonces: Vec<(String, i64)>,
    /// "gestionado" o "local" (desvinculado, sigue funcionando solo).
    #[serde(default = "modo_gestionado")]
    pub modo: String,
    #[serde(default)]
    pub fallos: HashMap<String, Fallos>,
    /// Destinos con sus credenciales (`crear_repositorio`; solo en el equipo).
    #[serde(default)]
    pub destinos: Vec<crate::gestion_v2::Destino>,
    /// Repositorios que gestiona el servidor, con su contraseña (solo en el equipo).
    #[serde(default)]
    pub repos_v2: Vec<crate::gestion_v2::RepoV2>,
    /// La última configuración aplicada (`Configuracion` v1, sin secretos).
    #[serde(default)]
    pub config_v1: Option<Value>,
    #[serde(default)]
    pub config_seq: u64,
    /// Cambio de servidor en curso (F6).
    #[serde(default)]
    pub cambio: Option<crate::traslado_v2::Cambio>,
    /// Servidores de respaldo (hasta 3) y días sin respuesta del principal antes de usarlos.
    #[serde(default)]
    pub respaldo: Vec<crate::traslado_v2::Servidor>,
    #[serde(default)]
    pub respaldo_dias: i64,
    /// Última vez que el servidor principal respondió (Unix).
    #[serde(default)]
    pub ultimo_ok: i64,
    /// Vinculado con un código a otro servidor, ya con clave de administración:
    /// pendiente de que ese servidor dé el alta con la misma clave (§3.5, camino 3).
    #[serde(default)]
    pub adopcion: Option<crate::traslado_v2::Adopcion>,
    // ---- v1.35: varias consolas a la vez (consolas_v2.rs, docs/consolas-multiples.md) ----
    /// Id interno del vínculo de estos campos (vacío en un archivo anterior: «principal»).
    #[serde(default)]
    pub enlace_id: String,
    /// Cómo se llama la consola de estos campos (vacío: su dirección).
    #[serde(default)]
    pub nombre_consola: String,
    /// La sal del cliente en ese servidor, si se sabe.
    #[serde(default)]
    pub sal_cliente: Option<String>,
    /// Desde cuándo gestiona el equipo esta consola (Unix; 0 si no se sabe).
    #[serde(default)]
    pub desde: i64,
    /// Hay que subirle la configuración (la cambió otra consola).
    #[serde(default)]
    pub config_pendiente: bool,
    /// Las demás consolas que gestionan este equipo.
    #[serde(default)]
    pub otras: Vec<crate::consolas_v2::Enlace>,
    /// El último cambio de configuración y desde qué consola.
    #[serde(default)]
    pub ultimo_cambio: Option<crate::consolas_v2::UltimoCambio>,
    /// Cambios hechos en el propio equipo con la clave de administración (la
    /// ventana, ipc_local): el canal abierto los ve como «cambiado fuera».
    #[serde(default, skip_serializing_if = "es_cero")]
    pub cambio_local: u64,
}

fn es_cero(n: &u64) -> bool {
    *n == 0
}

fn espera_por_defecto() -> i64 {
    24
}
fn modo_gestionado() -> String {
    "gestionado".into()
}

fn ruta() -> std::path::PathBuf {
    crate::agent::private_dir().join(ARCHIVO)
}

pub fn cargar() -> Option<Vinculo> {
    let enc = std::fs::read(ruta()).ok()?;
    serde_json::from_slice(&crate::platform::unprotect(&enc).ok()?).ok()
}

/// Cuándo se escribió el vínculo por última vez (para ver si lo cambió otro proceso).
fn escrito() -> Option<std::time::SystemTime> {
    std::fs::metadata(ruta()).and_then(|m| m.modified()).ok()
}

/// ¿Cambió el vínculo, desde otro proceso, de una forma que el canal abierto
/// no puede seguir? Sobre todo `resguardo-agente vincular` con el servicio en
/// marcha (volver a vincular con otro servidor: queda pendiente del alta), pero
/// también `desvincular` o un vínculo nuevo. El canal tiene su copia en memoria:
/// si siguiera, nunca miraría el alta pendiente (eso lo hace el sondeo) y la
/// borraría al guardar su copia con el siguiente informe o la siguiente orden.
fn cambiado_fuera(canal: &Vinculo, disco: &Vinculo) -> bool {
    disco.url != canal.url
        || disco.equipo_id != canal.equipo_id
        || disco.secreto != canal.secreto
        || disco.modo != canal.modo
        || disco.cambio_local > canal.cambio_local
}

pub fn guardar(v: &Vinculo) -> Result<(), String> {
    crate::agent::prepare_dir()?;
    let enc = crate::platform::protect(&serde_json::to_vec(v).map_err(|e| e.to_string())?)?;
    let tmp = crate::agent::private_dir().join(format!("{ARCHIVO}.tmp"));
    let _ = std::fs::remove_file(&tmp);
    std::fs::write(&tmp, enc).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, ruta()).map_err(|e| e.to_string())
}

fn ahora() -> i64 {
    chrono::Utc::now().timestamp()
}

fn aleatorio<const N: usize>() -> [u8; N] {
    use crypto_box::aead::rand_core::RngCore;
    let mut b = [0u8; N];
    crypto_box::aead::OsRng.fill_bytes(&mut b);
    b
}

// ---------- HTTP con la autoridad fijada ----------

/// `http://` solo para el propio equipo (pruebas o un proxy local).
/// ¿`http://` a este mismo equipo? Solo con el anfitrión exacto: sin usuario
/// en la URL («http://localhost@otro/» va a «otro») y con IPv6 entre corchetes.
pub(crate) fn es_local(url: &str) -> bool {
    let Some(resto) = url.strip_prefix("http://") else { return false };
    let autoridad = resto.split(['/', '?', '#', '\\']).next().unwrap_or("");
    if autoridad.contains('@') {
        return false;
    }
    let host = match autoridad.strip_prefix('[') {
        Some(r) => r.split_once(']').filter(|(_, resto)| resto.is_empty() || resto.starts_with(':')).map(|(h, _)| h).unwrap_or(""),
        None => autoridad.split(':').next().unwrap_or(""),
    };
    let host = host.to_ascii_lowercase();
    host == "127.0.0.1" || host == "localhost" || host == "::1"
}

fn agente_http(url: &str, ca_pem: Option<&str>) -> Result<ureq::Agent, String> {
    use ureq::tls::{Certificate, RootCerts, TlsConfig, TlsProvider};
    let base = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(60))).http_status_as_error(false);
    if url.starts_with("http://") {
        if !es_local(url) {
            return Err("El servidor tiene que usar https:// (http:// solo en este mismo equipo).".into());
        }
        return Ok(base.build().new_agent());
    }
    let tls = match ca_pem {
        Some(pem) => {
            let cert = Certificate::from_pem(pem.as_bytes()).map_err(|e| format!("Autoridad del servidor no válida: {e}"))?.to_owned();
            TlsConfig::builder().provider(TlsProvider::Rustls).root_certs(RootCerts::Specific(Arc::new(vec![cert]))).build()
        }
        // Solo para descargar la autoridad al vincular (la autentica el SAS v3).
        None => TlsConfig::builder().provider(TlsProvider::Rustls).disable_verification(true).build(),
    };
    Ok(base.tls_config(tls).build().new_agent())
}

/// Sin respuesta en esa dirección y puerto.
pub const ERROR_SIN_RESPUESTA: &str = "No se pudo conectar con el servidor: no responde en esa dirección y puerto. Comprueba la dirección, \
                                       que Resguardo Server esté en marcha y que ningún cortafuegos bloquee el puerto.";
/// El certificado TLS no es el fijado al vincular.
pub const ERROR_CERTIFICADO: &str = "El certificado del servidor no es el que este equipo conoce: o el servidor se reinstaló, o hay algo \
                                     entre este equipo y el servidor. Comprueba la dirección; si el servidor es nuevo, vuelve a vincular el equipo.";

/// Por qué falló una llamada al servidor, en palabras: qué pasó y qué hacer.
/// El detalle técnico va al final, entre paréntesis, solo si no se reconoce.
pub fn explicar_error_conexion(e: &ureq::Error) -> String {
    use std::io::ErrorKind as K;
    use ureq::Error as E;
    let detalle = e.to_string();
    let t = detalle.to_lowercase();
    match e {
        E::HostNotFound => "No se encuentra el servidor con ese nombre. Revisa la dirección o usa su IP (p. ej. https://192.168.1.20:8443).".into(),
        E::Timeout(_) => "El servidor no respondió a tiempo. Comprueba que está encendido y que este equipo llega a él (misma red o VPN).".into(),
        E::ConnectionFailed => ERROR_SIN_RESPUESTA.into(),
        E::Io(io)
            if matches!(
                io.kind(),
                K::ConnectionRefused | K::ConnectionReset | K::ConnectionAborted | K::TimedOut | K::HostUnreachable | K::NetworkUnreachable | K::NotConnected
            ) =>
        {
            ERROR_SIN_RESPUESTA.into()
        }
        _ if t.contains("certificate") || t.contains("rustls") || t.contains("tls") || t.contains("handshake") => ERROR_CERTIFICADO.into(),
        _ => format!("No se pudo conectar con el servidor. Revisa la dirección y la conexión de red. ({detalle})"),
    }
}

/// Por qué el servidor rechazó la vinculación (`POST /api/agente/unirse`).
fn explicar_rechazo_vincular(estado: u16, resp: &Value) -> String {
    let mensaje = resp["mensaje"].as_str().unwrap_or("").trim();
    match estado {
        404 => "El código no es válido o ha caducado. Cada código sirve una sola vez y dura poco: crea uno nuevo en la consola \
                (botón «Añadir equipo») y vuelve a intentarlo."
            .into(),
        429 => "Demasiados intentos de vincular desde este equipo. Espera un rato (como mucho una hora) y vuelve a intentarlo.".into(),
        500.. => format!("El servidor tuvo un problema (código {estado}). Vuelve a intentarlo en unos minutos; si se repite, mira el registro del servidor."),
        _ if !mensaje.is_empty() => mensaje.to_string(),
        _ => format!("El servidor rechazó la vinculación (código {estado})."),
    }
}

/// Cliente HTTP con la autoridad TLS fijada del servidor.
pub fn agente_de(v: &Vinculo) -> Result<ureq::Agent, String> {
    agente_http(&v.url, Some(&v.ca_pem))
}

pub fn llamar(v: &Vinculo, metodo: &str, ruta: &str, cuerpo: Option<&Value>) -> Result<(u16, Value), String> {
    let agente = agente_de(v)?;
    let url = format!("{}{ruta}", v.url.trim_end_matches('/'));
    let auth = format!("Equipo {}:{}", v.equipo_id, v.secreto);
    let res = match (metodo, cuerpo) {
        ("GET", _) => agente.get(&url).header("authorization", &auth).call(),
        (_, Some(c)) => agente.post(&url).header("authorization", &auth).send_json(c),
        (_, None) => agente.post(&url).header("authorization", &auth).send_empty(),
    };
    let mut res = res.map_err(|e| explicar_error_conexion(&e))?;
    let estado = res.status().as_u16();
    let v: Value = res.body_mut().read_json().unwrap_or(Value::Null);
    Ok((estado, v))
}

pub fn llamar_ok(v: &Vinculo, ruta: &str, cuerpo: &Value) -> Result<Value, String> {
    let (estado, r) = llamar(v, "POST", ruta, Some(cuerpo))?;
    if estado >= 400 {
        return Err(format!("El servidor respondió {estado}: {}", r["mensaje"].as_str().unwrap_or("error")));
    }
    Ok(r)
}

/// Comprueba la firma de identidad del servidor sobre nuestro reto.
pub fn comprueba_identidad(v: &Vinculo, reto: &str, firma_b64: &str) -> Result<(), String> {
    let clave: [u8; 32] = B64.decode(&v.identidad).ok().and_then(|b| b.try_into().ok()).ok_or("Identidad del servidor dañada.")?;
    let firma: [u8; 64] = B64.decode(firma_b64).ok().and_then(|b| b.try_into().ok()).ok_or("Firma del servidor no válida.")?;
    ed25519_dalek::VerifyingKey::from_bytes(&clave)
        .map_err(|_| "Identidad del servidor dañada.".to_string())?
        .verify(derivaciones::texto_identidad_servidor(reto, &v.equipo_id).as_bytes(), &ed25519_dalek::Signature::from_bytes(&firma))
        .map_err(|_| "El servidor no demostró su identidad: no es el servidor con el que se vinculó este equipo.".to_string())
}

// ---------- Vincular ----------

/// Resultado de vincular: el código de comprobación y la huella de la autoridad del servidor.
pub struct Vinculado {
    pub sas: String,
    /// ¿El código incluye la huella de la autoridad TLS (v3)? Con un servidor anterior, no:
    /// hay que comprobar también la huella.
    pub sas_v3: bool,
    pub huella_ca: String,
    pub servidor: String,
    /// El equipo ya tenía clave de administración: conserva todo y espera el
    /// alta del servidor nuevo con esa misma clave.
    pub espera_alta: bool,
}

/// `resguardo-agente vincular CÓDIGO --servidor URL`: se une al servidor.
/// Si el equipo ya tenía clave de administración (en modo local o con otro
/// servidor, quizá perdido), conserva sus claves, su verificador y su
/// configuración, y el vínculo nuevo queda pendiente (`adopcion`) hasta que
/// ese servidor dé el alta con la misma clave: el mismo cliente lo adopta sin
/// reconfigurar, y un código solo no basta para quedarse con el equipo. Para
/// otro cliente, antes `empezar_de_cero`.
pub fn vincular(url: &str, codigo: &str, nombre: &str) -> Result<Vinculado, String> {
    vincular_con(url, codigo, nombre, &Esperado::default())
}

/// Lo que se sabe de antemano del servidor (v1.17: instalador «listo» o
/// `--huella-ca`): si no cuadra, no se vincula.
#[derive(Default)]
pub struct Esperado {
    /// Huella SHA-256 de su autoridad TLS (`AB:CD:…`).
    pub huella_ca: Option<String>,
    /// El cliente al que va el equipo.
    pub cliente: Option<String>,
}

/// ¿Es la huella la esperada? (sin distinguir mayúsculas; los `:` cuentan igual).
pub fn huella_coincide(esperada: &str, real: &str) -> bool {
    let norm = |s: &str| s.chars().filter(|c| c.is_ascii_hexdigit()).collect::<String>().to_ascii_uppercase();
    let e = norm(esperada);
    e.len() == 64 && e == norm(real)
}

/// La huella de la autoridad TLS que da el servidor, que tiene que ser UN solo
/// certificado: con varios, la huella (la del primero) no cubriría los demás,
/// y el canal (que confía en todos los del PEM) aceptaría otra autoridad.
pub fn huella_de_una_autoridad(pem: &str) -> Result<String, String> {
    use rustls::pki_types::{pem::PemObject, CertificateDer};
    let certs: Vec<CertificateDer> = CertificateDer::pem_slice_iter(pem.as_bytes()).collect::<Result<_, _>>().map_err(|_| "Autoridad TLS no válida.")?;
    match certs.as_slice() {
        [una] => Ok(resguardo_motor::tls::huella(una.as_ref())),
        [] => Err("Autoridad TLS no válida.".into()),
        _ => Err("El servidor dio más de una autoridad TLS: no se vincula (Resguardo Server da una sola).".into()),
    }
}

/// Con `http://` en este mismo equipo: la huella de la autoridad del servidor, solo para
/// el SAS v3 (no se fija nada). Vacía si no la da.
fn huella_local(url: &str) -> String {
    let Ok(agente) = agente_http(url, None) else { return String::new() };
    let Ok(mut r) = agente.get(&format!("{url}/api/servidor/ca")).call() else { return String::new() };
    if r.status().as_u16() != 200 {
        return String::new();
    }
    let pem = r.body_mut().read_to_string().unwrap_or_default();
    huella_de_una_autoridad(&pem).unwrap_or_default()
}

pub fn vincular_con(url: &str, codigo: &str, nombre: &str, esperado: &Esperado) -> Result<Vinculado, String> {
    crate::agent::require_admin()?;
    let url = url.trim().trim_end_matches('/').to_string();
    if !url.starts_with("https://") && !es_local(&url) {
        return Err("Escribe la dirección del servidor con https:// (p. ej. https://192.168.1.20:8443).".into());
    }
    // 1. La autoridad TLS (sin comprobar esta única vez: la autentica el SAS v3, que la incluye).
    let ca_pem = if url.starts_with("https://") {
        let mut r = agente_http(&url, None)?.get(&format!("{url}/api/servidor/ca")).call().map_err(|e| match e {
            // Aquí aún no se comprueba el certificado: un fallo de TLS es que no habla HTTPS.
            ureq::Error::Tls(_) | ureq::Error::Rustls(_) => {
                "En esa dirección no responde un servidor HTTPS. Revisa la dirección y el puerto (por defecto, 8443).".to_string()
            }
            e => explicar_error_conexion(&e),
        })?;
        let pem = r.body_mut().read_to_string().unwrap_or_default();
        if !pem.contains("BEGIN CERTIFICATE") {
            return Err("En esa dirección no hay un Resguardo Server (no devolvió su certificado). Revisa la dirección y el puerto (por defecto, 8443).".into());
        }
        pem
    } else {
        String::new()
    };
    let huella_ca = if ca_pem.is_empty() { "(sin TLS: solo en este equipo)".into() } else { huella_de_una_autoridad(&ca_pem)? };
    // La huella que entra en el SAS v3: la de la autoridad fijada. Sin TLS (http:// en este
    // mismo equipo, sin nadie en medio) se pide igual la del servidor para que el número
    // coincida con el de la consola; si no la da, vacía.
    let huella_sas = if ca_pem.is_empty() { huella_local(&url) } else { huella_ca.clone() };
    // Con la huella de antemano (instalador «listo» o --huella-ca), el certificado ya no se
    // acepta a ciegas: si no es esa autoridad, no se sigue (ni se gasta el código).
    if let Some(h) = &esperado.huella_ca {
        if !huella_coincide(h, &huella_ca) {
            return Err(format!(
                "La autoridad TLS de {url} no es la esperada (tiene {huella_ca}). No se ha vinculado: puede haber algo entre este equipo y el servidor, o la dirección no es la de tu Resguardo Server."
            ));
        }
    }
    // 2. Claves del equipo (se conservan si ya las tenía).
    let previo = cargar();
    let box_secret = previo.as_ref().map(|p| p.box_secret.clone()).unwrap_or_else(claves::new_key);
    let sign_seed = previo.as_ref().map(|p| p.sign_seed.clone()).unwrap_or_else(|| B64.encode(aleatorio::<32>()));
    let sal_equipo = previo.as_ref().map(|p| p.sal_equipo.clone()).unwrap_or_else(|| B64.encode(aleatorio::<16>()));
    let box_pub = claves::public_of(&box_secret)?;
    let sign_pub = B64.encode(firma_de(&sign_seed)?.verifying_key().to_bytes());
    // 3. Unirse.
    let agente = agente_http(&url, if ca_pem.is_empty() { None } else { Some(&ca_pem) })?;
    let cuerpo = json!({
        "codigo_hash": resguardo_protocolo::mensajes::code_hash(codigo),
        "nombre": nombre, "so": crate::web::os_label(), "version": crate::version_programa(),
        "box_pub": box_pub, "sign_pub": sign_pub, "sal_equipo": sal_equipo,
        // v1.26: el número de comprobación incluye la huella de la autoridad TLS.
        "sas_version": 3,
    });
    // Ahora sí con el certificado recién recibido: si falla, cambió entre una llamada y otra.
    let mut r = agente.post(&format!("{url}/api/agente/unirse")).send_json(&cuerpo).map_err(|e| {
        let m = explicar_error_conexion(&e);
        if m == ERROR_CERTIFICADO {
            "El certificado del servidor cambió en mitad de la vinculación: puede haber algo entre este equipo y el servidor. \
             Vuelve a intentarlo; si se repite, no sigas y compruébalo."
                .to_string()
        } else {
            m
        }
    })?;
    let estado = r.status().as_u16();
    let resp: Value = r.body_mut().read_json().unwrap_or(Value::Null);
    if estado >= 400 {
        return Err(explicar_rechazo_vincular(estado, &resp));
    }
    let campo = |k: &str| resp[k].as_str().map(str::to_string).ok_or_else(|| format!("El servidor no devolvió «{k}»."));
    let identidad = resp["servidor"]["identidad"].as_str().ok_or("El servidor no devolvió su identidad.")?.to_string();
    // Instalador listo (v1.17): el cliente tiene que ser el de la cola, también al adoptar.
    if let Some(c) = &esperado.cliente {
        if resp["cliente_id"].as_str() != Some(c.as_str()) {
            return Err("El servidor puso el equipo en otro cliente que el del instalador: no se ha vinculado. Avisa a quien administra la consola.".into());
        }
    }
    // v3 (con la huella de la autoridad fijada) si el servidor dice que lo usa (v1.26). Un
    // servidor anterior no lo dice: v2, y hay que comprobar la huella a mano (se avisa). Si
    // alguien en medio quitase el campo, la consola también avisa de comparar la huella.
    let sas_v3 = resp["sas_version"].as_i64() == Some(3);
    let sas = if sas_v3 { derivaciones::sas_v3(&identidad, &box_pub, &sign_pub, &huella_sas) } else { derivaciones::sas_v2(&identidad, &box_pub, &sign_pub) };
    // Ya tiene clave de administración: el servidor nuevo no se queda con él
    // solo por el código. Hasta que dé el alta con la MISMA clave, el equipo
    // sigue como estaba (configuración y servidor de antes) y el nuevo no recibe nada.
    if let Some(mut v) = previo.clone().filter(|p| p.verificador.is_some()) {
        v.adopcion = Some(crate::traslado_v2::Adopcion {
            url: url.clone(),
            ca_pem,
            identidad,
            cliente_id: campo("cliente_id")?,
            equipo_id: campo("equipo_id")?,
            secreto: campo("secreto")?,
            codigo: codigo.to_string(),
            hasta: ahora() + crate::traslado_v2::PLAZO_ADOPCION_S,
            ..Default::default()
        });
        guardar(&v)?;
        crate::agent::log(&format!(
            "Vinculado con Resguardo Server ({url}), pendiente del alta con la clave de administración de este equipo. Código de comprobación: {sas} (autoridad TLS {huella_ca})."
        ));
        return Ok(Vinculado { sas, sas_v3, huella_ca, servidor: url, espera_alta: true });
    }
    let mut v = previo.unwrap_or_default();
    v.url = url;
    v.ca_pem = ca_pem;
    v.identidad = identidad.clone();
    v.cliente_id = campo("cliente_id")?;
    v.equipo_id = campo("equipo_id")?;
    v.secreto = campo("secreto")?;
    v.box_secret = box_secret;
    v.sign_seed = sign_seed;
    v.sal_equipo = sal_equipo;
    v.codigo = Some(codigo.to_string());
    v.ultimo_seq = 0;
    v.nonces.clear();
    v.modo = "gestionado".into();
    guardar(&v)?;
    crate::agent::log(&format!("Vinculado con Resguardo Server ({}). Código de comprobación: {sas} (autoridad TLS {huella_ca}).", v.url));
    Ok(Vinculado { sas, sas_v3, huella_ca, servidor: v.url, espera_alta: false })
}

fn firma_de(seed_b64: &str) -> Result<SigningKey, String> {
    let s: [u8; 32] = B64.decode(seed_b64).ok().and_then(|b| b.try_into().ok()).ok_or("Clave del equipo dañada.")?;
    Ok(SigningKey::from_bytes(&s))
}

/// `vincular --empezar-de-cero`: olvida el servidor, la clave de administración
/// y lo que gestionaba (repositorios, destinos y copias programadas por él), para
/// vincularlo como un equipo nuevo, quizá de otro cliente. Lo ya copiado sigue
/// en sus destinos; el Servidor de copias, el espejo y las nubes no se tocan.
pub fn empezar_de_cero() -> Result<(), String> {
    crate::agent::require_admin()?;
    let Some(mut v) = cargar() else { return Ok(()) };
    crate::gestion_v2::dejar_todo(&mut v);
    std::fs::remove_file(ruta()).map_err(|e| format!("No se pudo borrar el vínculo anterior: {e}"))?;
    crate::agent::log("Empezar de cero: el equipo olvida su servidor, su clave de administración y las copias que este gestionaba.");
    Ok(())
}

/// Pasa a modo local: deja el servidor y sigue haciendo sus copias solo.
pub fn desvincular_local() -> Result<(), String> {
    crate::agent::require_admin()?;
    let mut v = cargar().ok_or("Este equipo no está vinculado a ningún servidor.")?;
    v.modo = "local".into();
    v.secreto.clear();
    // Todas las consolas: el equipo entero pasa a funcionar solo.
    v.otras.clear();
    guardar(&v)?;
    crate::agent::log("Desvinculado del servidor: el equipo sigue con sus copias en modo local.");
    Ok(())
}

// ---------- Órdenes ----------

pub struct Resultado {
    pub estado: &'static str,
    pub mensaje: String,
    /// JSON en texto, firmado con el resultado. En claro (lo lee el servidor,
    /// p. ej. `{"espera_min_horas": 12}`) o, si la orden trae `responder_a`,
    /// `{"sellado": "<sobre para responder_a>"}` (solo lo abre la consola).
    pub detalle: Option<String>,
}

fn rechazada(m: impl Into<String>) -> Resultado {
    Resultado { estado: "rechazada", mensaje: m.into(), detalle: None }
}
fn hecha(m: impl Into<String>) -> Resultado {
    Resultado { estado: "hecha", mensaje: m.into(), detalle: None }
}
fn fallida(m: impl Into<String>) -> Resultado {
    Resultado { estado: "fallida", mensaje: m.into(), detalle: None }
}
/// El estado de una orden para el registro del equipo («en marcha», no «en_marcha»).
fn estado_legible(e: &str) -> String {
    e.replace('_', " ")
}

fn en_marcha(m: impl Into<String>) -> Resultado {
    Resultado { estado: "en_marcha", mensaje: m.into(), detalle: None }
}

/// ¿Reduce la protección? Por el tipo o, en algunas, por su cuerpo (que el
/// servidor no ve: allí se fía del `not_before` que declara la consola, y
/// aquí se vuelve a comprobar).
fn destructiva(v: &Vinculo, o: &orden_v2::OrdenV2, tipo: &ordenes::Tipo) -> bool {
    let c = &o.cuerpo;
    tipo.destructiva
        || match o.tipo.as_str() {
            "desvincular" => c["modo"] == "dejar_de_copiar",
            "guarda_copias" => c["activo"] == false || c.get("quitar").is_some() || c.get("espejo").is_some_and(quita_destinos_del_espejo),
            "cambiar_espera" => c["horas"].as_i64().is_some_and(|h| h < v.espera_min_horas),
            "restaurar" => c["destino"] == "original" && c["reemplazar"] == true,
            // Igual que al ejecutarla: sin una hora en texto, se quita la copia externa.
            "cambiar_copia_externa" => !c["hora"].is_string(),
            // Desconectar una nube que usa el espejo deja de proteger fuera.
            "quitar_nube" => crate::nube::usa_espejo(c["nombre"].as_str().unwrap_or("").trim()),
            // Vaciar o desactivar todas las copias que había: el equipo deja de copiar solo.
            "config" => copias_activas(v.config_v1.as_ref()) > 0 && copias_activas(c.get("config")) == 0,
            // v1.22: poner o cambiar la retención del almacén (borrará versiones); quitarla, no.
            "retencion_almacen" => c["quitar"] != true,
            _ => false,
        }
}

/// Copias activas de una configuración (`activa` falta = activa).
fn copias_activas(config: Option<&Value>) -> usize {
    config.and_then(|c| c["copias"].as_array()).map_or(0, |l| l.iter().filter(|k| k["activa"] != false).count())
}

/// ¿El espejo pedido quita algún destino del actual (o el espejo entero)? Un
/// pedido que no se entiende cuenta como destructivo (y luego se rechaza).
fn quita_destinos_del_espejo(e: &Value) -> bool {
    match crate::espejo::pedido(e) {
        Ok(nuevo) => crate::server::load().espejo.is_some_and(|actual| actual.quita_destinos(nuevo.as_ref())) || nuevo.is_none(),
        Err(_) => true,
    }
}

/// ¿Bloqueada esta clave por intentos fallidos?
fn bloqueada(v: &Vinculo, clave: &str) -> Option<i64> {
    v.fallos.get(clave).map(|f| f.bloqueado_hasta).filter(|h| *h > ahora())
}

/// Anota un fallo; devuelve el aviso para el servidor si toca (y lo deja en la bitácora).
fn anotar_fallo(v: &mut Vinculo, clave: &str) -> Option<String> {
    let aviso = anotar_fallo_sin_bitacora(v, clave);
    if let Some(a) = &aviso {
        crate::bitacora::aviso("intentos_fallidos", a);
    }
    aviso
}

fn anotar_fallo_sin_bitacora(v: &mut Vinculo, clave: &str) -> Option<String> {
    let t = ahora();
    // Fuera los registros viejos (ni bloqueados ni con fallos recientes).
    v.fallos.retain(|_, f| f.bloqueado_hasta > t || t - f.desde <= 24 * 3600);
    let f = v.fallos.entry(clave.into()).or_default();
    if t - f.desde > 15 * 60 {
        f.n = 0;
        f.desde = t;
    }
    f.n += 1;
    if f.n >= MAX_FALLOS {
        f.rachas += 1;
        let minutos = (15i64 << (f.rachas - 1).min(7)).min(24 * 60);
        f.bloqueado_hasta = t + minutos * 60;
        f.n = 0;
        return Some(format!("{MAX_FALLOS} intentos fallidos con {}: bloqueado {minutos} min.", nombre_clave(clave)));
    }
    if f.n >= 3 {
        return Some(format!("{} intentos fallidos con {}.", f.n, nombre_clave(clave)));
    }
    None
}

fn nombre_clave(clave: &str) -> String {
    match clave.strip_prefix("repo:") {
        Some(r) => format!("la contraseña del repositorio «{r}»"),
        None => "la clave de administración".into(),
    }
}

fn acierto(v: &mut Vinculo, clave: &str) {
    if let Some(f) = v.fallos.get_mut(clave) {
        f.n = 0;
        f.rachas = 0;
    }
}

/// Comprueba la prueba de administración contra el verificador guardado.
fn admin_ok(v: &Vinculo, o: &orden_v2::OrdenV2) -> bool {
    let (Some(p), Some(ver)) = (o.autorizacion.prueba_admin.as_deref(), v.verificador.as_deref()) else { return false };
    match (B64.decode(p), B64.decode(ver)) {
        (Ok(p), Ok(ver)) => derivaciones::comprueba_prueba(&p, &ver),
        _ => false,
    }
}

/// Comprueba la contraseña del repositorio contra la que tiene el agente.
fn repo_existe(v: &Vinculo, repo: &str) -> bool {
    v.repos_v2.iter().any(|r| r.id == repo) || crate::agent::load_secrets().is_ok_and(|s| s.contains_key(repo))
}

fn repo_ok(v: &Vinculo, o: &orden_v2::OrdenV2) -> Option<String> {
    let c = o.autorizacion.clave_repo.as_ref()?;
    // La de los repositorios gestionados; si no, la del agente (copias anteriores al servidor).
    let guardada = match v.repos_v2.iter().find(|r| r.id == c.repo) {
        Some(r) => r.contrasena.clone(),
        None => crate::agent::load_secrets().ok()?.get(&c.repo)?.password.clone(),
    };
    let igual: bool = guardada.len() == c.contrasena.len() && guardada.bytes().zip(c.contrasena.bytes()).fold(0u8, |a, (x, y)| a | (x ^ y)) == 0;
    igual.then(|| c.repo.clone())
}

/// Abre, comprueba y ejecuta una orden. Devuelve el resultado y, si hay, un aviso para el servidor.
pub fn procesar(v: &mut Vinculo, meta: &Value) -> (Resultado, Option<String>) {
    let tipo_meta = meta["tipo"].as_str().unwrap_or("");
    let seq_meta = meta["seq"].as_u64().unwrap_or(0);
    let cx = orden_v2::Contexto { cliente: &v.cliente_id, equipo: &v.equipo_id, ultimo_seq: v.ultimo_seq, ahora: ahora(), tipo_meta, seq_meta };
    let o = match orden_v2::abrir(meta["sellado"].as_str().unwrap_or(""), &v.box_secret, &cx) {
        Ok(o) => o,
        Err(e) => return (rechazada(e), None),
    };
    // Repeticiones: nonce nuevo y seq creciente, anotados antes de ejecutar.
    let t = ahora();
    v.nonces.retain(|(_, exp)| *exp > t);
    if v.nonces.iter().any(|(n, _)| *n == o.nonce) {
        return (rechazada("Orden repetida."), None);
    }
    v.ultimo_seq = o.seq;
    v.nonces.push((o.nonce.clone(), t + ordenes::MAX_CADUCIDAD_DIAS * 86_400 + 600));
    let _ = guardar(v);

    let Some(tipo) = ordenes::tipo(&o.tipo) else { return (rechazada("Tipo de orden desconocido."), None) };
    // La espera mínima del cliente, también aquí (no solo en el servidor).
    if destructiva(v, &o, tipo) {
        let emitida = chrono::DateTime::parse_from_rfc3339(&o.emitida).map(|d| d.timestamp()).unwrap_or(t);
        let nb = o.not_before.as_deref().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|d| d.timestamp()).unwrap_or(0);
        if nb < emitida + ordenes::segundos_de_espera(v.espera_min_horas) - orden_v2::HOLGURA_S {
            return (rechazada(format!("Esta orden reduce la protección y tenía que esperar {} h.", v.espera_min_horas)), None);
        }
    }
    // Autorización según el tipo.
    let mut aviso = None;
    let necesita_admin = matches!(tipo.nivel, ordenes::Nivel::Administracion | ordenes::Nivel::RepositorioYAdministracion) && o.tipo != "alta";
    let necesita_repo = matches!(tipo.nivel, ordenes::Nivel::Repositorio | ordenes::Nivel::RepositorioYAdministracion);
    if necesita_admin {
        if let Some(h) = bloqueada(v, "admin") {
            return (rechazada(format!("Bloqueado por intentos fallidos hasta {}.", fecha(h))), None);
        }
        if !admin_ok(v, &o) {
            aviso = anotar_fallo(v, "admin");
            let _ = guardar(v);
            return (rechazada("La clave de administración no es correcta."), aviso);
        }
        acierto(v, "admin");
    }
    let repo_autorizado = if necesita_repo {
        let clave = format!("repo:{}", o.autorizacion.clave_repo.as_ref().map(|c| c.repo.as_str()).unwrap_or("?"));
        if let Some(h) = bloqueada(v, &clave) {
            return (rechazada(format!("Bloqueado por intentos fallidos hasta {}.", fecha(h))), None);
        }
        match repo_ok(v, &o) {
            Some(r) => {
                acierto(v, &clave);
                Some(r)
            }
            None => {
                // Solo cuentan los fallos de repositorios que existen: con ids
                // inventados no se llena el estado (ni se escribe en disco).
                if repo_existe(v, &clave["repo:".len()..]) {
                    aviso = anotar_fallo(v, &clave);
                    let _ = guardar(v);
                }
                return (rechazada("La contraseña del repositorio no es correcta."), aviso);
            }
        }
    } else {
        None
    };
    let orden_id = meta["id"].as_str().unwrap_or("").to_string();
    // v1.36: de qué consola viene el cambio (las demás enseñan «Cambiado desde otra
    // consola»). Antes de ejecutarla, para que ya vaya en el resumen que sube; si no se
    // hace, se deja como estaba.
    let antes = v.ultimo_cambio.clone();
    crate::consolas_v2::anotar_cambio(v, &o.tipo);
    let r = ejecutar(v, &o, repo_autorizado.as_deref(), &orden_id);
    if !matches!(r.estado, "hecha" | "en_marcha") {
        v.ultimo_cambio = antes;
    }
    let _ = guardar(v);
    (r, aviso)
}

/// Las órdenes largas (restaurar, descargar, aplicar la retención): se
/// contesta «en_marcha» y el resultado final llega después, firmado igual.
fn en_segundo_plano(
    v: &Vinculo,
    orden: &str,
    seq: u64,
    tipo: &'static str,
    aviso: &str,
    f: impl FnOnce(&Vinculo) -> Result<Resultado, String> + Send + 'static,
) -> Resultado {
    let (v, orden) = (v.clone(), orden.to_string());
    std::thread::spawn(move || {
        let r = match f(&v) {
            Ok(r) => r,
            Err(e) => fallida(e),
        };
        crate::agent::log(&format!("Orden «{tipo}» del servidor: {} ({}).", estado_legible(r.estado), r.mensaje));
        for intento in 0..5 {
            if enviar_resultado(&v, &orden, seq, &r).is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_secs(30 << intento));
        }
    });
    en_marcha(aviso)
}

fn fecha(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0).map(|d| d.with_timezone(&chrono::Local).format("%H:%M").to_string()).unwrap_or_default()
}

fn texto(c: &Value, k: &str) -> String {
    c[k].as_str().unwrap_or("").to_string()
}

/// Un repositorio gestionado por el servidor (la orden no puede tocar otros).
fn repo_gestionado(v: &Vinculo, id: &str) -> Result<(), String> {
    if v.repos_v2.iter().any(|r| r.id == id) {
        Ok(())
    } else {
        Err("Ese repositorio no lo gestiona este servidor.".into())
    }
}

fn ejecutar(v: &mut Vinculo, o: &orden_v2::OrdenV2, repo: Option<&str>, orden_id: &str) -> Resultado {
    use crate::gestion_v2 as g;
    let c = &o.cuerpo;
    let seq = o.seq;
    let r: Result<Resultado, String> = (|| match o.tipo.as_str() {
        "alta" => alta(v, o),
        "config" => g::aplicar_config(v, c).map(hecha),
        "crear_repositorio" => g::crear_repositorio(v, c).map(hecha),
        "cambiar_destino" => g::cambiar_destino(v, c).map(hecha),
        "cambiar_copia_externa" => g::cambiar_copia_externa(v, c, repo.unwrap_or("")).map(hecha),
        "cambiar_servidor" => crate::traslado_v2::cambiar_servidor(v, c, orden_id, seq).map(en_marcha),
        "servidores_respaldo" => crate::traslado_v2::servidores_respaldo(v, c).map(hecha),
        // El permiso de Dropbox llega sellado; el resultado nunca lo repite.
        // El resumen se sube al momento: si no, la nube no sale en el espejo hasta el próximo informe.
        "conectar_nube" => {
            let m = crate::nube::conectar_desde_orden(c)?;
            let _ = g::subir_config(v);
            Ok(hecha(m))
        }
        "quitar_nube" => {
            let m = crate::nube::quitar_desde_orden(c)?;
            let _ = g::subir_config(v);
            Ok(hecha(m))
        }
        "importar_repositorio" => crate::traslado_v2::importar_repositorio(v, c).map(hecha),
        // v1.14: venir de la app de escritorio (adoptar_v2.rs).
        "adoptar_repositorio" => {
            let (m, detalle) = crate::adoptar_v2::adoptar_repositorio(v, c, o.responder_a.is_some())?;
            Ok(Resultado { detalle: Some(detalle.to_string()), ..hecha(m) })
        }
        "copiar_historial" => crate::adoptar_v2::copiar_historial(v, c, orden_id, seq).map(en_marcha),
        "compartir_acceso" => {
            let detalle = crate::traslado_v2::compartir_acceso(v, c, repo.unwrap_or(""))?;
            Ok(Resultado { detalle: Some(detalle), ..hecha("Acceso sellado para el otro equipo.") })
        }
        "desbloquear" => g::desbloquear(v, c).map(hecha),
        "actualizar_agente" => Ok(rechazada("Próximamente: las actualizaciones firmadas llegarán con la llave de publicación del proyecto.")),
        "guarda_copias" => {
            let (m, privado) = g::guarda_copias(c, o.responder_a.is_some())?;
            let _ = g::subir_config(v);
            Ok(Resultado { detalle: privado.map(|p| p.to_string()), ..hecha(m) })
        }
        "copiar_ahora" => {
            let m = g::copiar_ahora(v, c)?;
            // Una vuelta del agente ahora mismo (no a los 5 minutos); si ya hay una en marcha, esta sale sola.
            if crate::agent::is_managed_agent() && !crate::agent::test_mode() {
                if let Ok(exe) = std::env::current_exe() {
                    let _ = std::process::Command::new(exe)
                        .arg("--agent-run")
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn();
                }
            }
            Ok(hecha(m))
        }
        "verificar_ahora" | "subir_ahora" | "probar_restauracion" => {
            let r = texto(c, "repo");
            repo_gestionado(v, &r)?;
            let tarea = match o.tipo.as_str() {
                "verificar_ahora" => "verify",
                "subir_ahora" => "offsite",
                _ => "restore_test",
            };
            crate::tasks::request_now(&r, tarea)?;
            Ok(hecha("Tarea pedida: empieza en unos segundos."))
        }
        "pausar" => g::pausar(v, c).map(hecha),
        "reanudar" => g::reanudar(v, c).map(hecha),
        "cambiar_retencion" => g::cambiar_retencion(v, c, repo.unwrap_or("")).map(hecha),
        "aplicar_retencion" => {
            let r = repo.unwrap_or("").to_string();
            Ok(en_segundo_plano(v, orden_id, seq, "aplicar_retencion", "Aplicando la retención…", move |v| g::aplicar_retencion(v, &r).map(hecha)))
        }
        // v1.22: retención en el almacén (retencion_almacen.rs).
        "clave_almacen" => crate::retencion_almacen::anadir_clave(v, repo.unwrap_or(""), c).map(hecha),
        "retencion_almacen" => {
            let m = crate::retencion_almacen::configurar(c)?;
            let _ = g::subir_config(v);
            Ok(hecha(m))
        }
        "aplicar_retencion_almacen" => {
            let (u, r) = crate::retencion_almacen::validar_aplicar(c)?;
            // El resumen (con el resultado) se sube con el próximo informe (subir_resumen_si_cambio).
            Ok(en_segundo_plano(v, orden_id, seq, "aplicar_retencion_almacen", "Aplicando la retención en el almacén…", move |_| {
                crate::retencion_almacen::aplicar(&u, &r).map(hecha)
            }))
        }
        "dejar_de_copiar" => g::dejar_de_copiar(v, repo.unwrap_or(""), false).map(hecha),
        "quitar_repositorio" => g::dejar_de_copiar(v, repo.unwrap_or(""), true).map(hecha),
        "cambiar_espera" => {
            let h = c["horas"].as_i64().unwrap_or(0);
            if !(1..=168).contains(&h) {
                return Err("La espera debe estar entre 1 y 168 horas.".into());
            }
            v.espera_min_horas = h;
            // El servidor solo cambia su espera con este dato firmado.
            Ok(Resultado {
                detalle: Some(json!({ "espera_min_horas": h }).to_string()),
                ..hecha(format!("Espera mínima de las órdenes que reducen la protección: {h} h."))
            })
        }
        "cambiar_clave_admin" => {
            let (ver, kcfg) = (texto(c, "verificador"), texto(c, "k_cfg"));
            if B64.decode(&ver).map(|b| b.len()) != Ok(32) || B64.decode(&kcfg).map(|b| b.len()) != Ok(32) {
                return Err("Verificador o K_cfg no válidos.".into());
            }
            v.verificador = Some(ver);
            let vieja = v.k_cfg.replace(kcfg.clone());
            // v1.36: la K_cfg nueva de las demás consolas (cada una con su sal).
            crate::consolas_v2::cambiar_kcfg_de_las_demas(v, vieja.as_deref(), &kcfg, c);
            // La configuración, cifrada ya con la clave nueva (y su etiqueta nueva).
            let _ = g::subir_config(v);
            Ok(hecha("Clave de administración cambiada."))
        }
        // v1.35: varias consolas a la vez (consolas_v2.rs).
        "anadir_consola" => crate::consolas_v2::anadir(v, c, orden_id, seq).map(en_marcha),
        "quitar_consola" => {
            let m = crate::consolas_v2::quitar(v, c)?;
            if v.secreto.is_empty() {
                // Se fue la propia consola («Dejar esta consola»): lo dice en claro, firmado, para
                // que ese servidor marque el equipo como que ya no está aquí (como `desvincular`).
                let siguen: Vec<String> = v.otras.iter().map(|e| crate::consolas_v2::nombre_de(&e.nombre, &e.url)).collect();
                return Ok(Resultado { detalle: Some(json!({ "deja_esta_consola": true, "siguen": siguen }).to_string()), ..hecha(m) });
            }
            let _ = g::subir_config(v);
            Ok(hecha(m))
        }
        "elegir_carpetas" => crate::sesiones_v2::abrir(v, c, crate::sesiones_v2::Tipo::Carpetas).map(hecha),
        "abrir_sesion" => crate::sesiones_v2::abrir(v, c, crate::sesiones_v2::Tipo::Basica).map(hecha),
        "explorar" => {
            let acc = g::acceso(v, repo.unwrap_or(""))?;
            crate::sesiones_v2::abrir(v, c, crate::sesiones_v2::Tipo::Explorar(Box::new(acc))).map(hecha)
        }
        "restaurar" => {
            let acc = g::acceso(v, repo.unwrap_or(""))?;
            let c = c.clone();
            Ok(en_segundo_plano(v, orden_id, seq, "restaurar", "Restaurando…", move |_| crate::sesiones_v2::restaurar(&acc, &c, None).map(hecha)))
        }
        "descargar" => {
            let acc = g::acceso(v, repo.unwrap_or(""))?;
            let c = c.clone();
            Ok(en_segundo_plano(v, orden_id, seq, "descargar", "Preparando la descarga…", move |v| {
                let n = crate::sesiones_v2::descargar(v, &acc, &c)?;
                Ok(Resultado { detalle: Some(json!({ "trozos": n }).to_string()), ..hecha("Descarga lista en el relé.") })
            }))
        }
        // v1.36: con otras consolas, solo se va esta (las demás siguen gestionándolo).
        "desvincular" => {
            let quedan = v.otras.len();
            if c["modo"] == "dejar_de_copiar" {
                g::dejar_todo(v);
            }
            // Solo pasa a local si era la única (si no, `limpiar` quita esta consola y sigue con las demás).
            if quedan == 0 {
                v.modo = "local".into();
            }
            v.secreto.clear();
            Ok(hecha(match (c["modo"] == "dejar_de_copiar", quedan) {
                (true, 0) => "Desvinculado y sin copias programadas.".to_string(),
                (false, 0) => "Desvinculado: el equipo sigue con sus copias en modo local.".to_string(),
                (true, n) => format!("Sin copias programadas. Esta consola deja de gestionar el equipo; lo siguen gestionando {}.", consolas_txt(n)),
                (false, n) => format!("Esta consola deja de gestionar el equipo; lo siguen gestionando {}.", consolas_txt(n)),
            }))
        }
        "baja_equipo" => {
            let quedan = v.otras.len();
            g::dejar_todo(v);
            v.repos_v2.clear();
            v.destinos.clear();
            if quedan == 0 {
                v.modo = "local".into();
            }
            v.secreto.clear();
            Ok(hecha(if quedan == 0 {
                "Equipo dado de baja: ya no copia y deja el servidor.".to_string()
            } else {
                format!("Equipo dado de baja: ya no copia y deja esta consola (sigue vinculado a {}).", consolas_txt(quedan))
            }))
        }
        otro => Ok(rechazada(format!("Este agente aún no admite «{otro}»."))),
    })();
    let r = match r {
        Ok(r) => r,
        Err(e) => fallida(e),
    };
    // Un detalle para `responder_a` (la consola): sellado, el servidor no lo lee.
    match (&o.responder_a, &r.detalle) {
        (Some(destino), Some(d)) if !o.tipo.eq("cambiar_espera") => match claves::seal_bytes(destino, d.as_bytes()) {
            Ok(s) => Resultado { detalle: Some(json!({ "sellado": s }).to_string()), ..r },
            Err(_) => Resultado { detalle: None, ..r },
        },
        _ => r,
    }
}

fn consolas_txt(n: usize) -> String {
    if n == 1 {
        "otra consola".into()
    } else {
        format!("otras {n} consolas")
    }
}

/// `alta`: la primera orden. Fija el verificador de la clave de
/// administración y `K_cfg`. La autentica `prueba_codigo` (solo quien vio el
/// código de emparejamiento puede calcularla; el servidor solo tiene su hash).
pub(crate) fn alta(v: &mut Vinculo, o: &orden_v2::OrdenV2) -> Result<Resultado, String> {
    let c = &o.cuerpo;
    let (ver, kcfg) = (texto(c, "verificador"), texto(c, "k_cfg"));
    if B64.decode(&ver).map(|b| b.len()) != Ok(32) || B64.decode(&kcfg).map(|b| b.len()) != Ok(32) {
        return Err("Verificador o K_cfg no válidos.".into());
    }
    // La prueba de administración que trae corresponde al verificador que fija.
    let prueba = o.autorizacion.prueba_admin.as_deref().and_then(|p| B64.decode(p).ok()).ok_or("Falta la prueba de administración.")?;
    if !derivaciones::comprueba_prueba(&prueba, &B64.decode(&ver).unwrap_or_default()) {
        return Ok(rechazada("La prueba de administración no corresponde al verificador."));
    }
    match (&v.verificador, &v.codigo) {
        // Ya dado de alta: solo se acepta la misma clave (al volver a vincular).
        (Some(actual), _) if *actual != ver => return Ok(rechazada("Este equipo ya tiene otra clave de administración.")),
        (Some(_), None) => {}
        (_, Some(codigo)) => {
            let esperada = orden_v2::prueba_codigo(codigo, &v.equipo_id, &ver);
            if o.autorizacion.prueba_codigo.as_deref() != Some(esperada.as_str()) {
                return Ok(rechazada("El alta no viene de quien tiene el código de emparejamiento."));
            }
        }
        (None, None) => return Ok(rechazada("No hay emparejamiento pendiente.")),
    }
    // Repetir el alta en un equipo ya dado de alta no cambia K_cfg ni baja la
    // espera (eso sería un cambiar_espera destructivo sin esperar): solo puede subirla.
    let ya_estaba = v.verificador.is_some() && v.k_cfg.is_some();
    v.verificador = Some(ver);
    if !ya_estaba {
        v.k_cfg = Some(kcfg);
    }
    if let Some(h) = c["espera_min_horas"].as_i64().filter(|h| (1..=168).contains(h)) {
        if !ya_estaba || h > v.espera_min_horas {
            v.espera_min_horas = h;
        }
    }
    v.codigo = None;
    Ok(hecha("Equipo dado de alta con la clave de administración del cliente."))
}

/// Un resultado que no sale de `procesar` (el final de un cambio de servidor).
pub fn enviar_resultado_texto(v: &Vinculo, orden: &str, seq: u64, estado: &'static str, mensaje: &str, detalle: Option<String>) -> Result<(), String> {
    enviar_resultado(v, orden, seq, &Resultado { estado, mensaje: mensaje.into(), detalle })
}

/// Firma y envía el resultado (por HTTP).
pub fn enviar_resultado(v: &Vinculo, orden: &str, seq: u64, r: &Resultado) -> Result<(), String> {
    llamar_ok(v, "/api/agente/resultado", &cuerpo_resultado(v, orden, seq, r)?)?;
    Ok(())
}

fn cuerpo_resultado(v: &Vinculo, orden: &str, seq: u64, r: &Resultado) -> Result<Value, String> {
    // El mensaje lo ve el servidor: sin rutas del equipo ni contraseñas (el detalle, si lo hay, va aparte).
    let mensaje = crate::web::public_message(&r.mensaje);
    let firma = firma_de(&v.sign_seed)?.sign(derivaciones::texto_resultado(orden, seq, r.estado, Some(&mensaje), r.detalle.as_deref()).as_bytes());
    Ok(json!({ "orden": orden, "estado": r.estado, "mensaje": mensaje, "detalle": r.detalle, "firma": B64.encode(firma.to_bytes()) }))
}

/// Informe para el servidor principal: estado del servicio y de las copias, sin rutas.
pub fn informe() -> Value {
    match cargar() {
        Some(v) => informe_de(&v.id_enlace()),
        None => crate::gestion_v2::informe(None),
    }
}

/// El informe para la consola `id` (con su `seq`) y, si cambió, su resumen al día.
pub fn informe_de(id: &str) -> Value {
    crate::consolas_v2::con_enlace(id, |v| {
        if v.modo == "gestionado" && !v.secreto.is_empty() {
            crate::gestion_v2::subir_resumen_si_cambio(v);
        }
        crate::gestion_v2::informe(Some(v))
    })
    .unwrap_or_else(|| crate::gestion_v2::informe(cargar().as_ref()))
}

/// Desde otro proceso (una copia que termina, la línea de órdenes): la
/// configuración y el resumen a la consola principal, y pendiente para las
/// demás (las sube el servicio). Solo se guarda lo que cambia (el número de la
/// configuración y lo pendiente), sobre lo que haya en el disco en ese momento:
/// mientras subía, el servicio pudo cambiar el vínculo (una orden, otra consola).
pub fn subir_config_desde_fuera() {
    let Some(mut v) = cargar().filter(|v| v.modo == "gestionado" && !v.secreto.is_empty()) else { return };
    if crate::gestion_v2::subir_config_enlace(&mut v).is_err() {
        return;
    }
    let (id, seq) = (v.id_enlace(), v.config_seq);
    if let Some(mut d) = cargar() {
        if d.id_enlace() == id {
            d.config_seq = d.config_seq.max(seq);
            for e in d.otras.iter_mut() {
                e.config_pendiente = true;
            }
            let _ = guardar(&d);
        }
    }
}

/// La configuración que cambió otra consola, a esta (cifrada con su `K_cfg`).
fn subir_pendiente(id: &str) {
    if !crate::consolas_v2::vista(id).is_some_and(|v| v.config_pendiente) {
        return;
    }
    crate::consolas_v2::con_enlace(id, |v| {
        if crate::gestion_v2::subir_config_enlace(v).is_ok() {
            v.config_pendiente = false;
        }
    });
}

/// Que esta consola respondió (último contacto), como mucho cada 10 min en disco.
fn anotar_contacto(id: &str) {
    let ahora = chrono::Utc::now().timestamp();
    if crate::consolas_v2::vista(id).is_some_and(|v| ahora - v.ultimo_ok > 600) {
        crate::consolas_v2::con_enlace(id, |v| v.ultimo_ok = ahora);
    }
}

/// Abre y ejecuta una orden de la consola `id`. Devuelve el resultado, el aviso
/// y las credenciales de esa consola de antes de la orden (desvincular las borra).
fn orden_de(id: &str, o: &Value) -> Option<(Resultado, Option<String>, Vinculo)> {
    crate::consolas_v2::con_enlace(id, |v| {
        let credenciales = v.clone();
        let (res, aviso) = procesar(v, o);
        // Cambió algo en el equipo: las demás consolas reciben la configuración enseguida.
        if res.estado == "hecha" && crate::consolas_v2::CAMBIAN_CONFIG.contains(&o["tipo"].as_str().unwrap_or("")) {
            for e in v.otras.iter_mut() {
                e.config_pendiente = true;
            }
        }
        (res, aviso, credenciales)
    })
}

// ---------- Sondeo ----------

/// Una vuelta de sondeo: recoge las órdenes, las ejecuta y envía los resultados.
/// Devuelve si el servidor pide atención (consultar más a menudo).
pub fn ronda() -> Result<bool, String> {
    // Un servidor nuevo pendiente de dar el alta (vincular con clave ya puesta).
    crate::traslado_v2::ronda_adopcion();
    // v1.36: una vuelta con cada consola; el resultado es el de la principal.
    let ids = cargar().map(|v| v.ids_enlaces()).unwrap_or_default();
    let mut principal = Ok(false);
    for (i, id) in ids.iter().enumerate() {
        let r = ronda_enlace(id);
        if i == 0 {
            principal = r;
        }
    }
    principal
}

/// Una vuelta de sondeo con la consola `id` y, si no responde, sus servidores de respaldo.
pub fn ronda_enlace(id: &str) -> Result<bool, String> {
    let r = ronda_de(id);
    if r.is_err() {
        // Esa consola no responde: ¿toca pasar a uno de sus servidores de respaldo?
        crate::consolas_v2::con_enlace(id, crate::traslado_v2::comprobar_respaldo);
    }
    r
}

/// El cuerpo de `tomar`: el reto y el último número de orden que aceptó de esta consola (como en el canal).
fn cuerpo_tomar(v: &Vinculo, reto: &str) -> Value {
    let mut c = json!({ "reto": reto });
    if v.ultimo_seq > 0 {
        c["ultimo_seq"] = json!(v.ultimo_seq);
    }
    c
}

fn ronda_de(id: &str) -> Result<bool, String> {
    let Some(v) = crate::consolas_v2::vista(id) else { return Ok(false) };
    let reto = B64.encode(aleatorio::<32>());
    let r = llamar_ok(&v, "/api/agente/tomar", &cuerpo_tomar(&v, &reto))?;
    comprueba_identidad(&v, &reto, r["firma"].as_str().unwrap_or(""))?;
    // Responde: se anota (para los servidores de respaldo y el «último contacto»).
    anotar_contacto(id);
    for o in r["ordenes"].as_array().cloned().unwrap_or_default() {
        // Con las credenciales de antes: una orden (desvincular) puede borrarlas.
        let Some((res, aviso, credenciales)) = orden_de(id, &o) else { break };
        crate::agent::log(&format!(
            "Orden «{}» de {}: {} ({}).",
            o["tipo"].as_str().unwrap_or("?"),
            crate::consolas_v2::nombre_de(&credenciales.nombre_consola, &credenciales.url),
            estado_legible(res.estado),
            res.mensaje
        ));
        if let Some(a) = aviso {
            let _ = llamar_ok(&credenciales, "/api/agente/aviso", &json!({ "tipo": "intentos_fallidos", "mensaje": a }));
        }
        let _ = enviar_resultado(&credenciales, o["id"].as_str().unwrap_or(""), o["seq"].as_u64().unwrap_or(0), &res);
        if crate::consolas_v2::vista(id).is_none_or(|w| cambiado_fuera(&v, &w)) {
            break; // se desvinculó (o cambió de servidor) con esta orden
        }
    }
    let Some(v) = crate::consolas_v2::vista(id) else { return Ok(false) };
    // v1.23: lo que falte del historial del equipo en este servidor.
    subir_bitacora(&v, crate::bitacora::ultima_del_servidor(&r));
    // v1.36: la configuración que cambió otra consola.
    subir_pendiente(id);
    // Un cambio de servidor en curso: se intenta en cada vuelta.
    if v.cambio.is_some() {
        crate::consolas_v2::con_enlace(id, crate::traslado_v2::intentar_cambio);
    }
    Ok(r["atencion"].as_bool().unwrap_or(false))
}

/// Sube lo que falte de la bitácora si el servidor la admite (`ultima`: lo que dice
/// su `hola` o `tomar`). Devuelve lo que tiene ahora el servidor. Sin prisa: si
/// falla, se reintenta en la próxima vuelta.
fn subir_bitacora(v: &Vinculo, ultima: Option<Option<String>>) -> Option<Option<String>> {
    let ultima = ultima?;
    match crate::bitacora::subir(v, ultima.as_deref()) {
        Ok(nueva) => Some(nueva),
        Err(e) => {
            // El mismo error, una vez (el sondeo lo intenta cada minuto).
            static ULTIMO: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
            let mut u = ULTIMO.lock().unwrap_or_else(|e| e.into_inner());
            if *u != e {
                crate::agent::log(&format!("Historial del equipo: no se pudo enviar al servidor ({e}); se reintenta más tarde."));
                *u = e;
            }
            Some(ultima)
        }
    }
}

/// Envía el informe a la consola `id` (por HTTP).
pub fn enviar_informe(id: &str) -> Result<(), String> {
    let Some(v) = crate::consolas_v2::vista(id) else { return Ok(()) };
    llamar_ok(&v, "/api/agente/informe", &json!({ "datos": informe_de(id) }))?;
    Ok(())
}

/// Sin canal: el progreso de lo que está en marcha, por HTTP (v1.25). Un
/// servidor anterior responde 404 y no se le vuelve a mandar.
fn enviar_progreso(id: &str, e: &mut crate::progreso_v2::Emisor) {
    let Some(v) = crate::consolas_v2::vista(id) else { return };
    let Some(tareas) = e.toca(crate::progreso_v2::CADA_SONDEO, std::time::Instant::now(), || crate::progreso_v2::tareas(Some(&v))) else { return };
    if let Ok((404 | 405, _)) = llamar(&v, "POST", "/api/agente/progreso", Some(&json!({ "tareas": tareas }))) {
        e.desactivar();
    }
}

// ---------- WebSocket ----------

fn config_rustls(ca_pem: &str) -> Result<Arc<rustls::ClientConfig>, String> {
    let mut raices = rustls::RootCertStore::empty();
    use rustls::pki_types::{pem::PemObject, CertificateDer};
    // Solo la primera, como en HTTPS (`agente_http`): la autoridad fijada es una.
    let c = CertificateDer::pem_slice_iter(ca_pem.as_bytes()).next().ok_or("Autoridad del servidor no válida.")?;
    raices.add(c.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let cfg = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_root_certificates(raices)
        .with_no_client_auth();
    Ok(Arc::new(cfg))
}

/// Cada cuánto se despierta el canal sin mensajes del servidor (para el progreso).
const ESPERA_CANAL: Duration = Duration::from_secs(5);
/// Entre dos informes por el canal, como poco (salvo el de cuando termina una copia).
const ENTRE_INFORMES: Duration = Duration::from_secs(15);
/// El de cuando termina una copia: casi enseguida (una copia termina como mucho cada pocos segundos).
const ENTRE_INFORMES_FIN_COPIA: Duration = Duration::from_secs(2);
/// Lo que se espera, como mucho, a releer las versiones al terminar una copia
/// antes de mandar el informe o el resumen (el canal no lee mientras tanto: muy
/// por debajo de los 45 s).
pub const RELEER_TRAS_COPIA: Duration = Duration::from_secs(20);
/// Sin nada del servidor en este tiempo (manda un ping cada 30 s), el canal se da por caído.
const SIN_RESPUESTA_CANAL: Duration = Duration::from_secs(45);

/// La espera de lectura del canal, puesta en el socket que lee de verdad (el que
/// lleva dentro el WebSocket). En un duplicado (`try_clone`) no vale: en Windows
/// cada descriptor duplicado tiene la suya, y el canal solo se despertaba con el
/// ping del servidor (cada 30 s), así que el progreso en vivo de una copia no
/// llegaba a la consola y el informe al terminar tardaba.
fn poner_espera(ws: &tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>, espera: Duration) -> std::io::Result<()> {
    use tungstenite::stream::MaybeTlsStream;
    #[allow(unreachable_patterns)]
    let tcp = match ws.get_ref() {
        MaybeTlsStream::Plain(s) => s,
        MaybeTlsStream::Rustls(s) => s.get_ref(),
        _ => return Err(std::io::Error::other("TLS no esperado")),
    };
    tcp.set_read_timeout(Some(espera))
}

/// El WebSocket con la consola principal (ver `canal_de`).
pub fn canal() -> Result<(), String> {
    let id = cargar().map(|v| v.id_enlace()).ok_or("Sin vínculo con un servidor.")?;
    canal_de(&id)
}

/// La dirección del canal con la consola de `v`. Lleva el último número de orden
/// que el equipo aceptó de ESA consola: el servidor se pone al día antes de
/// contarlo como conectado (tras restaurar la copia de la consola recuerda uno
/// anterior, y la primera orden que se firmara al verlo conectado saldría «repetida»).
fn url_canal(v: &Vinculo, reto_url: &str) -> String {
    let mut u = format!("{}/api/agente/canal?reto={reto_url}", v.url.replacen("https://", "wss://", 1).replacen("http://", "ws://", 1));
    if v.ultimo_seq > 0 {
        u.push_str(&format!("&ultimo_seq={}", v.ultimo_seq));
    }
    u
}

/// Mantiene el WebSocket con la consola `id` mientras funcione. Devuelve al
/// cortarse, o con `Ok` si esa consola ya no está (se quitó o se desvinculó)
/// o cambió de servidor (el hilo vuelve a abrirlo con lo nuevo).
pub fn canal_de(id: &str) -> Result<(), String> {
    use tungstenite::client::IntoClientRequest;
    use tungstenite::Message;
    let mut v = crate::consolas_v2::vista(id).ok_or("Sin vínculo con un servidor.")?;
    let quien = crate::consolas_v2::nombre_de(&v.nombre_consola, &v.url);
    let reto = B64.encode(aleatorio::<32>());
    let reto_url: String = reto.bytes().map(|b| if b.is_ascii_alphanumeric() { (b as char).to_string() } else { format!("%{b:02X}") }).collect();
    let ws_url = url_canal(&v, &reto_url);
    let mut req = ws_url.as_str().into_client_request().map_err(|e| e.to_string())?;
    req.headers_mut().insert("authorization", format!("Equipo {}:{}", v.equipo_id, v.secreto).parse().map_err(|_| "Cabecera no válida.")?);
    let host = req.uri().host().ok_or("Dirección sin servidor.")?.to_string();
    let port = req.uri().port_u16().unwrap_or(if ws_url.starts_with("wss://") { 443 } else { 80 });
    let tcp = std::net::TcpStream::connect((host.trim_start_matches('[').trim_end_matches(']'), port)).map_err(|e| format!("{ERROR_SIN_RESPUESTA} ({e})"))?;
    tcp.set_read_timeout(Some(Duration::from_secs(45))).map_err(|e| e.to_string())?;
    // Tras suspender el equipo, la conexión puede quedar medio muerta: que escribir tampoco se cuelgue.
    tcp.set_write_timeout(Some(Duration::from_secs(45))).map_err(|e| e.to_string())?;
    let conector =
        if ws_url.starts_with("wss://") { Some(tungstenite::Connector::Rustls(config_rustls(&v.ca_pem)?)) } else { Some(tungstenite::Connector::Plain) };
    let (mut ws, _) = tungstenite::client_tls_with_config(req, tcp, None, conector).map_err(|e| format!("No se pudo abrir el canal: {e}"))?;
    // Primero, el servidor demuestra quién es.
    let hola: Value = match ws.read().map_err(|e| e.to_string())? {
        Message::Text(t) => serde_json::from_str(&t).map_err(|_| "Saludo no válido.")?,
        _ => return Err("Saludo no válido.".into()),
    };
    comprueba_identidad(&v, &reto, hola["firma"].as_str().unwrap_or(""))?;
    crate::agent::log(&format!("Canal con Resguardo Server abierto ({quien})."));
    anotar_contacto(id);
    let _ = ws.send(Message::Text(json!({ "t": "informe", "datos": informe_de(id) }).to_string().into()));
    // v1.23: el servidor dice hasta dónde tiene el historial del equipo; si es una
    // consola nueva (nada), se sube todo lo de los últimos 90 días.
    let mut historial = subir_bitacora(&v, crate::bitacora::ultima_del_servidor(&hola));
    let mut ultimo_informe = std::time::Instant::now();
    let mut huella = crate::informe_v2::huella_informe();
    let mut vueltas = crate::informe_v2::huella_vueltas();
    // Se despierta cada pocos segundos (para el progreso de las copias) aunque el
    // servidor no diga nada; «dejó de responder» se cuenta aparte (45 s, con un ping cada 30 s).
    poner_espera(&ws, ESPERA_CANAL).map_err(|e| format!("No se pudo preparar el canal: {e}"))?;
    let mut ultimo_recibido = std::time::Instant::now();
    let mut progreso = crate::progreso_v2::Emisor::default();
    let mut visto = escrito();
    // ¿Sigue esta consola, con las mismas credenciales? Si no (otro proceso o una orden
    // la quitó, la desvinculó o la cambió de servidor), se cierra el canal.
    let sigue = |v: &mut Vinculo| -> bool {
        match crate::consolas_v2::vista(id) {
            Some(d) if !cambiado_fuera(v, &d) => {
                *v = d;
                true
            }
            _ => false,
        }
    };
    loop {
        let ahora_escrito = escrito();
        if ahora_escrito != visto {
            visto = ahora_escrito;
            if !sigue(&mut v) {
                crate::agent::log(&format!("El vínculo con {quien} cambió (quitado, desvinculado o en otro servidor): se cierra su canal."));
                let _ = ws.close(None);
                return Ok(());
            }
        }
        // v1.36: la configuración que cambió otra consola, a esta.
        if v.config_pendiente {
            subir_pendiente(id);
        }
        let leido = ws.read();
        if leido.is_ok() {
            ultimo_recibido = std::time::Instant::now();
        }
        match leido {
            Ok(Message::Text(t)) => {
                let m: Value = serde_json::from_str(&t).unwrap_or(Value::Null);
                match m["t"].as_str().unwrap_or("") {
                    "ping" => {
                        let _ = ws.send(Message::Text(r#"{"t":"pong"}"#.into()));
                    }
                    // v1.30: el almacén aplicó la retención en un repositorio de este equipo.
                    // Solo una pista: se releen sus versiones (nada más; ver `pista_refrescar`).
                    "refrescar" => {
                        if let Some(repo) = m["repo"].as_str() {
                            crate::informe_v2::pista_refrescar(&v, repo);
                        }
                    }
                    // Un servidor anterior no conoce `progreso`: no se le vuelve a mandar.
                    "error" if m["mensaje"].as_str().is_some_and(|x| x.contains("desconocido")) => progreso.desactivar(),
                    "orden" => {
                        let o = &m["orden"];
                        let Some((res, aviso, credenciales)) = orden_de(id, o) else {
                            let _ = ws.close(None);
                            return Ok(());
                        };
                        crate::agent::log(&format!(
                            "Orden «{}» de {quien}: {} ({}).",
                            o["tipo"].as_str().unwrap_or("?"),
                            estado_legible(res.estado),
                            res.mensaje
                        ));
                        if let Some(a) = aviso {
                            let _ = ws.send(Message::Text(json!({ "t": "aviso", "tipo": "intentos_fallidos", "mensaje": a }).to_string().into()));
                        }
                        let mut cuerpo = cuerpo_resultado(&credenciales, o["id"].as_str().unwrap_or(""), o["seq"].as_u64().unwrap_or(0), &res)?;
                        cuerpo["t"] = json!("resultado");
                        ws.send(Message::Text(cuerpo.to_string().into())).map_err(|e| e.to_string())?;
                        visto = escrito();
                        if !sigue(&mut v) {
                            let _ = ws.close(None);
                            return Ok(());
                        }
                        if v.cambio.is_some() {
                            crate::consolas_v2::con_enlace(id, crate::traslado_v2::intentar_cambio);
                            if !sigue(&mut v) {
                                let _ = ws.close(None);
                                return Ok(()); // ya en el servidor nuevo: el hilo abre su canal
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Message::Close(_)) => return Ok(()),
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                if ultimo_recibido.elapsed() >= SIN_RESPUESTA_CANAL {
                    return Err("El servidor dejó de responder.".into());
                }
            }
            Err(e) => return Err(format!("Canal cerrado: {e}")),
        }
        // Progreso de lo que está en marcha (v1.25): cada 5 s mientras dura y uno vacío al terminar.
        if let Some(tareas) = progreso.toca(crate::progreso_v2::CADA_CANAL, std::time::Instant::now(), || crate::progreso_v2::tareas(Some(&v))) {
            ws.send(Message::Text(json!({ "t": "progreso", "tareas": tareas }).to_string().into())).map_err(|e| format!("Canal cerrado: {e}"))?;
        }
        // Cada 5 min o en cuanto termine (o empiece) una copia, con 15 s entre uno y otro como
        // poco; v1.30: el de cuando termina una copia, sin esperar a esos 15 s (una copia corta
        // terminaba antes y su resultado llegaba a la consola hasta 15 s tarde).
        let termino = crate::informe_v2::huella_vueltas() != vueltas;
        let minimo = if termino { ENTRE_INFORMES_FIN_COPIA } else { ENTRE_INFORMES };
        let cambio = ultimo_informe.elapsed() > minimo && {
            let h = crate::informe_v2::huella_informe();
            h != std::mem::replace(&mut huella, h.clone())
        };
        if cambio || ultimo_informe.elapsed() > Duration::from_secs(300) {
            // v1.30: si acaba de terminar una copia, sus versiones van ya en este informe.
            if crate::informe_v2::refrescar_tras_copias(&v, RELEER_TRAS_COPIA) {
                huella = crate::informe_v2::huella_informe();
            }
            ultimo_informe = std::time::Instant::now();
            vueltas = crate::informe_v2::huella_vueltas();
            let _ = ws.send(Message::Text(json!({ "t": "informe", "datos": informe_de(id) }).to_string().into()));
            historial = subir_bitacora(&v, historial);
            anotar_contacto(id);
            // Mientras tanto, otro proceso pudo cambiar el vínculo (p. ej. `vincular` con otro
            // servidor justo al terminar una copia): se cierra el canal como arriba.
            visto = escrito();
            if !sigue(&mut v) {
                let _ = ws.close(None);
                return Ok(());
            }
            if v.cambio.is_some() {
                crate::consolas_v2::con_enlace(id, crate::traslado_v2::intentar_cambio);
                if !sigue(&mut v) {
                    let _ = ws.close(None);
                    return Ok(());
                }
            }
        }
    }
}

/// El hilo del servicio: WebSocket mientras se pueda; si no, sondeo (cada
/// 60 s, o cada 2 s si el servidor pide atención) y reintento del canal cada 5 min.
/// Para no repetir en el registro el mismo error una y otra vez: se anota si
/// es distinto del anterior o si ya pasó una hora.
#[derive(Default)]
struct AvisoRepetido {
    ultimo: Option<(String, std::time::Instant)>,
}

impl AvisoRepetido {
    const CADA: Duration = Duration::from_secs(3600);

    fn toca(&mut self, mensaje: &str, ahora: std::time::Instant) -> bool {
        let toca = match &self.ultimo {
            Some((m, cuando)) => m != mensaje || ahora.saturating_duration_since(*cuando) >= Self::CADA,
            None => true,
        };
        if toca {
            self.ultimo = Some((mensaje.to_string(), ahora));
        }
        toca
    }
}

/// Dónde deja Resguardo Server, en su misma máquina, los datos para vincular
/// el agente local (v1.19, «Vincular este servidor»).
pub fn archivo_vincular_local() -> std::path::PathBuf {
    if cfg!(windows) {
        std::path::Path::new(&crate::platform::program_data()).join("Resguardo Server").join("vincular-local.json")
    } else {
        std::path::PathBuf::from("/var/lib/resguardo-server/vincular-local.json")
    }
}

/// ¿Solo lo pueden haber escrito administradores (o root)? Nada de archivos
/// que un usuario cualquiera pueda dejar o cambiar.
fn protegido(p: &std::path::Path) -> bool {
    if crate::platform::is_reparse_point(p) {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let ok = |q: &std::path::Path| std::fs::metadata(q).is_ok_and(|m| m.mode() & 0o022 == 0);
        ok(p) && p.parent().is_some_and(ok)
    }
    #[cfg(windows)]
    {
        crate::platform::owned_by_admins(p)
    }
}

/// Los datos de `vincular-local.json`, si valen: los mismos que la cola de un
/// instalador listo, y solo hacia este mismo equipo (127.0.0.1, localhost o ::1).
pub fn leer_vincular_local(texto: &[u8]) -> Result<resguardo_protocolo::instalador::DatosInstalador, String> {
    let d: resguardo_protocolo::instalador::DatosInstalador = serde_json::from_slice(texto).map_err(|_| "Datos para vincular no válidos.".to_string())?;
    d.validar()?;
    let host = d.servidor.trim_start_matches("https://");
    if !["127.0.0.1:", "localhost:", "[::1]:"].iter().any(|h| host.starts_with(h)) {
        return Err("Solo se vincula solo con el servidor de este mismo equipo.".into());
    }
    Ok(d)
}

/// ¿Un vínculo que se quedó a medias? Unido a un servidor pero sin el alta (nunca recibió
/// la clave de administración), sin otras consolas ni un cambio de servidor pendiente.
/// No tiene nada que perder: un «Vincular este servidor» nuevo lo sustituye.
pub fn a_medias(v: &Vinculo) -> bool {
    v.verificador.is_none() && v.codigo.is_some() && v.otras.is_empty() && v.adopcion.is_none()
}

/// Sin vincular nunca (ni local ni gestionado), o con un vínculo a medias, y con el
/// archivo del servidor local: se vincula a él (el SAS y la clave de administración
/// siguen en la consola). Así, si el alta se quedó sin hacer (y la consola anuló ese
/// emparejamiento), volver a pulsar «Vincular este servidor» basta: no hay que borrar
/// nada a mano en el equipo.
fn vincular_local_si_toca() {
    if cargar().is_some_and(|v| !a_medias(&v)) {
        return;
    }
    let p = archivo_vincular_local();
    let Ok(texto) = std::fs::read(&p) else { return };
    if !protegido(&p) {
        crate::agent::log("vincular-local.json no está protegido (solo administradores): no se usa.");
        return;
    }
    let d = match leer_vincular_local(&texto) {
        Ok(d) => d,
        Err(e) => {
            crate::agent::log(&format!("vincular-local.json: {e}"));
            let _ = std::fs::remove_file(&p);
            return;
        }
    };
    let esperado = Esperado { huella_ca: Some(d.huella_ca.clone()), cliente: Some(d.cliente.clone()) };
    match vincular_con(&d.servidor, &d.codigo, &d.nombre, &esperado) {
        Ok(_) => {
            let _ = std::fs::remove_file(&p);
        }
        // Sin conexión (el servidor arranca): se reintenta en la siguiente vuelta; lo demás no se repite.
        Err(e) if e.contains("conectar") => crate::agent::log(&format!("Vincular con el servidor local: {e}")),
        Err(e) => {
            crate::agent::log(&format!("Vincular con el servidor local: {e}"));
            let _ = std::fs::remove_file(&p);
        }
    }
}

/// Antes de reabrir un canal que se cerró porque cambió el vínculo: 1 s la primera vez;
/// si se repite con canales cortos, el doble cada vez (hasta 60 s) con un poco de azar.
fn espera_reabrir(rapidas: u32, azar: u8) -> Duration {
    let base = 1u64 << rapidas.min(6);
    Duration::from_millis((base.min(60) * 1000) + u64::from(azar) * 4)
}

/// El servicio: un hilo por consola (`hilo_enlace`) y este, que vigila la lista
/// (arranca el de una consola nueva), el alta pendiente de un servidor nuevo y
/// «Vincular este servidor».
pub fn hilo() {
    std::thread::spawn(move || {
        let mut hilos: HashMap<String, Arc<std::sync::atomic::AtomicBool>> = HashMap::new();
        loop {
            // Pendiente del alta en un servidor nuevo: el alta llega por sondeo.
            if cargar().is_some_and(|v| v.adopcion.is_some()) {
                crate::traslado_v2::ronda_adopcion();
            }
            match cargar().filter(|v| v.modo == "gestionado" && !v.secreto.is_empty()) {
                None => vincular_local_si_toca(),
                Some(v) => {
                    if a_medias(&v) {
                        vincular_local_si_toca();
                    }
                    hilos.retain(|_, vivo| vivo.load(std::sync::atomic::Ordering::Relaxed));
                    for id in v.ids_enlaces() {
                        if !hilos.contains_key(&id) {
                            let vivo = Arc::new(std::sync::atomic::AtomicBool::new(true));
                            hilos.insert(id.clone(), vivo.clone());
                            std::thread::spawn(move || {
                                hilo_enlace(&id);
                                vivo.store(false, std::sync::atomic::Ordering::Relaxed);
                            });
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(10));
        }
    });
}

/// Una consola: WebSocket mientras se pueda; si no, sondeo (cada 60 s, o cada
/// 2 s si el servidor pide atención) y reintento del canal cada 5 min. Termina
/// cuando esa consola ya no está.
fn hilo_enlace(id: &str) {
    let mut aviso = AvisoRepetido::default();
    // Reaperturas seguidas «porque cambió el vínculo» con canales que duran nada: si algo
    // lo cambiara una y otra vez, cada apertura y cierre avisa a las consolas abiertas
    // (y vuelven a pedir). Se espera cada vez más (hasta 1 min), nunca en bucle.
    let mut rapidas = 0u32;
    while crate::consolas_v2::vista(id).is_some() {
        let inicio = std::time::Instant::now();
        let antes = crate::consolas_v2::vista(id);
        let resultado = canal_de(id);
        // Cerrado porque esa consola cambió (otro servidor, un alta pendiente que se hizo):
        // se abre enseguida el canal con lo nuevo, sin pasar antes por el sondeo.
        if resultado.is_ok() {
            if let (Some(a), Some(d)) = (antes.as_ref(), crate::consolas_v2::vista(id)) {
                if cambiado_fuera(a, &d) {
                    rapidas = if inicio.elapsed() < Duration::from_secs(10) { rapidas.saturating_add(1) } else { 0 };
                    std::thread::sleep(espera_reabrir(rapidas, aleatorio::<1>()[0]));
                    continue;
                }
            }
        }
        // Un canal que estuvo abierto y se cortó (p. ej. el servidor se actualizó) se reintenta enseguida.
        let estuvo_abierto = inicio.elapsed() >= Duration::from_secs(30);
        match resultado {
            // Con el servidor apagado, el mismo error cada 5 minutos llenaría el registro: una vez por hora basta.
            Err(e) if aviso.toca(&e, std::time::Instant::now()) => {
                let quien = crate::consolas_v2::vista(id).map(|v| crate::consolas_v2::nombre_de(&v.nombre_consola, &v.url)).unwrap_or_default();
                let punto = if e.ends_with('.') || e.ends_with(')') { "" } else { "." };
                crate::agent::log(&format!("Canal con {quien}: {e}{punto} Mientras tanto, el equipo consulta a esa consola cada minuto."))
            }
            Err(_) => {}
            Ok(()) => aviso = AvisoRepetido::default(),
        }
        let fin = std::time::Instant::now() + Duration::from_secs(if estuvo_abierto { 15 } else { 300 });
        let _ = enviar_informe(id);
        let mut progreso = crate::progreso_v2::Emisor::default();
        while std::time::Instant::now() < fin && crate::consolas_v2::vista(id).is_some() {
            let atencion = ronda_enlace(id).unwrap_or(false);
            let pausa = Duration::from_secs(if atencion { 2 } else { 60 });
            let hasta = (std::time::Instant::now() + pausa).min(fin);
            // Entre vuelta y vuelta, el progreso de lo que esté en marcha (cada 10 s).
            loop {
                enviar_progreso(id, &mut progreso);
                let falta = hasta.saturating_duration_since(std::time::Instant::now());
                if falta.is_zero() {
                    break;
                }
                std::thread::sleep(falta.min(crate::progreso_v2::CADA_SONDEO));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use resguardo_protocolo::orden_v2::{Autorizacion, OrdenV2};

    /// Solo un vínculo a medias (unido, sin el alta) se sustituye al volver a vincular este servidor.
    #[test]
    fn solo_se_sustituye_un_vinculo_a_medias() {
        let pendiente = Vinculo { modo: "gestionado".into(), secreto: "s".into(), codigo: Some("ABCD-EFGH-JK".into()), ..Default::default() };
        assert!(a_medias(&pendiente));
        let dado_de_alta = Vinculo { verificador: Some("v".into()), codigo: None, ..pendiente.clone() };
        assert!(!a_medias(&dado_de_alta), "con la clave de administración, nunca");
        let mut con_otra = pendiente.clone();
        con_otra.otras.push(crate::consolas_v2::Enlace { id: "b".into(), ..Default::default() });
        assert!(!a_medias(&con_otra), "con otras consolas, tampoco");
    }

    #[test]
    fn reabrir_el_canal_nunca_en_bucle() {
        assert!(espera_reabrir(0, 0) >= Duration::from_secs(1));
        assert!(espera_reabrir(3, 0) >= Duration::from_secs(8));
        assert!(espera_reabrir(30, 255) <= Duration::from_secs(62));
        assert!(espera_reabrir(30, 0) >= Duration::from_secs(60));
    }

    #[test]
    fn vincular_local_solo_con_este_equipo() {
        let datos = |servidor: &str| {
            json!({ "v": 1, "servidor": servidor, "huella_ca": vec!["AB"; 32].join(":"), "cliente": "c1", "nombre": "SERVIDOR", "codigo": "ABCD-EFGH-JK" })
                .to_string()
        };
        for bien in ["https://127.0.0.1:8443", "https://localhost:8443", "https://[::1]:8443"] {
            assert!(leer_vincular_local(datos(bien).as_bytes()).is_ok(), "{bien}");
        }
        for mal in ["https://192.168.1.20:8443", "https://127.0.0.1.evil.com:8443", "https://localhost.evil:1", "http://127.0.0.1:8443"] {
            assert!(leer_vincular_local(datos(mal).as_bytes()).is_err(), "{mal}");
        }
        assert!(leer_vincular_local(b"no es json").is_err());
    }

    /// Con la huella de antemano (instalador listo, --huella-ca), un PEM con la
    /// autoridad buena primero y otra detrás no pasa: el canal confiaría en las dos.
    #[test]
    fn una_sola_autoridad_al_vincular() {
        let (buena, _, huella) = resguardo_motor::tls::generar_ca("Resguardo Server").unwrap();
        let (otra, _, _) = resguardo_motor::tls::generar_ca("Intrusa").unwrap();
        assert_eq!(huella_de_una_autoridad(&buena).unwrap(), huella);
        assert!(huella_de_una_autoridad(&format!("{buena}\n{otra}")).unwrap_err().contains("más de una"));
        assert!(huella_de_una_autoridad("sin certificados").is_err());
        // El canal solo fija la primera, como HTTPS.
        assert!(config_rustls(&format!("{buena}\n{otra}")).is_ok());
    }

    #[test]
    fn huella_esperada_de_la_autoridad() {
        let h = vec!["AB"; 32].join(":");
        assert!(huella_coincide(&h, &h));
        assert!(huella_coincide(&h.to_lowercase(), &h), "sin distinguir mayúsculas");
        assert!(huella_coincide(&h.replace(':', ""), &h), "con o sin «:»");
        assert!(!huella_coincide(&vec!["AC"; 32].join(":"), &h));
        assert!(!huella_coincide("", ""), "vacía nunca vale");
        assert!(!huella_coincide("AB:CD", "AB:CD"), "corta nunca vale");
    }

    const CLAVE: &str = "caballo bateria grapa correcta";

    #[test]
    fn http_solo_a_este_equipo() {
        for bien in ["http://127.0.0.1:8443/", "http://localhost:8443", "http://LOCALHOST/", "http://[::1]:8443/api"] {
            assert!(es_local(bien), "{bien}");
        }
        for mal in [
            "http://localhost@evil.example/",
            "http://localhost:80@evil.example:8443/",
            "http://127.0.0.1.evil.example/",
            "http://evil.example/localhost",
            "https://localhost/",
            "http://[::1].evil/",
        ] {
            assert!(!es_local(mal), "{mal}");
        }
    }

    /// Cada consola recibe SU último número de orden (en el canal y en el sondeo), no el de la principal.
    #[test]
    fn cada_consola_recibe_su_ultimo_seq() {
        let mut v = Vinculo { url: "https://a:8443".into(), secreto: "s".into(), modo: "gestionado".into(), ultimo_seq: 13, ..Default::default() };
        v.otras.push(crate::consolas_v2::Enlace { id: "b".into(), url: "http://b:8080".into(), secreto: "t".into(), ultimo_seq: 4, ..Default::default() });
        v.otras.push(crate::consolas_v2::Enlace { id: "c".into(), url: "https://c".into(), secreto: "u".into(), ..Default::default() });
        let vistas = v.vistas();
        let urls: Vec<String> = vistas.iter().map(|w| url_canal(w, "R")).collect();
        assert_eq!(
            urls,
            ["wss://a:8443/api/agente/canal?reto=R&ultimo_seq=13", "ws://b:8080/api/agente/canal?reto=R&ultimo_seq=4", "wss://c/api/agente/canal?reto=R"]
        );
        let tomar: Vec<Value> = vistas.iter().map(|w| cuerpo_tomar(w, "R")).collect();
        assert_eq!(tomar, [json!({ "reto": "R", "ultimo_seq": 13 }), json!({ "reto": "R", "ultimo_seq": 4 }), json!({ "reto": "R" })]);
    }

    #[test]
    fn el_canal_ve_un_vinculo_nuevo_hecho_fuera() {
        let canal = Vinculo { url: "https://a:8443".into(), equipo_id: "e1".into(), secreto: "s".into(), modo: "gestionado".into(), ..Default::default() };
        let mut disco = canal.clone();
        disco.ultimo_seq = 7;
        disco.config_seq = 3;
        assert!(!cambiado_fuera(&canal, &disco), "lo que guarda el propio agente (órdenes, configuración) no cuenta");
        // v1.36: un alta pendiente (vincular con otro servidor) ya no cierra el canal: la
        // mira el hilo del servicio por su lado, y el canal nunca guarda una copia vieja
        // del vínculo (cada cambio se hace sobre lo que hay en el disco).
        disco.adopcion = Some(crate::traslado_v2::Adopcion { url: "https://b:8443".into(), ..Default::default() });
        assert!(!cambiado_fuera(&canal, &disco), "«vincular» con otro servidor deja un alta pendiente, y el canal sigue");
        let mut disco = canal.clone();
        disco.modo = "local".into();
        disco.secreto.clear();
        assert!(cambiado_fuera(&canal, &disco), "«desvincular» en el equipo");
        let mut disco = canal.clone();
        disco.url = "https://c:8443".into();
        disco.equipo_id = "e2".into();
        assert!(cambiado_fuera(&canal, &disco), "un vínculo nuevo");
        let mut disco = canal.clone();
        disco.cambio_local = 1;
        assert!(cambiado_fuera(&canal, &disco), "ajustes cambiados en el equipo (la ventana)");
    }

    #[test]
    fn el_canal_se_despierta_aunque_el_servidor_calle() {
        // Un servidor de WebSocket que acepta y no dice nada (como Resguardo Server entre pings).
        let escucha = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let dir = escucha.local_addr().unwrap();
        let servidor = std::thread::spawn(move || {
            let (tcp, _) = escucha.accept().unwrap();
            let _ws = tungstenite::accept(tcp).unwrap();
            std::thread::sleep(Duration::from_secs(8));
        });
        let tcp = std::net::TcpStream::connect(dir).unwrap();
        let (mut ws, _) = tungstenite::client(format!("ws://{dir}/"), tungstenite::stream::MaybeTlsStream::Plain(tcp)).unwrap();
        poner_espera(&ws, Duration::from_millis(300)).unwrap();
        let inicio = std::time::Instant::now();
        match ws.read() {
            Err(tungstenite::Error::Io(e)) => assert!(matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut), "{e}"),
            otro => panic!("se esperaba la espera agotada: {otro:?}"),
        }
        assert!(inicio.elapsed() < Duration::from_secs(3), "la lectura esperó {:?}: la espera no está en el socket del canal", inicio.elapsed());
        drop(ws);
        servidor.join().unwrap();
    }

    #[test]
    fn config_sin_copias_activas_es_destructiva() {
        let tipo = ordenes::tipo("config").expect("config es un tipo conocido");
        let orden = |cfg: Value| -> OrdenV2 {
            serde_json::from_value(json!({ "v": 2, "cliente": "c", "equipo": "e", "seq": 1, "nonce": "n", "emitida": "", "caduca": "", "tipo": "config", "cuerpo": { "config": cfg } })).unwrap()
        };
        let mut v = Vinculo { config_v1: Some(json!({ "v": 1, "copias": [{ "id": "a", "activa": true }, { "id": "b" }] })), ..Default::default() };
        assert!(destructiva(&v, &orden(json!({ "v": 1, "copias": [] })), tipo), "vaciarla");
        assert!(destructiva(&v, &orden(json!({ "v": 1, "copias": [{ "id": "a", "activa": false }] })), tipo), "desactivarlas todas");
        assert!(!destructiva(&v, &orden(json!({ "v": 1, "copias": [{ "id": "a", "activa": false }, { "id": "c" }] })), tipo), "queda una activa");
        v.config_v1 = Some(json!({ "v": 1, "copias": [] }));
        assert!(!destructiva(&v, &orden(json!({ "v": 1, "copias": [] })), tipo), "no había ninguna activa");
    }

    #[test]
    fn retencion_en_el_almacen_espera() {
        let orden = |tipo: &str, cuerpo: Value| -> OrdenV2 {
            serde_json::from_value(
                json!({ "v": 2, "cliente": "c", "equipo": "e", "seq": 1, "nonce": "n", "emitida": "", "caduca": "", "tipo": tipo, "cuerpo": cuerpo }),
            )
            .unwrap()
        };
        let v = Vinculo::default();
        let t = ordenes::tipo("retencion_almacen").unwrap();
        let regla = json!({ "usuario": "ana", "repo": "siigo", "retencion": { "diarias": 7 }, "horario": { "dias": [7], "hora": "03:00" } });
        assert!(destructiva(&v, &orden("retencion_almacen", regla), t), "poner o cambiar la regla borra versiones");
        assert!(!destructiva(&v, &orden("retencion_almacen", json!({ "usuario": "ana", "repo": "siigo", "quitar": true })), t), "quitarla no");
        let t = ordenes::tipo("aplicar_retencion_almacen").unwrap();
        assert!(destructiva(&v, &orden("aplicar_retencion_almacen", json!({ "usuario": "ana", "repo": "siigo" })), t));
        assert!(!ordenes::tipo("clave_almacen").unwrap().destructiva);
    }

    #[test]
    fn el_mismo_error_una_vez_por_hora() {
        let mut a = AvisoRepetido::default();
        let t = std::time::Instant::now();
        assert!(a.toca("El servidor dejó de responder.", t));
        assert!(!a.toca("El servidor dejó de responder.", t + Duration::from_secs(300)));
        assert!(a.toca("Canal cerrado.", t + Duration::from_secs(600)), "otro error sí se anota");
        assert!(!a.toca("Canal cerrado.", t + Duration::from_secs(1200)));
        assert!(a.toca("Canal cerrado.", t + Duration::from_secs(600 + 3600)), "y el mismo, pasada una hora");
    }

    #[test]
    fn explica_por_que_no_se_vincula() {
        let r = |estado, v: Value| explicar_rechazo_vincular(estado, &v);
        assert!(r(404, json!({ "codigo": "codigo", "mensaje": "Código no válido o caducado." })).contains("crea uno nuevo"));
        assert!(r(429, Value::Null).contains("Espera"));
        assert!(r(503, Value::Null).contains("código 503"));
        assert_eq!(r(422, json!({ "mensaje": "Falta el nombre del equipo." })), "Falta el nombre del equipo.");
        assert!(r(400, Value::Null).contains("código 400"));
        assert!(explicar_error_conexion(&ureq::Error::HostNotFound).contains("nombre"));
        assert_eq!(explicar_error_conexion(&ureq::Error::ConnectionFailed), ERROR_SIN_RESPUESTA);
        let rechazada = std::io::Error::from(std::io::ErrorKind::ConnectionRefused);
        assert_eq!(explicar_error_conexion(&ureq::Error::Io(rechazada)), ERROR_SIN_RESPUESTA);
    }

    /// La consola de prueba: llama a la API con su cookie y la cabecera CSRF.
    struct Consola {
        url: String,
        ca: String,
        cookie: String,
    }

    impl Consola {
        fn pedir(&self, metodo: &str, ruta: &str, cuerpo: Option<Value>) -> (u16, Value) {
            let agente = agente_http(&self.url, Some(&self.ca)).unwrap();
            let url = format!("{}{ruta}", self.url);
            let r = match (metodo, cuerpo) {
                ("GET", _) => agente.get(&url).header("cookie", &self.cookie).call(),
                (_, Some(c)) => agente.post(&url).header("cookie", &self.cookie).header("x-resguardo", "1").send_json(&c),
                (_, None) => agente.post(&url).header("cookie", &self.cookie).header("x-resguardo", "1").send_empty(),
            };
            let mut r = r.unwrap();
            (r.status().as_u16(), r.body_mut().read_json().unwrap_or(Value::Null))
        }
    }

    fn orden(cliente: &str, equipo: &str, seq: u64, tipo: &str, cuerpo: Value, autorizacion: Autorizacion) -> OrdenV2 {
        let ahora = chrono::Local::now();
        OrdenV2 {
            v: 2,
            cliente: cliente.into(),
            equipo: equipo.into(),
            seq,
            nonce: B64.encode(aleatorio::<16>()),
            emitida: ahora.to_rfc3339(),
            caduca: (ahora + chrono::Duration::hours(1)).to_rfc3339(),
            not_before: None,
            tipo: tipo.into(),
            cuerpo,
            autorizacion,
            responder_a: None,
        }
    }

    /// Vínculo, alta, órdenes con y sin autorización, bloqueo y desvincular,
    /// contra un Resguardo Server real con TLS en este mismo proceso.
    #[test]
    fn agente_v2_con_servidor_real() {
        let _real = crate::restic::tests::real_repo_lock();
        let dir_agente = std::env::temp_dir().join(format!("resguardo-v2-agente-{}", std::process::id()));
        let dir_srv = std::env::temp_dir().join(format!("resguardo-v2-srv-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir_agente);
        let _ = std::fs::remove_dir_all(&dir_srv);
        std::env::set_var("RESGUARDO_AGENT_DIR", &dir_agente);

        // El servidor, con su TLS propio, en un puerto libre.
        resguardo_servidor::identidad::preparar_tls(&dir_srv, &[]).unwrap();
        let st = resguardo_servidor::preparar(&dir_srv, resguardo_servidor::estado::Opciones::default()).unwrap();
        let codigo_arranque = resguardo_servidor::api::preparar_codigo_arranque(&st).unwrap().unwrap();
        let puerto = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let addr: std::net::SocketAddr = format!("127.0.0.1:{puerto}").parse().unwrap();
        let (st2, tls) = (st.clone(), dir_srv.join("tls"));
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let _ = rt.block_on(resguardo_servidor::servir(
                st2,
                addr,
                resguardo_servidor::Tls::Propio { cert: tls.join("servidor.crt"), clave: tls.join("servidor.key") },
            ));
        });
        std::thread::sleep(Duration::from_millis(500));
        let url = format!("https://127.0.0.1:{puerto}");
        let ca = std::fs::read_to_string(dir_srv.join("tls").join("ca.crt")).unwrap();

        // Propietaria con TOTP.
        let mut consola = Consola { url: url.clone(), ca, cookie: String::new() };
        let agente = agente_http(&url, Some(&consola.ca)).unwrap();
        let mut r = agente
            .post(&format!("{url}/api/inicio"))
            .header("x-resguardo", "1")
            .send_json(json!({ "codigo_arranque": codigo_arranque, "correo": "ana@ejemplo.com", "nombre": "Ana", "contrasena": "una contraseña bien larga" }))
            .unwrap();
        consola.cookie = r.headers().get("set-cookie").unwrap().to_str().unwrap().split(';').next().unwrap().to_string();
        let secreto: Value = r.body_mut().read_json().unwrap();
        let paso = chrono::Utc::now().timestamp() / 30;
        let codigo = format!(
            "{:06}",
            resguardo_servidor::auth::hotp(&resguardo_servidor::auth::de_base32(secreto["totp"]["secreto"].as_str().unwrap()).unwrap(), paso as u64)
        );
        let r = agente
            .post(&format!("{url}/api/sesion/totp"))
            .header("cookie", &consola.cookie)
            .header("x-resguardo", "1")
            .send_json(json!({ "codigo": codigo }))
            .unwrap();
        assert_eq!(r.status().as_u16(), 200);
        // La sesión completa lleva otra ficha.
        consola.cookie = r.headers().get("set-cookie").unwrap().to_str().unwrap().split(';').next().unwrap().to_string();

        // Cliente y emparejamiento.
        let (_, cliente) = consola.pedir("POST", "/api/clientes", Some(json!({ "nombre": "Prueba", "espera_min_horas": 1 })));
        let (c, sal_cliente) = (cliente["id"].as_str().unwrap().to_string(), cliente["sal_cliente"].as_str().unwrap().to_string());
        let (_, emp) = consola.pedir("POST", &format!("/api/clientes/{c}/emparejamientos"), None);
        let codigo_emp = emp["codigo"].as_str().unwrap().to_string();
        let emp_id = emp["id"].as_str().unwrap().to_string();

        // Un código equivocado: el motivo y qué hacer.
        let e = vincular(&url, "ZZZZ-ZZZZ-ZZ", "PRUEBA-V2").err().unwrap();
        assert!(e.contains("no es válido o ha caducado") && e.contains("Añadir equipo"), "{e}");
        // Una dirección donde no hay nadie escuchando.
        let e = vincular("https://127.0.0.1:1", &codigo_emp, "PRUEBA-V2").err().unwrap();
        assert_eq!(e, ERROR_SIN_RESPUESTA);

        // El agente se vincula (fija la autoridad TLS por primera vez).
        let vinc = vincular(&url, &codigo_emp, "PRUEBA-V2").unwrap();
        let (_, estado) = consola.pedir("GET", &format!("/api/clientes/{c}/emparejamientos/{emp_id}"), None);
        assert_eq!(estado["sas"].as_str().unwrap(), vinc.sas, "el mismo código de comprobación en los dos lados");
        let eq = estado["equipo"].clone();
        let (equipo, box_pub, sign_pub, sal_equipo) = (
            eq["id"].as_str().unwrap().to_string(),
            eq["box_pub"].as_str().unwrap().to_string(),
            eq["sign_pub"].as_str().unwrap().to_string(),
            eq["sal_equipo"].as_str().unwrap().to_string(),
        );
        // v1.26: SAS v3, con la huella de la autoridad TLS: la consola lo calcula con `huella_ca`.
        assert_eq!(estado["sas_version"], 3);
        assert!(vinc.sas_v3);
        let (_, srv) = consola.pedir("GET", "/api/servidor", None);
        let huella = srv["huella_ca"].as_str().unwrap();
        assert!(huella_coincide(huella, &vinc.huella_ca), "{huella} / {}", vinc.huella_ca);
        assert_eq!(vinc.sas, derivaciones::sas_v3(srv["identidad"].as_str().unwrap(), &box_pub, &sign_pub, huella));
        assert_ne!(vinc.sas, derivaciones::sas_v2(srv["identidad"].as_str().unwrap(), &box_pub, &sign_pub));

        // La consola deriva lo de la clave de administración y confirma.
        let kcfg = derivaciones::k_cfg(CLAVE, &sal_cliente).unwrap();
        let etiqueta = derivaciones::etiqueta_equipo(&kcfg, &equipo, &box_pub, &sign_pub);
        assert_eq!(consola.pedir("POST", &format!("/api/clientes/{c}/emparejamientos/{emp_id}/confirmar"), Some(json!({ "etiqueta": etiqueta }))).0, 200);
        let prueba = derivaciones::prueba_admin(CLAVE, &sal_equipo).unwrap();
        let verificador = B64.encode(derivaciones::verificador(&prueba));
        let enviar = |consola: &Consola, o: &OrdenV2| {
            let sobre = orden_v2::sellar(o, &box_pub).unwrap();
            consola.pedir(
                "POST",
                &format!("/api/clientes/{c}/equipos/{equipo}/ordenes"),
                Some(json!({ "tipo": o.tipo, "seq": o.seq, "sellado": sobre, "caduca": o.caduca, "not_before": o.not_before })),
            )
        };
        let estado_orden = |consola: &Consola, seq: u64| -> Value {
            let (_, l) = consola.pedir("GET", &format!("/api/clientes/{c}/equipos/{equipo}/ordenes"), None);
            l.as_array().unwrap().iter().find(|o| o["seq"] == seq).cloned().unwrap()
        };

        // Un alta sin la prueba del código (como haría un servidor malicioso): rechazada.
        let mut falsa = orden(
            &c,
            &equipo,
            1,
            "alta",
            json!({ "verificador": verificador, "k_cfg": B64.encode(kcfg) }),
            Autorizacion { prueba_admin: Some(B64.encode(prueba)), ..Default::default() },
        );
        falsa.autorizacion.prueba_codigo = Some("inventada".into());
        assert_eq!(enviar(&consola, &falsa).0, 200);
        ronda().unwrap();
        assert_eq!(estado_orden(&consola, 1)["estado"], "rechazada");

        // El alta buena.
        let alta = orden(
            &c,
            &equipo,
            2,
            "alta",
            json!({ "verificador": verificador, "k_cfg": B64.encode(kcfg), "espera_min_horas": 1 }),
            Autorizacion {
                prueba_admin: Some(B64.encode(prueba)),
                prueba_codigo: Some(orden_v2::prueba_codigo(&codigo_emp, &equipo, &verificador)),
                ..Default::default()
            },
        );
        assert_eq!(enviar(&consola, &alta).0, 200);
        ronda().unwrap();
        let hecha_alta = estado_orden(&consola, 2);
        assert_eq!(hecha_alta["estado"], "hecha", "{hecha_alta}");
        // Y la firma del resultado es del equipo.
        let firma: [u8; 64] = B64.decode(hecha_alta["firma_agente"].as_str().unwrap()).unwrap().try_into().unwrap();
        let clave_eq: [u8; 32] = B64.decode(&sign_pub).unwrap().try_into().unwrap();
        let vk = ed25519_dalek::VerifyingKey::from_bytes(&clave_eq).unwrap();
        let texto = derivaciones::texto_resultado(hecha_alta["id"].as_str().unwrap(), 2, "hecha", hecha_alta["mensaje"].as_str(), None);
        assert!(vk.verify(texto.as_bytes(), &ed25519_dalek::Signature::from_bytes(&firma)).is_ok());

        // Una orden de administración con la clave mal: rechazada; a la quinta, bloqueo y aviso.
        let mala = derivaciones::prueba_admin("otra clave cualquiera", &sal_equipo).unwrap();
        for seq in 3..=7 {
            let o =
                orden(&c, &equipo, seq, "cambiar_espera", json!({ "horas": 48 }), Autorizacion { prueba_admin: Some(B64.encode(mala)), ..Default::default() });
            enviar(&consola, &o);
            ronda().unwrap();
            assert_eq!(estado_orden(&consola, seq)["estado"], "rechazada");
        }
        let (_, avisos) = consola.pedir("GET", &format!("/api/clientes/{c}/avisos?abiertos=1"), None);
        assert!(
            avisos.as_array().unwrap().iter().any(|a| a["tipo"] == "intentos_fallidos" && a["mensaje"].as_str().unwrap().contains("bloqueado")),
            "{avisos}"
        );
        // Bloqueada: ni con la clave buena hasta que pase el tiempo.
        let buena =
            orden(&c, &equipo, 8, "cambiar_espera", json!({ "horas": 48 }), Autorizacion { prueba_admin: Some(B64.encode(prueba)), ..Default::default() });
        enviar(&consola, &buena);
        ronda().unwrap();
        assert!(estado_orden(&consola, 8)["mensaje"].as_str().unwrap().contains("Bloqueado"));
        let mut v = cargar().unwrap();
        v.fallos.clear();
        guardar(&v).unwrap();
        let buena =
            orden(&c, &equipo, 9, "cambiar_espera", json!({ "horas": 48 }), Autorizacion { prueba_admin: Some(B64.encode(prueba)), ..Default::default() });
        enviar(&consola, &buena);
        ronda().unwrap();
        assert_eq!(estado_orden(&consola, 9)["estado"], "hecha");
        assert_eq!(cargar().unwrap().espera_min_horas, 48);
        // El servidor toma la espera del resultado firmado (detalle).
        let (_, e) = consola.pedir("GET", &format!("/api/clientes/{c}/equipos/{equipo}"), None);
        assert_eq!(e["espera_min_horas"], 48, "{e}");

        // Una copia de un repositorio que no gestiona: fallida (no rechazada: está autorizada).
        let copia = orden(&c, &equipo, 10, "copiar_ahora", json!({ "repo": "otro", "copia": "x" }), Autorizacion::default());
        enviar(&consola, &copia);
        ronda().unwrap();
        assert_eq!(estado_orden(&consola, 10)["estado"], "fallida");

        // Bajar la espera sin esperar: el servidor no ve el cuerpo y la acepta; el agente, no.
        let admin = || Autorizacion { prueba_admin: Some(B64.encode(prueba)), ..Default::default() };
        let baja = orden(&c, &equipo, 11, "cambiar_espera", json!({ "horas": 2 }), admin());
        enviar(&consola, &baja);
        ronda().unwrap();
        assert_eq!(estado_orden(&consola, 11)["estado"], "rechazada");

        // Nubes desde la consola: el cuerpo exacto de «Conectar Dropbox», sellado.
        let rt = "rt-de-prueba-que-no-sale-del-equipo";
        let nube = |k: &str| json!({ "tipo": "dropbox", "nombre": "Dropbox Prueba", "refresh_token": rt, "access_token": "at-x", "expira": "2099-01-01T00:00:00.000Z", "app_key": k });
        // Sin la clave de administración: rechazada.
        enviar(&consola, &orden(&c, &equipo, 12, "conectar_nube", nube("abc123def456ghi"), Autorizacion::default()));
        ronda().unwrap();
        assert_eq!(estado_orden(&consola, 12)["estado"], "rechazada");
        // Con el marcador de la consola (app sin registrar): fallida, con el motivo.
        enviar(&consola, &orden(&c, &equipo, 13, "conectar_nube", nube(crate::nube::DROPBOX_APP_KEY_MARCADOR), admin()));
        ronda().unwrap();
        let r = estado_orden(&consola, 13);
        assert_eq!(r["estado"], "fallida");
        assert!(r["mensaje"].as_str().unwrap().contains("no está configurada"), "{r}");
        // Un equipo que no guarda copias no conecta nubes; el token no sale en el resultado.
        enviar(&consola, &orden(&c, &equipo, 14, "conectar_nube", nube("abc123def456ghi"), admin()));
        ronda().unwrap();
        let r = estado_orden(&consola, 14);
        assert_eq!(r["estado"], "fallida");
        assert!(r["mensaje"].as_str().unwrap().contains("no guarda copias") && !r.to_string().contains(rt), "{r}");
        // Quitar una que no hay: fallida (no la usa el espejo: sin espera).
        enviar(&consola, &orden(&c, &equipo, 15, "quitar_nube", json!({ "nombre": "Dropbox Prueba" }), admin()));
        ronda().unwrap();
        assert!(estado_orden(&consola, 15)["mensaje"].as_str().unwrap().contains("No hay ninguna nube"));
        let mut seq = 15;

        // Repositorio, configuración v1, sesión cifrada, descarga por el relé y restaurar
        // (con restic de verdad; sin él, se omite).
        if resguardo_motor::restic::version().is_ok() {
            use resguardo_protocolo::simetrico::{self, Lado};
            let base = std::env::temp_dir().join(format!("resguardo-v2-datos-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&base);
            let datos = base.join("datos");
            std::fs::create_dir_all(&datos).unwrap();
            std::fs::write(datos.join("hola.txt"), b"hola").unwrap();
            std::fs::write(datos.join("adios.txt"), b"adios").unwrap();
            let mut siguiente = || {
                seq += 1;
                seq
            };
            let s_crear = siguiente();
            let crear = orden(
                &c,
                &equipo,
                s_crear,
                "crear_repositorio",
                json!({ "id": "r1", "nombre": "Principal", "contrasena": "contraseña del repo",
                        "destino": { "id": "d1", "nombre": "Disco", "tipo": "local", "donde": base.join("destino").display().to_string() } }),
                admin(),
            );
            enviar(&consola, &crear);
            ronda().unwrap();
            assert_eq!(estado_orden(&consola, s_crear)["estado"], "hecha", "{}", estado_orden(&consola, s_crear));

            let cfg = json!({ "v": 1, "copias": [{ "id": "docs", "nombre": "Documentos", "repo": "r1", "carpetas": [datos.display().to_string()],
                "exclusiones": [], "horario": { "dias": [1, 2, 3, 4, 5], "horas": ["13:00"] }, "activa": true }] });
            let s_cfg = siguiente();
            enviar(&consola, &orden(&c, &equipo, s_cfg, "config", json!({ "config": cfg }), admin()));
            ronda().unwrap();
            assert_eq!(estado_orden(&consola, s_cfg)["estado"], "hecha", "{}", estado_orden(&consola, s_cfg));
            // La consola descifra la configuración con K_cfg: sin secretos dentro; el resumen, sin rutas.
            let (_, cf) = consola.pedir("GET", &format!("/api/clientes/{c}/equipos/{equipo}/config"), None);
            let plano =
                simetrico::descifrar_config(&kcfg, &equipo, cf["seq"].as_u64().unwrap(), &B64.decode(cf["cifrado"].as_str().unwrap()).unwrap()).unwrap();
            let doc: Value = serde_json::from_slice(&plano).unwrap();
            assert_eq!(doc["copias"][0]["id"], "docs");
            assert_eq!(doc["destinos"][0]["tipo"], "local");
            assert!(!String::from_utf8_lossy(&plano).contains("contraseña del repo"));
            assert_eq!(cf["resumen"]["copias"][0]["carpetas"], 1);
            assert!(!cf["resumen"].to_string().contains("hola") && !cf["resumen"].to_string().contains("datos"));

            // Una versión para explorar.
            let acc = crate::gestion_v2::acceso(&cargar().unwrap(), "r1").unwrap();
            resguardo_motor::restic::run(&acc, &["backup", &datos.display().to_string()]).unwrap();
            let version = resguardo_motor::restic::snapshots(&acc).unwrap()[0].id.clone();
            let ruta_datos = resguardo_motor::restic::ruta_en_version(&datos.display().to_string());

            // Sesión «explorar» (contraseña del repositorio), con la orden por la API de la consola.
            let (sesion, clave) = (uuid::Uuid::new_v4().to_string(), [9u8; 32]);
            let s_exp = siguiente();
            let exp = orden(
                &c,
                &equipo,
                s_exp,
                "explorar",
                json!({ "repo": "r1", "sesion": sesion, "clave_sesion": B64.encode(clave) }),
                Autorizacion { clave_repo: Some(orden_v2::ClaveRepo { repo: "r1".into(), contrasena: "contraseña del repo".into() }), ..Default::default() },
            );
            let sobre = orden_v2::sellar(&exp, &box_pub).unwrap();
            let (st_exp, r_exp) = consola.pedir(
                "POST",
                &format!("/api/clientes/{c}/equipos/{equipo}/ordenes"),
                Some(json!({ "tipo": "explorar", "seq": s_exp, "sellado": sobre, "caduca": exp.caduca, "sesion": sesion })),
            );
            assert_eq!(st_exp, 200, "{r_exp}");
            ronda().unwrap();
            let (kc, ke) = (simetrico::clave_direccion(&clave, &sesion, Lado::Consola), simetrico::clave_direccion(&clave, &sesion, Lado::Equipo));
            let mut desde = 0i64;
            let mut leer = |consola: &Consola| -> Vec<Value> {
                let (_, l) = consola.pedir("GET", &format!("/api/clientes/{c}/sesiones/{sesion}/mensajes?desde={desde}"), None);
                l.as_array()
                    .unwrap()
                    .iter()
                    .map(|m| {
                        desde = desde.max(m["n"].as_i64().unwrap());
                        serde_json::from_slice(&simetrico::descifrar_mensaje(&ke, &sesion, &B64.decode(m["cifrado"].as_str().unwrap()).unwrap()).unwrap())
                            .unwrap()
                    })
                    .collect()
            };
            let pedir_sesion = |consola: &Consola, m: Value| {
                let cif = simetrico::cifrar_mensaje(&kc, &sesion, m.to_string().as_bytes(), &simetrico::nonce_aleatorio());
                assert_eq!(consola.pedir("POST", &format!("/api/clientes/{c}/sesiones/{sesion}/mensajes"), Some(json!({ "cifrado": B64.encode(cif) }))).0, 200);
            };
            let mut esperar = |consola: &Consola, re: u64| -> Value {
                for _ in 0..20 {
                    if let Some(m) = leer(consola).into_iter().find(|m| m["re"] == re || (re == 0 && m["op"] == "lista")) {
                        return m;
                    }
                }
                panic!("sin respuesta a {re}");
            };
            assert_eq!(esperar(&consola, 0)["op"], "lista");
            pedir_sesion(&consola, json!({ "i": 1, "op": "versiones" }));
            assert_eq!(esperar(&consola, 1)["versiones"][0]["id"], version.as_str());
            pedir_sesion(&consola, json!({ "i": 2, "op": "listar", "version": version, "ruta": ruta_datos }));
            let l = esperar(&consola, 2);
            assert!(l["entradas"].as_array().unwrap().iter().any(|e| e["nombre"] == "hola.txt" && e["tipo"] == "archivo"), "{l}");
            pedir_sesion(&consola, json!({ "i": 3, "op": "carpetas", "ruta": "/" }));
            assert!(esperar(&consola, 3)["error"].is_string(), "explorar no deja ver las carpetas del equipo");
            // Una repetición (i que no crece) se descarta.
            pedir_sesion(&consola, json!({ "i": 3, "op": "versiones" }));
            pedir_sesion(&consola, json!({ "i": 4, "op": "cerrar" }));

            // Descargar un archivo por el relé.
            let (relevo, clave_relevo) = (uuid::Uuid::new_v4().to_string(), [10u8; 32]);
            let s_des = siguiente();
            let des = orden(
                &c,
                &equipo,
                s_des,
                "descargar",
                json!({ "repo": "r1", "version": version, "rutas": [format!("{ruta_datos}/hola.txt")], "formato": "archivo", "relevo": { "id": relevo, "clave": B64.encode(clave_relevo) } }),
                Autorizacion { clave_repo: Some(orden_v2::ClaveRepo { repo: "r1".into(), contrasena: "contraseña del repo".into() }), ..Default::default() },
            );
            let sobre = orden_v2::sellar(&des, &box_pub).unwrap();
            let (st_des, r_des) = consola.pedir(
                "POST",
                &format!("/api/clientes/{c}/equipos/{equipo}/ordenes"),
                Some(json!({ "tipo": "descargar", "seq": s_des, "sellado": sobre, "caduca": des.caduca, "sesion": uuid::Uuid::new_v4().to_string(), "relevo": { "id": relevo, "max_bytes": 1_000_000 } })),
            );
            assert_eq!(st_des, 200, "{r_des}");
            ronda().unwrap();
            let mut hecha = false;
            for _ in 0..100 {
                if estado_orden(&consola, s_des)["estado"] == "hecha" {
                    hecha = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            assert!(hecha, "{}", estado_orden(&consola, s_des));
            let agente_c = agente_http(&consola.url, Some(&consola.ca)).unwrap();
            let mut r = agente_c.get(&format!("{}/api/clientes/{c}/relevos/{relevo}/trozos/0", consola.url)).header("cookie", &consola.cookie).call().unwrap();
            let trozo = r.body_mut().read_to_vec().unwrap();
            assert_eq!(simetrico::descifrar_trozo(&clave_relevo, &relevo, 0, true, &trozo).unwrap(), b"hola");

            // Restaurar junto al original (nunca sobrescribe).
            let s_res = siguiente();
            let res = orden(
                &c,
                &equipo,
                s_res,
                "restaurar",
                json!({ "repo": "r1", "version": version, "rutas": [format!("{ruta_datos}/hola.txt")], "destino": "junto" }),
                Autorizacion { clave_repo: Some(orden_v2::ClaveRepo { repo: "r1".into(), contrasena: "contraseña del repo".into() }), ..Default::default() },
            );
            let sobre = orden_v2::sellar(&res, &box_pub).unwrap();
            consola.pedir(
                "POST",
                &format!("/api/clientes/{c}/equipos/{equipo}/ordenes"),
                Some(json!({ "tipo": "restaurar", "seq": s_res, "sellado": sobre, "caduca": res.caduca, "sesion": uuid::Uuid::new_v4().to_string() })),
            );
            ronda().unwrap();
            let mut restaurado = false;
            for _ in 0..100 {
                if estado_orden(&consola, s_res)["estado"] == "hecha" {
                    restaurado = std::fs::read_dir(&datos)
                        .unwrap()
                        .flatten()
                        .any(|e| e.file_name().to_string_lossy().starts_with("Restaurado") && e.path().join("hola.txt").is_file());
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            assert!(restaurado, "{}", estado_orden(&consola, s_res));

            // Varios archivos: un zip por el relé.
            let (relevo, clave_relevo) = (uuid::Uuid::new_v4().to_string(), [11u8; 32]);
            let s_zip = siguiente();
            let zip_o = orden(
                &c,
                &equipo,
                s_zip,
                "descargar",
                json!({ "repo": "r1", "version": version, "rutas": [format!("{ruta_datos}/hola.txt"), format!("{ruta_datos}/adios.txt")], "formato": "zip", "relevo": { "id": relevo, "clave": B64.encode(clave_relevo) } }),
                Autorizacion { clave_repo: Some(orden_v2::ClaveRepo { repo: "r1".into(), contrasena: "contraseña del repo".into() }), ..Default::default() },
            );
            let sobre = orden_v2::sellar(&zip_o, &box_pub).unwrap();
            let (st_zip, r_zip) = consola.pedir(
                "POST",
                &format!("/api/clientes/{c}/equipos/{equipo}/ordenes"),
                Some(json!({ "tipo": "descargar", "seq": s_zip, "sellado": sobre, "caduca": zip_o.caduca, "sesion": uuid::Uuid::new_v4().to_string(), "relevo": { "id": relevo, "max_bytes": 1_000_000 } })),
            );
            assert_eq!(st_zip, 200, "{r_zip}");
            ronda().unwrap();
            for _ in 0..100 {
                if estado_orden(&consola, s_zip)["estado"] == "hecha" {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            assert_eq!(estado_orden(&consola, s_zip)["estado"], "hecha", "{}", estado_orden(&consola, s_zip));
            let mut r = agente_c.get(&format!("{}/api/clientes/{c}/relevos/{relevo}/trozos/0", consola.url)).header("cookie", &consola.cookie).call().unwrap();
            let zip_bytes = simetrico::descifrar_trozo(&clave_relevo, &relevo, 0, true, &r.body_mut().read_to_vec().unwrap()).unwrap();
            let mut z = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).unwrap();
            let nombres: Vec<String> = z.file_names().map(str::to_string).collect();
            assert!(nombres.iter().any(|n| n.ends_with("hola.txt")) && nombres.iter().any(|n| n.ends_with("adios.txt")), "{nombres:?}");
            let mut contenido = String::new();
            let n_adios = nombres.iter().find(|n| n.ends_with("adios.txt")).unwrap().clone();
            std::io::Read::read_to_string(&mut z.by_name(&n_adios).unwrap(), &mut contenido).unwrap();
            assert_eq!(contenido, "adios");

            // Quitar bloqueos antiguos (inofensiva).
            let s_unlock = siguiente();
            enviar(&consola, &orden(&c, &equipo, s_unlock, "desbloquear", json!({}), Autorizacion::default()));
            ronda().unwrap();
            assert_eq!(estado_orden(&consola, s_unlock)["estado"], "hecha", "{}", estado_orden(&consola, s_unlock));

            // Cambiar el destino: si con los datos nuevos no se abre, no cambia nada.
            let s_mal = siguiente();
            enviar(
                &consola,
                &orden(&c, &equipo, s_mal, "cambiar_destino", json!({ "destino": "d1", "donde": base.join("otro").display().to_string() }), admin()),
            );
            ronda().unwrap();
            assert_eq!(estado_orden(&consola, s_mal)["estado"], "fallida");
            assert!(cargar().unwrap().destinos[0].donde.ends_with("destino"));
            let s_bien = siguiente();
            enviar(
                &consola,
                &orden(&c, &equipo, s_bien, "cambiar_destino", json!({ "destino": "d1", "donde": base.join("destino").display().to_string() }), admin()),
            );
            ronda().unwrap();
            assert_eq!(estado_orden(&consola, s_bien)["estado"], "hecha", "{}", estado_orden(&consola, s_bien));

            // Guarda copias: añadir un cliente exige responder_a (la contraseña solo la ve la consola).
            let s_gc = siguiente();
            enviar(&consola, &orden(&c, &equipo, s_gc, "guarda_copias", json!({ "anadir": "pc-recepcion" }), admin()));
            ronda().unwrap();
            assert!(estado_orden(&consola, s_gc)["mensaje"].as_str().unwrap().contains("responder_a"));
            let _ = std::fs::remove_dir_all(&base);
        }
        let (s_ws, s_fuera) = (seq + 1, seq + 2);

        // Por el WebSocket: la orden llega al momento (sin esperar al sondeo).
        let canal_hilo = std::thread::spawn(canal);
        let mut conectado = false;
        for _ in 0..50 {
            let (_, e) = consola.pedir("GET", &format!("/api/clientes/{c}/equipos/{equipo}"), None);
            if e["conectado"] == true {
                conectado = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(conectado, "el canal no se abrió");
        let copia = orden(&c, &equipo, s_ws, "copiar_ahora", json!({ "repo": "otro", "copia": "x" }), Autorizacion::default());
        enviar(&consola, &copia);
        let mut llego = false;
        for _ in 0..50 {
            if estado_orden(&consola, s_ws)["estado"] == "fallida" {
                llego = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(llego, "la orden no llegó por el WebSocket");

        // Desvincular (por el canal): modo local, el canal se cierra y no hay más contacto.
        let fuera = orden(
            &c,
            &equipo,
            s_fuera,
            "desvincular",
            json!({ "modo": "seguir_local" }),
            Autorizacion { prueba_admin: Some(B64.encode(prueba)), ..Default::default() },
        );
        enviar(&consola, &fuera);
        canal_hilo.join().unwrap().unwrap();
        // El servidor lo procesa en segundo plano: se espera un poco.
        let mut hecha = false;
        for _ in 0..50 {
            if estado_orden(&consola, s_fuera)["estado"] == "hecha" {
                hecha = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(hecha, "el resultado de desvincular no llegó");
        let (_, e) = consola.pedir("GET", &format!("/api/clientes/{c}/equipos/{equipo}"), None);
        assert_eq!(e["modo"], "local");
        assert_eq!(cargar().unwrap().modo, "local");
        assert!(!ronda().unwrap());

        std::env::remove_var("RESGUARDO_AGENT_DIR");
        let _ = std::fs::remove_dir_all(dir_agente);
        let _ = std::fs::remove_dir_all(dir_srv);
    }
}
