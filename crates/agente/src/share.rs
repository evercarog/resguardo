//! Compartir un destino (la cuenta de la nube o el rest-server) entre los
//! equipos de un mismo usuario, con cifrado de extremo a extremo. Ver
//! docs/compartir.md (fase 3).
//!
//! - Cada equipo tiene un par X25519; la privada vive en `compartir.bin`
//!   (DPAPI de máquina, carpeta privada del agente: solo SYSTEM y
//!   administradores) y la pública se publica en la web.
//! - El equipo que comparte guarda allí las credenciales del destino; su
//!   agente las sella (sobre anónimo de libsodium) para la clave pública del
//!   equipo que las pide y la web solo transporta el sobre.
//! - Nunca se comparte la contraseña de un repositorio.

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

/// Un equipo tiene que llevar vinculado al menos esto antes de recibir nada.
const MIN_LINKED_MINUTES: i64 = 10;
/// Entregas como mucho por ciclo del agente.
const MAX_PER_TICK: usize = 5;
/// La web no acepta sobres de más de 8 KB: se deja margen.
const MAX_SEALED_B64: usize = 7 * 1024;

/// Credenciales de un destino (lo que va dentro del sobre).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct PlaceCreds {
    /// "s3", "b2", "azure", "gs" o "rest".
    pub kind: String,
    /// Ubicación base del destino (`s3:https://host/bucket`, `rest:https://host:8000/usuario/`).
    pub base: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_secret: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rest_user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rest_password: Option<String>,
    /// Certificado del servidor (PEM), si usa uno propio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_cert_pem: Option<String>,
}

/// Lo que se publica en la web de un destino compartido (sin secretos).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ShareMeta {
    pub kind: String,
    #[serde(default)]
    pub host: Option<String>,
    pub base: String,
    pub name: String,
}

/// El contenido del sobre.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct Envelope {
    v: u32,
    share: String,
    to_device: String,
    issued_at: String,
    creds: PlaceCreds,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct SharedPlace {
    pub share_id: Option<String>,
    pub meta: ShareMeta,
    pub creds: PlaceCreds,
    pub since: String,
    /// Entregas hechas: (equipo, cuándo).
    #[serde(default)]
    pub delivered: Vec<(String, String)>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ReceivedPlace {
    pub meta: ShareMeta,
    pub creds: PlaceCreds,
    pub from_device: String,
    pub received_at: String,
}

/// Lo que guarda el agente para compartir (cifrado con DPAPI).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Vault {
    /// Privada X25519 de este equipo (base64).
    #[serde(default)]
    pub device_key: Option<String>,
    /// La pública ya está en la web.
    #[serde(default)]
    pub key_published: bool,
    /// Destinos que comparte este equipo, por id local del destino.
    #[serde(default)]
    pub shared: HashMap<String, SharedPlace>,
    /// Destinos recibidos de otros equipos, por id del destino compartido.
    #[serde(default)]
    pub received: HashMap<String, ReceivedPlace>,
    /// Consola de equipos gestionados (fase 5): su clave Ed25519 (base64).
    #[serde(default)]
    pub console_key: Option<String>,
    #[serde(default)]
    pub console_key_published: bool,
    /// Equipos gestionados por esta consola, por id de equipo.
    #[serde(default)]
    pub endpoints: HashMap<String, crate::console::ManagedEndpoint>,
}

// ---------- Criptografía ----------

// Las claves y los sobres sellados son del crate `resguardo-protocolo`.
pub use resguardo_protocolo::claves::{new_key, public_of};

/// Sella las credenciales de un destino para la clave pública de un equipo.
pub fn seal(creds: &PlaceCreds, share: &str, to_device: &str, recipient_pub_b64: &str) -> Result<String, String> {
    let env = Envelope { v: 1, share: share.into(), to_device: to_device.into(), issued_at: chrono::Local::now().to_rfc3339(), creds: creds.clone() };
    let plain = serde_json::to_vec(&env).map_err(|e| e.to_string())?;
    let out = resguardo_protocolo::claves::seal_bytes(recipient_pub_b64, &plain)?;
    if out.len() > MAX_SEALED_B64 {
        return Err("Los datos del destino son demasiado grandes para enviarlos (¿un certificado enorme?).".into());
    }
    Ok(out)
}

/// Abre un sobre y comprueba que es para este equipo y para ese destino.
pub fn open(ciphertext_b64: &str, secret_b64: &str, share: &str, me: &str) -> Result<PlaceCreds, String> {
    let plain = resguardo_protocolo::claves::open_bytes(secret_b64, ciphertext_b64)?;
    let env: Envelope = serde_json::from_slice(&plain).map_err(|_| "Contenido del sobre no válido.".to_string())?;
    if env.v != 1 || env.share != share || env.to_device != me {
        return Err("El sobre no corresponde a esta petición.".into());
    }
    Ok(env.creds)
}

// ---------- Almacén ----------

fn vault_file() -> std::path::PathBuf {
    crate::agent::private_dir().join("compartir.bin")
}

pub fn load() -> Result<Vault, String> {
    match std::fs::read(vault_file()) {
        Ok(enc) => serde_json::from_slice(&crate::platform::unprotect(&enc)?).map_err(|e| format!("Datos de compartir dañados: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vault::default()),
        Err(e) => Err(format!("No se pudieron leer los datos de compartir: {e}")),
    }
}

pub fn save(v: &Vault) -> Result<(), String> {
    let enc = crate::platform::protect(&serde_json::to_vec(v).map_err(|e| e.to_string())?)?;
    let dir = crate::agent::private_dir();
    if !dir.is_dir() {
        return Err("La carpeta del agente no está preparada: abre Resguardo como administrador.".into());
    }
    let tmp = dir.join("compartir.bin.tmp");
    let _ = std::fs::remove_file(&tmp);
    std::fs::write(&tmp, enc).map_err(|e| format!("No se pudieron guardar los datos de compartir: {e}"))?;
    std::fs::rename(&tmp, vault_file()).map_err(|e| format!("No se pudieron guardar los datos de compartir: {e}"))
}

/// Clave de este equipo (la crea si no existe).
pub fn ensure_key(v: &mut Vault) -> String {
    if v.device_key.is_none() {
        v.device_key = Some(new_key());
        v.key_published = false;
    }
    v.device_key.clone().unwrap()
}

/// Metadatos y credenciales a partir de un repositorio del destino (las
/// credenciales salen del almacén del usuario, donde la app las tiene).
pub fn from_repo(repo: &crate::store::Repo, place_name: &str) -> Result<(ShareMeta, PlaceCreds), String> {
    let access = crate::store::access(repo)?;
    let kind = crate::places::kind(&repo.location);
    let (base, host) = match kind {
        "s3" => {
            let k = crate::places::key(&repo.location); // s3:host/bucket
            let rest = k.trim_start_matches("s3:");
            let scheme = if repo.location.contains("http://") { "http" } else { "https" };
            (format!("s3:{scheme}://{rest}"), rest.split('/').next().map(str::to_string))
        }
        "rest" => {
            let k = crate::places::key(&repo.location); // rest:https://host:port
            let base = format!("{}/", k);
            (base, k.rsplit("://").next().map(str::to_string))
        }
        "b2" | "azure" | "gs" => (crate::places::key(&repo.location), None),
        _ => return Err("Solo se pueden compartir destinos de la nube o servidores REST.".into()),
    };
    let cloud = crate::store::cloud_creds(repo)?;
    let creds = PlaceCreds {
        kind: kind.into(),
        base: base.clone(),
        region: cloud.as_ref().and_then(|c| c.region.clone()),
        key_id: cloud.as_ref().map(|c| c.key_id.clone()),
        key_secret: cloud.as_ref().map(|c| c.key_secret.clone()),
        rest_user: access.rest_auth.as_ref().map(|a| a.0.clone()),
        rest_password: access.rest_auth.as_ref().map(|a| a.1.clone()),
        tls_cert_pem: repo.cacert.as_ref().and_then(|p| std::fs::read_to_string(p).ok()),
    };
    // Lo que se ve en la web: sin usuario ni contraseña.
    let shown = match kind {
        "s3" => base.rsplit('/').next().unwrap_or("").to_string(),
        "rest" => "/".to_string(),
        _ => base.split_once(':').map_or(base.clone(), |x| x.1.to_string()),
    };
    Ok((ShareMeta { kind: kind.into(), host, base: shown, name: place_name.into() }, creds))
}

// ---------- Web ----------

fn rpc(link: &crate::web::WebLink, secret: &str, function: &str, extra: serde_json::Value) -> Result<serde_json::Value, String> {
    let mut body = json!({ "p_device": link.device_id, "p_secret": secret });
    if let (Some(b), Some(e)) = (body.as_object_mut(), extra.as_object()) {
        for (k, v) in e {
            b.insert(k.clone(), v.clone());
        }
    }
    crate::web::rpc(&link.url, &link.key, function, &body)
}

/// Publica (o retira, con `meta` None) un destino compartido. Devuelve el id del destino compartido.
pub fn publish(link: &crate::web::WebLink, secret: &str, place_id: &str, meta: Option<&ShareMeta>) -> Result<Option<String>, String> {
    let answer = rpc(link, secret, "place_share_set", json!({ "p_place_id": place_id, "p_meta": meta }))?;
    Ok(answer.get("id").and_then(|v| v.as_str()).map(str::to_string))
}

#[derive(Deserialize, Debug)]
struct Pending {
    request_id: String,
    share_id: String,
    place_id: String,
    requester_device: String,
    #[serde(default)]
    requester_name: String,
    #[serde(default)]
    requester_public_key: Option<String>,
    #[serde(default)]
    requester_linked_at: Option<String>,
}

#[derive(Deserialize, Debug)]
struct Inbox {
    request_id: String,
    share_id: String,
    ciphertext: String,
    #[serde(default)]
    from_device_name: String,
    meta: ShareMeta,
}

/// ¿Puede recibir este equipo? (vinculado hace más de MIN_LINKED_MINUTES).
fn old_enough(linked_at: Option<&str>, now: chrono::DateTime<chrono::Local>) -> bool {
    linked_at
        .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
        .is_some_and(|t| now.signed_duration_since(t) >= chrono::Duration::minutes(MIN_LINKED_MINUTES))
}

/// Ciclo del agente: publica la clave, entrega lo pedido y recoge lo recibido.
/// Devuelve los avisos que hay que mostrar (para el historial y Windows).
pub fn tick(link: &crate::web::WebLink, secret: &str) -> Vec<String> {
    let mut notes = Vec::new();
    let Ok(mut vault) = load() else { return notes };
    // Cada equipo vinculado tiene su clave (para poder recibir un destino cuando lo pida).
    let mut dirty = vault.device_key.is_none();
    let key = ensure_key(&mut vault);
    if !vault.key_published {
        if let Ok(public) = public_of(&key) {
            if rpc(link, secret, "device_set_key", json!({ "p_public_key": public })).is_ok() {
                vault.key_published = true;
                dirty = true;
            }
        }
    }

    // Entregar lo que pidieron mis otros equipos.
    if !vault.shared.is_empty() {
        if let Ok(answer) = rpc(link, secret, "share_pending", json!({})) {
            let list: Vec<Pending> = serde_json::from_value(answer).unwrap_or_default();
            for p in list.into_iter().take(MAX_PER_TICK) {
                let Some(place) = vault.shared.get_mut(&p.place_id) else {
                    let _ = rpc(link, secret, "share_reject", json!({ "p_request": p.request_id, "p_reason": "Ese destino ya no se comparte." }));
                    continue;
                };
                let why = if !old_enough(p.requester_linked_at.as_deref(), chrono::Local::now()) {
                    Some("El equipo que lo pide se vinculó hace muy poco: vuelve a pedirlo dentro de unos minutos.")
                } else if p.requester_public_key.is_none() {
                    Some("El equipo que lo pide aún no tiene clave: ábrelo y vuelve a pedirlo.")
                } else {
                    None
                };
                if let Some(why) = why {
                    let _ = rpc(link, secret, "share_reject", json!({ "p_request": p.request_id, "p_reason": why }));
                    continue;
                }
                match seal(&place.creds, &p.share_id, &p.requester_device, p.requester_public_key.as_deref().unwrap_or_default()) {
                    Ok(ct) => {
                        if rpc(link, secret, "share_deliver", json!({ "p_request": p.request_id, "p_ciphertext": ct })).is_ok() {
                            place.delivered.push((p.requester_name.clone(), chrono::Local::now().to_rfc3339()));
                            notes.push(format!("Entregado el destino «{}» a «{}».", place.meta.name, p.requester_name));
                            dirty = true;
                        }
                    }
                    Err(e) => {
                        let _ = rpc(link, secret, "share_reject", json!({ "p_request": p.request_id, "p_reason": e }));
                    }
                }
            }
        }
    }

    // Recoger lo que me entregaron.
    if let Ok(answer) = rpc(link, secret, "share_inbox", json!({})) {
        let list: Vec<Inbox> = serde_json::from_value(answer).unwrap_or_default();
        for item in list.into_iter().take(MAX_PER_TICK) {
            match open(&item.ciphertext, &key, &item.share_id, &link.device_id) {
                Ok(creds) => {
                    notes.push(format!("{} recibió el destino «{}» desde «{}».", link.device_name, item.meta.name, item.from_device_name));
                    vault.received.insert(
                        item.share_id.clone(),
                        ReceivedPlace { meta: item.meta, creds, from_device: item.from_device_name, received_at: chrono::Local::now().to_rfc3339() },
                    );
                    dirty = true;
                    let _ = rpc(link, secret, "share_ack", json!({ "p_request": item.request_id }));
                }
                Err(e) => crate::agent::log(&format!("No se pudo abrir un destino compartido recibido: {e}")),
            }
        }
    }
    if dirty {
        if let Err(e) = save(&vault) {
            crate::agent::log(&format!("ERROR: {e}"));
        }
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

    fn creds() -> PlaceCreds {
        PlaceCreds { kind: "s3".into(), base: "s3:https://h/b".into(), key_id: Some("K".into()), key_secret: Some("S".into()), ..Default::default() }
    }

    #[test]
    fn sella_y_abre_solo_quien_debe() {
        let (a, b) = (new_key(), new_key());
        let ct = seal(&creds(), "share-1", "dev-b", &public_of(&b).unwrap()).unwrap();
        assert_eq!(open(&ct, &b, "share-1", "dev-b").unwrap(), creds());
        // Otro equipo no puede abrirlo.
        assert!(open(&ct, &a, "share-1", "dev-b").is_err());
        // Ni reutilizarlo para otro destino u otro equipo.
        assert!(open(&ct, &b, "share-2", "dev-b").is_err());
        assert!(open(&ct, &b, "share-1", "dev-c").is_err());
        // Bien por debajo del límite de la web (8 KB), incluso con un certificado.
        let mut big = creds();
        big.tls_cert_pem = Some("A".repeat(3000));
        assert!(seal(&big, "s", "d", &public_of(&b).unwrap()).unwrap().len() < MAX_SEALED_B64);
        big.tls_cert_pem = Some("A".repeat(9000));
        assert!(seal(&big, "s", "d", &public_of(&b).unwrap()).is_err());
        // El sobre no lleva las credenciales a la vista.
        assert!(!String::from_utf8_lossy(&B64.decode(&ct).unwrap()).contains("\"S\""));
    }

    #[test]
    fn solo_equipos_vinculados_hace_rato() {
        let now = chrono::Local::now();
        assert!(!old_enough(Some(&now.to_rfc3339()), now));
        assert!(old_enough(Some(&(now - chrono::Duration::minutes(11)).to_rfc3339()), now));
        assert!(!old_enough(None, now));
    }
}
