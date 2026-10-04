//! Cifrado simétrico del protocolo v2 (docs/api-servidor.md, §1 «Formatos
//! cifrados»): XChaCha20-Poly1305 con datos asociados que atan cada mensaje a
//! su contexto. Tres usos:
//!
//! - **Sesiones interactivas.** La consola elige `clave_sesion` (32 bytes) y
//!   la manda dentro del sobre de la orden que abre la sesión. Cada dirección
//!   tiene su clave: `HKDF-SHA256(ikm = clave_sesion, sal = sesion_id,
//!   info = "resguardo-sesion-v1|consola" | "resguardo-sesion-v1|equipo")`.
//!   Mensaje: `nonce(24) ‖ XChaCha20-Poly1305(k, nonce, json,
//!   aad = "resguardo-sesion-v1|" + sesion_id)`.
//! - **Relé de descargas.** Trozo `n`: `nonce(24) ‖ XChaCha20-Poly1305(clave_relevo,
//!   nonce, datos, aad = relevo_id + "|" + n + "|" + (último ? "1" : "0"))`.
//! - **Configuración del equipo** (con `K_cfg`): `nonce(24) ‖
//!   XChaCha20-Poly1305(K_cfg, nonce, json, aad = "resguardo-config-v1|" +
//!   equipo_id + "|" + seq)`.
//!
//! La consola (JavaScript) da lo mismo: ver los vectores (`simetrico`).

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub const NONCE: usize = 24;
pub const ETIQUETA: usize = 16;
/// Tamaño de cada trozo del relé, sin cifrar (el último puede ser menor).
pub const TROZO: usize = 4 * 1024 * 1024;

/// Lado de una sesión: quién cifra con esa clave.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lado {
    Consola,
    Equipo,
}

impl Lado {
    fn info(self) -> &'static str {
        match self {
            Lado::Consola => "resguardo-sesion-v1|consola",
            Lado::Equipo => "resguardo-sesion-v1|equipo",
        }
    }
}

/// HKDF-SHA256 (RFC 5869) con sal: 32 bytes.
pub fn hkdf32_con_sal(ikm: &[u8], sal: &[u8], info: &str) -> [u8; 32] {
    hkdf32_bytes(ikm, sal, info.as_bytes())
}

fn hkdf32_bytes(ikm: &[u8], sal: &[u8], info: &[u8]) -> [u8; 32] {
    let mut ext = <HmacSha256 as Mac>::new_from_slice(sal).expect("HMAC acepta cualquier longitud");
    ext.update(ikm);
    let prk = ext.finalize().into_bytes();
    let mut exp = <HmacSha256 as Mac>::new_from_slice(&prk).expect("HMAC acepta cualquier longitud");
    exp.update(info);
    exp.update(&[1u8]);
    exp.finalize().into_bytes().into()
}

/// Clave de una dirección de la sesión.
pub fn clave_direccion(clave_sesion: &[u8; 32], sesion_id: &str, lado: Lado) -> [u8; 32] {
    hkdf32_con_sal(clave_sesion, sesion_id.as_bytes(), lado.info())
}

/// Nonce aleatorio de 24 bytes.
pub fn nonce_aleatorio() -> [u8; NONCE] {
    use crypto_box::aead::rand_core::RngCore;
    let mut n = [0u8; NONCE];
    crypto_box::aead::OsRng.fill_bytes(&mut n);
    n
}

/// `nonce ‖ XChaCha20-Poly1305(clave, nonce, datos, aad)`.
pub fn cifrar_con(clave: &[u8; 32], nonce: &[u8; NONCE], datos: &[u8], aad: &[u8]) -> Vec<u8> {
    let ct = XChaCha20Poly1305::new(clave.into())
        .encrypt(XNonce::from_slice(nonce), Payload { msg: datos, aad })
        .expect("XChaCha20-Poly1305 no falla al cifrar en memoria");
    let mut out = Vec::with_capacity(NONCE + ct.len());
    out.extend_from_slice(nonce);
    out.extend_from_slice(&ct);
    out
}

/// Abre `nonce ‖ ct` (falla si está alterado o no es de este contexto).
pub fn descifrar_con(clave: &[u8; 32], cifrado: &[u8], aad: &[u8]) -> Result<Vec<u8>, String> {
    if cifrado.len() < NONCE + ETIQUETA {
        return Err("Cifrado demasiado corto.".into());
    }
    XChaCha20Poly1305::new(clave.into())
        .decrypt(XNonce::from_slice(&cifrado[..NONCE]), Payload { msg: &cifrado[NONCE..], aad })
        .map_err(|_| "No se pudo descifrar (clave o contexto distintos, o datos alterados).".to_string())
}

fn aad_sesion(sesion_id: &str) -> Vec<u8> {
    format!("resguardo-sesion-v1|{sesion_id}").into_bytes()
}

/// Mensaje de una sesión (`json` ya serializado).
pub fn cifrar_mensaje(k: &[u8; 32], sesion_id: &str, json: &[u8], nonce: &[u8; NONCE]) -> Vec<u8> {
    cifrar_con(k, nonce, json, &aad_sesion(sesion_id))
}

pub fn descifrar_mensaje(k: &[u8; 32], sesion_id: &str, cifrado: &[u8]) -> Result<Vec<u8>, String> {
    descifrar_con(k, cifrado, &aad_sesion(sesion_id))
}

fn aad_trozo(relevo_id: &str, n: u64, ultimo: bool) -> Vec<u8> {
    format!("{relevo_id}|{n}|{}", u8::from(ultimo)).into_bytes()
}

/// Trozo `n` del relé (el último lo dice: no se puede recortar la descarga).
pub fn cifrar_trozo(k: &[u8; 32], relevo_id: &str, n: u64, ultimo: bool, datos: &[u8], nonce: &[u8; NONCE]) -> Vec<u8> {
    cifrar_con(k, nonce, datos, &aad_trozo(relevo_id, n, ultimo))
}

pub fn descifrar_trozo(k: &[u8; 32], relevo_id: &str, n: u64, ultimo: bool, cifrado: &[u8]) -> Result<Vec<u8>, String> {
    descifrar_con(k, cifrado, &aad_trozo(relevo_id, n, ultimo))
}

/// Clave de las plantillas de copia de un cliente (v1.20), derivada de `K_cfg`:
/// `HKDF-SHA256(ikm = K_cfg, salt = "", info = "resguardo-kplantilla-v1")`.
/// Solo la usa la consola: el servidor guarda las plantillas cifradas y no las lee.
pub fn clave_plantillas(k_cfg: &[u8; 32]) -> [u8; 32] {
    hkdf32_con_sal(k_cfg, b"", "resguardo-kplantilla-v1")
}

fn aad_plantilla(cliente: &str, id: &str) -> Vec<u8> {
    format!("resguardo-plantilla-v1|{cliente}|{id}").into_bytes()
}

/// Una plantilla de copia, atada al cliente y a su id (el servidor no puede
/// cambiarla de nombre ni pasarla a otro cliente sin que se note).
pub fn cifrar_plantilla(k_pla: &[u8; 32], cliente: &str, id: &str, json: &[u8], nonce: &[u8; NONCE]) -> Vec<u8> {
    cifrar_con(k_pla, nonce, json, &aad_plantilla(cliente, id))
}

pub fn descifrar_plantilla(k_pla: &[u8; 32], cliente: &str, id: &str, cifrado: &[u8]) -> Result<Vec<u8>, String> {
    descifrar_con(k_pla, cifrado, &aad_plantilla(cliente, id))
}

fn aad_config(equipo: &str, seq: u64) -> Vec<u8> {
    format!("resguardo-config-v1|{equipo}|{seq}").into_bytes()
}

/// Configuración del equipo con `K_cfg`, atada al equipo y a su número.
pub fn cifrar_config(k_cfg: &[u8; 32], equipo: &str, seq: u64, json: &[u8], nonce: &[u8; NONCE]) -> Vec<u8> {
    cifrar_con(k_cfg, nonce, json, &aad_config(equipo, seq))
}

pub fn descifrar_config(k_cfg: &[u8; 32], equipo: &str, seq: u64, cifrado: &[u8]) -> Result<Vec<u8>, String> {
    descifrar_con(k_cfg, cifrado, &aad_config(equipo, seq))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hkdf_con_sal_rfc5869_caso_1() {
        // RFC 5869, caso 1: IKM 22×0x0b, sal 0x00..=0x0c, info 0xf0..=0xf9; los 32 primeros bytes del OKM.
        let sal: Vec<u8> = (0u8..=0x0c).collect();
        let info: Vec<u8> = (0xf0u8..=0xf9).collect();
        let okm = hkdf32_bytes(&[0x0bu8; 22], &sal, &info);
        let hex: String = okm.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf");
    }

    #[test]
    fn sesion_ida_y_vuelta_y_contexto() {
        let clave = [7u8; 32];
        let kc = clave_direccion(&clave, "s-1", Lado::Consola);
        let ke = clave_direccion(&clave, "s-1", Lado::Equipo);
        assert_ne!(kc, ke, "una clave por dirección");
        let c = cifrar_mensaje(&ke, "s-1", br#"{"i":1,"op":"lista"}"#, &nonce_aleatorio());
        assert_eq!(descifrar_mensaje(&ke, "s-1", &c).unwrap(), br#"{"i":1,"op":"lista"}"#);
        assert!(descifrar_mensaje(&ke, "s-2", &c).is_err(), "otra sesión");
        assert!(descifrar_mensaje(&kc, "s-1", &c).is_err(), "otra dirección");
    }

    #[test]
    fn trozos_no_se_reordenan_ni_recortan() {
        let k = [9u8; 32];
        let t = cifrar_trozo(&k, "r-1", 3, false, b"datos", &nonce_aleatorio());
        assert_eq!(descifrar_trozo(&k, "r-1", 3, false, &t).unwrap(), b"datos");
        assert!(descifrar_trozo(&k, "r-1", 2, false, &t).is_err());
        assert!(descifrar_trozo(&k, "r-1", 3, true, &t).is_err());
        assert!(descifrar_trozo(&k, "r-2", 3, false, &t).is_err());
    }

    #[test]
    fn config_atada_a_equipo_y_seq() {
        let k = [1u8; 32];
        let c = cifrar_config(&k, "e-1", 4, b"{}", &nonce_aleatorio());
        assert!(descifrar_config(&k, "e-1", 4, &c).is_ok());
        assert!(descifrar_config(&k, "e-2", 4, &c).is_err());
        assert!(descifrar_config(&k, "e-1", 5, &c).is_err());
    }
}
