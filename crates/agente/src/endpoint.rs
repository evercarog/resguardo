//! El lado del equipo gestionado (fase 5, docs/agente-gestionado.md): lo que
//! hace `resguardo-agente.exe`. Guarda la clave de la consola fijada al
//! emparejarse, aplica solo lo que ella firma y deja el resto al agente de
//! siempre (copias, reintentos, VSS, historial e informe a la web).

use serde::{Deserialize, Serialize};
use serde_json::json;

/// Id del repositorio del equipo gestionado en la configuración del agente.
pub const REPO_ID: &str = "gestionado";

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct EndpointState {
    pub console_device: String,
    pub console_name: String,
    /// Clave Ed25519 de la consola, fijada al emparejar (base64).
    pub console_sign_key: String,
    /// Privada X25519 de este equipo (base64).
    pub box_key: String,
    /// Último mensaje aceptado.
    pub last_seq: u64,
    /// La consola ordenó dejar de copiar (solo un administrador local empareja de nuevo).
    #[serde(default)]
    pub stopped: bool,
    #[serde(default)]
    pub config: Option<crate::managed::EndpointConfig>,
}

fn state_file() -> std::path::PathBuf {
    crate::agent::private_dir().join("gestionado.bin")
}

pub fn load() -> Option<EndpointState> {
    let enc = std::fs::read(state_file()).ok()?;
    serde_json::from_slice(&crate::platform::unprotect(&enc).ok()?).ok()
}

pub fn save(s: &EndpointState) -> Result<(), String> {
    crate::agent::prepare_dir()?;
    let enc = crate::platform::protect(&serde_json::to_vec(s).map_err(|e| e.to_string())?)?;
    let tmp = crate::agent::private_dir().join("gestionado.bin.tmp");
    let _ = std::fs::remove_file(&tmp);
    std::fs::write(&tmp, enc).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, state_file()).map_err(|e| e.to_string())
}

/// `--pair <código>` (administrador local): se une a la consola y fija su clave.
/// Devuelve el código de comprobación (SAS) que hay que comparar con la consola.
pub fn pair(url: &str, key: &str, code: &str, device_name: &str) -> Result<(String, String), String> {
    crate::agent::require_admin()?;
    if load().is_some() {
        return Err("Este equipo ya está emparejado con una consola. Para cambiarla, primero «resguardo-agente.exe --unpair».".into());
    }
    let box_key = crate::share::new_key();
    let box_pub = crate::share::public_of(&box_key)?;
    let answer = crate::web::rpc(
        url,
        key,
        "managed_pairing_join",
        &json!({
            "p_code_hash": crate::managed::code_hash(code),
            "p_box_key": box_pub,
            "p_name": device_name,
            "p_os": "Windows",
            "p_app_version": crate::version_programa(),
        }),
    )?;
    // Un código mal escrito o caducado no es un error de la web: lo dice en la respuesta.
    if answer.get("error").is_some() {
        return Err(answer["message"].as_str().unwrap_or("Código no válido o caducado").to_string());
    }
    let get = |k: &str| answer.get(k).and_then(|v| v.as_str()).map(str::to_string).ok_or_else(|| format!("La web no devolvió «{k}»."));
    let (device_id, device_secret) = (get("device_id")?, get("secret")?);
    let state = EndpointState {
        console_device: get("console_device")?,
        console_name: get("console_name").unwrap_or_else(|_| "la consola".into()),
        console_sign_key: get("console_sign_key")?,
        box_key,
        ..Default::default()
    };
    // El vínculo con la web (como el de cualquier equipo) para recibir y para informar.
    crate::web::store_link(url, key, &device_id, device_name, &device_secret)?;
    let sas = crate::managed::sas(&state.console_sign_key, &box_pub);
    let console = state.console_name.clone();
    save(&state)?;
    crate::agent::log(&format!("Emparejado con la consola «{console}». Código de comprobación: {sas}."));
    Ok((console, sas))
}

/// `--unpair` (administrador local): olvida la consola y deja de copiar.
pub fn unpair() -> Result<(), String> {
    crate::agent::require_admin()?;
    let _ = std::fs::remove_file(state_file());
    let _ = crate::agent::set_schedule_by_id(REPO_ID, None);
    crate::agent::log("Desemparejado de la consola: este equipo deja de copiar.");
    Ok(())
}

/// Aplica una configuración firmada: el repositorio y las copias, en el agente.
fn apply_config(c: &crate::managed::EndpointConfig, console_name: &str) -> Result<(), String> {
    let cacert = match &c.tls_cert_pem {
        Some(pem) => {
            let path = crate::agent::agent_dir().join("gestionado-ca.pem");
            std::fs::write(&path, pem).map_err(|e| e.to_string())?;
            Some(path.display().to_string())
        }
        None => None,
    };
    let repo: crate::store::Repo = serde_json::from_value(json!({
        "id": REPO_ID,
        "name": format!("Copias en «{console_name}»"),
        "location": c.location,
        "rest_username": c.rest_user,
        "cacert": cacert,
        "plans": c.plans,
    }))
    .map_err(|e| e.to_string())?;
    let access = crate::restic::Access {
        location: c.location.clone(),
        password: c.repo_password.clone(),
        rest_auth: Some((c.rest_user.clone(), c.rest_password.clone())),
        cacert,
        env: Vec::new(),
    };
    let schedule = if c.plans.iter().any(|p| p.schedule.is_some()) { Some(crate::agent::Schedule::Plans) } else { None };
    crate::agent::set_schedule(&repo, Some(&access), schedule)
}

/// Una vuelta del servicio: recoge y aplica lo que firmó la consola.
pub fn tick() -> Vec<String> {
    let mut notes = Vec::new();
    let Some(mut state) = load() else { return notes };
    let Some(link) = crate::web::load_link().filter(|l| !l.revoked) else { return notes };
    let Ok(secrets) = crate::agent::load_secrets() else { return notes };
    let Some(secret) = crate::web::device_secret(&secrets) else { return notes };
    let Ok(answer) = crate::web::rpc(&link.url, &link.key, "managed_take", &json!({ "p_device": link.device_id, "p_secret": secret })) else {
        return notes;
    };
    let list = answer.as_array().cloned().unwrap_or_default();
    let mut dirty = false;
    for item in list {
        // Solo lo de la consola con la que se emparejó (además de su firma, que se comprueba al abrirlo).
        if item.get("console_device").and_then(|v| v.as_str()).is_some_and(|c| c != state.console_device) {
            notes.push("Mensaje de otra consola: se ignora.".into());
            continue;
        }
        let Some(ct) = item.get("ciphertext").and_then(|v| v.as_str()) else { continue };
        match crate::managed::open_message(ct, &state.box_key, &state.console_sign_key, &link.device_id, state.last_seq, chrono::Local::now()) {
            Ok(msg) => {
                state.last_seq = msg.seq;
                dirty = true;
                match msg.kind.as_str() {
                    "config" if !state.stopped => match serde_json::from_value::<crate::managed::EndpointConfig>(msg.body.clone()) {
                        Ok(c) => match apply_config(&c, &state.console_name) {
                            Ok(()) => {
                                notes.push(format!("Configuración recibida de «{}» ({} copias).", state.console_name, c.plans.len()));
                                state.config = Some(c);
                            }
                            Err(e) => notes.push(format!("No se pudo aplicar la configuración de la consola: {e}")),
                        },
                        Err(e) => notes.push(format!("Configuración no válida: {e}")),
                    },
                    "backup_now" if !state.stopped => {
                        if let Some(plan) = msg.body.get("plan").and_then(|v| v.as_str()) {
                            match crate::agent::request_backup(REPO_ID, plan) {
                                Ok(()) => notes.push("La consola pidió «Copiar ahora».".into()),
                                Err(e) => notes.push(format!("«Copiar ahora» de la consola: {e}")),
                            }
                        }
                    }
                    "unpair" => {
                        state.stopped = true;
                        let _ = crate::agent::set_schedule_by_id(REPO_ID, None);
                        notes.push(format!("«{}» dejó de gestionar este equipo: ya no se copia.", state.console_name));
                    }
                    other => notes.push(format!("Orden «{other}» ignorada.")),
                }
            }
            Err(e) => notes.push(e),
        }
    }
    if dirty {
        if let Err(e) = save(&state) {
            notes.push(format!("ERROR: {e}"));
        }
    }
    notes
}
