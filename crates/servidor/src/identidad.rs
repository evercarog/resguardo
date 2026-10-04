//! Identidad del servidor (Ed25519, la que fijan los agentes) y TLS propio:
//! una autoridad autofirmada y un certificado del servidor firmado por ella
//! (docs/plataforma.md, §2.2). Todo en la carpeta de datos.

use base64::Engine;
use ed25519_dalek::SigningKey;
use std::path::Path;
use std::sync::Arc;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
pub const NOMBRE_CA: &str = "Resguardo Server";

/// Escribe un archivo privado (en Unix, solo para el usuario del servidor).
pub fn escribir_privado(path: &Path, contenido: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    let _ = std::fs::remove_file(&tmp);
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp).map_err(|e| e.to_string())?;
        f.write_all(contenido).map_err(|e| e.to_string())?;
    }
    #[cfg(not(unix))]
    std::fs::write(&tmp, contenido).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// La identidad del servidor: la crea la primera vez.
pub fn identidad(datos: &Path) -> Result<SigningKey, String> {
    let path = datos.join("identidad.key");
    if let Ok(t) = std::fs::read_to_string(&path) {
        let bytes: [u8; 32] = B64.decode(t.trim()).ok().and_then(|v| v.try_into().ok()).ok_or("identidad.key dañada.")?;
        return Ok(SigningKey::from_bytes(&bytes));
    }
    let mut semilla = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rng(), &mut semilla);
    let key = SigningKey::from_bytes(&semilla);
    escribir_privado(&path, B64.encode(key.to_bytes()).as_bytes())?;
    Ok(key)
}

pub fn publica(key: &SigningKey) -> String {
    B64.encode(key.verifying_key().to_bytes())
}

/// Las IP con que este equipo sale hacia cada red privada (sin enviar nada:
/// un socket UDP «conectado» solo elige la interfaz).
fn ips_locales() -> Vec<std::net::IpAddr> {
    let mut ips = Vec::new();
    for probe in ["10.255.255.255:1", "192.168.255.255:1", "172.31.255.255:1"] {
        if let Ok(sock) = std::net::UdpSocket::bind("0.0.0.0:0") {
            if sock.connect(probe).is_ok() {
                if let Ok(addr) = sock.local_addr() {
                    if !addr.ip().is_unspecified() && !ips.contains(&addr.ip()) {
                        ips.push(addr.ip());
                    }
                }
            }
        }
    }
    ips
}

/// La IP de la red local más probable (para decir dónde abrir la consola).
pub fn ip_principal() -> Option<std::net::IpAddr> {
    let ips = ips_locales();
    // La de una red privada, si la hay; si no, la primera.
    ips.iter().copied().find(|ip| matches!(ip, std::net::IpAddr::V4(v4) if v4.is_private())).or_else(|| ips.first().copied())
}

/// El nombre del equipo (en Linux, el servicio de systemd no tiene HOSTNAME en el entorno).
pub fn nombre_equipo() -> Option<String> {
    let n = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .ok()
        .or_else(|| std::fs::read_to_string("/proc/sys/kernel/hostname").ok())
        .map(|h| h.trim().to_lowercase())?;
    (!n.is_empty()).then_some(n)
}

/// Nombres que cubre el certificado: localhost, el nombre del equipo y sus IP, más los indicados.
pub fn nombres(extra: &[String]) -> Vec<String> {
    let mut n = vec!["localhost".to_string(), "127.0.0.1".to_string()];
    n.extend(nombre_equipo());
    n.extend(ips_locales().iter().map(|ip| ip.to_string()));
    n.extend(extra.iter().cloned());
    n.retain(|x| !x.is_empty() && !x.starts_with("0."));
    n.sort();
    n.dedup();
    n
}

/// Prepara (la primera vez) la autoridad y el certificado del servidor; devuelve la huella de la autoridad.
pub fn preparar_tls(datos: &Path, extra: &[String]) -> Result<String, String> {
    let dir = datos.join("tls");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let (ca, ca_key, huella) = (dir.join("ca.crt"), dir.join("ca.key"), dir.join("ca.huella"));
    if !ca.is_file() || !ca_key.is_file() {
        let (pem, key, fp) = resguardo_motor::tls::generar_ca(NOMBRE_CA)?;
        escribir_privado(&ca_key, key.as_bytes())?;
        std::fs::write(&ca, pem).map_err(|e| e.to_string())?;
        std::fs::write(&huella, &fp).map_err(|e| e.to_string())?;
    }
    // El del servidor, cada vez (por si cambió la IP); misma autoridad.
    let key_pem = std::fs::read_to_string(&ca_key).map_err(|e| e.to_string())?;
    let ca_pem = std::fs::read_to_string(&ca).map_err(|e| e.to_string())?;
    let (cert, key) = resguardo_motor::tls::emitir_certificado(&key_pem, NOMBRE_CA, &nombres(extra))?;
    escribir_privado(&dir.join("servidor.key"), key.as_bytes())?;
    std::fs::write(dir.join("servidor.crt"), format!("{cert}{ca_pem}")).map_err(|e| e.to_string())?;
    std::fs::read_to_string(&huella).map(|s| s.trim().to_string()).map_err(|e| e.to_string())
}

/// Configuración de rustls con el certificado del servidor (de la carpeta de datos o los indicados).
pub fn config_rustls(cert_pem: &Path, key_pem: &Path) -> Result<Arc<rustls::ServerConfig>, String> {
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};
    let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(cert_pem)
        .map_err(|e| format!("{}: {e}", cert_pem.display()))?
        .collect::<Result<_, _>>()
        .map_err(|e| format!("{}: {e}", cert_pem.display()))?;
    if certs.is_empty() {
        return Err(format!("{}: no hay certificados.", cert_pem.display()));
    }
    let key = PrivateKeyDer::from_pem_file(key_pem).map_err(|e| format!("{}: {e}", key_pem.display()))?;
    let mut cfg = rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| e.to_string())?;
    cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::new(cfg))
}
