//! Trabajos largos de restic (copias, restauraciones) con salida `--json`.
//!
//! restic escribe una línea JSON por mensaje. Por stdout llegan `status`
//! (progreso) y `summary` (resultado); por stderr, `error` (archivos
//! concretos que fallaron) y `exit_error` (error fatal).

use crate::restic::{self, Access};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::Child;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Máximo de actualizaciones de progreso por segundo hacia la interfaz.
const STATUS_INTERVAL: Duration = Duration::from_millis(100);

/// Máximo de errores por archivo que se guardan para el resumen final.
pub const MAX_ERRORS: usize = 200;

struct Running {
    child: Child,
    /// Lo preparado para este proceso (una nube por rclone): se suelta al terminar.
    _vuelta: Option<restic::Preparado>,
    cancelled: bool,
}

/// Trabajos en curso. La clave identifica el trabajo (por ejemplo, el id del
/// repositorio para las copias), de modo que no se lancen dos iguales a la vez.
#[derive(Default)]
pub struct Jobs(Mutex<HashMap<String, Running>>);

impl Jobs {
    pub fn cancel(&self, key: &str) -> Result<(), String> {
        let mut map = self.0.lock().unwrap();
        if let Some(run) = map.get_mut(key) {
            run.cancelled = true;
            restic::kill_tree(&mut run.child);
        }
        Ok(())
    }

    /// Claves de los trabajos en curso.
    pub fn keys(&self) -> Vec<String> {
        self.0.lock().unwrap().keys().cloned().collect()
    }

    /// ¿Hay algún trabajo cuya clave empiece por este id de repositorio?
    pub fn any_for_repo(&self, repo_id: &str) -> bool {
        self.0.lock().unwrap().keys().any(|k| k == repo_id || k.starts_with(&format!("{repo_id}:")))
    }

    /// Detiene todo (al cerrar la app).
    pub fn cancel_all(&self) {
        for run in self.0.lock().unwrap().values_mut() {
            run.cancelled = true;
            restic::kill_tree(&mut run.child);
        }
    }

    /// ¿No queda ningún proceso registrado? (lo usan las pruebas de la app).
    pub fn is_empty(&self) -> bool {
        self.0.lock().unwrap().is_empty()
    }
}

pub struct JobOutput {
    pub summary: Option<Value>,
    pub errors: Vec<String>,
    pub error_count: usize,
    pub exit_code: Option<i32>,
    /// stderr que no era un error por archivo (para el mensaje de fallo).
    pub other_stderr: String,
    pub cancelled: bool,
}

impl JobOutput {
    /// Mensaje de error si restic no terminó bien (códigos distintos de `ok_codes`).
    pub fn failure(&self, ok_codes: &[i32]) -> Option<String> {
        if self.cancelled {
            return Some("Operación cancelada.".into());
        }
        match self.exit_code {
            Some(c) if ok_codes.contains(&c) => None,
            code => Some(restic::exit_error(code, &self.other_stderr)),
        }
    }
}

/// Salida de un trabajo leído por trozos (`run_stream`).
pub struct StreamOutput {
    pub exit_code: Option<i32>,
    pub stderr: String,
    pub cancelled: bool,
}

/// Ejecuta restic y pasa su stdout a `on_chunk` a medida que llega (para
/// salidas que no van por líneas, como el JSON de `find`). Si `on_chunk`
/// devuelve `false`, se detiene restic (por ejemplo, al llegar a un límite).
pub fn run_stream(jobs: &Jobs, key: &str, access: &Access, args: &[String], mut on_chunk: impl FnMut(&[u8]) -> bool) -> Result<StreamOutput, String> {
    use std::io::Read;
    let (mut stdout, stderr) = {
        let mut map = jobs.0.lock().unwrap();
        if map.contains_key(key) {
            return Err("Ya hay una operación igual en curso para este repositorio.".into());
        }
        let (mut child, vuelta) = restic::spawn_vuelta(access, args)?;
        let pipes = (child.stdout.take().unwrap(), child.stderr.take().unwrap());
        map.insert(key.to_string(), Running { child, cancelled: false, _vuelta: vuelta });
        pipes
    };
    let stderr_thread = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = BufReader::new(stderr).read_to_string(&mut s);
        s
    });

    let mut buf = [0u8; 16 * 1024];
    loop {
        match stdout.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if !on_chunk(&buf[..n]) {
                    if let Some(run) = jobs.0.lock().unwrap().get_mut(key) {
                        restic::kill_tree(&mut run.child);
                    }
                    break;
                }
            }
        }
    }
    drop(stdout);

    let Running { mut child, cancelled, _vuelta } = jobs.0.lock().unwrap().remove(key).expect("el trabajo se registró al empezar");
    let status = child.wait().map_err(|e| e.to_string())?;
    let stderr = stderr_thread.join().unwrap_or_default();
    Ok(StreamOutput { exit_code: status.code(), stderr, cancelled })
}

pub fn describe_error(v: &Value) -> String {
    let message = v["error"]["message"].as_str().or_else(|| v["error"].as_str()).unwrap_or("error desconocido");
    match v["item"].as_str() {
        Some(item) if !item.is_empty() => format!("{item}: {message}"),
        _ => message.to_string(),
    }
}

/// Ejecuta restic y bloquea hasta que termina. `on_status` recibe los
/// mensajes `status` (limitados en frecuencia) y `on_error` cada error por
/// archivo en cuanto llega.
pub fn run(
    jobs: &Jobs,
    key: &str,
    access: &Access,
    args: &[String],
    mut on_status: impl FnMut(Value),
    on_error: impl Fn(String) + Send + 'static,
) -> Result<JobOutput, String> {
    let (stdout, stderr) = {
        let mut map = jobs.0.lock().unwrap();
        if map.contains_key(key) {
            return Err(
                "Ya hay una operación en curso en este repositorio (una copia, una restauración o una búsqueda). Espera a que termine y vuelve a intentarlo."
                    .into(),
            );
        }
        let (mut child, vuelta) = restic::spawn_vuelta(access, args)?;
        let pipes = (child.stdout.take().unwrap(), child.stderr.take().unwrap());
        map.insert(key.to_string(), Running { child, cancelled: false, _vuelta: vuelta });
        pipes
    };

    // stderr se lee en otro hilo para que restic nunca se bloquee escribiendo.
    let stderr_thread = std::thread::spawn(move || {
        let mut errors = Vec::new();
        let mut error_count = 0;
        let mut other = String::new();
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            match serde_json::from_str::<Value>(&line) {
                Ok(v) if v["message_type"] == "error" => {
                    let message = describe_error(&v);
                    on_error(message.clone());
                    error_count += 1;
                    if errors.len() < MAX_ERRORS {
                        errors.push(message);
                    }
                }
                Ok(v) if v["message_type"] == "exit_error" => {
                    other.push_str(v["message"].as_str().unwrap_or_default());
                    other.push('\n');
                }
                _ => {
                    other.push_str(&line);
                    other.push('\n');
                }
            }
        }
        (errors, error_count, other)
    });

    let mut summary = None;
    let mut last_status: Option<Instant> = None;
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
        match v["message_type"].as_str() {
            Some("status") => {
                if last_status.is_some_and(|t| t.elapsed() < STATUS_INTERVAL) {
                    continue;
                }
                last_status = Some(Instant::now());
                on_status(v);
            }
            Some("summary") => summary = Some(v),
            _ => {}
        }
    }

    let Running { mut child, cancelled, _vuelta } = jobs.0.lock().unwrap().remove(key).expect("el trabajo se registró al empezar");
    let status = child.wait().map_err(|e| e.to_string())?;
    let (errors, error_count, other_stderr) = stderr_thread.join().unwrap_or_default();

    Ok(JobOutput { summary, errors, error_count, exit_code: status.code(), other_stderr, cancelled })
}
