//! Restauración de archivos de un snapshot (`restic restore --json`).
//!
//! Se restaura `snapshot:carpeta` con un `--include` por cada elemento
//! elegido, así los elementos quedan directamente dentro del destino en vez de
//! recrear toda la ruta original (`destino/C/Users/...`).

use crate::jobs::{self, Jobs};
use crate::restic::{self, Access};
use crate::store::Repo;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::{AppHandle, Emitter, Runtime};

/// Evento que recibe la interfaz durante una restauración.
pub const PROGRESS_EVENT: &str = "restore-progress";

/// Clave del trabajo: permite restaurar mientras se hace una copia del mismo repo.
pub fn job_key(repo_id: &str) -> String {
    format!("{repo_id}:restore")
}

#[derive(Deserialize)]
struct Status {
    #[serde(default)]
    percent_done: f64,
    #[serde(default)]
    total_files: u64,
    #[serde(default)]
    files_restored: u64,
    #[serde(default)]
    total_bytes: u64,
    #[serde(default)]
    bytes_restored: u64,
    seconds_remaining: Option<u64>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Summary {
    #[serde(default)]
    pub total_files: u64,
    #[serde(default)]
    pub files_restored: u64,
    #[serde(default)]
    pub files_skipped: u64,
    #[serde(default)]
    pub total_bytes: u64,
    #[serde(default)]
    pub bytes_restored: u64,
    #[serde(default)]
    pub bytes_skipped: u64,
    #[serde(default)]
    pub seconds_elapsed: f64,
}

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Progress {
    Status { repo_id: String, percent: f64, files_done: u64, total_files: u64, bytes_done: u64, total_bytes: u64, seconds_remaining: Option<u64> },
    ItemError { repo_id: String, message: String },
}

#[derive(Serialize)]
pub struct RestoreResult {
    pub summary: Option<Summary>,
    pub errors: Vec<String>,
    pub error_count: usize,
    pub target: String,
}

pub struct Request {
    pub snapshot: String,
    /// Carpeta del snapshot donde están los elementos elegidos.
    pub dir: String,
    /// Nombres dentro de `dir`. Vacío: restaurar el contenido completo de `dir`.
    pub names: Vec<String>,
    pub target: String,
    /// true: reemplazar archivos existentes en el destino. false: no tocarlos.
    pub overwrite: bool,
}

/// Los filtros de restic interpretan `*`, `?` y `[` como comodines, y en
/// Windows no se pueden escapar. Se cambian por `?` (un carácter cualquiera)
/// para que el patrón siga siendo lo más estrecho posible.
fn include_pattern(name: &str) -> String {
    let escaped: String = name.chars().map(|c| if matches!(c, '*' | '?' | '[' | ']') { '?' } else { c }).collect();
    format!("/{escaped}")
}

fn validate(req: &Request) -> Result<String, String> {
    if !restic::valid_snapshot_id(&req.snapshot) {
        return Err("ID de versión no válido.".into());
    }
    let dir = restic::snapshot_dir(&req.dir)?;
    for name in &req.names {
        if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\0') {
            return Err(format!("Nombre no válido: {name}"));
        }
    }
    if !Path::new(&req.target).is_absolute() {
        return Err("Elige la carpeta completa donde restaurar.".into());
    }
    Ok(dir)
}

fn restore_args(req: &Request, dir: &str) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "restore".into(),
        "--json".into(),
        "--target".into(),
        req.target.clone(),
        "--overwrite".into(),
        if req.overwrite { "always" } else { "never" }.into(),
    ];
    for name in &req.names {
        args.push("--include".into());
        args.push(include_pattern(name));
    }
    args.push("--".into());
    args.push(if dir == "/" { req.snapshot.clone() } else { format!("{}:{dir}", req.snapshot) });
    args
}

/// Restaura y bloquea hasta que termina. El progreso se emite como eventos `PROGRESS_EVENT`.
pub fn run<R: Runtime>(app: &AppHandle<R>, jobs: &Jobs, repo: &Repo, access: &Access, req: &Request) -> Result<RestoreResult, String> {
    let dir = validate(req)?;
    std::fs::create_dir_all(&req.target).map_err(|e| format!("No se pudo crear la carpeta donde restaurar: {e}"))?;

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
                    files_done: s.files_restored,
                    total_files: s.total_files,
                    bytes_done: s.bytes_restored,
                    total_bytes: s.total_bytes,
                    seconds_remaining: s.seconds_remaining,
                },
            );
        }
    };

    let out = jobs::run(jobs, &job_key(&repo.id), access, &restore_args(req, &dir), on_status, on_error)?;
    if out.cancelled {
        return Err("Restauración cancelada.".into());
    }
    // Con errores en archivos concretos restic termina con código 1, pero el
    // resto se restauró: se devuelve el resultado con la lista de errores.
    let partial = out.summary.is_some() && out.error_count > 0;
    if let (Some(msg), false) = (out.failure(&[0]), partial) {
        return Err(msg);
    }
    Ok(RestoreResult {
        summary: out.summary.and_then(|v| serde_json::from_value(v).ok()),
        errors: out.errors,
        error_count: out.error_count,
        target: req.target.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(names: &[&str], overwrite: bool) -> Request {
        Request {
            snapshot: "da37898f".into(),
            dir: "/C/Users/Ana/Documentos".into(),
            names: names.iter().map(|s| s.to_string()).collect(),
            target: if cfg!(windows) { r"C:\restaurado".into() } else { "/tmp/restaurado".into() },
            overwrite,
        }
    }

    #[test]
    fn argumentos_de_restauracion() {
        let r = req(&["informe.docx", "Fotos"], false);
        let dir = validate(&r).unwrap();
        let args = restore_args(&r, &dir);
        assert_eq!(&args[4..6], ["--overwrite", "never"]);
        assert!(args.windows(2).any(|w| w == ["--include", "/informe.docx"]));
        assert!(args.windows(2).any(|w| w == ["--include", "/Fotos"]));
        assert_eq!(args[args.len() - 2..], ["--", "da37898f:/C/Users/Ana/Documentos"]);
        assert_eq!(restore_args(&req(&[], true), &dir)[5], "always");
    }

    #[test]
    fn rechaza_entradas_peligrosas() {
        let mut r = req(&["../../Windows"], false);
        assert!(validate(&r).is_err());
        r = req(&["a/b"], false);
        assert!(validate(&r).is_err());
        r = req(&["ok"], false);
        r.snapshot = "--help".into();
        assert!(validate(&r).is_err());
        r = req(&["ok"], false);
        r.target = "relativa".into();
        assert!(validate(&r).is_err());
    }

    #[test]
    fn comodines_se_neutralizan() {
        assert_eq!(include_pattern("foto[1].jpg"), "/foto?1?.jpg");
        assert_eq!(include_pattern("*.txt"), "/?.txt");
    }

    /// Restauración real de un archivo del repo de prueba.
    #[test]
    fn restauracion_real() {
        let _real = crate::restic::tests::real_repo_lock();
        let (Ok(location), Ok(pw), Ok(data)) =
            (std::env::var("RESGUARDO_TEST_REPO"), std::env::var("RESGUARDO_TEST_PASSWORD"), std::env::var("RESGUARDO_TEST_DATA"))
        else {
            return;
        };
        let access = Access::new(location.clone(), pw);
        let snap = restic::snapshots(&access).unwrap().into_iter().max_by(|a, b| a.time.cmp(&b.time)).unwrap();
        let dir = restic::tests::to_snapshot_path(&data);
        let first = restic::list_dir(&access, &snap.short_id, &dir).unwrap().into_iter().find(|e| e.kind == "file").unwrap();
        let target = std::env::temp_dir().join(format!("resguardo-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&target);

        let repo = Repo {
            id: "t".into(),
            name: "t".into(),
            location,
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
        };
        let request =
            Request { snapshot: snap.short_id.clone(), dir, names: vec![first.name.clone()], target: target.to_string_lossy().into(), overwrite: false };
        let app = tauri::test::mock_app();
        let jobs = Jobs::default();
        let result = run(app.handle(), &jobs, &repo, &access, &request).expect("debería restaurar");
        assert_eq!(result.summary.unwrap().files_restored, 1);
        assert!(target.join(&first.name).is_file());
        assert_eq!(std::fs::read_dir(&target).unwrap().count(), 1, "solo el archivo elegido");
        let _ = std::fs::remove_dir_all(&target);
    }
}
