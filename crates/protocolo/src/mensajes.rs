//! Mensajes firmados y sellados (fase 5, docs/agente-gestionado.md): la consola firma
//! con Ed25519 todo lo que manda a cada agente, y el agente solo acepta lo
//! firmado por la consola que fijó al emparejarse, con `seq` creciente y
//! que no haya caducado. Además va cifrado para el agente (sobre sellado
//! X25519, como en la fase 3).

use base64::Engine;
use crypto_box::aead::OsRng;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
/// Una orden no vale pasado este tiempo.
pub const MAX_AGE_HOURS: i64 = 24;

// ---------- Claves de la consola ----------

pub fn new_signing_key() -> String {
    B64.encode(SigningKey::generate(&mut OsRng).to_bytes())
}

fn signing_of(b64: &str) -> Result<SigningKey, String> {
    let bytes: [u8; 32] = B64.decode(b64).ok().and_then(|v| v.try_into().ok()).ok_or("Clave de la consola dañada.")?;
    Ok(SigningKey::from_bytes(&bytes))
}

pub fn verifying_of_signing(signing_b64: &str) -> Result<String, String> {
    Ok(B64.encode(signing_of(signing_b64)?.verifying_key().to_bytes()))
}

fn verifying_of(b64: &str) -> Result<VerifyingKey, String> {
    let bytes: [u8; 32] = B64.decode(b64).ok().and_then(|v| v.try_into().ok()).ok_or("Clave pública de la consola no válida.")?;
    VerifyingKey::from_bytes(&bytes).map_err(|_| "Clave pública de la consola no válida.".to_string())
}

// ---------- Emparejamiento ----------

/// Letras y cifras de los códigos de emparejamiento: sin las que se confunden (I, L, O, 0, 1).
pub const ALFABETO_CODIGO: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";

/// Código de emparejamiento: 10 caracteres sin ambigüedades (~49,5 bits), como «ABCD-EFGH-JK».
///
/// Desde v1.4x la consola genera los suyos (consola/src/lib/codigo.ts) y este solo lo usan las
/// consolas anteriores y «Vincular este servidor». Antes tomaba los 10 primeros bytes de un
/// UUID v4 con `% 31`: el byte 6 (versión) y el 8 (variante) no son aleatorios del todo y el
/// módulo favorece unas letras. Ahora: solo bytes aleatorios y rechazo (sin sesgo).
pub fn pairing_code() -> String {
    let a = ALFABETO_CODIGO;
    // 248 = 8 × 31: lo que quede por encima se descarta para que todas salgan igual.
    let tope = (256 / a.len() * a.len()) as u8;
    let mut chars: Vec<char> = Vec::with_capacity(10);
    while chars.len() < 10 {
        let u = uuid::Uuid::new_v4();
        for (i, b) in u.as_bytes().iter().enumerate() {
            // En un UUID v4, los bytes 6 y 8 llevan la versión y la variante.
            if i == 6 || i == 8 || *b >= tope || chars.len() == 10 {
                continue;
            }
            chars.push(a[*b as usize % a.len()] as char);
        }
    }
    format!("{}-{}-{}", chars[..4].iter().collect::<String>(), chars[4..8].iter().collect::<String>(), chars[8..].iter().collect::<String>())
}

/// Lo que la web guarda del código: su hash (sin guiones ni mayúsculas que importen).
pub fn code_hash(code: &str) -> String {
    let norm: String = code.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_uppercase();
    Sha256::digest(norm.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// Código de comprobación (SAS) de 6 cifras: el mismo en la consola y en el
/// agente si las dos claves públicas son las que cada uno cree.
pub fn sas(console_sign_pub_b64: &str, endpoint_box_pub_b64: &str) -> String {
    let h = Sha256::digest(format!("resguardo-sas-v1|{console_sign_pub_b64}|{endpoint_box_pub_b64}").as_bytes());
    let n = u32::from_be_bytes([h[0], h[1], h[2], h[3]]) % 1_000_000;
    format!("{:03} {:03}", n / 1000, n % 1000)
}

// ---------- Mensajes firmados ----------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Message {
    pub v: u32,
    pub endpoint: String,
    pub seq: u64,
    pub issued_at: String,
    /// "config", "backup_now", "restore" o "unpair".
    pub kind: String,
    pub body: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Signed {
    /// El JSON exacto que se firmó (base64).
    payload: String,
    sig: String,
}

/// Firma un mensaje con la clave de la consola y lo sella para el agente.
pub fn seal_message(msg: &Message, signing_b64: &str, endpoint_box_pub_b64: &str) -> Result<String, String> {
    let payload = serde_json::to_vec(msg).map_err(|e| e.to_string())?;
    let sig = signing_of(signing_b64)?.sign(&payload);
    let signed = Signed { payload: B64.encode(&payload), sig: B64.encode(sig.to_bytes()) };
    let plain = serde_json::to_vec(&signed).map_err(|e| e.to_string())?;
    crate::claves::seal_bytes(endpoint_box_pub_b64, &plain)
}

/// Lo que el agente acepta: abre el sobre, comprueba la firma con la clave de
/// la consola fijada, que es para él, que `seq` es mayor que el último
/// aceptado y que no ha caducado.
pub fn open_message(
    ciphertext_b64: &str,
    box_secret_b64: &str,
    pinned_console_pub_b64: &str,
    me: &str,
    last_seq: u64,
    now: chrono::DateTime<chrono::Local>,
) -> Result<Message, String> {
    let plain = crate::claves::open_bytes(box_secret_b64, ciphertext_b64)?;
    verify_signed(&plain, pinned_console_pub_b64, me, last_seq, now)
}

/// La parte de [`open_message`] después de abrir el sobre: comprueba la firma
/// con la clave fijada, el destinatario, el `seq` y la caducidad. Recibe
/// datos de fuera: no debe hacer `panic` con ninguna entrada (ver fuzz/).
pub fn verify_signed(plain: &[u8], pinned_console_pub_b64: &str, me: &str, last_seq: u64, now: chrono::DateTime<chrono::Local>) -> Result<Message, String> {
    let signed: Signed = serde_json::from_slice(plain).map_err(|_| "Mensaje no válido.".to_string())?;
    let payload = B64.decode(&signed.payload).map_err(|_| "Mensaje no válido.".to_string())?;
    let sig_bytes: [u8; 64] = B64.decode(&signed.sig).ok().and_then(|v| v.try_into().ok()).ok_or("Firma no válida.")?;
    verifying_of(pinned_console_pub_b64)?
        .verify(&payload, &Signature::from_bytes(&sig_bytes))
        .map_err(|_| "La firma no es de la consola de este equipo: se ignora.".to_string())?;
    let msg: Message = serde_json::from_slice(&payload).map_err(|_| "Mensaje no válido.".to_string())?;
    if msg.v != 1 || msg.endpoint != me {
        return Err("El mensaje no es para este equipo.".into());
    }
    if msg.seq <= last_seq {
        return Err(format!("Mensaje repetido o antiguo (n.º {} ≤ {last_seq}): se ignora.", msg.seq));
    }
    let issued = chrono::DateTime::parse_from_rfc3339(&msg.issued_at).map_err(|_| "Fecha del mensaje no válida.".to_string())?;
    let age = now.signed_duration_since(issued);
    if age > chrono::Duration::hours(MAX_AGE_HOURS) || age < -chrono::Duration::minutes(10) {
        return Err("Mensaje caducado (o con la hora mal): se ignora.".into());
    }
    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> chrono::DateTime<chrono::Local> {
        chrono::Local::now()
    }
    fn msg(seq: u64, endpoint: &str) -> Message {
        Message { v: 1, endpoint: endpoint.into(), seq, issued_at: now().to_rfc3339(), kind: "backup_now".into(), body: serde_json::json!({ "plan": "p" }) }
    }

    #[test]
    fn solo_lo_firmado_por_su_consola() {
        let console = new_signing_key();
        let console_pub = verifying_of_signing(&console).unwrap();
        let other = new_signing_key();
        let box_secret = crate::claves::new_key();
        let box_pub = crate::claves::public_of(&box_secret).unwrap();

        let ct = seal_message(&msg(5, "e1"), &console, &box_pub).unwrap();
        assert_eq!(open_message(&ct, &box_secret, &console_pub, "e1", 4, now()).unwrap().seq, 5);
        // Firmado por otra clave (una web comprometida): rechazado.
        let forged = seal_message(&msg(6, "e1"), &other, &box_pub).unwrap();
        assert!(open_message(&forged, &box_secret, &console_pub, "e1", 4, now()).unwrap_err().contains("firma"));
        // Repetido: rechazado.
        assert!(open_message(&ct, &box_secret, &console_pub, "e1", 5, now()).unwrap_err().contains("repetido"));
        // Para otro equipo: rechazado.
        let to_other = seal_message(&msg(7, "e2"), &console, &box_pub).unwrap();
        assert!(open_message(&to_other, &box_secret, &console_pub, "e1", 4, now()).is_err());
        // Caducado: rechazado.
        let mut old = msg(8, "e1");
        old.issued_at = (now() - chrono::Duration::hours(30)).to_rfc3339();
        let ct_old = seal_message(&old, &console, &box_pub).unwrap();
        assert!(open_message(&ct_old, &box_secret, &console_pub, "e1", 4, now()).unwrap_err().contains("caducado"));
    }

    #[test]
    fn codigos() {
        let c = pairing_code();
        assert_eq!(c.len(), 12);
        assert_eq!(code_hash(&c), code_hash(&c.to_lowercase().replace('-', " ")));
        let s = sas("A", "B");
        assert_eq!(s.len(), 7);
        assert_eq!(s, sas("A", "B"));
        assert_ne!(s, sas("A", "C"));
    }
}
