//! 0.8.1 (H1, plan-mejoras 15mm): que la persona del equipo **recupere sus propios
//! archivos** desde su cuenta, sin la clave de administración ni la contraseña del
//! repositorio: «Versiones anteriores con Resguardo» en el Explorador, «Recuperar un
//! archivo» en la ventana del agente y `resguardo-agente recuperar <ruta>` en Linux.
//! El modelo de amenazas y cómo se comprueba cada salvaguarda: docs/recuperar-en-el-equipo.md.
//!
//! Lo pide la ventana (o la orden de la terminal), **como el usuario**, por el canal local
//! (`ipc_local`, que ya sabe quién está al otro lado por el sistema, nunca por la petición).
//! El servicio lo hace como SYSTEM o root, con estas salvaguardas:
//!
//! 1. **Solo lo que esa cuenta puede leer.** Cada archivo y carpeta se comprueba contra la
//!    cuenta que pide ([`Comprobador`]): en Windows, con el token de identificación de quien
//!    pide por la tubería (`AccessCheck`), contra el descriptor de seguridad que tiene ahora en
//!    el disco **y** contra el que tenía en la versión (restic lo guarda); en Linux, que sea su
//!    dueño (ahora y en la versión). Lo que no pasa no se restaura (`--exclude`) y se cuenta.
//!    Sin datos para decidir, no.
//! 2. **Nunca sobrescribe.** Se restaura en una carpeta privada del agente y se copia, con
//!    archivos y carpetas **nuevos** (`create_new`), junto al original con la fecha en el
//!    nombre («informe (14-10-2026 10.00).xlsx») o en una carpeta elegida; la cuenta tiene que
//!    poder crear archivos ahí, sin enlaces en el camino, y cada cosa creada se comprueba que
//!    quedó donde se esperaba.
//! 3. **Sin secretos ni repositorio.** Solo se ven las versiones de esa ruta (fecha y tamaño);
//!    ni contraseñas, ni el repositorio, ni lo de otras carpetas.
//! 4. **Desactivado por defecto**: por cliente (dato común `ajuste:recuperar`, con la clave
//!    de administración) o por copia (orden `recuperar_en_equipo`, con la clave), con un
//!    límite de tamaño.
//! 5. **A la vista de todas las consolas**: cada recuperación va a la bitácora (`restauracion`
//!    con `cuenta`, `motivo` y `recuperacion: true`), sin rutas.

use crate::servidor_v2::Vinculo;
use resguardo_motor::restic;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// `resumen.admite`: la orden `recuperar_en_equipo`, `resumen.recuperar` y el canal local.
pub const ADMITE: &str = "recuperar_en_equipo";
/// Lo de cada copia (por encima de lo del cliente).
const ARCHIVO: &str = "recuperar.json";
/// Como mucho, carpetas que se recorren para comprobar los permisos de cada cosa.
pub const MAX_CARPETAS: usize = 500;
/// Como mucho, archivos de una recuperación.
pub const MAX_ARCHIVOS: usize = 20_000;
/// Como mucho, versiones que se enseñan.
pub const MAX_VERSIONES: usize = 200;

// ---------- Ajustes ----------

/// Lo que se puso por copia (`<repo>#<copia>` → sí o no). Sin nada, lo del cliente.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct Ajustes {
    #[serde(default)]
    pub copias: BTreeMap<String, bool>,
}

pub fn ajustes() -> Ajustes {
    crate::agent::read_json(ARCHIVO)
}

/// Lo del cliente: (activo, límite en MB). Desactivado por defecto.
pub fn del_cliente(v: Option<&Vinculo>) -> (bool, u64) {
    let a = v.and_then(|v| crate::datos_cliente::ajuste(v, "recuperar"));
    let activo = a.as_ref().and_then(|a| a["activo"].as_bool()).unwrap_or(false);
    let mb = a
        .as_ref()
        .and_then(|a| a["max_mb"].as_u64())
        .filter(|n| (1..=resguardo_protocolo::datos_cliente::RECUPERAR_MAX_MB).contains(n))
        .unwrap_or(resguardo_protocolo::datos_cliente::RECUPERAR_MB_DEFECTO);
    (activo, mb)
}

/// ¿Se puede en alguna de estas copias (`<repo>#<copia>`)? Lo de la copia manda sobre lo del cliente.
pub fn activo_en(copias: &[String], cliente: bool, a: &Ajustes) -> bool {
    copias.iter().any(|k| a.copias.get(k).copied().unwrap_or(cliente))
}

fn clave_copia(repo: &str, copia: &str) -> String {
    format!("{repo}#{copia}")
}

fn id_valido(x: &str) -> bool {
    !x.is_empty() && x.len() <= 64 && x.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// La orden `recuperar_en_equipo { repo, copia, activo: true | false | null }` (con la clave
/// de administración): sí o no en esa copia, o `null` para seguir lo del cliente.
pub fn cambiar(c: &Value) -> Result<String, String> {
    let (repo, copia) = (c["repo"].as_str().unwrap_or(""), c["copia"].as_str().unwrap_or(""));
    if !id_valido(repo) || !id_valido(copia) {
        return Err("Copia no válida.".into());
    }
    let config = crate::agent::load_config();
    let plan = config.repos.iter().find(|r| r.id == repo).and_then(|r| r.plans.iter().find(|p| p.id == copia)).ok_or("Esa copia no está en este equipo.")?;
    let mut a = ajustes();
    let k = clave_copia(repo, copia);
    let m = match c.get("activo") {
        Some(Value::Bool(true)) => {
            a.copias.insert(k, true);
            format!("Cada persona puede recuperar en este equipo sus propios archivos de «{}».", plan.name)
        }
        Some(Value::Bool(false)) => {
            a.copias.insert(k, false);
            format!("Nadie recupera desde este equipo los archivos de «{}».", plan.name)
        }
        None | Some(Value::Null) => {
            a.copias.remove(&k);
            format!("«{}» sigue lo del cliente para recuperar en el equipo.", plan.name)
        }
        Some(_) => return Err("Activo: sí, no o lo del cliente.".into()),
    };
    a.copias.retain(|k, _| config.repos.iter().any(|r| r.plans.iter().any(|p| clave_copia(&r.id, &p.id) == *k)));
    crate::agent::write_json(ARCHIVO, &a)?;
    sincronizar_menu(Some(&a));
    Ok(m)
}

/// `resumen.recuperar`: lo del cliente que llegó y lo de cada copia (sin rutas).
pub fn resumen(v: &Vinculo) -> Value {
    let (activo, max_mb) = del_cliente(Some(v));
    json!({ "cliente": activo, "max_mb": max_mb, "copias": ajustes().copias })
}

/// ¿Está activo en alguna copia de este equipo? (Para el menú del Explorador.)
pub fn activo_en_alguna(v: Option<&Vinculo>, a: &Ajustes) -> bool {
    let (cliente, _) = del_cliente(v);
    let config = crate::agent::load_config();
    let todas: Vec<String> = config.repos.iter().flat_map(|r| r.plans.iter().map(move |p| clave_copia(&r.id, &p.id))).collect();
    activo_en(&todas, cliente, a)
}

// ---------- Rutas ----------

/// Una ruta del equipo que pide la persona: absoluta, de un disco de este equipo (en
/// Windows `X:\…`, nada de red ni `\\?\`), sin `.`, `..`, flujos (`a:b`), nombres de
/// dispositivo ni nombres que Windows recorta (`a.`, `a `). Con `/` también vale en Windows.
pub fn ruta_valida(r: &str) -> Result<PathBuf, String> {
    let mal = || "Esa ruta no vale: escribe la ruta entera de un archivo o una carpeta de este equipo.".to_string();
    let r = r.trim();
    if r.is_empty() || r.len() > 1024 || r.chars().any(char::is_control) {
        return Err(mal());
    }
    if cfg!(windows) {
        let r = r.replace('/', "\\");
        let b = r.as_bytes();
        if b.len() < 3 || !b[0].is_ascii_alphabetic() || b[1] != b':' || b[2] != b'\\' {
            return Err(mal());
        }
        let resto = r[3..].trim_end_matches('\\');
        if resto.is_empty() {
            return Err("Elige un archivo o una carpeta, no una unidad entera.".into());
        }
        let malo = |c: &str| {
            c.is_empty()
                || c == "."
                || c == ".."
                || c.ends_with(['.', ' '])
                || c.contains([':', '*', '?', '"', '<', '>', '|'])
                || crate::sesiones_v2::es_dispositivo(c)
        };
        if resto.split('\\').any(malo) {
            return Err(mal());
        }
        Ok(PathBuf::from(format!("{}:\\{resto}", b[0].to_ascii_uppercase() as char)))
    } else {
        if !r.starts_with('/') || r.starts_with("//") {
            return Err(mal());
        }
        let resto = r.trim_end_matches('/');
        if resto.is_empty() || resto[1..].split('/').any(|c| c.is_empty() || c == "." || c == "..") {
            return Err(mal());
        }
        Ok(PathBuf::from(resto))
    }
}

/// La ruta del equipo como ruta de la versión (`C:\Users\ana\x` → `/C/Users/ana/x`).
pub fn a_version(p: &Path) -> String {
    let s = p.to_string_lossy();
    if cfg!(windows) {
        let b = s.as_bytes();
        if b.len() >= 2 && b[1] == b':' {
            return format!("/{}/{}", &s[..1], s[2..].trim_start_matches('\\').replace('\\', "/")).trim_end_matches('/').to_string();
        }
    }
    s.to_string()
}

/// La ruta del equipo de una ruta de la versión (al revés que [`a_version`]).
pub fn a_local(v: &str) -> Option<PathBuf> {
    crate::sesiones_v2::ruta_local(v).ok().map(PathBuf::from)
}

fn normal(p: &str) -> String {
    let s = p.trim_end_matches(['\\', '/']).replace('/', if cfg!(windows) { "\\" } else { "/" });
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s
    }
}

/// ¿Está `ruta` dentro de `carpeta` (o es ella)?
pub fn dentro(ruta: &Path, carpeta: &str) -> bool {
    let (r, c) = (normal(&ruta.to_string_lossy()), normal(carpeta));
    let sep = if cfg!(windows) { '\\' } else { '/' };
    !c.is_empty() && (r == c || r.starts_with(&format!("{c}{sep}")))
}

/// Las copias (`<repo>#<copia>`) que copian `ruta`, con su repositorio.
pub fn copias_de(ruta: &Path) -> Vec<(String, String)> {
    let config = crate::agent::load_config();
    let mut out = Vec::new();
    for r in &config.repos {
        for p in &r.plans {
            if p.paths.iter().any(|c| dentro(ruta, c)) {
                out.push((r.id.clone(), clave_copia(&r.id, &p.id)));
            }
        }
    }
    out
}

/// El nombre de lo recuperado junto al original, con la fecha de la versión:
/// «informe (14-10-2026 10.00).xlsx», «Docs (14-10-2026 10.00)»; con `n` > 1, « (n)».
pub fn nombre_con_fecha(nombre: &str, carpeta: bool, fecha: &str, n: u32) -> String {
    let extra = if n > 1 { format!(" ({n})") } else { String::new() };
    let (base, ext) = match nombre.rsplit_once('.') {
        Some((b, e)) if !carpeta && !b.is_empty() && !e.is_empty() && e.len() <= 10 && !e.contains(' ') => (b, format!(".{e}")),
        _ => (nombre, String::new()),
    };
    format!("{base} ({fecha}){extra}{ext}")
}

/// «14-10-2026 10.00» (la hora local de la versión; sin `:`, que Windows no admite en un nombre).
pub fn fecha_de(t: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(t)
        .map(|d| d.with_timezone(&chrono::Local).format("%d-%m-%Y %H.%M").to_string())
        .unwrap_or_else(|_| chrono::Local::now().format("%d-%m-%Y %H.%M").to_string())
}

// ---------- Quién puede leer qué ----------

/// Lo que dice una comprobación: sí, no o «no se sabe» (no existe, o sin datos).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lectura {
    Si,
    No,
    NoSe,
}

/// Comprueba con la cuenta que pide (nunca con la del servicio).
pub trait Comprobador {
    /// ¿Puede leer ahora `p` en el disco? `NoSe` si no existe.
    fn leer_ahora(&self, p: &Path) -> Lectura;
    /// ¿Podía leerlo en la versión? (El nodo de `restic cat tree`.) `NoSe` sin datos.
    fn leer_en_version(&self, nodo: &Value) -> Lectura;
    /// ¿Puede crear archivos y carpetas en `dir`?
    fn escribir(&self, dir: &Path) -> bool;
}

/// Se recupera solo si lo dejan **las dos** cosas que se saben (ahora y en la versión), y
/// alguna se sabe: un «no» en cualquiera, o nada que mirar, es que no.
pub fn permitido(c: &dyn Comprobador, local: &Path, nodo: &Value) -> bool {
    if crate::platform::hay_enlace_en_el_camino(local) {
        return false;
    }
    match (c.leer_ahora(local), c.leer_en_version(nodo)) {
        (Lectura::No, _) | (_, Lectura::No) => false,
        (Lectura::Si, _) | (_, Lectura::Si) => true,
        (Lectura::NoSe, Lectura::NoSe) => false,
    }
}

/// Un comprobador que nunca deja nada (sin poder identificar a quien pide).
pub struct Nadie;
impl Comprobador for Nadie {
    fn leer_ahora(&self, _: &Path) -> Lectura {
        Lectura::No
    }
    fn leer_en_version(&self, _: &Value) -> Lectura {
        Lectura::No
    }
    fn escribir(&self, _: &Path) -> bool {
        false
    }
}

/// En Linux: es su dueño (ahora y en la versión). root puede todo (ya podía).
#[cfg(unix)]
pub struct PorDueno(pub u32);

#[cfg(unix)]
impl Comprobador for PorDueno {
    fn leer_ahora(&self, p: &Path) -> Lectura {
        use std::os::unix::fs::MetadataExt;
        match std::fs::symlink_metadata(p) {
            Ok(m) if m.file_type().is_symlink() => Lectura::No,
            Ok(m) if self.0 == 0 || m.uid() == self.0 => Lectura::Si,
            Ok(_) => Lectura::No,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Lectura::NoSe,
            Err(_) => Lectura::No,
        }
    }
    fn leer_en_version(&self, nodo: &Value) -> Lectura {
        match nodo["uid"].as_u64() {
            Some(u) if self.0 == 0 || u == u64::from(self.0) => Lectura::Si,
            Some(_) => Lectura::No,
            None => Lectura::NoSe,
        }
    }
    fn escribir(&self, dir: &Path) -> bool {
        use std::os::unix::fs::MetadataExt;
        std::fs::symlink_metadata(dir).is_ok_and(|m| m.is_dir() && (self.0 == 0 || m.uid() == self.0))
    }
}

/// El comprobador de quien pide por el canal local.
pub fn comprobador_de(cliente: &crate::ipc_local::Cliente) -> Box<dyn Comprobador> {
    #[cfg(windows)]
    {
        match cliente.token.clone() {
            Some(t) => Box::new(win::PorToken(t)),
            None => Box::new(Nadie),
        }
    }
    #[cfg(unix)]
    {
        match cliente.id.strip_prefix("uid:").and_then(|u| u.parse::<u32>().ok()) {
            Some(u) => Box::new(PorDueno(u)),
            None => Box::new(Nadie),
        }
    }
}

#[cfg(windows)]
pub mod win {
    //! `AccessCheck` con el token de identificación de quien pide por la tubería.
    use super::{Comprobador, Lectura};
    use base64::Engine;
    use serde_json::Value;
    use std::path::Path;
    use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, HANDLE};
    use windows_sys::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{
        AccessCheck, IsValidSecurityDescriptor, DACL_SECURITY_INFORMATION, GENERIC_MAPPING, GROUP_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION,
        PRIVILEGE_SET, PSECURITY_DESCRIPTOR,
    };

    /// Permisos de archivo (winnt.h), aquí para no depender de más módulos.
    const FILE_READ_DATA: u32 = 0x1;
    const FILE_ADD_FILE: u32 = 0x2;
    const FILE_ADD_SUBDIRECTORY: u32 = 0x4;
    const FILE_GENERIC_READ: u32 = 0x0012_0089;
    const FILE_GENERIC_WRITE: u32 = 0x0012_0116;
    const FILE_GENERIC_EXECUTE: u32 = 0x0012_00A0;
    const FILE_ALL_ACCESS: u32 = 0x001F_01FF;

    /// El token (de identificación) de quien pide. Se cierra al soltarlo.
    pub struct Token(usize);

    impl Token {
        /// Toma un token ya abierto (con `TOKEN_QUERY`).
        ///
        /// # Safety
        /// `h` tiene que ser un token válido que nadie más cierra.
        pub unsafe fn de(h: HANDLE) -> Token {
            Token(h as usize)
        }
        fn h(&self) -> HANDLE {
            self.0 as HANDLE
        }
    }

    impl Drop for Token {
        fn drop(&mut self) {
            // SAFETY: el token es nuestro y solo se cierra aquí.
            unsafe { CloseHandle(self.h()) };
        }
    }

    impl PartialEq for Token {
        fn eq(&self, o: &Self) -> bool {
            self.0 == o.0
        }
    }

    impl std::fmt::Debug for Token {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("Token")
        }
    }

    // SAFETY: un token es un handle del núcleo: se puede usar desde cualquier hilo.
    unsafe impl Send for Token {}
    unsafe impl Sync for Token {}

    /// `AccessCheck` de `deseado` en `sd`: `None` si no se pudo mirar (descriptor sin dueño, roto…).
    fn comprobar(t: &Token, sd: PSECURITY_DESCRIPTOR, deseado: u32) -> Option<bool> {
        let mapa = GENERIC_MAPPING {
            GenericRead: FILE_GENERIC_READ,
            GenericWrite: FILE_GENERIC_WRITE,
            GenericExecute: FILE_GENERIC_EXECUTE,
            GenericAll: FILE_ALL_ACCESS,
        };
        let mut privs = [0u64; 32];
        let mut largo = std::mem::size_of_val(&privs) as u32;
        let (mut dado, mut estado) = (0u32, 0i32);
        // SAFETY: `sd` es un descriptor válido mientras dura la llamada; todo lo demás, local.
        let ok = unsafe { AccessCheck(sd, t.h(), deseado, &mapa, privs.as_mut_ptr() as *mut PRIVILEGE_SET, &mut largo, &mut dado, &mut estado) };
        (ok != 0).then_some(estado != 0 && dado & deseado == deseado)
    }

    fn ancho(p: &Path) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        p.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
    }

    /// El descriptor de seguridad de `p` en el disco y `AccessCheck` con él.
    fn en_disco(t: &Token, p: &Path, deseado: u32) -> Lectura {
        let w = ancho(p);
        let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        // SAFETY: cadena terminada en cero; `sd` se libera con LocalFree.
        let r = unsafe {
            GetNamedSecurityInfoW(
                w.as_ptr(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut sd,
            )
        };
        if r == ERROR_FILE_NOT_FOUND || r == ERROR_PATH_NOT_FOUND {
            return Lectura::NoSe;
        }
        if r != 0 || sd.is_null() {
            return Lectura::No;
        }
        let x = comprobar(t, sd, deseado);
        // SAFETY: lo reservó GetNamedSecurityInfoW.
        unsafe { LocalFree(sd as _) };
        match x {
            Some(true) => Lectura::Si,
            _ => Lectura::No,
        }
    }

    /// El de quien pide.
    pub struct PorToken(pub std::sync::Arc<Token>);

    impl Comprobador for PorToken {
        fn leer_ahora(&self, p: &Path) -> Lectura {
            en_disco(&self.0, p, FILE_READ_DATA)
        }
        fn leer_en_version(&self, nodo: &Value) -> Lectura {
            let Some(b) =
                nodo["generic_attributes"]["windows.security_descriptor"].as_str().and_then(|s| base64::engine::general_purpose::STANDARD.decode(s).ok())
            else {
                return Lectura::NoSe;
            };
            if b.len() < 20 || b.len() > 64 * 1024 {
                return Lectura::NoSe;
            }
            // El descriptor (autorrelativo) alineado en memoria propia.
            let mut buf = vec![0u64; b.len().div_ceil(8)];
            // SAFETY: `buf` tiene al menos `b.len()` bytes.
            unsafe { std::ptr::copy_nonoverlapping(b.as_ptr(), buf.as_mut_ptr() as *mut u8, b.len()) };
            let sd = buf.as_mut_ptr() as PSECURITY_DESCRIPTOR;
            // SAFETY: `sd` apunta a `buf`, vivo durante las llamadas.
            if unsafe { IsValidSecurityDescriptor(sd) } == 0 {
                return Lectura::NoSe;
            }
            match comprobar(&self.0, sd, FILE_READ_DATA) {
                Some(true) => Lectura::Si,
                Some(false) => Lectura::No,
                None => Lectura::NoSe,
            }
        }
        fn escribir(&self, dir: &Path) -> bool {
            en_disco(&self.0, dir, FILE_ADD_FILE | FILE_ADD_SUBDIRECTORY) == Lectura::Si
        }
    }

    /// Para las pruebas: el token de este proceso, a nivel de identificación (como el de la tubería).
    pub fn token_propio() -> Option<std::sync::Arc<Token>> {
        use windows_sys::Win32::Security::{DuplicateToken, SecurityIdentification, TOKEN_DUPLICATE, TOKEN_QUERY};
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
        let (mut h, mut d): (HANDLE, HANDLE) = (std::ptr::null_mut(), std::ptr::null_mut());
        // SAFETY: salidas locales; el primario se cierra y el duplicado pasa a `Token`.
        unsafe {
            if OpenProcessToken(GetCurrentProcess(), TOKEN_DUPLICATE | TOKEN_QUERY, &mut h) == 0 {
                return None;
            }
            let ok = DuplicateToken(h, SecurityIdentification, &mut d) != 0;
            CloseHandle(h);
            ok.then(|| std::sync::Arc::new(Token::de(d)))
        }
    }
}

// ---------- Versiones de una ruta ----------

/// Una versión en la que está la ruta.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EnVersion {
    pub version: String,
    pub cuando: String,
    pub carpeta: bool,
    pub bytes: Option<u64>,
    pub modificado: Option<String>,
    /// La copia de qué repositorio (si hay más de uno que la copia).
    #[serde(skip)]
    pub repo: String,
}

/// Lee `restic find --json` y deja la ruta exacta (archivo o carpeta) en cada versión.
pub fn leer_find(salida: &[u8], ruta: &str, versiones: &[restic::Snapshot], repo: &str) -> Vec<EnVersion> {
    let v: Value = serde_json::from_slice(salida).unwrap_or(Value::Null);
    let mut out = Vec::new();
    for snap in v.as_array().into_iter().flatten() {
        let id = snap["snapshot"].as_str().unwrap_or_default();
        let Some(s) = versiones.iter().find(|s| s.id == id || s.short_id == id || (id.len() >= 8 && s.id.starts_with(id))) else { continue };
        let Some(m) = snap["matches"].as_array().into_iter().flatten().find(|m| m["path"].as_str() == Some(ruta)) else { continue };
        out.push(EnVersion {
            version: s.id.clone(),
            cuando: s.time.clone(),
            carpeta: m["type"] == "dir",
            bytes: m["size"].as_u64(),
            modificado: m["mtime"].as_str().map(str::to_string),
            repo: repo.to_string(),
        });
    }
    out
}

/// El nodo de `ruta` (de la versión) y, si es una carpeta, todo lo de dentro, con lo que
/// restic guardó de cada uno (descriptor de seguridad en Windows, dueño en Linux), por
/// `restic cat tree`. Como mucho [`MAX_CARPETAS`] carpetas y [`MAX_ARCHIVOS`] archivos.
pub fn nodos(acc: &restic::Access, version: &str, ruta: &str) -> Result<Vec<(String, Value)>, String> {
    let arbol = |dir: &str| -> Result<Vec<Value>, String> {
        let out = restic::run_with(acc, &["cat", "tree", "--no-lock", &format!("{version}:{dir}")], Duration::from_secs(600), None)?;
        let v: Value = serde_json::from_slice(&out).map_err(|_| "No se pudo leer la versión.".to_string())?;
        Ok(v["nodes"].as_array().cloned().unwrap_or_default())
    };
    let (padre, nombre) = ruta.rsplit_once('/').ok_or("Ruta no válida.")?;
    let padre = if padre.is_empty() { "/" } else { padre };
    let raiz = arbol(padre)?.into_iter().find(|n| n["name"].as_str() == Some(nombre)).ok_or("Eso no está en esa versión.")?;
    let mut out = vec![(ruta.to_string(), raiz.clone())];
    if raiz["type"] != "dir" {
        return Ok(out);
    }
    let mut pendientes = vec![ruta.to_string()];
    let mut carpetas = 0;
    while let Some(d) = pendientes.pop() {
        carpetas += 1;
        if carpetas > MAX_CARPETAS || out.len() > MAX_ARCHIVOS {
            return Err(
                "Esa carpeta tiene demasiadas cosas para recuperarla desde aquí: pide a quien administra las copias que la restaure desde la consola.".into()
            );
        }
        for n in arbol(&d)? {
            let Some(nm) = n["name"].as_str().filter(|x| !x.is_empty() && *x != "." && *x != ".." && !x.contains(['/', '\0'])) else { continue };
            let r = format!("{d}/{nm}");
            if n["type"] == "dir" {
                pendientes.push(r.clone());
            }
            out.push((r, n));
        }
    }
    Ok(out)
}

/// Qué se recupera de `nodos`: lo permitido (con su tamaño) y lo que se quita (lo más alto
/// de cada rama no permitida). `Err` si no se permite ni lo de arriba.
pub fn decidir(c: &dyn Comprobador, nodos: &[(String, Value)]) -> Result<(u64, u64, Vec<String>), String> {
    let mut quitadas: Vec<String> = Vec::new();
    let (mut archivos, mut bytes) = (0u64, 0u64);
    for (i, (r, n)) in nodos.iter().enumerate() {
        if quitadas.iter().any(|q| r.starts_with(&format!("{q}/"))) {
            continue;
        }
        let Some(local) = a_local(r) else {
            quitadas.push(r.clone());
            continue;
        };
        if !permitido(c, &local, n) {
            if i == 0 {
                return Err("Solo puedes recuperar lo que tu cuenta puede abrir, y esto no lo es (o ya no lo era en esa versión).".into());
            }
            quitadas.push(r.clone());
            continue;
        }
        if n["type"] == "file" {
            archivos += 1;
            bytes += n["size"].as_u64().unwrap_or(0);
        }
    }
    Ok((archivos, bytes, quitadas))
}

// ---------- Lo que pide la ventana ----------

static EN_MARCHA: AtomicBool = AtomicBool::new(false);

/// El repositorio y las copias que cubren `ruta`, si está activo en alguna. Si no, por qué no.
fn preparar(v: Option<&Vinculo>, ruta: &Path) -> Result<(String, u64), String> {
    let copias = copias_de(ruta);
    if copias.is_empty() {
        return Err("Esa carpeta no está en las copias de este equipo: no hay versiones anteriores que recuperar.".into());
    }
    let (cliente, max_mb) = del_cliente(v);
    let a = ajustes();
    let activas: Vec<&(String, String)> = copias.iter().filter(|(_, k)| a.copias.get(k).copied().unwrap_or(cliente)).collect();
    let Some((repo, _)) = activas.first() else {
        return Err(
            "Recuperar desde el equipo no está activado para estas copias. Pide a quien administra las copias que lo active o que te lo restaure.".into()
        );
    };
    Ok((repo.clone(), max_mb))
}

/// `recuperar_estado { ruta? }`: si se puede recuperar aquí (y esa ruta), el límite y si el motivo es obligatorio.
pub fn estado(c: &Value) -> Value {
    let v = crate::servidor_v2::cargar();
    let (cliente, max_mb) = del_cliente(v.as_ref());
    let a = ajustes();
    let alguna = activo_en_alguna(v.as_ref(), &a);
    let motivo_obligatorio =
        v.as_ref().and_then(|v| crate::datos_cliente::ajuste(v, "restauraciones")).and_then(|x| x["motivo_obligatorio"].as_bool()).unwrap_or(true);
    let mut out = json!({ "activo": alguna, "cliente": cliente, "max_mb": max_mb, "motivo_obligatorio": motivo_obligatorio });
    if let Some(r) = c["ruta"].as_str().filter(|r| !r.trim().is_empty()) {
        out["ruta"] = match ruta_valida(r).and_then(|p| preparar(v.as_ref(), &p)) {
            Ok(_) => json!({ "se_puede": true }),
            Err(e) => json!({ "se_puede": false, "motivo": e }),
        };
    }
    out
}

/// `recuperar_versiones { ruta }`: las versiones de esa ruta (fecha, tamaño, si es carpeta),
/// solo si la cuenta puede leerla (ahora o, si ya no está, en la más reciente).
pub fn versiones(c: &Value, quien: &dyn Comprobador) -> Result<Value, String> {
    let ruta = ruta_valida(c["ruta"].as_str().unwrap_or(""))?;
    let v = crate::servidor_v2::cargar().ok_or("Este equipo aún no está configurado.")?;
    let (repo, max_mb) = preparar(Some(&v), &ruta)?;
    let rv = a_version(&ruta);
    if !resguardo_motor::diferencias::ruta_valida(&rv) {
        return Err("Esa ruta no vale.".into());
    }
    // Antes de preguntar al repositorio: si existe, que la pueda leer ahora.
    if quien.leer_ahora(&ruta) == Lectura::No || crate::platform::hay_enlace_en_el_camino(&ruta) {
        return Err("Solo puedes recuperar lo que tu cuenta puede abrir.".into());
    }
    let acc = crate::gestion_v2::acceso(&v, &repo)?;
    let snaps = restic::snapshots(&acc)?;
    let patron = resguardo_motor::diferencias::patron_find(&rv);
    let out = restic::run_with(&acc, &["find", "--json", "--no-lock", "--", &patron], Duration::from_secs(1800), None)?;
    let mut lista = leer_find(&out, &rv, &snaps, &repo);
    lista.sort_by(|a, b| b.cuando.cmp(&a.cuando));
    lista.truncate(MAX_VERSIONES);
    // Si ya no está en el disco: la más reciente dice si era suya.
    if quien.leer_ahora(&ruta) == Lectura::NoSe {
        let Some(u) = lista.first() else { return Ok(json!({ "versiones": [], "max_mb": max_mb })) };
        let n = nodos_raiz(&acc, &u.version, &rv)?;
        if quien.leer_en_version(&n) != Lectura::Si {
            return Err("Solo puedes recuperar lo que tu cuenta puede abrir.".into());
        }
    }
    Ok(json!({ "versiones": lista, "max_mb": max_mb, "existe": ruta.exists() }))
}

fn nodos_raiz(acc: &restic::Access, version: &str, ruta: &str) -> Result<Value, String> {
    let (padre, nombre) = ruta.rsplit_once('/').ok_or("Ruta no válida.")?;
    let padre = if padre.is_empty() { "/" } else { padre };
    let out = restic::run_with(acc, &["cat", "tree", "--no-lock", &format!("{version}:{padre}")], Duration::from_secs(600), None)?;
    let v: Value = serde_json::from_slice(&out).map_err(|_| "No se pudo leer la versión.".to_string())?;
    v["nodes"].as_array().into_iter().flatten().find(|n| n["name"].as_str() == Some(nombre)).cloned().ok_or_else(|| "Eso no está en esa versión.".into())
}

/// Lo que se recuperó.
#[derive(Debug, Default)]
pub struct Hecho {
    pub donde: PathBuf,
    pub archivos: u64,
    pub bytes: u64,
    pub sin_permiso: usize,
}

/// `recuperar { ruta, version, motivo: { tipo, notas? }, carpeta? }`: lo recupera junto al
/// original (o en `carpeta`), nunca encima, solo lo que la cuenta puede leer. Lo anota.
pub fn recuperar(c: &Value, quien: &dyn Comprobador, cuenta: &str) -> Result<Value, String> {
    if EN_MARCHA.swap(true, Ordering::SeqCst) {
        return Err("Ya hay una recuperación en marcha en este equipo: espera a que termine.".into());
    }
    struct Suelta;
    impl Drop for Suelta {
        fn drop(&mut self) {
            EN_MARCHA.store(false, Ordering::SeqCst);
        }
    }
    let _s = Suelta;
    let inicio = chrono::Local::now();
    let v = crate::servidor_v2::cargar().ok_or("Este equipo aún no está configurado.")?;
    let ruta = ruta_valida(c["ruta"].as_str().unwrap_or(""))?;
    let (repo, max_mb) = preparar(Some(&v), &ruta)?;
    let motivo = crate::restaurar_v2::motivo(c);
    let obligatorio = crate::datos_cliente::ajuste(&v, "restauraciones").and_then(|x| x["motivo_obligatorio"].as_bool()).unwrap_or(true);
    if obligatorio && motivo.is_none() {
        return Err("Di por qué recuperas (un toque basta).".into());
    }
    let version = c["version"].as_str().unwrap_or("");
    if !restic::valid_snapshot_id(version) {
        return Err("Versión no válida.".into());
    }
    let r = (|| -> Result<Hecho, String> {
        let acc = crate::gestion_v2::acceso(&v, &repo)?;
        let snaps = restic::snapshots(&acc)?;
        let snap = snaps.iter().find(|s| s.id == version || s.short_id == version).ok_or("Esa versión ya no está.")?;
        hacer(&acc, &snap.id, &snap.time, &ruta, c["carpeta"].as_str(), quien, max_mb)
    })();
    anotar(&repo, version, &ruta, c, &motivo, cuenta, inicio, &r);
    let h = r?;
    crate::agent::log(&format!(
        "Recuperar en el equipo: la cuenta «{cuenta}» recuperó {} archivo(s){}.",
        h.archivos,
        if h.sin_permiso > 0 { format!(" (sin {} que no puede leer)", h.sin_permiso) } else { String::new() }
    ));
    Ok(json!({ "donde": h.donde.display().to_string(), "archivos": h.archivos, "bytes": h.bytes, "sin_permiso": h.sin_permiso }))
}

/// Lo de [`recuperar`] con la versión ya elegida.
pub fn hacer(
    acc: &restic::Access,
    version: &str,
    cuando: &str,
    ruta: &Path,
    carpeta: Option<&str>,
    quien: &dyn Comprobador,
    max_mb: u64,
) -> Result<Hecho, String> {
    let rv = a_version(ruta);
    let todo = nodos(acc, version, &rv)?;
    let (archivos, bytes, quitadas) = decidir(quien, &todo)?;
    if bytes > max_mb * 1024 * 1024 {
        return Err(format!(
            "Son {} y desde aquí se recupera como mucho {max_mb} MB: pide a quien administra las copias que lo restaure desde la consola.",
            crate::tasks::human_bytes(bytes)
        ));
    }
    // Dónde: junto al original o en la carpeta elegida; la cuenta tiene que poder crear ahí.
    let dir = match carpeta.map(str::trim).filter(|x| !x.is_empty()) {
        Some(d) => {
            crate::sesiones_v2::forma_de_carpeta(d)?;
            crate::sesiones_v2::carpeta_destino(d)?
        }
        None => ruta.parent().map(Path::to_path_buf).ok_or("Esa ruta no tiene carpeta.")?,
    };
    if !dir.is_dir() || crate::platform::hay_enlace_en_el_camino(&dir) {
        return Err("La carpeta donde dejarlo ya no existe o es un enlace: elige otra carpeta tuya.".into());
    }
    if !quien.escribir(&dir) {
        return Err(format!("No puedes crear archivos en «{}»: elige otra carpeta tuya.", dir.display()));
    }
    let real_dir = std::fs::canonicalize(&dir).map_err(|e| format!("No se pudo mirar la carpeta: {e}"))?;
    let nombre = ruta.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or("Ruta no válida.")?;
    let carpeta_v = todo.first().is_some_and(|(_, n)| n["type"] == "dir");
    // Se restaura primero en una carpeta privada del agente (solo SYSTEM y administradores).
    let privada = crate::agent::private_dir().join("recuperar").join(uuid::Uuid::new_v4().simple().to_string());
    std::fs::create_dir_all(&privada).map_err(|e| format!("No se pudo preparar la recuperación: {e}"))?;
    struct Limpia(PathBuf);
    impl Drop for Limpia {
        fn drop(&mut self) {
            if std::fs::remove_dir_all(&self.0).is_err() {
                crate::restore_test::clear_readonly(&self.0);
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }
    let _l = Limpia(privada.clone());
    let (padre_v, _) = rv.rsplit_once('/').ok_or("Ruta no válida.")?;
    crate::restaurar_v2::restaurar_grupo(
        acc,
        version,
        padre_v,
        std::slice::from_ref(&nombre),
        &privada,
        &quitadas,
        Some("never"),
        None,
        crate::restaurar_v2::Cifras::default(),
        Duration::from_secs(6 * 3600),
    )?;
    let origen = privada.join(&nombre);
    let fecha = fecha_de(cuando);
    let mut destino = None;
    for n in 1..=50 {
        let d = dir.join(nombre_con_fecha(&nombre, carpeta_v, &fecha, n));
        if std::fs::symlink_metadata(&d).is_err() {
            destino = Some(d);
            break;
        }
    }
    let destino = destino.ok_or("Ya hay demasiadas copias recuperadas con esa fecha: borra alguna.")?;
    let mut h = Hecho { donde: destino.clone(), sin_permiso: quitadas.len(), ..Default::default() };
    if let Err(e) = copiar(&origen, &destino, &real_dir, &dir, &mut h) {
        // A medias no sirve: fuera lo que se creó (solo eso: es nuevo).
        if destino.is_dir() {
            let _ = std::fs::remove_dir_all(&destino);
        } else {
            let _ = std::fs::remove_file(&destino);
        }
        return Err(e);
    }
    let _ = (archivos, bytes);
    Ok(h)
}

/// Copia `de` (lo restaurado en la carpeta privada) a `a`, **nuevo**: nada existente se
/// toca. Tras crear cada cosa, comprueba que quedó dentro de `real` (la ruta real de la
/// carpeta de destino): si alguien cambió una carpeta por un enlace a mitad, se para.
fn copiar(de: &Path, a: &Path, real: &Path, dir: &Path, h: &mut Hecho) -> Result<(), String> {
    let m = std::fs::symlink_metadata(de).map_err(|e| format!("No se pudo leer lo restaurado: {e}"))?;
    if m.file_type().is_symlink() {
        // Los enlaces no se recuperan desde aquí (podrían llevar fuera de lo suyo).
        return Ok(());
    }
    let comprobar = |p: &Path| -> Result<(), String> {
        let r = std::fs::canonicalize(p).map_err(|e| format!("No se pudo comprobar lo recuperado: {e}"))?;
        if !r.starts_with(real) || crate::platform::hay_enlace_en_el_camino(p) {
            return Err("La carpeta de destino cambió mientras se recuperaba: se ha parado.".into());
        }
        Ok(())
    };
    if m.is_dir() {
        std::fs::create_dir(a).map_err(|e| format!("No se pudo crear la carpeta: {e}"))?;
        comprobar(a)?;
        dueno(a, dir);
        let mut hijos: Vec<_> = std::fs::read_dir(de).map_err(|e| e.to_string())?.flatten().collect();
        hijos.sort_by_key(|e| e.file_name());
        for e in hijos {
            copiar(&e.path(), &a.join(e.file_name()), real, dir, h)?;
        }
    } else if m.is_file() {
        let mut f = std::fs::File::open(de).map_err(|e| format!("No se pudo leer lo restaurado: {e}"))?;
        let mut g = std::fs::OpenOptions::new().write(true).create_new(true).open(a).map_err(|e| format!("No se pudo crear el archivo: {e}"))?;
        comprobar(a)?;
        h.bytes += std::io::copy(&mut f, &mut g).map_err(|e| format!("No se pudo escribir: {e}"))?;
        h.archivos += 1;
        if let Ok(t) = m.modified() {
            let _ = g.set_modified(t);
        }
        drop(g);
        dueno(a, dir);
    }
    Ok(())
}

/// En Linux, lo recuperado es de quien es la carpeta donde se deja (su dueño), no de root.
#[cfg(unix)]
fn dueno(p: &Path, dir: &Path) {
    use std::os::unix::fs::MetadataExt;
    if let Ok(m) = std::fs::metadata(dir) {
        let _ = std::os::unix::fs::lchown(p, Some(m.uid()), Some(m.gid()));
    }
}

/// En Windows hereda los permisos de la carpeta donde se deja (como cualquier archivo nuevo).
#[cfg(windows)]
fn dueno(_: &Path, _: &Path) {}

/// La entrada `restauracion` de la bitácora (la ven todas las consolas): con la cuenta que la
/// pidió y su motivo; sin rutas.
#[allow(clippy::too_many_arguments)]
fn anotar(
    repo: &str,
    version: &str,
    ruta: &Path,
    c: &Value,
    motivo: &Option<Value>,
    cuenta: &str,
    inicio: chrono::DateTime<chrono::Local>,
    r: &Result<Hecho, String>,
) {
    let fin = chrono::Local::now();
    let _ = ruta;
    let (resultado, mensaje) = match r {
        Ok(_) => ("ok", None),
        Err(e) => ("fallo", Some(crate::web::public_message(e))),
    };
    crate::bitacora::anotar(
        "restauracion",
        json!({
            "id": format!("restauracion-{}", uuid::Uuid::new_v4()),
            "hora": fin.to_rfc3339(),
            "inicio": inicio.to_rfc3339(),
            "duracion_s": (fin - inicio).num_seconds().max(0),
            "repo": repo,
            "version": version.chars().take(8).collect::<String>(),
            "elementos": 1,
            "archivos": r.as_ref().ok().map(|h| h.archivos),
            "bytes": r.as_ref().ok().map(|h| h.bytes),
            "quitados": r.as_ref().ok().map(|h| h.sin_permiso).filter(|n| *n > 0),
            "destino": if c["carpeta"].as_str().is_some_and(|x| !x.trim().is_empty()) { "carpeta" } else { "junto" },
            "desde": "principal",
            "resultado": resultado,
            "mensaje": mensaje,
            "motivo": motivo,
            "cuenta": Some(cuenta.chars().filter(|c| !c.is_control()).take(80).collect::<String>()).filter(|x| !x.trim().is_empty()),
            "en_el_equipo": true,
            "recuperacion": true,
        }),
    );
}

// ---------- El menú del Explorador ----------

/// «Versiones anteriores con Resguardo» en el menú de archivos y carpetas del Explorador
/// (`HKLM\Software\Classes\*\shell` y `Directory\shell`), solo mientras esté activo en alguna
/// copia. Lanza `resguardo-agente --recuperar "<ruta>"` como el usuario (sin elevar). Nunca
/// en pruebas. El instalador lo quita al desinstalar.
pub fn sincronizar_menu(a: Option<&Ajustes>) {
    let a = a.cloned().unwrap_or_else(ajustes);
    sincronizar_menu_con(crate::servidor_v2::cargar().as_ref(), &a);
}

/// [`sincronizar_menu`] con el vínculo ya leído (p. ej. el que acaba de cambiar una orden).
pub fn sincronizar_menu_con(v: Option<&Vinculo>, a: &Ajustes) {
    if crate::agent::test_mode() || cfg!(test) {
        return;
    }
    #[cfg(windows)]
    {
        let quiere = activo_en_alguna(v, a);
        if let Err(e) = menu::poner(quiere) {
            crate::agent::log(&format!("Recuperar en el equipo: no se pudo {} el menú del Explorador: {e}", if quiere { "poner" } else { "quitar" }));
        }
    }
    #[cfg(not(windows))]
    let _ = (v, a);
}

#[cfg(windows)]
mod menu {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
    };

    /// Las dos claves (archivos y carpetas). El nombre del verbo es fijo (el instalador lo quita).
    pub const CLAVES: [&str; 2] = [r"Software\Classes\*\shell\ResguardoRecuperar", r"Software\Classes\Directory\shell\ResguardoRecuperar"];

    fn ancho(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn poner_valor(h: HKEY, nombre: Option<&str>, valor: &str) {
        let v = ancho(valor);
        let n = nombre.map(ancho);
        // SAFETY: cadenas terminadas en cero que viven durante la llamada.
        unsafe { RegSetValueExW(h, n.as_ref().map_or(std::ptr::null(), |n| n.as_ptr()), 0, REG_SZ, v.as_ptr() as *const u8, (v.len() * 2) as u32) };
    }

    fn crear(clave: &str) -> Result<HKEY, String> {
        let c = ancho(clave);
        let mut h: HKEY = std::ptr::null_mut();
        // SAFETY: cadena terminada en cero; `h` lo cierra quien llama.
        let r = unsafe {
            RegCreateKeyExW(
                HKEY_LOCAL_MACHINE,
                c.as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                std::ptr::null(),
                &mut h,
                std::ptr::null_mut(),
            )
        };
        if r != 0 {
            return Err(format!("error {r}"));
        }
        Ok(h)
    }

    pub fn poner(si: bool) -> Result<(), String> {
        for clave in CLAVES {
            if !si {
                let c = ancho(clave);
                // SAFETY: cadena terminada en cero; si no existe, no hace nada.
                unsafe { RegDeleteTreeW(HKEY_LOCAL_MACHINE, c.as_ptr()) };
                continue;
            }
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let exe = exe.display().to_string();
            if exe.contains('"') {
                return Err("ruta del programa no válida".into());
            }
            let h = crear(clave)?;
            poner_valor(h, Some("MUIVerb"), "Versiones anteriores con Resguardo");
            poner_valor(h, Some("Icon"), &format!("\"{exe}\",0"));
            // SAFETY: la abrió `crear`.
            unsafe { RegCloseKey(h) };
            let h = crear(&format!(r"{clave}\command"))?;
            poner_valor(h, None, &format!("\"{exe}\" --recuperar \"%1\""));
            // SAFETY: la abrió `crear`.
            unsafe { RegCloseKey(h) };
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un comprobador de pruebas: lo de `si` se puede leer, lo demás no; `existe` dice qué hay en el disco.
    struct DePrueba {
        existe: Vec<String>,
        si: Vec<String>,
        escribir: bool,
    }
    impl Comprobador for DePrueba {
        fn leer_ahora(&self, p: &Path) -> Lectura {
            let r = a_version(p);
            if !self.existe.contains(&r) {
                Lectura::NoSe
            } else if self.si.contains(&r) {
                Lectura::Si
            } else {
                Lectura::No
            }
        }
        fn leer_en_version(&self, n: &Value) -> Lectura {
            match n["dueno"].as_str() {
                Some("yo") => Lectura::Si,
                Some(_) => Lectura::No,
                None => Lectura::NoSe,
            }
        }
        fn escribir(&self, _: &Path) -> bool {
            self.escribir
        }
    }

    fn raiz() -> &'static str {
        if cfg!(windows) {
            "/C/Datos"
        } else {
            "/srv/datos"
        }
    }

    #[test]
    fn rutas_que_valen_y_que_no() {
        if cfg!(windows) {
            assert_eq!(ruta_valida(r"c:\Users\ana\informe.xlsx").unwrap(), PathBuf::from(r"C:\Users\ana\informe.xlsx"));
            assert_eq!(ruta_valida("C:/Users/ana/Docs/").unwrap(), PathBuf::from(r"C:\Users\ana\Docs"));
            for mal in [r"\\nas\x", r"\\?\C:\x", r"C:\", r"C:\a\..\Windows", r"C:\a\b.", r"C:\a\b:flujo", r"C:\CON", r"C:\a\nul.txt", "relativa", r"C:\a\*"] {
                assert!(ruta_valida(mal).is_err(), "{mal}");
            }
            assert_eq!(a_version(Path::new(r"C:\Users\ana\x.txt")), "/C/Users/ana/x.txt");
            assert_eq!(a_local("/C/Users/ana/x.txt").unwrap(), PathBuf::from(r"C:\Users\ana\x.txt"));
        } else {
            assert_eq!(ruta_valida("/home/ana/x/").unwrap(), PathBuf::from("/home/ana/x"));
            for mal in ["//x", "/", "/a/../etc", "relativa", "/a/./b"] {
                assert!(ruta_valida(mal).is_err(), "{mal}");
            }
        }
        assert!(ruta_valida("").is_err() && ruta_valida(&"a".repeat(2000)).is_err() && ruta_valida("C:\\a\u{7}").is_err());
    }

    #[test]
    fn nombres_con_la_fecha() {
        assert_eq!(nombre_con_fecha("informe.xlsx", false, "14-10-2026 10.00", 1), "informe (14-10-2026 10.00).xlsx");
        assert_eq!(nombre_con_fecha("informe.xlsx", false, "14-10-2026 10.00", 2), "informe (14-10-2026 10.00) (2).xlsx");
        assert_eq!(nombre_con_fecha("Docs.2026", true, "14-10-2026 10.00", 1), "Docs.2026 (14-10-2026 10.00)");
        assert_eq!(nombre_con_fecha(".bashrc", false, "x", 1), ".bashrc (x)");
        assert_eq!(nombre_con_fecha("sin extension", false, "x", 1), "sin extension (x)");
        assert!(!fecha_de("2026-10-14T10:00:00+00:00").contains(':'));
    }

    #[test]
    fn las_copias_mandan_sobre_el_cliente() {
        let mut a = Ajustes::default();
        let ks = vec!["r1#c1".to_string(), "r1#c2".to_string()];
        assert!(!activo_en(&ks, false, &a), "desactivado por defecto");
        assert!(activo_en(&ks, true, &a), "lo del cliente");
        a.copias.insert("r1#c1".into(), false);
        a.copias.insert("r1#c2".into(), false);
        assert!(!activo_en(&ks, true, &a), "las dos copias dicen que no");
        a.copias.insert("r1#c2".into(), true);
        assert!(activo_en(&ks, false, &a), "una copia dice que sí");
        assert!(!activo_en(&[], true, &a), "sin copias, nada");
    }

    #[test]
    fn solo_lo_que_la_cuenta_puede_leer() {
        let r = raiz();
        let n = |dueno: Option<&str>, tipo: &str, size: u64| json!({ "type": tipo, "size": size, "dueno": dueno });
        let nodos = vec![
            (format!("{r}/Docs"), n(Some("yo"), "dir", 0)),
            (format!("{r}/Docs/a.txt"), n(Some("yo"), "file", 10)),
            (format!("{r}/Docs/privado"), n(Some("otro"), "dir", 0)),
            (format!("{r}/Docs/privado/x.txt"), n(Some("yo"), "file", 99)),
            (format!("{r}/Docs/b.txt"), n(None, "file", 5)),
            (format!("{r}/Docs/c.txt"), n(Some("yo"), "file", 7)),
        ];
        // Ahora: existen Docs, a.txt (suyo) y c.txt (ya no lo puede leer).
        let c = DePrueba {
            existe: vec![format!("{r}/Docs"), format!("{r}/Docs/a.txt"), format!("{r}/Docs/c.txt")],
            si: vec![format!("{r}/Docs"), format!("{r}/Docs/a.txt")],
            escribir: true,
        };
        let (archivos, bytes, quitadas) = decidir(&c, &nodos).unwrap();
        assert_eq!((archivos, bytes), (1, 10), "solo a.txt");
        assert_eq!(
            quitadas,
            vec![format!("{r}/Docs/privado"), format!("{r}/Docs/b.txt"), format!("{r}/Docs/c.txt")],
            "lo de otro (y lo de dentro), sin datos y lo que ya no puede leer"
        );
        // Lo de arriba sin permiso: nada.
        let c2 = DePrueba { existe: vec![], si: vec![], escribir: true };
        assert!(decidir(&c2, &[(format!("{r}/x.txt"), n(Some("otro"), "file", 1))]).is_err());
        assert!(decidir(&c2, &[(format!("{r}/x.txt"), n(None, "file", 1))]).is_err(), "sin datos para decidir, no");
        // Borrado y suyo en la versión: sí.
        assert!(decidir(&c2, &[(format!("{r}/x.txt"), n(Some("yo"), "file", 1))]).is_ok());
        // Nadie (sin identificar a quien pide): nunca.
        assert!(decidir(&Nadie, &[(format!("{r}/x.txt"), n(Some("yo"), "file", 1))]).is_err());
    }

    /// Con restic de verdad (sin él, se omite): junto al original con la fecha, nunca encima,
    /// sin lo que la cuenta no puede leer y con el límite de tamaño.
    #[test]
    fn recuperar_con_restic() {
        let _real = crate::restic::tests::real_repo_lock();
        if restic::version().is_err() {
            return;
        }
        let base = std::env::temp_dir().join(format!("resguardo-recuperar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::env::set_var("RESGUARDO_AGENT_DIR", base.join("agente"));
        let datos = base.join("datos");
        std::fs::create_dir_all(datos.join("Docs").join("privado")).unwrap();
        std::fs::write(datos.join("Docs").join("a.txt"), b"version 1").unwrap();
        std::fs::write(datos.join("Docs").join("privado").join("x.txt"), b"de otro").unwrap();
        let acc = restic::Access {
            location: base.join("repo").display().to_string(),
            password: "contraseña de prueba".into(),
            rest_auth: None,
            cacert: None,
            env: Vec::new(),
        };
        assert_eq!(restic::run_raw(&acc, &["init"], Duration::from_secs(120)).unwrap().code, Some(0));
        assert_eq!(restic::run_raw(&acc, &["backup", &datos.display().to_string()], Duration::from_secs(120)).unwrap().code, Some(0));
        std::fs::write(datos.join("Docs").join("a.txt"), b"version 2").unwrap();
        let snap = restic::snapshots(&acc).unwrap().remove(0);
        let docs = datos.join("Docs");
        let rv = a_version(&docs);
        let todo = nodos(&acc, &snap.id, &rv).unwrap();
        assert!(todo.len() >= 4, "{todo:?}");

        /// Lo deja todo salvo «privado» (como si fuera de otra cuenta).
        struct SinPrivado;
        impl Comprobador for SinPrivado {
            fn leer_ahora(&self, p: &Path) -> Lectura {
                if p.to_string_lossy().contains("privado") {
                    Lectura::No
                } else if p.exists() {
                    Lectura::Si
                } else {
                    Lectura::NoSe
                }
            }
            fn leer_en_version(&self, _: &Value) -> Lectura {
                Lectura::NoSe
            }
            fn escribir(&self, _: &Path) -> bool {
                true
            }
        }
        // Un archivo: junto al original, con la fecha, y el original intacto.
        let h = hacer(&acc, &snap.id, &snap.time, &docs.join("a.txt"), None, &SinPrivado, 10).unwrap();
        assert!(h.donde.file_name().unwrap().to_string_lossy().starts_with("a ("), "{:?}", h.donde);
        assert_eq!(std::fs::read(&h.donde).unwrap(), b"version 1");
        assert_eq!(std::fs::read(docs.join("a.txt")).unwrap(), b"version 2", "nunca encima");
        // Otra vez: otro nombre, nunca el mismo.
        let h2 = hacer(&acc, &snap.id, &snap.time, &docs.join("a.txt"), None, &SinPrivado, 10).unwrap();
        assert_ne!(h.donde, h2.donde);
        assert_eq!(std::fs::read(&h.donde).unwrap(), b"version 1");
        // Una carpeta, en otra elegida: sin «privado».
        let otra = base.join("elegida");
        std::fs::create_dir_all(&otra).unwrap();
        let h3 = hacer(&acc, &snap.id, &snap.time, &docs, Some(&otra.display().to_string()), &SinPrivado, 10).unwrap();
        assert!(h3.donde.join("a.txt").is_file() && !h3.donde.join("privado").exists(), "{:?}", h3.donde);
        assert_eq!(h3.sin_permiso, 1);
        // Sin poder escribir donde se deja, o por encima del límite: no.
        assert!(hacer(&acc, &snap.id, &snap.time, &docs.join("a.txt"), None, &DePrueba { existe: vec![], si: vec![], escribir: false }, 10).is_err());
        // Nada se queda en la carpeta privada.
        assert!(std::fs::read_dir(crate::agent::private_dir().join("recuperar")).map(|d| d.count()).unwrap_or(0) == 0);
        std::env::remove_var("RESGUARDO_AGENT_DIR");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// En Windows, el comprobador de verdad con el token de este proceso: un archivo que su
    /// ACL le niega no se puede leer; uno suyo, sí; y la carpeta temporal, se puede escribir.
    #[cfg(windows)]
    #[test]
    fn access_check_con_un_token_de_verdad() {
        let Some(t) = win::token_propio() else { return };
        let c = win::PorToken(t);
        let dir = std::env::temp_dir().join(format!("resguardo-acl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mio = dir.join("mio.txt");
        let negado = dir.join("negado.txt");
        std::fs::write(&mio, b"x").unwrap();
        std::fs::write(&negado, b"x").unwrap();
        assert_eq!(c.leer_ahora(&mio), Lectura::Si);
        assert_eq!(c.leer_ahora(&dir.join("no-existe.txt")), Lectura::NoSe);
        assert!(c.escribir(&dir));
        // Negarse a sí mismo la lectura de un archivo propio (icacls; solo ese archivo temporal).
        let usuario = std::env::var("USERNAME").unwrap_or_default();
        let dominio = std::env::var("USERDOMAIN").unwrap_or_default();
        let ok =
            std::process::Command::new("icacls").arg(&negado).arg("/deny").arg(format!("{dominio}\\{usuario}:(R)")).output().is_ok_and(|o| o.status.success());
        if !ok {
            eprintln!("icacls no pudo negar la lectura: no se comprueba ese caso");
        }
        if ok {
            assert_eq!(c.leer_ahora(&negado), Lectura::No, "su ACL se lo niega");
            let _ = std::process::Command::new("icacls").arg(&negado).arg("/reset").output();
        }
        // Un descriptor guardado roto o ausente: «no se sabe».
        assert_eq!(c.leer_en_version(&json!({})), Lectura::NoSe);
        assert_eq!(c.leer_en_version(&json!({ "generic_attributes": { "windows.security_descriptor": "AAAA" } })), Lectura::NoSe);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
