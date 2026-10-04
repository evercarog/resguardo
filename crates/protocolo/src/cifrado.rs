//! Cifrado simétrico con una clave de 32 bytes (p. ej. `K_cfg`):
//! `crypto_secretbox` de libsodium (XSalsa20-Poly1305). En base64:
//! `nonce (24 bytes) || texto cifrado`. La consola lo reproduce con
//! `crypto_secretbox_easy` / `crypto_secretbox_open_easy`.

use crate::claves::B64;
use base64::Engine;
use crypto_secretbox::aead::{Aead, AeadCore, KeyInit, OsRng};
use crypto_secretbox::XSalsa20Poly1305;

pub fn cifrar(clave: &[u8; 32], datos: &[u8]) -> Result<String, String> {
    let caja = XSalsa20Poly1305::new(clave.into());
    let nonce = XSalsa20Poly1305::generate_nonce(&mut OsRng);
    let ct = caja.encrypt(&nonce, datos).map_err(|_| "No se pudo cifrar.".to_string())?;
    let mut out = nonce.to_vec();
    out.extend_from_slice(&ct);
    Ok(B64.encode(out))
}

pub fn descifrar(clave: &[u8; 32], b64: &str) -> Result<Vec<u8>, String> {
    let todo = B64.decode(b64).map_err(|_| "Datos cifrados no válidos.".to_string())?;
    if todo.len() < 24 + 16 {
        return Err("Datos cifrados no válidos.".into());
    }
    let (nonce, ct) = todo.split_at(24);
    XSalsa20Poly1305::new(clave.into()).decrypt(nonce.into(), ct).map_err(|_| "No se pudo descifrar (¿clave equivocada?).".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_y_vuelta() {
        let k = [9u8; 32];
        let c = cifrar(&k, b"config").unwrap();
        assert_eq!(descifrar(&k, &c).unwrap(), b"config");
        assert!(descifrar(&[1u8; 32], &c).is_err());
        assert!(descifrar(&k, "AAAA").is_err());
    }
}
