//! Claves X25519 de los equipos y sobres sellados (el «sealed box» de
//! libsodium: X25519 + XSalsa20-Poly1305, con una clave efímera por sobre).
//! Fases 3 y 5 (docs/compartir.md, docs/agente-gestionado.md).
//!
//! Todo se intercambia en base64 estándar.

use base64::Engine;
use crypto_box::aead::OsRng;
use crypto_box::{PublicKey, SecretKey};

pub(crate) const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// Clave privada X25519 nueva (base64).
pub fn new_key() -> String {
    B64.encode(SecretKey::generate(&mut OsRng).to_bytes())
}

fn secret_of(b64: &str) -> Result<SecretKey, String> {
    let bytes: [u8; 32] = B64.decode(b64).ok().and_then(|v| v.try_into().ok()).ok_or("Clave del equipo dañada.")?;
    Ok(SecretKey::from(bytes))
}

/// Clave pública (base64) de una privada (base64).
pub fn public_of(secret_b64: &str) -> Result<String, String> {
    Ok(B64.encode(secret_of(secret_b64)?.public_key().as_bytes()))
}

/// Sella `plain` para la clave pública `recipient_pub_b64`. Devuelve el sobre en base64.
pub fn seal_bytes(recipient_pub_b64: &str, plain: &[u8]) -> Result<String, String> {
    let bytes: [u8; 32] = B64.decode(recipient_pub_b64).ok().and_then(|v| v.try_into().ok()).ok_or("Clave pública del equipo no válida.")?;
    let sealed = PublicKey::from(bytes).seal(&mut OsRng, plain).map_err(|_| "No se pudo cifrar.".to_string())?;
    Ok(B64.encode(sealed))
}

/// Abre un sobre (base64) con la clave privada (base64).
pub fn open_bytes(secret_b64: &str, ciphertext_b64: &str) -> Result<Vec<u8>, String> {
    let sealed = B64.decode(ciphertext_b64).map_err(|_| "Sobre dañado.".to_string())?;
    secret_of(secret_b64)?.unseal(&sealed).map_err(|_| "No se pudo descifrar: no es para este equipo.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sella_y_abre_solo_su_destinatario() {
        let (a, b) = (new_key(), new_key());
        let ct = seal_bytes(&public_of(&b).unwrap(), b"hola").unwrap();
        assert_eq!(open_bytes(&b, &ct).unwrap(), b"hola");
        assert!(open_bytes(&a, &ct).is_err());
        // Cada sobre es distinto (clave efímera nueva).
        assert_ne!(ct, seal_bytes(&public_of(&b).unwrap(), b"hola").unwrap());
    }

    #[test]
    fn entradas_rotas_no_hacen_panic() {
        let k = new_key();
        for bad in ["", "%%%", "AAAA", &"A".repeat(200)] {
            assert!(open_bytes(&k, bad).is_err());
            assert!(public_of(bad).is_err());
            assert!(seal_bytes(bad, b"x").is_err());
        }
    }
}
