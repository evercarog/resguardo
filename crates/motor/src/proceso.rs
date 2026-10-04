//! Procesos: prioridad baja para restic («modo discreto»), herramientas del
//! sistema por su ruta completa, el entorno que heredan y si un proceso sigue vivo.

/// ¿Es un destino remoto (al que se sube por la red)? Las rutas locales y de
/// carpetas compartidas no lo son.
pub fn is_remote(location: &str) -> bool {
    location.split_once(':').is_some_and(|(scheme, _)| scheme.len() > 1 && scheme.chars().all(|c| c.is_ascii_alphanumeric()))
}

// ---------- Prioridad de los procesos de restic ----------

/// Prepara el proceso para que arranque con prioridad de CPU por debajo de lo normal.
#[cfg(windows)]
pub fn lower(cmd: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::System::Threading::BELOW_NORMAL_PRIORITY_CLASS;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
}

#[cfg(windows)]
#[link(name = "ntdll")]
extern "system" {
    fn NtSetInformationProcess(process: windows_sys::Win32::Foundation::HANDLE, class: i32, info: *const core::ffi::c_void, len: u32) -> i32;
}

/// Prioridad de E/S baja (`ProcessIoPriority`, la que usan los programas en
/// segundo plano de Windows).
#[cfg(windows)]
pub const IO_PRIORITY_LOW: u32 = 1;

/// Recién lanzado: E/S de disco y memoria con prioridad baja (lo que el
/// «modo en segundo plano» de Windows hace con un proceso, aplicado desde fuera).
#[cfg(windows)]
pub fn after_spawn(child: &std::process::Child) {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Wdk::System::Threading::ProcessIoPriority;
    use windows_sys::Win32::System::Threading::{ProcessMemoryPriority, SetProcessInformation, MEMORY_PRIORITY_INFORMATION, MEMORY_PRIORITY_LOW};
    let handle = child.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE;
    let io = IO_PRIORITY_LOW;
    let mem = MEMORY_PRIORITY_INFORMATION { MemoryPriority: MEMORY_PRIORITY_LOW };
    // SAFETY: el proceso es nuestro (handle válido mientras viva `child`) y los
    // datos tienen el tamaño que se indica. Si falla, sigue con prioridad de CPU baja.
    unsafe {
        NtSetInformationProcess(handle, ProcessIoPriority, (&io as *const u32).cast(), 4);
        SetProcessInformation(
            handle,
            ProcessMemoryPriority,
            (&mem as *const MEMORY_PRIORITY_INFORMATION).cast(),
            std::mem::size_of::<MEMORY_PRIORITY_INFORMATION>() as u32,
        );
    }
}

#[cfg(not(windows))]
pub fn lower(_cmd: &mut std::process::Command) {}

#[cfg(not(windows))]
pub fn after_spawn(_child: &std::process::Child) {}

/// Prioridades de un proceso (para comprobarlo): (clase de CPU, E/S, memoria).
#[cfg(windows)]
pub fn priorities(child: &std::process::Child) -> (u32, u32, u32) {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Wdk::System::Threading::{NtQueryInformationProcess, ProcessIoPriority};
    use windows_sys::Win32::System::Threading::{GetPriorityClass, GetProcessInformation, ProcessMemoryPriority, MEMORY_PRIORITY_INFORMATION};
    let handle = child.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE;
    let mut io = 99u32;
    let mut len = 0u32;
    let mut mem = MEMORY_PRIORITY_INFORMATION { MemoryPriority: 99 };
    // SAFETY: búferes del tamaño indicado; handle válido.
    unsafe {
        NtQueryInformationProcess(handle, ProcessIoPriority, (&mut io as *mut u32).cast(), 4, &mut len);
        GetProcessInformation(
            handle,
            ProcessMemoryPriority,
            (&mut mem as *mut MEMORY_PRIORITY_INFORMATION).cast(),
            std::mem::size_of::<MEMORY_PRIORITY_INFORMATION>() as u32,
        );
        (GetPriorityClass(handle), io, mem.MemoryPriority)
    }
}

// ---------- Herramientas del sistema ----------

/// `C:\Windows\System32` según Windows (no según la variable SystemRoot, que
/// el proceso hereda y se puede cambiar).
#[cfg(windows)]
pub fn system_dir() -> String {
    use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
    let mut buf = [0u16; 260];
    // SAFETY: búfer válido de 260 caracteres.
    let n = unsafe { GetSystemDirectoryW(buf.as_mut_ptr(), buf.len() as u32) } as usize;
    if n == 0 || n >= buf.len() {
        return r"C:\Windows\System32".into();
    }
    String::from_utf16_lossy(&buf[..n])
}

/// Ruta completa de una herramienta de Windows (`taskkill.exe`, `icacls.exe`…).
pub fn system_tool(exe: &str) -> String {
    #[cfg(windows)]
    {
        format!(r"{}\{exe}", system_dir())
    }
    #[cfg(not(windows))]
    {
        exe.to_string()
    }
}

/// ¿Sigue en marcha el proceso con este PID? Si no se puede saber (p. ej.
/// sin permiso para consultarlo), se supone que sí.
pub fn process_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_INVALID_PARAMETER, STILL_ACTIVE};
        use windows_sys::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                // No existe ese proceso: ERROR_INVALID_PARAMETER. Otro error (acceso denegado): vivo.
                return GetLastError() != ERROR_INVALID_PARAMETER;
            }
            let mut code = 0u32;
            let ok = GetExitCodeProcess(h, &mut code) != 0;
            CloseHandle(h);
            !ok || code == STILL_ACTIVE as u32
        }
    }
    #[cfg(target_os = "linux")]
    {
        // Sin /proc (montado de otra forma) no se puede saber: vivo.
        let proc_ok = std::path::Path::new("/proc/self").exists();
        !proc_ok || std::path::Path::new(&format!("/proc/{pid}")).exists()
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = pid;
        true
    }
}

// ---------- Entorno de los programas que se lanzan ----------

/// Variables del entorno que pasan a restic, rclone, rest-server y sqlcmd: lo
/// que necesitan para funcionar (carpetas del sistema y del usuario, idioma,
/// zona horaria y proxy). Todo lo demás se queda fuera: variables de restic,
/// de las nubes o de rclone que cambiarían el repositorio, las credenciales o
/// adónde se escribe (`RESTIC_PASSWORD_COMMAND`, `RCLONE_LOG_FILE`…), las de Go
/// que cambian la verificación TLS (`SSL_CERT_FILE`, `GODEBUG`…) y el guion de
/// arranque de sqlcmd (`SQLCMDINI`). Lo que el programa necesita de verdad lo
/// pone quien lo lanza, explícitamente (`Command::env`).
const ENTORNO_PERMITIDO: &[&str] = &[
    // Windows
    "SYSTEMROOT",
    "WINDIR",
    "SYSTEMDRIVE",
    "PATHEXT",
    "TEMP",
    "TMP",
    "USERPROFILE",
    "LOCALAPPDATA",
    "APPDATA",
    "PROGRAMDATA",
    "PROGRAMFILES",
    "PROGRAMFILES(X86)",
    "PROGRAMW6432",
    "COMMONPROGRAMFILES",
    "COMMONPROGRAMFILES(X86)",
    "HOMEDRIVE",
    "HOMEPATH",
    "USERNAME",
    "USERDOMAIN",
    "COMPUTERNAME",
    "NUMBER_OF_PROCESSORS",
    "PROCESSOR_ARCHITECTURE",
    "OS",
    // Unix
    "HOME",
    "USER",
    "LOGNAME",
    "TMPDIR",
    "XDG_CACHE_HOME",
    "XDG_CONFIG_HOME",
    "LANG",
    "LANGUAGE",
    "LC_ALL",
    "LC_CTYPE",
    "LC_MESSAGES",
    "TZ",
    // Los dos (restic busca rclone en el PATH con «rclone:»)
    "PATH",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
];

/// ¿Pasa esta variable a los programas que se lanzan? En Windows los nombres no
/// distinguen mayúsculas; en Unix, sí (salvo las del proxy, que se usan de las dos formas).
pub fn variable_permitida(nombre: &std::ffi::OsStr) -> bool {
    let Some(n) = nombre.to_str() else { return false };
    let mayus = n.to_ascii_uppercase();
    if matches!(mayus.as_str(), "HTTP_PROXY" | "HTTPS_PROXY" | "NO_PROXY") {
        return true;
    }
    if cfg!(windows) {
        ENTORNO_PERMITIDO.contains(&mayus.as_str())
    } else {
        ENTORNO_PERMITIDO.contains(&n)
    }
}

/// Deja a `cmd` con el entorno mínimo (ver [`ENTORNO_PERMITIDO`]), tomado de `vars`.
pub fn entorno_minimo_de(cmd: &mut std::process::Command, vars: impl IntoIterator<Item = (std::ffi::OsString, std::ffi::OsString)>) {
    cmd.env_clear();
    for (k, v) in vars {
        if variable_permitida(&k) {
            cmd.env(k, v);
        }
    }
}

/// Deja a `cmd` con el entorno mínimo del de este proceso. Lo que el programa
/// necesite además se pone después con `Command::env`.
pub fn entorno_minimo(cmd: &mut std::process::Command) {
    entorno_minimo_de(cmd, std::env::vars_os());
}

/// Nombre de este equipo, como lo anota restic en sus bloqueos
/// («locked by PID … on EQUIPO»). Vacío si no se sabe.
pub fn hostname() -> String {
    #[cfg(windows)]
    {
        std::env::var("COMPUTERNAME").unwrap_or_default()
    }
    #[cfg(not(windows))]
    {
        std::fs::read_to_string("/proc/sys/kernel/hostname")
            .or_else(|_| std::fs::read_to_string("/etc/hostname"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn este_proceso_sigue_vivo() {
        assert!(process_alive(std::process::id()));
        // Un PID que no existe (el máximo de Linux es 2^22; en Windows los PID son múltiplos de 4).
        #[cfg(any(windows, target_os = "linux"))]
        assert!(!process_alive(4_194_303 * 4 + 1));
    }

    #[test]
    fn entorno_minimo_sin_variables_peligrosas() {
        let os = |s: &str| std::ffi::OsString::from(s);
        let vars = [
            "PATH",
            "SystemRoot",
            "TEMP",
            "HOME",
            "https_proxy",
            "RESTIC_PASSWORD_COMMAND",
            "RESTIC_REPOSITORY",
            "RCLONE_LOG_FILE",
            "AWS_SECRET_ACCESS_KEY",
            "GOOGLE_APPLICATION_CREDENTIALS",
            "SSL_CERT_FILE",
            "GODEBUG",
            "SQLCMDINI",
            "SQLCMDPASSWORD",
            "LD_PRELOAD",
        ]
        .map(|k| (os(k), os("x")));
        let mut c = std::process::Command::new("programa");
        entorno_minimo_de(&mut c, vars);
        c.env("RESTIC_PASSWORD", "la que pone Resguardo");
        let puestas: Vec<String> = c.get_envs().filter(|(_, v)| v.is_some()).map(|(k, _)| k.to_string_lossy().to_ascii_uppercase()).collect();
        for k in ["PATH", "HTTPS_PROXY", "RESTIC_PASSWORD"] {
            assert!(puestas.contains(&k.to_string()), "{k}: {puestas:?}");
        }
        for k in
            ["RESTIC_PASSWORD_COMMAND", "RESTIC_REPOSITORY", "RCLONE_LOG_FILE", "AWS_SECRET_ACCESS_KEY", "SSL_CERT_FILE", "GODEBUG", "SQLCMDINI", "LD_PRELOAD"]
        {
            assert!(!puestas.contains(&k.to_string()), "{k} no debe pasar");
        }
        // En Windows, los nombres sin distinguir mayúsculas.
        assert_eq!(variable_permitida(std::ffi::OsStr::new("SystemRoot")), cfg!(windows));
        assert!(variable_permitida(std::ffi::OsStr::new("SYSTEMROOT")));
    }

    #[test]
    fn nombre_del_equipo() {
        #[cfg(any(windows, target_os = "linux"))]
        assert!(!hostname().is_empty());
    }
}
