//! `resguardo-agente.exe`: el Resguardo Agente de un equipo gestionado (fase
//! 5, docs/agente-gestionado.md). Sin WebView2: un servicio de Windows que
//! aplica lo que firma la consola y hace las copias con el agente de siempre,
//! y un icono nativo en la bandeja con un estado tranquilo.

use std::time::Duration;

pub const SERVICE_NAME: &str = "ResguardoAgente";
const TICK: Duration = Duration::from_secs(5 * 60);

/// Punto de entrada del binario. Devuelve el código de salida.
pub fn main() -> i32 {
    let args: Vec<String> = std::env::args().collect();
    let has = |a: &str| args.iter().any(|x| x == a);
    let value = |a: &str| args.iter().position(|x| x == a).and_then(|i| args.get(i + 1)).cloned();
    // Línea de órdenes en español (estado, copias, vincular…).
    if args.get(1).is_some_and(|a| crate::cli::ORDENES.contains(&a.as_str())) {
        return crate::cli::ejecutar(&args[1..]);
    }
    // --help, -h, --version…: la ayuda o la versión en la terminal (no una ventana).
    if let Some(orden) = args.get(1).and_then(|a| crate::cli::alias(a)) {
        return crate::cli::ejecutar(&[orden.to_string()]);
    }
    // El agente de siempre se lanza a sí mismo con estos modos.
    if has("--agent-run") {
        return crate::agent_main();
    }
    if has("--agent-tasks") {
        return crate::agent_tasks_main();
    }
    // El Servidor de copias (lo lanza su tarea o su servicio de systemd).
    if has("--server-run") {
        return crate::server_main();
    }
    // El desinstalador, si se pide borrar también los datos.
    if has("--agent-purge") {
        let _ = service::uninstall();
        return crate::agent_purge();
    }
    if let Some(code) = value("--pair") {
        let (url, key) = crate::web::defaults();
        let (url, key) = (value("--web-url").unwrap_or(url), value("--web-key").unwrap_or(key));
        let name = value("--name").unwrap_or_else(crate::web::default_device_name);
        // Con --result <archivo> (el instalador), el resultado va al archivo y
        // no se muestra ninguna ventana: «ok», el código de comprobación y la
        // consola, o «error» y el motivo, una cosa por línea.
        let result_file = value("--result");
        let outcome = crate::endpoint::pair(&url, &key, &code, &name);
        if let Some(path) = result_file {
            let text = match &outcome {
                Ok((console, sas)) => format!("ok\r\n{sas}\r\n{console}\r\n"),
                Err(e) => format!("error\r\n{}\r\n", e.replace(['\r', '\n'], " ")),
            };
            let _ = std::fs::write(path, text);
            return i32::from(outcome.is_err());
        }
        return match outcome {
            Ok((console, sas)) => {
                message(&format!(
                    "Este equipo está emparejado con «{console}».\n\nCódigo de comprobación: {sas}\n\nComprueba en la consola que ve el mismo código antes de confirmar."
                ));
                0
            }
            Err(e) => {
                message(&format!("No se pudo emparejar: {e}"));
                1
            }
        };
    }
    if has("--unpair") {
        return match crate::endpoint::unpair() {
            Ok(()) => {
                message("Este equipo ya no está gestionado y ha dejado de copiar.");
                0
            }
            Err(e) => {
                message(&format!("No se pudo desemparejar: {e}"));
                1
            }
        };
    }
    if has("--install-service") {
        return report(service::install());
    }
    if has("--uninstall-service") {
        return report(service::uninstall());
    }
    if has("--service") {
        return report(service::run());
    }
    if has("--tray") {
        return report(tray::run());
    }
    // La ventana (docs/agente-ventana.md): la abre la bandeja, como el usuario.
    if has("--ventana") {
        return report(crate::ventana::run());
    }
    // En primer plano, sin servicio (pruebas o Linux sin systemd): el canal con
    // el servidor y una vuelta del agente cada `--cada` segundos (300 por defecto).
    if has("--primer-plano") {
        let cada = value("--cada").and_then(|s| s.parse::<u64>().ok()).unwrap_or(300).max(10);
        crate::servidor_v2::hilo();
        crate::ipc_local::hilo();
        #[cfg(windows)]
        hilo_bandeja();
        // En pruebas, el Servidor de copias corre dentro de este proceso.
        if crate::agent::test_mode() && crate::server::load().enabled {
            std::thread::spawn(crate::server::run_forever);
        }
        loop {
            one_tick();
            std::thread::sleep(Duration::from_secs(cada));
        }
    }
    if has("--once") {
        // Una vuelta a mano (para probar): recoger lo de la consola o del servidor y copiar lo pendiente.
        if let Err(e) = crate::servidor_v2::ronda() {
            crate::agent::log(&format!("Servidor: {e}"));
        }
        one_tick();
        return 0;
    }
    message(
        "Resguardo Agente funciona solo, como servicio, y se administra desde la consola.\n\nPara ver su estado o vincularlo, abre el símbolo del sistema como administrador y escribe:\n\n  resguardo-agente ayuda",
    );
    0
}

fn report(r: Result<(), String>) -> i32 {
    match r {
        Ok(()) => 0,
        Err(e) => {
            crate::agent::log(&format!("ERROR: Resguardo Agente: {e}"));
            1
        }
    }
}

/// La bandeja al día ya (tras un cambio desde la ventana).
pub fn refrescar_bandeja() {
    write_tray_status();
}

/// Escribe lo que muestra la bandeja (`bandeja::ARCHIVO`). Lo hace el
/// servicio, que puede leer el estado cifrado; la bandeja corre como el
/// usuario y solo lee este archivo, sin rutas ni mensajes del agente.
fn write_tray_status() -> crate::bandeja::EstadoBandeja {
    // La vuelta del servicio y el hilo de la bandeja no escriben a la vez (mismo archivo temporal).
    static ESCRIBIENDO: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _turno = ESCRIBIENDO.lock().unwrap_or_else(|e| e.into_inner());
    let fase5 = crate::endpoint::load();
    let vinculo = crate::servidor_v2::cargar();
    let (config, estado, tareas) = (crate::agent::load_config(), crate::agent::load_state(), crate::tasks::load_state());
    let fuentes = crate::bandeja::Fuentes {
        fase5: fase5.as_ref(),
        vinculo: vinculo.as_ref(),
        config: &config,
        estado: &estado,
        tareas: &tareas,
        solicitudes: crate::agent::requests_dir().is_dir(),
    };
    let e = crate::bandeja::componer(&fuentes, &chrono::Local::now().fixed_offset());
    let _ = crate::agent::write_json(crate::bandeja::ARCHIVO, &e);
    e
}

/// Mientras corre el servicio, la bandeja al día cada medio minuto (también
/// durante una copia, para el progreso, y con la fecha que le dice que el
/// servicio sigue vivo). Con algo en marcha y la ventana o los avisos de
/// «empezó» encendidos, cada 2 s, con la serie del ritmo para la ventana
/// (docs/agente-ventana.md §7); con todo apagado, como siempre.
#[cfg(windows)]
fn hilo_bandeja() {
    std::thread::spawn(|| {
        let mut ventana = Ventana::default();
        loop {
            let e = write_tray_status();
            let rapido = crate::escritorio::muestreo_rapido(e.escritorio, !e.actividades.is_empty());
            ventana.escribir(&e);
            if rapido {
                std::thread::sleep(Duration::from_secs(2));
                continue;
            }
            if !crate::escritorio::muestreo_rapido(e.escritorio, true) {
                // Ventana y avisos de «empezó» apagados: como siempre.
                std::thread::sleep(Duration::from_secs(30));
                continue;
            }
            // Si no, entre vuelta y vuelta tranquila se mira cada 2 s si empezó algo
            // (leer los estados pequeños que ya escriben los procesos del agente).
            for _ in 0..15 {
                std::thread::sleep(Duration::from_secs(2));
                // Con las mismas reglas de «viva» (un estado que dejó un proceso cortado no cuenta).
                let (config, estado, tareas) = (crate::agent::load_config(), crate::agent::load_state(), crate::tasks::load_state());
                if !crate::escritorio::actividades_de(&config, &estado, &tareas).is_empty() {
                    break;
                }
            }
        }
    });
}

/// `gestionado-ventana.json`: la serie del ritmo y el historial pequeño. Solo
/// si la ventana no está apagada (si lo está, se borra y no se muestrea nada).
#[derive(Default)]
struct Ventana {
    serie: crate::escritorio::Serie,
    /// El historial se lee como mucho cada minuto (o al terminar una copia).
    historial: Option<(std::time::Instant, usize, Vec<crate::escritorio::Dia>)>,
}

impl Ventana {
    fn escribir(&mut self, e: &crate::bandeja::EstadoBandeja) {
        let archivo = crate::agent::agent_dir().join(crate::escritorio::ARCHIVO_VENTANA);
        if e.escritorio.ventana == crate::escritorio::Ventana::Off {
            self.serie = Default::default();
            let _ = std::fs::remove_file(archivo);
            return;
        }
        let ahora = chrono::Local::now();
        self.serie.muestra(ahora.timestamp(), &e.actividades);
        let hechas = e.hechas.len() + e.hechas.iter().map(|h| h.cuando.len()).sum::<usize>();
        let viejo = self.historial.as_ref().is_none_or(|(t, n, _)| t.elapsed() >= Duration::from_secs(60) || *n != hechas);
        if viejo {
            let dias = crate::escritorio::historial(&crate::history::read(&crate::history::agent_file()), ahora.date_naive());
            self.historial = Some((std::time::Instant::now(), hechas, dias));
        }
        let estado = crate::escritorio::EstadoVentana {
            v: 1,
            escrito: Some(ahora.to_rfc3339()),
            serie: self.serie.puntos.iter().cloned().collect(),
            historial: self.historial.as_ref().map(|h| h.2.clone()).unwrap_or_default(),
        };
        let _ = crate::agent::write_json(crate::escritorio::ARCHIVO_VENTANA, &estado);
    }
}

/// Una vuelta: lo firmado por la consola y, después, el ciclo del agente de
/// siempre en su propio proceso (copias pendientes, reintentos, informe).
fn one_tick() {
    crate::espejo::si_toca();
    // v1.22: la retención de los repositorios que guarda este almacén, a su hora.
    crate::retencion_almacen::si_toca();
    for note in crate::endpoint::tick() {
        crate::agent::log(&note);
        let now = chrono::Local::now().to_rfc3339();
        crate::history::append(
            &crate::history::agent_file(),
            &crate::history::Entry {
                kind: "config".into(),
                origin: "agent".into(),
                repo_id: crate::endpoint::REPO_ID.into(),
                repo_name: "Copias gestionadas".into(),
                started: now.clone(),
                finished: now,
                result: "info".into(),
                message: note,
                ..Default::default()
            },
        );
    }
    write_tray_status();
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).arg("--agent-run").stdin(std::process::Stdio::null()).status();
    }
    write_tray_status();
}

/// Estado del servicio para el informe a la web («detenido por un administrador»).
pub fn set_service_running(running: bool) {
    let _ = crate::agent::write_json("gestionado-servicio.json", &serde_json::json!({ "running": running, "at": chrono::Local::now().to_rfc3339() }));
}

pub fn service_running() -> Option<bool> {
    std::fs::read(crate::agent::agent_dir().join("gestionado-servicio.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v["running"].as_bool())
}

/// Un mensaje al usuario (el binario no tiene consola).
fn message(text: &str) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};
        let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
        let (t, c) = (wide(text), wide("Resguardo Agente"));
        unsafe {
            MessageBoxW(std::ptr::null_mut(), t.as_ptr(), c.as_ptr(), MB_OK | MB_ICONINFORMATION);
        }
    }
    #[cfg(not(windows))]
    eprintln!("{text}");
}

#[cfg(windows)]
mod service {
    use super::*;
    use std::ffi::OsString;
    use std::sync::mpsc;
    use windows_service::service::{
        ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode, ServiceInfo, ServiceStartType, ServiceState, ServiceStatus,
        ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
    use windows_service::{define_windows_service, service_dispatcher};

    define_windows_service!(ffi_service_main, service_main);

    /// Marca que deja el instalador antes de parar el servicio para actualizarlo.
    const MARCA_ACTUALIZACION: &str = "actualizando";

    /// ¿Lo para el instalador (marca de hace menos de 10 minutos)? La marca se consume.
    fn actualizando() -> bool {
        let marca = crate::agent::agent_dir().join(MARCA_ACTUALIZACION);
        let reciente = std::fs::metadata(&marca).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|d| d < Duration::from_secs(600));
        let _ = std::fs::remove_file(&marca);
        reciente
    }

    pub fn run() -> Result<(), String> {
        service_dispatcher::start(SERVICE_NAME, ffi_service_main).map_err(|e| format!("No se pudo arrancar el servicio: {e}"))
    }

    fn service_main(_args: Vec<OsString>) {
        let (stop_tx, stop_rx) = mpsc::channel();
        let handler = move |control| match control {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                let _ = stop_tx.send(control);
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        };
        let Ok(status) = service_control_handler::register(SERVICE_NAME, handler) else { return };
        let set = |state: ServiceState| {
            let _ = status.set_service_status(ServiceStatus {
                service_type: ServiceType::OWN_PROCESS,
                current_state: state,
                controls_accepted: if state == ServiceState::Running {
                    ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN
                } else {
                    ServiceControlAccept::empty()
                },
                exit_code: ServiceExitCode::Win32(0),
                checkpoint: 0,
                wait_hint: Duration::from_secs(10),
                process_id: None,
            });
        };
        set(ServiceState::Running);
        // La marca del instalador ya cumplió (o es de una actualización que no llegó a parar el servicio).
        let _ = std::fs::remove_file(crate::agent::agent_dir().join(MARCA_ACTUALIZACION));
        crate::agent::migrar_carpeta_fase5();
        // Agente v2: el canal con Resguardo Server (si el equipo está vinculado a uno).
        crate::servidor_v2::hilo();
        hilo_bandeja();
        // La ventana del equipo: ajustes y modo local con la clave (docs/agente-ventana.md §4).
        crate::ipc_local::hilo();
        set_service_running(true);
        crate::agent::log("Resguardo Agente: servicio en marcha.");
        loop {
            one_tick();
            match stop_rx.recv_timeout(crate::agent::wait_until_next_slot(TICK, chrono::Local::now())) {
                Ok(ServiceControl::Stop) if actualizando() => {
                    // Lo para el instalador para actualizarlo: vuelve en unos segundos.
                    crate::agent::log("Resguardo Agente: parado para actualizarse.");
                    break;
                }
                Ok(ServiceControl::Stop) => {
                    // Lo para un administrador: la consola y la web lo sabrán.
                    set_service_running(false);
                    crate::agent::log("Resguardo Agente: servicio detenido por un administrador.");
                    let _ = crate::web::report_now();
                    break;
                }
                Ok(_) => break, // apagado del equipo: no es «detenido por un administrador»
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => break,
            }
        }
        set(ServiceState::Stopped);
    }

    /// Si el servicio falla, Windows lo vuelve a arrancar al minuto (tres veces; la cuenta se reinicia cada día).
    fn recuperacion(svc: &windows_service::service::Service) {
        use windows_service::service::{ServiceAction, ServiceActionType, ServiceFailureActions, ServiceFailureResetPeriod};
        let reiniciar = || ServiceAction { action_type: ServiceActionType::Restart, delay: Duration::from_secs(60) };
        let _ = svc.update_failure_actions(ServiceFailureActions {
            reset_period: ServiceFailureResetPeriod::After(Duration::from_secs(86_400)),
            reboot_msg: None,
            command: None,
            actions: Some(vec![reiniciar(), reiniciar(), reiniciar()]),
        });
        let _ = svc.set_failure_actions_on_non_crash_failures(true);
    }

    pub fn install() -> Result<(), String> {
        crate::agent::require_admin()?;
        // Un servicio de SYSTEM con el ejecutable en una carpeta que un
        // usuario puede cambiar le daría control del equipo (como las tareas).
        crate::platform::check_task_exe()?;
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE)
            .map_err(|e| format!("Sin acceso al administrador de servicios: {e}"))?;
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        // Ya instalado (al actualizar): acciones de recuperación al día y se arranca.
        if let Ok(svc) = manager.open_service(SERVICE_NAME, ServiceAccess::START | ServiceAccess::QUERY_STATUS | ServiceAccess::CHANGE_CONFIG) {
            recuperacion(&svc);
            if svc.query_status().is_ok_and(|s| s.current_state != ServiceState::Running) {
                let _ = svc.start::<&str>(&[]);
            }
            return Ok(());
        }
        let info = ServiceInfo {
            name: OsString::from(SERVICE_NAME),
            display_name: OsString::from("Resguardo Agente"),
            service_type: ServiceType::OWN_PROCESS,
            start_type: ServiceStartType::AutoStart,
            error_control: ServiceErrorControl::Normal,
            executable_path: exe,
            launch_arguments: vec![OsString::from("--service")],
            dependencies: vec![],
            account_name: None, // LocalSystem
            account_password: None,
        };
        let svc =
            manager.create_service(&info, ServiceAccess::CHANGE_CONFIG | ServiceAccess::START).map_err(|e| format!("No se pudo crear el servicio: {e}"))?;
        let _ = svc.set_description("Copias de seguridad gestionadas por la consola de Resguardo.");
        recuperacion(&svc);
        let _ = svc.start::<&str>(&[]);
        Ok(())
    }

    pub fn uninstall() -> Result<(), String> {
        crate::agent::require_admin()?;
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT).map_err(|e| e.to_string())?;
        if let Ok(svc) = manager.open_service(SERVICE_NAME, ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS) {
            let _ = svc.stop();
            svc.delete().map_err(|e| format!("No se pudo quitar el servicio: {e}"))?;
        }
        Ok(())
    }
}

/// Linux (y otros Unix): servicio de systemd (`resguardo-agente.service`,
/// packaging/linux). Corre como root, con `--service`, en primer plano.
#[cfg(unix)]
mod service {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    static PARAR: AtomicBool = AtomicBool::new(false);

    extern "C" fn al_recibir_senal(_: libc::c_int) {
        PARAR.store(true, Ordering::SeqCst);
    }

    /// Unidad de systemd que instala `--install-service` (la misma que lleva el paquete).
    pub const UNIDAD: &str = include_str!("../../../packaging/linux/resguardo-agente.service");
    const RUTA_UNIDAD: &str = "/etc/systemd/system/resguardo-agente.service";

    pub fn run() -> Result<(), String> {
        // SIGTERM (systemctl stop, apagado) y SIGINT (Ctrl+C a mano): salir al final de la espera.
        // SAFETY: el manejador solo toca un atómico.
        unsafe {
            libc::signal(libc::SIGTERM, al_recibir_senal as *const () as libc::sighandler_t);
            libc::signal(libc::SIGINT, al_recibir_senal as *const () as libc::sighandler_t);
        }
        crate::servidor_v2::hilo();
        set_service_running(true);
        crate::agent::log("Resguardo Agente: servicio en marcha.");
        'vueltas: loop {
            one_tick();
            let inicio = std::time::Instant::now();
            // La vuelta normal, o antes si toca alguna copia («cada 5 minutos»…).
            let espera = crate::agent::wait_until_next_slot(TICK, chrono::Local::now());
            while inicio.elapsed() < espera {
                if PARAR.load(Ordering::SeqCst) {
                    break 'vueltas;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        }
        // Al apagar el equipo systemd está «stopping»: eso no es «detenido por un administrador».
        let apagando = std::process::Command::new(systemctl_bin())
            .arg("is-system-running")
            .output()
            .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "stopping");
        if !apagando {
            set_service_running(false);
            crate::agent::log("Resguardo Agente: servicio detenido por un administrador.");
            let _ = crate::web::report_now();
        }
        Ok(())
    }

    /// systemctl por su ruta, nunca del PATH (el agente es root).
    pub fn systemctl_bin() -> &'static str {
        ["/usr/bin/systemctl", "/bin/systemctl"].into_iter().find(|p| std::path::Path::new(p).is_file()).unwrap_or("/usr/bin/systemctl")
    }

    fn systemctl(args: &[&str]) -> Result<(), String> {
        let ok = std::process::Command::new(systemctl_bin()).args(args).status().map_err(|e| format!("No se pudo ejecutar systemctl: {e}"))?.success();
        if ok {
            Ok(())
        } else {
            Err(format!("systemctl {} ha fallado.", args.join(" ")))
        }
    }

    pub fn install() -> Result<(), String> {
        crate::agent::require_admin()?;
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let unidad = UNIDAD.replace("/opt/resguardo-agente/resguardo-agente", &exe.to_string_lossy());
        if std::fs::read_to_string(RUTA_UNIDAD).ok().as_deref() != Some(unidad.as_str()) {
            std::fs::write(RUTA_UNIDAD, unidad).map_err(|e| format!("No se pudo escribir {RUTA_UNIDAD}: {e}"))?;
            systemctl(&["daemon-reload"])?;
        }
        systemctl(&["enable", "--now", "resguardo-agente.service"])
    }

    pub fn uninstall() -> Result<(), String> {
        crate::agent::require_admin()?;
        if std::path::Path::new(RUTA_UNIDAD).exists() {
            let _ = systemctl(&["disable", "--now", "resguardo-agente.service"]);
            std::fs::remove_file(RUTA_UNIDAD).map_err(|e| format!("No se pudo quitar {RUTA_UNIDAD}: {e}"))?;
            let _ = systemctl(&["daemon-reload"]);
        }
        Ok(())
    }
}

#[cfg(not(any(windows, unix)))]
mod service {
    pub fn run() -> Result<(), String> {
        Err("Sistema no compatible.".into())
    }
    pub fn install() -> Result<(), String> {
        Err("Sistema no compatible.".into())
    }
    pub fn uninstall() -> Result<(), String> {
        Err("Sistema no compatible.".into())
    }
}

/// Fuera de Windows no hay icono en la bandeja (los servidores Linux no tienen escritorio).
#[cfg(not(windows))]
mod tray {
    pub fn run() -> Result<(), String> {
        Err("El icono de la bandeja solo existe en Windows.".into())
    }
}

#[cfg(windows)]
mod tray {
    use super::*;
    use crate::bandeja::{self, Accion, Entrada, EstadoBandeja, Variante};
    use std::collections::HashMap;
    use tray_icon::menu::{IsMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn leer() -> EstadoBandeja {
        std::fs::read(crate::agent::agent_dir().join(bandeja::ARCHIVO)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    fn ahora() -> chrono::DateTime<chrono::FixedOffset> {
        chrono::Local::now().fixed_offset()
    }

    fn windir() -> std::path::PathBuf {
        std::env::var_os("SystemRoot").map(std::path::PathBuf::from).unwrap_or_else(|| "C:\\Windows".into())
    }

    /// Nitidez con la escala de Windows (125 %, 150 %…): el icono se pinta
    /// al tamaño que pide la bandeja en vez de estirarse.
    fn lado_icono() -> u32 {
        use windows_sys::Win32::UI::HiDpi::{GetDpiForSystem, GetSystemMetricsForDpi, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_SYSTEM_AWARE};
        use windows_sys::Win32::UI::WindowsAndMessaging::SM_CXSMICON;
        // SAFETY: llamadas sin punteros; si fallan, se queda el tamaño por defecto.
        let lado = unsafe {
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE);
            GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForSystem())
        };
        if (16..=64).contains(&lado) {
            lado as u32
        } else {
            32
        }
    }

    fn icono(v: Variante, lado: u32) -> Option<tray_icon::Icon> {
        tray_icon::Icon::from_rgba(bandeja::pintar(v, lado), lado, lado).ok()
    }

    /// En los menús de Windows «&» marca la tecla de acceso: se dobla.
    fn literal(s: &str) -> String {
        s.replace('&', "&&")
    }

    /// El menú de Windows a partir de su descripción, y qué hace cada opción.
    fn construir(entradas: &[Entrada]) -> (Menu, HashMap<MenuId, Accion>) {
        fn items(entradas: &[Entrada], acciones: &mut HashMap<MenuId, Accion>) -> Vec<Box<dyn IsMenuItem>> {
            let mut out: Vec<Box<dyn IsMenuItem>> = Vec::new();
            for e in entradas {
                match e {
                    Entrada::Texto(t) => out.push(Box::new(MenuItem::new(literal(t), false, None))),
                    Entrada::Separador => out.push(Box::new(PredefinedMenuItem::separator())),
                    Entrada::Opcion(t, accion) => {
                        let item = MenuItem::new(literal(t), true, None);
                        acciones.insert(item.id().clone(), accion.clone());
                        out.push(Box::new(item));
                    }
                    Entrada::Submenu(t, sub) => {
                        let menu = Submenu::new(literal(t), true);
                        for item in items(sub, acciones) {
                            let _ = menu.append(item.as_ref());
                        }
                        out.push(Box::new(menu));
                    }
                }
            }
            out
        }
        let mut acciones = HashMap::new();
        let menu = Menu::new();
        for item in items(entradas, &mut acciones) {
            let _ = menu.append(item.as_ref());
        }
        (menu, acciones)
    }

    /// Un aviso de Windows sobre el icono (en Windows 10 y 11 sale como notificación).
    fn globo(tray: &tray_icon::TrayIcon, titulo: &str, texto: &str, error: bool) {
        use windows_sys::Win32::UI::Shell::{Shell_NotifyIconW, NIF_INFO, NIIF_ERROR, NIIF_INFO, NIM_MODIFY, NOTIFYICONDATAW};
        // SAFETY: estructura de datos simple, sin punteros que haya que rellenar.
        let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = tray.window_handle();
        // tray-icon numera sus iconos desde 1, y este proceso solo tiene uno.
        nid.uID = 1;
        nid.uFlags = NIF_INFO;
        nid.dwInfoFlags = if error { NIIF_ERROR } else { NIIF_INFO };
        for (dst, src) in nid.szInfoTitle.iter_mut().zip(bandeja::cortar(titulo, 63).encode_utf16()) {
            *dst = src;
        }
        for (dst, src) in nid.szInfo.iter_mut().zip(bandeja::cortar(texto, 255).encode_utf16()) {
            *dst = src;
        }
        // SAFETY: `nid` vive durante la llamada.
        unsafe {
            Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }

    /// Abre una dirección en el navegador. Si la bandeja corre como
    /// administrador (la abrió el instalador), a través del Explorador, que
    /// la abre con el usuario de la sesión y no como administrador.
    fn abrir(url: &str, elevado: bool) {
        if elevado {
            let _ = std::process::Command::new(windir().join("explorer.exe")).arg(url).spawn();
            return;
        }
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let (verbo, url) = (wide("open"), wide(url));
        // SAFETY: cadenas terminadas en cero que viven durante la llamada.
        unsafe {
            ShellExecuteW(std::ptr::null_mut(), verbo.as_ptr(), url.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL);
        }
    }

    /// El registro que pueden leer todos (sin rutas ni contraseñas), en el Bloc de notas.
    fn ver_registro() {
        let _ = std::process::Command::new(windir().join("System32").join("notepad.exe")).arg(crate::agent::agent_dir().join("agent.log")).spawn();
    }

    /// «Copiar ahora»: la misma solicitud que deja la app sin permisos de
    /// administrador; el agente solo la atiende si tiene programada esa copia.
    fn pedir_copias(claves: &[String]) -> Result<(), String> {
        for clave in claves {
            let (repo, plan) = clave.split_once('#').ok_or("Copia desconocida.")?;
            crate::agent::request_backup(repo, plan)?;
        }
        Ok(())
    }

    /// Abre la ventana (otro proceso, que solo vive mientras está abierta).
    fn abrir_ventana() {
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::process::Command::new(exe).arg("--ventana").spawn();
        }
    }

    /// Una sola bandeja por sesión (la del inicio de sesión y la que abre el instalador).
    fn single_instance() -> bool {
        use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
        use windows_sys::Win32::System::Threading::CreateMutexW;
        // En pruebas (carpeta de pruebas, solo en desarrollo), otro: no choca con la del agente instalado.
        let name = wide(if crate::agent::test_mode() { "Local\\ResguardoAgenteBandejaPruebas" } else { "Local\\ResguardoAgenteBandeja" });
        // El mutex vive mientras viva el proceso (no se cierra a propósito).
        unsafe {
            let h = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
            !h.is_null() && GetLastError() != ERROR_ALREADY_EXISTS
        }
    }

    pub fn run() -> Result<(), String> {
        if !single_instance() {
            return Ok(());
        }
        let lado = lado_icono();
        let elevado = crate::platform::is_elevated();
        let mut estado = leer();
        let mut variante = bandeja::variante(&estado, &ahora());
        let mut entradas = bandeja::menu(&estado, &ahora(), elevado);
        let (menu, mut acciones) = construir(&entradas);
        let mut tooltip = bandeja::tooltip(&estado, &ahora());
        let mut builder = tray_icon::TrayIconBuilder::new().with_tooltip(&tooltip).with_menu(Box::new(menu));
        if let Some(i) = icono(variante, lado) {
            builder = builder.with_icon(i);
        }
        // Con la ventana disponible, pulsar el icono la abre (el menú, con el botón derecho).
        let disponible = |e: &EstadoBandeja| e.escritorio.ventana != crate::escritorio::Ventana::Off && !elevado;
        let mut con_ventana = disponible(&estado);
        builder = builder.with_menu_on_left_click(!con_ventana);
        let tray = builder.build().map_err(|e| format!("No se pudo crear el icono de la bandeja: {e}"))?;
        // La consola puede ocultarlo.
        let mut visible = estado.show;
        let _ = tray.set_visible(visible);
        // Avisos nativos (WinRT) en su hilo; los que Windows no admita, como globo.
        let avisador = crate::ventana::Avisador::nuevo();
        // Las tareas ya en marcha al arrancar no abren la ventana ni avisan.
        let mut vistas = std::collections::HashSet::new();
        crate::escritorio::abrir_al_empezar(&mut vistas, &estado.actividades, estado.escritorio.ventana, true);
        // Bucle de mensajes de Windows (lo necesita el icono) y, cada pocos segundos, el estado al día
        // (cada segundo mientras algo está en marcha, para avisar y abrir la ventana enseguida).
        let mut ultima_lectura = std::time::Instant::now();
        loop {
            // SAFETY: bucle de mensajes estándar con un MSG propio.
            unsafe {
                use windows_sys::Win32::UI::WindowsAndMessaging::{DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE};
                let mut msg: MSG = std::mem::zeroed();
                while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            while let Ok(evento) = MenuEvent::receiver().try_recv() {
                match acciones.get(&evento.id) {
                    Some(Accion::Consola(url)) => abrir(url, elevado),
                    Some(Accion::Registro) => ver_registro(),
                    Some(Accion::Copiar(claves)) => match pedir_copias(claves) {
                        Ok(()) => globo(&tray, "Copia pedida", "Empieza en unos minutos. El icono dirá cuándo termina.", false),
                        Err(e) => globo(&tray, "No se pudo pedir la copia", &e, true),
                    },
                    // Hasta el próximo inicio de sesión (si se muestra o no lo decide la consola).
                    Some(Accion::Ocultar) => return Ok(()),
                    Some(Accion::Ventana) => abrir_ventana(),
                    None => {}
                }
            }
            while let Ok(evento) = tray_icon::TrayIconEvent::receiver().try_recv() {
                if let tray_icon::TrayIconEvent::Click { button: tray_icon::MouseButton::Left, button_state: tray_icon::MouseButtonState::Up, .. } = evento {
                    if con_ventana {
                        abrir_ventana();
                    }
                }
            }
            // Pulsar un aviso abre la ventana; los que no salieron como aviso nativo, como globo.
            while avisador.pulsados.try_recv().is_ok() {
                if con_ventana {
                    abrir_ventana();
                }
            }
            while let Ok(a) = avisador.globos.try_recv() {
                if visible {
                    globo(&tray, &a.titulo, &a.texto, a.error);
                }
            }
            let cada = if estado.actividades.is_empty() { 5 } else { 1 };
            if ultima_lectura.elapsed() >= Duration::from_secs(cada) {
                ultima_lectura = std::time::Instant::now();
                let nuevo = leer();
                let t = ahora();
                // Fallar, recuperarse, empezar y terminar, según `escritorio.avisos` (la bandeja y el
                // servicio son el mismo programa: siempre se entienden).
                if visible {
                    for a in crate::escritorio::avisos(&estado.actividades, &estado.hechas, &nuevo.actividades, &nuevo.hechas, nuevo.escritorio.avisos, false) {
                        avisador.avisar(a);
                    }
                }
                if crate::escritorio::abrir_al_empezar(&mut vistas, &nuevo.actividades, nuevo.escritorio.ventana, false) && !elevado {
                    abrir_ventana();
                }
                if disponible(&nuevo) != con_ventana {
                    con_ventana = disponible(&nuevo);
                    tray.set_show_menu_on_left_click(!con_ventana);
                }
                let v = bandeja::variante(&nuevo, &t);
                if v != variante {
                    variante = v;
                    let _ = tray.set_icon(icono(v, lado));
                }
                let texto = bandeja::tooltip(&nuevo, &t);
                if texto != tooltip {
                    let _ = tray.set_tooltip(Some(&texto));
                    tooltip = texto;
                }
                let nuevas = bandeja::menu(&nuevo, &t, elevado);
                if nuevas != entradas {
                    let (menu, a) = construir(&nuevas);
                    tray.set_menu(Some(Box::new(menu)));
                    acciones = a;
                    entradas = nuevas;
                }
                if nuevo.show != visible {
                    visible = nuevo.show;
                    let _ = tray.set_visible(visible);
                }
                estado = nuevo;
            }
            std::thread::sleep(Duration::from_millis(150));
        }
    }
}
