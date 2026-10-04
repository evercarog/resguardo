//! Los secretos de los canales (contraseña SMTP, tokens, la dirección de un
//! webhook…) se guardan cifrados en la base de datos, con XChaCha20-Poly1305 y
//! una clave derivada de la identidad del servidor (`identidad.key`, en la
//! carpeta de datos). Así una copia de `control.db` sola no los revela, y la
//! copia de la consola (que lleva las dos cosas, cifradas para su clave de
//! respaldo) los restaura enteros. La API nunca los devuelve.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;

/// La clave de los secretos de las notificaciones.
#[derive(Clone)]
pub struct Clave([u8; 32]);

impl std::fmt::Debug for Clave {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Clave(…)")
    }
}

const PREFIJO: &str = "n1:";

impl Clave {
    /// HMAC-SHA256(identidad, etiqueta): una clave propia, que no sirve para nada más.
    pub fn de_identidad(identidad: &ed25519_dalek::SigningKey) -> Self {
        let mut m = <Hmac<Sha256> as Mac>::new_from_slice(&identidad.to_bytes()).expect("HMAC acepta cualquier longitud");
        m.update(b"resguardo-notificaciones-secretos-v1");
        Self(m.finalize().into_bytes().into())
    }

    #[cfg(test)]
    pub fn fija(b: [u8; 32]) -> Self {
        Self(b)
    }

    /// `n1:` + base64(nonce ‖ cifrado). `contexto` (canal y campo) va como datos asociados:
    /// un secreto copiado a otro canal o campo no se abre.
    pub fn cerrar(&self, contexto: &str, texto: &str) -> String {
        let mut nonce = [0u8; 24];
        rand::rng().fill_bytes(&mut nonce);
        let ct = XChaCha20Poly1305::new((&self.0).into())
            .encrypt(XNonce::from_slice(&nonce), Payload { msg: texto.as_bytes(), aad: contexto.as_bytes() })
            .expect("cifrar en memoria no falla");
        let mut out = nonce.to_vec();
        out.extend_from_slice(&ct);
        format!("{PREFIJO}{}", B64.encode(out))
    }

    pub fn abrir(&self, contexto: &str, cerrado: &str) -> Option<String> {
        let b = B64.decode(cerrado.strip_prefix(PREFIJO)?).ok()?;
        if b.len() < 24 + 16 {
            return None;
        }
        let (nonce, ct) = b.split_at(24);
        let claro = XChaCha20Poly1305::new((&self.0).into()).decrypt(XNonce::from_slice(nonce), Payload { msg: ct, aad: contexto.as_bytes() }).ok()?;
        String::from_utf8(claro).ok()
    }
}

/// El contexto de un secreto: ámbito, canal y campo.
pub fn contexto(ambito: &str, canal: &str, campo: &str) -> String {
    format!("resguardo-notif-v1|{ambito}|{canal}|{campo}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_y_vuelta_y_atado_a_su_campo() {
        let k = Clave::fija([7; 32]);
        let c = contexto("servidor", "canal-1", "contrasena");
        let cerrado = k.cerrar(&c, "contraseña SMTP muy secreta");
        assert!(cerrado.starts_with("n1:"));
        assert!(!cerrado.contains("secreta"));
        assert_eq!(k.abrir(&c, &cerrado).as_deref(), Some("contraseña SMTP muy secreta"));
        // Dos veces lo mismo no da lo mismo (nonce aleatorio).
        assert_ne!(k.cerrar(&c, "x"), k.cerrar(&c, "x"));
        // Otro campo, otro canal u otra clave: no se abre.
        assert_eq!(k.abrir(&contexto("servidor", "canal-1", "token"), &cerrado), None);
        assert_eq!(k.abrir(&contexto("servidor", "canal-2", "contrasena"), &cerrado), None);
        assert_eq!(Clave::fija([8; 32]).abrir(&c, &cerrado), None);
        assert_eq!(k.abrir(&c, "n1:AAAA"), None);
        assert_eq!(k.abrir(&c, "texto en claro"), None);
        // La de la identidad: siempre la misma para la misma identidad.
        let id = ed25519_dalek::SigningKey::from_bytes(&[1; 32]);
        let (a, b) = (Clave::de_identidad(&id), Clave::de_identidad(&id));
        assert_eq!(b.abrir(&c, &a.cerrar(&c, "hola")).as_deref(), Some("hola"));
        assert_eq!(format!("{a:?}"), "Clave(…)");
    }
}
