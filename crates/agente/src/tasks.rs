//! Tareas de mantenimiento del agente:
//!
//! - **Verificación** (`restic check`): comprueba que el repositorio está sano
//!   y, si se pide, lee una parte de los datos para detectar daños.
//! - **Copia externa** (`restic copy`): sube las copias a otro repositorio
//!   (S3, B2, otro servidor…) y aplica allí la política de retención.
//!
//! Corren en un proceso aparte (`resguardo.exe --agent-tasks`) con su propio
//! bloqueo y su propio estado (`tasks.json`): una subida de horas no frena las
//! copias, que el agente sigue haciendo cada 5 minutos. Mientras se verifica
//! un repositorio, sus copias esperan (la verificación lo bloquea).

use crate::agent::{self, AgentConfig, AgentRepo, RunRecord, Schedule, Secret};
use crate::restic::{self, Access};
use crate::retention::Policy;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::time::Duration;

/// Una verificación o una subida pueden tardar horas (la primera subida de un
/// repositorio grande, sobre todo).
const TASK_TIMEOUT: Duration = Duration::from_secs(20 * 3600);
/// Cada cuánto se guarda el progreso en `tasks.json`.
const PROGRESS_EVERY: std::time::Duration = std::time::Duration::from_secs(5);
/// Cada cuánto se avisa a la web durante una tarea larga (el servidor limita
/// a un informe cada pocos segundos: un minuto deja margen de sobra).
const WEB_EVERY: std::time::Duration = std::time::Duration::from_secs(60);
/// restic solo escribe su barra de progreso fuera de una terminal si se le pide
/// una frecuencia: una línea cada 5 s.
const RESTIC_FPS: (&str, &str) = ("RESTIC_PROGRESS_FPS", "0.2");

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verify {
    pub schedule: Schedule,
    /// Porcentaje de los datos que se leen y comprueban cada vez (0: solo la
    /// estructura). Con un porcentaje pequeño y frecuente, con el tiempo se
    /// revisa todo sin cargar el servidor.
    #[serde(default)]
    pub subset_percent: u8,
    /// RFC 3339: cuándo se activó (referencia para la primera vez).
    pub enabled_at: String,
    /// Verificación rotativa: cada vez se lee una parte fija de las `t`
    /// (`--read-data-subset n/t`), así en `t` verificaciones se lee todo el
    /// repositorio. 0: no (se usa `subset_percent`, una parte al azar).
    #[serde(default)]
    pub rotate_parts: u32,
}

impl Verify {
    pub fn validate(&self) -> Result<(), String> {
        self.schedule.validate_task()?;
        if self.subset_percent > 100 {
            return Err("El porcentaje debe estar entre 0 y 100.".into());
        }
        if self.rotate_parts == 1 || self.rotate_parts > 52 {
            return Err("La verificación rotativa va de 2 a 52 partes.".into());
        }
        Ok(())
    }

    /// Qué datos leer en esta verificación: la parte `part` de la rotativa
    /// (`n/t`), un porcentaje al azar (`P%`) o nada (solo la estructura).
    pub fn subset_arg(&self, part: u32) -> Option<String> {
        if self.rotate_parts > 0 {
            Some(format!("{}/{}", part.clamp(1, self.rotate_parts), self.rotate_parts))
        } else if self.subset_percent > 0 {
            Some(format!("{}%", self.subset_percent.min(100)))
        } else {
            None
        }
    }
}

/// Avance de la verificación rotativa de un destino (en `tasks.json`: solo lo
/// escribe el proceso de tareas; si la app cambia el número de partes, la
/// rotación vuelve a empezar al leerla, sin tener que escribir aquí).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Rotation {
    /// Número de partes con el que se contó (si la configuración cambia, se empieza de 1).
    pub parts: u32,
    /// Próxima parte (1 a `parts`).
    pub next_part: u32,
    /// RFC 3339: cuándo se terminó de leer la última parte de un ciclo (todo el repositorio).
    #[serde(default)]
    pub last_full_at: Option<String>,
}

/// Parte que toca leer con `parts` partes según el avance guardado.
pub fn current_part(rot: Option<&Rotation>, parts: u32) -> u32 {
    match rot {
        Some(r) if r.parts == parts && (1..=parts).contains(&r.next_part) => r.next_part,
        _ => 1,
    }
}

/// Nuevo avance de la rotativa tras una verificación: solo si salió bien (una
/// fallida o cortada repite la misma parte) y si la rotativa está activa.
pub fn rotation_after(rot: Option<&Rotation>, parts: u32, part: u32, record: &RunRecord) -> Option<Rotation> {
    (parts > 0 && record.result == "ok").then(|| advance(rot, parts, part, &record.finished))
}

/// Avance tras una verificación correcta de la parte `part`: la siguiente (o
/// vuelta a la 1 al completar el ciclo, anotando la lectura completa).
pub fn advance(rot: Option<&Rotation>, parts: u32, part: u32, now: &str) -> Rotation {
    let full = part >= parts;
    Rotation {
        parts,
        next_part: if full { 1 } else { part + 1 },
        last_full_at: if full { Some(now.to_string()) } else { rot.and_then(|r| r.last_full_at.clone()) },
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Offsite {
    /// Repositorio de destino: `s3:https://…/bucket/carpeta`, `b2:bucket:carpeta`,
    /// `rest:https://…`, una carpeta, `rclone:…`.
    pub location: String,
    /// Proveedor elegido en la app (informativo): b2, wasabi, r2, aws, s3, otro.
    #[serde(default)]
    pub provider: String,
    /// Región de S3 (algunos proveedores la exigen; R2 usa "auto").
    #[serde(default)]
    pub region: Option<String>,
    pub schedule: Schedule,
    /// Retención que se aplica en el destino después de cada subida.
    #[serde(default)]
    pub retention: Option<Policy>,
    /// Velocidad máxima de subida en KiB/s (para no saturar la conexión).
    #[serde(default)]
    pub limit_upload_kib: Option<u32>,
    /// Nombre del destino de la app al que se sube (si es uno de ellos).
    #[serde(default)]
    pub target_name: Option<String>,
    pub enabled_at: String,
    /// Freno ante cambios inusuales (p. ej. un ransomware que cifra todo):
    /// si una copia cambia mucho más de lo normal, no se sube hasta revisarlo.
    #[serde(default)]
    pub guard: Option<Guard>,
    /// Verificar también la copia externa (`restic check` en el destino, con
    /// las mismas credenciales de la subida). `enabled_at` es el de esta verificación.
    #[serde(default)]
    pub verify: Option<Verify>,
    /// Cómo es el destino: un repositorio que ya existía, con bloqueo de
    /// objetos o de solo añadir (v1.46; sin nada, como siempre).
    #[serde(default, skip_serializing_if = "DestinoExterno::normal")]
    pub dest: DestinoExterno,
    /// Tarea 4c: solo las versiones que pasan este filtro (equipos, etiquetas,
    /// carpetas, fechas). Sin él, todas (como siempre).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filtro: Option<crate::adoptar_v2::Filtro>,
}

/// Tarea 4b (docs/copias-en-cadena.md): otra copia derivada de un repositorio,
/// además de la copia externa de siempre (`AgentRepo::offsite`, que es la
/// primera). Cada una con su destino, su contraseña (en `Secret::derived`), su
/// retención, su horario (también «después de cada copia») y su verificación.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Derived {
    /// `[a-z0-9_-]`, único en el repositorio (no «externa», que es la de siempre).
    pub id: String,
    pub offsite: Offsite,
}

/// Las tareas de una copia derivada (las de la externa de siempre son `offsite` y `verify_offsite`).
pub const DERIVADA: &str = "derivada";
pub const VERIFY_DERIVADA: &str = "verify_derivada";

/// La clave de una tarea en `tasks.json` (`runs`, solicitudes y rotación): la de
/// siempre (`<tipo>:<repo>`) o, en una derivada, `<tipo>:<repo>:<derivada>`.
/// `repo` es el que da [`due`]: en las de una derivada, con ella sola en `derived`.
pub fn task_key(kind: &str, repo: &AgentRepo) -> String {
    match repo.derived.first().filter(|_| kind == DERIVADA || kind == VERIFY_DERIVADA) {
        Some(d) => format!("{kind}:{}:{}", repo.id, d.id),
        None => key(kind, &repo.id),
    }
}

/// La clave de la última vuelta de una derivada, sin el repositorio virtual.
pub fn derived_key(kind: &str, repo_id: &str, derived_id: &str) -> String {
    format!("{kind}:{repo_id}:{derived_id}")
}

/// El nombre del archivo de la solicitud «ahora» de una derivada (`<repo>.derivada-<id>`).
pub fn derived_request(kind: &str, derived_id: &str) -> String {
    format!("{kind}-{derived_id}")
}

/// Las credenciales de una derivada en lugar de las de la copia externa de
/// siempre (así `dest_access` y `offsite_repo` sirven igual para las dos).
pub fn derived_secret(secret: &Secret, derived_id: &str) -> Secret {
    let d = secret.derived.get(derived_id).cloned().unwrap_or_default();
    Secret {
        password: secret.password.clone(),
        rest_password: secret.rest_password.clone(),
        env: secret.env.clone(),
        offsite_password: d.password,
        offsite_key_id: None,
        offsite_secret: None,
        offsite_location: d.location,
        offsite_env: d.env,
        derived: std::collections::HashMap::new(),
    }
}

/// El repositorio tal como lo ve una tarea de una derivada: con ella como su
/// copia externa (y sola en `derived`, para [`task_key`]).
pub fn derived_view(repo: &AgentRepo, d: &Derived) -> AgentRepo {
    AgentRepo { offsite: Some(d.offsite.clone()), derived: vec![d.clone()], ..repo.clone() }
}

/// Lo que se sabe del destino de una copia externa (`cambiar_copia_externa`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DestinoExterno {
    /// Era un repositorio que ya existía («Usar uno que ya existe»): si un día
    /// no aparece, la subida falla en vez de crear otro vacío allí (que lo
    /// subiría todo de nuevo).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub existing: bool,
    /// Días de bloqueo de objetos (Object Lock de B2/S3) del destino: nada de
    /// lo subido se puede borrar antes. La retención solo quita versiones
    /// (`forget`, sin `prune`) y nunca las de esos últimos días.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_lock_days: Option<u32>,
    /// Un rest-server de solo añadir: desde aquí no se puede borrar nada (la
    /// retención la aplica el propio servidor).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub append_only: bool,
    /// Tarea 4a: una nube conectada en este equipo (`nube.rs`), por su nombre:
    /// la ubicación es `rclone:rnube:…` y sus credenciales se ponen al usarla.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nube: Option<String>,
}

impl DestinoExterno {
    pub fn normal(&self) -> bool {
        *self == DestinoExterno::default()
    }

    /// Días de bloqueo que cuentan (0 o nada: sin bloqueo).
    pub fn lock_days(&self) -> Option<u32> {
        self.object_lock_days.filter(|d| *d > 0)
    }
}

/// Días máximos de bloqueo que se aceptan (10 años).
pub const MAX_LOCK_DAYS: u32 = 3650;

/// La retención que se aplica en un destino con bloqueo de `days` días: la
/// misma, pero guardando siempre todo lo de esos días (más uno de margen) para
/// no intentar borrar archivos que aún están bloqueados. Si ya guardaba más
/// (`keep_within`), se queda la suya.
pub fn locked_policy(policy: &Policy, days: u32) -> Policy {
    let mut p = policy.clone();
    let minimo = days.saturating_add(1);
    let suyos = p.keep_within.as_deref().and_then(approx_days);
    if suyos.is_none_or(|d| d < u64::from(minimo)) {
        p.keep_within = Some(format!("{minimo}d"));
    }
    p
}

/// Días aproximados de un plazo de restic («30d», «1y6m», «48h»), o `None` si no se entiende.
fn approx_days(s: &str) -> Option<u64> {
    let (mut horas, mut n) = (0u64, None::<u64>);
    for c in s.trim().chars() {
        match c {
            '0'..='9' => n = Some(n.unwrap_or(0).checked_mul(10)?.checked_add(u64::from(c.to_digit(10)?))?),
            'y' | 'm' | 'd' | 'h' => {
                let v = n.take()?;
                horas = horas.checked_add(v.checked_mul(match c {
                    'y' => 8766,
                    'm' => 730,
                    'd' => 24,
                    _ => 1,
                })?)?;
            }
            _ => return None,
        }
    }
    (n.is_none() && horas > 0).then_some(horas / 24)
}

/// La verificación de cada tipo de tarea: la del destino o la de su copia externa.
pub fn task_verify<'a>(repo: &'a AgentRepo, kind: &str) -> Option<&'a Verify> {
    match kind {
        "verify" => repo.verify.as_ref(),
        "verify_offsite" | VERIFY_DERIVADA => repo.offsite.as_ref().and_then(|o| o.verify.as_ref()),
        _ => None,
    }
}

/// Clave del avance de la rotativa: el id del destino (como siempre) o
/// "offsite:<id>" para la verificación de su copia externa.
pub fn rotation_key(kind: &str, repo_id: &str) -> String {
    if kind == "verify_offsite" {
        format!("offsite:{repo_id}")
    } else {
        repo_id.to_string()
    }
}

/// La de la verificación de una tarea (con las derivadas: `derivada:<repo>:<id>`).
pub fn task_rotation_key(kind: &str, repo: &AgentRepo) -> String {
    match repo.derived.first().filter(|_| kind == VERIFY_DERIVADA) {
        Some(d) => format!("{DERIVADA}:{}:{}", repo.id, d.id),
        None => rotation_key(kind, &repo.id),
    }
}

/// Umbrales del freno: una copia es inusual si añade más de `factor` veces lo
/// normal (la mediana de las últimas copias) y, como mínimo, `min_bytes` o
/// `min_files` archivos nuevos o cambiados.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Guard {
    #[serde(default = "default_factor")]
    pub factor: u32,
    #[serde(default = "default_min_bytes")]
    pub min_bytes: u64,
    #[serde(default = "default_min_files")]
    pub min_files: u64,
}

fn default_factor() -> u32 {
    20
}
fn default_min_bytes() -> u64 {
    2 * 1024 * 1024 * 1024
}
fn default_min_files() -> u64 {
    5000
}

impl Default for Guard {
    fn default() -> Self {
        Self { factor: default_factor(), min_bytes: default_min_bytes(), min_files: default_min_files() }
    }
}

impl Guard {
    pub fn validate(&self) -> Result<(), String> {
        if !(2..=1000).contains(&self.factor) {
            return Err("El factor del freno debe estar entre 2 y 1000.".into());
        }
        if !(1024 * 1024..=1u64 << 50).contains(&self.min_bytes) {
            return Err("El mínimo de datos del freno no es válido.".into());
        }
        if !(1..=1_000_000_000).contains(&self.min_files) {
            return Err("El mínimo de archivos del freno no es válido.".into());
        }
        Ok(())
    }
}

/// Subida frenada por un cambio inusual (hasta que alguien la reanude).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hold {
    /// RFC 3339: cuándo se frenó.
    pub since: String,
    /// Versión (snapshot) con el cambio inusual.
    pub snapshot_id: String,
    pub plan_name: String,
    /// Lo que añadió esa copia.
    pub data_added: u64,
    /// Archivos nuevos + cambiados.
    pub files: u64,
    /// Lo normal (mediana de las últimas copias); `None` si aún hay pocas.
    #[serde(default)]
    pub typical_bytes: Option<u64>,
    #[serde(default)]
    pub typical_files: Option<u64>,
    /// RFC 3339: cuándo terminó esa copia.
    #[serde(default)]
    pub backup_finished: String,
}

/// Estado del freno (`guard.json`, junto a la configuración del agente: lo
/// escriben el proceso de tareas y la app como administrador; los usuarios
/// solo pueden leerlo). Va aparte de `tasks.json` para que reanudar desde la
/// app no se pierda si en ese momento hay una subida en marcha.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct GuardState {
    /// Subidas frenadas, por destino.
    #[serde(default)]
    pub holds: HashMap<String, Hold>,
    /// Hasta qué copia (RFC 3339 de su final) ya se revisó cada destino.
    #[serde(default)]
    pub checked: HashMap<String, String>,
}

pub fn load_guard() -> GuardState {
    agent::read_json("guard.json")
}

// ---------- Copias con versión nueva (para «después de cada copia» y el freno) ----------

/// Una copia de un destino que guardó una versión nueva.
#[derive(Debug, Clone, PartialEq)]
pub struct BackupEvent {
    pub finished: DateTime<Local>,
    pub snapshot_id: String,
    pub plan_name: String,
    pub data_added: u64,
    /// Archivos nuevos + cambiados.
    pub files: u64,
}

/// Copia a mano que la app avisa al agente (`solicitudes\<id>.copia`): la
/// escriben los usuarios, así que se valida todo y la fecha nunca es futura.
#[derive(Serialize, Deserialize)]
struct ManualNote {
    finished: String,
    snapshot_id: String,
    #[serde(default)]
    plan_name: String,
    #[serde(default)]
    data_added: u64,
    #[serde(default)]
    files_new: u64,
    #[serde(default)]
    files_changed: u64,
}

fn manual_note_path(repo_id: &str) -> std::path::PathBuf {
    agent::requests_dir().join(format!("{repo_id}.copia"))
}

/// Anota una copia a mano con versión nueva (la app, sin administrador), para
/// que la copia externa «después de cada copia» y el freno la tengan en cuenta.
pub fn note_manual_backup(repo_id: &str, plan_name: &str, snapshot_id: &str, data_added: u64, files: (u64, u64)) {
    let wanted = agent::load_config().repos.iter().find(|r| r.id == repo_id).is_some_and(|r| {
        let after = |o: &Offsite| o.guard.is_some() || matches!(o.schedule, Schedule::AfterBackup { .. });
        r.offsite.as_ref().is_some_and(after) || r.derived.iter().any(|d| after(&d.offsite))
    });
    if !wanted || !agent::requests_dir().is_dir() {
        return;
    }
    let note = ManualNote {
        finished: Local::now().to_rfc3339(),
        snapshot_id: snapshot_id.into(),
        plan_name: plan_name.into(),
        data_added,
        files_new: files.0,
        files_changed: files.1,
    };
    if let Ok(body) = serde_json::to_vec(&note) {
        let _ = fs::write(manual_note_path(repo_id), body);
    }
}

fn read_manual_note(repo_id: &str, now: DateTime<Local>) -> Option<BackupEvent> {
    let path = manual_note_path(repo_id);
    let meta = fs::symlink_metadata(&path).ok().filter(|m| m.is_file() && m.len() < 4096)?;
    let note: ManualNote = serde_json::from_slice(&fs::read(&path).ok()?).ok()?;
    let id_ok = !note.snapshot_id.is_empty() && note.snapshot_id.len() <= 64 && note.snapshot_id.chars().all(|c| c.is_ascii_hexdigit());
    if !id_ok {
        return None;
    }
    // Nunca después de la fecha del archivo ni de ahora.
    let modified = DateTime::<chrono::Utc>::from(meta.modified().ok()?).with_timezone(&Local);
    let finished = DateTime::parse_from_rfc3339(&note.finished).ok()?.with_timezone(&Local).min(modified).min(now);
    Some(BackupEvent {
        finished,
        snapshot_id: note.snapshot_id,
        plan_name: note.plan_name.chars().take(80).collect(),
        data_added: note.data_added,
        files: note.files_new.saturating_add(note.files_changed),
    })
}

/// Copias de este destino que guardaron versión (sin las «sin cambios» ni las
/// fallidas), de la más antigua a la más reciente: las del agente (su estado y
/// su historial) y la última a mano que avisó la app.
pub fn backup_events(repo: &AgentRepo, state: &agent::AgentState, history: &[crate::history::Entry], now: DateTime<Local>) -> Vec<BackupEvent> {
    let parse = |t: &str| DateTime::parse_from_rfc3339(t).ok().map(|d| d.with_timezone(&Local));
    let mut out: Vec<BackupEvent> = Vec::new();
    for e in history {
        if e.repo_id != repo.id || e.kind != "backup" || e.unchanged || e.result == "error" {
            continue;
        }
        let (Some(id), Some(finished)) = (e.snapshot_id.clone(), parse(&e.finished)) else { continue };
        out.push(BackupEvent {
            finished,
            snapshot_id: id,
            plan_name: e.plan_name.clone().unwrap_or_default(),
            data_added: e.data_added.unwrap_or(0),
            files: e.files_new.unwrap_or(0) + e.files_changed.unwrap_or(0),
        });
    }
    for plan in &repo.plans {
        let Some(r) = state.runs.get(&crate::plans::plan_key(&repo.id, &plan.id)) else { continue };
        if r.unchanged || r.result == "error" {
            continue;
        }
        let (Some(id), Some(finished)) = (r.snapshot_id.clone(), parse(&r.finished)) else { continue };
        out.push(BackupEvent {
            finished,
            snapshot_id: id,
            plan_name: plan.name.clone(),
            data_added: r.data_added.unwrap_or(0),
            files: r.files_new.unwrap_or(0) + r.files_changed.unwrap_or(0),
        });
    }
    out.extend(read_manual_note(&repo.id, now));
    let mut seen = std::collections::HashSet::new();
    out.retain(|e| e.finished <= now + chrono::Duration::minutes(1) && seen.insert(e.snapshot_id.clone()));
    out.sort_by_key(|e| e.finished);
    out
}

/// «Después de cada copia con cambios»: hay una versión nueva terminada después
/// del inicio de la última subida y pasaron al menos `min_minutes` desde entonces.
pub fn after_backup_due(events: &[BackupEvent], last_start: DateTime<Local>, min_minutes: u32, now: DateTime<Local>) -> bool {
    events.iter().any(|e| e.finished > last_start) && now.signed_duration_since(last_start) >= chrono::Duration::minutes(min_minutes as i64)
}

fn median(mut v: Vec<u64>) -> u64 {
    v.sort_unstable();
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2
    }
}

/// Muestras mínimas para saber «lo normal» (con menos, solo cuentan los mínimos).
const GUARD_MIN_SAMPLES: usize = 5;

/// ¿Es inusual esta copia? Compara con la mediana de las (hasta 20) copias con
/// versión anteriores. Devuelve lo normal (bytes y archivos, si se sabe) si lo es.
pub fn unusual(prior: &[BackupEvent], ev: &BackupEvent, guard: &Guard) -> Option<(Option<u64>, Option<u64>)> {
    let samples: Vec<&BackupEvent> = prior.iter().filter(|e| e.finished < ev.finished && e.snapshot_id != ev.snapshot_id).collect();
    let samples = &samples[samples.len().saturating_sub(20)..];
    let (typ_b, typ_f) = if samples.len() >= GUARD_MIN_SAMPLES {
        (Some(median(samples.iter().map(|e| e.data_added).collect())), Some(median(samples.iter().map(|e| e.files).collect())))
    } else {
        (None, None)
    };
    let limit_b = typ_b.map_or(0, |m| m.saturating_mul(guard.factor as u64)).max(guard.min_bytes);
    let limit_f = typ_f.map_or(0, |m| m.saturating_mul(guard.factor as u64)).max(guard.min_files);
    (ev.data_added > limit_b || ev.files > limit_f).then_some((typ_b, typ_f))
}

/// «38,2 GB», «20 MB».
pub fn human_bytes(n: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < units.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    let s = if i == 0 || v >= 100.0 { format!("{v:.0}") } else { format!("{v:.1}").replace('.', ",") };
    format!("{} {}", s.trim_end_matches(",0"), units[i])
}

/// «12.400».
pub fn human_count(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(c);
    }
    out
}

/// Frase del freno (historial y registro).
pub fn hold_message(h: &Hold) -> String {
    let at = DateTime::parse_from_rfc3339(&h.backup_finished).map(|d| d.with_timezone(&Local).format("%H:%M").to_string()).unwrap_or_default();
    let what = if h.plan_name.is_empty() { "La copia".to_string() } else { format!("La copia «{}»", h.plan_name) };
    let when = if at.is_empty() { String::new() } else { format!(" de las {at}") };
    let normal = match (h.typical_bytes, h.typical_files) {
        (Some(b), Some(f)) => format!("lo normal: ~{} y ~{}", human_bytes(b), human_count(f)),
        _ => "aún no hay copias suficientes para saber lo normal".into(),
    };
    format!(
        "{what}{when} añadió {} y {} archivos ({normal}). La subida a la nube está frenada por precaución.",
        human_bytes(h.data_added),
        human_count(h.files)
    )
}

/// Datos que necesita la decisión de qué toca: copias con versión de cada
/// destino y subidas frenadas.
#[derive(Default)]
pub struct DueContext {
    pub events: HashMap<String, Vec<BackupEvent>>,
    pub holds: std::collections::HashSet<String>,
}

impl DueContext {
    fn load(config: &AgentConfig, now: DateTime<Local>) -> Self {
        let after = |o: &Offsite| o.guard.is_some() || matches!(o.schedule, Schedule::AfterBackup { .. });
        let needs = config.repos.iter().any(|r| r.offsite.as_ref().is_some_and(after) || r.derived.iter().any(|d| after(&d.offsite)));
        let mut ctx = DueContext { holds: load_guard().holds.into_keys().collect(), ..Default::default() };
        if needs {
            let state = agent::load_state();
            let history = crate::history::read(&crate::history::agent_file());
            for r in &config.repos {
                if r.offsite.is_some() || !r.derived.is_empty() {
                    ctx.events.insert(r.id.clone(), backup_events(r, &state, &history, now));
                }
            }
        }
        ctx
    }
}

/// Revisa las copias nuevas de los destinos con freno. Si alguna es inusual,
/// frena su subida (y lo anota). Devuelve si queda alguna por revisar (para
/// lanzar el proceso de tareas) sin cambiar nada cuando `apply` es false.
fn check_guards(config: &AgentConfig, ctx: &DueContext, apply: bool) -> bool {
    let mut guard = load_guard();
    let mut changed = false;
    for repo in &config.repos {
        let Some(g) = repo.offsite.as_ref().and_then(|o| o.guard.as_ref()) else { continue };
        let events = ctx.events.get(&repo.id).map(Vec::as_slice).unwrap_or(&[]);
        let parse = |t: &str| DateTime::parse_from_rfc3339(t).ok().map(|d| d.with_timezone(&Local));
        // Sin revisar aún: desde que se activó la copia externa (no el historial antiguo).
        let from = guard.checked.get(&repo.id).and_then(|t| parse(t)).or_else(|| repo.offsite.as_ref().and_then(|o| parse(&o.enabled_at)));
        let fresh: Vec<&BackupEvent> = events.iter().filter(|e| from.is_none_or(|f| e.finished > f)).collect();
        if fresh.is_empty() {
            continue;
        }
        if !apply {
            return true;
        }
        for ev in &fresh {
            if guard.holds.contains_key(&repo.id) {
                break;
            }
            if let Some((typical_bytes, typical_files)) = unusual(events, ev, g) {
                let hold = Hold {
                    since: Local::now().to_rfc3339(),
                    snapshot_id: ev.snapshot_id.clone(),
                    plan_name: ev.plan_name.clone(),
                    data_added: ev.data_added,
                    files: ev.files,
                    typical_bytes,
                    typical_files,
                    backup_finished: ev.finished.to_rfc3339(),
                };
                let message = hold_message(&hold);
                agent::log(&format!("AVISO: cambio inusual en «{}»: {message}", repo.name));
                crate::history::append(
                    &crate::history::agent_file(),
                    &crate::history::Entry {
                        kind: "guard".into(),
                        origin: "agent".into(),
                        repo_id: repo.id.clone(),
                        repo_name: repo.name.clone(),
                        plan_name: Some(ev.plan_name.clone()).filter(|p| !p.is_empty()),
                        started: hold.since.clone(),
                        finished: hold.since.clone(),
                        result: "warning".into(),
                        message,
                        snapshot_id: Some(ev.snapshot_id.clone()),
                        data_added: Some(ev.data_added),
                        files_new: Some(ev.files),
                        ..Default::default()
                    },
                );
                guard.holds.insert(repo.id.clone(), hold);
            }
        }
        if let Some(last) = fresh.last() {
            guard.checked.insert(repo.id.clone(), last.finished.to_rfc3339());
        }
        changed = true;
    }
    if changed {
        if let Err(e) = agent::write_json("guard.json", &guard) {
            agent::log(&format!("ERROR: no se pudo guardar el estado del freno: {e}"));
        }
    }
    false
}

/// «Es normal, reanudar la subida» (la app como administrador).
pub fn clear_hold(repo_id: &str) -> Result<(), String> {
    agent::require_admin()?;
    let mut guard = load_guard();
    let Some(hold) = guard.holds.remove(repo_id) else { return Ok(()) };
    agent::write_json("guard.json", &guard)?;
    let name = agent::load_config().repos.into_iter().find(|r| r.id == repo_id).map(|r| r.name).unwrap_or_default();
    agent::log(&format!("Subida a la nube de «{name}» reanudada: el cambio de la versión {} era normal.", &hold.snapshot_id[..hold.snapshot_id.len().min(8)]));
    let now = Local::now().to_rfc3339();
    crate::history::append(
        &crate::history::agent_file(),
        &crate::history::Entry {
            kind: "guard".into(),
            origin: "manual".into(),
            repo_id: repo_id.into(),
            repo_name: name,
            started: now.clone(),
            finished: now,
            result: "info".into(),
            message: "Subida reanudada: el cambio era normal.".into(),
            snapshot_id: Some(hold.snapshot_id),
            ..Default::default()
        },
    );
    Ok(())
}

/// Tarea en curso (una a la vez).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunningTask {
    pub repo_id: String,
    /// "verify" u "offsite".
    pub kind: String,
    pub started: String,
    /// Qué está haciendo, en palabras.
    #[serde(default)]
    pub stage: String,
    /// Copias subidas hasta ahora (copia externa).
    #[serde(default)]
    pub done: u64,
    #[serde(default)]
    pub updated: Option<String>,
    /// Copias (versiones) que se suben en esta tanda.
    #[serde(default)]
    pub total: Option<u64>,
    /// Progreso de 0 a 1 (en la copia externa, ponderado por lo que ocupa cada versión).
    #[serde(default)]
    pub percent: Option<f64>,
    /// Segundos que faltan, estimados con el ritmo hasta ahora.
    #[serde(default)]
    pub eta_s: Option<u64>,
    /// Datos subidos y por subir (estimados con el resumen de cada versión).
    #[serde(default)]
    pub bytes_done: Option<u64>,
    #[serde(default)]
    pub bytes_total: Option<u64>,
    /// Fecha de la versión que se está subiendo.
    #[serde(default)]
    pub current_snapshot_time: Option<String>,
    /// Lo que restic lee y escribe o sube (bytes/s), de sus contadores de E/S.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_bps: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upload_bps: Option<u64>,
}

/// Progreso de una tarea, tal como lo informa quien la ejecuta.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TaskProgress {
    pub stage: String,
    pub done: u64,
    pub total: Option<u64>,
    pub percent: Option<f64>,
    pub eta_s: Option<u64>,
    pub bytes_done: Option<u64>,
    pub bytes_total: Option<u64>,
    pub current_snapshot_time: Option<String>,
}

impl TaskProgress {
    fn stage(stage: &str) -> Self {
        Self { stage: stage.into(), ..Default::default() }
    }
}

/// Línea de la barra de progreso de restic sin terminal:
/// «[0:05] 40.00%  2 / 5 packs copied» → (0.40, 2, 5, "packs").
pub fn parse_progress(line: &str) -> Option<(f64, u64, u64, String)> {
    let l = line.trim();
    let rest = l.strip_prefix('[')?;
    let (_, rest) = rest.split_once(']')?;
    let mut parts = rest.split_whitespace();
    let pct: f64 = parts.next()?.strip_suffix('%')?.replace(',', ".").parse().ok()?;
    let a: u64 = parts.next()?.parse().ok()?;
    if parts.next()? != "/" {
        return None;
    }
    let b: u64 = parts.next()?.parse().ok()?;
    let unit = parts.next().unwrap_or("").to_string();
    Some(((pct / 100.0).clamp(0.0, 1.0), a, b, unit))
}

/// Segundos que faltan con el ritmo hasta ahora (solo con algo de avance, para no dar cifras absurdas).
pub fn eta(elapsed: std::time::Duration, percent: f64) -> Option<u64> {
    ((0.01..1.0).contains(&percent) && elapsed.as_secs() >= 20).then(|| (elapsed.as_secs_f64() * (1.0 - percent) / percent).round() as u64)
}

/// Progreso de una subida por versiones: las terminadas y la fracción de la
/// actual, cada una con su peso (lo que ocupa: la primera suele subir casi todo).
pub fn weighted_progress(weights: &[u64], done: usize, current_fraction: f64) -> f64 {
    let total: u64 = weights.iter().sum();
    if total == 0 {
        return 0.0;
    }
    let finished: u64 = weights.iter().take(done).sum();
    let current = weights.get(done).copied().unwrap_or(0) as f64 * current_fraction.clamp(0.0, 1.0);
    ((finished as f64 + current) / total as f64).clamp(0.0, 1.0)
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TasksState {
    /// Última ejecución de cada tarea ("verify:<id>", "offsite:<id>").
    #[serde(default)]
    pub runs: HashMap<String, RunRecord>,
    #[serde(default)]
    pub running: Option<RunningTask>,
    /// Última vez que cambió algo (para que el agente avise a la web).
    #[serde(default)]
    pub changed_at: Option<String>,
    /// Última solicitud «ejecutar ahora» atendida de cada tarea.
    #[serde(default)]
    pub requests_seen: HashMap<String, String>,
    /// Verificación rotativa de cada destino.
    #[serde(default)]
    pub rotation: HashMap<String, Rotation>,
}

pub fn key(kind: &str, repo_id: &str) -> String {
    format!("{kind}:{repo_id}")
}

pub fn load_state() -> TasksState {
    agent::read_json("tasks.json")
}

fn save_state(state: &mut TasksState) {
    state.changed_at = Some(Local::now().to_rfc3339());
    let _ = agent::write_json("tasks.json", state);
}

/// ¿Hay una tarea en curso de este tipo en este repositorio? (Si el proceso
/// de tareas se cortó, su estado caduca a los 15 minutos sin noticias.)
pub fn running_on(repo_id: &str, kind: &str) -> bool {
    load_state().live_running().is_some_and(|r| r.repo_id == repo_id && r.kind == kind)
}

impl TasksState {
    /// La tarea en curso, si da señales de vida (un proceso cortado deja de
    /// contar a los 15 minutos).
    pub fn live_running(&self) -> Option<&RunningTask> {
        self.running.as_ref().filter(|r| {
            DateTime::parse_from_rfc3339(r.updated.as_deref().unwrap_or(&r.started))
                .is_ok_and(|t| Local::now().signed_duration_since(t) < chrono::Duration::minutes(15))
        })
    }
}

// ---------- Qué toca ----------

/// Cuándo toca la próxima verificación automática de la consola («cada N
/// horas» o, v1.40, con reglas); `None` con otros horarios.
pub fn next_verify(repo: &AgentRepo, state: &TasksState) -> Option<DateTime<Local>> {
    let v = repo.verify.as_ref()?;
    let desde = since(state, &key("verify", &repo.id), &v.enabled_at);
    match &v.schedule {
        Schedule::Hours { every } => Some(desde + chrono::Duration::hours(i64::from(*every))),
        // v1.40: con reglas, la siguiente después de la última (si ya pasó, toca en cuanto pueda).
        Schedule::Rules { rules } => crate::plans::PlanSchedule::from_rules(rules.clone()).next_slot(desde),
        _ => None,
    }
}

/// Tarea 8: cuándo toca la próxima prueba de restauración automática (la de la consola, cada N días).
pub fn next_restore_test(repo: &AgentRepo, state: &TasksState) -> Option<DateTime<Local>> {
    let t = repo.restore_test.as_ref()?;
    let desde = since(state, &key("restore_test", &repo.id), &t.enabled_at);
    match &t.schedule {
        Schedule::Hours { every } => Some(desde + chrono::Duration::hours(i64::from(*every))),
        _ => None,
    }
}

fn since(state: &TasksState, k: &str, enabled_at: &str) -> DateTime<Local> {
    let s = state.runs.get(k).map(|r| r.started.as_str()).unwrap_or(enabled_at);
    DateTime::parse_from_rfc3339(s).map(|d| d.with_timezone(&Local)).unwrap_or_else(|_| Local::now())
}

/// Solicitudes «ejecutar ahora»: `solicitudes\<id>.<tipo>` escrito después de
/// la última atendida (como las solicitudes de copia: solo se miran fechas).
fn requested(state: &mut TasksState, repo_id: &str, kind: &str, consume: bool) -> bool {
    let path = agent::requests_dir().join(format!("{repo_id}.{kind}"));
    let Ok(meta) = fs::symlink_metadata(&path) else { return false };
    if !meta.is_file() {
        return false;
    }
    let Ok(modified) = meta.modified() else { return false };
    let now = chrono::Utc::now();
    let stamp = DateTime::<chrono::Utc>::from(modified).min(now);
    let k = key(kind, repo_id);
    let seen = state.requests_seen.get(&k).and_then(|s| DateTime::parse_from_rfc3339(s).ok());
    if seen.is_some_and(|seen| seen >= stamp) {
        return false;
    }
    if consume {
        state.requests_seen.insert(k, stamp.to_rfc3339());
    }
    true
}

/// Tareas que tocan ahora, en orden (primero subir a la nube, después verificar).
/// Con las copias automáticas en pausa, solo las pedidas con «Ahora».
/// Con la subida frenada por un cambio inusual, tampoco la copia externa (salvo con «Ahora»).
fn due(config: &AgentConfig, state: &mut TasksState, ctx: &DueContext, consume: bool) -> Vec<(&'static str, AgentRepo)> {
    due_at(config, state, ctx, consume, Local::now())
}

fn due_at(config: &AgentConfig, state: &mut TasksState, ctx: &DueContext, consume: bool, now: DateTime<Local>) -> Vec<(&'static str, AgentRepo)> {
    let mut out = Vec::new();
    for repo in &config.repos {
        if let Some(o) = &repo.offsite {
            let asked = requested(state, &repo.id, "offsite", consume);
            let paused = repo.active_pause(now).is_some();
            let held = ctx.holds.contains(&repo.id);
            let last = since(state, &key("offsite", &repo.id), &o.enabled_at);
            let scheduled = match &o.schedule {
                Schedule::AfterBackup { min_minutes } => after_backup_due(ctx.events.get(&repo.id).map(Vec::as_slice).unwrap_or(&[]), last, *min_minutes, now),
                s => s.is_due(last, now),
            };
            if asked || (!paused && !held && scheduled) {
                out.push(("offsite", repo.clone()));
            }
        }
        // Tarea 4b: las demás copias derivadas, cada una con su horario.
        for d in &repo.derived {
            let asked = requested(state, &repo.id, &derived_request(DERIVADA, &d.id), consume);
            let paused = repo.active_pause(now).is_some();
            let held = ctx.holds.contains(&repo.id);
            let last = since(state, &derived_key(DERIVADA, &repo.id, &d.id), &d.offsite.enabled_at);
            let scheduled = match &d.offsite.schedule {
                Schedule::AfterBackup { min_minutes } => after_backup_due(ctx.events.get(&repo.id).map(Vec::as_slice).unwrap_or(&[]), last, *min_minutes, now),
                s => s.is_due(last, now),
            };
            if asked || (!paused && !held && scheduled) {
                out.push((DERIVADA, derived_view(repo, d)));
            }
        }
    }
    for repo in &config.repos {
        if let Some(v) = &repo.verify {
            let asked = requested(state, &repo.id, "verify", consume);
            let paused = repo.active_pause(now).is_some();
            if asked || (!paused && v.schedule.is_due(since(state, &key("verify", &repo.id), &v.enabled_at), now)) {
                out.push(("verify", repo.clone()));
            }
        }
    }
    // Prueba de restauración: no en los destinos que solo reciben una copia
    // externa (se prueban desde su origen).
    for repo in &config.repos {
        if let Some(t) = &repo.restore_test {
            let target_of_offsite = config.repos.iter().any(|r| r.offsite.as_ref().is_some_and(|o| o.provider == format!("destino:{}", repo.id)));
            let asked = requested(state, &repo.id, "restore_test", consume);
            let paused = repo.active_pause(now).is_some();
            if asked || (!paused && !target_of_offsite && t.schedule.is_due(since(state, &key("restore_test", &repo.id), &t.enabled_at), now)) {
                out.push(("restore_test", repo.clone()));
            }
        }
    }
    // La verificación de la copia externa, siempre después de la subida (las
    // tareas van de una en una: nunca a la vez que la subida del mismo destino).
    // Con la subida frenada por un cambio inusual sí se verifica: solo lee, y
    // sirve para saber que lo que hay en la nube está bien.
    for repo in &config.repos {
        if let Some(v) = repo.offsite.as_ref().and_then(|o| o.verify.as_ref()) {
            let asked = requested(state, &repo.id, "verify_offsite", consume);
            let paused = repo.active_pause(now).is_some();
            if asked || (!paused && v.schedule.is_due(since(state, &key("verify_offsite", &repo.id), &v.enabled_at), now)) {
                out.push(("verify_offsite", repo.clone()));
            }
        }
        for d in &repo.derived {
            let Some(v) = d.offsite.verify.as_ref() else { continue };
            let asked = requested(state, &repo.id, &derived_request(VERIFY_DERIVADA, &d.id), consume);
            let paused = repo.active_pause(now).is_some();
            if asked || (!paused && v.schedule.is_due(since(state, &derived_key(VERIFY_DERIVADA, &repo.id, &d.id), &v.enabled_at), now)) {
                out.push((VERIFY_DERIVADA, derived_view(repo, d)));
            }
        }
    }
    out
}

/// Pide una ejecución inmediata (la app sin administrador; la atiende el agente en ≤ 5 min).
pub fn request_now(repo_id: &str, kind: &str) -> Result<(), String> {
    let config = agent::load_config();
    let repo = config.repos.iter().find(|r| r.id == repo_id).ok_or("Este repositorio no está en el agente.")?;
    let configured = match kind {
        "verify" => repo.verify.is_some(),
        "offsite" => repo.offsite.is_some(),
        "verify_offsite" => repo.offsite.as_ref().is_some_and(|o| o.verify.is_some()),
        "restore_test" => repo.restore_test.is_some(),
        k => match k.split_once('-') {
            Some((DERIVADA, id)) => repo.derived.iter().any(|d| d.id == id),
            Some((VERIFY_DERIVADA, id)) => repo.derived.iter().any(|d| d.id == id && d.offsite.verify.is_some()),
            _ => false,
        },
    };
    if !configured {
        return Err("Esa tarea no está programada en este repositorio.".into());
    }
    let body = serde_json::json!({ "requested_at": Local::now().to_rfc3339() });
    fs::write(agent::requests_dir().join(format!("{repo_id}.{kind}")), body.to_string()).map_err(|e| format!("No se pudo dejar la solicitud al agente: {e}"))
}

// ---------- Proceso de tareas ----------

/// Bloqueo del proceso de tareas (en la carpeta privada, como el del agente).
struct TasksLock(#[allow(dead_code)] fs::File);

impl TasksLock {
    fn acquire() -> Option<Self> {
        let dir = if agent::private_dir().is_dir() { agent::private_dir() } else { agent::agent_dir() };
        let _ = fs::create_dir_all(&dir);
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(false);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0);
        }
        let f = options.open(dir.join("tasks.lock")).ok()?;
        #[cfg(unix)]
        {
            if !agent::bloqueo_exclusivo(&f) {
                return None;
            }
        }
        Some(Self(f))
    }
}

/// Lo llama el agente al final de cada vuelta: si alguna tarea toca y no hay
/// otra en marcha, lanza el proceso de tareas y sigue (no lo espera).
pub fn spawn_if_due(config: &AgentConfig) {
    let mut state = load_state();
    let ctx = DueContext::load(config, Local::now());
    // También si hay copias nuevas que el freno aún no revisó.
    if due(config, &mut state, &ctx, false).is_empty() && !check_guards(config, &ctx, false) {
        return;
    }
    // ¿Ya hay un proceso de tareas? (Se prueba el bloqueo y se suelta.)
    match TasksLock::acquire() {
        Some(lock) => drop(lock),
        None => return,
    }
    if agent::test_mode() {
        run(); // en los tests, en el mismo proceso
        return;
    }
    let Ok(exe) = std::env::current_exe() else { return };
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--agent-tasks").stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        // Fuera del trabajo de la tarea programada, para que el Programador
        // no lo cuente como «la tarea sigue en ejecución».
        cmd.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS | CREATE_BREAKAWAY_FROM_JOB);
        if cmd.spawn().is_ok() {
            return;
        }
        cmd.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
    }
    if let Err(e) = cmd.spawn() {
        agent::log(&format!("ERROR: no se pudo iniciar el proceso de tareas: {e}"));
    }
}

/// `resguardo.exe --agent-tasks`: hace las tareas que tocan, una tras otra.
pub fn run() -> i32 {
    restic::require_bundled();
    if !agent::dirs_safe() {
        return 2;
    }
    let Some(_lock) = TasksLock::acquire() else { return 0 };
    let config = agent::load_config();
    let secrets = match agent::load_secrets() {
        Ok(s) => s,
        Err(e) => {
            agent::log(&format!("ERROR: {e}"));
            return 1;
        }
    };
    let mut state = load_state();
    state.running = None;
    // Primero el freno: una copia inusual frena la subida antes de decidir qué toca.
    check_guards(&config, &DueContext::load(&config, Local::now()), true);
    let ctx = DueContext::load(&config, Local::now());
    let todo = due(&config, &mut state, &ctx, true);
    save_state(&mut state);

    let mut failures = 0;
    for (kind, repo) in todo {
        let Some(secret) = secrets.get(&repo.id) else {
            agent::log(&format!("ERROR: «{}» no tiene contraseña guardada en el agente.", repo.name));
            continue;
        };
        // Tarea 4b: una derivada usa sus credenciales como si fuera la copia externa.
        let derivada = repo.derived.first().filter(|_| kind == DERIVADA || kind == VERIFY_DERIVADA).cloned();
        let secret_derivada = derivada.as_ref().map(|d| derived_secret(secret, &d.id));
        let secret = secret_derivada.as_ref().unwrap_or(secret);
        let destino = repo.offsite.as_ref().and_then(|o| o.target_name.clone()).unwrap_or_default();
        let what = match kind {
            "verify" => "Verificación".to_string(),
            "verify_offsite" => "Verificación de la copia en la nube".to_string(),
            "restore_test" => "Prueba de restauración".to_string(),
            VERIFY_DERIVADA => format!("Verificación de la copia derivada a «{destino}»"),
            DERIVADA => format!("Copia derivada a «{destino}»"),
            _ => "Copia externa".to_string(),
        };
        let quiet = if agent::apply_discreet(&config) { " (modo discreto: prioridad baja)" } else { "" };
        agent::log(&format!("{what} de «{}»…{quiet}", repo.name));
        state.running = Some(RunningTask {
            repo_id: repo.id.clone(),
            kind: kind.into(),
            started: Local::now().to_rfc3339(),
            stage: "Preparando…".into(),
            done: 0,
            updated: Some(Local::now().to_rfc3339()),
            ..Default::default()
        });
        save_state(&mut state);

        // Verificación rotativa: la parte que toca.
        let rot_key = task_rotation_key(kind, &repo);
        let parts = task_verify(&repo, kind).map_or(0, |v| v.rotate_parts);
        let part = current_part(state.rotation.get(&rot_key), parts);
        let mut last_write = std::time::Instant::now();
        let mut last_web = std::time::Instant::now();
        let mut ritmo = crate::progreso_v2::RitmoIo::default();
        let mut report = |p: &TaskProgress| {
            // Se llama desde la salida de restic (en su hilo): el restic que corre ahora.
            let (lectura, escritura) = ritmo.medir(restic::pid_en_marcha());
            if let Some(r) = state.running.as_mut() {
                r.read_bps = lectura;
                r.upload_bps = escritura;
                r.stage = p.stage.clone();
                r.done = p.done;
                r.total = p.total;
                r.percent = p.percent;
                r.eta_s = p.eta_s;
                r.bytes_done = p.bytes_done;
                r.bytes_total = p.bytes_total;
                r.current_snapshot_time = p.current_snapshot_time.clone();
                r.updated = Some(Local::now().to_rfc3339());
            }
            if last_write.elapsed() >= PROGRESS_EVERY {
                last_write = std::time::Instant::now();
                save_state(&mut state);
            }
            // La web también ve el avance de una subida larga (en segundo plano:
            // no se detiene la lectura de restic esperando a la red).
            if last_web.elapsed() >= WEB_EVERY {
                last_web = std::time::Instant::now();
                save_state(&mut state);
                std::thread::spawn(crate::web::report_detached);
            }
        };
        let record = match kind {
            "verify" => verify_repo(source_access(&repo, secret), repo.verify.as_ref().unwrap(), part, &mut report),
            "restore_test" => {
                let mut stage_report = |stage: &str, percent: Option<f64>| {
                    report(&TaskProgress { stage: stage.into(), percent, ..Default::default() });
                };
                crate::restore_test::run(&source_access(&repo, secret), repo.restore_test.as_ref().unwrap(), &mut stage_report)
            }
            "verify_offsite" | VERIFY_DERIVADA => {
                // El destino de la copia externa (o de la derivada), con las credenciales de la subida.
                let o = repo.offsite.as_ref().unwrap();
                match dest_access_listo(o, secret) {
                    Ok(acc) => verify_repo(acc, o.verify.as_ref().unwrap(), part, &mut report),
                    Err(e) => failed(RunRecord { started: Local::now().to_rfc3339(), ..Default::default() }, e),
                }
            }
            _ => offsite_repo(&repo, secret, repo.offsite.as_ref().unwrap(), load_guard().holds.contains_key(&repo.id), &mut report),
        };
        let tag = match record.result.as_str() {
            "ok" => "",
            "warning" => "AVISO: ",
            _ => "ERROR: ",
        };
        agent::log(&format!("{tag}{what} de «{}»: {}", repo.name, record.message));
        if record.result == "error" {
            failures += 1;
        }
        state.running = None;
        // Solo avanza si salió bien: una verificación fallida o cortada repite la parte.
        if kind == "verify" || kind == "verify_offsite" || kind == VERIFY_DERIVADA {
            if let Some(next) = rotation_after(state.rotation.get(&rot_key), parts, part, &record) {
                state.rotation.insert(rot_key.clone(), next);
            }
        }
        // El estado y el historial los leen todos los usuarios: sin rutas.
        let mut record = record;
        record.message = crate::web::local_message(&record.message);
        crate::history::append(
            &crate::history::agent_file(),
            &crate::history::Entry {
                kind: kind.into(),
                origin: "agent".into(),
                repo_id: repo.id.clone(),
                repo_name: repo.name.clone(),
                started: record.started.clone(),
                finished: record.finished.clone(),
                result: record.result.clone(),
                // Tarea 4b: de qué derivada es (en el historial no hay otro campo para decirlo).
                message: if derivada.is_some() { format!("«{destino}»: {}", record.message) } else { record.message.clone() },
                files_new: record.files_new,
                ..Default::default()
            },
        );
        state.runs.insert(task_key(kind, &repo), record);
        save_state(&mut state);
    }
    if failures > 0 {
        2
    } else {
        0
    }
}

// ---------- Accesos ----------

pub fn source_access(repo: &AgentRepo, secret: &Secret) -> Access {
    Access {
        location: repo.location.clone(),
        password: secret.password.clone(),
        rest_auth: repo.rest_username.clone().zip(secret.rest_password.clone()),
        cacert: repo.cacert.clone(),
        env: secret.env.clone(),
    }
}

/// Variables de credenciales según el tipo de destino.
pub fn cloud_env(location: &str, region: Option<&str>, key_id: Option<&str>, key_secret: Option<&str>) -> Vec<(String, String)> {
    let mut env = Vec::new();
    let (id_var, secret_var) = if location.starts_with("b2:") {
        ("B2_ACCOUNT_ID", "B2_ACCOUNT_KEY")
    } else if location.starts_with("azure:") {
        ("AZURE_ACCOUNT_NAME", "AZURE_ACCOUNT_KEY")
    } else {
        ("AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY")
    };
    if let (Some(id), Some(secret)) = (key_id.filter(|s| !s.is_empty()), key_secret.filter(|s| !s.is_empty())) {
        env.push((id_var.to_string(), id.to_string()));
        env.push((secret_var.to_string(), secret.to_string()));
    }
    if let Some(region) = region.filter(|r| !r.is_empty()) {
        env.push(("AWS_DEFAULT_REGION".to_string(), region.to_string()));
    }
    env
}

/// Acceso al repositorio de destino. La contraseña es la propia si se eligió
/// otra; si no, la misma del repositorio de origen.
fn dest_access(offsite: &Offsite, secret: &Secret) -> Access {
    // Hacia otro destino de la app: su ubicación (con usuario de servidor si
    // lo tiene) y sus variables de nube, guardadas en los secretos.
    if let Some(location) = secret.offsite_location.as_ref().filter(|_| offsite.provider.starts_with("destino:")) {
        return Access {
            location: location.clone(),
            password: secret.offsite_password.clone().unwrap_or_else(|| secret.password.clone()),
            rest_auth: None,
            cacert: None,
            env: secret.offsite_env.clone(),
        };
    }
    Access {
        location: offsite.location.clone(),
        password: secret.offsite_password.clone().unwrap_or_else(|| secret.password.clone()),
        rest_auth: None,
        cacert: None,
        env: cloud_env(&offsite.location, offsite.region.as_deref(), secret.offsite_key_id.as_deref(), secret.offsite_secret.as_deref()),
    }
}

/// El acceso al destino listo para usar: con una nube (tarea 4a), con sus
/// credenciales de ahora (el token al día; nunca guardadas con la copia).
fn dest_access_listo(offsite: &Offsite, secret: &Secret) -> Result<Access, String> {
    let mut a = dest_access(offsite, secret);
    if let Some(n) = &offsite.dest.nube {
        a.env = crate::nube::entorno_restic(n)?;
    }
    Ok(a)
}

/// Ubicación de restic con el usuario y la contraseña del servidor REST
/// incrustados (`rest:https://usuario:clave@host/ruta`). Solo se usa en la
/// variable de entorno del proceso de restic, nunca en archivos legibles.
pub fn location_with_auth(location: &str, auth: Option<&(String, String)>) -> String {
    let Some((user, pass)) = auth else { return location.to_string() };
    let enc = |s: &str| {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
                _ => format!("%{b:02X}"),
            })
            .collect::<String>()
    };
    for scheme in ["rest:https://", "rest:http://"] {
        if let Some(rest) = location.strip_prefix(scheme) {
            // Si la ubicación ya trae un usuario (`ana@host`), se sustituye.
            let host_and_path = match rest.find('/') {
                Some(slash) => {
                    let (authority, path) = rest.split_at(slash);
                    format!("{}{path}", authority.rsplit('@').next().unwrap_or(authority))
                }
                None => rest.rsplit('@').next().unwrap_or(rest).to_string(),
            };
            return format!("{scheme}{}:{}@{host_and_path}", enc(user), enc(pass));
        }
    }
    location.to_string()
}

/// `restic -r DESTINO <cmd> --from-repo ORIGEN`: credenciales del destino, y
/// del origen su contraseña (RESTIC_FROM_PASSWORD) y su usuario de rest-server.
fn copy_access(src: &Access, dest: &Access) -> Access {
    let mut env = dest.env.clone();
    env.push(("RESTIC_FROM_PASSWORD".into(), src.password.clone()));
    Access { location: dest.location.clone(), password: dest.password.clone(), rest_auth: src.rest_auth.clone(), cacert: src.cacert.clone(), env }
}

// ---------- Verificación ----------

/// Traduce las líneas de `restic check` a una frase. `reading`: qué se lee
/// («el 5 %», «la parte 2 de 4»).
fn check_stage(line: &str, reading: &str) -> Option<String> {
    let l = line.trim().to_lowercase();
    let s = if l.starts_with("load indexes") {
        "Cargando índices…".to_string()
    } else if l.starts_with("check all packs") {
        "Revisando los paquetes de datos…".to_string()
    } else if l.starts_with("check snapshots") {
        "Revisando copias, carpetas y bloques…".to_string()
    } else if l.starts_with("read ") {
        format!("Leyendo {reading} de los datos…")
    } else if l.contains("lock") && l.contains("create") {
        "Esperando el repositorio…".to_string()
    } else {
        return None;
    };
    Some(s)
}

/// `restic check` con la lectura de datos que toque (ver `Verify::subset_arg`).
pub fn verify_args(verify: &Verify, part: u32) -> Vec<String> {
    let mut args: Vec<String> = vec!["check".into(), "--retry-lock".into(), "30m".into()];
    if let Some(subset) = verify.subset_arg(part) {
        args.push("--read-data-subset".into());
        args.push(subset);
    }
    args
}

/// Lo que dice una verificación que encontró datos dañados. Antes: «Revisa el registro y
/// ejecuta «restic check» en el servidor», que no le dice qué hacer a quien mira la consola
/// (prueba de resistencia con un archivo del almacén estropeado, docs/estabilidad.md).
pub const DATOS_DANADOS: &str =
    "Hay datos dañados en el destino: algún archivo de las copias no se lee bien. Revisa su disco y pide a tu soporte que lo repare («restic repair packs»).";

/// ¿La salida de `restic check` (en minúsculas) habla de datos dañados (no de conexión, contraseña…)?
fn datos_danados(text: &str) -> bool {
    text.contains("error") && (text.contains("pack") || text.contains("blob") || text.contains("tree"))
}

/// `part`: la parte de la verificación rotativa que toca (1 a `rotate_parts`).
/// `access`: el destino (o su copia externa) que se verifica.
fn verify_repo(mut access: Access, verify: &Verify, part: u32, report: &mut dyn FnMut(&TaskProgress)) -> RunRecord {
    let mut record = RunRecord { started: Local::now().to_rfc3339(), ..Default::default() };
    access.env.push((RESTIC_FPS.0.into(), RESTIC_FPS.1.into()));
    let percent = verify.subset_percent.min(100);
    let parts = verify.rotate_parts;
    let part = part.clamp(1, parts.max(1));
    let reading = if parts > 0 { format!("la parte {part} de {parts}") } else { format!("el {percent} %") };
    let args = verify_args(verify, part);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    // Cada fase (revisar copias, leer datos) tiene su barra: porcentaje y tiempo
    // restante de la fase en curso.
    let mut current = TaskProgress::stage(&if parts > 0 { format!("Preparando la parte {part} de {parts}…") } else { "Preparando…".into() });
    report(&current);
    let mut phase_start = std::time::Instant::now();
    let mut on_line = |line: &str| {
        if let Some(stage) = check_stage(line, &reading) {
            current = TaskProgress::stage(&stage);
            phase_start = std::time::Instant::now();
            report(&current);
        } else if let Some((p, a, b, _)) = parse_progress(line) {
            current.percent = Some(p);
            current.done = a;
            current.total = Some(b);
            current.eta_s = eta(phase_start.elapsed(), p);
            report(&current);
        }
    };
    match restic::run_raw_lines(&access, &refs, TASK_TIMEOUT, &mut on_line) {
        Ok(out) if out.code == Some(0) => {
            record.result = "ok".into();
            record.message = if parts > 0 {
                format!("Parte {part} de {parts} verificada sin errores (en {parts} verificaciones se leen todos los datos).")
            } else if percent > 0 {
                format!("Sin errores (se leyó el {percent} % de los datos).")
            } else {
                "Sin errores en la estructura del repositorio.".into()
            };
        }
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout).to_lowercase() + &out.stderr.to_lowercase();
            record.result = "error".into();
            record.message = if datos_danados(&text) { DATOS_DANADOS.into() } else { restic::exit_error(out.code, &out.stderr) };
        }
        Err(e) => {
            record.result = "error".into();
            record.message = e;
        }
    }
    record.finished = Local::now().to_rfc3339();
    record
}

// ---------- Copia externa ----------

/// Prepara el destino: si ya es un repositorio, comprueba que la contraseña
/// sirve; si no existe, lo crea con los mismos parámetros de troceado que el
/// origen (así la deduplicación entre ambos es completa y se sube menos).
/// Devuelve "existing" o "created".
pub fn prepare_destination(src: &Access, dest: &Access) -> Result<&'static str, String> {
    let probe = restic::run_raw(dest, &["cat", "config", "--no-lock"], restic::CHECK_TIMEOUT)?;
    match probe.code {
        Some(0) => Ok("existing"),
        Some(10) => {
            let both = copy_access(src, dest);
            let out = restic::run_raw(&both, &["init", "--from-repo", &src.location, "--copy-chunker-params"], restic::CHECK_TIMEOUT)?;
            if out.code == Some(0) {
                Ok("created")
            } else {
                Err(restic::exit_error(out.code, &out.stderr))
            }
        }
        code => Err(restic::exit_error(code, &probe.stderr)),
    }
}

/// Antes de cada subida: el destino tiene que existir. Uno nuevo se crea la
/// primera vez (con los parámetros de troceado del origen); uno que ya
/// existía («Usar uno que ya existe») nunca se vuelve a crear: si no aparece,
/// es que algo cambió (otra dirección, otro bucket) y crear uno vacío lo
/// subiría todo de nuevo.
pub fn ensure_destination(src: &Access, dest: &Access, existing: bool) -> Result<&'static str, String> {
    if !existing {
        return prepare_destination(src, dest);
    }
    let probe = restic::run_raw(dest, &["cat", "config", "--no-lock"], restic::CHECK_TIMEOUT)?;
    match probe.code {
        Some(0) => Ok("existing"),
        Some(10) => Err(MISSING_EXISTING.into()),
        code => Err(restic::exit_error(code, &probe.stderr)),
    }
}

pub const MISSING_EXISTING: &str = "El repositorio de la copia externa (uno que ya existía) no aparece en su destino: revisa la dirección y las credenciales. \
                                    No se crea otro allí para no tener que subirlo todo de nuevo.";

/// El polinomio de troceado de un repositorio (`restic cat config`): dos
/// repositorios con el mismo trocean igual los archivos y comparten los bloques.
pub fn chunker_polynomial(acc: &Access) -> Result<String, String> {
    let out = restic::run_raw(acc, &["cat", "config", "--no-lock"], restic::CHECK_TIMEOUT)?;
    if out.code != Some(0) {
        return Err(restic::exit_error(out.code, &out.stderr));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let json = text.lines().skip_while(|l| !l.trim_start().starts_with('{')).collect::<Vec<_>>().join("\n");
    let v: serde_json::Value = serde_json::from_str(&json).map_err(|e| format!("Respuesta inesperada de restic: {e}"))?;
    v["chunker_polynomial"].as_str().map(str::to_string).ok_or_else(|| "Respuesta inesperada de restic: falta el troceado.".into())
}

/// ¿Ya está esa versión del origen en el destino? Como `restic copy`: por su
/// id o por el de la versión de la que es copia (`original`). Así una versión
/// traída de otro repositorio (`copiar_historial`) que ya se subió desde
/// aquel no se cuenta como pendiente.
fn already_in(present: &std::collections::HashSet<String>, s: &restic::Snapshot) -> bool {
    present.contains(&s.id) || s.original.as_ref().is_some_and(|o| present.contains(o))
}

/// Cuántas versiones no se pudieron quitar en un `restic forget` (cada una sale
/// como «unable to remove snapshots/…» y restic termina igual con 0).
fn forget_blocked(text: &str) -> usize {
    text.lines().filter(|l| l.to_lowercase().contains("unable to remove")).count()
}

fn failed(mut record: RunRecord, message: String) -> RunRecord {
    record.result = "error".into();
    record.message = message;
    record.finished = Local::now().to_rfc3339();
    record
}

/// `held`: la subida está frenada por un cambio inusual y se hace porque se
/// pidió a mano: se sube, pero no se aplica la retención (no se borra nada allí).
fn offsite_repo(repo: &AgentRepo, secret: &Secret, offsite: &Offsite, held: bool, report: &mut dyn FnMut(&TaskProgress)) -> RunRecord {
    let mut record = RunRecord { started: Local::now().to_rfc3339(), ..Default::default() };
    let src = source_access(repo, secret);
    let dest = match dest_access_listo(offsite, secret) {
        Ok(d) => d,
        Err(e) => return failed(record, e),
    };
    let mut both = copy_access(&src, &dest);
    both.env.push((RESTIC_FPS.0.into(), RESTIC_FPS.1.into()));

    // Con retención en el destino, solo se suben las copias que esa política
    // conservaría (calculado por restic sobre el origen) y que aún no están en
    // el destino. Si no, cada subida volvería a subir las que la retención del
    // destino ya quitó, una y otra vez.
    // Con bloqueo de objetos, la retención de allí guarda además todo lo de esos días.
    let policy: Option<Policy> = offsite.retention.as_ref().filter(|p| !p.is_empty()).map(|p| match offsite.dest.lock_days() {
        Some(days) => locked_policy(p, days),
        None => p.clone(),
    });
    // Hacia un destino de la consola (`cambiar_copia_externa`), que el agente
    // crea la primera vez. Las de la app de escritorio ya lo traían creado.
    if offsite.provider.starts_with("destino:") {
        report(&TaskProgress::stage("Comprobando el destino…"));
        if let Err(e) = ensure_destination(&src, &dest, offsite.dest.existing) {
            return failed(record, e);
        }
    }
    report(&TaskProgress::stage("Calculando qué versiones subir…"));
    let wanted: Option<std::collections::HashSet<String>> = match &policy {
        None => None,
        Some(policy) => match crate::retention::preview(&src, policy) {
            Ok(p) => Some(p.items.into_iter().filter(|i| i.keep).map(|i| i.snapshot.id).collect()),
            Err(e) => return failed(record, format!("No se pudo calcular la retención: {e}")),
        },
    };
    let present: std::collections::HashSet<String> = match restic::snapshots(&dest) {
        Ok(list) => list.into_iter().flat_map(|s| [Some(s.id), s.original]).flatten().collect(),
        Err(e) => return failed(record, e),
    };
    // Lo que se sube: las versiones del origen que faltan en el destino (y que la
    // retención de allí conservaría), de la más antigua a la más reciente. En ese
    // orden (restic respeta el de los argumentos) la primera sube casi todo y las
    // demás solo sus cambios: su «datos añadidos» sirve de peso para el progreso.
    let mut skipped = 0usize;
    let mut todo: Vec<restic::Snapshot> = match restic::snapshots(&src) {
        Ok(list) => list
            .into_iter()
            .filter(|s| {
                // Tarea 4c: solo las que pasan el filtro (las demás, ni se cuentan).
                if offsite.filtro.as_ref().is_some_and(|f| !f.deja_en(s, Local::now())) {
                    return false;
                }
                let ya = already_in(&present, s);
                skipped += usize::from(ya);
                !ya && wanted.as_ref().is_none_or(|w| w.contains(&s.id))
            })
            .collect(),
        Err(e) => return failed(record, e),
    };
    todo.sort_by(|a, b| a.time.cmp(&b.time));
    let added = |s: &restic::Snapshot| s.summary.as_ref().and_then(|x| x.data_added_packed.or(x.data_added));
    // Cada versión pesa lo que añadió (y al menos 1 MB: restic también sube su árbol).
    let weights: Vec<u64> = todo.iter().map(|s| added(s).unwrap_or(0).max(1 << 20)).collect();
    // Bytes estimados solo si casi todas traen su resumen (restic 0.17+).
    let with_summary = todo.iter().filter(|s| added(s).is_some()).count();
    let bytes_total = (with_summary * 10 >= todo.len() * 9 && !todo.is_empty()).then(|| weights.iter().sum::<u64>());
    let total = todo.len() as u64;
    let clock = std::time::Instant::now();

    // restic escribe «snapshot X of [...] at FECHA …» al empezar cada copia, su
    // barra («[0:05] 40.00%  2 / 5 packs copied») y, al final de todo,
    // «snapshot Y saved, copied from source snapshot X» por cada una.
    let mut started: u64 = 0;
    let mut copied: u64 = 0;
    let mut current = TaskProgress {
        stage: "Preparando la subida…".into(),
        total: Some(total),
        percent: Some(0.0),
        bytes_total,
        bytes_done: bytes_total.map(|_| 0),
        ..Default::default()
    };
    report(&current);
    let mut on_line = |line: &str| {
        let l = line.trim();
        let mut update = |cur: &mut TaskProgress, done: usize, fraction: f64| {
            let p = weighted_progress(&weights, done, fraction);
            cur.percent = Some(p);
            cur.eta_s = eta(clock.elapsed(), p);
            cur.bytes_done = bytes_total.map(|t| (t as f64 * p) as u64);
            report(cur);
        };
        if l.starts_with("snapshot ") && l.contains(" of [") {
            started += 1;
            let when = l.split("] at ").nth(1).map(|t| t.chars().take(16).collect::<String>()).unwrap_or_default();
            let n = started.min(total.max(1));
            current.stage = format!("Subiendo la versión del {when} ({n} de {total})…");
            current.done = started - 1;
            current.current_snapshot_time = todo.get(started as usize - 1).map(|s| s.time.clone());
            update(&mut current, started as usize - 1, 0.0);
        } else if let Some((fraction, _, _, unit)) = parse_progress(l) {
            if unit == "packs" && started > 0 {
                update(&mut current, started as usize - 1, fraction);
            }
        } else if l.starts_with("snapshot ") && l.contains(" saved") {
            copied += 1;
        }
    };

    let limit = offsite.limit_upload_kib.filter(|k| *k > 0).map(|k| k.to_string());
    // Por tandas: la línea de órdenes de Windows tiene un límite de longitud.
    let ids: Vec<String> = todo.iter().map(|s| s.id.clone()).collect();
    let batches: Vec<Vec<String>> = ids.chunks(100).map(|c| c.to_vec()).collect();
    let mut result: Result<restic::RawOutput, String> = Ok(restic::RawOutput { code: Some(0), stdout: Vec::new(), stderr: String::new() });
    for batch in &batches {
        let mut args: Vec<&str> = vec!["copy", "--from-repo", src.location.as_str(), "--retry-lock", "30m"];
        if let Some(limit) = &limit {
            args.extend(["--limit-upload", limit.as_str()]);
        }
        args.extend(batch.iter().map(String::as_str));
        result = restic::run_raw_lines(&both, &args, TASK_TIMEOUT, &mut on_line);
        if !matches!(&result, Ok(out) if out.code == Some(0)) {
            break;
        }
    }
    match result {
        Ok(out) if out.code == Some(0) => {
            record.result = "ok".into();
            record.message = match copied.max(started) {
                0 => "Nada nuevo que subir: el repositorio ya estaba al día.".into(),
                1 => "1 copia subida.".into(),
                n => format!("{n} copias subidas."),
            };
            // La primera subida a un repositorio que ya existía: lo que ya estaba no se repite.
            if copied.max(started) > 0 && skipped > 0 {
                record.message.pop();
                record.message.push_str(&match skipped {
                    1 => " (1 ya estaba en el destino).".to_string(),
                    n => format!(" ({n} ya estaban en el destino)."),
                });
            }
            record.files_new = Some(copied.max(started));
        }
        Ok(out) => {
            record.result = "error".into();
            record.message = restic::exit_error(out.code, &out.stderr);
            record.finished = Local::now().to_rfc3339();
            return record;
        }
        Err(e) => {
            record.result = "error".into();
            record.message = e;
            record.finished = Local::now().to_rfc3339();
            return record;
        }
    }

    // Retención en el destino (el origen la gestiona el servidor). Con la subida
    // frenada no se borra nada allí: las versiones anteriores al cambio quedan a salvo.
    if held && policy.is_some() {
        record.message.push_str(" Retención no aplicada: la subida está frenada por un cambio inusual.");
    }
    // En un servidor de solo añadir no se puede borrar nada desde aquí.
    if offsite.dest.append_only && policy.is_some() && !held {
        record.message.push_str(" Retención no aplicada aquí: el destino es de solo añadir (la aplica el propio servidor).");
    }
    if let Some(policy) = policy.as_ref().filter(|_| !held && !offsite.dest.append_only) {
        report(&TaskProgress {
            stage: "Aplicando la retención en el repositorio…".into(),
            done: copied,
            total: Some(total),
            percent: Some(1.0),
            ..Default::default()
        });
        let lock = offsite.dest.lock_days();
        // Con bloqueo de objetos, sin `prune`: borraría (o reescribiría) datos
        // que aún no se pueden borrar y, al no poder, solo gastaría subida.
        let mut args: Vec<String> = vec!["forget".into()];
        if lock.is_none() {
            args.push("--prune".into());
        }
        args.extend(["--retry-lock".into(), "30m".into()]);
        args.extend(policy.args());
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        // Lo que había y lo que queda, para «Retención en detalle» (bitácora).
        let inicio = Local::now();
        let antes = restic::snapshots(&dest).ok();
        let salida = restic::run_raw(&dest, &refs, TASK_TIMEOUT);
        let resultado = match (&salida, lock) {
            (Ok(out), None) if out.code == Some(0) => Ok("Retención aplicada en el repositorio.".to_string()),
            (Ok(out), Some(days)) if out.code == Some(0) => {
                let mut m = format!(
                    "Retención aplicada sin liberar espacio (el destino tiene bloqueo de {days} días: se quitan versiones antiguas, sus datos se quedan)."
                );
                match forget_blocked(&out.stderr) {
                    0 => {}
                    1 => m.push_str(" 1 versión antigua sigue bloqueada: se quitará cuando venza su bloqueo."),
                    n => m.push_str(&format!(" {n} versiones antiguas siguen bloqueadas: se quitarán cuando venza su bloqueo.")),
                }
                Ok(m)
            }
            (Ok(out), Some(days)) => Err(format!(
                "La retención en el destino no se pudo aplicar (tiene bloqueo de {days} días; lo subido está a salvo): {}",
                restic::exit_error(out.code, &out.stderr)
            )),
            (Ok(out), None) => Err(format!("La retención en el repositorio falló: {}", restic::exit_error(out.code, &out.stderr))),
            (Err(e), _) => Err(format!("La retención en el repositorio falló: {e}")),
        };
        match &resultado {
            Ok(m) => record.message.push_str(&format!(" {m}")),
            Err(m) => {
                record.result = "warning".into();
                record.message.push_str(&format!(" {m}"));
            }
        }
        if let Some(antes) = &antes {
            use crate::retencion_registro as rr;
            let despues = restic::snapshots(&dest).ok();
            let regla = rr::retencion_de_politica(policy);
            let motivos = regla.as_ref().map(|r| rr::motivos(&rr::candidatas(antes), r)).unwrap_or_default();
            let texto = salida.as_ref().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
            rr::anotar(&rr::Vuelta {
                origen: "externa",
                por: "automatica",
                repo: &repo.id,
                usuario: None,
                regla: regla.as_ref(),
                inicio,
                antes,
                despues: despues.as_deref(),
                motivos: &motivos,
                copias: &crate::informe_v2::copias_por_version(),
                liberado: rr::liberado(&texto).or(despues.as_ref().filter(|d| d.len() == antes.len()).map(|_| 0)),
                sospechosas: None,
                resultado: match &resultado {
                    Ok(m) => Ok(m.as_str()),
                    Err(m) => Err(m.as_str()),
                },
            });
        }
    }
    record.finished = Local::now().to_rfc3339();
    record
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verificacion_con_datos_danados() {
        // Lo que dice restic 0.18 con un archivo de datos estropeado (`check --read-data-subset`).
        let salida = "check snapshots, trees and blobs\nread 10.0% of data packs\nPack ID does not match, want 6751b1e7, got 3a2c91d0\n\
                      pack 6751b1e7 contains 1 errors: [blob 9f: decrypting blob 9f failed: ciphertext verification failed]\nFatal: repository contains errors";
        assert!(datos_danados(&salida.to_lowercase()));
        assert!(DATOS_DANADOS.contains("Revisa su disco") && !DATOS_DANADOS.contains("ejecuta"));
        // Cabe entero en el `mensaje_corto` del informe (160): en la prueba de resistencia salía cortado.
        assert!(DATOS_DANADOS.chars().count() <= 160, "{}", DATOS_DANADOS.chars().count());
        // Un fallo de conexión o de contraseña no es «datos dañados».
        assert!(!datos_danados("fatal: wrong password or no key found"));
        assert!(!datos_danados("fatal: unable to open repository: dial tcp: connection refused"));
    }

    #[test]
    fn credenciales_segun_destino() {
        let env = cloud_env("s3:https://s3.us-west-004.backblazeb2.com/b/r", Some("us-west-004"), Some("id"), Some("sec"));
        assert!(env.contains(&("AWS_ACCESS_KEY_ID".into(), "id".into())));
        assert!(env.contains(&("AWS_DEFAULT_REGION".into(), "us-west-004".into())));
        let env = cloud_env("b2:bucket:carpeta", None, Some("id"), Some("sec"));
        assert!(env.contains(&("B2_ACCOUNT_KEY".into(), "sec".into())));
        assert!(cloud_env(r"E:\copia", None, None, None).is_empty());
    }

    #[test]
    fn ubicacion_con_usuario_de_servidor() {
        let auth = ("siigo".to_string(), "cl@ve:1".to_string());
        assert_eq!(location_with_auth("rest:http://192.168.1.30:8001/siigo", Some(&auth)), "rest:http://siigo:cl%40ve%3A1@192.168.1.30:8001/siigo");
        assert_eq!(location_with_auth("s3:https://x/b", Some(&auth)), "s3:https://x/b");
        assert_eq!(location_with_auth("rest:https://ana@h:8000/r", Some(&auth)), "rest:https://siigo:cl%40ve%3A1@h:8000/r");
        assert_eq!(location_with_auth("rest:https://h/r", None), "rest:https://h/r");
    }

    #[test]
    fn la_pausa_salta_verificacion_y_copia_externa() {
        let at = |s: &str| {
            use chrono::TimeZone;
            Local.from_local_datetime(&chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()).unwrap()
        };
        let hace_rato = at("2026-09-30 06:00").to_rfc3339();
        let mut repo = crate::agent::tests::repo_cada_hora();
        // Id propio: ninguna solicitud «Ahora» de verdad puede coincidir.
        repo.id = format!("prueba-pausa-{}", std::process::id());
        repo.verify = Some(Verify { schedule: Schedule::Hours { every: 1 }, subset_percent: 0, enabled_at: hace_rato.clone(), rotate_parts: 0 });
        repo.offsite = Some(Offsite {
            location: r"E:\copia".into(),
            provider: "otro".into(),
            region: None,
            schedule: Schedule::Hours { every: 1 },
            retention: None,
            limit_upload_kib: None,
            target_name: None,
            enabled_at: hace_rato,
            guard: None,
            verify: None,
            dest: Default::default(),
            filtro: None,
        });
        let now = at("2026-09-30 10:30");
        let mut config = AgentConfig { repos: vec![repo], ..Default::default() };
        let mut state = TasksState::default();
        let ctx = DueContext::default();
        assert_eq!(due_at(&config, &mut state, &ctx, false, now).len(), 2, "sin pausa tocan las dos");
        config.repos[0].pause = Some(crate::agent::Pause { since: now.to_rfc3339(), until: Some(at("2026-09-30 12:00").to_rfc3339()) });
        assert!(due_at(&config, &mut state, &ctx, false, now).is_empty(), "en pausa, ninguna");
        assert_eq!(due_at(&config, &mut state, &ctx, false, at("2026-09-30 12:01")).len(), 2, "al terminar la pausa, vuelven");
        // Con la subida frenada, la copia externa espera (la verificación no).
        config.repos[0].pause = None;
        let held = DueContext { holds: [config.repos[0].id.clone()].into(), ..Default::default() };
        let todo = due_at(&config, &mut state, &held, false, now);
        assert_eq!(todo.iter().map(|(k, _)| *k).collect::<Vec<_>>(), ["verify"]);
    }

    fn t(s: &str) -> DateTime<Local> {
        use chrono::TimeZone;
        Local.from_local_datetime(&chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()).unwrap()
    }

    fn ev(when: &str, id: &str, bytes: u64, files: u64) -> BackupEvent {
        BackupEvent { finished: t(when), snapshot_id: id.into(), plan_name: "Principal".into(), data_added: bytes, files }
    }

    #[test]
    fn despues_de_cada_copia_con_cambios() {
        let last_upload = t("2026-10-01 09:00");
        let events = vec![ev("2026-10-01 08:30", "a1", 10, 1)];
        // Sin copias nuevas desde la última subida: no toca.
        assert!(!after_backup_due(&events, last_upload, 30, t("2026-10-01 12:00")));
        let events = vec![ev("2026-10-01 08:30", "a1", 10, 1), ev("2026-10-01 09:10", "b2", 10, 1)];
        // Hay una nueva, pero como mucho cada 30 min desde la última subida.
        assert!(!after_backup_due(&events, last_upload, 30, t("2026-10-01 09:20")));
        assert!(after_backup_due(&events, last_upload, 30, t("2026-10-01 09:30")));
        assert!(after_backup_due(&events, last_upload, 0, t("2026-10-01 09:11")));

        // Las copias «sin cambios» o fallidas no cuentan.
        let mut repo = crate::agent::tests::repo_cada_hora();
        repo.offsite = Some(Offsite {
            location: "s3:x".into(),
            provider: "b2".into(),
            region: None,
            schedule: Schedule::AfterBackup { min_minutes: 30 },
            retention: None,
            limit_upload_kib: None,
            target_name: None,
            enabled_at: t("2026-10-01 06:00").to_rfc3339(),
            guard: None,
            verify: None,
            dest: Default::default(),
            filtro: None,
        });
        let entry = |finished: &str, snap: Option<&str>, unchanged: bool, result: &str| crate::history::Entry {
            kind: "backup".into(),
            origin: "agent".into(),
            repo_id: repo.id.clone(),
            finished: t(finished).to_rfc3339(),
            result: result.into(),
            snapshot_id: snap.map(String::from),
            unchanged,
            ..Default::default()
        };
        let history = vec![
            entry("2026-10-01 10:00", None, true, "ok"),
            entry("2026-10-01 10:05", Some("c3"), false, "error"),
            entry("2026-10-01 10:10", Some("d4"), false, "ok"),
        ];
        let state = agent::AgentState::default();
        let now = t("2026-10-01 11:00");
        let events = backup_events(&repo, &state, &history, now);
        assert_eq!(events.iter().map(|e| e.snapshot_id.as_str()).collect::<Vec<_>>(), ["d4"]);

        // En el proceso de tareas: toca (una versión nueva después de la última subida)…
        let config = AgentConfig { repos: vec![repo.clone()], ..Default::default() };
        let mut state = TasksState::default();
        state.runs.insert(key("offsite", &repo.id), RunRecord { started: t("2026-10-01 09:00").to_rfc3339(), ..Default::default() });
        let ctx = DueContext { events: [(repo.id.clone(), events)].into(), ..Default::default() };
        assert_eq!(due_at(&config, &mut state, &ctx, false, now).len(), 1);
        // …pero no en pausa.
        let mut paused = config;
        paused.repos[0].pause = Some(agent::Pause { since: now.to_rfc3339(), until: None });
        assert!(due_at(&paused, &mut state, &ctx, false, now).is_empty());
    }

    #[test]
    fn freno_ante_cambios_inusuales() {
        let g = Guard::default();
        assert_eq!((g.factor, g.min_bytes, g.min_files), (20, 2 * 1024 * 1024 * 1024, 5000));
        let mb = 1024 * 1024;
        // Pocas copias (menos de 5): solo cuentan los mínimos absolutos.
        let prior = vec![ev("2026-10-01 08:00", "a", 20 * mb, 40), ev("2026-10-01 09:00", "b", 20 * mb, 40)];
        assert!(unusual(&prior, &ev("2026-10-01 10:00", "x", 1024 * mb, 4000), &g).is_none(), "1 GB y 4.000 archivos: por debajo de los mínimos");
        assert_eq!(unusual(&prior, &ev("2026-10-01 10:00", "x", 3 * 1024 * mb, 10), &g), Some((None, None)));
        assert!(unusual(&prior, &ev("2026-10-01 10:00", "x", 10, 6000), &g).is_some());

        // Con historial: mediana ~20 MB y ~40 archivos.
        let prior: Vec<BackupEvent> = (0..25).map(|i| ev(&format!("2026-09-30 {:02}:00", i % 24), &format!("p{i}"), (15 + i % 10) * mb, 30 + i % 20)).collect();
        let normal = unusual(&prior, &ev("2026-10-01 10:00", "x", 300 * mb, 500), &g);
        assert!(normal.is_none(), "15 veces lo normal y bajo los mínimos: no frena");
        let big = unusual(&prior, &ev("2026-10-01 10:00", "x", 38 * 1024 * mb, 12_400), &g).expect("38 GB es inusual");
        assert!(big.0.is_some_and(|b| (15 * mb..25 * mb).contains(&b)));
        assert!(big.1.is_some_and(|f| (30..50).contains(&f)));
        // Con un mínimo bajo, el factor decide: más de 20 veces la mediana.
        let low = Guard { min_bytes: mb, min_files: 1, ..Guard::default() };
        assert!(unusual(&prior, &ev("2026-10-01 10:00", "x", 1000 * mb, 10), &low).is_some());
        assert!(unusual(&prior, &ev("2026-10-01 10:00", "x", 100 * mb, 10), &low).is_none());

        // Mensaje.
        let h = Hold {
            since: String::new(),
            snapshot_id: "abc".into(),
            plan_name: "Siigo".into(),
            data_added: 38 * 1024 * 1024 * mb / 1024,
            files: 12_400,
            typical_bytes: Some(20 * mb),
            typical_files: Some(40),
            backup_finished: t("2026-10-01 17:00").to_rfc3339(),
        };
        assert_eq!(
            hold_message(&h),
            "La copia «Siigo» de las 17:00 añadió 38 GB y 12.400 archivos (lo normal: ~20 MB y ~40). La subida a la nube está frenada por precaución."
        );
        assert!(Guard { factor: 1, ..Guard::default() }.validate().is_err());
    }

    #[test]
    fn configuracion_anterior_de_la_copia_externa() {
        // Sin `guard` y con un horario de siempre.
        let old = r#"{"location":"s3:x","provider":"b2","schedule":{"kind":"daily","time":"03:00"},"enabled_at":"2026-09-29T08:00:00-05:00"}"#;
        let o: Offsite = serde_json::from_str(old).unwrap();
        assert!(o.guard.is_none());
        // «Después de cada copia»: forma exacta para la web; sin `min_minutes`, 30.
        let s: Schedule = serde_json::from_str(r#"{"kind":"after_backup"}"#).unwrap();
        assert_eq!(s, Schedule::AfterBackup { min_minutes: 30 });
        assert_eq!(serde_json::to_value(Schedule::AfterBackup { min_minutes: 45 }).unwrap(), serde_json::json!({ "kind": "after_backup", "min_minutes": 45 }));
        assert!(Schedule::AfterBackup { min_minutes: 1441 }.validate().is_err());
        assert!(Schedule::AfterBackup { min_minutes: 30 }.validate_offsite().is_ok());
        assert!(Schedule::AfterBackup { min_minutes: 30 }.validate_task().is_err(), "no vale para verificar");
        assert!(Schedule::Monitor { every: 24 }.validate_offsite().is_err());
        // Freno con campos que faltan: valores por defecto.
        let g: Guard = serde_json::from_str(r#"{"factor":10}"#).unwrap();
        assert_eq!(g, Guard { factor: 10, ..Guard::default() });
    }

    #[test]
    fn progreso_de_restic_y_ponderado() {
        assert_eq!(parse_progress("[0:05] 40.00%  2 / 5 packs copied"), Some((0.4, 2, 5, "packs".into())));
        assert_eq!(parse_progress("[1:02:03] 100.00%  6 / 6 snapshots"), Some((1.0, 6, 6, "snapshots".into())));
        assert_eq!(parse_progress("[0:00] 12.50%  1 / 8 packs"), Some((0.125, 1, 8, "packs".into())));
        assert!(parse_progress("snapshot 0082661c of [C:\\datos] at 2026-09-29").is_none());
        assert!(parse_progress("no errors were found").is_none());
        // La primera versión sube casi todo: a mitad de ella va por el ~50 %, no por el 1 de 4.
        let w = [1000, 10, 10, 10];
        assert!((weighted_progress(&w, 0, 0.5) - 500.0 / 1030.0).abs() < 1e-9);
        assert!((weighted_progress(&w, 1, 0.0) - 1000.0 / 1030.0).abs() < 1e-9);
        assert_eq!(weighted_progress(&w, 4, 0.0), 1.0);
        assert_eq!(weighted_progress(&[], 0, 0.5), 0.0);
        // Tiempo restante: con el 25 % en 10 min, faltan 30 min; sin avance, no se sabe.
        assert_eq!(eta(std::time::Duration::from_secs(600), 0.25), Some(1800));
        assert_eq!(eta(std::time::Duration::from_secs(600), 0.0), None);
        assert_eq!(eta(std::time::Duration::from_secs(5), 0.5), None, "demasiado pronto");
    }

    #[test]
    fn etapas_de_la_verificacion() {
        assert_eq!(check_stage("check snapshots, trees and blobs", "el 5 %").as_deref(), Some("Revisando copias, carpetas y bloques…"));
        assert_eq!(check_stage("read 5.0% of packfiles", "el 5 %").as_deref(), Some("Leyendo el 5 % de los datos…"));
        assert_eq!(
            check_stage("read group #1 of 63 data packs (out of total 139 packs in 2 groups", "la parte 1 de 2").as_deref(),
            Some("Leyendo la parte 1 de 2 de los datos…")
        );
        assert!(check_stage("no errors were found", "el 5 %").is_none());
    }

    fn verify(rotate_parts: u32, subset_percent: u8) -> Verify {
        Verify { schedule: Schedule::Weekly { weekday: 6, time: "03:00".into() }, subset_percent, enabled_at: String::new(), rotate_parts }
    }

    #[test]
    fn verificacion_de_la_copia_externa() {
        let hace_rato = t("2026-10-01 06:00").to_rfc3339();
        let mut repo = crate::agent::tests::repo_cada_hora();
        // Id propio: ninguna solicitud «Ahora» de verdad puede coincidir.
        repo.id = format!("prueba-nube-{}", std::process::id());
        repo.offsite = Some(Offsite {
            location: "s3:https://s3.example.com/b/siigo".into(),
            provider: "destino:nube".into(),
            region: None,
            schedule: Schedule::Hours { every: 1 },
            retention: None,
            limit_upload_kib: None,
            target_name: Some("Siigo · Backblaze".into()),
            enabled_at: hace_rato.clone(),
            guard: None,
            verify: Some(Verify { schedule: Schedule::Hours { every: 1 }, subset_percent: 0, enabled_at: hace_rato, rotate_parts: 3 }),
            dest: Default::default(),
            filtro: None,
        });
        let now = t("2026-10-01 10:30");
        let mut config = AgentConfig { repos: vec![repo], ..Default::default() };
        let mut state = TasksState::default();
        let ctx = DueContext::default();
        // Primero la subida y después su verificación: de una en una, nunca a la vez.
        let kinds = |c: &AgentConfig, s: &mut TasksState, x: &DueContext| due_at(c, s, x, false, now).iter().map(|(k, _)| *k).collect::<Vec<_>>();
        assert_eq!(kinds(&config, &mut state, &ctx), ["offsite", "verify_offsite"]);
        // Con la subida frenada por un cambio inusual se sigue verificando (solo lee).
        let held = DueContext { holds: [config.repos[0].id.clone()].into(), ..Default::default() };
        assert_eq!(kinds(&config, &mut state, &held), ["verify_offsite"]);
        // En pausa, nada.
        config.repos[0].pause = Some(agent::Pause { since: now.to_rfc3339(), until: None });
        assert!(kinds(&config, &mut state, &ctx).is_empty());

        // Su verificación, su clave y su rotativa: independientes de la del destino.
        let repo = &config.repos[0];
        let v = task_verify(repo, "verify_offsite").unwrap();
        assert_eq!(verify_args(v, 2), ["check", "--retry-lock", "30m", "--read-data-subset", "2/3"]);
        assert!(task_verify(repo, "verify").is_none());
        assert_eq!(rotation_key("verify_offsite", "r"), "offsite:r");
        assert_eq!(rotation_key("verify", "r"), "r");
        assert_eq!(key("verify_offsite", "r"), "verify_offsite:r");
        let mut st = TasksState::default();
        st.rotation.insert(rotation_key("verify", "r"), Rotation { parts: 3, next_part: 3, last_full_at: None });
        assert_eq!(current_part(st.rotation.get(&rotation_key("verify_offsite", "r")), 3), 1, "la de la nube va por su cuenta");

        // Configuración anterior de la copia externa: sin verificación.
        let o: Offsite = serde_json::from_str(r#"{"location":"s3:x","provider":"b2","schedule":{"kind":"daily","time":"03:00"},"enabled_at":"x"}"#).unwrap();
        assert!(o.verify.is_none());
    }

    #[test]
    fn prueba_de_restauracion_cuando_toca() {
        let hace_rato = t("2026-10-01 06:00").to_rfc3339();
        let mut repo = crate::agent::tests::repo_cada_hora();
        repo.id = format!("prueba-rest-{}", std::process::id());
        repo.restore_test = Some(crate::restore_test::RestoreTest { schedule: Schedule::Hours { every: 1 }, files: 20, max_mb: 200, enabled_at: hace_rato });
        let now = t("2026-10-01 10:30");
        let mut config = AgentConfig { repos: vec![repo], ..Default::default() };
        let mut state = TasksState::default();
        let ctx = DueContext::default();
        let kinds = |c: &AgentConfig, s: &mut TasksState| due_at(c, s, &ctx, false, now).iter().map(|(k, _)| *k).collect::<Vec<_>>();
        assert_eq!(kinds(&config, &mut state), ["restore_test"]);
        // En pausa, no.
        config.repos[0].pause = Some(agent::Pause { since: now.to_rfc3339(), until: None });
        assert!(kinds(&config, &mut state).is_empty());
        config.repos[0].pause = None;
        // Si recibe la copia externa de otro destino del agente, se prueba desde el origen.
        let mut origen = crate::agent::tests::repo_cada_hora();
        origen.id = "origen".into();
        origen.offsite = Some(Offsite {
            location: "s3:x".into(),
            provider: format!("destino:{}", config.repos[0].id),
            region: None,
            schedule: Schedule::Daily { time: "03:00".into() },
            retention: None,
            limit_upload_kib: None,
            target_name: None,
            enabled_at: t("2026-10-01 10:00").to_rfc3339(),
            guard: None,
            verify: None,
            dest: Default::default(),
            filtro: None,
        });
        config.repos.push(origen);
        assert!(!kinds(&config, &mut state).contains(&"restore_test"));
    }

    #[test]
    fn verificacion_rotativa() {
        // Argumentos: la parte toca; la rotativa manda sobre el porcentaje.
        assert_eq!(verify_args(&verify(4, 5), 2), ["check", "--retry-lock", "30m", "--read-data-subset", "2/4"]);
        assert_eq!(verify_args(&verify(0, 5), 1), ["check", "--retry-lock", "30m", "--read-data-subset", "5%"]);
        assert_eq!(verify_args(&verify(0, 0), 1), ["check", "--retry-lock", "30m"]);
        assert_eq!(verify(4, 0).subset_arg(9).as_deref(), Some("4/4"), "nunca fuera de rango");

        // Vuelta completa: 1, 2, 3, 4 y otra vez 1, anotando la lectura completa.
        let mut rot: Option<Rotation> = None;
        let mut seen = Vec::new();
        for i in 0..5 {
            let part = current_part(rot.as_ref(), 4);
            seen.push(part);
            rot = Some(advance(rot.as_ref(), 4, part, &format!("dia-{i}")));
        }
        assert_eq!(seen, [1, 2, 3, 4, 1]);
        assert_eq!(rot.as_ref().unwrap().last_full_at.as_deref(), Some("dia-3"), "el ciclo se completó en la 4.ª");
        assert_eq!(current_part(rot.as_ref(), 4), 2);

        // Si falla, no avanza: la próxima repite la misma parte.
        let fallo = RunRecord { result: "error".into(), finished: "z".into(), ..Default::default() };
        assert!(rotation_after(rot.as_ref(), 4, 2, &fallo).is_none());
        let bien = RunRecord { result: "ok".into(), finished: "z".into(), ..Default::default() };
        assert_eq!(rotation_after(rot.as_ref(), 4, 2, &bien).map(|r| r.next_part), Some(3));
        assert!(rotation_after(None, 0, 1, &bien).is_none(), "sin rotativa no se guarda nada");

        // Si cambia el número de partes, se empieza de 1 (sin perder la fecha del último ciclo).
        assert_eq!(current_part(rot.as_ref(), 6), 1);
        let r = advance(rot.as_ref(), 6, 1, "x");
        assert_eq!((r.next_part, r.last_full_at.as_deref()), (2, Some("dia-3")));

        // Validación: 0 o de 2 a 52.
        assert!(verify(0, 5).validate().is_ok() && verify(2, 0).validate().is_ok() && verify(52, 0).validate().is_ok());
        assert!(verify(1, 0).validate().is_err() && verify(53, 0).validate().is_err());
        // Configuración anterior: sin `rotate_parts`.
        let old: Verify = serde_json::from_str(r#"{"schedule":{"kind":"weekly","weekday":6,"time":"03:00"},"subset_percent":5,"enabled_at":"x"}"#).unwrap();
        assert_eq!(old.rotate_parts, 0);
        let t: TasksState = serde_json::from_str(r#"{"runs":{}}"#).unwrap();
        assert!(t.rotation.is_empty());
    }

    /// Verificación real de la parte 1 de 2 (con el repo de prueba).
    #[test]
    fn verificacion_rotativa_real() {
        let _real = crate::restic::tests::real_repo_lock();
        let (Ok(location), Ok(pw)) = (std::env::var("RESGUARDO_TEST_REPO"), std::env::var("RESGUARDO_TEST_PASSWORD")) else {
            return;
        };
        let repo = AgentRepo { location, ..crate::agent::tests::repo_cada_hora() };
        let secret = Secret { password: pw, ..Default::default() };
        let mut stages = Vec::new();
        let record = verify_repo(source_access(&repo, &secret), &verify(2, 0), 1, &mut |p: &TaskProgress| stages.push(p.stage.clone()));
        assert_eq!(record.result, "ok", "{}", record.message);
        assert!(record.message.starts_with("Parte 1 de 2 verificada sin errores"), "{}", record.message);
        assert!(stages.iter().any(|s| s.contains("la parte 1 de 2")), "{stages:?}");
    }

    #[test]
    fn destino_externo_y_bloqueo() {
        // Sin los campos nuevos se lee y se escribe como siempre.
        let o: Offsite =
            serde_json::from_str(r#"{"location":"b2:cubo:siigo","provider":"destino:nube","schedule":{"kind":"daily","time":"21:00"},"enabled_at":"x"}"#)
                .unwrap();
        assert!(o.dest.normal() && o.dest.lock_days().is_none());
        assert!(!serde_json::to_string(&o).unwrap().contains(r#""dest":"#), "un agente anterior la lee igual");
        let mut o2 = o.clone();
        o2.dest = DestinoExterno { existing: true, object_lock_days: Some(30), append_only: false, nube: None };
        let t = serde_json::to_string(&o2).unwrap();
        assert!(t.contains(r#""dest":{"existing":true,"object_lock_days":30}"#), "{t}");
        assert_eq!(serde_json::from_str::<Offsite>(&t).unwrap(), o2);
        assert_eq!(DestinoExterno { object_lock_days: Some(0), ..Default::default() }.lock_days(), None);

        // Con bloqueo, la retención guarda siempre lo de esos días (y uno más).
        let p = Policy { keep_last: 1, ..Default::default() };
        assert_eq!(locked_policy(&p, 30).keep_within.as_deref(), Some("31d"));
        assert!(locked_policy(&p, 30).args().windows(2).any(|w| w == ["--keep-within", "31d"]), "{:?}", locked_policy(&p, 30).args());
        // Si ya guardaba más, se queda la suya; si menos, la del bloqueo.
        let mas = Policy { keep_within: Some("1y".into()), ..Default::default() };
        assert_eq!(locked_policy(&mas, 30).keep_within.as_deref(), Some("1y"));
        let menos = Policy { keep_within: Some("7d".into()), ..Default::default() };
        assert_eq!(locked_policy(&menos, 30).keep_within.as_deref(), Some("31d"));
        assert_eq!(approx_days("1y6m"), Some(547));
        assert_eq!(approx_days("48h"), Some(2));
        assert_eq!(approx_days("x"), None);

        // Lo que dice restic al no poder quitar versiones bloqueadas (y sigue con 0).
        let salida = "Applying Policy: keep 1 latest snapshots\nunable to remove snapshot/1a2b3c4d from the repository\n\
                      unable to remove snapshot/5e6f7a8b from the repository\n[0:00] 100.00%  2 / 2 files deleted";
        assert_eq!(forget_blocked(salida), 2);
        assert_eq!(forget_blocked("removed snapshot/1a2b3c4d"), 0);
    }

    #[test]
    fn versiones_ya_subidas_desde_otro_repositorio() {
        let snap = |id: &str, original: Option<&str>| {
            serde_json::from_value::<restic::Snapshot>(
                serde_json::json!({ "id": id, "short_id": id, "time": "2026-01-01T00:00:00Z", "hostname": "PC", "original": original }),
            )
            .unwrap()
        };
        // En el destino: la copia de la versión «a1» del repositorio antiguo.
        let present: std::collections::HashSet<String> = ["d1".to_string(), "a1".to_string()].into();
        // En el nuevo: la misma versión traída del antiguo (`copiar_historial`) y una nueva.
        assert!(already_in(&present, &snap("c1", Some("a1"))));
        assert!(!already_in(&present, &snap("c2", None)));
        assert!(already_in(&present, &snap("d1", None)));
    }

    /// Datos que no se comprimen (para medir lo que se sube de verdad).
    fn aleatorios(semilla: u64, n: usize) -> Vec<u8> {
        let mut x = semilla.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                (x >> 24) as u8
            })
            .collect()
    }

    /// Lo que ocupan los datos de un repositorio local (su carpeta `data`).
    fn tamano_datos(repo: &std::path::Path) -> u64 {
        fn suma(p: &std::path::Path) -> u64 {
            fs::read_dir(p)
                .map(|d| d.flatten().map(|e| if e.path().is_dir() { suma(&e.path()) } else { e.metadata().map(|m| m.len()).unwrap_or(0) }).sum())
                .unwrap_or(0)
        }
        suma(&repo.join("data"))
    }

    fn ok(acc: &Access, args: &[&str]) {
        let out = restic::run_raw(acc, args, Duration::from_secs(300)).unwrap();
        assert_eq!(out.code, Some(0), "restic {args:?}: {}", out.stderr);
    }

    /// `restic -r DESTINO <args>` leyendo de ORIGEN (como `copy_access`).
    fn ok_desde(src: &Access, dest: &Access, args: &[&str]) {
        let mut both = dest.clone();
        both.env.push(("RESTIC_FROM_PASSWORD".into(), src.password.clone()));
        let mut a: Vec<&str> = args.to_vec();
        a.extend(["--from-repo", src.location.as_str()]);
        ok(&both, &a);
    }

    fn base_prueba(nombre: &str) -> std::path::PathBuf {
        let b = std::env::temp_dir().join(format!("resguardo-externa-{nombre}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&b);
        fs::create_dir_all(b.join("datos")).unwrap();
        b
    }

    fn externa(dest: &std::path::Path, existing: bool, retention: Option<Policy>, lock: Option<u32>) -> Offsite {
        Offsite {
            location: dest.display().to_string(),
            provider: "destino:nube".into(),
            region: None,
            schedule: Schedule::Daily { time: "21:00".into() },
            retention,
            limit_upload_kib: None,
            target_name: Some("Nube".into()),
            enabled_at: Local::now().to_rfc3339(),
            guard: None,
            verify: None,
            dest: DestinoExterno { existing, object_lock_days: lock, append_only: false, nube: None },
            filtro: None,
        }
    }

    /// El caso de la subida a la nube de la app de escritorio: el repositorio
    /// antiguo ya se subió (`restic copy`) a la nube; se movió a un almacén
    /// (repositorio nuevo con su troceado y todo su historial traído) y la
    /// copia externa del nuevo va a ese mismo repositorio de la nube. Solo
    /// sube lo nuevo: ni las versiones traídas (ya están allí, por `original`)
    /// ni sus datos (mismos bloques).
    #[test]
    fn copia_externa_a_un_repositorio_que_ya_existe() {
        if restic::version().is_err() {
            return; // sin restic
        }
        const MIB: usize = 1 << 20;
        let b = base_prueba("existente");
        let datos = b.join("datos");
        fs::write(datos.join("contabilidad.bin"), aleatorios(1, 3 * MIB)).unwrap();
        let d = datos.display().to_string();
        // El repositorio antiguo, con una versión.
        let antiguo = Access::new(b.join("antiguo").display().to_string(), "clave del antiguo");
        ok(&antiguo, &["init"]);
        ok(&antiguo, &["backup", "--host", "PC-CONTABLE", "--time", "2026-01-10 10:00:00", &d]);
        // Su subida a la nube: su propio troceado, otra contraseña.
        let nube_dir = b.join("nube");
        let nube = Access::new(nube_dir.display().to_string(), "clave de la nube");
        ok_desde(&antiguo, &nube, &["init", "--copy-chunker-params"]);
        ok_desde(&antiguo, &nube, &["copy"]);
        let primera = tamano_datos(&nube_dir);
        assert!(primera >= 3 * MIB as u64, "la primera subida lo lleva todo: {primera}");
        // Otra vez `restic copy` sin nada nuevo: no sube nada ni repite la versión.
        ok_desde(&antiguo, &nube, &["copy"]);
        assert_eq!(tamano_datos(&nube_dir), primera);
        assert_eq!(restic::snapshots(&nube).unwrap().len(), 1);
        // El nuevo (en el almacén), con el troceado del antiguo y su historial.
        let nuevo = Access::new(b.join("nuevo").display().to_string(), "clave del nuevo");
        ok_desde(&antiguo, &nuevo, &["init", "--copy-chunker-params"]);
        ok_desde(&antiguo, &nuevo, &["copy"]);
        // Una copia nueva en el almacén: 1 MiB nuevo.
        fs::write(datos.join("facturas.bin"), aleatorios(2, MIB)).unwrap();
        ok(&nuevo, &["backup", "--host", "PC-CONTABLE", &d]);

        let repo = AgentRepo { location: nuevo.location.clone(), ..crate::agent::tests::repo_cada_hora() };
        let secret = Secret {
            password: nuevo.password.clone(),
            offsite_password: Some(nube.password.clone()),
            offsite_location: Some(nube.location.clone()),
            ..Default::default()
        };
        let o = externa(&nube_dir, true, None, None);
        let mut etapas = Vec::new();
        let rec = offsite_repo(&repo, &secret, &o, false, &mut |p: &TaskProgress| etapas.push(p.clone()));
        assert_eq!(rec.result, "ok", "{}", rec.message);
        assert_eq!(rec.message, "1 copia subida (1 ya estaba en el destino).");
        assert_eq!(restic::snapshots(&nube).unwrap().len(), 2, "la traída no se repite");
        let anadido = tamano_datos(&nube_dir) - primera;
        assert!(anadido >= MIB as u64 && anadido < (MIB + MIB / 2) as u64, "solo lo nuevo (≈1 MiB): {anadido}");
        // El progreso contaba solo la que faltaba, con su peso estimado.
        assert!(etapas.iter().any(|p| p.total == Some(1) && p.bytes_total.is_some_and(|t| t >= MIB as u64)), "{etapas:?}");
        // restic mismo tampoco la repite (`copy` de todo).
        ok_desde(&nuevo, &nube, &["copy"]);
        assert_eq!(restic::snapshots(&nube).unwrap().len(), 2);
        // Al día: nada nuevo.
        let rec = offsite_repo(&repo, &secret, &o, false, &mut |_| {});
        assert_eq!(rec.message, "Nada nuevo que subir: el repositorio ya estaba al día.");
        // Si el que ya existía desaparece (otra dirección…), falla y no crea otro vacío.
        fs::rename(&nube_dir, b.join("nube-movida")).unwrap();
        let rec = offsite_repo(&repo, &secret, &o, false, &mut |_| {});
        assert_eq!((rec.result.as_str(), rec.message.as_str()), ("error", MISSING_EXISTING));
        assert!(!nube_dir.exists());
        // Uno nuevo (no «existente») sí se crea la primera vez, con el troceado del origen.
        let rec = offsite_repo(&repo, &secret, &externa(&nube_dir, false, None, None), false, &mut |_| {});
        assert_eq!(rec.result, "ok", "{}", rec.message);
        assert_eq!(chunker_polynomial(&nube).unwrap(), chunker_polynomial(&nuevo).unwrap());
        let _ = fs::remove_dir_all(&b);
    }

    /// Tarea 4b: cada copia derivada toca con su horario, con su clave en
    /// `tasks.json` (no pisa la de la copia externa) y con sus credenciales.
    #[test]
    fn copias_derivadas_cuando_tocan() {
        let mut repo = crate::agent::tests::repo_cada_hora();
        let o = |schedule: Schedule| Offsite {
            schedule,
            enabled_at: t("2026-10-01 06:00").to_rfc3339(),
            ..externa(std::path::Path::new("E:\\x"), false, None, None)
        };
        repo.offsite = Some(o(Schedule::Daily { time: "21:00".into() }));
        let mut nube = o(Schedule::AfterBackup { min_minutes: 0 });
        nube.verify =
            Some(Verify { schedule: Schedule::Hours { every: 24 }, subset_percent: 0, enabled_at: t("2026-10-01 06:00").to_rfc3339(), rotate_parts: 0 });
        repo.derived =
            vec![Derived { id: "nube".into(), offsite: nube }, Derived { id: "disco-e".into(), offsite: o(Schedule::Daily { time: "23:00".into() }) }];
        let config = AgentConfig { repos: vec![repo.clone()], ..Default::default() };
        let mut state = TasksState::default();
        let now = t("2026-10-01 22:00");
        // Una copia con versión nueva a las 21:30: la de «después de cada copia» toca; la de las 23:00, aún no;
        // la externa de las 21:00, sí (nunca se hizo). Y la verificación de la derivada (cada 24 h, desde las 06:00), no.
        let ctx = DueContext { events: [(repo.id.clone(), vec![ev("2026-10-01 21:30", "a1", 10, 1)])].into(), ..Default::default() };
        let todo = due_at(&config, &mut state, &ctx, false, now);
        let claves: Vec<String> = todo.iter().map(|(k, r)| task_key(k, r)).collect();
        assert_eq!(claves, ["offsite:r", "derivada:r:nube"]);
        // La tarea de la derivada ve su destino como la copia externa (y sabe de cuál es).
        let (_, vista) = &todo[1];
        assert!(matches!(vista.offsite.as_ref().unwrap().schedule, Schedule::AfterBackup { .. }));
        assert_eq!(task_rotation_key(VERIFY_DERIVADA, vista), "derivada:r:nube");
        // Ya hecha después de esa copia: no repite. La de las 23:00, a su hora.
        state.runs.insert(derived_key(DERIVADA, "r", "nube"), RunRecord { started: t("2026-10-01 21:35").to_rfc3339(), ..Default::default() });
        state.runs.insert(key("offsite", "r"), RunRecord { started: t("2026-10-01 21:00").to_rfc3339(), ..Default::default() });
        let todo = due_at(&config, &mut state, &ctx, false, t("2026-10-01 23:01"));
        assert_eq!(todo.iter().map(|(k, r)| task_key(k, r)).collect::<Vec<_>>(), ["derivada:r:disco-e"]);
        let todo = due_at(&config, &mut state, &ctx, false, t("2026-10-02 06:30"));
        assert!(todo.iter().any(|(k, r)| task_key(k, r) == "verify_derivada:r:nube"));
        // Con la subida frenada por un cambio inusual, tampoco las derivadas.
        let frenado = DueContext { holds: ["r".to_string()].into(), ..DueContext { events: ctx.events.clone(), ..Default::default() } };
        let mut limpio = TasksState::default();
        assert!(!due_at(&config, &mut limpio, &frenado, false, now).iter().any(|(k, _)| *k == DERIVADA));
        // Sus credenciales en lugar de las de la copia externa.
        let mut secret = Secret { password: "origen".into(), offsite_password: Some("de la externa".into()), ..Default::default() };
        secret
            .derived
            .insert("nube".into(), agent::DerivedSecret { password: Some("de la nube".into()), location: Some("rest:https://x/r".into()), env: vec![] });
        let s = derived_secret(&secret, "nube");
        assert_eq!(
            (s.password.as_str(), s.offsite_password.as_deref(), s.offsite_location.as_deref()),
            ("origen", Some("de la nube"), Some("rest:https://x/r"))
        );
        let s = derived_secret(&secret, "otra");
        assert_eq!(s.offsite_password, None, "sin la suya, la del origen (nunca la de la externa)");
    }

    /// Tarea 4b y 4c: una copia derivada con otra contraseña y un filtro (solo
    /// las de los últimos 30 días con la etiqueta «diaria»), con restic de verdad.
    #[test]
    fn copia_derivada_con_filtro_y_otra_contrasena() {
        if restic::version().is_err() {
            return;
        }
        let b = base_prueba("derivada");
        let datos = b.join("datos");
        fs::write(datos.join("factura.txt"), b"factura 1").unwrap();
        let d = datos.display().to_string();
        let origen = Access::new(b.join("origen").display().to_string(), "clave del origen");
        ok(&origen, &["init"]);
        let hace = |dias: i64| (Local::now() - chrono::Duration::days(dias)).format("%Y-%m-%d %H:%M:%S").to_string();
        ok(&origen, &["backup", "--host", "PC-ANA", "--tag", "diaria", "--time", &hace(200), &d]);
        ok(&origen, &["backup", "--host", "PC-ANA", "--tag", "semanal", "--time", &hace(10), &d]);
        ok(&origen, &["backup", "--host", "PC-ANA", "--tag", "diaria", "--time", &hace(2), &d]);
        let repo = AgentRepo { location: origen.location.clone(), ..crate::agent::tests::repo_cada_hora() };
        let destino = b.join("derivada");
        let mut o = externa(&destino, false, None, None);
        o.filtro = Some(crate::adoptar_v2::Filtro::de(&serde_json::json!({ "etiquetas": ["diaria"], "ultimos_dias": 30 })).unwrap());
        let d1 = Derived { id: "d1".into(), offsite: o };
        let mut secret = Secret { password: origen.password.clone(), ..Default::default() };
        secret
            .derived
            .insert("d1".into(), agent::DerivedSecret { password: Some("otra clave".into()), location: Some(destino.display().to_string()), env: vec![] });
        let vista = derived_view(&repo, &d1);
        let rec = offsite_repo(&vista, &derived_secret(&secret, "d1"), vista.offsite.as_ref().unwrap(), false, &mut |_| {});
        assert_eq!(rec.result, "ok", "{}", rec.message);
        assert_eq!(rec.message, "1 copia subida.");
        // Se abre con su contraseña (no con la del origen) y solo tiene la que pasa el filtro.
        let dest = Access::new(destino.display().to_string(), "otra clave");
        let v = restic::snapshots(&dest).unwrap();
        assert_eq!((v.len(), v[0].tags.as_slice()), (1, &["diaria".to_string()][..]));
        assert!(restic::snapshots(&Access::new(destino.display().to_string(), "clave del origen")).is_err());
        // Otra vuelta: nada nuevo (las que no pasan el filtro ni se cuentan).
        let rec = offsite_repo(&vista, &derived_secret(&secret, "d1"), vista.offsite.as_ref().unwrap(), false, &mut |_| {});
        assert_eq!(rec.message, "Nada nuevo que subir: el repositorio ya estaba al día.");
        let _ = fs::remove_dir_all(&b);
    }

    /// Con bloqueo de objetos: la retención del destino quita versiones
    /// (`forget`) pero no hace `prune` (no libera espacio) y nunca quita las de
    /// los días bloqueados. Sin bloqueo, `forget --prune` sí libera.
    #[test]
    fn retencion_con_bloqueo_de_objetos() {
        if restic::version().is_err() {
            return;
        }
        const MIB: usize = 1 << 20;
        let b = base_prueba("bloqueo");
        let datos = b.join("datos");
        let d = datos.display().to_string();
        let origen = Access::new(b.join("origen").display().to_string(), "clave del origen");
        ok(&origen, &["init"]);
        let hace = |dias: i64| (Local::now() - chrono::Duration::days(dias)).format("%Y-%m-%d %H:%M:%S").to_string();
        for (i, cuando) in [hace(400), hace(5), hace(0)].iter().enumerate() {
            fs::write(datos.join("datos.bin"), aleatorios(10 + i as u64, MIB)).unwrap();
            ok(&origen, &["backup", "--host", "PC", "--time", cuando, &d]);
        }
        let repo = AgentRepo { location: origen.location.clone(), ..crate::agent::tests::repo_cada_hora() };
        let ultima = Policy { keep_last: 1, ..Default::default() };
        for (lock, quedan) in [(Some(30), 2usize), (None, 1)] {
            let nube_dir = b.join(format!("nube-{}", lock.unwrap_or(0)));
            let nube = Access::new(nube_dir.display().to_string(), "clave del origen");
            let secret = Secret { password: origen.password.clone(), offsite_location: Some(nube.location.clone()), ..Default::default() };
            // Primero se sube todo (sin retención propia) …
            let rec = offsite_repo(&repo, &secret, &externa(&nube_dir, false, None, lock), false, &mut |_| {});
            assert_eq!((rec.result.as_str(), rec.message.as_str()), ("ok", "3 copias subidas."));
            let todo = tamano_datos(&nube_dir);
            // … y después, con «la última»: lo de los 30 días bloqueados se queda.
            let rec = offsite_repo(&repo, &secret, &externa(&nube_dir, false, Some(ultima.clone()), lock), false, &mut |_| {});
            assert_eq!(rec.result, "ok", "{}", rec.message);
            assert_eq!(restic::snapshots(&nube).unwrap().len(), quedan, "{lock:?}");
            if lock.is_some() {
                assert!(rec.message.contains("sin liberar espacio") && rec.message.contains("bloqueo de 30 días"), "{}", rec.message);
                assert_eq!(tamano_datos(&nube_dir), todo, "sin prune: no se borra ningún dato");
            } else {
                assert!(rec.message.contains("Retención aplicada en el repositorio."), "{}", rec.message);
                assert!(tamano_datos(&nube_dir) < todo - MIB as u64, "con prune se libera");
            }
        }
        let _ = fs::remove_dir_all(&b);
    }
}
