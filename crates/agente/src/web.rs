//! Conexión con Resguardo Web (Supabase): vincular este equipo y enviar su
//! informe de estado.
//!
//! Qué se envía: nombre y tipo de cada repositorio programado, servidor (sin
//! credenciales), horario, número de snapshots, fecha, duración y tamaño de
//! la última copia, y el resultado de la última copia automática (su mensaje
//! de error, sin rutas: ver `public_message`) y si sus copias automáticas
//! están en pausa. Nunca se envían contraseñas, rutas locales ni nombres de
//! archivos.
//!
//! El secreto del equipo (que la web solo conoce por su huella) se guarda con
//! los demás secretos del agente, cifrado con DPAPI.

use crate::agent::{self, AgentConfig, AgentRepo, AgentState, Secret};
use crate::restic::{self, Access};
use chrono::{DateTime, Duration, Local};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Clave del secreto del equipo dentro de `secrets.bin`.
const SECRET_KEY: &str = "__web__";
/// Informe periódico aunque no haya copias (latido). El agente se despierta
/// cada 5 minutos; el margen evita que una diferencia de décimas de segundo
/// retrase el informe a la siguiente vuelta.
const REPORT_EVERY: Duration = Duration::seconds(9 * 60 + 30);
/// Snapshots por repositorio que se envían a la web (los más recientes).
const SNAPSHOTS_SENT: usize = 400;
/// Consultar la lista de snapshots de cada repositorio como mucho cada hora.
const SNAPSHOTS_EVERY: Duration = Duration::minutes(60);

/// Dirección y clave publicable de la web por defecto (se fijan al compilar
/// con RESGUARDO_WEB_URL / RESGUARDO_WEB_KEY; ver src-tauri/.cargo/config.toml).
pub fn defaults() -> (String, String) {
    (option_env!("RESGUARDO_WEB_URL").unwrap_or_default().to_string(), option_env!("RESGUARDO_WEB_KEY").unwrap_or_default().to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebLink {
    /// URL del proyecto de Supabase (https://….supabase.co).
    pub url: String,
    /// Clave publicable (pública por diseño).
    pub key: String,
    pub device_id: String,
    pub device_name: String,
    pub paired_at: String,
    /// La web desvinculó este equipo.
    #[serde(default)]
    pub revoked: bool,
}

/// Resumen de snapshots de un repositorio, guardado entre ejecuciones.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SnapSummary {
    pub queried_at: String,
    /// La lista de snapshots ya llegó a la web al menos una vez.
    #[serde(default)]
    pub list_sent: bool,
    pub count: usize,
    pub last_time: Option<String>,
    pub last_duration_s: Option<f64>,
    pub last_data_added: Option<u64>,
    pub last_total_bytes: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebState {
    pub last_report: Option<String>,
    pub last_error: Option<String>,
    #[serde(default)]
    pub snapshots: HashMap<String, SnapSummary>,
}

pub fn load_link() -> Option<WebLink> {
    agent::read_json::<Option<WebLink>>("web.json")
}

fn validate(url: &str, key: &str) -> Result<(), String> {
    let local = url.starts_with("http://localhost") || url.starts_with("http://127.0.0.1");
    if !(url.starts_with("https://") || local) {
        return Err("La dirección de la web debe empezar por https://".into());
    }
    if key.trim().is_empty() || key.len() > 300 {
        return Err("Falta la clave publicable de la web.".into());
    }
    Ok(())
}

/// TLS con las autoridades del sistema. En Windows, su almacén de
/// certificados (respeta proxys y CAs corporativas); en Linux, rustls con las
/// autoridades de webpki (sin OpenSSL: el binario no depende de la distribución).
pub fn tls_sistema() -> ureq::tls::TlsConfig {
    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
    #[cfg(windows)]
    {
        TlsConfig::builder().provider(TlsProvider::NativeTls).root_certs(RootCerts::PlatformVerifier).build()
    }
    #[cfg(not(windows))]
    {
        TlsConfig::builder().provider(TlsProvider::Rustls).root_certs(RootCerts::WebPki).build()
    }
}

/// Llama a una función RPC de Supabase con la clave publicable.
pub fn rpc(url: &str, key: &str, function: &str, body: &Value) -> Result<Value, String> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(30)))
        .http_status_as_error(false)
        .tls_config(tls_sistema())
        .build()
        .new_agent();
    let endpoint = format!("{}/rest/v1/rpc/{function}", url.trim_end_matches('/'));
    let mut resp = agent.post(&endpoint).header("apikey", key).send_json(body).map_err(|e| format!("No se pudo conectar con la web: {e}"))?;
    let status = resp.status().as_u16();
    let value: Value = resp.body_mut().read_json().unwrap_or(Value::Null);
    if status >= 400 {
        let msg = value["message"].as_str().unwrap_or("respuesta inesperada");
        return Err(format!("La web respondió: {msg}"));
    }
    Ok(value)
}

pub fn os_label() -> String {
    format!("{} {}", if cfg!(windows) { "Windows" } else { std::env::consts::OS }, std::env::consts::ARCH)
}

/// Secreto del equipo con la web (si está vinculado).
pub fn device_secret(secrets: &HashMap<String, Secret>) -> Option<String> {
    secrets.get(SECRET_KEY).map(|s| s.password.clone())
}

pub fn default_device_name() -> String {
    std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| "Equipo".into())
}

/// Vincula este equipo con el código que muestra la web. Requiere administrador.
pub fn pair(url: &str, key: &str, code: &str, name: &str) -> Result<WebLink, String> {
    agent::require_admin()?;
    let (url, key) = (url.trim().trim_end_matches('/'), key.trim());
    validate(url, key)?;
    let code: String = code.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_uppercase();
    if code.len() != 8 {
        return Err("El código tiene 8 caracteres (por ejemplo ABCD-EFGH).".into());
    }
    let name = if name.trim().is_empty() { default_device_name() } else { name.trim().to_string() };

    let answer = rpc(url, key, "device_pair", &json!({ "p_code": code, "p_name": name, "p_os": os_label(), "p_app_version": crate::version_programa() }))?;
    let (Some(device_id), Some(secret)) = (answer["device_id"].as_str(), answer["secret"].as_str()) else {
        return Err("La web no devolvió los datos del equipo.".into());
    };

    let mut secrets = agent::load_secrets()?;
    secrets.insert(SECRET_KEY.into(), Secret { password: secret.into(), ..Default::default() });
    agent::prepare_dir()?;
    agent::save_secrets(&secrets)?;
    let link =
        WebLink { url: url.into(), key: key.into(), device_id: device_id.into(), device_name: name, paired_at: Local::now().to_rfc3339(), revoked: false };
    agent::write_json("web.json", &Some(link.clone()))?;
    agent::sync_task()?;
    agent::log(&format!("Equipo vinculado con la web como «{}».", link.device_name));

    // Primer informe enseguida, para que aparezca en la web sin esperar.
    let _ = report_now();
    Ok(link)
}

/// Guarda el vínculo de un equipo creado al emparejarse con una consola
/// (equipos gestionados): igual que `pair`, sin crear la tarea del agente
/// (la lleva el servicio de Resguardo Agente).
pub fn store_link(url: &str, key: &str, device_id: &str, name: &str, secret: &str) -> Result<(), String> {
    agent::require_admin()?;
    let mut secrets = agent::load_secrets()?;
    secrets.insert(SECRET_KEY.into(), Secret { password: secret.into(), ..Default::default() });
    agent::prepare_dir()?;
    agent::save_secrets(&secrets)?;
    let link = WebLink {
        url: url.trim().trim_end_matches('/').into(),
        key: key.trim().into(),
        device_id: device_id.into(),
        device_name: name.into(),
        paired_at: Local::now().to_rfc3339(),
        revoked: false,
    };
    agent::write_json("web.json", &Some(link))
}

/// Desvincula este equipo (deja de enviar informes). Requiere administrador.
pub fn unpair() -> Result<(), String> {
    agent::require_admin()?;
    let mut secrets = agent::load_secrets()?;
    secrets.remove(SECRET_KEY);
    agent::save_secrets(&secrets)?;
    agent::write_json::<Option<WebLink>>("web.json", &None)?;
    agent::sync_task()?;
    agent::log("Equipo desvinculado de la web.");
    Ok(())
}

/// Envía un informe ahora mismo (desde la app, como administrador).
pub fn report_now() -> Result<(), String> {
    let config = agent::load_config();
    let secrets = agent::load_secrets()?;
    let mut state = agent::load_state();
    let ids: Vec<String> = config.repos.iter().map(|r| r.id.clone()).collect();
    let result = tick(&config, &secrets, &mut state, &ids, true);
    agent::write_json("state.json", &state)?;
    result
}

/// `servidor[:puerto]` de una ubicación de restic, sin usuario ni contraseña.
/// Las ubicaciones locales no envían nada (su ruta podría ser sensible).
fn kind_and_host(location: &str) -> (&'static str, Option<String>) {
    let Some((prefix, rest)) = location.split_once(':') else { return ("local", None) };
    let kind = match prefix {
        "rest" => "rest",
        "sftp" => "sftp",
        "s3" => "s3",
        "b2" => "b2",
        "azure" => "azure",
        "gs" => "gs",
        "rclone" => "rclone",
        p if p.len() == 1 => return ("local", None), // C:\…
        _ => "other",
    };
    let host = match kind {
        "rest" | "s3" => {
            let rest = rest.trim_start_matches("https://").trim_start_matches("http://");
            rest.split('/').next().map(|h| h.rsplit_once('@').map_or(h, |(_, h)| h).to_string())
        }
        "sftp" => rest.split(':').next().map(|h| h.rsplit_once('@').map_or(h, |(_, h)| h).to_string()),
        _ => None,
    };
    (kind, host.filter(|h| !h.is_empty()))
}

fn access_for(repo: &AgentRepo, secret: &Secret) -> Access {
    Access {
        location: repo.location.clone(),
        password: secret.password.clone(),
        rest_auth: repo.rest_username.clone().zip(secret.rest_password.clone()),
        cacert: repo.cacert.clone(),
        env: secret.env.clone(),
    }
}

/// Duración de una copia (segundos) según su resumen.
fn duration_s(s: &restic::SnapshotSummary) -> Option<f64> {
    let (a, b) = (s.backup_start.as_deref()?, s.backup_end.as_deref()?);
    let (a, b) = (DateTime::parse_from_rfc3339(a).ok()?, DateTime::parse_from_rfc3339(b).ok()?);
    Some((b - a).num_milliseconds() as f64 / 1000.0)
}

/// Lista de snapshots para la web: solo metadatos (sin rutas ni archivos).
/// Última copia de un repositorio con planes: la más reciente con error si
/// algún plan tiene su última copia fallida; si no, la más reciente.
fn plans_last_run<'a>(repo: &AgentRepo, state: &'a AgentState) -> Option<&'a agent::RunRecord> {
    let runs: Vec<&agent::RunRecord> =
        repo.plans.iter().filter_map(|p| state.runs.get(&crate::plans::plan_key(&repo.id, &p.id))).chain(state.runs.get(&repo.id)).collect();
    runs.iter()
        .filter(|r| r.result == "error")
        .max_by(|a, b| a.finished.cmp(&b.finished))
        .copied()
        .or_else(|| runs.iter().max_by(|a, b| a.finished.cmp(&b.finished)).copied())
}

/// Mensaje de una copia apto para la web: primera línea, sin rutas locales
/// (`C:\...`, `\\servidor\...`) y de longitud acotada.
pub fn public_message(msg: &str) -> String {
    let line = msg.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default();
    let mut out = strip_paths(line);
    if out.chars().count() > 240 {
        out = out.chars().take(240).collect::<String>() + "…";
    }
    out
}

/// Mensaje para los archivos del agente que leen todos los usuarios del
/// equipo (estado, historial, registro): todas sus líneas, sin rutas ni
/// contraseñas. Los nombres de destinos y copias se conservan.
pub fn local_message(msg: &str) -> String {
    msg.lines().map(str::trim).filter(|l| !l.is_empty()).map(strip_paths).collect::<Vec<_>>().join("\n")
}

/// Una línea sin rutas (`[ruta]` en su lugar) ni contraseñas en direcciones.
pub fn strip_paths(line: &str) -> String {
    let line = crate::restic::redact_credentials(line);
    let mut out = String::new();
    let mut words = line.split(' ').peekable();
    // Ruta entre comillas con espacios («/C/a/b c.txt»): hasta la comilla de cierre, todo es ruta.
    let mut closing: Option<char> = None;
    while let Some(word) = words.next() {
        if let Some(close) = closing {
            if let Some(pos) = word.find(close) {
                closing = None;
                // Lo que sigue a la comilla («: acceso denegado») se conserva.
                let after = &word[pos + close.len_utf8()..];
                if !after.is_empty() {
                    out.push_str(after);
                    if words.peek().is_some() {
                        out.push(' ');
                    }
                }
            }
            continue;
        }
        let bare = word.trim_start_matches(['"', '\'', '(', '«']);
        let opening = word.chars().next().filter(|c| matches!(c, '"' | '\'' | '«'));
        let b = bare.as_bytes();
        // Justo después de una ruta, un trozo con barras («fotos/x.jpg») o un
        // nombre con extensión antes de los dos puntos («marzo.xlsx:») es la
        // misma ruta con espacios.
        let after_path = out.ends_with("[ruta] ");
        let continues =
            after_path && !bare.contains("://") && (bare.contains(['/', '\\']) || (bare.ends_with(':') && bare.trim_end_matches(':').contains('.')));
        let is_path = continues
            || (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/'))
            || bare.starts_with(r"\\")
            || bare.starts_with('/')
            || bare.contains('\\');
        if let (true, Some(open)) = (is_path, opening) {
            let close = if open == '«' { '»' } else { open };
            // La comilla de cierre no está en esta palabra: la ruta sigue.
            if !bare.contains(close) {
                closing = Some(close);
            }
        }
        if is_path && after_path {
            // Una ruta con espacios: un solo marcador (con los dos puntos de después).
            if bare.ends_with(':') {
                out.pop();
                out.push_str(": ");
            }
            continue;
        }
        let shown = match (is_path, bare.ends_with(':')) {
            (true, true) => "[ruta]:",
            (true, false) => "[ruta]",
            _ => word,
        };
        out.push_str(shown);
        if words.peek().is_some() {
            out.push(' ');
        }
    }
    out.trim_end().replace("[ruta] [ruta]", "[ruta]")
}

/// Pausa de las copias automáticas para la web: `null` o
/// `{ "since": RFC 3339, "until": RFC 3339 | null }` (null: hasta reanudarlas a mano).
fn paused_json(repo: &AgentRepo, now: DateTime<Local>) -> Value {
    repo.active_pause(now).map_or(Value::Null, |p| json!({ "since": p.since, "until": p.until }))
}

fn snapshot_rows(snaps: &[restic::Snapshot]) -> Vec<Value> {
    let mut sorted: Vec<&restic::Snapshot> = snaps.iter().collect();
    sorted.sort_by(|a, b| b.time.cmp(&a.time));
    sorted
        .into_iter()
        .take(SNAPSHOTS_SENT)
        .map(|s| {
            let sum = s.summary.clone().unwrap_or_default();
            json!({
                "id": s.short_id,
                "time": s.time,
                "hostname": s.hostname,
                "tags": s.tags,
                "duration_s": duration_s(&sum),
                "data_added": sum.data_added_packed.or(sum.data_added),
                "total_bytes": sum.total_bytes_processed,
                "total_files": sum.total_files_processed,
                "files_new": sum.files_new,
                "files_changed": sum.files_changed,
                "files_unmodified": sum.files_unmodified,
            })
        })
        .collect()
}

fn summarize(access: &Access) -> Result<(SnapSummary, Vec<Value>), String> {
    let snaps = restic::snapshots(access)?;
    let rows = snapshot_rows(&snaps);
    let last = snaps.iter().max_by(|a, b| a.time.cmp(&b.time));
    let summary = last.and_then(|s| s.summary.clone()).unwrap_or_default();
    Ok((
        SnapSummary {
            queried_at: Local::now().to_rfc3339(),
            // Se marca al confirmar la web que la recibió.
            list_sent: false,
            count: snaps.len(),
            last_time: last.map(|s| s.time.clone()),
            last_duration_s: duration_s(&summary),
            last_data_added: summary.data_added_packed.or(summary.data_added),
            last_total_bytes: summary.total_bytes_processed,
        },
        rows,
    ))
}

fn older_than(when: Option<&str>, age: Duration) -> bool {
    when.and_then(|t| DateTime::parse_from_rfc3339(t).ok()).is_none_or(|t| Local::now().signed_duration_since(t) >= age)
}

/// Llamado en cada ejecución del agente. `ran` son los repositorios que
/// acaban de hacer copia (se informa enseguida y con datos frescos).
pub fn tick(config: &AgentConfig, secrets: &HashMap<String, Secret>, state: &mut AgentState, ran: &[String], force: bool) -> Result<(), String> {
    send(config, secrets, state, ran, force, false)
}

/// Informe ligero desde el proceso de tareas durante una subida o verificación
/// larga: el avance llega a la web aunque el agente solo informe cada 10 min.
/// No guarda nada en `state.json` (lo escribe el agente).
pub fn report_detached() {
    if !load_link().is_some_and(|l| !l.revoked) {
        return;
    }
    let Ok(secrets) = agent::load_secrets() else { return };
    let config = agent::load_config();
    let mut state = agent::load_state();
    let _ = send(&config, &secrets, &mut state, &[], true, true);
}

/// Informe ligero (empieza una copia automática, avanza una tarea): no
/// consulta restic, usa lo último que se sabe.
pub fn report_started(config: &AgentConfig, secrets: &HashMap<String, Secret>, state: &mut AgentState) {
    let _ = send(config, secrets, state, &[], true, true);
}

fn send(config: &AgentConfig, secrets: &HashMap<String, Secret>, state: &mut AgentState, ran: &[String], force: bool, light: bool) -> Result<(), String> {
    let Some(mut link) = load_link().filter(|l| !l.revoked) else { return Ok(()) };
    if !force && ran.is_empty() && !older_than(state.web.last_report.as_deref(), REPORT_EVERY) {
        return Ok(());
    }
    let Some(device_secret) = secrets.get(SECRET_KEY) else {
        return Err("Falta el secreto del equipo: vuelve a vincularlo con la web.".into());
    };

    let tasks = crate::tasks::load_state();
    let guard = crate::tasks::load_guard();
    // Última ejecución de una tarea, con el mensaje apto para la web.
    let task_run = |kind: &str, id: &str| {
        tasks.runs.get(&crate::tasks::key(kind, id)).map(|r| {
            let mut v = serde_json::to_value(r).unwrap_or(Value::Null);
            v["message"] = json!(public_message(&r.message));
            v
        })
    };
    // Destinos que este equipo comparte con los demás (solo informativo).
    let shared_places: std::collections::HashSet<String> = crate::share::load().map(|v| v.shared.into_keys().collect()).unwrap_or_default();
    let mut repos = Vec::new();
    let mut sent_lists: Vec<(String, bool)> = Vec::new();
    for repo in &config.repos {
        let cached = state.web.snapshots.get(&repo.id);
        // La lista de snapshots solo viaja cuando se acaba de consultar (como mucho cada hora).
        let mut list: Option<Vec<Value>> = None;
        let never_sent = !cached.is_some_and(|c| c.list_sent);
        let due = force || never_sent || ran.contains(&repo.id) || older_than(cached.map(|c| c.queried_at.as_str()), SNAPSHOTS_EVERY);
        if due && !light {
            if let Some(secret) = secrets.get(&repo.id) {
                match summarize(&access_for(repo, secret)) {
                    Ok((s, rows)) => {
                        state.web.snapshots.insert(repo.id.clone(), s);
                        list = Some(rows);
                    }
                    Err(e) => agent::log(&format!("No se pudo leer los snapshots de «{}»: {e}", repo.name)),
                }
            }
        }
        let snap = state.web.snapshots.get(&repo.id).cloned().unwrap_or_default();
        let (kind, host) = kind_and_host(&repo.location);
        let protection = {
            let p = crate::protection::evaluate(
                &crate::protection::agent_facts(&repo.id, &repo.location, config, state, &tasks, &guard, Local::now()),
                Local::now(),
            );
            json!({
                "score": p.score,
                "total": p.total,
                "items": p.items.iter().map(|i| json!({ "id": i.id, "state": i.state, "label": i.label, "detail": i.detail })).collect::<Vec<_>>(),
            })
        };
        repos.push(json!({
            "id": repo.id,
            "name": repo.name,
            "kind": kind,
            "host": host,
            // Destino (lugar) del repositorio: ver docs/destinos.md.
            "place": repo.place_id.as_ref().map(|id| json!({
                "id": id,
                "name": repo.place_name.clone().unwrap_or_else(|| crate::places::default_name(&repo.location)),
                "kind": crate::places::kind(&repo.location),
                "shared": shared_places.contains(id),
            })),
            "schedule": repo.schedule,
            "snapshots_count": if snap.queried_at.is_empty() { None } else { Some(snap.count) },
            "last_snapshot_at": snap.last_time,
            "last_duration_s": snap.last_duration_s,
            "last_data_added": snap.last_data_added,
            "last_total_bytes": snap.last_total_bytes,
            // Con planes: la última copia con error (para avisar) o, si no hay
            // ninguna, la más reciente.
            "last_run": plans_last_run(repo, state).map(|r| {
                let mut v = serde_json::to_value(r).unwrap_or(Value::Null);
                v["message"] = json!(public_message(&r.message));
                v
            }),
            // Cada cuánto se espera una copia: el plan más frecuente, midiendo
            // su mayor hueco (noches y días sin copias no son «retraso»).
            "expected_hours": repo.plans.iter().map(|p| p.schedule.max_gap_hours()).min(),
            "plans": repo.plans.iter().map(|p| json!({
                "id": p.id,
                "name": p.name,
                "tags": p.tags,
                "schedule": p.schedule,
                // «Solo guardar si hay cambios»: una revisión sin cambios no crea versión.
                "skip_unchanged": p.skip_unchanged,
                "last_run": state.runs.get(&crate::plans::plan_key(&repo.id, &p.id)).map(|r| {
                    let mut v = serde_json::to_value(r).unwrap_or(Value::Null);
                    v["message"] = json!(public_message(&r.message));
                    v
                }),
            })).collect::<Vec<_>>(),
            "snapshots": list,
            // Copias automáticas en pausa (la web no las cuenta como retrasadas).
            "paused": paused_json(repo, Local::now()),
            // Copia automática en marcha ahora mismo (si es de este repositorio).
            "running_since": state.running.as_ref().filter(|r| r.repo_id == repo.id).map(|r| r.started.clone()),
            // Mantenimiento: verificación y copia externa (horario, última vez, en curso).
            "maintenance": {
                // Prueba de restauración (restaurar unos archivos y comprobarlos).
                "restore_test": repo.restore_test.as_ref().map(|t| json!({ "schedule": t.schedule, "files": t.files, "max_mb": t.max_mb })),
                "verify": repo.verify.as_ref().map(|v| {
                    let rot = tasks.rotation.get(&repo.id);
                    json!({
                        "schedule": v.schedule,
                        "subset_percent": v.subset_percent,
                        // Rotativa: todo el repositorio cada `rotate_parts` verificaciones.
                        "rotate_parts": v.rotate_parts,
                        "next_part": if v.rotate_parts > 0 { Some(crate::tasks::current_part(rot, v.rotate_parts)) } else { None },
                        "last_full_at": rot.and_then(|r| r.last_full_at.clone()),
                    })
                }),
                "offsite": repo.offsite.as_ref().map(|o| json!({
                    // Verificación de la copia en la nube (desde este destino).
                    "verify": o.verify.as_ref().map(|v| {
                        let rot = tasks.rotation.get(&crate::tasks::rotation_key("verify_offsite", &repo.id));
                        json!({
                            "schedule": v.schedule,
                            "subset_percent": v.subset_percent,
                            "rotate_parts": v.rotate_parts,
                            "next_part": if v.rotate_parts > 0 { Some(crate::tasks::current_part(rot, v.rotate_parts)) } else { None },
                            "last_full_at": rot.and_then(|r| r.last_full_at.clone()),
                        })
                    }),
                    "schedule": o.schedule,
                    "provider": o.provider,
                    "target_name": o.target_name,
                    "retention": o.retention.is_some(),
                })),
            },
            // Subida a la nube frenada por un cambio inusual (la web avisa).
            "offsite_hold": guard.holds.get(&repo.id).map(|h| json!({
                "since": h.since,
                "snapshot_id": h.snapshot_id,
                "plan_name": h.plan_name,
                "data_added": h.data_added,
                "files": h.files,
                "typical_bytes": h.typical_bytes,
                "typical_files": h.typical_files,
            })),
            "verify_run": task_run("verify", &repo.id),
            "offsite_run": task_run("offsite", &repo.id),
            "offsite_verify_run": task_run("verify_offsite", &repo.id),
            "restore_test_run": task_run("restore_test", &repo.id),
            // Salud de la protección: la misma lista de comprobaciones que en la app.
            "protection": protection,
            // Kit de recuperación guardado (null si falta o ya no vale para esta ubicación).
            "kit_saved_at": repo.kit.as_ref().filter(|k| k.matches(&repo.location, None)).map(|k| k.saved_at.clone()),
            "task_running": tasks.live_running().filter(|t| t.repo_id == repo.id).map(|t| json!({
                "kind": t.kind,
                "started": t.started,
                "stage": public_message(&t.stage),
                "done": t.done,
                "total": t.total,
                "percent": t.percent,
                "eta_s": t.eta_s,
                "bytes_done": t.bytes_done,
                "bytes_total": t.bytes_total,
                "current_snapshot_time": t.current_snapshot_time,
            })),
        }));
        sent_lists.push((repo.id.clone(), repos.last().is_some_and(|r| r["snapshots"].is_array())));
    }
    state.web.snapshots.retain(|id, _| config.repos.iter().any(|r| &r.id == id));

    let report = json!({
        "app_version": crate::version_programa(),
        "os": os_label(),
        "repos": repos,
        // Acepta «Copiar ahora» a distancia (lo decide cada equipo).
        "remote_backup": config.remote_backup,
        // Servidor de copias (fase 4): solo nombres de usuarios y repositorios, nunca contenido ni contraseñas.
        "server": server_report(),
        // Equipo gestionado por una consola (fase 5): de quién y si el servicio sigue en marcha.
        "managed": crate::endpoint::load().map(|s| json!({
            "console_device": s.console_device,
            "seq": s.last_seq,
            "service": if crate::agente::service_running() == Some(false) { "stopped_by_admin" } else { "running" },
        })),
    });
    let body = json!({ "p_device": link.device_id, "p_secret": device_secret.password, "p_report": report });
    let mut result = rpc(&link.url, &link.key, "device_report", &body);
    // La web limita la frecuencia (un informe cada pocos segundos): tras una
    // copia muy corta se reintenta una vez para que el resultado llegue ya.
    if result.as_ref().is_err_and(|e| e.contains("Demasiados informes")) {
        std::thread::sleep(std::time::Duration::from_secs(6));
        result = rpc(&link.url, &link.key, "device_report", &body);
    }
    match result {
        Ok(answer) => {
            // Las listas enviadas en este informe ya están en la web.
            for (id, _) in sent_lists.iter().filter(|(_, sent)| *sent) {
                if let Some(s) = state.web.snapshots.get_mut(id) {
                    s.list_sent = true;
                }
            }
            if answer["revoked"].as_bool() == Some(true) {
                link.revoked = true;
                let _ = agent::write_json("web.json", &Some(link));
                agent::log("La web desvinculó este equipo: se dejan de enviar informes.");
            }
            state.web.last_report = Some(Local::now().to_rfc3339());
            state.web.last_error = None;
            Ok(())
        }
        Err(e) => {
            agent::log(&format!("Informe a la web: {e}"));
            state.web.last_error = Some(local_message(&e));
            Err(e)
        }
    }
}

/// El Servidor de copias en el informe (solo si está activo). Ver docs/compartir.md.
fn server_report() -> Value {
    let c = crate::server::load();
    if !c.enabled {
        return Value::Null;
    }
    json!({
        "port": c.port,
        "lan_addresses": crate::server::lan_addresses(),
        "tls_sha256": c.tls_sha256,
        "local_subnet_only": c.local_subnet_only,
        "users": crate::server::repos_by_user(&c).into_iter().map(|(user, repos)| json!({ "user": user, "repos": repos })).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {

    #[test]
    fn mensaje_publico_sin_rutas() {
        let m = public_message("open C:\\Users\\ana\\secreto.xlsx: acceso denegado\nmás detalle");
        assert_eq!(m, "open [ruta]: acceso denegado");
        assert_eq!(public_message(r#"lstat "\\srv\datos\x": fallo"#), "lstat [ruta]: fallo");
        assert_eq!(public_message("Contraseña incorrecta."), "Contraseña incorrecta.");
        // Rutas entre comillas con espacios: ni un trozo del nombre.
        let m = public_message("«/C/Users/ana/Nóminas/secreto marzo 2026.xlsx» no se restauró");
        assert!(!m.contains("marzo") && !m.contains("2026") && m.contains("no se restauró"), "{m}");
        let m = public_message(r#"open "C:\Users\ana\mis cosas\x.txt": acceso denegado"#);
        assert!(!m.contains("cosas") && m.contains("acceso denegado"), "{m}");
        assert!(!public_message("Get https://ana:clave@h/r: 401").contains("clave"));
        // Rutas sin comillas con espacios (como las escribe restic).
        let m = public_message("error: open /C/Users/ana/mis fotos/playa 2026.jpg: Access is denied.");
        assert!(!m.contains("fotos") && !m.contains("playa") && m.contains("Access is denied"), "{m}");
        let m = public_message(r"error: open C:\Users\Ana López\Nóminas\marzo.xlsx: acceso denegado");
        assert!(!m.contains("López") && !m.contains("marzo") && m.contains("acceso denegado"), "{m}");
        // Para los archivos del agente: todas las líneas, sin rutas; los nombres se quedan.
        let m = local_message("Copia «Laboral» con avisos\nopen C:\\Users\\ana\\x.pst: en uso");
        assert_eq!(m, "Copia «Laboral» con avisos\nopen [ruta]: en uso");
        assert!(public_message(&"x".repeat(500)).chars().count() <= 241);
    }

    use super::*;

    #[test]
    fn servidor_sin_credenciales() {
        assert_eq!(kind_and_host(r"D:\Copias\restic"), ("local", None));
        assert_eq!(kind_and_host("rest:http://siigo:clave@192.168.1.30:8001/siigo/"), ("rest", Some("192.168.1.30:8001".into())));
        assert_eq!(kind_and_host("rest:https://backups.ejemplo.com/equipo/"), ("rest", Some("backups.ejemplo.com".into())));
        assert_eq!(kind_and_host("sftp:ana@nas.local:/volume1/restic"), ("sftp", Some("nas.local".into())));
        assert_eq!(kind_and_host("s3:s3.amazonaws.com/mi-bucket"), ("s3", Some("s3.amazonaws.com".into())));
        assert_eq!(kind_and_host("b2:bucket:ruta"), ("b2", None));
    }

    /// Contra la web real (red): `cargo test -- --ignored`. Un código falso debe rechazarse.
    #[test]
    #[ignore]
    fn web_real_rechaza_codigo_falso() {
        let (url, key) = defaults();
        let err = rpc(&url, &key, "device_pair", &json!({ "p_code": "AAAAAAAA", "p_name": "prueba" })).unwrap_err();
        assert!(err.contains("Código no válido"), "{err}");
    }

    #[test]
    fn pausa_para_la_web() {
        let now = Local::now();
        let mut repo = crate::agent::tests::repo_cada_hora();
        assert_eq!(paused_json(&repo, now), Value::Null);
        repo.pause = Some(agent::Pause { since: "2026-09-30T09:00:00-05:00".into(), until: None });
        assert_eq!(paused_json(&repo, now), json!({ "since": "2026-09-30T09:00:00-05:00", "until": null }));
        let until = (now + Duration::hours(4)).to_rfc3339();
        repo.pause.as_mut().unwrap().until = Some(until.clone());
        assert_eq!(paused_json(&repo, now), json!({ "since": "2026-09-30T09:00:00-05:00", "until": until }));
        // Una pausa que ya terminó (aún sin quitar) no se envía.
        assert_eq!(paused_json(&repo, now + Duration::hours(5)), Value::Null);
    }

    #[test]
    fn valida_direccion_y_clave() {
        assert!(validate("https://x.supabase.co", "sb_publishable_x").is_ok());
        assert!(validate("http://x.supabase.co", "k").is_err());
        assert!(validate("https://x.supabase.co", " ").is_err());
    }
}
