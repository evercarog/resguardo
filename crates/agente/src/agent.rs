//! Modo agente: copias programadas aunque la ventana esté cerrada.
//!
//! El Programador de tareas ejecuta `resguardo.exe --agent-run` cada 5
//! minutos como SYSTEM. En cada ejecución el agente mira qué repositorios
//! tienen una copia pendiente según su horario, la hace y lo anota.
//!
//! Todo vive en `%ProgramData%\Resguardo` (lo modifican SYSTEM y
//! Administradores; los usuarios pueden leer la programación y el estado):
//! - `agent.json`: repositorios programados (sin secretos).
//! - `secrets.bin`: contraseñas, cifradas con DPAPI de este equipo.
//! - `state.json`: resultado de cada ejecución (lo que verá la web).
//! - `agent.log`: registro legible.

use crate::platform;
use crate::restic::{self, Access};
use crate::store::Repo;
use chrono::{DateTime, Datelike, Duration, Local, NaiveTime, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Carpeta de pruebas (`RESGUARDO_AGENT_DIR`). Solo existe en los tests y en
/// las compilaciones de desarrollo: en la app publicada, una variable de
/// entorno nunca puede saltarse los permisos ni el requisito de administrador.
fn test_dir() -> Option<PathBuf> {
    if cfg!(any(test, debug_assertions)) {
        std::env::var("RESGUARDO_AGENT_DIR").ok().map(PathBuf::from)
    } else {
        None
    }
}

/// Carpeta del agente: `C:\ProgramData\Resguardo` para la app de escritorio
/// y su tarea programada; `C:\ProgramData\ResguardoAgente` para
/// `resguardo-agente.exe` (el agente gestionado). Son carpetas distintas a
/// propósito: los dos conviven en el mismo equipo sin compartir
/// configuración, secretos, bloqueos ni historial.
pub fn agent_dir() -> PathBuf {
    if let Some(dir) = test_dir() {
        return dir;
    }
    #[cfg(unix)]
    {
        PathBuf::from("/var/lib/resguardo-agente")
    }
    #[cfg(not(unix))]
    {
        PathBuf::from(platform::program_data()).join(if is_managed_agent() { CARPETA_AGENTE_GESTIONADO } else { "Resguardo" })
    }
}

/// Carpeta (en ProgramData) de `resguardo-agente.exe` en Windows.
pub const CARPETA_AGENTE_GESTIONADO: &str = "ResguardoAgente";

/// Versiones anteriores de `resguardo-agente.exe` (fase 5) usaban
/// `ProgramData\Resguardo`, la de la app. Al arrancar el servicio: si la
/// carpeta nueva aún no existe, la antigua tiene un emparejamiento y la app de
/// escritorio **no** está instalada (así que la carpeta era solo del agente),
/// se copia a la nueva (la antigua se conserva). Con la app instalada no se
/// toca nada: el agente empieza limpio y hay que volver a emparejarlo.
#[cfg(windows)]
pub fn migrar_carpeta_fase5() {
    if test_dir().is_some() || !is_managed_agent() {
        return;
    }
    let nueva = agent_dir();
    let antigua = PathBuf::from(platform::program_data()).join("Resguardo");
    let app = std::path::Path::new(&std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into())).join("Resguardo").join("resguardo.exe");
    let emparejado = ["gestionado.bin", "servidor.bin"].iter().any(|f| antigua.join("privado").join(f).is_file());
    if nueva.exists() || !emparejado || app.exists() {
        return;
    }
    fn copiar(de: &std::path::Path, a: &std::path::Path) -> std::io::Result<()> {
        fs::create_dir_all(a)?;
        for e in fs::read_dir(de)?.flatten() {
            let t = e.file_type()?;
            if t.is_symlink() {
                continue;
            }
            if t.is_dir() {
                copiar(&e.path(), &a.join(e.file_name()))?;
            } else {
                fs::copy(e.path(), a.join(e.file_name()))?;
            }
        }
        Ok(())
    }
    match copiar(&antigua, &nueva) {
        Ok(()) => {
            let _ = prepare_dir();
            log(&format!("Datos del agente copiados de {} a {} (la carpeta antigua se conserva).", antigua.display(), nueva.display()));
        }
        Err(e) => {
            let _ = fs::remove_dir_all(&nueva);
            log(&format!("No se pudieron copiar los datos del agente de {}: {e}", antigua.display()));
        }
    }
}

#[cfg(not(windows))]
pub fn migrar_carpeta_fase5() {}

/// Subcarpeta sin acceso para los usuarios: secretos y archivos temporales
/// del sistema (el XML de la tarea).
pub fn private_dir() -> PathBuf {
    agent_dir().join("privado")
}

fn secrets_file() -> PathBuf {
    private_dir().join("secrets.bin")
}

/// Ubicación de los secretos en versiones anteriores (0.3.3 y antes).
fn legacy_secrets_file() -> PathBuf {
    agent_dir().join("secrets.bin")
}

/// Versión de los permisos de la carpeta: si la del estado es menor, el
/// agente (como SYSTEM) los vuelve a aplicar. Así se corrigen instalaciones
/// anteriores sin que nadie tenga que abrir la app como administrador.
const ACL_VERSION: u32 = 2;

// ---------- Horarios ----------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Schedule {
    /// Cada N horas desde la última copia.
    Hours { every: u32 },
    /// Todos los días a una hora ("HH:MM", hora local).
    Daily { time: String },
    /// Un día de la semana (0 = lunes … 6 = domingo) a una hora.
    Weekly { weekday: u32, time: String },
    /// Solo vigilar: las copias las hace otro programa (p. ej. un script).
    /// El agente no copia nada; revisa los snapshots y avisa si pasan más de
    /// `every` horas sin copias.
    Monitor { every: u32 },
    /// Las copias las definen los planes del repositorio (cada uno con su horario).
    Plans,
    /// Solo para la copia externa: después de cada copia de este destino que
    /// guardó una versión nueva, dejando al menos `min_minutes` entre subidas.
    AfterBackup {
        #[serde(default = "default_min_minutes")]
        min_minutes: u32,
    },
    /// Cuando toque cualquiera de estas reglas (las de los horarios de copia:
    /// a estas horas, cada N minutos u horas en una franja, cada N días, un
    /// día de cada mes). La verificación automática con horario (v1.40).
    Rules { rules: Vec<crate::plans::ScheduleRule> },
}

fn default_min_minutes() -> u32 {
    30
}

fn parse_time(t: &str) -> Result<NaiveTime, String> {
    NaiveTime::parse_from_str(t, "%H:%M").map_err(|_| format!("Hora no válida: {t} (usa HH:MM)"))
}

impl Schedule {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Schedule::Hours { every } | Schedule::Monitor { every } if !(1..=24 * 31).contains(every) => Err("Elige entre 1 hora y 31 días.".into()),
            Schedule::Daily { time } => parse_time(time).map(|_| ()),
            Schedule::Weekly { weekday, time } if *weekday <= 6 => parse_time(time).map(|_| ()),
            Schedule::Weekly { .. } => Err("Día de la semana no válido.".into()),
            Schedule::AfterBackup { min_minutes } if *min_minutes > 1440 => Err("Entre subidas, como mucho 1440 minutos (un día).".into()),
            Schedule::Rules { rules } => crate::plans::PlanSchedule::from_rules(rules.clone()).validate(),
            _ => Ok(()),
        }
    }

    /// Para verificaciones (y cualquier tarea que no sea la copia externa):
    /// «después de cada copia» no tiene sentido.
    pub fn validate_task(&self) -> Result<(), String> {
        if matches!(self, Schedule::AfterBackup { .. } | Schedule::Monitor { .. } | Schedule::Plans) {
            return Err("Ese horario no vale para esta tarea.".into());
        }
        self.validate()
    }

    /// Para la copia externa: cualquier horario menos «solo vigilar» y «según los planes».
    pub fn validate_offsite(&self) -> Result<(), String> {
        if matches!(self, Schedule::Monitor { .. } | Schedule::Plans) {
            return Err("Ese horario no vale para la copia externa.".into());
        }
        self.validate()
    }

    /// Último momento programado que ya pasó (para horarios diarios y semanales).
    fn latest_slot(&self, now: DateTime<Local>) -> Option<DateTime<Local>> {
        let at = |date: chrono::NaiveDate, time: &str| {
            let t = parse_time(time).ok()?;
            Local.from_local_datetime(&date.and_time(t)).earliest()
        };
        match self {
            Schedule::Hours { .. } | Schedule::Monitor { .. } | Schedule::Plans | Schedule::AfterBackup { .. } => None,
            Schedule::Rules { rules } => crate::plans::PlanSchedule::from_rules(rules.clone()).latest_slot(now),
            Schedule::Daily { time } => {
                let today = at(now.date_naive(), time)?;
                Some(if today <= now { today } else { at(now.date_naive() - Duration::days(1), time)? })
            }
            Schedule::Weekly { weekday, time } => {
                let back = (now.weekday().num_days_from_monday() + 7 - weekday) % 7;
                let slot = at(now.date_naive() - Duration::days(back as i64), time)?;
                Some(if slot <= now { slot } else { slot - Duration::days(7) })
            }
        }
    }

    /// ¿Toca copia? `since` es la última copia del agente o, si nunca hizo
    /// ninguna, el momento en que se activó la programación. Si el equipo
    /// estuvo apagado a la hora prevista, la copia se hace al encenderlo.
    /// Un `since` en el futuro (el reloj estuvo adelantado y se corrigió)
    /// no hace esperar a esa fecha: cuenta como si no hubiera habido copia.
    pub fn is_due(&self, since: DateTime<Local>, now: DateTime<Local>) -> bool {
        let reloj_atrasado = crate::plans::clock_went_back(since, now);
        match self {
            Schedule::Hours { every } => reloj_atrasado || now >= since + Duration::hours(*every as i64),
            // «Después de cada copia» se decide en el proceso de tareas (`tasks::after_backup_due`).
            Schedule::Monitor { .. } | Schedule::Plans | Schedule::AfterBackup { .. } => false,
            // Como un plan de copia: una vez aunque se perdieran varias, y no dos seguidas.
            Schedule::Rules { rules } => crate::plans::PlanSchedule::from_rules(rules.clone()).is_due(since, now),
            _ => self.latest_slot(now).is_some_and(|slot| since < slot || reloj_atrasado),
        }
    }
}

// ---------- Configuración, secretos y estado ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRepo {
    pub id: String,
    pub name: String,
    pub location: String,
    #[serde(default)]
    pub rest_username: Option<String>,
    #[serde(default)]
    pub cacert: Option<String>,
    pub paths: Vec<String>,
    #[serde(default)]
    pub excludes: Vec<String>,
    pub schedule: Schedule,
    /// RFC 3339: cuándo se activó (referencia para la primera copia).
    pub enabled_at: String,
    /// Planes de copia programados (con `schedule: Plans`).
    #[serde(default)]
    pub plans: Vec<AgentPlan>,
    /// Verificación programada (`restic check`).
    #[serde(default)]
    pub verify: Option<crate::tasks::Verify>,
    /// Copia externa programada (`restic copy` a otro repositorio).
    #[serde(default)]
    pub offsite: Option<crate::tasks::Offsite>,
    /// Copias automáticas en pausa (p. ej. durante un mantenimiento del servidor).
    #[serde(default)]
    pub pause: Option<Pause>,
    /// Kit de recuperación guardado (copia de lo que anotó la app como
    /// administrador; lo lee el informe a la web).
    #[serde(default)]
    pub kit: Option<crate::kit::KitStatus>,
    /// Prueba de restauración programada.
    #[serde(default)]
    pub restore_test: Option<crate::restore_test::RestoreTest>,
    /// Copia de datos del destino en la app (para la salud de la protección en
    /// la web): tiene política de retención y el bucket declara bloqueo de objetos.
    #[serde(default)]
    pub has_retention: bool,
    #[serde(default)]
    pub object_lock: bool,
    /// Destino (lugar) del repositorio en la app y su nombre (para la web).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place_name: Option<String>,
}

impl AgentRepo {
    /// Pausa vigente en `now` (una que ya terminó no cuenta, aunque el agente
    /// aún no la haya quitado de la configuración).
    pub fn active_pause(&self, now: DateTime<Local>) -> Option<&Pause> {
        self.pause.as_ref().filter(|p| p.active(now))
    }
}

/// Pausa de las copias automáticas de un destino. Mientras dura, el agente no
/// empieza copias programadas, reintentos, verificaciones ni copias externas
/// de ese destino; sigue informando a la web y conserva toda su configuración.
/// Lo que ya esté en marcha termina normalmente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pause {
    /// RFC 3339: cuándo empezó.
    pub since: String,
    /// RFC 3339: cuándo se reanudan solas (`None`: hasta reanudarlas a mano).
    #[serde(default)]
    pub until: Option<String>,
}

/// Duración máxima de una pausa con fecha de fin.
pub const MAX_PAUSE_DAYS: i64 = 30;

impl Pause {
    /// ¿Sigue en pausa en `now`? Sin fecha de fin, sí. Una fecha ilegible
    /// cuenta como ya pasada: ante la duda, se vuelve a copiar.
    pub fn active(&self, now: DateTime<Local>) -> bool {
        match &self.until {
            None => true,
            Some(u) => DateTime::parse_from_rfc3339(u).is_ok_and(|u| now < u.with_timezone(&Local)),
        }
    }

    /// «hasta el jueves 2 de octubre a las 06:00» o «hasta que las reanudes».
    pub fn until_words(&self) -> String {
        match self.until.as_deref().and_then(|u| DateTime::parse_from_rfc3339(u).ok()) {
            Some(u) => format!("hasta {}", date_in_words(u.with_timezone(&Local))),
            None => "hasta que las reanudes".into(),
        }
    }
}

/// «el jueves 2 de octubre a las 06:00» (hora local).
fn date_in_words(t: DateTime<Local>) -> String {
    const DAYS: [&str; 7] = ["lunes", "martes", "miércoles", "jueves", "viernes", "sábado", "domingo"];
    const MONTHS: [&str; 12] = ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"];
    format!("el {} {} de {} a las {}", DAYS[t.weekday().num_days_from_monday() as usize], t.day(), MONTHS[t.month0() as usize], t.format("%H:%M"))
}

/// Comprueba el final de una pausa (RFC 3339): en el futuro y como mucho a
/// 30 días. Se compara el instante, así que un cambio de hora no lo altera.
pub fn parse_pause_until(until: &str, now: DateTime<Local>) -> Result<DateTime<Local>, String> {
    let t = DateTime::parse_from_rfc3339(until.trim()).map_err(|_| "La fecha para reanudar las copias no es válida.".to_string())?.with_timezone(&Local);
    if t <= now {
        return Err("Elige un momento futuro para reanudar las copias.".into());
    }
    if t > now + Duration::days(MAX_PAUSE_DAYS) {
        return Err(format!("La pausa puede durar como mucho {MAX_PAUSE_DAYS} días. Para más tiempo, elige «Hasta que la reanude»."));
    }
    Ok(t)
}

/// Un plan programado, tal como lo ejecuta el agente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentPlan {
    pub id: String,
    pub name: String,
    pub paths: Vec<String>,
    #[serde(default)]
    pub excludes: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub schedule: crate::plans::PlanSchedule,
    /// RFC 3339: cuándo se activó (referencia para la primera copia).
    pub enabled_at: String,
    /// Solo guardar una versión si algo cambió (`--skip-if-unchanged`).
    #[serde(default)]
    pub skip_unchanged: bool,
    /// Ganchos de plantilla (antes y después de la copia; ganchos.rs).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ganchos: Vec<crate::ganchos::Gancho>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AgentConfig {
    #[serde(default)]
    pub repos: Vec<AgentRepo>,
    /// «Copias a distancia»: se aceptan peticiones de «Copiar ahora» de la web
    /// o de Resguardo en otro equipo (ver remote.rs). Desactivado por defecto.
    #[serde(default)]
    pub remote_backup: bool,
    /// «Modo discreto»: días y horas en que restic va con prioridad baja (ver
    /// discreto.rs). None: desactivado.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discreet: Option<crate::discreto::Discreet>,
}

/// Antes de cada copia o tarea: ¿toca ir con prioridad baja? Se lo dice a
/// restic y lo devuelve (para anotarlo en el registro).
pub fn apply_discreet(config: &AgentConfig) -> bool {
    let d = config.discreet.as_ref();
    let on = d.is_some_and(|d| d.active(Local::now()));
    restic::set_discreet(on, d.and_then(|d| d.upload_kib));
    on
}

/// Activa, cambia o quita el modo discreto. Solo como administrador
/// (agent.json); la app pide además la contraseña de un destino.
pub fn set_discreet(discreet: Option<crate::discreto::Discreet>) -> Result<(), String> {
    require_admin()?;
    if let Some(d) = &discreet {
        d.validate()?;
    }
    let mut config = load_config();
    if config.discreet == discreet {
        return Ok(());
    }
    config.discreet = discreet.clone();
    write_json("agent.json", &config)?;
    log(&match discreet {
        Some(d) => format!(
            "Modo discreto activado: prioridad baja de {} a {}{}.",
            d.from,
            d.to,
            d.upload_kib.map(|k| format!(", subida limitada a {k} KB/s")).unwrap_or_default()
        ),
        None => "Modo discreto desactivado.".into(),
    });
    Ok(())
}

#[derive(Default, Serialize, Deserialize)]
pub struct Secret {
    pub password: String,
    #[serde(default)]
    pub rest_password: Option<String>,
    /// Copia externa: contraseña propia del destino (si no, la del origen)
    /// y credenciales de la nube.
    #[serde(default)]
    pub offsite_password: Option<String>,
    #[serde(default)]
    pub offsite_key_id: Option<String>,
    #[serde(default)]
    pub offsite_secret: Option<String>,
    /// Credenciales de nube del propio repositorio (S3, B2, Azure) como
    /// variables de entorno para restic.
    #[serde(default)]
    pub env: Vec<(String, String)>,
    /// Copia externa hacia otro destino de la app: su ubicación con
    /// credenciales de servidor (solo aquí, nunca en agent.json) y sus
    /// variables de nube.
    #[serde(default)]
    pub offsite_location: Option<String>,
    #[serde(default)]
    pub offsite_env: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunRecord {
    pub started: String,
    pub finished: String,
    /// "ok", "warning" (terminó pero faltaron archivos) o "error".
    pub result: String,
    pub message: String,
    #[serde(default)]
    pub snapshot_id: Option<String>,
    #[serde(default)]
    pub data_added: Option<u64>,
    #[serde(default)]
    pub files_new: Option<u64>,
    #[serde(default)]
    pub files_changed: Option<u64>,
    /// Correcta y sin cambios: no se guardó una versión nueva (sin `snapshot_id`).
    /// Cuenta como una copia al día.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unchanged: bool,
    /// Resultado de los ganchos de la copia (ganchos.rs), si tiene.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ganchos: Vec<crate::ganchos::ResultadoGancho>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AgentState {
    /// Última ejecución de cada repositorio.
    #[serde(default)]
    pub runs: HashMap<String, RunRecord>,
    /// Última vez que el agente se despertó (sirve para saber si está vivo).
    #[serde(default)]
    pub last_tick: Option<String>,
    /// Copia que el agente está haciendo ahora mismo (si hay alguna).
    #[serde(default)]
    pub running: Option<RunningCopy>,
    /// Fecha de la última solicitud atendida de cada repositorio.
    #[serde(default)]
    pub requests_seen: HashMap<String, String>,
    /// Reintentos hechos tras una copia fallida de cada plan (se reinicia al
    /// salir bien o en la siguiente hora programada).
    #[serde(default)]
    pub retries: HashMap<String, u32>,
    /// Versión de los permisos aplicados a la carpeta (ver `ACL_VERSION`).
    #[serde(default)]
    pub acl_version: u32,
    /// Último intento fallido de aplicar los permisos (se reintenta cada 6 h).
    #[serde(default)]
    pub acl_failed_at: Option<String>,
    /// Informes a Resguardo Web.
    #[serde(default)]
    pub web: crate::web::WebState,
    /// Servidores REST: ¿de solo añadir? (se comprueba como mucho una vez al día).
    #[serde(default)]
    pub append_only: HashMap<String, crate::protection::AppendOnlyCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningCopy {
    pub repo_id: String,
    /// Plan que se está copiando.
    #[serde(default)]
    pub plan_id: Option<String>,
    /// RFC 3339.
    pub started: String,
    /// Progreso según restic (0 a 1); `None` mientras prepara la copia.
    #[serde(default)]
    pub percent: Option<f64>,
    #[serde(default)]
    pub files_done: u64,
    #[serde(default)]
    pub total_files: u64,
    #[serde(default)]
    pub bytes_done: u64,
    #[serde(default)]
    pub total_bytes: u64,
    #[serde(default)]
    pub seconds_remaining: Option<u64>,
    /// RFC 3339: última actualización del progreso.
    #[serde(default)]
    pub updated: Option<String>,
    /// `"hooks"` mientras corren los ganchos «Antes de copiar»; si no, nada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    /// Ritmo reciente de restic (bytes leídos por segundo, suavizado).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes_per_s: Option<u64>,
    /// Lectura real del disco de restic (bytes/s, suavizado), si el sistema la da.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_bps: Option<u64>,
    /// Lo que restic escribe o sube al destino (bytes/s, suavizado).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upload_bps: Option<u64>,
    /// Archivos por segundo (suavizado).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files_per_s: Option<u64>,
}

/// Ritmo suavizado: el nuevo tramo pesa un 40 % (sin saltos bruscos en la consola).
pub fn smoothed_rate(previous: Option<u64>, bytes: u64, seconds: f64) -> Option<u64> {
    if seconds < 0.5 {
        return previous;
    }
    let now = bytes as f64 / seconds;
    Some(match previous {
        Some(p) => (p as f64 * 0.6 + now * 0.4).round() as u64,
        None => now.round() as u64,
    })
}

/// Cada cuánto se guarda el progreso de una copia en `state.json`.
const PROGRESS_EVERY: std::time::Duration = std::time::Duration::from_secs(3);

pub fn read_json<T: for<'de> Deserialize<'de> + Default>(name: &str) -> T {
    fs::read(agent_dir().join(name)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Escribe en un archivo recién creado: nunca reutiliza uno que ya existiera
/// (con otros permisos, abierto por otro proceso o un enlace).
pub fn write_new(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    if fs::symlink_metadata(path).is_ok() {
        fs::remove_file(path)?;
    }
    fs::OpenOptions::new().write(true).create_new(true).open(path)?.write_all(bytes)
}

pub fn write_json<T: Serialize>(name: &str, value: &T) -> Result<(), String> {
    let dir = agent_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("No se pudo crear {}: {e}", dir.display()))?;
    let tmp = dir.join(format!("{name}.tmp"));
    write_new(&tmp, &serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?).map_err(|e| format!("No se pudo guardar {name}: {e}"))?;
    fs::rename(&tmp, dir.join(name)).map_err(|e| format!("No se pudo guardar {name}: {e}"))
}

pub fn load_config() -> AgentConfig {
    read_json("agent.json")
}

pub fn load_state() -> AgentState {
    read_json("state.json")
}

pub fn load_secrets() -> Result<HashMap<String, Secret>, String> {
    let file = if secrets_file().is_file() { secrets_file() } else { legacy_secrets_file() };
    match fs::read(file) {
        Ok(enc) => {
            let plain = platform::unprotect(&enc)?;
            serde_json::from_slice(&plain).map_err(|e| format!("Secretos del agente dañados: {e}"))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
        Err(e) => Err(format!("No se pudieron leer los secretos del agente: {e}")),
    }
}

/// La contraseña nueva de un repositorio también para el agente, si lo
/// tiene programado. Devuelve si lo tenía.
pub fn update_password(repo_id: &str, password: &str) -> Result<bool, String> {
    let mut secrets = load_secrets()?;
    let Some(s) = secrets.get_mut(repo_id) else { return Ok(false) };
    require_admin()?;
    s.password = password.to_string();
    save_secrets(&secrets)?;
    log(&format!("Contraseña guardada actualizada para el repositorio {repo_id}."));
    Ok(true)
}

pub fn save_secrets(secrets: &HashMap<String, Secret>) -> Result<(), String> {
    let plain = serde_json::to_vec(secrets).map_err(|e| e.to_string())?;
    let enc = platform::protect(&plain)?;
    // La carpeta privada ya existe con sus permisos (prepare_dir): el archivo
    // nace sin acceso para los usuarios. Se escribe aparte y se renombra.
    let dir = private_dir();
    if !dir.is_dir() {
        return Err("La carpeta del agente no está preparada.".into());
    }
    let tmp = dir.join("secrets.bin.tmp");
    // Siempre un archivo nuevo, que hereda los permisos de la carpeta privada.
    write_new(&tmp, &enc).map_err(|e| format!("No se pudieron guardar los secretos: {e}"))?;
    fs::rename(&tmp, secrets_file()).map_err(|e| format!("No se pudieron guardar los secretos: {e}"))?;
    let _ = fs::remove_file(legacy_secrets_file());
    Ok(())
}

/// Las carpetas del agente son carpetas de verdad (no enlaces). Un usuario
/// puede crear `C:\ProgramData\Resguardo` (o algo dentro) antes de instalar
/// Resguardo; si fuera una unión que pudiera redirigir después, SYSTEM leería
/// la configuración y escribiría los secretos donde ese usuario quisiera.
pub fn dirs_safe() -> bool {
    test_mode() || ![agent_dir(), private_dir(), requests_dir()].iter().any(|d| platform::is_reparse_point(d))
}

/// Quita un enlace (unión o simbólico) en lugar de seguirlo: se borra el
/// enlace, nunca lo que hay en su destino.
fn remove_link(path: &std::path::Path) -> Result<bool, String> {
    if !platform::is_reparse_point(path) {
        return Ok(false);
    }
    fs::remove_dir(path).or_else(|_| fs::remove_file(path)).map_err(|e| format!("{} es un enlace y no se pudo quitar: {e}", path.display()))?;
    Ok(true)
}

pub fn log(line: &str) {
    if platform::is_reparse_point(&agent_dir()) {
        return;
    }
    let line = line.split(['\r', '\n']).map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" · ");
    // Con rutas completas, solo para SYSTEM y Administradores.
    log_detail(&line);
    // El registro lo leen todos los usuarios del equipo: sin rutas ni contraseñas.
    let line = crate::web::strip_paths(&line);
    let line = line.as_str();
    let path = agent_dir().join("agent.log");
    // Registro acotado: si pasa de 1 MB se rota.
    if fs::metadata(&path).is_ok_and(|m| m.len() > 1_000_000) {
        let _ = fs::rename(&path, agent_dir().join("agent.old.log"));
    }
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{} {line}", Local::now().format("%Y-%m-%d %H:%M:%S"));
    }
}

/// Registro detallado: las mismas líneas con las rutas completas (nunca
/// contraseñas) y los archivos que no se pudieron leer en cada copia. Vive en
/// la carpeta privada (solo SYSTEM y Administradores) y rota a ~5 MB.
const DETAIL_LOG: &str = "agente-detalle.log";
const DETAIL_OLD: &str = "agente-detalle.old.log";
const DETAIL_MAX: u64 = 5_000_000;

pub fn log_detail(line: &str) {
    let dir = private_dir();
    if !dir.is_dir() || platform::is_reparse_point(&agent_dir()) || platform::is_reparse_point(&dir) {
        return;
    }
    log_detail_in(&dir, line);
}

fn log_detail_in(dir: &std::path::Path, line: &str) {
    let line = line.split(['\r', '\n']).map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" · ");
    let line = restic::redact_credentials(&line);
    let path = dir.join(DETAIL_LOG);
    if fs::metadata(&path).is_ok_and(|m| m.len() > DETAIL_MAX) {
        let _ = fs::remove_file(dir.join(DETAIL_OLD));
        let _ = fs::rename(&path, dir.join(DETAIL_OLD));
    }
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{} {line}", Local::now().format("%Y-%m-%d %H:%M:%S"));
    }
}

/// Últimas líneas del registro detallado (solo administradores).
pub fn detail_tail(max: usize) -> Result<Vec<String>, String> {
    require_admin()?;
    Ok(detail_tail_in(&private_dir(), max))
}

fn detail_tail_in(dir: &std::path::Path, max: usize) -> Vec<String> {
    let read = |name: &str| fs::read(dir.join(name)).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let text = read(DETAIL_OLD) + &read(DETAIL_LOG);
    let lines: Vec<String> = text.lines().filter(|l| !l.trim().is_empty()).map(String::from).collect();
    lines[lines.len().saturating_sub(max)..].to_vec()
}

/// Marcas del registro detallado para los archivos de una copia.
fn detail_start(key: &str) -> String {
    format!("COPIA [{key}] empieza")
}
fn detail_file(key: &str) -> String {
    format!("ARCHIVO [{key}] ")
}

/// Archivos que no se pudieron leer en la última copia automática de un plan
/// (del registro detallado; solo administradores). «ruta: error» cada uno.
pub fn failed_files(repo_id: &str, plan_id: &str) -> Result<Vec<String>, String> {
    require_admin()?;
    Ok(failed_files_in(&private_dir(), &crate::plans::plan_key(repo_id, plan_id)))
}

fn failed_files_in(dir: &std::path::Path, key: &str) -> Vec<String> {
    let lines = detail_tail_in(dir, usize::MAX);
    let start = detail_start(key);
    let from = lines.iter().rposition(|l| l.contains(&start)).map_or(0, |i| i + 1);
    let marker = detail_file(key);
    lines[from..].iter().filter_map(|l| l.split_once(&marker).map(|(_, rest)| rest.to_string())).take(MAX_DETAIL_FILES).collect()
}

/// Como mucho, cuántos archivos con error se anotan por copia.
const MAX_DETAIL_FILES: usize = 500;

/// Últimas líneas del registro del agente (lo pueden leer todos los usuarios
/// del equipo; no contiene contraseñas). Incluye el registro anterior si el
/// actual acaba de rotar.
pub fn log_tail(max: usize) -> Vec<String> {
    let read = |name: &str| fs::read(agent_dir().join(name)).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let text = read("agent.old.log") + &read("agent.log");
    let lines: Vec<String> = text.lines().filter(|l| !l.trim().is_empty()).map(String::from).collect();
    lines[lines.len().saturating_sub(max)..].to_vec()
}

// ---------- Configurar desde la app (requiere administrador) ----------

/// Modo de pruebas: carpeta temporal, sin permisos ni tarea programada.
pub fn test_mode() -> bool {
    test_dir().is_some()
}

pub fn require_admin() -> Result<(), String> {
    if platform::is_elevated() || test_mode() {
        Ok(())
    } else {
        Err("Esta acción requiere abrir Resguardo como administrador.".into())
    }
}

/// Crea la carpeta del agente con sus permisos (o los corrige).
///
/// Los secretos se guardan en memoria mientras se restablecen los permisos,
/// para que nunca queden legibles ni un instante.
pub fn prepare_dir() -> Result<(), String> {
    let dir = agent_dir();
    if test_mode() {
        for d in [dir.clone(), private_dir(), requests_dir()] {
            fs::create_dir_all(&d).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    // Si el dueño no es SYSTEM ni Administradores (por ejemplo, la creó la
    // cuenta Administrador con UAC desactivado, o un usuario antes que
    // Resguardo), se toma igualmente: dueño Administradores y permisos
    // protegidos, lo que quita cualquier otro acceso. No se aparta la carpeta:
    // se perderían las copias programadas de un equipo en producción.
    // Un enlace en lugar de la carpeta (o de sus subcarpetas) no se sigue: se
    // quita y se crea una carpeta de verdad con sus permisos.
    let mut links = Vec::new();
    if remove_link(&dir)? {
        links.push(dir.display().to_string());
    }
    if dir.exists() && !platform::owned_by_admins(&dir) {
        log("AVISO: la carpeta del agente no pertenecía a Administradores; se corrigen su dueño y sus permisos.");
    }
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    platform::apply_sddl(&dir, platform::SDDL_AGENT_DIR)?;
    for sub in [private_dir(), requests_dir()] {
        if remove_link(&sub)? {
            links.push(sub.display().to_string());
        }
    }
    for l in &links {
        log(&format!("AVISO: {l} era un enlace (unión o simbólico); se quitó y se creó una carpeta normal."));
    }

    // Restablecer lo que ya hay dentro es una capa extra: si falla con algún
    // archivo en uso, se anota y se sigue.
    let reset = |d: &std::path::Path, skip: &[&str]| {
        if let Err(e) = platform::reset_children(d, skip) {
            log(&format!("Aviso al restablecer permisos en {}: {e}", d.display()));
        }
    };

    // 1. Carpeta privada protegida antes de mover nada a ella.
    let private = private_dir();
    fs::create_dir_all(&private).map_err(|e| e.to_string())?;
    platform::apply_sddl(&private, platform::SDDL_PRIVATE_DIR)?;
    reset(&private, &[]);

    // 2. Secretos de versiones anteriores: se mueven (el archivo conserva sus
    //    permisos, que ya eran solo SYSTEM y Administradores) y después
    //    heredan los de la carpeta privada. Nunca se borran ni se reescriben.
    let legacy = legacy_secrets_file();
    if legacy.exists() {
        if secrets_file().exists() {
            let _ = fs::remove_file(&legacy); // la de la carpeta privada es la vigente
        } else {
            fs::rename(&legacy, secrets_file()).map_err(|e| format!("No se pudieron mover los secretos del agente: {e}"))?;
            reset(&private, &[]);
        }
    }

    // 3. Solicitudes.
    let requests = requests_dir();
    fs::create_dir_all(&requests).map_err(|e| e.to_string())?;
    platform::apply_sddl(&requests, platform::SDDL_REQUESTS_DIR)?;
    reset(&requests, &[]);

    // 4. El resto (agent.json, state.json, registro…): solo permisos heredados.
    reset(&dir, &["privado", "solicitudes"]);
    Ok(())
}

/// Borra los datos del agente (al desinstalar, si el usuario lo pide): la
/// tarea programada y `C:\ProgramData\Resguardo` entero, con los secretos.
/// Si la carpeta fuera un enlace, solo se quita el enlace: nunca se borra lo
/// que hay en su destino (un usuario podría haberla creado apuntando a otro
/// sitio). Dentro no se siguen enlaces (`remove_dir_all` no los sigue).
pub fn purge() -> Result<(), String> {
    require_admin()?;
    if !test_mode() {
        let _ = platform::uninstall_task();
    }
    let dir = agent_dir();
    if remove_link(&dir)? {
        return Ok(());
    }
    if fs::symlink_metadata(&dir).is_err() {
        return Ok(());
    }
    if fs::remove_dir_all(&dir).is_err() {
        // Archivos de solo lectura (p. ej. restos de una prueba de restauración).
        crate::restore_test::clear_readonly(&dir);
        fs::remove_dir_all(&dir).map_err(|e| format!("No se pudo borrar {}: {e}", dir.display()))?;
    }
    Ok(())
}

// ---------- Solicitudes de copia (p. ej. al cerrar la app) ----------

/// Carpeta donde la app (sin permisos de administrador) deja peticiones de copia.
pub fn requests_dir() -> PathBuf {
    agent_dir().join("solicitudes")
}

/// ¿Puede el agente terminar una copia de este repositorio? Solo si lo tiene
/// configurado con carpetas (tiene su contraseña) y su tarea existe.
pub fn can_hand_off(repo_id: &str, plan_id: &str) -> bool {
    let configured = load_config().repos.iter().any(|r| r.id == repo_id && r.plans.iter().any(|p| p.id == plan_id));
    // Quien atiende las solicitudes: la tarea de la app o el servicio de resguardo-agente.exe.
    configured && requests_dir().is_dir() && (test_mode() || is_managed_agent() || platform::task_installed())
}

/// Pide al agente una copia de este repositorio en su próxima vuelta (≤ 5 min).
pub fn request_backup(repo_id: &str, plan_id: &str) -> Result<(), String> {
    if !can_hand_off(repo_id, plan_id) {
        return Err("El agente no tiene programado este plan de copia.".into());
    }
    let body = serde_json::json!({ "requested_at": Local::now().to_rfc3339() });
    let key = crate::plans::plan_key(repo_id, plan_id);
    fs::write(requests_dir().join(format!("{key}.json")), body.to_string()).map_err(|e| format!("No se pudo dejar la solicitud al agente: {e}"))
}

/// Solicitudes nuevas: `<id>.json` de un repositorio configurado, escrito
/// después de la última solicitud atendida. El agente nunca borra ni escribe
/// en esta carpeta (los usuarios pueden escribir en ella): solo mira fechas.
fn take_requests(config: &AgentConfig, state: &mut AgentState) -> Vec<String> {
    let mut ids = Vec::new();
    let keys = config.repos.iter().flat_map(|r| r.plans.iter().map(|p| crate::plans::plan_key(&r.id, &p.id)));
    for key in keys {
        let path = requests_dir().join(format!("{key}.json"));
        // Solo archivos normales (no enlaces ni carpetas).
        let Ok(meta) = fs::symlink_metadata(&path) else { continue };
        if !meta.is_file() {
            continue;
        }
        let Ok(modified) = meta.modified() else { continue };
        // En UTC (sin saltos por cambio de hora) y nunca en el futuro: una
        // fecha futura puesta a mano no puede bloquear solicitudes posteriores.
        let now = chrono::Utc::now();
        let stamp = DateTime::<chrono::Utc>::from(modified).min(now);
        let seen = state.requests_seen.get(&key).and_then(|s| DateTime::parse_from_rfc3339(s).ok());
        if seen.is_some_and(|seen| seen >= stamp) {
            continue;
        }
        state.requests_seen.insert(key.clone(), stamp.to_rfc3339());
        ids.push(key);
    }
    ids
}

/// Convierte la programación de versiones anteriores (un horario y unas
/// carpetas por repositorio) en un plan «Principal» con el horario
/// equivalente. Devuelve true si cambió algo.
pub fn migrate_legacy(config: &mut AgentConfig, state: &mut AgentState) -> bool {
    let mut changed = false;
    for repo in &mut config.repos {
        if !repo.plans.is_empty() || repo.paths.is_empty() {
            continue;
        }
        let Some(schedule) = plan_schedule_from_legacy(&repo.schedule) else { continue };
        let plan_id = crate::plans::LEGACY_PLAN.to_string();
        repo.plans.push(AgentPlan {
            id: plan_id.clone(),
            name: "Principal".into(),
            paths: std::mem::take(&mut repo.paths),
            excludes: std::mem::take(&mut repo.excludes),
            tags: vec![],
            schedule,
            enabled_at: repo.enabled_at.clone(),
            skip_unchanged: false,
            ganchos: vec![],
        });
        repo.schedule = Schedule::Plans;
        if let Some(run) = state.runs.remove(&repo.id) {
            state.runs.insert(crate::plans::plan_key(&repo.id, &plan_id), run);
        }
        changed = true;
    }
    changed
}

/// La tarea existe mientras haya copias programadas o el equipo esté vinculado a la web.
pub fn sync_task() -> Result<(), String> {
    if test_mode() {
        return Ok(());
    }
    // En un equipo gestionado, el ciclo lo lanza el servicio de Resguardo
    // Agente cada 5 minutos: sin tarea programada (la de la app completa, si
    // también está instalada, la gestiona resguardo.exe).
    if is_managed_agent() {
        return Ok(());
    }
    let needed = !load_config().repos.is_empty() || crate::web::load_link().is_some_and(|l| !l.revoked);
    if needed {
        // Instalaciones anteriores (sin carpeta privada): se prepara aquí, que
        // se ejecuta como administrador (instalador o app elevada). Si algo
        // falla, la tarea se crea igualmente (nunca se quedan sin copias
        // automáticas) y el agente vuelve a intentar los permisos.
        let mut xml_dir = private_dir();
        if !xml_dir.is_dir() {
            if let Err(e) = prepare_dir() {
                log(&format!("No se pudo preparar la carpeta privada del agente: {e}"));
                xml_dir = agent_dir();
            }
        }
        platform::install_task(&xml_dir)
    } else {
        platform::uninstall_task()
    }
}

/// ¿Es este proceso `resguardo-agente` (equipo gestionado)?
pub fn is_managed_agent() -> bool {
    static ES: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ES.get_or_init(|| {
        std::env::current_exe().ok().and_then(|e| e.file_stem().map(|n| n.to_string_lossy().eq_ignore_ascii_case("resguardo-agente"))).unwrap_or(false)
    })
}

/// Linux y macOS: bloqueo exclusivo del archivo (lo suelta el sistema si el proceso muere).
#[cfg(unix)]
pub fn bloqueo_exclusivo(f: &fs::File) -> bool {
    use std::os::unix::io::AsRawFd;
    // SAFETY: descriptor válido mientras viva `f`.
    unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) == 0 }
}

/// Equivalente del horario de versiones anteriores (una sola programación
/// por repositorio). «Solo vigilar» no tiene equivalente.
pub fn plan_schedule_from_legacy(s: &Schedule) -> Option<crate::plans::PlanSchedule> {
    let all = (0..=6).collect::<Vec<u8>>();
    match s {
        // Más de un día entre copias: una vez al día (menos de una semana)
        // o los domingos (una semana o más), a medianoche.
        Schedule::Hours { every } if *every > 24 => Some(crate::plans::PlanSchedule {
            days: if *every >= 168 { vec![6] } else { all },
            mode: "at".into(),
            times: vec!["00:00".into()],
            every_hours: 1,
            from: String::new(),
            to: String::new(),
            rules: vec![],
        }),
        Schedule::Hours { every } => Some(crate::plans::PlanSchedule {
            days: all,
            mode: "every".into(),
            times: vec![],
            every_hours: (*every).clamp(1, 24),
            from: "00:00".into(),
            to: "23:59".into(),
            rules: vec![],
        }),
        Schedule::Daily { time } => Some(crate::plans::PlanSchedule {
            days: all,
            mode: "at".into(),
            times: vec![time.clone()],
            every_hours: 1,
            from: String::new(),
            to: String::new(),
            rules: vec![],
        }),
        Schedule::Weekly { weekday, time } => Some(crate::plans::PlanSchedule {
            days: vec![(*weekday).min(6) as u8],
            mode: "at".into(),
            times: vec![time.clone()],
            every_hours: 1,
            from: String::new(),
            to: String::new(),
            rules: vec![],
        }),
        Schedule::Rules { rules } => Some(crate::plans::PlanSchedule::from_rules(rules.clone())),
        Schedule::Monitor { .. } | Schedule::Plans | Schedule::AfterBackup { .. } => None,
    }
}

/// Copia el certificado de CA propio de un destino a la carpeta privada.
/// SYSTEM nunca usa un archivo que un usuario pueda cambiar: con otro
/// certificado podría hacerse pasar por el servidor (y recibir su contraseña).
fn private_cacert(repo_id: &str, path: &str) -> Result<String, String> {
    let safe_id: String = repo_id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
    let source = std::path::Path::new(path);
    // Ya copiado (se vuelve a programar el mismo destino).
    if source.parent() == Some(private_dir().as_path()) {
        return Ok(path.to_string());
    }
    let meta = fs::metadata(source).map_err(|e| format!("No se pudo leer el certificado {path}: {e}"))?;
    if !meta.is_file() || meta.len() > 1_000_000 {
        return Err(format!("{path} no parece un certificado."));
    }
    let bytes = fs::read(source).map_err(|e| format!("No se pudo leer el certificado {path}: {e}"))?;
    let dest = private_dir().join(format!("ca-{safe_id}.pem"));
    write_new(&dest, &bytes).map_err(|e| format!("No se pudo copiar el certificado: {e}"))?;
    Ok(dest.to_string_lossy().into_owned())
}

pub fn is_scheduled(repo_id: &str) -> bool {
    load_config().repos.iter().any(|r| r.id == repo_id)
}

/// Activa, cambia o desactiva la programación de un repositorio. Copia al
/// agente la configuración y las contraseñas (desde el almacén del usuario).
/// `schedule`: `Plans` (copiar según los planes con horario del repositorio),
/// `Monitor` (solo vigilar) o `None` (desactivar).
/// Quita del agente un repositorio por su id (equipos gestionados).
pub fn set_schedule_by_id(id: &str, _schedule: Option<Schedule>) -> Result<(), String> {
    require_admin()?;
    let mut config = load_config();
    config.repos.retain(|r| r.id != id);
    write_json("agent.json", &config)?;
    let mut secrets = load_secrets()?;
    if secrets.remove(id).is_some() {
        save_secrets(&secrets)?;
    }
    Ok(())
}

pub fn set_schedule(repo: &Repo, access: Option<&Access>, schedule: Option<Schedule>) -> Result<(), String> {
    require_admin()?;
    prepare_dir()?;
    let mut config = load_config();
    let mut secrets = load_secrets()?;
    // Las tareas (verificación, copia externa) y sus credenciales se conservan
    // al cambiar el horario de las copias.
    let previous = config.repos.iter().find(|r| r.id == repo.id).cloned();
    let previous_secret = secrets.remove(&repo.id);
    config.repos.retain(|r| r.id != repo.id);

    if let Some(schedule) = schedule {
        schedule.validate()?;
        if !test_mode() {
            platform::check_task_exe()?;
        }
        // Horarios de versiones anteriores: se guardan como planes.
        let schedule = match schedule {
            Schedule::Monitor { .. } | Schedule::Plans => schedule,
            _ => return Err("Programa las copias con los planes del repositorio.".into()),
        };
        let now = Local::now().to_rfc3339();
        let plans: Vec<AgentPlan> = if matches!(schedule, Schedule::Plans) {
            repo.plans
                .iter()
                .filter_map(|p| {
                    let s = p.schedule.clone()?;
                    // Si el plan ya estaba igual, conserva su fecha de activación.
                    let same = previous
                        .as_ref()
                        .and_then(|prev| prev.plans.iter().find(|q| q.id == p.id))
                        .filter(|q| q.schedule == s && q.paths == p.paths)
                        .map(|q| q.enabled_at.clone());
                    Some(AgentPlan {
                        id: p.id.clone(),
                        name: p.name.clone(),
                        paths: p.paths.clone(),
                        excludes: p.excludes.clone(),
                        tags: p.tags.clone(),
                        schedule: s,
                        enabled_at: same.unwrap_or_else(|| now.clone()),
                        skip_unchanged: p.skip_unchanged,
                        ganchos: p.ganchos.clone(),
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        let keeps_tasks = previous.as_ref().is_some_and(|p| p.verify.is_some() || p.offsite.is_some() || p.restore_test.is_some());
        if matches!(schedule, Schedule::Plans) && plans.is_empty() && !keeps_tasks {
            return Err("Ningún plan tiene horario: añade días y horas a algún plan.".into());
        }
        let access = access.ok_or("Faltan los datos de acceso del repositorio.")?;
        config.repos.push(AgentRepo {
            id: repo.id.clone(),
            name: repo.name.clone(),
            location: repo.location.clone(),
            rest_username: repo.rest_username.clone(),
            cacert: match &repo.cacert {
                Some(path) => Some(private_cacert(&repo.id, path)?),
                None => None,
            },
            paths: Vec::new(),
            excludes: Vec::new(),
            schedule,
            enabled_at: now,
            plans,
            verify: previous.as_ref().and_then(|p| p.verify.clone()),
            restore_test: previous.as_ref().and_then(|p| p.restore_test.clone()),
            offsite: previous.as_ref().and_then(|p| p.offsite.clone()),
            // Cambiar las copias no quita una pausa.
            pause: previous.as_ref().and_then(|p| p.pause.clone()),
            kit: repo.kit.clone().or_else(|| previous.as_ref().and_then(|p| p.kit.clone())),
            has_retention: repo.retention.is_some(),
            object_lock: repo.object_lock,
            place_id: repo.place_id.clone(),
            place_name: repo.place_name.clone(),
        });
        let old = previous_secret.unwrap_or_default();
        secrets.insert(
            repo.id.clone(),
            Secret {
                password: access.password.clone(),
                rest_password: access.rest_auth.as_ref().map(|(_, p)| p.clone()),
                offsite_password: old.offsite_password,
                offsite_key_id: old.offsite_key_id,
                offsite_secret: old.offsite_secret,
                env: access.env.clone(),
                offsite_location: old.offsite_location,
                offsite_env: old.offsite_env,
            },
        );
    }

    save_secrets(&secrets)?;
    write_json("agent.json", &config)?;
    // Una sola tarea para todo: existe mientras haya copias programadas o vínculo con la web.
    sync_task()?;
    log(&format!("Programación de «{}» actualizada.", repo.name));
    Ok(())
}

/// Activa, cambia o quita la verificación programada de un repositorio que
/// ya está en el agente (con copias automáticas o «solo vigilar»).
pub fn set_verify(repo_id: &str, verify: Option<crate::tasks::Verify>) -> Result<(), String> {
    require_admin()?;
    let mut config = load_config();
    let entry = config.repos.iter_mut().find(|r| r.id == repo_id).ok_or("Activa primero las copias automáticas o «Solo vigilar» en este repositorio.")?;
    if let Some(v) = &verify {
        v.validate()?;
    }
    entry.verify = verify;
    let name = entry.name.clone();
    write_json("agent.json", &config)?;
    log(&format!("Verificación de «{name}» actualizada."));
    Ok(())
}

/// Activa, cambia o quita la prueba de restauración de un repositorio del agente.
pub fn set_restore_test(repo_id: &str, test: Option<crate::restore_test::RestoreTest>) -> Result<(), String> {
    require_admin()?;
    let mut config = load_config();
    let entry = config.repos.iter_mut().find(|r| r.id == repo_id).ok_or("Activa primero las copias automáticas o «Solo vigilar» en este repositorio.")?;
    if let Some(t) = &test {
        t.validate()?;
    }
    entry.restore_test = test;
    let name = entry.name.clone();
    write_json("agent.json", &config)?;
    log(&format!("Prueba de restauración de «{name}» actualizada."));
    Ok(())
}

/// Credenciales de la copia externa (van a los secretos cifrados del agente).
pub struct OffsiteSecrets {
    pub password: Option<String>,
    pub key_id: Option<String>,
    pub key_secret: Option<String>,
    /// Destino de la app: ubicación con credenciales y variables de nube.
    pub location: Option<String>,
    pub env: Vec<(String, String)>,
}

/// Activa, cambia o quita la copia externa de un repositorio del agente.
pub fn set_offsite(repo_id: &str, offsite: Option<crate::tasks::Offsite>, creds: Option<OffsiteSecrets>) -> Result<(), String> {
    require_admin()?;
    let mut config = load_config();
    let mut secrets = load_secrets()?;
    let entry = config.repos.iter_mut().find(|r| r.id == repo_id).ok_or("Activa primero las copias automáticas o «Solo vigilar» en este repositorio.")?;
    if let Some(o) = &offsite {
        o.schedule.validate_offsite()?;
        if let Some(g) = &o.guard {
            g.validate()?;
        }
        if let Some(v) = &o.verify {
            v.validate()?;
        }
        if let Some(p) = &o.retention {
            p.validate()?;
        }
        if o.location.trim().is_empty() {
            return Err("Falta la ubicación del repositorio.".into());
        }
        if crate::restic::has_embedded_password(&o.location) {
            return Err("No pongas la contraseña dentro de la ubicación: usa los campos de credenciales.".into());
        }
    }
    let secret = secrets.get_mut(repo_id).ok_or("Este repositorio no tiene contraseña guardada en el agente.")?;
    match (&offsite, creds) {
        (Some(_), Some(c)) => {
            secret.offsite_password = c.password.filter(|p| !p.is_empty());
            secret.offsite_key_id = c.key_id.filter(|p| !p.is_empty());
            secret.offsite_secret = c.key_secret.filter(|p| !p.is_empty());
            secret.offsite_location = c.location.filter(|p| !p.is_empty());
            secret.offsite_env = c.env;
        }
        (Some(_), None) => {} // se mantienen las credenciales guardadas
        (None, _) => {
            secret.offsite_password = None;
            secret.offsite_key_id = None;
            secret.offsite_secret = None;
            secret.offsite_location = None;
            secret.offsite_env = Vec::new();
        }
    }
    entry.offsite = offsite;
    let name = entry.name.clone();
    save_secrets(&secrets)?;
    write_json("agent.json", &config)?;
    log(&format!("Copia externa de «{name}» actualizada."));
    Ok(())
}

/// Activa o desactiva «Copias a distancia» en este equipo. Solo como
/// administrador (agent.json); la app pide además la contraseña de un destino.
pub fn set_remote_backup(enabled: bool) -> Result<(), String> {
    require_admin()?;
    let mut config = load_config();
    if config.remote_backup == enabled {
        return Ok(());
    }
    config.remote_backup = enabled;
    write_json("agent.json", &config)?;
    log(if enabled {
        "Copias a distancia activadas: se aceptan peticiones de «Copiar ahora» de la web y de otros equipos de la cuenta."
    } else {
        "Copias a distancia desactivadas."
    });
    Ok(())
}

/// Copia en el agente lo que sabe la app de un destino y usa el informe a la
/// web (kit de recuperación, retención, bloqueo de objetos). Solo como
/// administrador: los usuarios no pueden tocar agent.json. Si el destino no
/// está en el agente, no hace nada.
pub fn sync_meta(repo: &Repo) -> Result<(), String> {
    require_admin()?;
    let mut config = load_config();
    let Some(entry) = config.repos.iter_mut().find(|r| r.id == repo.id) else { return Ok(()) };
    if !copy_meta(entry, repo) {
        return Ok(());
    }
    write_json("agent.json", &config)
}

/// Copia al agente los indicadores que guarda la app (kit, retención,
/// bloqueo de objetos y destino). Devuelve true si cambió algo.
fn copy_meta(entry: &mut AgentRepo, repo: &Repo) -> bool {
    let (kit, ret, lock) = (repo.kit.clone(), repo.retention.is_some(), repo.object_lock);
    if entry.kit == kit && entry.has_retention == ret && entry.object_lock == lock && entry.place_id == repo.place_id && entry.place_name == repo.place_name {
        return false;
    }
    entry.kit = kit;
    entry.has_retention = ret;
    entry.object_lock = lock;
    entry.place_id = repo.place_id.clone();
    entry.place_name = repo.place_name.clone();
    true
}

/// Como `sync_meta`, para todos los destinos a la vez (al abrir la app como
/// administrador): la retención, el kit o el bloqueo de objetos guardados sin
/// administrador llegan así al agente (y a la web). Es un reflejo de datos ya
/// guardados con la contraseña de cada destino, y solo indicadores para la
/// salud de la protección: no cambia qué copia, verifica ni borra el agente.
/// Devuelve cuántos destinos cambiaron.
pub fn sync_all_meta(repos: &[Repo]) -> Result<usize, String> {
    require_admin()?;
    let mut config = load_config();
    let mut changed = 0;
    for entry in config.repos.iter_mut() {
        let Some(repo) = repos.iter().find(|r| r.id == entry.id) else { continue };
        if copy_meta(entry, repo) {
            changed += 1;
        }
    }
    if changed > 0 {
        write_json("agent.json", &config)?;
    }
    Ok(changed)
}

/// Anota en el historial de actividad una pausa o una reanudación.
fn pause_history(repo_id: &str, repo_name: &str, kind: &str, origin: &str, message: String) {
    let now = Local::now().to_rfc3339();
    crate::history::append(
        &crate::history::agent_file(),
        &crate::history::Entry {
            kind: kind.into(),
            origin: origin.into(),
            repo_id: repo_id.into(),
            repo_name: repo_name.into(),
            started: now.clone(),
            finished: now,
            result: "info".into(),
            message,
            ..Default::default()
        },
    );
}

/// Primera letra en mayúscula («hasta…» → «Hasta…»).
fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// Pausa las copias automáticas de un destino del agente hasta `until`
/// (RFC 3339; `None`: hasta reanudarlas a mano). Si ya estaba en pausa, solo
/// cambia cuándo termina.
pub fn set_pause(repo_id: &str, until: Option<&str>) -> Result<(), String> {
    require_admin()?;
    let now = Local::now();
    let until = until.map(|u| parse_pause_until(u, now)).transpose()?;
    let mut config = load_config();
    let entry = config.repos.iter_mut().find(|r| r.id == repo_id).ok_or("Este repositorio no tiene copias automáticas en este equipo.")?;
    let since = entry.active_pause(now).map(|p| p.since.clone()).unwrap_or_else(|| now.to_rfc3339());
    let pause = Pause { since, until: until.map(|u| u.to_rfc3339()) };
    let words = pause.until_words();
    entry.pause = Some(pause);
    let name = entry.name.clone();
    write_json("agent.json", &config)?;
    log(&format!("Copias automáticas de «{name}» en pausa {words}."));
    pause_history(repo_id, &name, "pause", "manual", format!("{}.", capitalize(&words)));
    Ok(())
}

/// Reanuda las copias automáticas de un destino (si no estaba en pausa, no hace nada).
pub fn resume(repo_id: &str) -> Result<(), String> {
    require_admin()?;
    let mut config = load_config();
    let entry = config.repos.iter_mut().find(|r| r.id == repo_id).ok_or("Este repositorio no tiene copias automáticas en este equipo.")?;
    if entry.pause.take().is_none() {
        return Ok(());
    }
    let name = entry.name.clone();
    write_json("agent.json", &config)?;
    log(&format!("Copias automáticas de «{name}» reanudadas."));
    pause_history(repo_id, &name, "resume", "manual", "Reanudadas a mano.".into());
    Ok(())
}

/// Quita las pausas que ya terminaron. Devuelve los destinos reanudados (id y nombre).
pub fn resume_expired(config: &mut AgentConfig, now: DateTime<Local>) -> Vec<(String, String)> {
    let mut resumed = Vec::new();
    for repo in &mut config.repos {
        if repo.pause.as_ref().is_some_and(|p| !p.active(now)) {
            repo.pause = None;
            resumed.push((repo.id.clone(), repo.name.clone()));
        }
    }
    resumed
}

// ---------- Ejecución (Programador de tareas) ----------

/// Impide dos ejecuciones a la vez (por si una copia dura más de 5 minutos).
/// Evita dos ejecuciones del agente a la vez. El archivo se abre en exclusiva
/// y se mantiene abierto: si el proceso muere (se actualiza la app, se apaga
/// el equipo…), Windows cierra el archivo y el bloqueo desaparece solo.
struct RunLock(#[allow(dead_code)] fs::File);

impl RunLock {
    fn acquire() -> Option<Self> {
        // En la carpeta privada: ningún usuario puede abrirlo para bloquear al
        // agente. (Instalaciones anteriores aún sin ella: la carpeta principal.)
        let dir = if private_dir().is_dir() { private_dir() } else { agent_dir() };
        let _ = fs::create_dir_all(&dir);
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(false);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0);
        }
        let f = options.open(dir.join("agent.lock")).ok()?;
        #[cfg(unix)]
        {
            if !bloqueo_exclusivo(&f) {
                return None;
            }
        }
        Some(Self(f))
    }
}

/// Reintentos tras una copia programada fallida (p. ej. un corte de red) y
/// espera entre ellos.
const MAX_RETRIES: u32 = 2;
const RETRY_AFTER_MIN: i64 = 15;

/// ¿Toca reintentar? Si la última copia falló, hace al menos 15 minutos y
/// aún quedan reintentos.
fn retry_due(last: Option<&RunRecord>, attempts: u32, now: DateTime<Local>) -> bool {
    let Some(last) = last.filter(|r| r.result == "error") else { return false };
    if attempts >= MAX_RETRIES {
        return false;
    }
    // Si terminó «en el futuro», el reloj se corrigió hacia atrás: no se espera a esa hora.
    DateTime::parse_from_rfc3339(&last.finished).is_ok_and(|f| {
        let f = f.with_timezone(&Local);
        now.signed_duration_since(f) >= Duration::minutes(RETRY_AFTER_MIN) || crate::plans::clock_went_back(f, now)
    })
}

/// ¿Toca copiar este plan en esta vuelta? `None`: no; `Some(false)`: copia
/// (programada o pedida); `Some(true)`: reintento tras un fallo. En pausa solo
/// se atienden las copias pedidas (p. ej. una copia a mano que la app pasa al
/// agente al cerrarse). Al terminar la pausa, si se saltó alguna hora, se hace
/// una sola copia (la última hora que pasó), no una por cada hora saltada.
fn plan_turn(repo: &AgentRepo, plan: &AgentPlan, state: &AgentState, asked: bool, now: DateTime<Local>) -> Option<bool> {
    if asked {
        return Some(false);
    }
    if repo.active_pause(now).is_some() {
        return None;
    }
    let key = crate::plans::plan_key(&repo.id, &plan.id);
    let since = state.runs.get(&key).map(|r| r.started.as_str()).unwrap_or(&plan.enabled_at);
    let since = DateTime::parse_from_rfc3339(since).map(|d| d.with_timezone(&Local)).unwrap_or_else(|_| Local::now());
    if plan.schedule.is_due(since, now) {
        return Some(false);
    }
    let attempts = state.retries.get(&key).copied().unwrap_or(0);
    retry_due(state.runs.get(&key), attempts, now).then_some(true)
}

/// Cuánto esperar hasta la próxima vuelta del servicio: `max` (la vuelta
/// normal) o menos si antes toca alguna copia programada (un «cada 5
/// minutos», o «a las 10:02»), para no llegar hasta 5 minutos tarde. Nunca
/// menos de 30 segundos.
pub fn wait_until_next_slot(max: std::time::Duration, now: DateTime<Local>) -> std::time::Duration {
    let config = load_config();
    let next = config.repos.iter().filter(|r| r.active_pause(now).is_none()).flat_map(|r| r.plans.iter()).filter_map(|p| p.schedule.next_slot(now)).min();
    wait_for(next, now, max)
}

fn wait_for(next: Option<DateTime<Local>>, now: DateTime<Local>, max: std::time::Duration) -> std::time::Duration {
    match next.and_then(|t| t.signed_duration_since(now).to_std().ok()) {
        // Un par de segundos de margen: la vuelta empieza ya pasada la hora.
        Some(d) => (d + std::time::Duration::from_secs(2)).clamp(std::time::Duration::from_secs(30), max.max(std::time::Duration::from_secs(30))),
        None => max,
    }
}

/// Explica por qué el agente no encuentra unas carpetas. Las unidades de red
/// asignadas (p. ej. `S:` a `\\servidor\datos`) son de cada usuario: SYSTEM no
/// las ve; con la ruta de red completa sí puede, si el equipo tiene permiso.
fn missing_paths_message(missing: &[&String]) -> String {
    // Sin las rutas (el estado lo leen todos los usuarios): cuántas y, si son
    // unidades, cuáles (la letra basta para reconocerlas).
    let n = missing.len();
    let mut drives: Vec<String> = missing
        .iter()
        .filter_map(|p| {
            let b = p.as_bytes();
            (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':').then(|| format!("{}:", (b[0] as char).to_ascii_uppercase()))
        })
        .collect();
    drives.dedup();
    let list = match (n, drives.is_empty()) {
        (1, true) => "1 carpeta de esta copia".to_string(),
        (1, false) => format!("1 carpeta de esta copia (en {})", drives.join(", ")),
        (_, true) => format!("{n} carpetas de esta copia"),
        (_, false) => format!("{n} carpetas de esta copia (en {})", drives.join(", ")),
    };
    let drive_letter = missing.iter().any(|p| {
        let b = p.as_bytes();
        b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
    });
    let unc = missing.iter().any(|p| p.starts_with(r"\\"));
    if unc {
        format!(
            "El agente no puede acceder a {list}, en una carpeta compartida de red. Las copias automáticas se hacen como el sistema del equipo: \
             da permiso de lectura a la cuenta del equipo en esa carpeta compartida."
        )
    } else if drive_letter {
        format!(
            "El agente no encuentra {list}. Si es una unidad de red, el sistema no ve las unidades asignadas a \
             los usuarios: usa la ruta de red completa (\\\\servidor\\carpeta). Si es un disco, comprueba que esté \
             conectado."
        )
    } else {
        format!("El agente no encuentra {list}.")
    }
}

/// Resultado de una copia según el código de salida de restic y su resumen.
/// Con «Solo guardar si hay cambios», un código 0 sin `snapshot_id` es una
/// copia correcta sin cambios: "ok", sin versión y `unchanged` (no se reintenta).
fn apply_result(record: &mut RunRecord, code: Option<i32>, summary: Option<&serde_json::Value>, skip_unchanged: bool, stderr: &str) {
    if let Some(s) = summary {
        record.snapshot_id = s["snapshot_id"].as_str().map(String::from);
        record.data_added = s["data_added_packed"].as_u64().or(s["data_added"].as_u64());
        record.files_new = s["files_new"].as_u64();
        record.files_changed = s["files_changed"].as_u64();
    }
    record.unchanged = skip_unchanged && code == Some(0) && summary.is_some_and(crate::restic::summary_unchanged);
    (record.result, record.message) = match code {
        Some(0) if record.unchanged => ("ok".into(), crate::restic::UNCHANGED_MESSAGE.into()),
        Some(0) => ("ok".into(), "Copia completada.".into()),
        Some(3) => ("warning".into(), "Copia terminada, pero algunos archivos no se pudieron leer.".into()),
        code => ("error".into(), restic::exit_error(code, stderr)),
    };
}

/// Límite de una copia programada.
const BACKUP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20 * 3600);

fn backup(repo: &AgentRepo, plan: &AgentPlan, secret: &Secret, progress: &mut dyn FnMut(&serde_json::Value)) -> RunRecord {
    let started = Local::now();
    let access = Access {
        location: repo.location.clone(),
        password: secret.password.clone(),
        rest_auth: repo.rest_username.clone().zip(secret.rest_password.clone()),
        cacert: repo.cacert.clone(),
        env: secret.env.clone(),
    };
    // --retry-lock: si el servidor está aplicando la retención, se espera.
    let mut args: Vec<String> = vec!["backup".into(), "--json".into(), "--retry-lock".into(), "30m".into()];
    // «Solo guardar si hay cambios»: sin cambios, restic no crea versión (y termina bien).
    if plan.skip_unchanged {
        args.push("--skip-if-unchanged".into());
    }
    // Etiquetas del plan; sin ninguna, se marca como copia del agente.
    let tags = if plan.tags.is_empty() { vec!["resguardo-agente".to_string()] } else { plan.tags.clone() };
    for t in tags {
        args.push("--tag".into());
        args.push(t);
    }
    // Instantánea de disco (VSS) para copiar archivos abiertos; requiere administrador.
    if cfg!(windows) && platform::is_elevated() {
        args.push("--use-fs-snapshot".into());
    }
    for pattern in &plan.excludes {
        args.push(restic::exclude_flag().into());
        args.push(pattern.clone());
    }
    // La copia la hace SYSTEM: nunca incluye el registro de Windows (SAM,
    // SECURITY…), que expondría las credenciales del equipo en el repositorio.
    let windir = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    // (--iexclude: sin distinguir mayúsculas; %SystemRoot% suele ser C:\WINDOWS.)
    for protected in [r"System32\config", r"repair"] {
        args.push("--iexclude".into());
        args.push(format!(r"{windir}\{protected}"));
    }
    args.push("--".into());
    args.extend(plan.paths.iter().cloned());

    let mut record = RunRecord { started: started.to_rfc3339(), ..Default::default() };
    let key = crate::plans::plan_key(&repo.id, &plan.id);
    log_detail(&format!("{} «{}» / «{}»", detail_start(&key), repo.name, plan.name));
    // Carpetas que el agente (SYSTEM) no ve: mejor un mensaje claro que el de restic.
    let missing: Vec<&String> = plan.paths.iter().filter(|p| !std::path::Path::new(p.as_str()).exists()).collect();
    for p in &missing {
        log_detail(&format!("{}{p}: el agente no encuentra esta carpeta", detail_file(&key)));
    }
    if !missing.is_empty() && missing.len() == plan.paths.len() {
        record.result = "error".into();
        record.message = missing_paths_message(&missing);
        record.finished = Local::now().to_rfc3339();
        return record;
    }
    // Ganchos de plantilla (volcados de bases de datos, comprobaciones): sus
    // carpetas entran en la copia y sus volcados se borran al terminar.
    if !plan.ganchos.is_empty() {
        progress(&serde_json::json!({ "message_type": "phase", "phase": "hooks" }));
    }
    let preparado = crate::ganchos::antes(&plan.ganchos);
    if !plan.ganchos.is_empty() {
        progress(&serde_json::json!({ "message_type": "phase", "phase": null }));
    }
    for c in &preparado.carpetas {
        if !plan.paths.contains(c) {
            args.push(c.clone());
        }
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut on_line = |line: &str| {
        if line.contains(r#""message_type":"status""#) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                progress(&v);
            }
        }
    };
    match restic::run_raw_lines(&access, &refs, BACKUP_TIMEOUT, &mut on_line) {
        Ok(out) => {
            let summary = String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                .find(|v| v["message_type"] == "summary");
            apply_result(&mut record, out.code, summary.as_ref(), plan.skip_unchanged, &out.stderr);
            // Archivos que no se pudieron leer: con su ruta, solo en el registro detallado.
            let errors = out.stderr.lines().filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok()).filter(|v| v["message_type"] == "error");
            for v in errors.take(MAX_DETAIL_FILES) {
                log_detail(&format!("{}{}", detail_file(&key), crate::jobs::describe_error(&v)));
            }
        }
        Err(e) => {
            record.result = "error".into();
            record.message = e;
        }
    }
    crate::ganchos::despues(&preparado);
    crate::ganchos::aplicar(&mut record, &preparado);
    record.finished = Local::now().to_rfc3339();
    record
}

/// Punto de entrada de `--agent-run`. Devuelve el código de salida.
pub fn run() -> i32 {
    restic::require_bundled();
    if !dirs_safe() {
        // Ni configuración ni secretos desde una carpeta desviada. Se arregla
        // al abrir la app como administrador o al reinstalar (prepare_dir).
        return 2;
    }
    let Some(_lock) = RunLock::acquire() else {
        return 0; // ya hay una ejecución en curso
    };
    let config = load_config();
    let mut state = load_state();
    state.last_tick = Some(Local::now().to_rfc3339());
    // Si una ejecución anterior se cortó a mitad de copia, ya no está en curso.
    state.running = None;
    let linked = crate::web::load_link().is_some_and(|l| !l.revoked);
    if config.repos.is_empty() && !linked {
        let _ = write_json("state.json", &state);
        return 0;
    }
    let secrets = match load_secrets() {
        Ok(s) => s,
        Err(e) => {
            log(&format!("ERROR: {e}"));
            return 1;
        }
    };
    // Consola de equipos gestionados: IP nueva del servidor y retención.
    if !test_mode() {
        crate::console::agent_hook();
    }

    // Instalaciones anteriores: el agente (como SYSTEM) corrige los permisos.
    let retry_acl = state
        .acl_failed_at
        .as_deref()
        .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
        .is_none_or(|t| Local::now().signed_duration_since(t) > Duration::hours(6));
    if state.acl_version < ACL_VERSION && retry_acl && !test_mode() {
        match prepare_dir() {
            Ok(()) => {
                state.acl_version = ACL_VERSION;
                state.acl_failed_at = None;
                log("Permisos de la carpeta del agente actualizados.");
            }
            Err(e) => {
                state.acl_failed_at = Some(Local::now().to_rfc3339());
                log(&format!("ERROR: no se pudieron actualizar los permisos de la carpeta (se reintentará): {e}"));
            }
        }
    }
    // Configuración de versiones anteriores: se convierte en planes.
    let mut config = config;
    if migrate_legacy(&mut config, &mut state) {
        if let Err(e) = write_json("agent.json", &config) {
            log(&format!("ERROR: no se pudo guardar la conversión a planes: {e}"));
        } else {
            log("Programación convertida a planes de copia (plan «Principal», mismo horario).");
        }
    }
    // Certificados de CA de versiones anteriores (en una carpeta del usuario):
    // se copian a la carpeta privada para que nadie pueda cambiarlos después.
    let mut moved = false;
    for repo in config.repos.iter_mut() {
        let Some(path) = repo.cacert.clone() else { continue };
        if std::path::Path::new(&path).parent() == Some(private_dir().as_path()) {
            continue;
        }
        match private_cacert(&repo.id, &path) {
            Ok(copy) => {
                log(&format!("Certificado de «{}» copiado a la carpeta privada del agente.", repo.name));
                repo.cacert = Some(copy);
                moved = true;
            }
            Err(e) => log(&format!("AVISO: certificado de «{}»: {e}", repo.name)),
        }
    }
    if moved {
        if let Err(e) = write_json("agent.json", &config) {
            log(&format!("ERROR: no se pudo guardar la configuración: {e}"));
        }
    }
    // Pausas que ya terminaron: se reanudan las copias. (Si no se pudiera
    // guardar, en esta vuelta se copia igualmente y se reintenta en la siguiente.)
    let resumed = resume_expired(&mut config, Local::now());
    if !resumed.is_empty() {
        match write_json("agent.json", &config) {
            Ok(()) => {
                for (id, name) in &resumed {
                    log(&format!("Copias automáticas de «{name}» reanudadas: terminó la pausa."));
                    pause_history(id, name, "resume", "agent", "Reanudadas solas al terminar la pausa.".into());
                }
            }
            Err(e) => log(&format!("ERROR: no se pudo guardar el final de una pausa: {e}")),
        }
    }
    let config = config;

    let mut requested = take_requests(&config, &mut state);
    for id in &requested {
        log(&format!("Solicitud de copia recibida para {id}."));
    }
    // Copias pedidas a distancia (si este equipo lo permite y está vinculado).
    let web_access = crate::web::load_link().filter(|l| !l.revoked).and_then(|l| crate::web::device_secret(&secrets).map(|s| (l, s)));
    let mut remote: HashMap<String, crate::remote::Command> = HashMap::new();
    if config.remote_backup {
        if let Some((link, secret)) = &web_access {
            match crate::remote::take(&link.url, &link.key, &link.device_id, secret) {
                Ok(commands) => {
                    for cmd in commands {
                        match crate::remote::validate(&cmd, &config, Local::now()) {
                            Ok(key) if remote.contains_key(&key) => {
                                crate::remote::finish(
                                    &link.url,
                                    &link.key,
                                    &link.device_id,
                                    secret,
                                    &cmd.id,
                                    "rejected",
                                    "Ya había otra petición igual en marcha.",
                                );
                            }
                            Ok(key) => {
                                log(&format!("Copia pedida a distancia (desde {}) para {key}.", cmd.origin_label()));
                                if !requested.contains(&key) {
                                    requested.push(key.clone());
                                }
                                remote.insert(key, cmd);
                            }
                            Err(why) => {
                                log(&format!("Petición de copia a distancia rechazada (desde {}): {why}", cmd.origin_label()));
                                crate::remote::finish(&link.url, &link.key, &link.device_id, secret, &cmd.id, "rejected", &why);
                            }
                        }
                    }
                }
                Err(e) => log(&format!("No se pudieron recoger las copias pedidas a distancia: {e}")),
            }
        }
    }
    let mut remote_done: Vec<String> = Vec::new();
    // Destinos compartidos entre los equipos del usuario (cifrado de extremo a extremo).
    if let Some((link, secret)) = &web_access {
        for note in crate::share::tick(link, secret) {
            log(&note);
            let now = Local::now().to_rfc3339();
            crate::history::append(
                &crate::history::agent_file(),
                &crate::history::Entry {
                    kind: "share".into(),
                    origin: "agent".into(),
                    started: now.clone(),
                    finished: now,
                    result: "info".into(),
                    message: note,
                    ..Default::default()
                },
            );
        }
    }

    let mut failures = 0;
    let mut ran: Vec<String> = Vec::new();
    for repo in &config.repos {
        // La verificación bloquea el repositorio: las copias se hacen al terminar.
        if crate::tasks::running_on(&repo.id, "verify") {
            continue;
        }
        for plan in &repo.plans {
            let key = crate::plans::plan_key(&repo.id, &plan.id);
            let asked = requested.contains(&key);
            let Some(retry) = plan_turn(repo, plan, &state, asked, Local::now()) else { continue };
            let attempts = state.retries.get(&key).copied().unwrap_or(0);
            if retry {
                log(&format!("Reintento {} de {MAX_RETRIES} de «{}» (plan «{}»).", attempts + 1, repo.name, plan.name));
            }
            let Some(secret) = secrets.get(&repo.id) else {
                log(&format!("ERROR: «{}» no tiene contraseña guardada en el agente.", repo.name));
                continue;
            };
            let from_remote = remote.get(&key);
            let quiet = if apply_discreet(&config) { " (modo discreto: prioridad baja)" } else { "" };
            match from_remote {
                Some(cmd) => log(&format!("Copia de «{}» (plan «{}»), pedida a distancia desde {}…{quiet}", repo.name, plan.name, cmd.origin_label())),
                None => log(&format!("Copia de «{}» (plan «{}»)…{quiet}", repo.name, plan.name)),
            }
            state.running = Some(RunningCopy {
                repo_id: repo.id.clone(),
                plan_id: Some(plan.id.clone()),
                started: Local::now().to_rfc3339(),
                percent: None,
                files_done: 0,
                total_files: 0,
                bytes_done: 0,
                total_bytes: 0,
                seconds_remaining: None,
                updated: None,
                phase: None,
                bytes_per_s: None,
                read_bps: None,
                upload_bps: None,
                files_per_s: None,
            });
            let _ = write_json("state.json", &state);
            // Lo que restic ha leído y escrito (para la lectura y la subida reales).
            let mut io_antes: Option<(u32, (u64, u64))> = None;
            // La web muestra «Copiando…» mientras dura.
            crate::web::report_started(&config, &secrets, &mut state);
            let mut last_write = std::time::Instant::now();
            let record = backup(repo, plan, secret, &mut |v| {
                // Cambio de fase (ganchos «Antes de copiar»): se guarda enseguida.
                if v["message_type"] == "phase" {
                    if let Some(r) = state.running.as_mut() {
                        r.phase = v["phase"].as_str().map(str::to_string);
                        r.updated = Some(Local::now().to_rfc3339());
                    }
                    let _ = write_json("state.json", &state);
                    return;
                }
                if last_write.elapsed() < PROGRESS_EVERY {
                    return;
                }
                let seconds = last_write.elapsed().as_secs_f64();
                last_write = std::time::Instant::now();
                let now = Local::now().to_rfc3339();
                let io = restic::pid_en_marcha().and_then(|pid| crate::platform::io_proceso(pid).map(|x| (pid, x)));
                if let Some(r) = state.running.as_mut() {
                    let bytes_done = v["bytes_done"].as_u64().unwrap_or(0);
                    let files_done = v["files_done"].as_u64().unwrap_or(0);
                    if r.updated.is_some() && r.phase.is_none() {
                        r.bytes_per_s = smoothed_rate(r.bytes_per_s, bytes_done.saturating_sub(r.bytes_done), seconds);
                        r.files_per_s = smoothed_rate(r.files_per_s, files_done.saturating_sub(r.files_done), seconds);
                        // Del mismo restic que la muestra anterior.
                        if let (Some((p0, (l0, e0))), Some((p1, (l1, e1)))) = (io_antes, io) {
                            if p0 == p1 {
                                r.read_bps = smoothed_rate(r.read_bps, l1.saturating_sub(l0), seconds);
                                r.upload_bps = smoothed_rate(r.upload_bps, e1.saturating_sub(e0), seconds);
                            }
                        }
                    }
                    r.phase = None;
                    r.percent = v["percent_done"].as_f64();
                    r.files_done = v["files_done"].as_u64().unwrap_or(0);
                    r.total_files = v["total_files"].as_u64().unwrap_or(0);
                    r.bytes_done = v["bytes_done"].as_u64().unwrap_or(0);
                    r.total_bytes = v["total_bytes"].as_u64().unwrap_or(0);
                    r.seconds_remaining = v["seconds_remaining"].as_u64();
                    r.updated = Some(now.clone());
                }
                io_antes = io;
                // El agente sigue vivo aunque la copia sea larga.
                state.last_tick = Some(now);
                let _ = write_json("state.json", &state);
            });
            state.running = None;
            let tag = match record.result.as_str() {
                "ok" => "",
                "warning" => "AVISO: ",
                _ => "ERROR: ",
            };
            log(&format!("{tag}«{}» (plan «{}»): {}", repo.name, plan.name, record.message));
            if record.result == "error" {
                failures += 1;
            }
            if !ran.contains(&repo.id) {
                ran.push(repo.id.clone());
            }
            // Reintentos: se cuentan solo los que siguen a un fallo. Una copia
            // pedida a distancia que falla no se reintenta sola.
            if record.result == "error" {
                let next = if from_remote.is_some() {
                    MAX_RETRIES
                } else if retry {
                    attempts + 1
                } else if asked {
                    attempts
                } else {
                    0
                };
                state.retries.insert(key.clone(), next);
            } else {
                state.retries.remove(&key);
            }
            // El estado y el historial los leen todos los usuarios: sin rutas.
            let mut record = record;
            record.message = crate::web::local_message(&record.message);
            crate::history::append(
                &crate::history::agent_file(),
                &crate::history::Entry {
                    kind: "backup".into(),
                    origin: if from_remote.is_some() {
                        "remote"
                    } else if retry {
                        "retry"
                    } else {
                        "agent"
                    }
                    .into(),
                    requested_from: from_remote.map(|c| c.origin_label()),
                    repo_id: repo.id.clone(),
                    repo_name: repo.name.clone(),
                    plan_id: Some(plan.id.clone()),
                    plan_name: Some(plan.name.clone()),
                    started: record.started.clone(),
                    finished: record.finished.clone(),
                    result: record.result.clone(),
                    message: record.message.clone(),
                    snapshot_id: record.snapshot_id.clone(),
                    data_added: record.data_added,
                    files_new: record.files_new,
                    files_changed: record.files_changed,
                    unchanged: record.unchanged,
                    user: None,
                    ganchos: record.ganchos.clone(),
                },
            );
            if let (Some(cmd), Some((link, secret))) = (from_remote, &web_access) {
                let status = if record.result == "error" { "failed" } else { "done" };
                crate::remote::finish(&link.url, &link.key, &link.device_id, secret, &cmd.id, status, &record.message);
                remote_done.push(key.clone());
            }
            state.runs.insert(key, record);
            // Se guarda tras cada copia: si algo se corta, lo hecho queda anotado.
            let _ = write_json("state.json", &state);
        }
    }
    // Peticiones a distancia que no se pudieron empezar (p. ej. una verificación
    // tenía el destino bloqueado): se avisa en lugar de dejarlas sin respuesta.
    if let Some((link, secret)) = &web_access {
        for (key, cmd) in remote.iter().filter(|(k, _)| !remote_done.contains(k)) {
            log(&format!("No se pudo empezar la copia pedida a distancia para {key}: el repositorio estaba ocupado."));
            crate::remote::finish(
                &link.url,
                &link.key,
                &link.device_id,
                secret,
                &cmd.id,
                "failed",
                "El repositorio estaba ocupado (verificación o subida en marcha). Vuelve a pedirla en unos minutos.",
            );
        }
    }
    // Informe a la web (si está vinculado): tras una copia, si cambió alguna
    // tarea de mantenimiento o cada 10 minutos.
    let tasks_changed = crate::tasks::load_state().changed_at.as_deref().is_some_and(|c| state.web.last_report.as_deref().is_none_or(|r| c > r));
    if tasks_changed && ran.is_empty() {
        // Solo cambió una tarea (p. ej. progreso de una subida): informe ligero,
        // sin volver a consultar las copias de cada repositorio.
        crate::web::report_started(&config, &secrets, &mut state);
    }
    // ¿Servidores REST de solo añadir? (Una vez al día, sin borrar nada.)
    for repo in config.repos.iter().filter(|r| r.location.starts_with("rest:")) {
        if state.append_only.get(&repo.id).is_some_and(|c| c.fresh(Local::now())) {
            continue;
        }
        let secret = secrets.get(&repo.id);
        let auth = repo.rest_username.as_deref().zip(secret.and_then(|s| s.rest_password.as_deref()));
        let result = crate::protection::probe_append_only(&repo.location, auth, repo.cacert.as_deref());
        state.append_only.insert(repo.id.clone(), crate::protection::AppendOnlyCheck { checked_at: Local::now().to_rfc3339(), append_only: result });
    }
    let _ = crate::web::tick(&config, &secrets, &mut state, &ran, false);
    let _ = write_json("state.json", &state);
    // Verificaciones y copias externas: en otro proceso, sin esperar.
    crate::tasks::spawn_if_due(&config);
    if failures > 0 {
        2
    } else {
        0
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn horario_anterior_equivalente() {
        let s = plan_schedule_from_legacy(&Schedule::Daily { time: "02:00".into() }).unwrap();
        assert!(s.is_due(at("2026-09-27 02:05"), at("2026-09-28 02:00")));
        let s = plan_schedule_from_legacy(&Schedule::Hours { every: 1 }).unwrap();
        assert_eq!(s.times_of_day().len(), 24);
        assert!(plan_schedule_from_legacy(&Schedule::Monitor { every: 1 }).is_none());
        // «Cada 48 h» → una vez al día; «cada 168 h» → los domingos (no a diario).
        let s = plan_schedule_from_legacy(&Schedule::Hours { every: 48 }).unwrap();
        assert_eq!((s.days.len(), s.times_of_day().len()), (7, 1));
        let s = plan_schedule_from_legacy(&Schedule::Hours { every: 168 }).unwrap();
        assert_eq!(s.days, vec![6]);
    }

    /// Una unión (la puede crear cualquier usuario) se detecta y se quita sin
    /// tocar la carpeta a la que apunta.
    #[cfg(windows)]
    #[test]
    fn enlaces_en_las_carpetas_del_agente() {
        let base = std::env::temp_dir().join(format!("resguardo-enlace-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let target = base.join("destino");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("dato.txt"), "no se toca").unwrap();
        let link = base.join("Resguardo");
        let ok = std::process::Command::new(platform::system_tool("cmd.exe"))
            .args(["/C", "mklink", "/J", &link.to_string_lossy(), &target.to_string_lossy()])
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "no se pudo crear la unión de prueba");
        assert!(platform::is_reparse_point(&link));
        assert!(!platform::is_reparse_point(&target));
        assert!(remove_link(&link).unwrap());
        assert!(!link.exists());
        assert!(target.join("dato.txt").exists(), "lo que había en el repositorio sigue ahí");
        assert!(!remove_link(&target).unwrap(), "una carpeta normal no se quita");
        let _ = fs::remove_dir_all(&base);
    }

    /// Borrar los datos del agente: quita la carpeta; si es un enlace, solo el enlace.
    #[cfg(windows)]
    #[test]
    fn borrar_datos_del_agente() {
        let base = std::env::temp_dir().join(format!("resguardo-purga-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let target = base.join("ajeno");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("importante.txt"), "no se toca").unwrap();
        let link = base.join("Resguardo");
        let ok = std::process::Command::new(platform::system_tool("cmd.exe"))
            .args(["/C", "mklink", "/J", &link.to_string_lossy(), &target.to_string_lossy()])
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok);
        // Como `purge`, pero sobre esta carpeta (sin tocar la del agente de verdad).
        assert!(remove_link(&link).unwrap());
        assert!(target.join("importante.txt").exists(), "nunca se borra el repositorio de un enlace");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn registro_detallado_y_archivos_con_error() {
        let dir = std::env::temp_dir().join(format!("resguardo-detalle-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let key = crate::plans::plan_key("rdet", "p1");
        log_detail_in(&dir, &format!("{} «D» / «C»", detail_start(&key)));
        log_detail_in(&dir, &format!(r"{}C:\viejo.txt: en uso", detail_file(&key)));
        log_detail_in(&dir, &format!("{} «D» / «C»", detail_start(&key)));
        log_detail_in(&dir, &format!(r"{}C:\Users\ana\a b.pst: en uso", detail_file(&key)));
        log_detail_in(&dir, &format!(r"{}C:\otra.txt: x", detail_file("otra#p")));
        log_detail_in(&dir, "Get https://ana:clave@h/r: 401");
        let files = failed_files_in(&dir, &key);
        assert_eq!(files, vec![r"C:\Users\ana\a b.pst: en uso".to_string()], "solo los de la última copia de ese plan");
        let tail = detail_tail_in(&dir, 50);
        assert!(tail.iter().any(|l| l.contains(r"C:\Users\ana\a b.pst")), "el detalle conserva las rutas");
        assert!(!tail.iter().any(|l| l.contains("clave")), "nunca contraseñas");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn escribir_siempre_en_archivo_nuevo() {
        let base = std::env::temp_dir().join(format!("resguardo-nuevo-{}", std::process::id()));
        fs::create_dir_all(&base).unwrap();
        let f = base.join("x.tmp");
        fs::write(&f, "antiguo y más largo").unwrap();
        write_new(&f, b"nuevo").unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap(), "nuevo");
        let _ = fs::remove_dir_all(&base);
    }

    fn at(s: &str) -> DateTime<Local> {
        Local.from_local_datetime(&chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()).unwrap()
    }

    #[test]
    fn horario_cada_n_horas() {
        let s = Schedule::Hours { every: 6 };
        assert!(!s.is_due(at("2026-09-28 08:00"), at("2026-09-28 13:59")));
        assert!(s.is_due(at("2026-09-28 08:00"), at("2026-09-28 14:00")));
    }

    #[test]
    fn horario_diario() {
        let s = Schedule::Daily { time: "02:00".into() };
        // Última copia ayer a las 02:05 → hoy a las 01:00 aún no toca; a las 02:00 sí.
        assert!(!s.is_due(at("2026-09-27 02:05"), at("2026-09-28 01:00")));
        assert!(s.is_due(at("2026-09-27 02:05"), at("2026-09-28 02:00")));
        // Ya se hizo hoy a las 02:03 → no se repite.
        assert!(!s.is_due(at("2026-09-28 02:03"), at("2026-09-28 18:00")));
        // Equipo apagado a las 02:00 de hoy: al encenderlo a las 09:00 se hace.
        assert!(s.is_due(at("2026-09-27 02:01"), at("2026-09-28 09:00")));
        // Recién activada a las 15:00: la primera copia es a las 02:00 del día siguiente.
        assert!(!s.is_due(at("2026-09-28 15:00"), at("2026-09-28 23:00")));
        assert!(s.is_due(at("2026-09-28 15:00"), at("2026-09-29 02:00")));
    }

    #[test]
    fn horario_semanal() {
        // Domingo (6) a las 03:00. El 27 de septiembre de 2026 es domingo.
        let s = Schedule::Weekly { weekday: 6, time: "03:00".into() };
        assert!(!s.is_due(at("2026-09-20 03:10"), at("2026-09-27 02:59")));
        assert!(s.is_due(at("2026-09-20 03:10"), at("2026-09-27 03:00")));
        assert!(!s.is_due(at("2026-09-27 03:02"), at("2026-10-03 23:00")));
    }

    #[test]
    fn reloj_corregido_hacia_atras() {
        // La última copia se anotó con el reloj adelantado (2030); ya está bien.
        let futuro = at("2030-01-01 02:05");
        assert!(Schedule::Daily { time: "02:00".into() }.is_due(futuro, at("2026-09-28 09:00")));
        assert!(Schedule::Hours { every: 6 }.is_due(futuro, at("2026-09-28 09:00")));
        let fallo = RunRecord { finished: futuro.to_rfc3339(), result: "error".into(), ..Default::default() };
        assert!(retry_due(Some(&fallo), 0, at("2026-09-28 09:00")));
        // Vigilar sigue sin copiar nunca.
        assert!(!Schedule::Monitor { every: 24 }.is_due(futuro, at("2026-09-28 09:00")));
    }

    #[test]
    fn vigilar_nunca_copia() {
        let s = Schedule::Monitor { every: 24 };
        assert!(!s.is_due(at("2020-01-01 00:00"), at("2026-09-28 12:00")));
        assert!(s.validate().is_ok());
        assert!(Schedule::Monitor { every: 0 }.validate().is_err());
    }

    #[test]
    fn valida_horarios() {
        assert!(Schedule::Daily { time: "25:00".into() }.validate().is_err());
        assert!(Schedule::Weekly { weekday: 7, time: "10:00".into() }.validate().is_err());
        assert!(Schedule::Hours { every: 0 }.validate().is_err());
        assert!(Schedule::Daily { time: "23:30".into() }.validate().is_ok());
    }

    /// Ciclo completo con el repo de prueba, en una carpeta temporal (sin tarea programada).
    #[test]
    fn carpetas_que_el_agente_no_ve() {
        let s = "S:\\".to_string();
        assert!(missing_paths_message(&[&s]).contains("unidad de red"));
        let u = "\\\\servidor\\datos".to_string();
        assert!(missing_paths_message(&[&u]).contains("cuenta del equipo"));
        // Sin rutas: el estado lo leen todos los usuarios del equipo.
        let m = missing_paths_message(&[&s, &u]);
        assert!(!m.contains("Users") && !m.contains("servidor") && m.contains("2 carpetas"), "{m}");
    }

    #[test]
    fn copia_sin_cambios_es_correcta_y_sin_version() {
        let sin_version = serde_json::json!({ "message_type": "summary", "files_new": 0, "files_changed": 0, "data_added": 0 });
        let mut r = RunRecord::default();
        apply_result(&mut r, Some(0), Some(&sin_version), true, "");
        assert_eq!((r.result.as_str(), r.unchanged, r.snapshot_id.as_deref()), ("ok", true, None));
        assert_eq!(r.message, crate::restic::UNCHANGED_MESSAGE);
        // Correcta: no toca reintentar.
        r.finished = at("2026-09-29 13:02").to_rfc3339();
        assert!(!retry_due(Some(&r), 0, at("2026-09-29 14:00")));
        // Para la web: `unchanged: true`, `result: "ok"` y sin versión.
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!((v["unchanged"].clone(), v["result"].clone()), (serde_json::json!(true), serde_json::json!("ok")));

        // Con versión, o sin la opción, es una copia normal (y `unchanged` no se envía).
        let con_version = serde_json::json!({ "message_type": "summary", "snapshot_id": "abc" });
        let mut r = RunRecord::default();
        apply_result(&mut r, Some(0), Some(&con_version), true, "");
        assert!(!r.unchanged && r.snapshot_id.is_some() && r.message == "Copia completada.");
        assert!(serde_json::to_value(&r).unwrap().get("unchanged").is_none());
        let mut r = RunRecord::default();
        apply_result(&mut r, Some(3), Some(&sin_version), true, "");
        assert!(!r.unchanged && r.result == "warning");
    }

    #[test]
    fn reintentos_tras_un_fallo() {
        let fallo = RunRecord { result: "error".into(), finished: at("2026-09-29 13:02").to_rfc3339(), ..Default::default() };
        let bien = RunRecord { result: "ok".into(), finished: at("2026-09-29 13:02").to_rfc3339(), ..Default::default() };
        assert!(!retry_due(Some(&fallo), 0, at("2026-09-29 13:10")), "espera 15 minutos");
        assert!(retry_due(Some(&fallo), 0, at("2026-09-29 13:17")));
        assert!(retry_due(Some(&fallo), 1, at("2026-09-29 13:40")));
        assert!(!retry_due(Some(&fallo), 2, at("2026-09-29 14:40")), "como mucho 2 reintentos");
        assert!(!retry_due(Some(&bien), 0, at("2026-09-29 14:40")));
        assert!(!retry_due(None, 0, at("2026-09-29 14:40")));
    }

    #[test]
    fn convierte_la_programacion_anterior_en_planes() {
        let mut config = AgentConfig {
            repos: vec![AgentRepo {
                id: "r".into(),
                name: "Siigo".into(),
                location: "rest:http://x/siigo".into(),
                rest_username: None,
                cacert: None,
                paths: vec!["S:\\".into()],
                excludes: vec!["*.tmp".into()],
                schedule: Schedule::Hours { every: 1 },
                enabled_at: "2026-09-29T08:00:00-05:00".into(),
                plans: vec![],
                verify: None,
                offsite: None,
                pause: None,
                kit: None,
                restore_test: None,
                has_retention: false,
                place_id: None,
                place_name: None,
                object_lock: false,
            }],
            ..Default::default()
        };
        let mut state = AgentState::default();
        state.runs.insert("r".into(), RunRecord { started: "x".into(), ..Default::default() });
        assert!(migrate_legacy(&mut config, &mut state));
        let r = &config.repos[0];
        assert_eq!(r.schedule, Schedule::Plans);
        assert!(r.paths.is_empty());
        assert_eq!(r.plans[0].id, crate::plans::LEGACY_PLAN);
        assert_eq!(r.plans[0].paths, vec!["S:\\".to_string()]);
        assert_eq!(r.plans[0].excludes, vec!["*.tmp".to_string()]);
        assert_eq!(r.plans[0].schedule.times_of_day().len(), 24);
        assert!(state.runs.contains_key("r#principal") && !state.runs.contains_key("r"));
        // Una segunda vez no cambia nada; «solo vigilar» no se convierte.
        assert!(!migrate_legacy(&mut config, &mut state));
        config.repos[0].plans.clear();
        config.repos[0].schedule = Schedule::Monitor { every: 1 };
        config.repos[0].paths = vec![];
        assert!(!migrate_legacy(&mut config, &mut state));
    }

    /// Destino con un plan que copia cada hora en punto, activado a las 08:10.
    pub fn repo_cada_hora() -> AgentRepo {
        AgentRepo {
            id: "r".into(),
            name: "Siigo".into(),
            location: "rest:http://x/siigo".into(),
            rest_username: None,
            cacert: None,
            paths: vec![],
            excludes: vec![],
            schedule: Schedule::Plans,
            enabled_at: at("2026-09-30 08:10").to_rfc3339(),
            plans: vec![AgentPlan {
                id: "p".into(),
                name: "Horaria".into(),
                paths: vec!["S:\\".into()],
                excludes: vec![],
                tags: vec![],
                schedule: crate::plans::PlanSchedule {
                    days: (0..=6).collect(),
                    mode: "every".into(),
                    times: vec![],
                    every_hours: 1,
                    from: "00:00".into(),
                    to: "23:59".into(),
                    rules: vec![],
                },
                enabled_at: at("2026-09-30 08:10").to_rfc3339(),
                skip_unchanged: false,
                ganchos: vec![],
            }],
            verify: None,
            offsite: None,
            pause: None,
            kit: None,
            restore_test: None,
            has_retention: false,
            place_id: None,
            place_name: None,
            object_lock: false,
        }
    }

    #[test]
    fn la_pausa_salta_los_planes() {
        let mut repo = repo_cada_hora();
        let plan = repo.plans[0].clone();
        let state = AgentState::default();
        let now = at("2026-09-30 10:30");
        assert_eq!(plan_turn(&repo, &plan, &state, false, now), Some(false), "sin pausa toca copiar");

        // Hasta reanudarla a mano: nada programado, pero sí lo pedido.
        repo.pause = Some(Pause { since: at("2026-09-30 09:00").to_rfc3339(), until: None });
        assert_eq!(plan_turn(&repo, &plan, &state, false, now), None);
        assert_eq!(plan_turn(&repo, &plan, &state, false, at("2026-12-30 10:30")), None, "no caduca sola");
        assert_eq!(plan_turn(&repo, &plan, &state, true, now), Some(false), "una copia pedida sí se hace");

        // Con fecha de fin: en pausa hasta entonces; después, vuelve a copiar.
        repo.pause = Some(Pause { since: at("2026-09-30 09:00").to_rfc3339(), until: Some(at("2026-09-30 11:00").to_rfc3339()) });
        assert_eq!(plan_turn(&repo, &plan, &state, false, now), None);
        assert_eq!(plan_turn(&repo, &plan, &state, false, at("2026-09-30 11:05")), Some(false));

        // Tampoco reintenta una copia fallida mientras está en pausa.
        let mut state = AgentState::default();
        let fallo = RunRecord {
            started: at("2026-09-30 10:00").to_rfc3339(),
            finished: at("2026-09-30 10:01").to_rfc3339(),
            result: "error".into(),
            ..Default::default()
        };
        state.runs.insert("r#p".into(), fallo);
        repo.pause = None;
        assert_eq!(plan_turn(&repo, &plan, &state, false, now), Some(true), "sin pausa, reintento");
        repo.pause = Some(Pause { since: at("2026-09-30 10:10").to_rfc3339(), until: None });
        assert_eq!(plan_turn(&repo, &plan, &state, false, now), None);

        // Una fecha de fin ilegible no deja las copias paradas para siempre.
        repo.pause = Some(Pause { since: "x".into(), until: Some("mañana".into()) });
        assert!(repo.active_pause(now).is_none());
    }

    #[test]
    fn reanuda_sola_al_terminar_la_pausa() {
        let mut con_fin = repo_cada_hora();
        con_fin.pause = Some(Pause { since: at("2026-09-30 07:30").to_rfc3339(), until: Some(at("2026-09-30 10:00").to_rfc3339()) });
        let mut sin_fin = repo_cada_hora();
        sin_fin.id = "otro".into();
        sin_fin.pause = Some(Pause { since: at("2026-09-30 07:30").to_rfc3339(), until: None });
        let mut config = AgentConfig { repos: vec![con_fin, sin_fin], ..Default::default() };

        assert!(resume_expired(&mut config, at("2026-09-30 09:59")).is_empty(), "aún no termina");
        let resumed = resume_expired(&mut config, at("2026-09-30 10:01"));
        assert_eq!(resumed, vec![("r".to_string(), "Siigo".to_string())]);
        assert!(config.repos[0].pause.is_none());
        assert!(config.repos[1].pause.is_some(), "la pausa sin fecha sigue");
        assert!(resume_expired(&mut config, at("2026-09-30 10:06")).is_empty());

        // Tras la pausa se saltaron las 08:00, 09:00 y 10:00: una sola copia de recuperación.
        let repo = &config.repos[0];
        let plan = &repo.plans[0];
        let mut state = AgentState::default();
        state.runs.insert("r#p".into(), RunRecord { started: at("2026-09-30 07:00").to_rfc3339(), result: "ok".into(), ..Default::default() });
        assert_eq!(plan_turn(repo, plan, &state, false, at("2026-09-30 10:01")), Some(false));
        state.runs.insert("r#p".into(), RunRecord { started: at("2026-09-30 10:01").to_rfc3339(), result: "ok".into(), ..Default::default() });
        assert_eq!(plan_turn(repo, plan, &state, false, at("2026-09-30 10:06")), None, "no hay ráfaga de copias");
        assert_eq!(plan_turn(repo, plan, &state, false, at("2026-09-30 11:00")), Some(false), "y vuelve a su horario");
    }

    /// Un plan «cada 10 minutos» (agente gestionado ≥ 0.7.9) con una copia
    /// que dura más que el intervalo: al terminar, una sola copia, no una por
    /// cada vez que pasó mientras copiaba.
    #[test]
    fn cada_10_minutos_con_una_copia_larga() {
        let mut repo = repo_cada_hora();
        repo.plans[0].schedule = crate::plans::PlanSchedule::from_rules(vec![crate::plans::ScheduleRule::Every {
            days: (0..=6).collect(),
            every_min: 10,
            from: "00:00".into(),
            to: "23:59".into(),
        }]);
        let plan = repo.plans[0].clone();
        let mut state = AgentState::default();
        state.runs.insert("r#p".into(), RunRecord { started: at("2026-09-30 10:00").to_rfc3339(), result: "ok".into(), ..Default::default() });
        // La copia de las 10:00 terminó a las 10:35: toca una (la de las 10:30)…
        assert_eq!(plan_turn(&repo, &plan, &state, false, at("2026-09-30 10:35")), Some(false));
        state.runs.insert("r#p".into(), RunRecord { started: at("2026-09-30 10:35").to_rfc3339(), result: "ok".into(), ..Default::default() });
        // …y la siguiente, a las 10:40 (pasados 5 minutos desde que empezó la anterior).
        assert_eq!(plan_turn(&repo, &plan, &state, false, at("2026-09-30 10:36")), None);
        assert_eq!(plan_turn(&repo, &plan, &state, false, at("2026-09-30 10:40")), Some(false));
        // El servicio se despierta para la próxima en vez de esperar la vuelta de 5 minutos.
        let max = std::time::Duration::from_secs(300);
        let s = |n: u64| std::time::Duration::from_secs(n);
        assert_eq!(wait_for(plan.schedule.next_slot(at("2026-09-30 10:37")), at("2026-09-30 10:37"), max), s(182));
        assert_eq!(wait_for(Some(at("2026-09-30 10:37")), at("2026-09-30 10:37"), max), s(30), "nunca menos de 30 s");
        assert_eq!(wait_for(Some(at("2026-09-30 12:00")), at("2026-09-30 10:37"), max), max);
        assert_eq!(wait_for(None, at("2026-09-30 10:37"), max), max);
    }

    #[test]
    fn valida_el_final_de_la_pausa() {
        let now = at("2026-09-30 10:30");
        assert!(parse_pause_until(&at("2026-09-30 10:00").to_rfc3339(), now).is_err(), "en el pasado");
        assert!(parse_pause_until(&now.to_rfc3339(), now).is_err());
        assert!(parse_pause_until("pronto", now).is_err());
        assert!(parse_pause_until(&(now + Duration::days(31)).to_rfc3339(), now).is_err(), "más de 30 días");
        assert!(parse_pause_until(&(now + Duration::days(30)).to_rfc3339(), now).is_ok());
        // Otra zona horaria: se compara el instante.
        let utc = (now + Duration::hours(1)).with_timezone(&chrono::Utc).to_rfc3339();
        assert_eq!(parse_pause_until(&utc, now).unwrap(), now + Duration::hours(1));

        let p = Pause { since: now.to_rfc3339(), until: Some(at("2026-10-01 06:00").to_rfc3339()) };
        assert_eq!(p.until_words(), "hasta el jueves 1 de octubre a las 06:00");
        assert_eq!(Pause { since: now.to_rfc3339(), until: None }.until_words(), "hasta que las reanudes");
    }

    #[test]
    fn configuracion_anterior_sin_pausa() {
        // agent.json de una versión anterior: sin el campo `pause`.
        let old = r#"{"repos":[{"id":"r","name":"Siigo","location":"rest:http://x/siigo","paths":[],
            "schedule":{"kind":"plans"},"enabled_at":"2026-09-29T08:00:00-05:00","plans":[],
            "verify":{"schedule":{"kind":"weekly","weekday":6,"time":"03:00"},"subset_percent":5,"enabled_at":"2026-09-29T08:00:00-05:00"}}]}"#;
        let config: AgentConfig = serde_json::from_str(old).unwrap();
        assert!(config.repos[0].pause.is_none());
        assert!(config.repos[0].verify.is_some(), "el resto se conserva");

        // Con pausa: ida y vuelta.
        let mut repo = config.repos[0].clone();
        repo.pause = Some(Pause { since: "2026-09-30T09:00:00-05:00".into(), until: None });
        let json = serde_json::to_value(&repo).unwrap();
        assert_eq!(json["pause"], serde_json::json!({ "since": "2026-09-30T09:00:00-05:00", "until": null }));
        let back: AgentRepo = serde_json::from_value(json).unwrap();
        assert_eq!(back.pause, repo.pause);
    }

    #[cfg(windows)]
    #[test]
    fn ciclo_completo_del_agente() {
        let _real = crate::restic::tests::real_repo_lock();
        let (Ok(location), Ok(pw), Ok(data)) =
            (std::env::var("RESGUARDO_TEST_REPO"), std::env::var("RESGUARDO_TEST_PASSWORD"), std::env::var("RESGUARDO_TEST_DATA"))
        else {
            return;
        };
        let dir = std::env::temp_dir().join(format!("resguardo-agente-{}", std::process::id()));
        std::env::set_var("RESGUARDO_AGENT_DIR", &dir);
        // Un plan: todos los días, cada hora en punto.
        let cada_hora = crate::plans::PlanSchedule {
            days: (0..=6).collect(),
            mode: "every".into(),
            times: vec![],
            every_hours: 1,
            from: "00:00".into(),
            to: "23:59".into(),
            rules: vec![],
        };
        let repo = Repo {
            id: "agente-test".into(),
            name: "Prueba".into(),
            location: location.clone(),
            rest_username: None,
            cacert: None,
            cloud_key_id: None,
            cloud_region: None,
            paths: vec![],
            excludes: vec![],
            plans: vec![crate::plans::Plan {
                id: "p1".into(),
                name: "Horaria".into(),
                paths: vec![data.clone()],
                excludes: vec![],
                tags: vec!["prueba-horaria".into()],
                schedule: Some(cada_hora),
                skip_unchanged: false,
                ganchos: vec![],
            }],
            retention: None,
            expected_hours: None,
            kit: None,
            object_lock: false,
            append_only: None,
            place_id: None,
            place_name: None,
        };
        // Los horarios de antes ya no se aceptan: se programa por planes.
        assert!(set_schedule(&repo, Some(&Access::new(location.clone(), pw.clone())), Some(Schedule::Hours { every: 1 })).is_err());
        set_schedule(&repo, Some(&Access::new(location.clone(), pw.clone())), Some(Schedule::Plans)).unwrap();
        assert_eq!(load_config().repos[0].plans.len(), 1);
        assert!(!fs::read(secrets_file()).unwrap().windows(4).any(|w| w == b"demo"), "secretos cifrados");
        assert!(!legacy_secrets_file().exists(), "los secretos viven en la carpeta privada");

        // Recién activada: aún no toca.
        assert_eq!(run(), 0);
        assert!(load_state().runs.is_empty());

        // Simulamos que se activó hace 2 horas: ahora sí toca.
        let mut cfg = load_config();
        cfg.repos[0].plans[0].enabled_at = (Local::now() - Duration::hours(2)).to_rfc3339();
        write_json("agent.json", &cfg).unwrap();
        assert_eq!(run(), 0);
        let rec = load_state().runs.remove("agente-test#p1").expect("debe haber una ejecución");
        assert_eq!(rec.result, "ok", "{}", rec.message);
        assert!(rec.snapshot_id.is_some());
        // La copia lleva la etiqueta del plan.
        let snaps = restic::snapshots(&Access::new(location.clone(), pw.clone())).unwrap();
        let hecha = snaps.iter().find(|s| Some(&s.id) == rec.snapshot_id.as_ref()).expect("la copia existe");
        assert_eq!(hecha.tags, vec!["prueba-horaria".to_string()]);
        // Y queda en el historial de actividad.
        let hist = crate::history::read(&crate::history::agent_file());
        assert!(hist.iter().any(|e| e.kind == "backup" && e.plan_name.as_deref() == Some("Horaria") && e.result == "ok"));

        assert!(load_state().running.is_none(), "al terminar ya no hay copia en curso");

        // Bloqueo: una segunda ejecución a la vez no hace nada, y al soltarlo
        // (o al morir el proceso) desaparece.
        {
            let _held = RunLock::acquire().expect("bloqueo libre");
            assert!(RunLock::acquire().is_none());
        }
        assert!(RunLock::acquire().is_some());

        // Y no se repite enseguida.
        assert_eq!(run(), 0);
        assert_eq!(load_state().runs["agente-test#p1"].started, rec.started);

        // Pero una solicitud (p. ej. al cerrar la app) hace copiar en la siguiente vuelta.
        assert!(can_hand_off("agente-test", "p1"));
        assert!(!can_hand_off("agente-test", "otro-plan"));
        request_backup("agente-test", "p1").unwrap();
        fs::write(requests_dir().join("desconocido.json"), "{}").unwrap();
        assert_eq!(run(), 0);
        assert_ne!(load_state().runs["agente-test#p1"].started, rec.started, "debe haber copiado de nuevo");
        // La misma solicitud no vuelve a contar (el agente no borra nada en esa carpeta).
        let again = load_state().runs["agente-test#p1"].started.clone();
        assert_eq!(run(), 0);
        assert_eq!(load_state().runs["agente-test#p1"].started, again, "una solicitud se atiende una sola vez");
        assert!(request_backup("otro-repo", "p1").is_err());

        // ---- Tareas de mantenimiento: copia externa a una carpeta y verificación ----
        let dest_dir = dir.join("destino");
        let src_access = Access::new(location.clone(), pw.clone());
        let dest_access = Access::new(dest_dir.to_string_lossy().into_owned(), pw.clone());
        assert_eq!(crate::tasks::prepare_destination(&src_access, &dest_access).unwrap(), "created");
        assert_eq!(crate::tasks::prepare_destination(&src_access, &dest_access).unwrap(), "existing");
        let hace_2h = (Local::now() - Duration::hours(2)).to_rfc3339();
        set_offsite(
            "agente-test",
            Some(crate::tasks::Offsite {
                location: dest_dir.to_string_lossy().into_owned(),
                provider: "otro".into(),
                region: None,
                schedule: Schedule::Hours { every: 1 },
                retention: Some(crate::retention::Policy { keep_last: 2, ..Default::default() }),
                limit_upload_kib: Some(100_000),
                target_name: None,
                enabled_at: hace_2h.clone(),
                guard: None,
                // También se verifica la copia externa (parte 1 de 2, con las credenciales de la subida).
                verify: Some(crate::tasks::Verify { schedule: Schedule::Hours { every: 1 }, subset_percent: 0, enabled_at: hace_2h.clone(), rotate_parts: 2 }),
            }),
            None,
        )
        .unwrap();
        set_verify(
            "agente-test",
            Some(crate::tasks::Verify { schedule: Schedule::Hours { every: 1 }, subset_percent: 10, enabled_at: hace_2h, rotate_parts: 0 }),
        )
        .unwrap();
        // Cambiar el horario de las copias conserva las tareas.
        set_schedule(&repo, Some(&Access::new(location.clone(), pw.clone())), Some(Schedule::Plans)).unwrap();
        assert!(load_config().repos[0].offsite.is_some() && load_config().repos[0].verify.is_some());

        assert_eq!(crate::tasks::run(), 0);
        let t = crate::tasks::load_state();
        let off = &t.runs["offsite:agente-test"];
        assert_eq!(off.result, "ok", "{}", off.message);
        assert!(off.files_new.unwrap_or(0) > 0, "debe haber subido copias: {}", off.message);
        let ver = &t.runs["verify:agente-test"];
        assert_eq!(ver.result, "ok", "{}", ver.message);
        let nube = &t.runs["verify_offsite:agente-test"];
        assert_eq!(nube.result, "ok", "{}", nube.message);
        assert!(nube.message.starts_with("Parte 1 de 2"), "{}", nube.message);
        assert!(nube.started >= off.finished, "la verificación de la nube va después de la subida");
        assert_eq!(t.rotation["offsite:agente-test"].next_part, 2);
        assert!(t.running.is_none());
        // La retención del destino dejó como mucho 2 copias por grupo.
        let dest_snaps = restic::snapshots(&dest_access).unwrap();
        assert!(!dest_snaps.is_empty() && dest_snaps.len() <= 2, "{} copias en el repositorio", dest_snaps.len());
        // Recién hechas: no se repiten; «ejecutar ahora» sí.
        assert_eq!(crate::tasks::run(), 0);
        assert_eq!(crate::tasks::load_state().runs["offsite:agente-test"].started, off.started);
        crate::tasks::request_now("agente-test", "offsite").unwrap();
        assert_eq!(crate::tasks::run(), 0);
        let again = &crate::tasks::load_state().runs["offsite:agente-test"];
        assert_ne!(again.started, off.started);
        assert!(again.message.starts_with("Nada nuevo"), "{}", again.message);

        // v1.28: la verificación automática de la consola (`config.verificaciones`).
        let mut vin = crate::servidor_v2::Vinculo::default();
        vin.repos_v2.push(crate::gestion_v2::RepoV2 { id: "agente-test".into(), ..Default::default() });
        let cfg = |j: serde_json::Value| serde_json::from_value::<crate::gestion_v2::Configuracion>(j).unwrap();
        let semanal = cfg(serde_json::json!({ "v": 1, "verificaciones": { "agente-test": { "cada_dias": 7, "porcentaje": 10 } } }));
        assert_eq!(crate::gestion_v2::aplicar_verificaciones(&vin, &semanal).unwrap(), 1);
        let puesta = load_config().repos[0].verify.clone().unwrap();
        assert_eq!((puesta.schedule.clone(), puesta.rotate_parts, puesta.subset_percent), (Schedule::Hours { every: 168 }, 10, 0));
        // La última verificación (la de arriba) cuenta: la próxima, 7 días después de ella.
        let proxima = crate::tasks::next_verify(&load_config().repos[0], &crate::tasks::load_state()).unwrap();
        assert_eq!(proxima, DateTime::parse_from_rfc3339(&ver.started).unwrap() + Duration::hours(168));
        // La misma otra vez no vuelve a empezar; sin el campo no se toca; sin el repositorio, se quita.
        crate::gestion_v2::aplicar_verificaciones(&vin, &semanal).unwrap();
        assert_eq!(load_config().repos[0].verify.as_ref().unwrap().enabled_at, puesta.enabled_at);
        crate::gestion_v2::aplicar_verificaciones(&vin, &cfg(serde_json::json!({ "v": 1 }))).unwrap();
        assert!(load_config().repos[0].verify.is_some());
        assert_eq!(crate::gestion_v2::aplicar_verificaciones(&vin, &cfg(serde_json::json!({ "v": 1, "verificaciones": {} }))).unwrap(), 0);
        assert!(load_config().repos[0].verify.is_none());

        set_schedule(&repo, None, None).unwrap();
        assert!(load_config().repos.is_empty());
        std::env::remove_var("RESGUARDO_AGENT_DIR");
        let _ = fs::remove_dir_all(dir);
    }
}
