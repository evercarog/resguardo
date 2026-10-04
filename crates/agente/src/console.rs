//! La consola de los equipos gestionados (fase 5, docs/agente-gestionado.md):
//! empareja equipos, les asigna copias en este Servidor de copias y les
//! manda órdenes firmadas con su clave Ed25519.

use crate::managed::{self, EndpointConfig, Message};
use crate::share;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ManagedEndpoint {
    pub device_id: String,
    pub name: String,
    /// X25519 del equipo (base64): para sellarle los mensajes.
    pub box_pub: String,
    pub seq: u64,
    pub server_user: String,
    pub config: EndpointConfig,
    pub paired_at: String,
    #[serde(default)]
    pub stopped: bool,
    /// Retención en el servidor: "frecuente" (predeterminada), "equilibrada", "ligera" o "todo".
    #[serde(default = "default_retention")]
    pub retention: String,
    #[serde(default)]
    pub last_prune: Option<String>,
    /// Resultado de la última retención (vacío si fue bien).
    #[serde(default)]
    pub last_prune_error: Option<String>,
}

fn default_retention() -> String {
    "frecuente".into()
}

/// La política de cada opción de retención (como los ajustes de la app).
pub fn retention_policy(preset: &str) -> Option<crate::retention::Policy> {
    use crate::retention::{Policy, UNLIMITED};
    match preset {
        "frecuente" => {
            Some(Policy { keep_within_hourly: Some("15d".into()), keep_within_daily: Some("1y".into()), keep_monthly: UNLIMITED, ..Default::default() })
        }
        "equilibrada" => Some(Policy {
            keep_within_daily: Some("30d".into()),
            keep_within_weekly: Some("6m".into()),
            keep_within_monthly: Some("2y".into()),
            ..Default::default()
        }),
        "ligera" => Some(Policy { keep_within_daily: Some("60d".into()), keep_monthly: UNLIMITED, ..Default::default() }),
        _ => None,
    }
}

/// Lo que ve la interfaz (sin contraseñas).
#[derive(Serialize, Clone, Debug)]
pub struct EndpointInfo {
    pub device_id: String,
    pub name: String,
    pub server_user: String,
    pub location: String,
    pub plans: Vec<crate::plans::Plan>,
    pub paired_at: String,
    pub stopped: bool,
    pub tray: bool,
    pub tray_toasts: bool,
    pub retention: String,
    pub last_prune: Option<String>,
    pub last_prune_error: Option<String>,
    /// De la carpeta del servidor (sin abrir el repositorio): la última
    /// versión guardada, cuántas hay y cuánto ocupan.
    pub last_snapshot: Option<String>,
    pub snapshots: usize,
    pub bytes: u64,
}

impl From<&ManagedEndpoint> for EndpointInfo {
    fn from(e: &ManagedEndpoint) -> Self {
        Self {
            device_id: e.device_id.clone(),
            name: e.name.clone(),
            server_user: e.server_user.clone(),
            location: e.config.location.clone(),
            plans: e.config.plans.clone(),
            paired_at: e.paired_at.clone(),
            stopped: e.stopped,
            tray: e.config.tray,
            tray_toasts: e.config.tray_toasts,
            retention: e.retention.clone(),
            last_prune: e.last_prune.clone(),
            last_prune_error: e.last_prune_error.clone(),
            last_snapshot: None,
            snapshots: 0,
            bytes: 0,
        }
    }
}

/// Carpeta del repositorio de un equipo en el Servidor de copias.
fn repo_dir(server: &crate::server::ServerConfig, user: &str) -> std::path::PathBuf {
    std::path::Path::new(&server.path).join(user).join("equipo")
}

fn dir_size(dir: &std::path::Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map(|m| m.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

/// Última versión, número de versiones y tamaño, mirando solo los archivos
/// del repositorio en el servidor (sin contraseña: los nombres y las fechas
/// de los archivos no revelan nada de su contenido).
fn folder_facts(info: &mut EndpointInfo, server: &crate::server::ServerConfig) {
    let dir = repo_dir(server, &info.server_user);
    let mut last: Option<std::time::SystemTime> = None;
    if let Ok(entries) = std::fs::read_dir(dir.join("snapshots")) {
        for e in entries.flatten() {
            info.snapshots += 1;
            if let Ok(t) = e.metadata().and_then(|m| m.modified()) {
                last = Some(last.map_or(t, |l: std::time::SystemTime| l.max(t)));
            }
        }
    }
    info.last_snapshot = last.map(|t| chrono::DateTime::<chrono::Local>::from(t).to_rfc3339());
    info.bytes = dir_size(&dir);
}

/// La IP de la red local con la que los equipos llegan al servidor.
fn server_ip() -> Option<String> {
    crate::server::lan_addresses().into_iter().next()
}

/// `rest:https://<ip>:<puerto>/…`: la IP de una ubicación.
fn location_host(location: &str) -> Option<&str> {
    location.strip_prefix("rest:https://")?.split(['/', ':']).next()
}

/// Si la IP del servidor cambió, manda a cada equipo su ubicación nueva
/// (firmada, como cualquier configuración). Devuelve cuántos se actualizaron.
pub fn refresh_locations() -> Result<usize, String> {
    let server = crate::server::load();
    let Some(ip) = server_ip().filter(|_| server.enabled) else { return Ok(0) };
    let mut vault = share::load()?;
    if vault.endpoints.values().all(|e| e.stopped || location_host(&e.config.location) == Some(ip.as_str())) {
        return Ok(0);
    }
    let key = console_key(&mut vault)?;
    let mut n = 0;
    for e in vault.endpoints.values_mut().filter(|e| !e.stopped) {
        let Some(old) = location_host(&e.config.location).map(str::to_string) else { continue };
        if old == ip {
            continue;
        }
        e.config.location = e.config.location.replacen(&old, &ip, 1);
        e.config.tls_cert_pem = std::fs::read_to_string(crate::server::cert_file()).ok();
        let body = serde_json::to_value(&e.config).map_err(|err| err.to_string())?;
        match send(e, &key, "config", body) {
            Ok(()) => n += 1,
            Err(err) => crate::agent::log(&format!("No se pudo mandar la IP nueva del servidor a «{}»: {err}", e.name)),
        }
    }
    share::save(&vault)?;
    if n > 0 {
        crate::agent::log(&format!("La IP del Servidor de copias cambió a {ip}: ubicación nueva enviada a {n} equipos gestionados."));
    }
    Ok(n)
}

/// Retención de un equipo en el servidor: `forget --prune` sobre su carpeta
/// (en este mismo equipo, sin pasar por el rest-server de solo añadir).
pub fn prune(device_id: &str) -> Result<(), String> {
    let (name, preset, user, password) = {
        let vault = share::load()?;
        let e = vault.endpoints.get(device_id).ok_or("Ese equipo no está gestionado por esta consola.")?;
        (e.name.clone(), e.retention.clone(), e.server_user.clone(), e.config.repo_password.clone())
    };
    let Some(policy) = retention_policy(&preset) else { return Ok(()) };
    let server = crate::server::load();
    let dir = repo_dir(&server, &user);
    let result = if dir.join("config").is_file() {
        let access = crate::restic::Access::new(dir.display().to_string(), password);
        let mut args: Vec<String> = vec!["forget".into(), "--prune".into(), "--retry-lock".into(), "30m".into()];
        args.extend(policy.args());
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        match crate::restic::run_raw(&access, &refs, std::time::Duration::from_secs(6 * 3600)) {
            Ok(out) if out.code == Some(0) => Ok(()),
            Ok(out) => Err(crate::restic::exit_error(out.code, &out.stderr)),
            Err(e) => Err(e),
        }
    } else {
        Ok(()) // aún sin copias
    };
    let mut vault = share::load()?;
    if let Some(e) = vault.endpoints.get_mut(device_id) {
        e.last_prune = Some(chrono::Local::now().to_rfc3339());
        e.last_prune_error = result.as_ref().err().cloned();
    }
    share::save(&vault)?;
    match &result {
        Ok(()) => crate::agent::log(&format!("Retención aplicada a las copias de «{name}» en el servidor.")),
        Err(e) => crate::agent::log(&format!("ERROR: retención de las copias de «{name}» en el servidor: {e}")),
    }
    result
}

/// Equipos cuya retención toca (una vez por semana).
pub fn prune_due() -> Vec<String> {
    let Ok(vault) = share::load() else { return Vec::new() };
    let now = chrono::Local::now();
    vault
        .endpoints
        .values()
        .filter(|e| retention_policy(&e.retention).is_some())
        .filter(|e| {
            e.last_prune
                .as_deref()
                .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                .is_none_or(|t| now.signed_duration_since(t) > chrono::Duration::days(7))
        })
        .map(|e| e.device_id.clone())
        .collect()
}

/// Lo que hace el agente de la consola en cada vuelta: IP nueva a los
/// equipos y, si toca, la retención en otro proceso (puede tardar).
pub fn agent_hook() {
    if !crate::server::load().enabled {
        return;
    }
    let has = share::load().map(|v| !v.endpoints.is_empty()).unwrap_or(false);
    if !has {
        return;
    }
    if let Err(e) = refresh_locations() {
        crate::agent::log(&format!("Equipos gestionados: {e}"));
    }
    if !prune_due().is_empty() {
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::process::Command::new(exe).arg("--managed-maintenance").stdin(std::process::Stdio::null()).spawn();
        }
    }
}

/// `resguardo.exe --managed-maintenance`: la retención que toque, de uno en uno.
pub fn maintenance_main() -> i32 {
    // Un solo proceso a la vez.
    let lock = crate::agent::private_dir().join("gestionados-mantenimiento.lock");
    let fresh = std::fs::metadata(&lock).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age.as_secs() < 12 * 3600);
    if fresh {
        return 0;
    }
    let _ = std::fs::write(&lock, std::process::id().to_string());
    for id in prune_due() {
        let _ = prune(&id);
    }
    let _ = std::fs::remove_file(&lock);
    0
}

fn link_and_secret() -> Result<(crate::web::WebLink, String), String> {
    let link = crate::web::load_link().filter(|l| !l.revoked).ok_or("Vincula antes este equipo con Resguardo Web (Ajustes → Este equipo).")?;
    let secret = crate::web::device_secret(&crate::agent::load_secrets()?).ok_or("Falta el secreto del equipo: vuelve a vincularlo con la web.")?;
    Ok((link, secret))
}

fn rpc(function: &str, extra: serde_json::Value) -> Result<serde_json::Value, String> {
    let (link, secret) = link_and_secret()?;
    let mut body = json!({ "p_device": link.device_id, "p_secret": secret });
    if let (Some(b), Some(e)) = (body.as_object_mut(), extra.as_object()) {
        for (k, v) in e {
            b.insert(k.clone(), v.clone());
        }
    }
    crate::web::rpc(&link.url, &link.key, function, &body)
}

/// Clave de firma de la consola (la crea y la publica si hace falta).
fn console_key(vault: &mut share::Vault) -> Result<String, String> {
    if vault.console_key.is_none() {
        vault.console_key = Some(managed::new_signing_key());
        vault.console_key_published = false;
    }
    let key = vault.console_key.clone().unwrap();
    if !vault.console_key_published {
        rpc("managed_console_set_key", json!({ "p_public_key": managed::verifying_of_signing(&key)? }))?;
        vault.console_key_published = true;
    }
    Ok(key)
}

pub fn list() -> Result<Vec<EndpointInfo>, String> {
    // De paso, si la IP del servidor cambió, la ubicación nueva a los equipos.
    if let Err(e) = refresh_locations() {
        crate::agent::log(&format!("Equipos gestionados: {e}"));
    }
    let vault = share::load()?;
    let server = crate::server::load();
    let mut out: Vec<EndpointInfo> = vault
        .endpoints
        .values()
        .map(|e| {
            let mut info = EndpointInfo::from(e);
            folder_facts(&mut info, &server);
            info
        })
        .collect();
    out.sort_by_key(|e| e.name.to_lowercase());
    Ok(out)
}

#[derive(Serialize)]
pub struct PairingStart {
    pub pairing_id: String,
    pub code: String,
    pub expires_at: String,
}

pub fn pair_start() -> Result<PairingStart, String> {
    if !crate::server::load().enabled {
        return Err("Activa antes el Servidor de copias (Ajustes → Este equipo): los equipos gestionados copian en él.".into());
    }
    let mut vault = share::load()?;
    console_key(&mut vault)?;
    share::save(&vault)?;
    let code = managed::pairing_code();
    let answer = rpc("managed_pairing_open", json!({ "p_code_hash": managed::code_hash(&code) }))?;
    Ok(PairingStart {
        pairing_id: answer["id"].as_str().unwrap_or_default().to_string(),
        code,
        expires_at: answer["expires_at"].as_str().unwrap_or_default().to_string(),
    })
}

#[derive(Serialize)]
pub struct PairingPoll {
    pub status: String,
    /// Hasta cuándo se puede confirmar.
    pub expires_at: Option<String>,
    pub endpoint_name: Option<String>,
    /// Código de comprobación de 6 cifras (cuando el equipo ya se unió).
    pub sas: Option<String>,
}

pub fn pair_poll(pairing_id: &str) -> Result<PairingPoll, String> {
    let vault = share::load()?;
    let key = vault.console_key.ok_or("Falta la clave de la consola.")?;
    let answer = rpc("managed_pairing_status", json!({ "p_pairing": pairing_id }))?;
    let status = answer["status"].as_str().unwrap_or("open").to_string();
    let box_key = answer["endpoint_box_key"].as_str();
    Ok(PairingPoll {
        status,
        expires_at: answer["expires_at"].as_str().map(str::to_string),
        endpoint_name: answer["endpoint_name"].as_str().map(str::to_string),
        sas: box_key.map(|b| managed::sas(&managed::verifying_of_signing(&key).unwrap_or_default(), b)),
    })
}

/// Tras comprobar el código SAS: confirma, crea su usuario y su repositorio en
/// este Servidor de copias y le manda la primera configuración firmada.
pub fn pair_confirm(pairing_id: &str) -> Result<EndpointInfo, String> {
    let answer = rpc("managed_pairing_status", json!({ "p_pairing": pairing_id }))?;
    let device_id = answer["endpoint_device"].as_str().ok_or("El equipo aún no se ha unido.")?.to_string();
    let box_pub = answer["endpoint_box_key"].as_str().ok_or("El equipo aún no se ha unido.")?.to_string();
    let name = answer["endpoint_name"].as_str().unwrap_or("Equipo").to_string();
    rpc("managed_pairing_confirm", json!({ "p_pairing": pairing_id }))?;

    // Usuario del servidor y repositorio del equipo (contraseña generada; la consola puede restaurar).
    let mut server = crate::server::load();
    let (user, rest_password) = crate::server::create_user(&mut server, &name)?;
    crate::server::save(&server)?;
    let ip = crate::server::lan_addresses().into_iter().next().unwrap_or_else(|| "localhost".into());
    let location = format!("rest:https://{ip}:{}/{user}/equipo/", server.port);
    let repo_password = crate::server::new_password() + &crate::server::new_password();
    // El repositorio se crea desde aquí (por la propia máquina del servidor).
    let local = crate::restic::Access {
        location: format!("rest:https://localhost:{}/{user}/equipo/", server.port),
        password: repo_password.clone(),
        rest_auth: Some((user.clone(), rest_password.clone())),
        cacert: Some(crate::server::cert_file().display().to_string()),
        env: Vec::new(),
    };
    crate::restic::run_with(&local, &["init"], crate::restic::CHECK_TIMEOUT, None)?;

    let config = EndpointConfig {
        location,
        rest_user: user.clone(),
        rest_password,
        repo_password,
        tls_cert_pem: std::fs::read_to_string(crate::server::cert_file()).ok(),
        plans: Vec::new(),
        tray: true,
        tray_toasts: false,
    };
    let mut endpoint = ManagedEndpoint {
        device_id: device_id.clone(),
        name,
        box_pub,
        seq: 0,
        server_user: user,
        config,
        paired_at: chrono::Local::now().to_rfc3339(),
        stopped: false,
        retention: default_retention(),
        last_prune: None,
        last_prune_error: None,
    };
    let mut vault = share::load()?;
    let key = console_key(&mut vault)?;
    let body = serde_json::to_value(&endpoint.config).map_err(|e| e.to_string())?;
    send(&mut endpoint, &key, "config", body)?;
    let info = EndpointInfo::from(&endpoint);
    vault.endpoints.insert(device_id, endpoint);
    share::save(&vault)?;
    Ok(info)
}

/// El código SAS no coincide (o se cancela): la web borra el equipo sin confirmar.
pub fn pair_cancel(pairing_id: &str) -> Result<(), String> {
    rpc("managed_pairing_cancel", json!({ "p_pairing": pairing_id })).map(|_| ())
}

/// Firma, sella y manda un mensaje (con el siguiente `seq`).
fn send(e: &mut ManagedEndpoint, key: &str, kind: &str, body: serde_json::Value) -> Result<(), String> {
    let msg = Message { v: 1, endpoint: e.device_id.clone(), seq: e.seq + 1, issued_at: chrono::Local::now().to_rfc3339(), kind: kind.into(), body };
    let ct = managed::seal_message(&msg, key, &e.box_pub)?;
    rpc("managed_send", json!({ "p_endpoint": e.device_id, "p_seq": msg.seq, "p_ciphertext": ct }))?;
    e.seq = msg.seq;
    Ok(())
}

fn with_endpoint<T>(device_id: &str, f: impl FnOnce(&mut ManagedEndpoint, &str) -> Result<T, String>) -> Result<T, String> {
    let mut vault = share::load()?;
    let key = console_key(&mut vault)?;
    let e = vault.endpoints.get_mut(device_id).ok_or("Ese equipo no está gestionado por esta consola.")?;
    let out = f(e, &key)?;
    share::save(&vault)?;
    Ok(out)
}

/// Asigna las copias de un equipo (carpetas, exclusiones, horario) y su bandeja.
pub fn set_config(device_id: &str, plans: Vec<crate::plans::Plan>, tray: bool, tray_toasts: bool) -> Result<EndpointInfo, String> {
    for p in &plans {
        p.validate()?;
    }
    with_endpoint(device_id, |e, key| {
        e.config.plans = plans;
        e.config.tray = tray;
        e.config.tray_toasts = tray_toasts;
        let body = serde_json::to_value(&e.config).map_err(|err| err.to_string())?;
        send(e, key, "config", body)?;
        Ok(EndpointInfo::from(&*e))
    })
}

/// Cambia la retención de un equipo en el servidor (no se manda al equipo:
/// la aplica esta consola).
pub fn set_retention(device_id: &str, preset: &str) -> Result<EndpointInfo, String> {
    if !matches!(preset, "frecuente" | "equilibrada" | "ligera" | "todo") {
        return Err("Retención no válida.".into());
    }
    let mut vault = share::load()?;
    let e = vault.endpoints.get_mut(device_id).ok_or("Ese equipo no está gestionado por esta consola.")?;
    e.retention = preset.to_string();
    let mut info = EndpointInfo::from(&*e);
    share::save(&vault)?;
    folder_facts(&mut info, &crate::server::load());
    Ok(info)
}

pub fn backup_now(device_id: &str, plan_id: &str) -> Result<(), String> {
    with_endpoint(device_id, |e, key| send(e, key, "backup_now", json!({ "plan": plan_id })))
}

/// Deja de gestionar un equipo: deja de copiar. Volver a emparejarlo exige un administrador local.
pub fn unpair(device_id: &str) -> Result<EndpointInfo, String> {
    with_endpoint(device_id, |e, key| {
        send(e, key, "unpair", json!({}))?;
        e.stopped = true;
        Ok(EndpointInfo::from(&*e))
    })
}

/// Para restaurar desde la consola: los datos del repositorio de un equipo
/// (por localhost; con la contraseña que generó la consola).
pub fn repo_access(device_id: &str) -> Result<(String, crate::restic::Access), String> {
    let vault = share::load()?;
    let e = vault.endpoints.get(device_id).ok_or("Ese equipo no está gestionado por esta consola.")?;
    let server = crate::server::load();
    Ok((
        e.name.clone(),
        crate::restic::Access {
            location: format!("rest:https://localhost:{}/{}/equipo/", server.port, e.server_user),
            password: e.config.repo_password.clone(),
            rest_auth: Some((e.config.rest_user.clone(), e.config.rest_password.clone())),
            cacert: Some(crate::server::cert_file().display().to_string()),
            env: Vec::new(),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ip_de_la_ubicacion() {
        assert_eq!(location_host("rest:https://192.168.1.20:8000/pc/equipo/"), Some("192.168.1.20"));
        assert_eq!(location_host("rest:https://localhost/pc/"), Some("localhost"));
        assert_eq!(location_host("s3:https://x"), None);
        for p in ["frecuente", "equilibrada", "ligera"] {
            let policy = retention_policy(p).unwrap();
            assert!(!policy.is_empty() && policy.validate().is_ok());
        }
        assert!(retention_policy("todo").is_none());
    }
}
