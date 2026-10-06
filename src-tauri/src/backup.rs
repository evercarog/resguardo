//! Copias de seguridad con progreso en vivo (`restic backup --json`).

use crate::jobs::{self, Jobs};
use crate::plans::Plan;
use crate::restic;
use crate::store::Repo;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Runtime};

/// Evento que recibe la interfaz durante una copia.
pub const PROGRESS_EVENT: &str = "backup-progress";

#[derive(Deserialize)]
struct Status {
    #[serde(default)]
    percent_done: f64,
    #[serde(default)]
    total_files: u64,
    #[serde(default)]
    files_done: u64,
    #[serde(default)]
    total_bytes: u64,
    #[serde(default)]
    bytes_done: u64,
    seconds_remaining: Option<u64>,
    #[serde(default)]
    current_files: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Summary {
    #[serde(default)]
    pub files_new: u64,
    #[serde(default)]
    pub files_changed: u64,
    #[serde(default)]
    pub files_unmodified: u64,
    #[serde(default)]
    pub data_added: u64,
    #[serde(default)]
    pub total_files_processed: u64,
    #[serde(default)]
    pub total_bytes_processed: u64,
    #[serde(default)]
    pub total_duration: f64,
    pub snapshot_id: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Progress {
    Status {
        repo_id: String,
        percent: f64,
        files_done: u64,
        total_files: u64,
        bytes_done: u64,
        total_bytes: u64,
        seconds_remaining: Option<u64>,
        current: Option<String>,
    },
    ItemError {
        repo_id: String,
        message: String,
    },
}

#[derive(Serialize)]
pub struct BackupResult {
    pub summary: Option<Summary>,
    /// Errores por archivo (limitados a jobs::MAX_ERRORS).
    pub errors: Vec<String>,
    pub error_count: usize,
    /// restic terminó con código 3: el snapshot se creó pero faltan archivos.
    pub incomplete: bool,
    /// «Solo guardar si hay cambios» y no había cambios: terminó bien pero no
    /// se creó una versión nueva (el resumen no trae `snapshot_id`).
    pub unchanged: bool,
}

/// Mensaje de una copia correcta que no guardó versión por no haber cambios.
pub use resguardo_motor::restic::{summary_unchanged, UNCHANGED_MESSAGE};

/// Argumentos de `restic backup`. Las rutas van tras `--` para que ninguna
/// pueda interpretarse como opción.
fn backup_args(plan: &Plan, fs_snapshot: bool) -> Vec<String> {
    let mut args = vec!["backup".to_string(), "--json".to_string()];
    // Sin cambios, restic no crea una versión nueva (termina bien igualmente).
    if plan.skip_unchanged {
        args.push("--skip-if-unchanged".into());
    }
    // Como administrador, una instantánea del disco (VSS) copia también los
    // archivos abiertos (Outlook, bases de datos…), igual que el agente.
    if fs_snapshot {
        args.push("--use-fs-snapshot".into());
    }
    for t in &plan.tags {
        args.push("--tag".into());
        args.push(t.clone());
    }
    for pattern in &plan.excludes {
        args.push(restic::exclude_flag().into());
        args.push(pattern.clone());
    }
    args.push("--".into());
    args.extend(plan.paths.iter().cloned());
    args
}

/// Ejecuta una copia y bloquea hasta que termina. El progreso se emite como
/// eventos `PROGRESS_EVENT`. La clave del trabajo es el id del repositorio.
pub fn run<R: Runtime>(app: &AppHandle<R>, jobs: &Jobs, repo: &Repo, plan: &Plan, access: &restic::Access) -> Result<BackupResult, String> {
    if plan.paths.is_empty() {
        return Err("Este plan no tiene carpetas para copiar.".into());
    }

    let on_error = {
        let app = app.clone();
        let repo_id = repo.id.clone();
        move |message: String| {
            let _ = app.emit(PROGRESS_EVENT, Progress::ItemError { repo_id: repo_id.clone(), message });
        }
    };
    let on_status = |v| {
        if let Ok(s) = serde_json::from_value::<Status>(v) {
            let _ = app.emit(
                PROGRESS_EVENT,
                Progress::Status {
                    repo_id: repo.id.clone(),
                    percent: s.percent_done,
                    files_done: s.files_done,
                    total_files: s.total_files,
                    bytes_done: s.bytes_done,
                    total_bytes: s.total_bytes,
                    seconds_remaining: s.seconds_remaining,
                    current: s.current_files.into_iter().next(),
                },
            );
        }
    };

    let out = jobs::run(jobs, &repo.id, access, &backup_args(plan, cfg!(windows) && crate::platform::is_elevated()), on_status, on_error)?;
    if let Some(msg) = out.failure(&[0, 3]) {
        return Err(if out.cancelled { "Copia cancelada.".into() } else { msg });
    }
    let unchanged = plan.skip_unchanged && out.exit_code == Some(0) && out.summary.as_ref().is_some_and(summary_unchanged);
    Ok(BackupResult {
        unchanged,
        summary: out.summary.and_then(|v| serde_json::from_value(v).ok()),
        errors: out.errors,
        error_count: out.error_count,
        incomplete: out.exit_code == Some(3),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(location: &str) -> Repo {
        Repo {
            id: "test".into(),
            name: "test".into(),
            location: location.into(),
            rest_username: None,
            cacert: None,
            cloud_key_id: None,
            cloud_region: None,
            paths: vec![],
            excludes: vec![],
            plans: vec![],
            retention: None,
            expected_hours: None,
            kit: None,
            object_lock: false,
            append_only: None,
            place_id: None,
            place_name: None,
        }
    }

    fn plan(paths: Vec<String>, tags: Vec<String>) -> Plan {
        Plan {
            id: "p".into(),
            name: "Plan".into(),
            paths,
            excludes: vec!["*.tmp".into()],
            tags,
            schedule: None,
            skip_unchanged: false,
            ganchos: vec![],
            after: None,
        }
    }

    /// Copia real. Requiere RESGUARDO_TEST_REPO, RESGUARDO_TEST_PASSWORD y
    /// RESGUARDO_TEST_DATA (carpeta a copiar); si no, se omite.
    #[test]
    fn copia_real_devuelve_resumen() {
        let _real = crate::restic::tests::real_repo_lock();
        let Ok(data) = std::env::var("RESGUARDO_TEST_DATA") else { return };
        let (Ok(location), Ok(password)) = (std::env::var("RESGUARDO_TEST_REPO"), std::env::var("RESGUARDO_TEST_PASSWORD")) else {
            return;
        };
        let app = tauri::test::mock_app();
        let backups = Jobs::default();
        let access = restic::Access::new(location.clone(), password);
        let result = run(app.handle(), &backups, &repo(&location), &plan(vec![data], vec!["prueba-plan".into()]), &access).expect("la copia debería terminar");
        let summary = result.summary.expect("restic debería enviar un resumen");
        assert!(summary.snapshot_id.is_some());
        assert!(summary.total_files_processed > 0);
        assert!(!result.incomplete);
        assert!(backups.is_empty(), "la copia debe quitarse del registro");
        // La etiqueta del plan queda en la copia.
        let snaps = restic::snapshots(&access).unwrap();
        assert!(snaps.iter().any(|s| s.id == summary.snapshot_id.clone().unwrap() && s.tags.contains(&"prueba-plan".to_string())));
    }

    /// «Solo guardar si hay cambios» con restic real: la segunda copia seguida
    /// no cambia nada, termina bien y no crea versión. Usa los datos y la
    /// contraseña de prueba (`RESGUARDO_TEST_*`) en un repositorio propio, para
    /// no añadir versiones al compartido (otros tests miran su última versión).
    #[test]
    fn copia_real_sin_cambios_no_guarda_version() {
        let _real = crate::restic::tests::real_repo_lock();
        let Ok(data) = std::env::var("RESGUARDO_TEST_DATA") else { return };
        let (Ok(_), Ok(password)) = (std::env::var("RESGUARDO_TEST_REPO"), std::env::var("RESGUARDO_TEST_PASSWORD")) else {
            return;
        };
        // restic guarda también las carpetas superiores: si la de los datos está
        // dentro de TEMP (donde restic y otros tests crean archivos), su fecha
        // cambia entre una copia y otra y siempre habría «cambios». Se copian
        // los datos a una carpeta propia fuera de TEMP.
        fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
            std::fs::create_dir_all(to).unwrap();
            for e in std::fs::read_dir(from).unwrap().flatten() {
                let target = to.join(e.file_name());
                if e.path().is_dir() {
                    copy_dir(&e.path(), &target);
                } else {
                    std::fs::copy(e.path(), target).unwrap();
                }
            }
        }
        let own = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("prueba-sin-cambios");
        let _ = std::fs::remove_dir_all(&own);
        copy_dir(std::path::Path::new(&data), &own.join("datos"));
        let location = own.join("repo").to_string_lossy().into_owned();
        let access = restic::Access::new(location.clone(), password);
        let init = restic::run_raw(&access, &["init"], restic::CHECK_TIMEOUT).expect("restic init");
        assert_eq!(init.code, Some(0), "{}", init.stderr);

        let app = tauri::test::mock_app();
        let backups = Jobs::default();
        let dir = own.join("datos").to_string_lossy().into_owned();
        let plan = Plan { skip_unchanged: true, ..plan(vec![dir], vec!["prueba-sin-cambios".into()]) };
        // La primera guarda una versión (carpeta nueva); la segunda, sin cambios, no.
        let first = run(app.handle(), &backups, &repo(&location), &plan, &access).expect("primera copia");
        assert!(!first.unchanged);
        let before = restic::snapshots(&access).unwrap().len();
        let second = run(app.handle(), &backups, &repo(&location), &plan, &access).expect("segunda copia");
        let after = restic::snapshots(&access).unwrap().len();
        let _ = std::fs::remove_dir_all(&own);
        assert!(second.unchanged, "sin cambios no debe guardar una versión nueva");
        assert!(!second.incomplete);
        let summary = second.summary.expect("restic envía el resumen igualmente");
        assert!(summary.snapshot_id.is_none());
        assert_eq!((before, after), (1, 1), "no se creó ninguna versión");
    }

    #[test]
    fn resumen_sin_version_es_sin_cambios() {
        let unchanged = serde_json::json!({ "message_type": "summary", "files_new": 0, "files_changed": 0, "total_files_processed": 12 });
        assert!(summary_unchanged(&unchanged));
        let saved = serde_json::json!({ "message_type": "summary", "files_new": 1, "snapshot_id": "abc123" });
        assert!(!summary_unchanged(&saved));
        assert!(!summary_unchanged(&serde_json::json!({ "message_type": "status" })));
        // Con la opción, el resumen se deserializa sin versión.
        let s: Summary = serde_json::from_value(unchanged).unwrap();
        assert!(s.snapshot_id.is_none());
        assert!(backup_args(&Plan { skip_unchanged: true, ..plan(vec![r"C:\datos".into()], vec![]) }, false).contains(&"--skip-if-unchanged".to_string()));
        assert!(!backup_args(&plan(vec![r"C:\datos".into()], vec![]), false).contains(&"--skip-if-unchanged".to_string()));
        // Instantánea del disco solo si se pide (como administrador en Windows).
        assert!(backup_args(&plan(vec![r"C:\datos".into()], vec![]), true).contains(&"--use-fs-snapshot".to_string()));
        assert!(!backup_args(&plan(vec![r"C:\datos".into()], vec![]), false).contains(&"--use-fs-snapshot".to_string()));
    }

    #[test]
    fn sin_carpetas_no_ejecuta_restic() {
        let app = tauri::test::mock_app();
        let err = run(app.handle(), &Jobs::default(), &repo("x"), &plan(vec![], vec![]), &restic::Access::new("x", "x")).err().unwrap();
        assert_eq!(err, "Este plan no tiene carpetas para copiar.");
    }

    #[test]
    fn rutas_van_despues_de_doble_guion() {
        assert_eq!(
            backup_args(&plan(vec!["--password-file=/etc/passwd".into()], vec!["diaria".into()]), false),
            ["backup", "--json", "--tag", "diaria", restic::exclude_flag(), "*.tmp", "--", "--password-file=/etc/passwd"]
        );
    }
}
