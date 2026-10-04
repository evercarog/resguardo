//! «Ver versiones en Resguardo» desde el menú contextual del Explorador.
//!
//! El Explorador lanza `resguardo.exe --versiones "<ruta>"` (o, si la app ya
//! está abierta, le pasa esos argumentos por la instancia única). La ruta llega
//! de fuera: se valida con cuidado antes de usarla y solo sirve para buscar en
//! qué copias está y para restaurar una versión con otro nombre junto al
//! original (nunca se reemplaza nada).
//!
//! El menú se registra por usuario en `HKCU\Software\Classes\*\shell` y
//! `HKCU\Software\Classes\Directory\shell` (verbo clásico; en Windows 11 sale
//! en «Mostrar más opciones»).

use serde::Serialize;
use std::fs;
use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::Mutex;

/// Argumento con el que se pide abrir las versiones de una ruta.
pub const ARG: &str = "--versiones";

/// Ruta pedida al arrancar (la recoge la interfaz al cargar).
#[derive(Default)]
pub struct Pending(pub Mutex<Option<String>>);

/// La ruta que sigue a `--versiones` en una lista de argumentos (sin validar).
pub fn from_args<I: IntoIterator<Item = String>>(args: I) -> Option<String> {
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if a == ARG {
            return it.next();
        }
    }
    None
}

/// Comprueba una ruta que llega de fuera y la deja en forma normal
/// (`C:\Carpeta\archivo.ext`, sin barra final). Solo rutas completas de una
/// unidad (`C:\…`) o de una carpeta compartida (`\\servidor\recurso\…`),
/// sin `.` ni `..`, sin rutas de dispositivo (`\\?\`, `\\.\`) ni caracteres
/// raros, y que existan.
pub fn validate_path(raw: &str) -> Result<PathBuf, String> {
    const INVALID: &str = "Esa ruta no es válida.";
    let raw = raw.trim();
    if raw.is_empty() || raw.chars().count() > 4096 {
        return Err(INVALID.into());
    }
    if raw.chars().any(|c| c.is_control() || matches!(c, '"' | '*' | '?' | '<' | '>' | '|')) {
        return Err(INVALID.into());
    }
    let unified = raw.replace('/', "\\");
    if unified.starts_with("\\\\?\\") || unified.starts_with("\\\\.\\") {
        return Err(INVALID.into());
    }
    // Una unidad: «C:\…». Una carpeta compartida: «\\servidor\recurso\…».
    let bytes = unified.as_bytes();
    let drive = bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'\\';
    let unc = unified.starts_with("\\\\") && unified[2..].split('\\').filter(|s| !s.is_empty()).count() >= 2;
    if !drive && !unc {
        return Err(INVALID.into());
    }
    // Sin «.» ni «..» ni dos puntos fuera de la unidad (flujos alternativos).
    let rest = if drive { &unified[3..] } else { &unified[2..] };
    for part in rest.split('\\').filter(|s| !s.is_empty()) {
        if part == "." || part == ".." || part.contains(':') {
            return Err(INVALID.into());
        }
    }
    let mut normal = unified.clone();
    while normal.ends_with('\\') && normal.len() > 3 {
        normal.pop();
    }
    if drive {
        // «c:\» → «C:\».
        normal.replace_range(0..1, &normal[0..1].to_ascii_uppercase());
    }
    let path = PathBuf::from(&normal);
    if cfg!(windows) {
        let ok = matches!(
            path.components().next(),
            Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_) | Prefix::UNC(_, _))
        );
        if !ok || path.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir)) {
            return Err(INVALID.into());
        }
    }
    if fs::symlink_metadata(&path).is_err() {
        return Err("Esa ruta ya no existe en este equipo.".into());
    }
    Ok(path)
}

/// Clave para comparar rutas de Windows (sin distinguir mayúsculas).
fn key(path: &str) -> String {
    let mut k = path.replace('/', "\\").to_lowercase();
    while k.ends_with('\\') && k.len() > 3 {
        k.pop();
    }
    k
}

/// ¿`path` es `dir` o está dentro de ella?
pub fn within(path: &str, dir: &str) -> bool {
    let (p, d) = (key(path), key(dir));
    p == d || p.starts_with(&if d.ends_with('\\') { d.clone() } else { format!("{d}\\") })
}

/// Una copia que incluye la ruta.
#[derive(Serialize, Debug, PartialEq)]
pub struct Found {
    pub repo_id: String,
    pub plan_id: String,
    /// "inside": la ruta está dentro de una carpeta de la copia;
    /// "contains": la ruta es una carpeta que contiene carpetas de la copia.
    pub kind: &'static str,
}

/// Copias (planes) que incluyen la ruta. Lo excluido no se tiene en cuenta
/// aquí: si algo está excluido, simplemente no aparecerá en las versiones.
pub fn locate(repos: &[crate::store::Repo], path: &str) -> Vec<Found> {
    let mut out = Vec::new();
    for repo in repos {
        for plan in &repo.plans {
            let kind = if plan.paths.iter().any(|p| within(path, p)) {
                "inside"
            } else if plan.paths.iter().any(|p| within(p, path)) {
                "contains"
            } else {
                continue;
            };
            out.push(Found { repo_id: repo.id.clone(), plan_id: plan.id.clone(), kind });
        }
    }
    // Primero las que la incluyen entera.
    out.sort_by_key(|f| f.kind != "inside");
    out
}

/// Lo que se sabe de la ruta en este equipo (para compararla con sus versiones).
#[derive(Serialize)]
pub struct Located {
    pub path: String,
    pub name: String,
    pub parent: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    /// Fecha de modificación (RFC 3339).
    pub mtime: Option<String>,
    pub found: Vec<Found>,
}

pub fn describe(path: &Path, found: Vec<Found>) -> Located {
    let meta = fs::metadata(path).ok();
    let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
    let s = path.to_string_lossy().into_owned();
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| s.clone());
    let parent = path.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    Located {
        path: s,
        name,
        parent,
        is_dir,
        size: meta.as_ref().filter(|m| m.is_file()).map(|m| m.len()),
        mtime: meta.and_then(|m| m.modified().ok()).map(|t| chrono::DateTime::<chrono::Local>::from(t).to_rfc3339()),
        found,
    }
}

/// ¿Vale como nombre de archivo o carpeta nuevo?
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= 200
        && name != "."
        && name != ".."
        && !name.ends_with(['.', ' '])
        && !name.chars().any(|c| c.is_control() || matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
}

/// Primer nombre libre en `dir`: «Informe (versión del 30-09).xlsx», y si ya
/// existe, «Informe (versión del 30-09) (2).xlsx»…
pub fn free_name(dir: &Path, name: &str, is_dir: bool) -> Option<PathBuf> {
    let (stem, ext) = match name.rfind('.') {
        Some(i) if !is_dir && i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    (1..=100).map(|n| if n == 1 { name.to_string() } else { format!("{stem} ({n}){ext}") }).map(|n| dir.join(n)).find(|p| fs::symlink_metadata(p).is_err())
}

/// Ruta de Windows → ruta dentro de una versión (`C:\A\b` → `/C/A/b`).
pub fn snapshot_path(path: &str) -> String {
    let p = path.replace('\\', "/");
    let b = p.as_bytes();
    if b.len() >= 2 && b[1] == b':' {
        format!("/{}{}", (b[0] as char).to_ascii_uppercase(), &p[2..]).trim_end_matches('/').to_string()
    } else {
        p.trim_end_matches('/').to_string()
    }
}

/// Restaura `original` tal como estaba en una versión, con el nombre `name`
/// junto al original. Nunca reemplaza: si el nombre está ocupado se usa
/// «(2)», «(3)»… Se restaura primero en una carpeta temporal nueva al lado (el
/// mismo disco) y luego se mueve. Devuelve la ruta final.
pub fn restore_as<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    jobs: &crate::jobs::Jobs,
    repo: &crate::store::Repo,
    access: &crate::restic::Access,
    snapshot: &str,
    original: &Path,
    name: &str,
) -> Result<PathBuf, String> {
    if !valid_name(name) {
        return Err("Ese nombre no vale para un archivo.".into());
    }
    let parent = original.parent().filter(|p| p.is_dir()).ok_or("No se encuentra la carpeta del original.")?;
    let base = original.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or("Esa ruta no es válida.")?;
    let is_dir = original.is_dir();
    let snap_path = snapshot_path(&original.to_string_lossy());
    let snap_dir = snap_path.rsplit_once('/').map(|(d, _)| if d.is_empty() { "/" } else { d }).unwrap_or("/").to_string();

    let staging = parent.join(format!(".resguardo-restaurando-{}", uuid::Uuid::new_v4().simple()));
    fs::create_dir(&staging).map_err(|e| format!("No se pudo escribir en la carpeta del original: {e}"))?;
    let req = crate::restore::Request {
        snapshot: snapshot.into(),
        dir: snap_dir,
        names: vec![base.clone()],
        target: staging.to_string_lossy().into_owned(),
        overwrite: false,
    };
    let result = crate::restore::run(app, jobs, repo, access, &req).and_then(|r| {
        if r.error_count > 0 && r.summary.as_ref().is_none_or(|s| s.files_restored == 0) {
            return Err(r.errors.first().cloned().unwrap_or_else(|| "No se pudo restaurar.".into()));
        }
        let restored = staging.join(&base);
        if fs::symlink_metadata(&restored).is_err() {
            return Err("No está en esa versión.".into());
        }
        let target = free_name(parent, name, is_dir).ok_or("Ya hay demasiadas versiones restauradas con ese nombre.")?;
        // Se comprueba justo antes: `rename` reemplazaría un archivo con ese nombre.
        if fs::symlink_metadata(&target).is_ok() {
            return Err("Ya existe un archivo con ese nombre.".into());
        }
        fs::rename(&restored, &target).map_err(|e| format!("No se pudo poner en su sitio: {e}"))?;
        Ok(target)
    });
    // La carpeta temporal es nuestra y nueva: se quita (sin seguir enlaces).
    let _ = fs::remove_dir_all(&staging);
    result
}

// ---------- Menú contextual del Explorador ----------

const VERB: &str = "ResguardoVersiones";
const LABEL: &str = "Ver versiones en Resguardo";
const ROOTS: [&str; 2] = [r"Software\Classes\*\shell", r"Software\Classes\Directory\shell"];

/// Línea de órdenes del verbo.
pub fn command_line(exe: &Path) -> String {
    format!("\"{}\" {ARG} \"%1\"", exe.display())
}

/// Claves y valores que se escriben (clave, nombre del valor —vacío: el
/// predeterminado—, valor).
pub fn entries(exe: &Path) -> Vec<(String, &'static str, String)> {
    let mut out = Vec::new();
    for root in ROOTS {
        let verb = format!(r"{root}\{VERB}");
        out.push((verb.clone(), "MUIVerb", LABEL.to_string()));
        out.push((verb.clone(), "Icon", format!("\"{}\",0", exe.display())));
        // Solo con un elemento seleccionado.
        out.push((verb.clone(), "MultiSelectModel", "Single".to_string()));
        out.push((format!(r"{verb}\command"), "", command_line(exe)));
    }
    out
}

#[cfg(windows)]
mod reg {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegGetValueW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ,
        RRF_RT_REG_SZ,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn set(key: &str, name: &str, value: &str) -> Result<(), String> {
        let mut hkey: HKEY = std::ptr::null_mut();
        let k = wide(key);
        // SAFETY: cadenas terminadas en cero y punteros válidos durante la llamada.
        let rc = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                k.as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                std::ptr::null(),
                &mut hkey,
                std::ptr::null_mut(),
            )
        };
        if rc != ERROR_SUCCESS {
            return Err(format!("No se pudo escribir en el registro (código {rc})."));
        }
        let n = wide(name);
        let v = wide(value);
        let name_ptr = if name.is_empty() { std::ptr::null() } else { n.as_ptr() };
        // SAFETY: `v` vive hasta el final; el tamaño va en bytes con el cero final.
        let rc = unsafe { RegSetValueExW(hkey, name_ptr, 0, REG_SZ, v.as_ptr().cast(), (v.len() * 2) as u32) };
        // SAFETY: clave abierta arriba.
        unsafe { RegCloseKey(hkey) };
        if rc != ERROR_SUCCESS {
            return Err(format!("No se pudo escribir en el registro (código {rc})."));
        }
        Ok(())
    }

    pub fn get(key: &str, name: &str) -> Option<String> {
        let k = wide(key);
        let n = wide(name);
        let name_ptr = if name.is_empty() { std::ptr::null() } else { n.as_ptr() };
        let mut buf = vec![0u16; 2048];
        let mut size = (buf.len() * 2) as u32;
        // SAFETY: `buf` tiene `size` bytes.
        let rc = unsafe { RegGetValueW(HKEY_CURRENT_USER, k.as_ptr(), name_ptr, RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut size) };
        if rc != ERROR_SUCCESS {
            return None;
        }
        let len = (size as usize / 2).saturating_sub(1);
        Some(String::from_utf16_lossy(&buf[..len.min(buf.len())]))
    }

    pub fn delete_tree(key: &str) -> Result<(), String> {
        let k = wide(key);
        // SAFETY: cadena terminada en cero.
        let rc = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, k.as_ptr()) };
        // 2: no existía.
        if rc != ERROR_SUCCESS && rc != 2 {
            return Err(format!("No se pudo quitar del registro (código {rc})."));
        }
        Ok(())
    }
}

/// ¿Está el menú puesto (y apunta a este programa)?
#[cfg(windows)]
pub fn menu_installed() -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    let want = command_line(&exe);
    ROOTS.iter().all(|root| reg::get(&format!(r"{root}\{VERB}\command"), "").is_some_and(|v| v == want))
}

/// Pone o quita el menú del Explorador (solo para este usuario).
#[cfg(windows)]
pub fn set_menu(enabled: bool) -> Result<(), String> {
    for root in ROOTS {
        reg::delete_tree(&format!(r"{root}\{VERB}"))?;
    }
    if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        for (key, name, value) in entries(&exe) {
            reg::set(&key, name, &value)?;
        }
    }
    notify_shell();
    Ok(())
}

/// Avisa al Explorador de que cambiaron las asociaciones.
#[cfg(windows)]
fn notify_shell() {
    use windows_sys::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};
    // SAFETY: sin punteros.
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED as i32, SHCNF_IDLIST, std::ptr::null(), std::ptr::null()) };
}

#[cfg(not(windows))]
pub fn menu_installed() -> bool {
    false
}

#[cfg(not(windows))]
pub fn set_menu(_enabled: bool) -> Result<(), String> {
    Err("Solo en Windows.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lee_la_ruta_de_los_argumentos() {
        let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(from_args(args(&["resguardo.exe", "--versiones", r"C:\a b\c.txt"])), Some(r"C:\a b\c.txt".into()));
        assert_eq!(from_args(args(&["resguardo.exe", "--bandeja"])), None);
        assert_eq!(from_args(args(&["resguardo.exe", "--versiones"])), None);
    }

    #[test]
    #[cfg(windows)]
    fn valida_las_rutas() {
        let dir = std::env::temp_dir().join(format!("resguardo-versiones-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Informe final.xlsx");
        fs::write(&file, b"x").unwrap();
        let s = file.to_string_lossy().to_string();
        assert_eq!(validate_path(&s).unwrap(), file);
        assert_eq!(validate_path(&format!("{}\\", dir.display())).unwrap(), dir);
        assert_eq!(validate_path(&s.replace('\\', "/")).unwrap(), file);
        // Inválidas.
        for bad in [
            "",
            "Informe.xlsx",
            r"\Windows",
            r"C:Windows",
            r"\\?\C:\Windows",
            r"\\.\PhysicalDrive0",
            r"C:\Windows\..\Windows",
            r"C:\Windows\.\System32",
            r"C:\a*",
            "C:\\a\u{0}b",
            r"C:\x.txt:secreto",
            r#"C:\a" --agent-run"#,
        ] {
            assert!(validate_path(bad).is_err(), "{bad}");
        }
        // Que no existe.
        assert!(validate_path(&dir.join("no-existe").to_string_lossy()).unwrap_err().contains("ya no existe"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn encuentra_las_copias_que_la_incluyen() {
        let repos: Vec<crate::store::Repo> = serde_json::from_value(serde_json::json!([
            { "id": "r1", "name": "Disco", "location": "D:\\copias", "paths": [], "plans": [
                { "id": "p1", "name": "Laboral", "paths": ["C:\\Users\\Ana\\Documentos"] },
                { "id": "p2", "name": "Fotos", "paths": ["C:\\Users\\Ana\\Pictures\\"] }
            ] }
        ]))
        .unwrap();
        let f = locate(&repos, r"c:\users\ana\documentos\Informe.xlsx");
        assert_eq!(f, vec![Found { repo_id: "r1".into(), plan_id: "p1".into(), kind: "inside" }]);
        // Una carpeta que contiene carpetas de copias.
        let f = locate(&repos, r"C:\Users\Ana");
        assert_eq!(f.len(), 2);
        assert!(f.iter().all(|x| x.kind == "contains"));
        // Prefijo de nombre, no carpeta: no cuenta.
        assert!(locate(&repos, r"C:\Users\Ana\Documentos viejos\x.txt").is_empty());
        assert!(within(r"C:\Users\Ana\Pictures", r"C:\Users\Ana\Pictures\"));
    }

    #[test]
    fn nombres_nuevos() {
        assert!(valid_name("Informe (versión del 30-09).xlsx"));
        for bad in ["", ".", "..", "a/b", "a\\b", "a:b", "nombre.", "x?"] {
            assert!(!valid_name(bad), "{bad}");
        }
        let dir = std::env::temp_dir().join(format!("resguardo-nombres-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(free_name(&dir, "Informe (v).xlsx", false).unwrap(), dir.join("Informe (v).xlsx"));
        fs::write(dir.join("Informe (v).xlsx"), b"x").unwrap();
        assert_eq!(free_name(&dir, "Informe (v).xlsx", false).unwrap(), dir.join("Informe (v) (2).xlsx"));
        fs::create_dir(dir.join("Carpeta.v1")).unwrap();
        assert_eq!(free_name(&dir, "Carpeta.v1", true).unwrap(), dir.join("Carpeta.v1 (2)"));
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(snapshot_path(r"c:\Users\Ana\x.txt"), "/C/Users/Ana/x.txt");
    }

    /// Con un repositorio real (ver restore.rs): restaura un archivo con otro
    /// nombre junto al original, sin tocarlo, y sin dejar la carpeta temporal.
    #[test]
    fn restaura_con_otro_nombre_junto_al_original() {
        let (Ok(location), Ok(pw), Ok(data)) =
            (std::env::var("RESGUARDO_TEST_REPO"), std::env::var("RESGUARDO_TEST_PASSWORD"), std::env::var("RESGUARDO_TEST_DATA"))
        else {
            return;
        };
        let _lock = crate::restic::tests::real_repo_lock();
        let access = crate::restic::Access::new(location.clone(), pw);
        let snap = crate::restic::snapshots(&access).unwrap().into_iter().max_by(|a, b| a.time.cmp(&b.time)).unwrap();
        let entry = crate::restic::list_dir(&access, &snap.short_id, &snapshot_path(&data)).unwrap().into_iter().find(|e| e.kind == "file").unwrap();
        let original = Path::new(&data).join(&entry.name);
        let before = fs::read(&original).unwrap();
        let repo: crate::store::Repo = serde_json::from_value(serde_json::json!({ "id": "t", "name": "t", "location": location, "paths": [] })).unwrap();
        let app = tauri::test::mock_app();
        let jobs = crate::jobs::Jobs::default();
        let name = format!("{} (versión de prueba)", entry.name);
        let first = restore_as(app.handle(), &jobs, &repo, &access, &snap.short_id, &original, &name).expect("debería restaurar");
        let second = restore_as(app.handle(), &jobs, &repo, &access, &snap.short_id, &original, &name).expect("debería restaurar");
        let cleanup = || {
            let _ = fs::remove_file(&first);
            let _ = fs::remove_file(&second);
        };
        assert_eq!(first, Path::new(&data).join(&name));
        assert_ne!(first, second, "no reemplaza: usa otro nombre");
        assert!(second.to_string_lossy().contains("(2)"));
        assert_eq!(fs::read(&original).unwrap(), before, "el original no se toca");
        let leftovers = fs::read_dir(&data).unwrap().filter_map(|e| e.ok()).any(|e| e.file_name().to_string_lossy().starts_with(".resguardo-restaurando-"));
        cleanup();
        assert!(!leftovers, "sin carpeta temporal");
    }

    #[test]
    fn entradas_del_registro() {
        let exe = Path::new(r"C:\Program Files\Resguardo\resguardo.exe");
        assert_eq!(command_line(exe), r#""C:\Program Files\Resguardo\resguardo.exe" --versiones "%1""#);
        let e = entries(exe);
        assert_eq!(e.len(), 8);
        assert!(e.iter().all(|(k, _, _)| k.starts_with(r"Software\Classes\")));
        assert!(e.iter().any(|(k, n, v)| k == r"Software\Classes\Directory\shell\ResguardoVersiones" && *n == "MUIVerb" && v == LABEL));
    }
}
