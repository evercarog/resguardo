//! Certificados propios: una autoridad (lo que fijan los clientes) y un
//! certificado de servidor firmado por ella, que se puede volver a emitir
//! (p. ej. si cambia la IP) sin que los clientes dejen de confiar. Lo usan el
//! Servidor de copias (rest-server) y Resguardo Server.

use sha2::{Digest, Sha256};

fn ca_params(nombre: &str) -> Result<rcgen::CertificateParams, String> {
    let mut p = rcgen::CertificateParams::new(Vec::<String>::new()).map_err(|e| e.to_string())?;
    p.distinguished_name.push(rcgen::DnType::CommonName, nombre);
    p.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Constrained(0));
    p.key_usages = vec![rcgen::KeyUsagePurpose::KeyCertSign, rcgen::KeyUsagePurpose::CrlSign, rcgen::KeyUsagePurpose::DigitalSignature];
    p.not_before = rcgen::date_time_ymd(2024, 1, 1);
    p.not_after = rcgen::date_time_ymd(2046, 1, 1);
    Ok(p)
}

/// Autoridad propia llamada `nombre`: certificado (PEM), clave (PEM) y huella SHA-256.
pub fn generar_ca(nombre: &str) -> Result<(String, String, String), String> {
    let key = rcgen::KeyPair::generate().map_err(|e| e.to_string())?;
    let cert = ca_params(nombre)?.self_signed(&key).map_err(|e| e.to_string())?;
    Ok((cert.pem(), key.serialize_pem(), huella(cert.der())))
}

/// Certificado de servidor para estos nombres (DNS o IP), firmado por la
/// autoridad `nombre_ca` (su clave en PEM). Devuelve (certificado, clave) en PEM.
pub fn emitir_certificado(ca_key_pem: &str, nombre_ca: &str, nombres: &[String]) -> Result<(String, String), String> {
    let ca_key = rcgen::KeyPair::from_pem(ca_key_pem).map_err(|e| e.to_string())?;
    let ca = ca_params(nombre_ca)?.self_signed(&ca_key).map_err(|e| e.to_string())?;
    let mut p = rcgen::CertificateParams::new(nombres.to_vec()).map_err(|e| e.to_string())?;
    p.distinguished_name.push(rcgen::DnType::CommonName, format!("{nombre_ca} (servidor)"));
    p.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
    p.not_before = rcgen::date_time_ymd(2024, 1, 1);
    p.not_after = rcgen::date_time_ymd(2046, 1, 1);
    let key = rcgen::KeyPair::generate().map_err(|e| e.to_string())?;
    let cert = p.signed_by(&key, &ca, &ca_key).map_err(|e| e.to_string())?;
    Ok((cert.pem(), key.serialize_pem()))
}

/// Huella SHA-256 de un certificado en DER: `AB:CD:…`.
pub fn huella(der: &[u8]) -> String {
    Sha256::digest(der).iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autoridad_y_certificado() {
        let (ca, ca_key, fp) = generar_ca("Prueba").unwrap();
        assert!(ca.starts_with("-----BEGIN CERTIFICATE-----") && ca_key.contains("PRIVATE KEY"));
        assert_eq!(fp.split(':').count(), 32);
        let (leaf, key) = emitir_certificado(&ca_key, "Prueba", &["localhost".into(), "192.168.1.20".into()]).unwrap();
        assert!(leaf.starts_with("-----BEGIN CERTIFICATE-----") && key.contains("PRIVATE KEY"));
        if let Ok(dir) = std::env::var("RESGUARDO_CERT_OUT") {
            std::fs::write(format!("{dir}/ca.pem"), &ca).unwrap();
            std::fs::write(format!("{dir}/leaf.pem"), &leaf).unwrap();
        }
        // Se puede volver a emitir con la misma autoridad (otra IP).
        assert!(emitir_certificado(&ca_key, "Prueba", &["localhost".into(), "10.0.0.7".into()]).is_ok());
    }
}
