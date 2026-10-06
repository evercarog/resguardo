//! Resguardo Server como servicio de Windows («ResguardoServer»): arranque
//! automático, reinicio si falla, datos en `C:\ProgramData\Resguardo Server`
//! (solo SYSTEM y Administradores) y una regla del cortafuegos para su puerto.

use crate::{arrancar, datos_por_defecto, Config};
use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
use std::time::Duration;
use windows_service::service::{
    ServiceAccess, ServiceAction, ServiceActionType, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode, ServiceFailureActions,
    ServiceFailureResetPeriod, ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
use windows_service::{define_windows_service, service_dispatcher};

pub const NOMBRE: &str = "ResguardoServer";
const REGLA: &str = "Resguardo Server";

fn sistema(exe: &str) -> std::path::PathBuf {
    Path::new(&std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join("System32").join(exe)
}

fn herramienta(exe: &str, args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new(sistema(exe)).args(args).output().map_err(|e| format!("No se pudo ejecutar {exe}: {e}"))?;
    let texto = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    if out.status.success() {
        Ok(texto)
    } else {
        Err(format!("{exe}: {}", texto.trim()))
    }
}

/// Registro del servicio (no tiene consola): `servidor.log` en la carpeta de datos.
fn registro(datos: &Path, linea: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(datos.join("servidor.log")) {
        let _ = writeln!(f, "{} {linea}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    }
}

/// ¿Es un enlace (unión, enlace simbólico u otro punto de análisis)?
fn es_enlace(m: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    m.file_type().is_symlink() || m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

/// El primer enlace que haya en `dir` o dentro (sin seguir ninguno).
fn enlace_dentro(dir: &Path) -> Option<std::path::PathBuf> {
    let mut pendientes = vec![dir.to_path_buf()];
    while let Some(d) = pendientes.pop() {
        let m = std::fs::symlink_metadata(&d).ok()?;
        if es_enlace(&m) {
            return Some(d);
        }
        if m.is_dir() {
            for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                pendientes.push(e.path());
            }
        }
    }
    None
}

/// La carpeta de datos, solo para SYSTEM y Administradores (sin herencia).
///
/// Si ya existía (p. ej. creada antes por un usuario, que puede crear carpetas
/// en ProgramData), primero pasa a ser de Administradores y pierde los permisos
/// explícitos que tuviera; y no se sigue ningún enlace de dentro: con una unión
/// en `tls` o `clientes`, SYSTEM escribiría sus claves donde quiera ese usuario.
pub fn proteger_carpeta(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("No se pudo crear {}: {e}", dir.display()))?;
    if let Some(e) = enlace_dentro(dir) {
        return Err(format!("{} es un enlace (unión o simbólico): quítalo y vuelve a instalar. La carpeta de datos no puede tener enlaces.", e.display()));
    }
    let d = dir.display().to_string();
    // Dueño: Administradores (el anterior dueño ya no puede cambiar los permisos).
    herramienta("icacls.exe", &[&d, "/setowner", "*S-1-5-32-544", "/C", "/Q"])?;
    // Fuera los permisos explícitos que tuviera (p. ej. «Todos: control total»)...
    herramienta("icacls.exe", &[&d, "/reset", "/C", "/Q"])?;
    // ... y sin herencia: solo SYSTEM y Administradores, heredable.
    herramienta("icacls.exe", &[&d, "/inheritance:r", "/grant:r", "*S-1-5-18:(OI)(CI)F", "*S-1-5-32-544:(OI)(CI)F", "/C", "/Q"])?;
    // Lo de dentro, que herede de ella. (Con /T, los permisos (OI)(CI) no
    // valen para archivos y quedaban sin ninguno: ni SYSTEM podía abrirlos.)
    if std::fs::read_dir(dir).is_ok_and(|mut it| it.next().is_some()) {
        herramienta("icacls.exe", &[&format!("{d}\\*"), "/setowner", "*S-1-5-32-544", "/T", "/L", "/C", "/Q"])?;
        herramienta("icacls.exe", &[&format!("{d}\\*"), "/reset", "/T", "/L", "/C", "/Q"])?;
    }
    Ok(())
}

// ---------- El servicio ----------

static mut CONFIG: Option<Config> = None;
static ESTADO: std::sync::OnceLock<service_control_handler::ServiceStatusHandle> = std::sync::OnceLock::new();

fn parado(codigo: u32) -> ServiceStatus {
    ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(codigo),
        checkpoint: 0,
        wait_hint: Duration::from_secs(10),
        process_id: None,
    }
}

/// Lo que tiene el servidor para parar ordenadamente antes de salir igualmente.
const PLAZO_PARADA: Duration = Duration::from_secs(8);

/// «Parándose», con el plazo que se dice al administrador de servicios (más que `PLAZO_PARADA`).
fn parandose() -> ServiceStatus {
    ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StopPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 1,
        wait_hint: PLAZO_PARADA + Duration::from_secs(4),
        process_id: None,
    }
}

define_windows_service!(ffi_principal, principal_servicio);

pub fn ejecutar(c: Config) -> Result<(), String> {
    // SAFETY: se escribe una sola vez, antes de que el dispatcher cree el hilo del servicio.
    unsafe {
        CONFIG = Some(c);
    }
    service_dispatcher::start(NOMBRE, ffi_principal).map_err(|e| format!("No se pudo arrancar el servicio: {e}"))
}

fn principal_servicio(_args: Vec<OsString>) {
    // SAFETY: solo se lee aquí, después de escribirse en `ejecutar`.
    #[allow(static_mut_refs)]
    let Some(c) = (unsafe { CONFIG.take() }) else {
        return;
    };
    let datos = c.datos.clone();
    let datos_parada = c.datos.clone();
    let manejador = move |control| match control {
        ServiceControl::Stop | ServiceControl::Shutdown => {
            // Antes, «Parado» y exit(0) aquí mismo, desde el hilo del manejador: el
            // administrador de servicios podía ver el proceso aún vivo con el servicio ya
            // «parado» y `Restart-Service` fallaba (había que pulsar «Iniciar»). Ahora:
            // «parándose» con su plazo, el servidor deja de escuchar y `principal_servicio`
            // dice «Parado» y vuelve. Si en `PLAZO_PARADA` no lo ha hecho, se sale igual.
            if let Some(e) = ESTADO.get() {
                let _ = e.set_service_status(parandose());
            }
            registro(&datos_parada, "Parando el servicio…");
            resguardo_servidor::pedir_parada();
            std::thread::spawn(|| {
                std::thread::sleep(PLAZO_PARADA);
                if let Some(e) = ESTADO.get() {
                    let _ = e.set_service_status(parado(0));
                }
                std::process::exit(0);
            });
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        _ => ServiceControlHandlerResult::NotImplemented,
    };
    let Ok(estado) = service_control_handler::register(NOMBRE, manejador) else { return };
    let _ = ESTADO.set(estado);
    let _ = estado.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(10),
        process_id: None,
    });
    let r = arrancar(c, &|l| registro(&datos, l));
    match &r {
        Err(e) => registro(&datos, &format!("ERROR: {e}")),
        Ok(()) => registro(&datos, "Servicio parado."),
    }
    let _ = estado.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        // Con error: el administrador de servicios lo vuelve a arrancar (acciones de recuperación).
        exit_code: ServiceExitCode::Win32(if r.is_err() { 1 } else { 0 }),
        checkpoint: 0,
        wait_hint: Duration::from_secs(10),
        process_id: None,
    });
    if r.is_err() {
        std::process::exit(1);
    }
}

// ---------- Instalar y desinstalar ----------

pub fn es_administrador() -> bool {
    herramienta("net.exe", &["session"]).is_ok()
}

/// `--instalar-servicio`: datos protegidos, servicio con reinicio si falla, regla del cortafuegos y arranque.
pub fn instalar(c: &Config, toda_la_red: bool) -> Result<(), String> {
    if !es_administrador() {
        return Err("Ejecútalo como administrador.".into());
    }
    if std::net::TcpListener::bind(c.escuchar).is_err() && !servicio_existe() {
        return Err(format!("El puerto {} ya está en uso en este equipo: elige otro con --escuchar.", c.escuchar.port()));
    }
    proteger_carpeta(&c.datos)?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut args = vec![
        OsString::from("--servicio"),
        OsString::from("--datos"),
        c.datos.clone().into_os_string(),
        OsString::from("--escuchar"),
        OsString::from(c.escuchar.to_string()),
    ];
    for n in &c.nombres {
        args.extend([OsString::from("--nombre"), OsString::from(n)]);
    }
    if let Some(d) = &c.consola {
        args.extend([OsString::from("--consola"), d.clone().into_os_string()]);
    }
    args.extend([OsString::from("--max-descarga"), OsString::from(c.max_mb.to_string())]);
    let gestor = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE)
        .map_err(|e| format!("Sin acceso al administrador de servicios: {e}"))?;
    let info = ServiceInfo {
        name: OsString::from(NOMBRE),
        display_name: OsString::from("Resguardo Server"),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: exe,
        launch_arguments: args,
        dependencies: vec![],
        account_name: None, // LocalSystem
        account_password: None,
    };
    let acceso = ServiceAccess::CHANGE_CONFIG | ServiceAccess::START | ServiceAccess::STOP | ServiceAccess::QUERY_STATUS;
    let svc = match gestor.open_service(NOMBRE, acceso) {
        // Ya instalado (al actualizar): se para, se cambia la configuración y se vuelve a arrancar.
        Ok(svc) => {
            let _ = svc.stop();
            esperar(&svc, ServiceState::Stopped);
            svc.change_config(&info).map_err(|e| format!("No se pudo actualizar el servicio: {e}"))?;
            svc
        }
        Err(_) => gestor.create_service(&info, acceso).map_err(|e| format!("No se pudo crear el servicio: {e}"))?,
    };
    let _ = svc.set_description("Consola web de Resguardo y canal de los agentes (copias de seguridad gestionadas).");
    let reiniciar = || ServiceAction { action_type: ServiceActionType::Restart, delay: Duration::from_secs(60) };
    let _ = svc.update_failure_actions(ServiceFailureActions {
        reset_period: ServiceFailureResetPeriod::After(Duration::from_secs(86_400)),
        reboot_msg: None,
        command: None,
        actions: Some(vec![reiniciar(), reiniciar(), reiniciar()]),
    });
    let _ = svc.set_failure_actions_on_non_crash_failures(true);
    regla_cortafuegos(c.escuchar.port(), toda_la_red)?;
    svc.start::<&str>(&[]).map_err(|e| format!("No se pudo arrancar el servicio: {e}"))?;
    esperar(&svc, ServiceState::Running);
    // El código de primer arranque (solo si aún no hay cuentas).
    let archivo = crate::archivo_codigo(&c.datos);
    for _ in 0..40 {
        if archivo.is_file() || c.datos.join("control.db").is_file() {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    println!("Resguardo Server está en marcha como servicio «{NOMBRE}».");
    println!("Consola: https://<este equipo>:{}/", c.escuchar.port());
    if archivo.is_file() {
        println!("Código de primer arranque (solo administradores): {}", archivo.display());
    }
    Ok(())
}

fn servicio_existe() -> bool {
    ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT).is_ok_and(|g| g.open_service(NOMBRE, ServiceAccess::QUERY_STATUS).is_ok())
}

fn esperar(svc: &windows_service::service::Service, estado: ServiceState) {
    for _ in 0..60 {
        if svc.query_status().is_ok_and(|s| s.current_state == estado) {
            return;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// Regla de entrada solo para el puerto del servidor (TCP), de las redes internas (la subred y las privadas) salvo `toda_la_red`.
pub fn regla_cortafuegos(puerto: u16, toda_la_red: bool) -> Result<(), String> {
    let _ = herramienta("netsh.exe", &["advfirewall", "firewall", "delete", "rule", &format!("name={REGLA}")]);
    herramienta(
        "netsh.exe",
        &[
            "advfirewall",
            "firewall",
            "add",
            "rule",
            &format!("name={REGLA}"),
            "dir=in",
            "action=allow",
            "protocol=TCP",
            &format!("localport={puerto}"),
            // Otras subredes o VLAN de la empresa también; internet nunca.
            if toda_la_red { "remoteip=any" } else { "remoteip=localsubnet,10.0.0.0/8,172.16.0.0/12,192.168.0.0/16" },
        ],
    )
    .map(|_| ())
    .map_err(|e| format!("No se pudo crear la regla del cortafuegos: {e}"))
}

/// `--desinstalar-servicio`: lo para y lo quita, con su regla. Los datos se quedan.
pub fn desinstalar() -> Result<(), String> {
    if !es_administrador() {
        return Err("Ejecútalo como administrador.".into());
    }
    let gestor = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT).map_err(|e| e.to_string())?;
    if let Ok(svc) = gestor.open_service(NOMBRE, ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS) {
        let _ = svc.stop();
        esperar(&svc, ServiceState::Stopped);
        svc.delete().map_err(|e| format!("No se pudo quitar el servicio: {e}"))?;
    }
    let _ = herramienta("netsh.exe", &["advfirewall", "firewall", "delete", "rule", &format!("name={REGLA}")]);
    println!("Servicio quitado. Los datos siguen en {}.", datos_por_defecto().display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encuentra_enlaces_en_la_carpeta_de_datos() {
        let base = std::env::temp_dir().join(format!("resguardo-srv-enlaces-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let datos = base.join("datos");
        std::fs::create_dir_all(datos.join("clientes")).unwrap();
        std::fs::write(datos.join("clientes").join("c.db"), b"x").unwrap();
        assert_eq!(enlace_dentro(&datos), None);
        // Una unión (no hace falta ser administrador para crearla).
        let fuera = base.join("fuera");
        std::fs::create_dir_all(&fuera).unwrap();
        let tls = datos.join("tls");
        let ok = std::process::Command::new(sistema("cmd.exe")).arg("/c").arg("mklink").arg("/J").arg(&tls).arg(&fuera).output().unwrap().status.success();
        assert!(ok, "mklink /J");
        assert_eq!(enlace_dentro(&datos), Some(tls.clone()));
        assert!(proteger_carpeta(&datos).unwrap_err().contains("enlace"));
        std::fs::remove_dir(&tls).unwrap();
        let _ = std::fs::remove_dir_all(&base);
    }
}
