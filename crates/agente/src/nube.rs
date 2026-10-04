//! Nubes conectadas (Dropbox, Google Drive) para el espejo del Servidor de
//! copias, con el rclone oficial que acompaña al agente.
//!
//! - **Conectar desde la consola** (orden `conectar_nube`, clave de
//!   administración): la consola hace OAuth 2 con PKCE contra la app
//!   «Resguardo» de Dropbox (permiso «App folder», sin app secret) y manda el
//!   refresh token **sellado** para este equipo. Aquí se comprueba renovándolo
//!   una vez y se guarda **solo** en la carpeta privada del agente, protegido
//!   como los demás secretos (DPAPI en Windows). El resultado nunca lo repite.
//! - **Conectar en el propio equipo** (`resguardo-agente nube conectar
//!   dropbox|drive --nombre …`, como administrador): `rclone authorize`, que
//!   abre el navegador para dar permiso; se guarda igual.
//! - **Renovar** (Dropbox conectada desde la consola): antes de cada vuelta,
//!   el agente pide a Dropbox un access token nuevo (`grant_type=refresh_token`
//!   con `client_id` y sin secreto, como cliente público con PKCE) y se lo da
//!   a rclone ya fresco (4 h). rclone recibe también el refresh token y el
//!   `client_id`: si una vuelta dura más, renueva él con la misma petición
//!   (comprobado con rclone 1.75.1: con `client_id` propio y sin secreto, tras
//!   un intento con «Basic», manda `client_id` en el cuerpo). Ver docs/destinos.md.
//! - **Usar**: rclone recibe el remoto por variables de entorno
//!   (`RCLONE_CONFIG_RNUBE_TYPE`, `…_TOKEN`); nunca se escribe un `rclone.conf`
//!   con el token. El archivo de configuración que rclone pudiera tocar es uno
//!   vacío dentro de la carpeta privada, y se borra al terminar.
//! - Lo que sube son los archivos del Servidor de copias tal cual: paquetes de
//!   restic ya cifrados. Ninguna contraseña de repositorio interviene.
//! - Nunca se borra nada en la nube: `rclone copy` (nunca `sync`) con
//!   `--immutable` (un archivo que ya está no se vuelve a escribir).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const ARCHIVO: &str = "nubes.bin";
/// Donde Dropbox renueva los access tokens.
const DROPBOX_TOKEN_URL: &str = "https://api.dropboxapi.com/oauth2/token";
/// El marcador de la consola mientras la app de Dropbox no está registrada.
pub const DROPBOX_APP_KEY_MARCADOR: &str = "PENDIENTE_APP_KEY_DROPBOX";
/// Se renueva el access token si caduca antes de esto (los de Dropbox duran 4 h).
const MARGEN_RENOVAR_S: i64 = 30 * 60;
/// Nombre del remoto en las variables de entorno de rclone.
const REMOTO: &str = "rnube";

/// Tipos de nube admitidos (los que rclone conecta con `authorize` sin más preguntas).
pub const TIPOS: &[(&str, &str)] = &[("dropbox", "Dropbox"), ("drive", "Google Drive")];

/// SHA-256 del rclone oficial que acompaña a esta versión (1.75.1; ver scripts/fetch-rclone.ps1).
/// En otras plataformas no hay huella fijada y la nube no se usa.
pub const RCLONE_SHA256: &str = if cfg!(all(windows, target_arch = "x86_64")) {
    "033eee51c9ad47c2de2624b6674d355274bcd6cf0027a5f85db4437ba24ae81c"
} else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
    "f66d8c1d552ad90296a11bc8b46d56a7fa5da1a7fa05e7ca522d95df92c4a4c0"
} else {
    "sin huella fijada"
};

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Nube {
    pub nombre: String,
    /// "dropbox" o "drive".
    pub tipo: String,
    /// El token de rclone (JSON). Nunca sale del equipo.
    pub token: String,
    /// App key pública de Dropbox con la que se conectó desde la consola
    /// (PKCE, sin secreto): el agente renueva el access token con ella.
    /// `None` en las conectadas con `rclone authorize` (renueva rclone).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_key: Option<String>,
}

/// Lo que se puede mostrar (sin token).
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct NubePublica {
    pub nombre: String,
    pub tipo: String,
}

fn ruta() -> PathBuf {
    crate::agent::private_dir().join(ARCHIVO)
}

pub fn cargar() -> Vec<Nube> {
    std::fs::read(ruta()).ok().and_then(|b| crate::platform::unprotect(&b).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn guardar(nubes: &[Nube]) -> Result<(), String> {
    crate::agent::prepare_dir()?;
    let datos = crate::platform::protect(&serde_json::to_vec(nubes).map_err(|e| e.to_string())?)?;
    let tmp = crate::agent::private_dir().join(format!("{ARCHIVO}.tmp"));
    let _ = std::fs::remove_file(&tmp);
    std::fs::write(&tmp, datos).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, ruta()).map_err(|e| e.to_string())
}

/// Las nubes conectadas, sin tokens.
pub fn lista() -> Vec<NubePublica> {
    cargar().into_iter().map(|n| NubePublica { nombre: n.nombre, tipo: n.tipo }).collect()
}

pub fn buscar(nombre: &str) -> Option<Nube> {
    cargar().into_iter().find(|n| n.nombre == nombre)
}

fn nombre_valido(n: &str) -> bool {
    let n = n.trim();
    !n.is_empty() && n.chars().count() <= 60 && !n.chars().any(char::is_control)
}

// ---------- rclone ----------

/// El rclone que acompaña al agente (junto al ejecutable). En las pruebas, el de src-tauri/binaries.
pub fn binario() -> PathBuf {
    let nombre = if cfg!(windows) { "rclone.exe" } else { "rclone" };
    let junto = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(nombre))).unwrap_or_else(|| nombre.into());
    if cfg!(test) && !junto.is_file() {
        let triple = if cfg!(windows) { "x86_64-pc-windows-msvc.exe" } else { "x86_64-unknown-linux-gnu" };
        return Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/binaries").join(format!("rclone-{triple}"));
    }
    junto
}

/// ¿Está rclone y es el de esta versión (huella fijada)?
pub fn comprobar_binario() -> Result<PathBuf, String> {
    let b = binario();
    let datos = std::fs::read(&b).map_err(|_| "Falta rclone junto al agente: reinstala Resguardo Agente.".to_string())?;
    let h: String = Sha256::digest(&datos).iter().map(|x| format!("{x:02x}")).collect();
    if h != RCLONE_SHA256 {
        return Err("rclone no es el que acompaña a esta versión (la huella no coincide): no se ejecuta.".into());
    }
    Ok(b)
}

/// rclone con el remoto `rnube` por variables de entorno y un archivo de
/// configuración vacío en la carpeta privada (rclone no escribe el token en disco).
pub fn comando(n: &Nube, conf: &Path) -> Result<std::process::Command, String> {
    let mut c = std::process::Command::new(comprobar_binario()?);
    // Solo el entorno mínimo (ninguna `RCLONE_*` heredada: `RCLONE_DUMP`,
    // `RCLONE_LOG_FILE`… sacarían el token); las del remoto, las pone el agente.
    resguardo_motor::proceso::entorno_minimo(&mut c);
    let m = REMOTO.to_uppercase();
    c.env(format!("RCLONE_CONFIG_{m}_TYPE"), &n.tipo).env(format!("RCLONE_CONFIG_{m}_TOKEN"), &n.token).arg("--config").arg(conf);
    if let Some(k) = &n.app_key {
        // La app «Resguardo» (sin secreto): si rclone tiene que renovar, lo hace como cliente público.
        c.env(format!("RCLONE_CONFIG_{m}_CLIENT_ID"), k);
    }
    c.stdin(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // sin ventana
    }
    Ok(c)
}

/// Quita las variables `RCLONE_*` heredadas del entorno (`RCLONE_DUMP`,
/// `RCLONE_LOG_FILE`, otros remotos…): podrían sacar el token a un archivo o
/// cambiar lo que hace rclone. Solo valen las que pone el agente.
fn quitar_heredadas(c: &mut std::process::Command, claves: impl Iterator<Item = std::ffi::OsString>) {
    for k in claves {
        if k.to_string_lossy().to_ascii_uppercase().starts_with("RCLONE_") {
            c.env_remove(k);
        }
    }
}

/// El token de la salida de `rclone authorize` (entre «--->» y «<---End paste»).
pub fn token_de_salida(salida: &str) -> Option<String> {
    let ini = salida.find("--->")? + 4;
    let fin = salida[ini..].find("<---")? + ini;
    let t = salida[ini..fin].trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// `nube conectar <tipo> --nombre …`: abre el navegador para dar permiso y guarda el token.
pub fn conectar(tipo: &str, nombre: &str) -> Result<String, String> {
    crate::agent::require_admin()?;
    if !TIPOS.iter().any(|(t, _)| *t == tipo) {
        return Err(format!("Tipo de nube no admitido: «{tipo}» (dropbox o drive)."));
    }
    let nombre = nombre.trim().to_string();
    if !nombre_valido(&nombre) {
        return Err("Escribe un nombre para la nube (hasta 60 caracteres).".into());
    }
    if cargar().iter().any(|n| n.nombre == nombre) {
        return Err(format!("Ya hay una nube «{nombre}»: quítala antes o usa otro nombre."));
    }
    let b = comprobar_binario()?;
    eprintln!("Se abre el navegador para dar permiso a Resguardo en {tipo}. Vuelve aquí cuando termines.");
    crate::agent::prepare_dir()?;
    let conf = crate::agent::private_dir().join("rclone-vacio.conf");
    // Los avisos de rclone (y el enlace, por si el navegador no se abre) se
    // muestran; el token, que va entre «--->» y «<---End paste», no.
    let mut orden = std::process::Command::new(b);
    quitar_heredadas(&mut orden, std::env::vars_os().map(|(k, _)| k));
    let mut hijo = orden
        .args(["authorize", tipo, "--config"])
        .arg(&conf)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .map_err(|e| format!("No se pudo ejecutar rclone: {e}"))?;
    let mut salida = String::new();
    if let Some(out) = hijo.stdout.take() {
        use std::io::BufRead;
        let mut dentro = false;
        for l in std::io::BufReader::new(out).lines().map_while(Result::ok) {
            salida.push_str(&l);
            salida.push('\n');
            if l.contains("<---") {
                dentro = false;
            } else if l.contains("--->") {
                dentro = true;
            } else if !dentro {
                println!("{l}");
            }
        }
    }
    let ok = hijo.wait().is_ok_and(|s| s.success());
    let _ = std::fs::remove_file(&conf);
    if !ok {
        return Err("rclone no terminó la autorización (¿se cerró el navegador?).".into());
    }
    let token = token_de_salida(&salida).ok_or("No llegó el permiso de la nube.")?;
    anadir(tipo, &nombre, &token)
}

/// Guarda una nube con el token que dio `rclone authorize` (aquí o, en la
/// ventana del equipo, como el usuario: él abre el navegador y lo trae por
/// ipc_local con la clave de administración).
pub fn anadir(tipo: &str, nombre: &str, token: &str) -> Result<String, String> {
    if !TIPOS.iter().any(|(t, _)| *t == tipo) {
        return Err(format!("Tipo de nube no admitido: «{tipo}» (dropbox o drive)."));
    }
    let nombre = nombre.trim().to_string();
    if !nombre_valido(&nombre) {
        return Err("Escribe un nombre para la nube (hasta 60 caracteres).".into());
    }
    if !token_plausible(token) {
        return Err("No llegó el permiso de la nube.".into());
    }
    let mut nubes = cargar();
    if nubes.iter().any(|n| n.nombre == nombre) {
        return Err(format!("Ya hay una nube «{nombre}»: quítala antes o usa otro nombre."));
    }
    nubes.push(Nube { nombre: nombre.clone(), tipo: tipo.into(), token: token.into(), app_key: None });
    guardar(&nubes)?;
    crate::agent::log(&format!("Nube «{nombre}» ({tipo}) conectada."));
    Ok(format!("Nube «{nombre}» conectada. Ya se puede usar como destino del espejo."))
}

/// `nube quitar <nombre>`: olvida el token (lo ya subido se queda en la nube).
pub fn quitar(nombre: &str) -> Result<String, String> {
    crate::agent::require_admin()?;
    let mut nubes = cargar();
    let antes = nubes.len();
    nubes.retain(|n| n.nombre != nombre);
    if nubes.len() == antes {
        return Err(format!("No hay ninguna nube «{nombre}»."));
    }
    if crate::server::load().espejo.as_ref().is_some_and(|e| e.destinos().iter().any(|d| d.nube.as_deref() == Some(nombre))) {
        return Err(format!("El espejo usa «{nombre}»: quítala antes del espejo."));
    }
    guardar(&nubes)?;
    crate::agent::log(&format!("Nube «{nombre}» quitada del equipo."));
    Ok(format!("Nube «{nombre}» quitada (lo ya subido sigue en ella; revoca también el permiso en su web)."))
}

/// Ruta remota válida: relativa, sin «..» ni caracteres raros.
pub fn carpeta_remota_valida(c: &str) -> bool {
    let c = c.trim().trim_matches('/');
    !c.is_empty() && c.len() <= 200 && !c.chars().any(char::is_control) && !c.contains(':') && !c.split('/').any(|p| p == ".." || p == "." || p.is_empty())
}

/// Argumentos de `rclone copy` del espejo: solo añade, nunca borra.
pub fn argumentos_copia(origen: &Path, carpeta: &str, limite_kib: Option<u32>) -> Vec<String> {
    let mut a = vec![
        "copy".to_string(),
        origen.display().to_string(),
        format!("{REMOTO}:{}", carpeta.trim().trim_matches('/')),
        // Los bloqueos de restic no se copian; ni lo escrito en los últimos 10 minutos.
        "--exclude".into(),
        "locks/**".into(),
        "--exclude".into(),
        "*.tmp-espejo".into(),
        "--min-age".into(),
        "10m".into(),
        // Un archivo que ya está en la nube nunca se reescribe (los de restic no cambian).
        "--immutable".into(),
        // Registro en JSON: al final, una línea con las estadísticas («stats»).
        "--use-json-log".into(),
        "--stats".into(),
        "24h".into(),
        "--stats-one-line".into(),
        "--stats-log-level".into(),
        "NOTICE".into(),
        "--retries".into(),
        "3".into(),
    ];
    if let Some(k) = limite_kib.filter(|k| *k > 0) {
        a.push("--bwlimit".into());
        a.push(format!("{k}K"));
    }
    a
}

/// El token del remoto en un rclone.conf (si rclone lo renovó y lo escribió).
fn token_de_conf(t: &str) -> Option<String> {
    let mut en_remoto = false;
    for l in t.lines().map(str::trim) {
        if l.starts_with('[') {
            en_remoto = l == format!("[{REMOTO}]");
        } else if en_remoto {
            if let Some(v) = l.strip_prefix("token").map(str::trim_start).and_then(|r| r.strip_prefix('=')) {
                return Some(v.trim().to_string()).filter(|v| !v.is_empty());
            }
        }
    }
    None
}

/// Las estadísticas finales del registro JSON de rclone, en castellano, y el
/// último error (si hubo).
fn resumen_de_rclone(err: &str) -> (String, Option<String>) {
    let lineas: Vec<serde_json::Value> = err.lines().filter_map(|l| serde_json::from_str(l.trim()).ok()).collect();
    let error = lineas.iter().rev().find(|l| l["level"] == "error").and_then(|l| l["msg"].as_str()).map(crate::web::public_message);
    let texto = match lineas.iter().rev().find_map(|l| l.get("stats")) {
        Some(s) => {
            let n = |k: &str| s[k].as_u64().unwrap_or(0);
            let mut t = format!("{} archivos subidos ({} MB), {} ya estaban", n("transfers"), n("bytes") / (1024 * 1024), n("checks"));
            if n("errors") > 0 {
                t += &format!(", {} errores", n("errors"));
            }
            t + "."
        }
        None => "Nada nuevo que subir.".into(),
    };
    (texto, error)
}

/// Copia `origen` a la nube. Devuelve el resumen de rclone.
pub fn copiar(n: &Nube, origen: &Path, carpeta: &str, limite_kib: Option<u32>, ritmos: &mut dyn FnMut(Option<u64>, Option<u64>)) -> Result<String, String> {
    if !carpeta_remota_valida(carpeta) {
        return Err("Carpeta de la nube no válida.".into());
    }
    let n = &al_dia(n, &renovar_dropbox)?;
    let conf = crate::agent::private_dir().join("rclone-vacio.conf");
    let out = comando(n, &conf).and_then(|mut c| salida_midiendo(c.args(argumentos_copia(origen, carpeta, limite_kib)), ritmos));
    // Si rclone renovó el token, lo deja en ese archivo (carpeta privada): se
    // guarda protegido con los demás y el archivo se borra.
    if let Some(nuevo) = std::fs::read_to_string(&conf).ok().and_then(|t| token_de_conf(&t)).filter(|t| *t != n.token) {
        let mut nubes = cargar();
        if let Some(x) = nubes.iter_mut().find(|x| x.nombre == n.nombre) {
            x.token = nuevo;
            let _ = guardar(&nubes);
        }
    }
    let _ = std::fs::remove_file(&conf);
    let out = out?;
    let err = String::from_utf8_lossy(&out.stderr);
    let (texto, error) = resumen_de_rclone(&err);
    if out.status.success() {
        Ok(texto)
    } else {
        Err(format!("{} {texto}", error.unwrap_or_else(|| "rclone terminó con error.".into())))
    }
}

/// Como `Command::output`, pero mientras rclone trabaja dice cada 2 s lo que lee
/// y sube de verdad (bytes/s, de sus contadores de E/S), para la ventana del equipo.
fn salida_midiendo(c: &mut std::process::Command, ritmos: &mut dyn FnMut(Option<u64>, Option<u64>)) -> Result<std::process::Output, String> {
    use std::io::Read;
    let mut hijo =
        c.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().map_err(|e| format!("No se pudo ejecutar rclone: {e}"))?;
    // Leer en hilos aparte: rclone nunca se bloquea escribiendo.
    let leer = |r: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut b = Vec::new();
            if let Some(mut r) = r {
                let _ = r.read_to_end(&mut b);
            }
            b
        })
    };
    let salida = leer(hijo.stdout.take().map(|x| Box::new(x) as Box<dyn Read + Send>));
    let errores = leer(hijo.stderr.take().map(|x| Box::new(x) as Box<dyn Read + Send>));
    let pid = hijo.id();
    let mut ritmo = crate::progreso_v2::RitmoIo::default();
    let estado = loop {
        match hijo.try_wait() {
            Ok(Some(e)) => break e,
            Ok(None) => {
                let (l, s) = ritmo.medir(Some(pid));
                ritmos(l, s);
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            Err(e) => {
                let _ = hijo.kill();
                return Err(format!("No se pudo esperar a rclone: {e}"));
            }
        }
    };
    Ok(std::process::Output { status: estado, stdout: salida.join().unwrap_or_default(), stderr: errores.join().unwrap_or_default() })
}

/// v1.31 («¿Cuándo se llena?»): el espacio de la cuenta (`rclone about --json`).
/// Se pide tras cada espejo a esa nube (una vez por noche): una petición a la
/// API, sin listar archivos. `None` si la nube no lo dice (o no hay conexión).
pub fn cuota(n: &Nube) -> Option<crate::espacio::Espacio> {
    let n = &al_dia(n, &renovar_dropbox).ok()?;
    let conf = crate::agent::private_dir().join("rclone-vacio.conf");
    let out = comando(n, &conf).ok()?.args(["about", &format!("{REMOTO}:"), "--json"]).output();
    let _ = std::fs::remove_file(&conf);
    let out = out.ok().filter(|o| o.status.success())?;
    cuota_de_about(&String::from_utf8_lossy(&out.stdout))
}

/// `{ "total", "used", "free" }` de `rclone about --json` (sin «free», total − usado).
fn cuota_de_about(t: &str) -> Option<crate::espacio::Espacio> {
    let v: serde_json::Value = serde_json::from_str(t).ok()?;
    let total = v["total"].as_u64().filter(|t| *t > 0)?;
    let libre = v["free"].as_u64().or_else(|| v["used"].as_u64().map(|u| total.saturating_sub(u)))?;
    Some(crate::espacio::Espacio { libre: libre.min(total), total })
}

// ---------- Dropbox desde la consola (OAuth 2 con PKCE) ----------

/// Por qué no se pudo renovar el access token.
#[derive(Debug, PartialEq)]
pub enum FalloRenovar {
    /// Dropbox dijo que no (permiso retirado, app key que no es): hay que volver a conectar.
    Rechazado(String),
    /// Sin respuesta útil (sin conexión, Dropbox caído): se reintenta en la próxima vuelta.
    SinConexion(String),
}

/// Un access token nuevo y cuándo caduca.
pub type Renovado = (String, chrono::DateTime<chrono::Utc>);
type Renovar<'a> = &'a dyn Fn(&str, &str) -> Result<Renovado, FalloRenovar>;

/// ¿Parece una app key de Dropbox? (pública; letras minúsculas y números).
pub fn app_key_valida(k: &str) -> Result<(), String> {
    let k = k.trim();
    if k.is_empty() || k == DROPBOX_APP_KEY_MARCADOR {
        return Err("La app «Resguardo» de Dropbox aún no está configurada en el servidor (falta su app key). Avisa a quien lo administra.".into());
    }
    if !(8..=40).contains(&k.len()) || !k.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()) {
        return Err("La app key de Dropbox que llegó no es válida.".into());
    }
    Ok(())
}

/// Nombre de una nube conectada desde la consola (las mismas reglas que allí).
pub fn nombre_de_consola_valido(n: &str) -> bool {
    let n = n.trim();
    (1..=40).contains(&n.chars().count()) && n.chars().all(|c| c.is_alphanumeric() || matches!(c, ' ' | '_' | '.' | '-'))
}

/// Un token de OAuth razonable (sin espacios ni controles, de tamaño acotado).
fn token_plausible(t: &str) -> bool {
    !t.is_empty() && t.len() <= 4096 && !t.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Lo que trae `conectar_nube`, ya comprobado.
pub struct PedidoConectar {
    pub nombre: String,
    pub app_key: String,
    refresh_token: String,
    access_token: Option<Renovado>,
}

/// `conectar_nube { tipo: "dropbox", nombre, refresh_token, access_token?, expira?, app_key }`.
pub fn leer_conectar(c: &serde_json::Value) -> Result<PedidoConectar, String> {
    if c["tipo"] != "dropbox" {
        return Err("Tipo de nube no admitido: desde la consola, solo Dropbox.".into());
    }
    let app_key = c["app_key"].as_str().unwrap_or("").trim().to_string();
    app_key_valida(&app_key)?;
    let nombre = c["nombre"].as_str().unwrap_or("").trim().to_string();
    if !nombre_de_consola_valido(&nombre) {
        return Err("Nombre de nube no válido: letras, números, espacios, guiones o puntos (hasta 40).".into());
    }
    let refresh_token = c["refresh_token"].as_str().unwrap_or("").to_string();
    if !token_plausible(&refresh_token) {
        return Err("Falta el permiso de Dropbox (o no es válido): vuelve a conectar.".into());
    }
    let expira = c["expira"].as_str().and_then(|e| chrono::DateTime::parse_from_rfc3339(e).ok());
    let access_token = match (c["access_token"].as_str(), expira) {
        (Some(a), Some(e)) if token_plausible(a) => Some((a.to_string(), e.with_timezone(&chrono::Utc))),
        _ => None,
    };
    Ok(PedidoConectar { nombre, app_key, refresh_token, access_token })
}

/// El token en la forma que entiende rclone (oauth2.Token en JSON).
fn token_rclone(access: &str, refresh: &str, expira: chrono::DateTime<chrono::Utc>) -> String {
    serde_json::json!({
        "access_token": access, "token_type": "bearer", "refresh_token": refresh,
        "expiry": expira.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    })
    .to_string()
}

/// Pide a Dropbox un access token nuevo, como cliente público (PKCE: `client_id`, sin secreto).
pub fn renovar_dropbox(app_key: &str, refresh_token: &str) -> Result<Renovado, FalloRenovar> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(30)))
        .http_status_as_error(false)
        .tls_config(crate::web::tls_sistema())
        .build()
        .new_agent();
    let mut r = agent
        .post(DROPBOX_TOKEN_URL)
        .send_form([("grant_type", "refresh_token"), ("refresh_token", refresh_token), ("client_id", app_key)])
        .map_err(|_| FalloRenovar::SinConexion("No se pudo hablar con Dropbox desde este equipo (¿sin internet?).".into()))?;
    let status = r.status().as_u16();
    let v: serde_json::Value = r.body_mut().read_json().unwrap_or(serde_json::Value::Null);
    interpretar_renovacion(status, &v)
}

/// La respuesta de Dropbox al renovar, en castellano (sin repetir nada de ella).
fn interpretar_renovacion(status: u16, v: &serde_json::Value) -> Result<Renovado, FalloRenovar> {
    if status == 200 {
        if let Some(a) = v["access_token"].as_str().filter(|a| token_plausible(a)) {
            let segundos = v["expires_in"].as_i64().filter(|s| *s > 0).unwrap_or(4 * 3600);
            return Ok((a.to_string(), chrono::Utc::now() + chrono::Duration::seconds(segundos)));
        }
    }
    if status >= 500 || status == 429 {
        return Err(FalloRenovar::SinConexion("Dropbox no responde ahora; se reintenta más tarde.".into()));
    }
    Err(FalloRenovar::Rechazado(match v["error"].as_str().unwrap_or("") {
        "invalid_client" => "Dropbox no reconoce la app «Resguardo» (app key): avisa a quien administra el servidor.".into(),
        "invalid_grant" => "Dropbox ya no acepta el permiso (se retiró o caducó): vuelve a conectar la nube desde la consola.".into(),
        _ => "Dropbox no aceptó el permiso: vuelve a conectar la nube desde la consola.".into(),
    }))
}

/// Antes de usar una Dropbox conectada desde la consola: su access token,
/// renovado si caduca pronto (y guardado). Las de `rclone authorize`, tal cual.
fn al_dia(n: &Nube, renovar: Renovar) -> Result<Nube, String> {
    let Some(app_key) = n.app_key.as_deref() else { return Ok(n.clone()) };
    let t: serde_json::Value = serde_json::from_str(&n.token).unwrap_or_default();
    let refresh = t["refresh_token"].as_str().unwrap_or("");
    if refresh.is_empty() {
        return Err(format!("La nube «{}» no tiene permiso guardado: vuelve a conectarla desde la consola.", n.nombre));
    }
    let caduca = t["expiry"].as_str().and_then(|e| chrono::DateTime::parse_from_rfc3339(e).ok()).map(|d| d.timestamp()).unwrap_or(0);
    let ahora = chrono::Utc::now().timestamp();
    if caduca > ahora + MARGEN_RENOVAR_S {
        return Ok(n.clone());
    }
    match renovar(app_key, refresh) {
        Ok((access, expira)) => {
            let nueva = Nube { token: token_rclone(&access, refresh, expira), ..n.clone() };
            let mut nubes = cargar();
            if let Some(x) = nubes.iter_mut().find(|x| x.nombre == n.nombre) {
                x.token = nueva.token.clone();
                let _ = guardar(&nubes);
            }
            Ok(nueva)
        }
        // Sin conexión, pero el que hay aún vale: se usa (rclone renovará si hace falta).
        Err(FalloRenovar::SinConexion(_)) if caduca > ahora + 60 => Ok(n.clone()),
        Err(FalloRenovar::SinConexion(m) | FalloRenovar::Rechazado(m)) => Err(format!("«{}»: {m}", n.nombre)),
    }
}

/// `conectar_nube`: guarda (o renueva) una Dropbox conectada desde la consola.
/// El mensaje nunca lleva el token.
pub fn conectar_desde_orden(c: &serde_json::Value) -> Result<String, String> {
    let p = leer_conectar(c)?;
    if !crate::server::load().enabled {
        return Err("Este equipo no guarda copias: activa antes el Servidor de copias.".into());
    }
    conectar_con(p, &renovar_dropbox)
}

fn conectar_con(p: PedidoConectar, renovar: Renovar) -> Result<String, String> {
    let mut nubes = cargar();
    if nubes.iter().any(|n| n.nombre == p.nombre && n.tipo != "dropbox") {
        return Err(format!("Ya hay una nube «{}» de otro tipo en este equipo: usa otro nombre.", p.nombre));
    }
    // Se comprueba ya: renovar una vez dice si Dropbox acepta este permiso con esta app.
    let (access, expira, comprobada) = match renovar(&p.app_key, &p.refresh_token) {
        Ok((a, e)) => (a, e, true),
        Err(FalloRenovar::Rechazado(m)) => return Err(m),
        Err(FalloRenovar::SinConexion(_)) => match p.access_token.clone() {
            Some((a, e)) => (a, e, false),
            None => (String::new(), chrono::Utc::now(), false),
        },
    };
    let reconectada = nubes.iter().any(|n| n.nombre == p.nombre);
    nubes.retain(|n| n.nombre != p.nombre);
    nubes.push(Nube { nombre: p.nombre.clone(), tipo: "dropbox".into(), token: token_rclone(&access, &p.refresh_token, expira), app_key: Some(p.app_key) });
    guardar(&nubes)?;
    let hecho = if reconectada { "reconectada" } else { "conectada" };
    crate::agent::log(&format!("Nube «{}» (Dropbox) {hecho} desde la consola.", p.nombre));
    let mut m = format!("Dropbox «{}» {hecho} (solo Aplicaciones/Resguardo).", p.nombre);
    if !comprobada {
        m += " No se pudo comprobar ahora con Dropbox: se comprobará en la próxima vuelta del espejo.";
    }
    Ok(m)
}

/// ¿Usa el espejo la nube `nombre`? (quitarla entonces es destructivo).
pub fn usa_espejo(nombre: &str) -> bool {
    crate::server::load().espejo.as_ref().is_some_and(|e| e.destinos().iter().any(|d| d.tipo == "nube" && d.nube.as_deref() == Some(nombre)))
}

/// `quitar_nube { nombre }`: olvida el permiso. Si el espejo la usaba (orden
/// que ya ha esperado), deja de subir a ella; sin más destinos, el espejo se quita.
pub fn quitar_desde_orden(c: &serde_json::Value) -> Result<String, String> {
    let nombre = c["nombre"].as_str().unwrap_or("").trim().to_string();
    let mut nubes = cargar();
    if !nubes.iter().any(|n| n.nombre == nombre) {
        return Err(format!("No hay ninguna nube «{nombre}» en este equipo."));
    }
    let mut aviso = "";
    let mut conf = crate::server::load();
    if let Some(mut e) = conf.espejo.take() {
        e.normalizar();
        let antes = e.destinos.len();
        e.destinos.retain(|d| !(d.tipo == "nube" && d.nube.as_deref() == Some(nombre.as_str())));
        if e.destinos.len() != antes {
            aviso = if e.destinos.is_empty() { " El espejo se quitó: no le quedaban destinos." } else { " El espejo ya no sube a ella." };
        }
        conf.espejo = (!e.destinos.is_empty()).then_some(e);
        if !aviso.is_empty() {
            crate::server::save(&conf)?;
        }
    }
    nubes.retain(|n| n.nombre != nombre);
    guardar(&nubes)?;
    crate::agent::log(&format!("Nube «{nombre}» quitada desde la consola."));
    Ok(format!(
        "«{nombre}» desconectada: el equipo olvidó su permiso. Lo ya subido sigue en la nube; puedes retirar también el permiso de la app en su web.{aviso}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cuota_de_rclone_about() {
        let e = cuota_de_about(r#"{"total":2199023255552,"used":2194728288256,"free":4294967296}"#).unwrap();
        assert_eq!((e.libre, e.total), (4_294_967_296, 2_199_023_255_552));
        // Sin «free»: total menos lo usado.
        assert_eq!(cuota_de_about(r#"{"total":100,"used":30}"#).map(|e| e.libre), Some(70));
        // Sin total (cuentas sin límite) o salida rara: nada.
        assert_eq!(cuota_de_about(r#"{"used":30}"#), None);
        assert_eq!(cuota_de_about("no es json"), None);
    }

    const KEY: &str = "abc123def456ghi";
    const RT: &str = "rt-secreto-NO-SALE-0123456789";
    const AT: &str = "at-secreto-NO-SALE-9876543210";

    fn pedido(c: serde_json::Value) -> Result<PedidoConectar, String> {
        leer_conectar(&c)
    }

    #[test]
    fn conectar_nube_lee_el_cuerpo_de_la_consola() {
        // Lo que manda la consola: { tipo, nombre, refresh_token, access_token?, expira?, app_key }.
        let p = pedido(json!({ "tipo": "dropbox", "nombre": " Dropbox Sur ", "refresh_token": RT, "access_token": AT, "expira": "2026-10-03T00:00:00.000Z", "app_key": KEY }))
            .unwrap();
        assert_eq!((p.nombre.as_str(), p.app_key.as_str(), p.refresh_token.as_str()), ("Dropbox Sur", KEY, RT));
        assert_eq!(p.access_token.as_ref().unwrap().0, AT);
        // Sin access_token (o sin expira): vale igual.
        assert!(pedido(json!({ "tipo": "dropbox", "nombre": "Ñandú 2.0_x-y", "refresh_token": RT, "app_key": KEY })).unwrap().access_token.is_none());
        // La app key: el marcador o vacía, con el mensaje claro; con otra forma, no vale.
        for k in [json!(DROPBOX_APP_KEY_MARCADOR), json!(""), json!(null)] {
            let e = pedido(json!({ "tipo": "dropbox", "nombre": "D", "refresh_token": RT, "app_key": k })).err().unwrap();
            assert!(e.contains("no está configurada"), "{e}");
        }
        for k in ["ABCDEFGHIJ", "abc", "abc-def-ghi-jkl", "abc def ghi"] {
            assert!(pedido(json!({ "tipo": "dropbox", "nombre": "D", "refresh_token": RT, "app_key": k })).err().unwrap().contains("no es válida"), "{k}");
        }
        // Tipo, nombre y token.
        assert!(pedido(json!({ "tipo": "drive", "nombre": "D", "refresh_token": RT, "app_key": KEY })).is_err());
        for n in ["", "   ", "a/b", "x:y", &"n".repeat(41), "tab\tx"] {
            assert!(pedido(json!({ "tipo": "dropbox", "nombre": n, "refresh_token": RT, "app_key": KEY })).is_err(), "{n}");
        }
        for t in [json!(""), json!(null), json!(5), json!("con espacio"), json!("x".repeat(5000))] {
            let e = pedido(json!({ "tipo": "dropbox", "nombre": "D", "refresh_token": t, "app_key": KEY })).err().unwrap();
            assert!(e.contains("permiso de Dropbox"), "{e}");
        }
    }

    #[test]
    fn respuesta_de_dropbox_al_renovar() {
        let (a, e) = interpretar_renovacion(200, &json!({ "access_token": AT, "token_type": "bearer", "expires_in": 14400 })).unwrap();
        assert_eq!(a, AT);
        assert!((e - chrono::Utc::now()).num_seconds() > 14000);
        let m = |r: Result<Renovado, FalloRenovar>| match r {
            Err(FalloRenovar::Rechazado(m)) => ("rechazado", m),
            Err(FalloRenovar::SinConexion(m)) => ("sin_conexion", m),
            Ok(_) => ("ok", String::new()),
        };
        let (k, t) = m(interpretar_renovacion(400, &json!({ "error": "invalid_grant", "error_description": RT })));
        assert_eq!(k, "rechazado");
        assert!(t.contains("vuelve a conectar") && !t.contains(RT));
        assert!(m(interpretar_renovacion(400, &json!({ "error": "invalid_client" }))).1.contains("app key"));
        assert_eq!(m(interpretar_renovacion(503, &json!(null))).0, "sin_conexion");
        assert_eq!(m(interpretar_renovacion(429, &json!(null))).0, "sin_conexion");
        assert_eq!(m(interpretar_renovacion(200, &json!({}))).0, "rechazado");
    }

    /// Guardar, renovar y quitar, con la carpeta del agente en un temporal (DPAPI de verdad en Windows).
    #[test]
    fn conectar_renovar_y_quitar_guardado() {
        let _l = crate::restic::tests::real_repo_lock();
        let dir = std::env::temp_dir().join(format!("resguardo-nube-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("RESGUARDO_AGENT_DIR", &dir);
        let bien = |_: &str, _: &str| -> Result<Renovado, FalloRenovar> { Ok((AT.into(), chrono::Utc::now() + chrono::Duration::hours(4))) };
        let p = || pedido(json!({ "tipo": "dropbox", "nombre": "Dropbox Sur", "refresh_token": RT, "app_key": KEY })).unwrap();

        // Conectar: se guarda con la app key y el refresh token; el mensaje no los lleva.
        let m = conectar_con(p(), &|k, r| {
            assert_eq!((k, r), (KEY, RT), "renueva con la app key y el refresh token que llegaron");
            bien(k, r)
        })
        .unwrap();
        assert!(m.contains("conectada") && !m.contains(RT) && !m.contains(AT) && !m.contains(KEY), "{m}");
        let n = buscar("Dropbox Sur").unwrap();
        assert_eq!((n.tipo.as_str(), n.app_key.as_deref()), ("dropbox", Some(KEY)));
        let t: serde_json::Value = serde_json::from_str(&n.token).unwrap();
        assert_eq!((t["access_token"].as_str(), t["refresh_token"].as_str(), t["token_type"].as_str()), (Some(AT), Some(RT), Some("bearer")));
        assert!(chrono::DateTime::parse_from_rfc3339(t["expiry"].as_str().unwrap()).is_ok());
        // Lo que se muestra (resumen: guarda_copias.nubes), sin tokens.
        let l = serde_json::to_string(&lista()).unwrap();
        assert_eq!(l, r#"[{"nombre":"Dropbox Sur","tipo":"dropbox"}]"#);
        // En disco, protegido: en Windows, ni el token ni la app key en claro.
        let crudo = std::fs::read(ruta()).unwrap();
        if cfg!(windows) {
            let s = String::from_utf8_lossy(&crudo);
            assert!(!s.contains(RT) && !s.contains(KEY), "DPAPI");
        }

        // Dropbox dice que no: error claro y no se guarda nada nuevo.
        let no = |_: &str, _: &str| -> Result<Renovado, FalloRenovar> { Err(FalloRenovar::Rechazado("Dropbox no aceptó el permiso.".into())) };
        let otro = pedido(json!({ "tipo": "dropbox", "nombre": "Otra", "refresh_token": RT, "app_key": KEY })).unwrap();
        assert_eq!(conectar_con(otro, &no).err().unwrap(), "Dropbox no aceptó el permiso.");
        assert!(buscar("Otra").is_none());
        // Sin conexión: se guarda con el access token que trajo la consola, avisando.
        let sin = |_: &str, _: &str| -> Result<Renovado, FalloRenovar> { Err(FalloRenovar::SinConexion("sin internet".into())) };
        let otra =
            pedido(json!({ "tipo": "dropbox", "nombre": "Otra", "refresh_token": RT, "access_token": AT, "expira": "2099-01-01T00:00:00Z", "app_key": KEY }))
                .unwrap();
        let m = conectar_con(otra, &sin).unwrap();
        assert!(m.contains("No se pudo comprobar") && !m.contains(RT) && !m.contains(AT), "{m}");
        // Volver a conectar con el mismo nombre la sustituye.
        assert!(conectar_con(p(), &bien).unwrap().contains("reconectada"));
        assert_eq!(cargar().len(), 2);

        // Renovar antes de usar: solo si caduca pronto, y se guarda.
        let mut vieja = buscar("Dropbox Sur").unwrap();
        assert_eq!(al_dia(&vieja, &|_, _| panic!("aún vale: no se renueva")).unwrap(), vieja);
        vieja.token = token_rclone("caducado", RT, chrono::Utc::now() - chrono::Duration::minutes(1));
        let nueva = al_dia(&vieja, &|_, _| Ok(("fresco".into(), chrono::Utc::now() + chrono::Duration::hours(4)))).unwrap();
        assert!(nueva.token.contains("\"fresco\"") && nueva.token.contains(RT));
        assert_eq!(buscar("Dropbox Sur").unwrap().token, nueva.token, "guardado");
        let e = al_dia(&vieja, &no).err().unwrap();
        assert!(e.contains("Dropbox Sur") && !e.contains(RT));
        assert!(al_dia(&vieja, &sin).is_err(), "caducado y sin conexión: no se usa");
        // Las de rclone authorize, tal cual.
        let rc = Nube { nombre: "R".into(), tipo: "drive".into(), token: "{}".into(), app_key: None };
        assert_eq!(al_dia(&rc, &|_, _| panic!("no")).unwrap(), rc);

        // Quitar: una que no existe, error; la que usa el espejo sale también del espejo.
        assert!(quitar_desde_orden(&json!({ "nombre": "No existe" })).is_err());
        let mut conf = crate::server::load();
        conf.enabled = true;
        conf.espejo = Some(crate::espejo::Espejo {
            hora: "02:00".into(),
            destinos: vec![
                crate::espejo::Destino { tipo: "nube".into(), carpeta: "Sur".into(), nube: Some("Dropbox Sur".into()), ..Default::default() },
                crate::espejo::Destino { tipo: "carpeta".into(), carpeta: "E:/Espejo".into(), ..Default::default() },
            ],
            ..Default::default()
        });
        crate::server::save(&conf).unwrap();
        assert!(usa_espejo("Dropbox Sur") && !usa_espejo("Otra"));
        let m = quitar_desde_orden(&json!({ "nombre": "Dropbox Sur" })).unwrap();
        assert!(m.contains("desconectada") && m.contains("ya no sube"), "{m}");
        assert!(buscar("Dropbox Sur").is_none() && buscar("Otra").is_some());
        let e = crate::server::load().espejo.unwrap();
        assert_eq!(e.destinos.len(), 1);
        assert_eq!(e.destinos[0].tipo, "carpeta");
        let m = quitar_desde_orden(&json!({ "nombre": "Otra" })).unwrap();
        assert!(!m.contains("espejo"), "{m}");
        assert!(cargar().is_empty());

        std::env::remove_var("RESGUARDO_AGENT_DIR");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rclone_recibe_el_client_id_de_la_app() {
        if comprobar_binario().is_err() {
            return;
        }
        let n = Nube { nombre: "D".into(), tipo: "dropbox".into(), token: token_rclone(AT, RT, chrono::Utc::now()), app_key: Some(KEY.into()) };
        let c = comando(&n, Path::new("vacio.conf")).unwrap();
        let env: Vec<(String, String)> =
            c.get_envs().filter_map(|(k, v)| Some((k.to_string_lossy().into_owned(), v?.to_string_lossy().into_owned()))).collect();
        assert!(env.contains(&("RCLONE_CONFIG_RNUBE_CLIENT_ID".into(), KEY.into())));
        assert!(env.contains(&("RCLONE_CONFIG_RNUBE_TYPE".into(), "dropbox".into())));
        assert!(!env.iter().any(|(k, _)| k.contains("SECRET")), "sin app secret");
        assert!(!c.get_args().any(|a| a.to_string_lossy().contains(RT)), "el token nunca en la línea de órdenes");
    }

    #[test]
    fn quita_las_variables_de_rclone_heredadas() {
        let mut c = std::process::Command::new("rclone");
        let claves = ["RCLONE_DUMP", "rclone_log_file", "RCLONE_CONFIG_OTRA_TOKEN", "PATH", "SYSTEMROOT"].map(std::ffi::OsString::from);
        quitar_heredadas(&mut c, claves.into_iter());
        let quitadas: Vec<String> = c.get_envs().filter(|(_, v)| v.is_none()).map(|(k, _)| k.to_string_lossy().to_uppercase()).collect();
        assert_eq!(quitadas.len(), 3, "{quitadas:?}");
        assert!(quitadas.iter().all(|k| k.starts_with("RCLONE_")));
    }

    #[test]
    fn token_y_argumentos() {
        let salida = "Paste the following into your remote machine --->\n{\"access_token\":\"x\",\"expiry\":\"…\"}\n<---End paste\n";
        assert_eq!(token_de_salida(salida).as_deref(), Some("{\"access_token\":\"x\",\"expiry\":\"…\"}"));
        assert!(token_de_salida("nada").is_none());
        assert!(carpeta_remota_valida("Resguardo/Sur"));
        for mal in ["", "../x", "a//b", "c:\\x", "a/./b"] {
            assert!(!carpeta_remota_valida(mal), "{mal}");
        }
        let a = argumentos_copia(Path::new("D:/almacen"), "/Resguardo/Sur/", Some(512));
        assert_eq!(a[0], "copy", "nunca sync");
        assert!(a.contains(&"rnube:Resguardo/Sur".to_string()) && a.contains(&"--immutable".to_string()) && a.contains(&"512K".to_string()));
        assert!(!a.iter().any(|x| x == "sync" || x.contains("delete")));
        let conf = "[otro]\ntoken = no\n\n[rnube]\ntype = dropbox\ntoken = {\"access_token\":\"nuevo\"}\n";
        assert_eq!(token_de_conf(conf).as_deref(), Some("{\"access_token\":\"nuevo\"}"));
        assert_eq!(token_de_conf("[otro]\ntoken = x\n"), None);
        let err = concat!(
            r#"{"level":"notice","msg":"Config file not found - using defaults"}"#,
            "\n",
            r#"{"level":"error","msg":"data/ab: immutable file modified"}"#,
            "\n",
            r#"{"level":"notice","msg":"4 B / 4 B","stats":{"bytes":3145728,"checks":5,"errors":1,"transfers":2}}"#,
            "\n"
        );
        let (t, e) = resumen_de_rclone(err);
        assert_eq!(t, "2 archivos subidos (3 MB), 5 ya estaban, 1 errores.");
        assert_eq!(e.as_deref(), Some("data/ab: immutable file modified"));
        assert_eq!(resumen_de_rclone("").0, "Nada nuevo que subir.");
    }

    /// Con el rclone de verdad y un remoto «local» (sin cuenta de nube): copia, sin locks ni recientes, y nunca borra.
    #[test]
    fn espejo_con_rclone_y_remoto_local() {
        if comprobar_binario().is_err() {
            eprintln!("Sin rclone en src-tauri/binaries: se salta la prueba.");
            return;
        }
        let base = std::env::temp_dir().join(format!("resguardo-rclone-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (o, d) = (base.join("origen"), base.join("nube"));
        std::fs::create_dir_all(o.join("ana/repo/data/ab")).unwrap();
        std::fs::create_dir_all(o.join("ana/repo/locks")).unwrap();
        std::fs::create_dir_all(&d).unwrap();
        let viejo = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        for (f, contenido) in [("ana/repo/config", "c"), ("ana/repo/data/ab/abcdef", "datos"), ("ana/repo/locks/l1", "lock")] {
            std::fs::write(o.join(f), contenido).unwrap();
            std::fs::File::options().write(true).open(o.join(f)).unwrap().set_modified(viejo).unwrap();
        }
        std::fs::write(o.join("ana/repo/data/ab/reciente"), "nuevo").unwrap();
        // Remoto «local» con la ruta del destino como «carpeta»: el mismo camino que Dropbox, sin cuenta.
        let n = Nube { nombre: "Prueba".into(), tipo: "local".into(), token: String::new(), app_key: None };
        let conf = base.join("vacio.conf");
        let destino = d.display().to_string().replace('\\', "/");
        let args = argumentos_copia(&o, "x", None);
        let mut args: Vec<String> = args.into_iter().map(|a| if a == "rnube:x" { format!("rnube:{destino}") } else { a }).collect();
        args.push("--local-no-check-updated".into());
        let out = comando(&n, &conf).unwrap().args(&args).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        assert_eq!(resumen_de_rclone(&String::from_utf8_lossy(&out.stderr)).0, "2 archivos subidos (0 MB), 0 ya estaban.");
        assert_eq!(std::fs::read_to_string(d.join("ana/repo/data/ab/abcdef")).unwrap(), "datos");
        assert!(d.join("ana/repo/config").is_file());
        assert!(!d.join("ana/repo/locks").exists(), "sin bloqueos");
        assert!(!d.join("ana/repo/data/ab/reciente").exists(), "lo reciente espera");
        assert!(!conf.exists(), "rclone no escribe configuración");
        // Lo borrado en el origen sigue en la nube.
        std::fs::remove_file(o.join("ana/repo/config")).unwrap();
        let out = comando(&n, &conf).unwrap().args(&args).output().unwrap();
        assert!(out.status.success());
        assert!(d.join("ana/repo/config").is_file(), "copy nunca borra");
        // Un archivo que cambia en el origen no se reescribe en la nube (--immutable): error, y la copia sigue intacta.
        let f = o.join("ana/repo/data/ab/abcdef");
        std::fs::write(&f, "otra cosa").unwrap();
        std::fs::File::options().write(true).open(&f).unwrap().set_modified(viejo).unwrap();
        let out = comando(&n, &conf).unwrap().args(&args).output().unwrap();
        let (_, error) = resumen_de_rclone(&String::from_utf8_lossy(&out.stderr));
        assert!(!out.status.success() && error.is_some());
        assert_eq!(std::fs::read_to_string(d.join("ana/repo/data/ab/abcdef")).unwrap(), "datos");
        let _ = std::fs::remove_dir_all(&base);
    }
}
