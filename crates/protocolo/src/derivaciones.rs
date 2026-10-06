//! Lo que se deriva de la clave de administración y de las claves del equipo
//! (docs/api-servidor.md, §1). La consola (JavaScript) debe dar exactamente
//! lo mismo: ver los vectores.

use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

use crate::claves::B64;

type HmacSha256 = Hmac<Sha256>;

/// Parámetros de Argon2id: 64 MiB, 3 pasadas, 1 hilo, 32 bytes.
pub const ARGON2_M_KIB: u32 = 65_536;
pub const ARGON2_T: u32 = 3;
pub const ARGON2_P: u32 = 1;

/// La clave de administración se normaliza a Unicode NFC antes de Argon2id:
/// una «ñ» o una tilde escritas en otro sistema (o compuestas de otra forma)
/// dan la misma clave. La consola hace lo mismo (`clave.normalize("NFC")`).
pub fn normalizar_clave(clave: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    clave.nfc().collect()
}

/// Argon2id de la clave (normalizada a NFC) con esa sal (en bytes, al menos 16).
pub fn argon2id(clave: &str, sal: &[u8]) -> Result<[u8; 32], String> {
    if sal.len() < 16 {
        return Err("Sal demasiado corta.".into());
    }
    let clave = normalizar_clave(clave);
    let params = Params::new(ARGON2_M_KIB, ARGON2_T, ARGON2_P, Some(32)).map_err(|e| e.to_string())?;
    let mut out = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params).hash_password_into(clave.as_bytes(), sal, &mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

fn sal_de(b64: &str) -> Result<Vec<u8>, String> {
    B64.decode(b64).map_err(|_| "Sal no válida.".to_string())
}

/// `prueba_e = Argon2id(clave, sal_equipo)`.
pub fn prueba_admin(clave: &str, sal_equipo_b64: &str) -> Result<[u8; 32], String> {
    argon2id(clave, &sal_de(sal_equipo_b64)?)
}

/// `verificador_e = SHA-256(prueba_e)`: lo único que guarda el equipo.
pub fn verificador(prueba: &[u8]) -> [u8; 32] {
    Sha256::digest(prueba).into()
}

/// ¿Corresponde la prueba al verificador? (tiempo constante).
pub fn comprueba_prueba(prueba: &[u8], verificador_guardado: &[u8]) -> bool {
    let v = verificador(prueba);
    v.len() == verificador_guardado.len() && v.iter().zip(verificador_guardado).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
}

/// HKDF-SHA256 (RFC 5869) con sal vacía: una clave de 32 bytes para `info`.
pub fn hkdf32(ikm: &[u8], info: &str) -> [u8; 32] {
    // Extract: PRK = HMAC(sal vacía, IKM). HMAC rellena la clave con ceros, así que es lo mismo que 32 ceros.
    let mut ext = HmacSha256::new_from_slice(&[0u8; 32]).expect("HMAC acepta cualquier longitud");
    ext.update(ikm);
    let prk = ext.finalize().into_bytes();
    // Expand: T(1) = HMAC(PRK, info | 0x01).
    let mut exp = HmacSha256::new_from_slice(&prk).expect("HMAC acepta cualquier longitud");
    exp.update(info.as_bytes());
    exp.update(&[1u8]);
    exp.finalize().into_bytes().into()
}

/// Clave de la configuración cifrada (`K_cfg`).
pub fn k_cfg(clave: &str, sal_cliente_b64: &str) -> Result<[u8; 32], String> {
    Ok(hkdf32(&argon2id(clave, &sal_de(sal_cliente_b64)?)?, "resguardo-kcfg-v1"))
}

/// Clave del paquete de exportación (`K_exp`).
pub fn k_exp(clave: &str, sal_cliente_b64: &str) -> Result<[u8; 32], String> {
    Ok(hkdf32(&argon2id(clave, &sal_de(sal_cliente_b64)?)?, "resguardo-kexp-v1"))
}

/// Etiqueta de la identidad de un equipo: `HMAC(K_cfg, "resguardo-etiqueta-v1|" + equipo + "|" + box_pub + "|" + sign_pub)`.
pub fn etiqueta_equipo(k_cfg: &[u8; 32], equipo: &str, box_pub: &str, sign_pub: &str) -> String {
    let mut m = HmacSha256::new_from_slice(k_cfg).expect("HMAC acepta cualquier longitud");
    m.update(format!("resguardo-etiqueta-v1|{equipo}|{box_pub}|{sign_pub}").as_bytes());
    B64.encode(m.finalize().into_bytes())
}

/// Código de comprobación v2: liga la identidad del servidor y las claves del equipo.
pub fn sas_v2(identidad_servidor: &str, box_pub: &str, sign_pub: &str) -> String {
    let h = Sha256::digest(format!("resguardo-sas-v2|{identidad_servidor}|{box_pub}|{sign_pub}").as_bytes());
    let n = u32::from_be_bytes([h[0], h[1], h[2], h[3]]) % 1_000_000;
    format!("{:03} {:03}", n / 1000, n % 1000)
}

/// La huella SHA-256 de un certificado tal como entra en el SAS v3: solo las cifras
/// hexadecimales, en mayúsculas (`AB:CD:…` y `abcd…` dan lo mismo). Vacía si no hay TLS.
pub fn huella_para_sas(huella: &str) -> String {
    huella.chars().filter(char::is_ascii_hexdigit).collect::<String>().to_ascii_uppercase()
}

/// Código de comprobación v3 (v1.26, agentes ≥ 0.7.10): el de v2 más la huella de la
/// autoridad TLS. El equipo pone la que fijó al vincular; la consola, la que da el servidor
/// (`huella_ca` de `GET /api/servidor`). Alguien en medio que haga fijar **su** autoridad
/// da otro número aunque reenvíe intactas la identidad y las llaves.
pub fn sas_v3(identidad_servidor: &str, box_pub: &str, sign_pub: &str, huella_ca: &str) -> String {
    let huella = huella_para_sas(huella_ca);
    let h = Sha256::digest(format!("resguardo-sas-v3|{identidad_servidor}|{box_pub}|{sign_pub}|{huella}").as_bytes());
    let n = u32::from_be_bytes([h[0], h[1], h[2], h[3]]) % 1_000_000;
    format!("{:03} {:03}", n / 1000, n % 1000)
}

/// Texto que firma el servidor para demostrar su identidad a un agente.
pub fn texto_identidad_servidor(reto_b64: &str, equipo: &str) -> String {
    format!("resguardo-servidor-v1|{reto_b64}|{equipo}")
}

/// Texto que firma el servidor con el ancla de la auditoría de un cliente (v1.50,
/// plan-mejoras 9b): la entrada `n` de su cadena, de `creado` (segundos Unix), tiene la
/// huella `hash`. El agente la comprueba con la identidad que fijó al vincular.
pub fn texto_ancla_auditoria(cliente: &str, n: u64, creado: i64, hash: &str) -> String {
    format!("resguardo-ancla-auditoria-v1|{cliente}|{n}|{creado}|{hash}")
}

/// El ancla en una línea, para el correo y para pegarla en «Comprobar con un ancla»
/// (la consola la lee en `lib/auditoria.ts`, `leerAncla`).
pub fn linea_ancla(cliente: &str, n: u64, creado: i64, hash: &str) -> String {
    format!("resguardo-ancla:1:{cliente}:{n}:{creado}:{hash}")
}

/// Texto que firma el agente con el resultado de una orden.
pub fn texto_resultado(orden: &str, seq: u64, estado: &str, mensaje: Option<&str>, detalle: Option<&str>) -> String {
    format!("resguardo-resultado-v1|{orden}|{seq}|{estado}|{}|{}", mensaje.unwrap_or(""), detalle.unwrap_or(""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hkdf_coincide_con_rfc5869() {
        // RFC 5869, caso de prueba 3 (sal e info vacías): los 32 primeros bytes del OKM.
        let ikm = [0x0bu8; 22];
        let mut ext = HmacSha256::new_from_slice(&[]).unwrap();
        ext.update(&ikm);
        let prk = ext.finalize().into_bytes();
        let mut exp = HmacSha256::new_from_slice(&prk).unwrap();
        exp.update(&[1u8]);
        let okm: [u8; 32] = exp.finalize().into_bytes().into();
        let hex: String = okm.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, "8da4e775a563c18f715f802a063c5a31b8a11f5c5ee1879ec3454e5f3c738d2d");
        // La de este módulo (sal de 32 ceros) da lo mismo.
        assert_eq!(hkdf32(&ikm, ""), okm);
    }

    #[test]
    fn prueba_por_equipo() {
        let sal1 = B64.encode([1u8; 16]);
        let sal2 = B64.encode([2u8; 16]);
        let p1 = prueba_admin("caballo bateria grapa correcta", &sal1).unwrap();
        let p2 = prueba_admin("caballo bateria grapa correcta", &sal2).unwrap();
        assert_ne!(p1, p2, "cada equipo tiene su prueba");
        let v1 = verificador(&p1);
        assert!(comprueba_prueba(&p1, &v1));
        assert!(!comprueba_prueba(&p2, &v1));
        assert!(prueba_admin("x", &B64.encode([1u8; 4])).is_err(), "sal corta");
    }

    #[test]
    fn la_clave_se_normaliza_a_nfc() {
        let sal = B64.encode([1u8; 16]);
        // «ñ» compuesta (U+00F1) y descompuesta (n + U+0303).
        let compuesta = "pi\u{f1}a colada";
        let descompuesta = "pin\u{303}a colada";
        assert_ne!(compuesta, descompuesta);
        assert_eq!(normalizar_clave(descompuesta), compuesta);
        assert_eq!(prueba_admin(compuesta, &sal).unwrap(), prueba_admin(descompuesta, &sal).unwrap());
    }

    #[test]
    fn sas_v3_cubre_la_autoridad_tls() {
        let (id, bx, sg) = ("aWRlbnRpZGFk", "Ym94", "c2lnbg==");
        let h = "3F:A1:09:7C:52:E4:8B:D0:16:2A:9E:C3:75:0B:F8:44:D9:61:2C:A7:30:5E:B2:8F:E1:47:0A:96:CD:13:7B:58";
        // Da igual cómo se escriba la huella (separadores, mayúsculas).
        assert_eq!(sas_v3(id, bx, sg, h), sas_v3(id, bx, sg, &h.replace(':', "").to_lowercase()));
        // Otra autoridad (un solo bit), otro número; y no es el de v2.
        let otra = format!("{}9", &h[..h.len() - 1]);
        assert_ne!(sas_v3(id, bx, sg, h), sas_v3(id, bx, sg, &otra));
        assert_ne!(sas_v3(id, bx, sg, h), sas_v2(id, bx, sg));
        assert_ne!(sas_v3(id, bx, sg, h), sas_v3(id, bx, sg, ""));
        assert_eq!(sas_v3(id, bx, sg, h).len(), 7);
    }
}
