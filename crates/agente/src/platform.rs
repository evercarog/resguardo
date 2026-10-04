//! Integración con el sistema operativo para el modo agente (Windows).
//!
//! - DPAPI a nivel de equipo: cifra los secretos del agente para que solo
//!   este equipo pueda descifrarlos (el archivo copiado a otro no sirve).
//! - Permisos: la carpeta del agente pertenece a Administradores y solo la
//!   modifican SYSTEM y Administradores; los usuarios solo pueden leer la
//!   programación y el estado. Los secretos viven en una subcarpeta sin
//!   acceso para los usuarios (heredado desde que el archivo se crea). Los
//!   permisos se aplican de una vez (dueño + lista protegida), sin ventanas
//!   intermedias como las de varias llamadas a icacls.
//! - Programador de tareas: una tarea que ejecuta `--agent-run` cada 5 minutos
//!   como SYSTEM, aunque no haya ninguna sesión iniciada.

use std::path::Path;

pub const TASK_NAME: &str = r"Resguardo\Agente";

#[cfg(windows)]
mod win {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    pub const NO_WINDOW: u32 = 0x0800_0000;

    pub fn system32(exe: &str) -> String {
        format!(r"{}\{exe}", super::system_dir())
    }

    /// Ejecuta una herramienta del sistema sin ventana y devuelve (éxito, salida).
    pub fn tool(exe: &str, args: &[&str]) -> Result<(bool, String), String> {
        let out = Command::new(system32(exe))
            .args(args)
            .stdin(Stdio::null())
            .creation_flags(NO_WINDOW)
            .output()
            .map_err(|e| format!("No se pudo ejecutar {exe}: {e}"))?;
        let text = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
        Ok((out.status.success(), text))
    }
}

/// Las DLL solo se cargan de System32 y de la carpeta del programa (Archivos
/// de programa): nunca de la carpeta actual ni del PATH, que un usuario podría
/// controlar. Se llama al principio de `main`, en la app y en el agente.
pub fn harden_dll_search() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::LibraryLoader::{SetDefaultDllDirectories, LOAD_LIBRARY_SEARCH_APPLICATION_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32};
        // SAFETY: sin punteros; solo cambia el orden de búsqueda del proceso.
        unsafe {
            SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32 | LOAD_LIBRARY_SEARCH_APPLICATION_DIR);
        }
    }
}

// Herramientas del sistema por su ruta completa y si un proceso sigue vivo: en el motor.
#[cfg(windows)]
pub use resguardo_motor::proceso::system_dir;
#[cfg(test)]
pub use resguardo_motor::proceso::system_tool;

/// Carpeta conocida de Windows (`FOLDERID_*`), o None si no se pudo obtener.
#[cfg(windows)]
fn known_folder(id: &windows_sys::core::GUID) -> Option<String> {
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::SHGetKnownFolderPath;
    let mut path: *mut u16 = std::ptr::null_mut();
    // SAFETY: Windows reserva la cadena y se libera con CoTaskMemFree.
    unsafe {
        let ok = SHGetKnownFolderPath(id, 0, std::ptr::null_mut(), &mut path) == 0 && !path.is_null();
        let out = ok.then(|| {
            let len = (0..).take_while(|&i| *path.add(i) != 0).count();
            String::from_utf16_lossy(std::slice::from_raw_parts(path, len))
        });
        CoTaskMemFree(path as _);
        out
    }
}

/// `C:\ProgramData` según Windows (no según la variable de entorno).
pub fn program_data() -> String {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Shell::FOLDERID_ProgramData;
        if let Some(p) = known_folder(&FOLDERID_ProgramData) {
            return p;
        }
    }
    std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into())
}

/// Carpeta local para datos que escribe SYSTEM (almacén del Servidor de
/// copias, espejo, volcados): ruta completa de un disco del equipo (en
/// Windows, `X:\…`), que no sea la raíz de un disco ni una ruta de red, y sin
/// enlaces en ninguna parte del camino que ya exista.
pub fn carpeta_local_valida(p: &str) -> Result<(), String> {
    let p = p.trim();
    let b = p.as_bytes();
    let forma = if cfg!(windows) {
        b.len() > 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\' && !p[3..].starts_with('\\')
    } else {
        p.starts_with('/') && !p.starts_with("//") && p.trim_end_matches('/').len() > 1
    };
    if !forma || p.chars().any(char::is_control) || p.split(['\\', '/']).any(|s| s == "..") {
        return Err(if cfg!(windows) {
            "Elige una carpeta de un disco de este equipo, no su raíz ni una carpeta de red (por ejemplo, E:\\Resguardo).".into()
        } else {
            "Elige una carpeta completa de este equipo, que no sea la raíz (por ejemplo, /srv/resguardo).".into()
        });
    }
    let mut camino = Some(Path::new(p));
    while let Some(c) = camino {
        if is_reparse_point(c) {
            return Err(format!("{} es un enlace: elige una carpeta normal.", c.display()));
        }
        camino = c.parent();
    }
    Ok(())
}

#[cfg(all(test, windows))]
#[test]
fn carpetas_locales() {
    for bien in [r"E:\Resguardo-espejo", r"D:\Copias\Sur"] {
        assert!(carpeta_local_valida(bien).is_ok(), "{bien}");
    }
    for mal in [r"E:\", r"\\nas\copias", r"\\?\E:\x", "relativa", r"E:\a\..\b", "E:/x", r"E:\\x"] {
        assert!(carpeta_local_valida(mal).is_err(), "{mal}");
    }
}

/// ¿Es `sid` un SID en texto (`S-1-5-21-…`)? Solo cifras y guiones: va dentro de un SDDL.
pub fn sid_valido(sid: &str) -> bool {
    sid.len() <= 184 && sid.starts_with("S-1-") && sid[4..].split('-').all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Crea **una** carpeta nueva (nunca sigue un enlace: si ya hay algo con ese
/// nombre, falla con `AlreadyExists`). Con `dueno` (el SID de quien la pide),
/// en Windows nace ya con permisos propios, sin heredar los de la carpeta de
/// arriba: SYSTEM, Administradores y esa cuenta. Así nadie más puede poner un
/// enlace dentro mientras el servicio escribe en ella. En Unix, de root y 0755.
pub fn crear_carpeta_nueva(p: &Path, dueno: Option<&str>) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        if let Some(sid) = dueno {
            use windows_sys::Win32::Foundation::LocalFree;
            use windows_sys::Win32::Security::Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1};
            use windows_sys::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
            use windows_sys::Win32::Storage::FileSystem::CreateDirectoryW;
            if !sid_valido(sid) {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "cuenta no válida"));
            }
            let ancho = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
            let sddl = ancho(&format!("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FA;;;{sid})"));
            let ruta = ancho(&p.to_string_lossy());
            let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
            // SAFETY: cadenas terminadas en cero; `sd` lo reserva Windows y se libera con LocalFree.
            unsafe {
                if ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), SDDL_REVISION_1, &mut sd, std::ptr::null_mut()) == 0 {
                    return Err(std::io::Error::last_os_error());
                }
                let sa = SECURITY_ATTRIBUTES { nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32, lpSecurityDescriptor: sd, bInheritHandle: 0 };
                let ok = CreateDirectoryW(ruta.as_ptr(), &sa) != 0;
                let err = std::io::Error::last_os_error();
                LocalFree(sd as _);
                return if ok { Ok(()) } else { Err(err) };
            }
        }
        std::fs::create_dir(p)
    }
    #[cfg(unix)]
    {
        let _ = dueno;
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().mode(0o755).create(p)
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = dueno;
        std::fs::create_dir(p)
    }
}

/// ¿Hay un enlace (simbólico, unión o punto de montaje) en `p` o en alguna
/// carpeta de su camino? Los marcadores de OneDrive y otros puntos de
/// reanálisis que no redirigen no cuentan.
pub fn hay_enlace_en_el_camino(p: &Path) -> bool {
    p.ancestors().any(|a| std::fs::symlink_metadata(a).is_ok_and(|m| m.file_type().is_symlink()))
}

/// Crea (si hace falta) una carpeta de datos de SYSTEM y la deja solo para
/// SYSTEM y Administradores (como la carpeta privada del agente).
pub fn carpeta_privada(p: &Path) -> Result<(), String> {
    carpeta_local_valida(&p.to_string_lossy())?;
    std::fs::create_dir_all(p).map_err(|e| format!("No se pudo crear la carpeta {}: {e}", p.display()))?;
    if is_reparse_point(p) {
        return Err(format!("{} es un enlace: elige una carpeta normal.", p.display()));
    }
    // En pruebas (RESGUARDO_AGENT_DIR, sin administrador), como la carpeta del
    // agente: sin cambiar permisos. Solo para SYSTEM y Administradores, el propio
    // proceso de la prueba (sin elevar) ya no podría escribir en ella.
    if crate::agent::test_mode() {
        return Ok(());
    }
    apply_sddl(p, SDDL_PRIVATE_DIR)
}

/// ¿Es un enlace (unión, enlace simbólico u otro punto de análisis)? Nunca
/// se siguen en las carpetas del agente: un usuario podría haberlos creado
/// antes de instalar Resguardo para desviar lo que escribe SYSTEM.
pub fn is_reparse_point(path: &Path) -> bool {
    let Ok(meta) = std::fs::symlink_metadata(path) else { return false };
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}

/// Cifra con DPAPI a nivel de equipo.
#[cfg(windows)]
pub fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
    dpapi(data, true)
}

#[cfg(windows)]
pub fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
    dpapi(data, false)
}

#[cfg(windows)]
fn dpapi(data: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_LOCAL_MACHINE, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    // SAFETY: punteros válidos durante la llamada; la salida la reserva Windows y se libera con LocalFree.
    let ok = unsafe {
        if encrypt {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_LOCAL_MACHINE | CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(&input, std::ptr::null_mut(), std::ptr::null(), std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut output)
        }
    };
    if ok == 0 {
        return Err(format!(
            "No se pudieron {} los datos del agente (DPAPI): {}",
            if encrypt { "cifrar" } else { "descifrar" },
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: Windows devolvió `cbData` bytes válidos en `pbData`.
    let bytes = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe { LocalFree(output.pbData as _) };
    Ok(bytes)
}

/// Linux y macOS: sin DPAPI. Los secretos del agente se guardan tal cual en
/// la carpeta privada (`/var/lib/resguardo-agente/privado`, 0700 de root);
/// los protegen los permisos, como a los de cualquier servicio del sistema.
/// (Pendiente: `systemd-creds` con TPM2 donde lo haya; docs/plataforma.md, §4.)
#[cfg(not(windows))]
pub fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
    Ok(data.to_vec())
}

#[cfg(not(windows))]
pub fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
    Ok(data.to_vec())
}

/// Lo que ha leído y escrito un proceso desde que empezó, en bytes:
/// `(leído, escrito)`. En Windows, «escrito» suma la escritura en disco y la
/// «otra» E/S (la red: lo que sube a un servidor o a la nube); en Linux,
/// `rchar` y `wchar` de `/proc/<pid>/io` (también cuentan la red). Para las
/// gráficas de la ventana y de la consola (lectura y subida reales).
pub fn io_proceso(pid: u32) -> Option<(u64, u64)> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{GetProcessIoCounters, OpenProcess, IO_COUNTERS, PROCESS_QUERY_LIMITED_INFORMATION};
        // SAFETY: handle propio que se cierra; estructura de salida local.
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return None;
            }
            let mut c: IO_COUNTERS = std::mem::zeroed();
            let ok = GetProcessIoCounters(h, &mut c) != 0;
            CloseHandle(h);
            ok.then_some((c.ReadTransferCount, c.WriteTransferCount + c.OtherTransferCount))
        }
    }
    #[cfg(unix)]
    {
        let t = std::fs::read_to_string(format!("/proc/{pid}/io")).ok()?;
        let campo = |k: &str| t.lines().find_map(|l| l.strip_prefix(k)).and_then(|v| v.trim().parse::<u64>().ok());
        Some((campo("rchar:")?, campo("wchar:")?))
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = pid;
        None
    }
}

/// ¿El proceso tiene permisos de administrador?
pub fn is_elevated() -> bool {
    #[cfg(windows)]
    {
        // SAFETY: función sin parámetros.
        unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
    }
    #[cfg(unix)]
    {
        // SAFETY: función sin parámetros.
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(not(any(windows, unix)))]
    {
        false
    }
}

/// Vuelve a abrir la app como administrador (Windows pedirá confirmación).
pub fn relaunch_elevated() -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
        let (verb, file) = (wide("runas"), wide(&exe.to_string_lossy()));
        // SAFETY: cadenas terminadas en cero que viven durante la llamada.
        let r = unsafe { ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), file.as_ptr(), std::ptr::null(), std::ptr::null(), 1) };
        if (r as isize) <= 32 {
            return Err("No se abrió como administrador (¿se canceló la confirmación de Windows?).".into());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("Solo disponible en Windows.".into())
    }
}

// Descriptores de seguridad (SDDL, con SIDs: no depende del idioma de
// Windows). Dueño: Administradores (BA). Lista protegida: no hereda de
// ProgramData, donde los usuarios pueden crear cosas.
/// Carpeta del agente: control total SYSTEM y Administradores; usuarios, leer.
pub const SDDL_AGENT_DIR: &str = "O:BAD:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;0x1200a9;;;BU)";
/// Secretos y archivos internos: sin acceso para los usuarios.
pub const SDDL_PRIVATE_DIR: &str = "O:BAD:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)";
/// Solicitudes de copia: los usuarios pueden crear archivos y escribir en
/// ellos, pero no borrar ni cambiar la carpeta (ni convertirla en un enlace).
/// El agente nunca borra ni escribe aquí: solo lee la fecha de cada archivo.
pub const SDDL_REQUESTS_DIR: &str = "O:BAD:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;;0x100082;;;BU)(A;OIIO;0x12019f;;;BU)";

/// Aplica dueño y permisos (SDDL) de una vez. Los hijos que heredan se
/// actualizan; los que tengan permisos explícitos se restablecen aparte.
pub fn apply_sddl(path: &Path, sddl: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::LocalFree;
        use windows_sys::Win32::Security::Authorization::{
            ConvertStringSecurityDescriptorToSecurityDescriptorW, SetNamedSecurityInfoW, SDDL_REVISION_1, SE_FILE_OBJECT,
        };
        use windows_sys::Win32::Security::{
            GetSecurityDescriptorDacl, GetSecurityDescriptorOwner, ACL, DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION,
            PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
        };
        let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
        let (w_sddl, w_path) = (wide(sddl), wide(&path.to_string_lossy()));
        let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        // SAFETY: cadenas terminadas en cero; `sd` lo reserva Windows y se
        // libera con LocalFree; `owner` y `dacl` apuntan dentro de `sd`.
        unsafe {
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(w_sddl.as_ptr(), SDDL_REVISION_1, &mut sd, std::ptr::null_mut()) == 0 {
                return Err(format!("Permisos no válidos: {}", std::io::Error::last_os_error()));
            }
            let mut owner: *mut core::ffi::c_void = std::ptr::null_mut();
            let mut dacl: *mut ACL = std::ptr::null_mut();
            let (mut present, mut defaulted) = (0, 0);
            GetSecurityDescriptorOwner(sd, &mut owner, &mut defaulted);
            GetSecurityDescriptorDacl(sd, &mut present, &mut dacl, &mut defaulted);
            let set = |info| SetNamedSecurityInfoW(w_path.as_ptr() as _, SE_FILE_OBJECT, info, owner, std::ptr::null_mut(), dacl, std::ptr::null());
            let mut err = set(OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION);
            // Si Windows no deja poner a Administradores como dueño (1307:
            // dueño no válido para este token; 5: acceso denegado), se aplican
            // al menos los permisos. El dueño ya se comprobó (owned_by_admins).
            if err == 1307 || err == 5 {
                err = set(DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION);
            }
            LocalFree(sd as _);
            if err != 0 {
                return Err(format!("No se pudieron ajustar los permisos de {}: {}", path.display(), std::io::Error::from_raw_os_error(err as i32)));
            }
        }
    }
    #[cfg(unix)]
    {
        // Equivalente en permisos POSIX (dueño: root).
        use std::os::unix::fs::PermissionsExt;
        let modo = match sddl {
            SDDL_PRIVATE_DIR => 0o700,
            // Como /tmp: todos pueden dejar solicitudes, nadie borra las de otros ni lista.
            SDDL_REQUESTS_DIR => 0o1733,
            _ => 0o755,
        };
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(modo))
            .map_err(|e| format!("No se pudieron ajustar los permisos de {}: {e}", path.display()))?;
        if is_elevated() {
            std::os::unix::fs::chown(path, Some(0), Some(0)).map_err(|e| format!("No se pudo cambiar el dueño de {}: {e}", path.display()))?;
        }
    }
    let _ = (path, sddl);
    Ok(())
}

/// ¿El dueño es SYSTEM o Administradores? (Si no, se anota en el registro:
/// la carpeta pudo crearla otra cuenta antes que Resguardo.)
pub fn owned_by_admins(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::LocalFree;
        use windows_sys::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
        use windows_sys::Win32::Security::{IsWellKnownSid, WinBuiltinAdministratorsSid, WinLocalSystemSid, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR};
        let w_path: Vec<u16> = path.to_string_lossy().encode_utf16().chain(Some(0)).collect();
        let mut owner: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        // SAFETY: ruta terminada en cero; `owner` apunta dentro de `sd`, que se libera al final.
        unsafe {
            let err = GetNamedSecurityInfoW(
                w_path.as_ptr() as _,
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION,
                &mut owner,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut sd,
            );
            if err != 0 {
                return false;
            }
            let ok = IsWellKnownSid(owner, WinLocalSystemSid) != 0 || IsWellKnownSid(owner, WinBuiltinAdministratorsSid) != 0;
            LocalFree(sd as _);
            ok
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        std::fs::symlink_metadata(path).is_ok_and(|m| m.uid() == 0)
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = path;
        true
    }
}

/// Restablece lo que hay dentro de `dir` (dueño Administradores y solo
/// permisos heredados): quita permisos explícitos que alguien haya puesto.
/// `skip`: nombres que no se tocan (subcarpetas con permisos propios).
pub fn reset_children(dir: &Path, skip: &[&str]) -> Result<(), String> {
    #[cfg(windows)]
    {
        let Ok(entries) = std::fs::read_dir(dir) else { return Ok(()) };
        let mut failed = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name();
            if skip.iter().any(|s| name.eq_ignore_ascii_case(s)) {
                continue;
            }
            let ruta = entry.path();
            // Un enlace aquí no lo ha puesto el agente: se quita (solo el
            // enlace) y nunca se siguen, o SYSTEM cambiaría el dueño y los
            // permisos de la carpeta a la que apunta (p. ej. System32).
            if is_reparse_point(&ruta) {
                if std::fs::remove_dir(&ruta).or_else(|_| std::fs::remove_file(&ruta)).is_err() {
                    failed.push(name.to_string_lossy().into_owned());
                }
                continue;
            }
            let p = ruta.to_string_lossy();
            let _ = win::tool("icacls.exe", &[p.as_ref(), "/setowner", "*S-1-5-32-544", "/T", "/L", "/C", "/Q"]);
            let (ok, _) = win::tool("icacls.exe", &[p.as_ref(), "/reset", "/T", "/L", "/C", "/Q"])?;
            if !ok {
                failed.push(name.to_string_lossy().into_owned());
            }
        }
        if !failed.is_empty() {
            return Err(format!("No se pudieron restablecer los permisos de: {}", failed.join(", ")));
        }
    }
    #[cfg(unix)]
    {
        // Sin permisos de escritura para el grupo ni para otros en lo que hay dentro.
        use std::os::unix::fs::PermissionsExt;
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if skip.iter().any(|s| entry.file_name().to_string_lossy().eq_ignore_ascii_case(s)) {
                    continue;
                }
                if let Ok(m) = std::fs::symlink_metadata(entry.path()) {
                    if !m.file_type().is_symlink() {
                        let _ = std::fs::set_permissions(entry.path(), std::fs::Permissions::from_mode(m.permissions().mode() & !0o022));
                    }
                }
            }
        }
    }
    let _ = (dir, skip);
    Ok(())
}

/// Carpetas de Archivos de programa según Windows (no según variables de
/// entorno, que un usuario podría cambiar).
#[cfg(windows)]
pub(crate) fn program_files_dirs() -> Vec<String> {
    use windows_sys::Win32::UI::Shell::{FOLDERID_ProgramFiles, FOLDERID_ProgramFilesX64, FOLDERID_ProgramFilesX86};
    [&FOLDERID_ProgramFiles, &FOLDERID_ProgramFilesX64, &FOLDERID_ProgramFilesX86].into_iter().filter_map(known_folder).collect()
}

/// ¿Es seguro que la tarea de SYSTEM ejecute este programa? Solo si está en
/// Archivos de programa (donde los usuarios no pueden cambiar archivos): un
/// ejecutable en una carpeta editable por usuarios daría control de SYSTEM.
pub fn exe_in_program_files(exe: &Path) -> bool {
    let Ok(exe) = exe.canonicalize() else { return false };
    let exe = exe.to_string_lossy().trim_start_matches(r"\\?\").to_lowercase();
    #[cfg(windows)]
    let bases = program_files_dirs();
    #[cfg(not(windows))]
    let bases: Vec<String> = Vec::new();
    bases.into_iter().map(|base| base.trim_end_matches('\\').to_lowercase()).any(|base| !base.is_empty() && exe.starts_with(&format!(r"{base}\")))
}

/// Error si la tarea de SYSTEM no puede usar este ejecutable (ver arriba).
pub fn check_task_exe() -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Ok(());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    if exe_in_program_files(&exe) {
        Ok(())
    } else {
        Err(format!(
            "Por seguridad, las copias automáticas solo funcionan con Resguardo instalado en Archivos de \
             programa (este es {}). Instálalo con el instalador.",
            exe.display()
        ))
    }
}

pub fn task_installed() -> bool {
    #[cfg(windows)]
    {
        win::tool("schtasks.exe", &["/Query", "/TN", TASK_NAME]).is_ok_and(|(ok, _)| ok)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Crea (o reemplaza) la tarea del agente. Requiere administrador.
/// `resguardo-agente.exe` nunca la toca: es la de la app de escritorio.
pub fn install_task(dir: &Path) -> Result<(), String> {
    if crate::agent::is_managed_agent() {
        return Ok(());
    }
    #[cfg(windows)]
    {
        check_task_exe()?;
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let xml = task_xml(&exe.to_string_lossy());
        // schtasks lee el XML de forma fiable en UTF-16 con BOM.
        let mut bytes = vec![0xFF, 0xFE];
        for u in xml.encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        let file = dir.join("tarea.xml");
        // Archivo nuevo: lo que lee schtasks (y ejecutará SYSTEM) no puede ser
        // uno que ya existiera, con otros permisos o abierto por otro proceso.
        crate::agent::write_new(&file, &bytes).map_err(|e| format!("No se pudo preparar la tarea: {e}"))?;
        let result = win::tool("schtasks.exe", &["/Create", "/TN", TASK_NAME, "/XML", &file.to_string_lossy(), "/F"]);
        let _ = std::fs::remove_file(&file);
        let (ok, out) = result?;
        if !ok {
            return Err(format!("No se pudo crear la tarea programada: {}", out.trim()));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = dir;
        Err("Solo disponible en Windows.".into())
    }
}

pub fn uninstall_task() -> Result<(), String> {
    // La tarea es de la app de escritorio: el agente gestionado no la quita.
    if crate::agent::is_managed_agent() {
        return Ok(());
    }
    #[cfg(windows)]
    {
        if !task_installed() {
            return Ok(());
        }
        let (ok, out) = win::tool("schtasks.exe", &["/Delete", "/TN", TASK_NAME, "/F"])?;
        if !ok {
            return Err(format!("No se pudo quitar la tarea programada: {}", out.trim()));
        }
    }
    Ok(())
}

/// Ejecuta una herramienta de System32 sin ventana: (éxito, salida).
pub fn tool(exe: &str, args: &[&str]) -> Result<(bool, String), String> {
    #[cfg(windows)]
    {
        win::tool(exe, args)
    }
    #[cfg(not(windows))]
    {
        // En Linux y macOS, `exe` es una ruta absoluta (p. ej. /usr/sbin/nft).
        let out =
            std::process::Command::new(exe).args(args).stdin(std::process::Stdio::null()).output().map_err(|e| format!("No se pudo ejecutar {exe}: {e}"))?;
        Ok((out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)))
    }
}

/// Tarea del Servidor de copias: al arrancar el equipo, como SYSTEM, sin
/// límite de tiempo y reiniciándose si falla. Requiere administrador.
pub fn install_server_task(dir: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        check_task_exe()?;
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let escape = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;");
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Resguardo: servidor de copias (rest-server, solo añadir) para los equipos de la cuenta.</Description>
  </RegistrationInfo>
  <Triggers>
    <BootTrigger>
      <Delay>PT1M</Delay>
      <Enabled>true</Enabled>
    </BootTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>S-1-5-18</UserId>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <StartWhenAvailable>true</StartWhenAvailable>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <RestartOnFailure>
      <Interval>PT1M</Interval>
      <Count>999</Count>
    </RestartOnFailure>
    <Priority>6</Priority>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{}</Command>
      <Arguments>--server-run</Arguments>
    </Exec>
  </Actions>
</Task>"#,
            escape(&exe.to_string_lossy())
        );
        let mut bytes = vec![0xFF, 0xFE];
        for u in xml.encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        let file = dir.join("tarea-servidor.xml");
        crate::agent::write_new(&file, &bytes).map_err(|e| format!("No se pudo preparar la tarea: {e}"))?;
        let name = crate::server::nombre_tarea();
        let result = win::tool("schtasks.exe", &["/Create", "/TN", name, "/XML", &file.to_string_lossy(), "/F"]);
        let _ = std::fs::remove_file(&file);
        let (ok, out) = result?;
        if !ok {
            return Err(format!("No se pudo crear la tarea del servidor: {}", out.trim()));
        }
        // Que empiece ya (sin esperar al próximo arranque).
        let _ = win::tool("schtasks.exe", &["/End", "/TN", name]);
        let _ = win::tool("schtasks.exe", &["/Run", "/TN", name]);
        Ok(())
    }
    #[cfg(unix)]
    {
        let _ = dir;
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let carpeta = crate::server::load().path;
        let unidad = unidad_servidor(&exe.to_string_lossy(), &carpeta, &crate::agent::agent_dir().to_string_lossy());
        std::fs::write(UNIDAD_SERVIDOR, unidad).map_err(|e| format!("No se pudo escribir {UNIDAD_SERVIDOR}: {e}"))?;
        for args in [&["daemon-reload"][..], &["enable", "resguardo-guarda-copias.service"], &["restart", "resguardo-guarda-copias.service"]] {
            let (ok, out) = tool("/usr/bin/systemctl", args).or_else(|_| tool("/bin/systemctl", args))?;
            if !ok {
                return Err(format!("systemctl {}: {}", args.join(" "), out.trim()));
            }
        }
        Ok(())
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = dir;
        Err("Solo disponible en Windows y Linux.".into())
    }
}

/// Linux: el servicio de systemd del Servidor de copias.
#[cfg(unix)]
const UNIDAD_SERVIDOR: &str = "/etc/systemd/system/resguardo-guarda-copias.service";

/// Unidad de systemd del Servidor de copias (como root, para poner las
/// reglas de nftables; solo puede escribir en su carpeta y en la del agente).
pub fn unidad_servidor(exe: &str, carpeta: &str, carpeta_agente: &str) -> String {
    // systemd: rutas entre comillas (pueden llevar espacios).
    let cita = |s: &str| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""));
    format!(
        "# Resguardo: Servidor de copias. Lo genera el agente (resguardo-agente guardar-copias activar).
[Unit]
Description=Resguardo: servidor de copias (rest-server, solo añadir)
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart={} --server-run
Restart=on-failure
RestartSec=60
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=strict
ReadWritePaths={} {}
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectKernelLogs=yes
ProtectControlGroups=yes
ProtectClock=yes
ProtectHostname=yes
RestrictRealtime=yes
RestrictSUIDSGID=yes
LockPersonality=yes
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX AF_NETLINK
SystemCallArchitectures=native
UMask=0077

[Install]
WantedBy=multi-user.target
",
        cita(exe),
        cita(carpeta),
        cita(carpeta_agente)
    )
}

pub fn uninstall_server_task() {
    #[cfg(windows)]
    {
        let _ = win::tool("schtasks.exe", &["/End", "/TN", crate::server::nombre_tarea()]);
        let _ = win::tool("schtasks.exe", &["/Delete", "/TN", crate::server::nombre_tarea(), "/F"]);
    }
    #[cfg(unix)]
    {
        if std::path::Path::new(UNIDAD_SERVIDOR).exists() {
            let systemctl = if std::path::Path::new("/usr/bin/systemctl").is_file() { "/usr/bin/systemctl" } else { "/bin/systemctl" };
            let _ = tool(systemctl, &["disable", "--now", "resguardo-guarda-copias.service"]);
            let _ = std::fs::remove_file(UNIDAD_SERVIDOR);
            let _ = tool(systemctl, &["daemon-reload"]);
        }
    }
}

/// Tarea: cada 5 minutos, como SYSTEM, sin límite de tiempo, también con
/// batería, sin instancias simultáneas y recuperando ejecuciones perdidas.
fn task_xml(exe: &str) -> String {
    let escape = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;");
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Resguardo: copias de seguridad programadas con restic.</Description>
  </RegistrationInfo>
  <Triggers>
    <BootTrigger>
      <Delay>PT2M</Delay>
      <Enabled>true</Enabled>
    </BootTrigger>
    <TimeTrigger>
      <StartBoundary>2020-01-01T00:00:00</StartBoundary>
      <Repetition>
        <Interval>PT5M</Interval>
        <StopAtDurationEnd>false</StopAtDurationEnd>
      </Repetition>
      <Enabled>true</Enabled>
    </TimeTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>S-1-5-18</UserId>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{}</Command>
      <Arguments>--agent-run</Arguments>
    </Exec>
  </Actions>
</Task>"#,
        escape(exe)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unidad_del_servidor_de_copias() {
        let u = unidad_servidor("/opt/resguardo-agente/resguardo-agente", "/srv/mis copias", "/var/lib/resguardo-agente");
        assert!(u.contains("ExecStart=\"/opt/resguardo-agente/resguardo-agente\" --server-run"));
        assert!(u.contains("ReadWritePaths=\"/srv/mis copias\" \"/var/lib/resguardo-agente\""));
        assert!(u.contains("ProtectSystem=strict"));
        // Las comillas de una ruta no rompen la línea.
        assert!(unidad_servidor("/x", "/a\"b", "/y").contains(r#"ReadWritePaths="/a\"b" "/y""#));
    }

    /// Un enlace dentro de la carpeta del agente se quita y no se sigue.
    #[cfg(windows)]
    #[test]
    fn restablecer_no_sigue_enlaces() {
        let base = std::env::temp_dir().join(format!("resguardo-reset-enlace-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (dir, fuera) = (base.join("agente"), base.join("fuera"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir_all(&fuera).unwrap();
        std::fs::write(fuera.join("ajeno.txt"), b"x").unwrap();
        std::fs::write(dir.join("agent.json"), b"{}").unwrap();
        let enlace = dir.join("x");
        let ok = std::process::Command::new(system_tool("cmd.exe")).args(["/c", "mklink", "/J"]).arg(&enlace).arg(&fuera).output().unwrap().status.success();
        assert!(ok, "mklink /J");
        reset_children(&dir, &[]).unwrap();
        assert!(std::fs::symlink_metadata(&enlace).is_err(), "el enlace se quita");
        assert!(fuera.join("ajeno.txt").is_file(), "lo de fuera sigue");
        assert!(dir.join("agent.json").is_file());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(windows)]
    #[test]
    fn dpapi_ida_y_vuelta() {
        let secret = "contraseña-secreta".as_bytes();
        let enc = protect(secret).unwrap();
        assert_ne!(enc, secret);
        assert_eq!(unprotect(&enc).unwrap(), secret);
        assert!(unprotect(b"basura").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn permisos_sddl_se_aplican() {
        let dir = std::env::temp_dir().join(format!("resguardo-sddl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(apply_sddl(&dir, "O:BAD:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FA;;;WD)").is_ok());
        for sddl in [SDDL_AGENT_DIR, SDDL_PRIVATE_DIR, SDDL_REQUESTS_DIR] {
            assert!(apply_sddl(&dir, sddl).is_ok(), "{sddl}");
        }
        let (_, out) = win::tool("icacls.exe", &[&dir.to_string_lossy()]).unwrap();
        assert!(out.contains("S-1-5-32-545") || out.contains("BUILTIN") || out.contains("Usuarios"), "{out}");
        assert!(!out.contains("(I)"), "no debe heredar de la carpeta superior: {out}");
        let _ = win::tool("icacls.exe", &[&dir.to_string_lossy(), "/reset"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn solo_archivos_de_programa() {
        // Instalación real (si existe en este equipo): debe reconocerse.
        let installed = &std::path::Path::new("C:/Program Files").join("Resguardo").join("resguardo.exe");
        if installed.exists() {
            assert!(exe_in_program_files(installed));
        }
        assert!(!exe_in_program_files(std::path::Path::new(r"C:\Windows\notepad.exe")));
        assert!(!exe_in_program_files(&std::env::temp_dir()));
    }

    #[test]
    fn xml_de_la_tarea_escapa_la_ruta() {
        let xml = task_xml(r#"C:\A&B\resguardo.exe"#);
        assert!(xml.contains(r"C:\A&amp;B\resguardo.exe"));
        assert!(xml.contains("<UserId>S-1-5-18</UserId>"));
        assert!(xml.contains("<Arguments>--agent-run</Arguments>"));
    }
}
