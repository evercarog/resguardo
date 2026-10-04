//! Ejecución del binario oficial de restic.
//!
//! Reglas de seguridad:
//! - Nunca se usa una shell: los argumentos se pasan como lista.
//! - La contraseña, la ubicación del repositorio y las credenciales del
//!   servidor REST viajan por variables de entorno del proceso hijo, nunca
//!   como argumentos (se verían en la lista de procesos).
//! - Se eliminan variables heredadas que podrían redirigir la contraseña o
//!   el repositorio a otro origen.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, ErrorKind, Read};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Todo lo necesario para abrir un repositorio.
#[derive(Clone)]
pub struct Access {
    pub location: String,
    pub password: String,
    /// Usuario y contraseña HTTP de un rest-server (restic 0.17+).
    pub rest_auth: Option<(String, String)>,
    /// Certificado de CA propio para servidores HTTPS (`--cacert`).
    pub cacert: Option<String>,
    /// Variables extra del proceso (credenciales de S3, etc.). Nunca van
    /// como argumentos, que se ven en la lista de procesos.
    pub env: Vec<(String, String)>,
}

impl Access {
    pub fn new(location: impl Into<String>, password: impl Into<String>) -> Self {
        Self { location: location.into(), password: password.into(), rest_auth: None, cacert: None, env: Vec::new() }
    }
}

/// El restic que se usa: el que va incluido junto a Resguardo (instalador) o,
/// si no está (p. ej. en los tests), el del PATH. Usar primero el incluido
/// evita depender de lo que haya instalado y que otro programa llamado
/// "restic" en el PATH se haga pasar por él.
pub fn program() -> std::path::PathBuf {
    let name = if cfg!(windows) { "restic.exe" } else { "restic" };
    let bundled = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|dir| dir.join(name)));
    match bundled {
        Some(p) if p.is_file() => p,
        // Linux: el restic del paquete de la distribución (/usr/bin es de root),
        // si no va uno junto al agente.
        Some(_) if cfg!(unix) && BUNDLED_ONLY.load(Ordering::Relaxed) && std::path::Path::new("/usr/bin/restic").is_file() => "/usr/bin/restic".into(),
        // El agente (SYSTEM) nunca busca restic en el PATH: una carpeta del
        // PATH editable por usuarios le haría ejecutar otro programa.
        Some(p) if BUNDLED_ONLY.load(Ordering::Relaxed) => p,
        _ => std::path::PathBuf::from("restic"),
    }
}

/// «Modo discreto» (lo decide el agente antes de cada copia o tarea): restic
/// con prioridad baja y, en destinos remotos, la subida limitada (KiB/s; 0: sin límite).
static DISCREET: AtomicBool = AtomicBool::new(false);
static UPLOAD_LIMIT_KIB: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub fn set_discreet(on: bool, upload_kib: Option<u32>) {
    DISCREET.store(on, Ordering::Relaxed);
    UPLOAD_LIMIT_KIB.store(if on { upload_kib.unwrap_or(0) } else { 0 }, Ordering::Relaxed);
}

fn discreet() -> bool {
    DISCREET.load(Ordering::Relaxed)
}

/// Lanza restic y, en modo discreto, le baja la prioridad de E/S y memoria.
fn spawn_cmd(cmd: &mut Command) -> Result<Child, String> {
    let child = cmd.spawn().map_err(spawn_error)?;
    if discreet() {
        crate::proceso::after_spawn(&child);
    }
    Ok(child)
}

/// Activado por el agente: solo se usa el restic incluido junto a Resguardo.
static BUNDLED_ONLY: AtomicBool = AtomicBool::new(false);

pub fn require_bundled() {
    BUNDLED_ONLY.store(!cfg!(any(test, debug_assertions)), Ordering::Relaxed);
}

fn base_command() -> Command {
    let mut cmd = Command::new(program());
    // Nada del entorno que cambie el repositorio, las credenciales o el TLS:
    // lo que restic necesita lo pone `repo_command` (ver `proceso::entorno_minimo`).
    crate::proceso::entorno_minimo(&mut cmd);
    cmd.stdin(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    if discreet() {
        crate::proceso::lower(&mut cmd);
    }
    cmd
}

/// Termina restic y todos sus procesos hijos.
///
/// En Windows restic suele instalarse con un lanzador (Chocolatey, Scoop) que
/// arranca el ejecutable real como proceso hijo. Terminar solo el lanzador
/// dejaría el restic real funcionando (y a la app esperando su salida).
pub fn kill_tree(child: &mut Child) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new(crate::proceso::system_tool("taskkill.exe"))
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000)
            .status();
    }
    let _ = child.kill();
}

fn spawn_error(e: std::io::Error) -> String {
    if e.kind() == ErrorKind::NotFound {
        if !BUNDLED_ONLY.load(Ordering::Relaxed) {
            "No se encontró el ejecutable de restic. Instálalo y asegúrate de que esté en el PATH.".into()
        } else if cfg!(windows) {
            "No se encontró restic junto a Resguardo. Vuelve a instalar Resguardo para recuperarlo.".into()
        } else {
            "No se encontró restic junto al agente ni en /usr/bin/restic. Vuelve a instalar Resguardo Agente \
             (su paquete trae el restic oficial)."
                .into()
        }
    } else {
        format!("No se pudo ejecutar restic: {e}")
    }
}

fn repo_command(access: &Access) -> Command {
    let mut cmd = base_command();
    cmd.env("RESTIC_REPOSITORY", &access.location).env("RESTIC_PASSWORD", &access.password);
    if let Some((user, pass)) = &access.rest_auth {
        cmd.env("RESTIC_REST_USERNAME", user).env("RESTIC_REST_PASSWORD", pass);
    }
    for (k, v) in &access.env {
        cmd.env(k, v);
    }
    // Opción global: puede ir antes del subcomando.
    if let Some(cacert) = &access.cacert {
        cmd.arg("--cacert").arg(cacert);
    }
    // Modo discreto: subida limitada a destinos remotos.
    let limit = UPLOAD_LIMIT_KIB.load(Ordering::Relaxed);
    if limit > 0 && crate::proceso::is_remote(&access.location) {
        cmd.arg("--limit-upload").arg(limit.to_string());
    }
    cmd
}

/// Explica en español los errores de conexión más comunes, si los reconoce.
fn explain(stderr: &str) -> Option<String> {
    let s = stderr.to_lowercase();
    // Un bloqueo sin renovar desde hace más de media hora: la operación que lo
    // puso ya no está en marcha (restic lo renueva cada pocos minutos).
    if s.contains("already locked") {
        if let Some(age) = stale_lock_age(stderr) {
            return Some(format!(
                "{STALE_LOCK} (de hace {age}), de una operación que se cortó. Si no hay nada en marcha en otro equipo, quítalo con «Desbloquear»."
            ));
        }
        // De este mismo equipo y de un proceso que ya no existe: también antiguo.
        if let Some((pid, host)) = lock_owner(stderr) {
            let me = crate::proceso::hostname();
            if !me.is_empty() && host.eq_ignore_ascii_case(&me) && !crate::proceso::process_alive(pid) {
                return Some(format!("{STALE_LOCK}, de una operación de este equipo que se cortó. Quítalo con «Desbloquear»."));
            }
        }
    }
    // Límites de la cuenta en la nube (p. ej. el «Daily Storage Cap» de
    // Backblaze): se comprueba antes que el 403 y el bloqueo, porque restic
    // suele fallar al crear su archivo de bloqueo.
    let msg = if s.contains("cap exceeded")
        || s.contains("cap_exceeded")
        || s.contains("quotaexceeded")
        || s.contains("quota exceeded")
        || s.contains("storage limit")
    {
        "El servicio en la nube rechazó la escritura: se alcanzó el límite de la cuenta (por ejemplo, el límite \
         diario de almacenamiento de Backblaze, en «Caps & Alerts»). Súbelo o añade un método de pago y vuelve a \
         intentarlo."
    } else if s.contains("401 unauthorized") || s.contains("(401)") {
        "El servidor rechazó el usuario o la contraseña de acceso (401). Comprueba en el servidor que ese usuario sigue \
         existiendo y tiene esa contraseña."
    } else if s.contains("403 forbidden") || s.contains("(403)") {
        "El servidor no permite el acceso a esa ruta (403). Si usa --private-repos, la ruta debe empezar por tu \
         usuario: http://servidor:8000/USUARIO/"
    } else if s.contains("404 not found") || s.contains("(404)") {
        "No hay ningún repositorio en esa ruta del servidor (404). Revisa la URL."
    } else if s.contains("x509:") || s.contains("certificate") {
        "Problema con el certificado HTTPS del servidor. Si usa un certificado propio, indica su CA en Opciones \
         avanzadas."
    } else if s.contains("server gave http response to https client") {
        "El servidor no usa HTTPS: cambia la URL a http://"
    } else if s.contains("connection refused")
        || s.contains("actively refused")
        || s.contains("no such host")
        || s.contains("i/o timeout")
        || s.contains("no route to host")
    {
        "No se pudo conectar con el servidor. Revisa la dirección, el puerto y que esté encendido."
    } else if s.contains("no space left on device") || s.contains("not enough space") || s.contains("disk is full") {
        "No queda espacio en el disco del destino. Libera espacio en él o borra versiones antiguas (pestaña «Retención» del \
         repositorio) y vuelve a copiar."
    } else if s.contains("repository is already locked") || s.contains("unable to create lock") {
        BUSY
    } else if s.contains("unknown flag") || s.contains("unknown shorthand flag") {
        "La versión de restic de este equipo es demasiado antigua para Resguardo (hace falta la 0.17 o posterior). \
         Reinstala Resguardo, que trae la suya, o pon el restic oficial junto al agente."
    } else if s.contains("wrong password") || s.contains("no key found") {
        "Contraseña incorrecta."
    } else if (s.contains("unable to open config file") || s.contains("is there a repository at")) && !s.contains("://") {
        "No se encuentra el repositorio en esa carpeta. ¿Está conectado el disco? Conéctalo (con la misma letra de unidad) y \
         vuelve a intentarlo."
    } else if cfg!(windows) && (s.contains("access is denied") || s.contains("acceso denegado") || s.contains("permission denied")) {
        "Windows no permite acceder a algún archivo o carpeta. Si son archivos del sistema o de otro usuario, abre Resguardo como administrador."
    } else if s.contains("permission denied") || s.contains("operation not permitted") {
        "El sistema no permite acceder a algún archivo o carpeta. El agente corre como root: revisa los permisos y, en un \
         contenedor sin privilegios, a quién pertenecen los archivos montados."
    } else if s.contains("cannot find the path") || s.contains("the system cannot find") || s.contains("no such file or directory") {
        "No se encuentra alguna de las carpetas o el disco. Revisa que siga existiendo y esté conectado; si la moviste o la \
         renombraste, edita la copia para que apunte a la nueva."
    } else if s.contains("context deadline exceeded") || s.contains("timeout") {
        "El servidor tardó demasiado en responder. Revisa la conexión e inténtalo de nuevo."
    } else {
        return None;
    };
    Some(msg.to_string())
}

/// Última línea útil de stderr (para dar contexto sin volcar todo).
fn last_line(stderr: &str) -> Option<&str> {
    stderr.lines().map(str::trim).rfind(|l| !l.is_empty())
}

/// La línea más informativa de stderr: la de "Fatal:" si la hay; si no, la última.
fn key_line(stderr: &str) -> Option<&str> {
    stderr.lines().map(str::trim).find(|l| l.starts_with("Fatal:")).map(|l| l.trim_start_matches("Fatal:").trim()).or_else(|| last_line(stderr))
}

/// Principio del mensaje de un bloqueo antiguo: la interfaz ofrece «Desbloquear».
pub const STALE_LOCK: &str = "El repositorio tiene un bloqueo antiguo";

/// Quién tiene el bloqueo: «… locked by PID 24020 on EQUIPO by …» → (24020, "EQUIPO").
fn lock_owner(stderr: &str) -> Option<(u32, String)> {
    let rest = stderr.split_once("locked by PID ")?.1;
    let (pid, rest) = rest.split_once(" on ")?;
    let host = rest.split_once(" by ").map_or(rest, |(h, _)| h).lines().next()?.trim();
    Some((pid.trim().parse().ok()?, host.to_string()))
}

/// La antigüedad del bloqueo («lock was created at … (2h5m3s ago)») si es de
/// hace más de 30 minutos, en palabras («2 h 5 min»).
fn stale_lock_age(stderr: &str) -> Option<String> {
    let line = stderr.lines().find(|l| l.contains("lock was created at"))?;
    let inner = line.rsplit_once('(')?.1.split_once(" ago)")?.0;
    let (mut hours, mut minutes) = (0u64, 0u64);
    let mut num = String::new();
    for c in inner.chars() {
        if c.is_ascii_digit() || c == '.' {
            num.push(c);
            continue;
        }
        let n = num.split('.').next().and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
        num.clear();
        match c {
            'h' => hours = hours.saturating_add(n),
            'm' => minutes = minutes.saturating_add(n),
            _ => {}
        }
    }
    // «ms» (milisegundos) no cuenta como minutos: «1m2.5s» sí, «350ms» no.
    if inner.ends_with("ms") && !inner.contains('h') && inner.matches('m').count() == 1 {
        minutes = 0;
    }
    let total = hours.saturating_mul(60).saturating_add(minutes);
    if total < 30 {
        return None;
    }
    Some(if total >= 48 * 60 {
        format!("{} días", total / (24 * 60))
    } else if hours > 0 {
        format!("{} h {} min", total / 60, total % 60)
    } else {
        format!("{total} min")
    })
}

const BUSY: &str = "El repositorio está ocupado por otra operación (por ejemplo, una copia automática o desde otro equipo). \
                    Espera a que termine y vuelve a intentarlo.";

/// restic terminó sin código de salida: lo cortó una señal (otro programa, el
/// sistema sin memoria o el apagado del equipo).
pub const KILLED: &str = "restic se detuvo de golpe (lo cerró el sistema, otro programa o el apagado del equipo). \
                          Vuelve a intentarlo; si se repite, revisa la memoria libre del equipo.";

/// Código 10 de restic: no hay repositorio en esa ubicación.
const NOT_FOUND: &str = "No hay copias de Resguardo en esa ubicación. ¿Está conectado el disco o encendido el servidor? \
                         Si moviste el repositorio, quítalo y vuelve a conectarlo en su nueva ubicación.";

/// Traduce un código de salida de restic (0.17+) a un mensaje para el usuario.
pub fn exit_error(code: Option<i32>, stderr: &str) -> String {
    redact_credentials(&exit_error_text(code, stderr))
}

/// Quita la contraseña de las direcciones con usuario (`https://ana:clave@host`
/// → `https://ana:***@host`). restic ya las oculta, pero sus mensajes van al
/// estado del agente (legible por todos los usuarios), al registro y a la web.
pub fn redact_credentials(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("://") {
        let (head, tail) = rest.split_at(i + 3);
        out.push_str(head);
        let end = tail.find(|c: char| c == '/' || c.is_whitespace() || matches!(c, '"' | '\'' | '»' | ')')).unwrap_or(tail.len());
        let authority = &tail[..end];
        match (authority.rfind('@'), authority.find(':')) {
            (Some(at), Some(colon)) if colon < at => {
                out.push_str(&authority[..colon]);
                out.push_str(":***");
                out.push_str(&authority[at..]);
            }
            _ => out.push_str(authority),
        }
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

fn exit_error_text(code: Option<i32>, stderr: &str) -> String {
    match code {
        Some(10) => NOT_FOUND.to_string(),
        Some(11) => explain(stderr).filter(|m| !m.contains("ocupado")).unwrap_or_else(|| BUSY.to_string()),
        Some(12) => "Contraseña incorrecta.".to_string(),
        _ => explain(stderr).unwrap_or_else(|| {
            if let Some(line) = key_line(stderr) {
                format!("restic: {line}")
            } else if let Some(c) = code {
                format!("restic terminó con un error (código {c}). Vuelve a intentarlo; si se repite, mira el registro.")
            } else {
                KILLED.to_string()
            }
        }),
    }
}

/// Límite para comprobar una conexión (añadir un repositorio).
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(40);
/// Límite para consultas normales (snapshots, listar carpetas).
pub const QUERY_TIMEOUT: Duration = Duration::from_secs(120);

pub const CANCELLED: &str = "Operación cancelada.";

/// Ejecuta restic contra un repositorio y devuelve la salida estándar.
pub fn run(access: &Access, args: &[&str]) -> Result<Vec<u8>, String> {
    run_with(access, args, QUERY_TIMEOUT, None)
}

/// Como `run`, con límite de tiempo y cancelación. restic reintenta en
/// silencio durante minutos cuando el servidor responde con errores (por
/// ejemplo, una ruta equivocada), así que nunca se le espera sin límite.
pub fn run_with(access: &Access, args: &[&str], timeout: Duration, cancel: Option<&AtomicBool>) -> Result<Vec<u8>, String> {
    let out = run_raw_with(access, args, timeout, cancel)?;
    if out.code == Some(0) {
        Ok(out.stdout)
    } else {
        Err(exit_error(out.code, &out.stderr))
    }
}

/// Salida completa de restic, con su código de salida.
pub struct RawOutput {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

/// Ejecuta restic con límite de tiempo y devuelve la salida aunque el código
/// no sea 0 (p. ej. 3 = copia hecha pero faltaron archivos), entregando
/// cada línea de la salida a `on_line` en cuanto
/// llega (para mostrar progreso). Las líneas de estado de `--json` no se
/// acumulan en `stdout`: en una copia larga serían miles.
pub fn run_raw_lines(access: &Access, args: &[&str], timeout: Duration, on_line: &mut dyn FnMut(&str)) -> Result<RawOutput, String> {
    run_raw_inner(access, args, timeout, None, Some(on_line))
}

/// Ejecuta restic con límite de tiempo y devuelve la salida aunque el código
/// no sea 0. Solo es `Err` si no se pudo lanzar o se agotó el tiempo.
pub fn run_raw(access: &Access, args: &[&str], timeout: Duration) -> Result<RawOutput, String> {
    run_raw_with(access, args, timeout, None)
}

fn run_raw_with(access: &Access, args: &[&str], timeout: Duration, cancel: Option<&AtomicBool>) -> Result<RawOutput, String> {
    run_raw_inner(access, args, timeout, cancel, None)
}

/// Guarda una línea de la salida (y se la pasa a `on_line`, si hay).
fn take_line(line: String, out: &mut Vec<u8>, on_line: &mut Option<&mut dyn FnMut(&str)>) {
    if let Some(f) = on_line.as_mut() {
        f(&line);
        // Progreso de `--json` y entradas de `ls --json`: ya las procesó
        // `on_line`; no se acumulan (en una copia o versión grande son miles).
        if line.contains(r#""message_type":"status""#) || line.contains(r#""struct_type":"node""#) {
            return;
        }
    }
    out.extend_from_slice(line.as_bytes());
    out.push(b'\n');
}

thread_local! {
    /// El restic que está lanzando este hilo (para medir su lectura y escritura).
    static PID_EN_MARCHA: std::cell::Cell<Option<u32>> = const { std::cell::Cell::new(None) };
}

/// El proceso de restic que corre ahora en este hilo, si hay uno (dentro de
/// `on_line` de [`run_raw_lines`]: el de esa misma ejecución).
pub fn pid_en_marcha() -> Option<u32> {
    PID_EN_MARCHA.with(|p| p.get())
}

fn run_raw_inner(
    access: &Access,
    args: &[&str],
    timeout: Duration,
    cancel: Option<&AtomicBool>,
    on_line: Option<&mut dyn FnMut(&str)>,
) -> Result<RawOutput, String> {
    let mut child = spawn_cmd(repo_command(access).args(args).stdout(Stdio::piped()).stderr(Stdio::piped()))?;
    PID_EN_MARCHA.with(|p| p.set(Some(child.id())));
    let r = run_child(&mut child, timeout, cancel, on_line);
    PID_EN_MARCHA.with(|p| p.set(None));
    r
}

fn run_child(child: &mut Child, timeout: Duration, cancel: Option<&AtomicBool>, mut on_line: Option<&mut dyn FnMut(&str)>) -> Result<RawOutput, String> {
    // Leer en hilos aparte para que restic nunca se bloquee escribiendo.
    let stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let out_thread = std::thread::spawn(move || {
        for line in BufReader::new(stdout).split(b'\n').map_while(Result::ok) {
            let line = String::from_utf8_lossy(&line).trim_end_matches('\r').to_string();
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut out: Vec<u8> = Vec::new();
    let err_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).into_owned()
    });

    let started = Instant::now();
    let outcome = loop {
        while let Ok(line) = rx.try_recv() {
            take_line(line, &mut out, &mut on_line);
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => {}
            Err(e) => break Err(format!("No se pudo esperar a restic: {e}")),
        }
        if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            break Err(CANCELLED.to_string());
        }
        if started.elapsed() >= timeout {
            break Err(String::new()); // se completa abajo con el contexto de stderr
        }
        std::thread::sleep(Duration::from_millis(50));
    };

    if outcome.is_err() {
        kill_tree(child);
        let _ = child.wait();
    }
    let _ = out_thread.join();
    while let Ok(line) = rx.try_recv() {
        take_line(line, &mut out, &mut on_line);
    }
    let err = err_thread.join().unwrap_or_default();

    match outcome {
        Ok(status) => Ok(RawOutput { code: status.code(), stdout: out, stderr: err }),
        Err(msg) if msg.is_empty() => {
            let secs = timeout.as_secs();
            Err(match (explain(&err), last_line(&err)) {
                (Some(why), _) => format!("{why}\n\n(restic seguía reintentando tras {secs} s y se detuvo.)"),
                (None, Some(line)) => format!("No hubo respuesta en {secs} s. Último mensaje de restic:\n{line}"),
                (None, None) => format!("No hubo respuesta en {secs} s. Revisa la ubicación, el usuario y la contraseña del servidor."),
            })
        }
        Err(msg) => Err(msg),
    }
}

/// Lanza restic con stdout y stderr conectados para leerlos mientras se ejecuta.
pub fn spawn(access: &Access, args: &[String]) -> Result<Child, String> {
    spawn_cmd(repo_command(access).args(args).stdout(Stdio::piped()).stderr(Stdio::piped()))
}

/// Opción de exclusión: en Windows las rutas no distinguen mayúsculas, así
/// que `*.TMP` debe excluir también `archivo.tmp` (`--iexclude`).
pub fn exclude_flag() -> &'static str {
    if cfg!(windows) {
        "--iexclude"
    } else {
        "--exclude"
    }
}

pub fn version() -> Result<String, String> {
    let output = base_command().arg("version").output().map_err(spawn_error)?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Resumen que restic (0.17+) guarda en cada snapshot.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SnapshotSummary {
    #[serde(default)]
    pub backup_start: Option<String>,
    #[serde(default)]
    pub backup_end: Option<String>,
    #[serde(default)]
    pub files_new: Option<u64>,
    #[serde(default)]
    pub files_changed: Option<u64>,
    #[serde(default)]
    pub files_unmodified: Option<u64>,
    #[serde(default)]
    pub dirs_new: Option<u64>,
    #[serde(default)]
    pub dirs_changed: Option<u64>,
    /// Datos nuevos que añadió este snapshot, antes de comprimir.
    #[serde(default)]
    pub data_added: Option<u64>,
    /// Espacio real que ocupó en disco (comprimido y deduplicado).
    #[serde(default)]
    pub data_added_packed: Option<u64>,
    #[serde(default)]
    pub total_files_processed: Option<u64>,
    #[serde(default)]
    pub total_bytes_processed: Option<u64>,
}

/// Subconjunto de los campos de `restic snapshots --json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub short_id: String,
    pub time: String,
    pub hostname: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub excludes: Vec<String>,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub program_version: Option<String>,
    #[serde(default)]
    pub summary: Option<SnapshotSummary>,
    /// En una copia hecha con `restic copy`: id de la copia de origen.
    #[serde(default)]
    pub original: Option<String>,
}

/// Espacio real del repositorio (`restic stats --mode raw-data`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoStats {
    /// Bytes que ocupa en disco (comprimido y deduplicado).
    #[serde(default)]
    pub total_size: u64,
    /// Lo mismo sin comprimir.
    #[serde(default)]
    pub total_uncompressed_size: u64,
    #[serde(default)]
    pub compression_ratio: f64,
    /// Porcentaje ahorrado por la compresión.
    #[serde(default)]
    pub compression_space_saving: f64,
    #[serde(default)]
    pub total_blob_count: u64,
    #[serde(default)]
    pub snapshots_count: u64,
}

/// Las estadísticas recorren todo el repositorio: en uno grande y remoto tardan.
pub const STATS_TIMEOUT: Duration = Duration::from_secs(15 * 60);

pub fn stats(access: &Access) -> Result<RepoStats, String> {
    parse_stats(&run_with(access, &["stats", "--mode", "raw-data", "--json", "--no-lock"], STATS_TIMEOUT, None)?)
}

/// La salida de `restic stats --json` (el JSON puede venir tras líneas de texto).
pub fn parse_stats(out: &[u8]) -> Result<RepoStats, String> {
    let text = String::from_utf8_lossy(out);
    let json = text.lines().find(|l| l.trim_start().starts_with('{')).unwrap_or("{}");
    serde_json::from_str(json).map_err(|e| format!("Respuesta inesperada de restic: {e}"))
}

pub fn snapshots(access: &Access) -> Result<Vec<Snapshot>, String> {
    parse_snapshots(&run(access, &["snapshots", "--json", "--no-lock"])?)
}

/// La salida de `restic snapshots --json`.
pub fn parse_snapshots(out: &[u8]) -> Result<Vec<Snapshot>, String> {
    serde_json::from_slice(out).map_err(|e| format!("Respuesta inesperada de restic: {e}"))
}

/// Un elemento de un snapshot (`restic ls --json`).
#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    /// "dir", "file", "symlink"…
    #[serde(rename = "type")]
    pub kind: String,
    /// Ruta dentro del snapshot, con `/` (en Windows: `/C/Users/...`).
    pub path: String,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub mtime: Option<String>,
}

/// Un id de snapshot válido: solo hexadecimal, así nunca se confunde con una opción.
pub fn valid_snapshot_id(id: &str) -> bool {
    (8..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Normaliza una carpeta dentro del snapshot: absoluta, con `/` y sin `..`.
pub fn snapshot_dir(dir: &str) -> Result<String, String> {
    let trimmed = dir.trim_end_matches('/');
    let dir = if trimmed.is_empty() { "/" } else { trimmed };
    if !dir.starts_with('/') || dir.contains('\0') || dir.split('/').any(|s| s == "..") {
        return Err(format!("Ruta no válida dentro de la versión: {dir}"));
    }
    Ok(dir.to_string())
}

/// Mensaje de una copia correcta sin cambios (con `--skip-if-unchanged`).
pub const UNCHANGED_MESSAGE: &str = "Sin cambios desde la última versión: no se guardó una nueva.";

/// ¿Es el resumen (`"message_type":"summary"`) de una copia sin cambios? Con
/// `--skip-if-unchanged`, si nada cambió restic termina con código 0, envía
/// el resumen de siempre y no crea versión: falta la clave `snapshot_id`.
pub fn summary_unchanged(summary: &serde_json::Value) -> bool {
    summary["message_type"] == "summary" && summary.get("snapshot_id").is_none_or(|v| v.is_null())
}

/// ¿Lleva la ubicación una contraseña dentro (`rest:https://ana:clave@host/`)?
/// No se aceptan: acabaría en texto plano en la configuración.
pub fn has_embedded_password(location: &str) -> bool {
    let Some(start) = location.find("://") else { return false };
    let rest = &location[start + 3..];
    let authority = rest.split('/').next().unwrap_or_default();
    authority.rsplit_once('@').is_some_and(|(userinfo, _)| userinfo.contains(':'))
}

/// Ruta de un archivo del equipo tal como la guarda restic en una versión
/// (`C:\\Users\\Ana` pasa a `/C/Users/Ana`; en Linux y macOS queda igual).
pub fn ruta_en_version(p: &str) -> String {
    let p = p.replace('\\', "/");
    match p.split_once(":/") {
        Some((drive, rest)) if drive.len() == 1 => format!("/{drive}/{rest}"),
        _ => p,
    }
}

/// Contenido directo de una carpeta del snapshot: carpetas primero, luego por nombre.
pub fn list_dir(access: &Access, snapshot: &str, dir: &str) -> Result<Vec<Entry>, String> {
    if !valid_snapshot_id(snapshot) {
        return Err("ID de versión no válido.".into());
    }
    let dir = snapshot_dir(dir)?;
    let out = run(access, &["ls", "--json", "--no-lock", snapshot, &dir])?;
    Ok(parse_ls(&out, &dir))
}

/// Los hijos directos de `dir` (ya normalizada con [`snapshot_dir`]) en la salida
/// de `restic ls --json`: carpetas primero, luego por nombre.
pub fn parse_ls(out: &[u8], dir: &str) -> Vec<Entry> {
    let prefix = if dir == "/" { "/".to_string() } else { format!("{dir}/") };
    let mut entries: Vec<Entry> = String::from_utf8_lossy(out)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|v| v["struct_type"] == "node" || v["message_type"] == "node")
        .filter_map(|v| serde_json::from_value::<Entry>(v).ok())
        // `ls` incluye la propia carpeta; solo queremos sus hijos directos.
        .filter(|e| e.path.strip_prefix(&prefix).is_some_and(|rest| !rest.is_empty() && !rest.contains('/')))
        .collect();
    entries.sort_by(|a, b| (b.kind == "dir").cmp(&(a.kind == "dir")).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    entries
}

#[cfg(any(test, feature = "testing"))]
pub mod tests {
    #[cfg_attr(not(test), allow(unused_imports))]
    use super::*;

    #[test]
    fn modo_discreto_limita_la_subida_solo_en_remotos() {
        let args = |loc: &str| repo_command(&Access::new(loc, "x")).get_args().map(|a| a.to_string_lossy().into_owned()).collect::<Vec<_>>();
        // Mientras dura la prueba, otros tests no se ven afectados en local (el
        // límite solo se aplica a destinos remotos).
        set_discreet(true, Some(512));
        let remote = args("rest:https://servidor:8000/ana/");
        let local = args(r"D:\Copias");
        set_discreet(false, Some(512));
        let after = args("rest:https://servidor:8000/ana/");
        assert_eq!(remote, ["--limit-upload", "512"]);
        assert!(local.is_empty());
        assert!(after.is_empty(), "fuera del horario, sin límite");
    }

    /// Los tests que bloquean el repositorio de prueba compartido (copias,
    /// restauraciones, verificaciones, búsquedas) se ejecutan de uno en uno:
    /// en paralelo, restic choca al crear sus bloqueos y fallan al azar.
    pub fn real_repo_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Prueba contra un repositorio real. Se ejecuta solo si se definen
    /// RESGUARDO_TEST_REPO y RESGUARDO_TEST_PASSWORD; si no, se omite.
    #[test]
    fn lista_snapshots_de_repo_real() {
        let (Ok(repo), Ok(pw)) = (std::env::var("RESGUARDO_TEST_REPO"), std::env::var("RESGUARDO_TEST_PASSWORD")) else {
            eprintln!("omitido: define RESGUARDO_TEST_REPO y RESGUARDO_TEST_PASSWORD");
            return;
        };
        let snaps = snapshots(&Access::new(repo, pw)).expect("debería listar snapshots");
        assert!(!snaps.is_empty());
        for s in &snaps {
            assert!(!s.short_id.is_empty());
            assert!(!s.paths.is_empty());
        }
    }

    #[test]
    fn contrasena_incorrecta_da_mensaje_claro() {
        let Ok(repo) = std::env::var("RESGUARDO_TEST_REPO") else { return };
        let err = snapshots(&Access::new(repo, "contraseña-equivocada")).unwrap_err();
        assert_eq!(err, "Contraseña incorrecta.");
    }

    #[test]
    fn estadisticas_reales() {
        let (Ok(repo), Ok(pw)) = (std::env::var("RESGUARDO_TEST_REPO"), std::env::var("RESGUARDO_TEST_PASSWORD")) else {
            return;
        };
        let access = Access::new(repo, pw);
        let s = stats(&access).unwrap();
        assert!(s.total_size > 0 && s.snapshots_count > 0);
        let snap = snapshots(&access).unwrap().into_iter().next().unwrap();
        assert!(snap.summary.unwrap().backup_start.is_some(), "restic 0.17+ guarda el resumen");
    }

    #[test]
    fn limite_de_la_nube_no_es_ocupado() {
        let e = "Fatal: unable to create lock in backend: client.PutObject: Cannot upload files, storage cap exceeded.";
        assert!(exit_error(Some(11), e).contains("límite de la cuenta"));
        assert!(!exit_error(Some(11), e).contains("  "), "sin huecos de una línea partida");
        assert!(exit_error(Some(11), "Fatal: repository is already locked by PID 1").contains("ocupado"));
        assert!(exit_error(Some(11), "").contains("ocupado"));
    }

    #[test]
    fn bloqueo_antiguo() {
        let old = "repo already locked, waiting up to 0s for the lock\nunable to create lock in backend: repository is already locked by PID 4242 on PC by SYSTEM (UID 0, GID 0)\nlock was created at 2026-09-30 10:00:00 (26h3m10.5s ago)\nstorage ID 1a2b3c4d\nthe `unlock` command can be used to remove stale locks";
        let m = exit_error(Some(11), old);
        assert!(m.starts_with(STALE_LOCK) && m.contains("26 h 3 min") && m.contains("«Desbloquear»"), "{m}");
        let fresh = old.replace("26h3m10.5s ago", "4m2s ago");
        assert!(exit_error(Some(11), &fresh).contains("ocupado"));
        let ms = old.replace("26h3m10.5s ago", "350ms ago");
        assert!(exit_error(Some(11), &ms).contains("ocupado"));
        assert!(exit_error(Some(11), &old.replace("26h3m10.5s ago", "75h0m0s ago")).contains("3 días"));
        assert_eq!(lock_owner(old), Some((4242, "PC".to_string())));
    }

    #[test]
    fn explica_errores_http() {
        let e = |s: &str| exit_error(Some(1), s);
        assert!(e("Fatal: unexpected HTTP response (404): 404 Not Found").contains("(404)"));
        assert!(e("unexpected HTTP response (403): 403 Forbidden").contains("--private-repos"));
        assert!(e("Stat(<config/>) failed: unexpected HTTP response (401): 401 Unauthorized").contains("(401)"));
        assert!(
            e("dial tcp 10.0.0.1:8000: connectex: No connection could be made because the target machine actively refused it.").contains("No se pudo conectar")
        );
        assert!(e("Fatal: unable to save snapshot: write D:\\x: There is not enough space on the disk.").contains("espacio"));
        assert!(e("unable to create lock in backend: repository is already locked by PID 12").contains("ocupado"));
        assert!(e("Fatal: unable to open config file: stat E:\\Backups\\config: The system cannot find the path specified.\nIs there a repository at the following location?")
            .contains("¿Está conectado el disco?"));
        // Desconocido: solo la línea importante, no todo el volcado.
        assert_eq!(e("detalle 1\nFatal: algo raro pasó\ndetalle 2"), "restic: algo raro pasó");
        // restic anterior a la 0.17 (p. ej. el de Debian 12) no conoce las opciones nuevas.
        assert!(e("unknown flag: --skip-if-unchanged").contains("0.17"));
        // Sin código de salida (lo mató una señal) y sin nada en stderr.
        assert_eq!(exit_error(None, ""), KILLED);
        assert!(exit_error(Some(1), "").contains("código 1"));
    }

    #[test]
    fn corta_procesos_colgados() {
        // Un servidor que no responde (puerto cerrado en una IP no enrutable) no debe bloquear para siempre.
        let access = Access::new("rest:http://10.255.255.1:9/", "x");
        let t = Instant::now();
        let err = run_with(&access, &["cat", "config", "--no-lock"], Duration::from_secs(3), None).unwrap_err();
        assert!(t.elapsed() < Duration::from_secs(10), "tardó {:?}", t.elapsed());
        assert!(!err.is_empty());
    }

    #[test]
    fn se_puede_cancelar() {
        if sin_restic() {
            return;
        }
        let access = Access::new("rest:http://10.255.255.1:9/", "x");
        let flag = AtomicBool::new(true);
        let err = run_with(&access, &["cat", "config", "--no-lock"], Duration::from_secs(30), Some(&flag)).unwrap_err();
        assert_eq!(err, CANCELLED);
    }

    /// Las pruebas que ejecutan restic se omiten si no está (p. ej. en la CI de
    /// Linux, que solo comprueba los crates; la de Windows lo descarga con su huella).
    #[cfg(test)]
    fn sin_restic() -> bool {
        let falta = version().is_err();
        if falta {
            eprintln!("omitido: no hay restic (ni junto a la prueba ni en el PATH)");
        }
        falta
    }

    #[test]
    fn oculta_contrasenas_en_direcciones() {
        assert_eq!(
            redact_credentials(r#"Fatal: Get "https://ana:s3cr3t@nas:8000/repo/config": EOF"#),
            r#"Fatal: Get "https://ana:***@nas:8000/repo/config": EOF"#
        );
        assert_eq!(redact_credentials("rest:http://u:p@h/r y rest:http://h2/r"), "rest:http://u:***@h/r y rest:http://h2/r");
        assert_eq!(redact_credentials("sin direcciones"), "sin direcciones");
        assert_eq!(redact_credentials("https://ana@host/x"), "https://ana@host/x");
        assert_eq!(redact_credentials("http://host:8000/x"), "http://host:8000/x");
        assert!(!exit_error(Some(1), "Fatal: unable to open repo at rest:https://ana:clave@h/r: 401").contains("clave"));
    }

    #[test]
    fn valida_ids_y_rutas_de_snapshot() {
        assert!(valid_snapshot_id("da37898f"));
        assert!(!valid_snapshot_id("--help"));
        assert!(!valid_snapshot_id("latest"));
        assert_eq!(snapshot_dir("/C/Users/").unwrap(), "/C/Users");
        assert_eq!(snapshot_dir("").unwrap(), "/");
        assert!(snapshot_dir("C/Users").is_err());
        assert!(snapshot_dir("/C/../etc").is_err());
    }

    /// `C:\Users\Ana` → `/C/Users/Ana` (formato de rutas de restic en Windows).
    pub fn to_snapshot_path(p: &str) -> String {
        super::ruta_en_version(p)
    }

    /// Lista la carpeta copiada en el snapshot más reciente del repo de prueba.
    #[test]
    fn lista_carpeta_de_snapshot_real() {
        let (Ok(repo), Ok(pw), Ok(data)) =
            (std::env::var("RESGUARDO_TEST_REPO"), std::env::var("RESGUARDO_TEST_PASSWORD"), std::env::var("RESGUARDO_TEST_DATA"))
        else {
            return;
        };
        let access = Access::new(repo, pw);
        let snap = snapshots(&access).unwrap().into_iter().max_by(|a, b| a.time.cmp(&b.time)).unwrap();
        let dir = to_snapshot_path(&data);
        let entries = list_dir(&access, &snap.short_id, &dir).unwrap();
        assert!(!entries.is_empty());
        assert!(entries.iter().all(|e| e.path.starts_with(&dir) && e.path != dir));
    }

    #[test]
    fn repo_inexistente_da_mensaje_claro() {
        if sin_restic() {
            return;
        }
        let dir = std::env::temp_dir().join("resguardo-no-existe-xyz");
        let err = snapshots(&Access::new(dir.to_str().unwrap(), "x")).unwrap_err();
        assert_eq!(err, NOT_FOUND);
    }
}
