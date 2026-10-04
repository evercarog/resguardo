//! La ventana del agente (`resguardo-agente --ventana`) y los avisos nativos
//! de Windows (docs/agente-ventana.md).
//!
//! La ventana es **otro proceso**, como el usuario, que solo vive mientras
//! está abierta: el WebView2 del sistema (wry + tao) con una página metida en
//! el ejecutable (`crates/agente/ventana/`, compilada desde `consola/ventana`)
//! y servida con un protocolo propio (`http://resguardo.localhost/`, sin red).
//! Lee `gestionado-bandeja.json` y `gestionado-ventana.json` cada segundo y se
//! los pasa a la página. Lo que pide la página:
//! - «Copiar ahora»: la solicitud de siempre (`agent::request_backup`);
//! - con la clave de administración (`ipc_local`): ajustes del escritorio y,
//!   en modo local, las copias. La prueba se calcula aquí (Argon2id) y se
//!   guarda en memoria mientras la ventana sigue «desbloqueada» (10 minutos
//!   sin usarla): nunca la clave.

// La página de la ventana y sus partes (lo que deja `npm run build:ventana`
// en crates/agente/ventana; si no está compilada, una página que lo dice).
include!(concat!(env!("OUT_DIR"), "/ventana_archivos.rs"));

/// Dónde se sirve la página (protocolo propio: sin servidor web ni puertos).
pub const ORIGEN: &str = "http://resguardo.localhost/";

/// Un archivo de la página y su tipo.
pub fn archivo(ruta: &str) -> Option<(&'static [u8], &'static str)> {
    let nombre = ruta.trim_start_matches('/');
    let nombre = if nombre.is_empty() { "index.html" } else { nombre };
    let (_, datos) = ARCHIVOS.iter().find(|(n, _)| *n == nombre)?;
    let tipo = match nombre.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    };
    Some((datos, tipo))
}

/// Política de contenido de la página: solo lo suyo, sin red.
pub const CSP: &str = "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

/// Título de la ventana (también para encontrarla si ya está abierta).
pub const TITULO: &str = "Resguardo Agente";

/// Lo que dura desbloqueada sin usarla.
pub const DESBLOQUEO_S: u64 = 600;

/// Lo que la página puede pedir al servicio con la clave.
pub const OPS_CON_CLAVE: [&str; 18] = [
    "ajustes",
    "estado_local",
    "crear_repositorio",
    "config",
    "carpetas",
    "explorar",
    "restaurar",
    "retencion",
    "copia_externa",
    "pausar",
    "reanudar",
    "guarda_copias",
    "conectar_nube",
    "quitar_nube",
    "historial",
    "kit",
    "vincular",
    "comprobar",
];

/// La ventana, en sistemas sin ella.
#[cfg(not(windows))]
pub fn run() -> Result<(), String> {
    Err("La ventana solo existe en Windows por ahora.".into())
}

#[cfg(windows)]
pub use win::{run, Avisador};

#[cfg(windows)]
mod win {
    use super::*;
    use serde_json::{json, Value};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    fn ancho(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Carpeta del usuario para lo que la ventana necesita guardar (los datos de
    /// WebView2 y el icono de los avisos). En pruebas, dentro de la de pruebas.
    pub fn carpeta_usuario() -> std::path::PathBuf {
        if crate::agent::test_mode() {
            return crate::agent::agent_dir().join("usuario");
        }
        let base = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
        base.join("Resguardo").join("Agente")
    }

    // ---------- Avisos nativos ----------

    /// Identidad de los avisos (AppUserModelID). En pruebas, otra.
    fn aumid() -> &'static str {
        if crate::agent::test_mode() {
            "Resguardo.Agente.Pruebas"
        } else {
            "Resguardo.Agente"
        }
    }

    /// Registra para el usuario el nombre y el icono de los avisos
    /// (`HKCU\Software\Classes\AppUserModelId\<id>`), sin instalador.
    fn registrar_identidad() -> Result<(), String> {
        use windows_sys::Win32::System::Registry::{
            RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
        };
        let dir = carpeta_usuario();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let icono = dir.join("icono-avisos.png");
        let lado = 96;
        let rgba = crate::bandeja::pintar(crate::bandeja::Variante { nivel: crate::bandeja::Nivel::AlDia, octavos: None }, lado);
        std::fs::write(&icono, crate::escritorio::png(&rgba, lado)).map_err(|e| e.to_string())?;
        let clave = ancho(&format!(r"Software\Classes\AppUserModelId\{}", aumid()));
        let mut h: HKEY = std::ptr::null_mut();
        // SAFETY: cadenas terminadas en cero; `h` se cierra al final.
        unsafe {
            if RegCreateKeyExW(
                HKEY_CURRENT_USER,
                clave.as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                std::ptr::null(),
                &mut h,
                std::ptr::null_mut(),
            ) != 0
            {
                return Err("no se pudo registrar la identidad de los avisos".into());
            }
            for (nombre, valor) in [("DisplayName", "Resguardo".to_string()), ("IconUri", icono.to_string_lossy().into_owned())] {
                let (n, v) = (ancho(nombre), ancho(&valor));
                RegSetValueExW(h, n.as_ptr(), 0, REG_SZ, v.as_ptr() as *const u8, (v.len() * 2) as u32);
            }
            RegCloseKey(h);
        }
        Ok(())
    }

    fn xml(s: &str) -> String {
        s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
    }

    fn mostrar(a: &crate::escritorio::AvisoEscritorio, al_pulsar: std::sync::mpsc::Sender<()>) -> windows::core::Result<()> {
        use windows::core::HSTRING;
        use windows::Data::Xml::Dom::XmlDocument;
        use windows::Foundation::TypedEventHandler;
        use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
        let texto = format!(
            r#"<toast launch="abrir"><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual>{}</toast>"#,
            xml(&a.titulo),
            xml(&a.texto),
            if a.error { "" } else { r#"<audio silent="true"/>"# }
        );
        let doc = XmlDocument::new()?;
        doc.LoadXml(&HSTRING::from(texto))?;
        let t = ToastNotification::CreateToastNotification(&doc)?;
        t.SetGroup(&HSTRING::from(a.grupo))?;
        // Una por tarea: «terminó» reemplaza a «empezó» (como mucho 64 caracteres).
        t.SetTag(&HSTRING::from(a.etiqueta.chars().take(64).collect::<String>()))?;
        t.Activated(&TypedEventHandler::new(move |_, _| {
            let _ = al_pulsar.send(());
            Ok(())
        }))?;
        let avisador = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(aumid()))?;
        // Avisos apagados por el usuario (para Resguardo o para todo Windows): se respeta,
        // sin globo de reserva (Windows también lo callaría).
        if avisador.Setting()? != windows::UI::Notifications::NotificationSetting::Enabled {
            return Ok(());
        }
        avisador.Show(&t)
    }

    /// Los avisos se enseñan en su propio hilo (WinRT, sin bloquear la bandeja).
    /// Si Windows no los admite, vuelven por `globos` para enseñarlos como
    /// globo del icono, como antes.
    pub struct Avisador {
        tx: std::sync::mpsc::Sender<crate::escritorio::AvisoEscritorio>,
        pub globos: std::sync::mpsc::Receiver<crate::escritorio::AvisoEscritorio>,
        pub pulsados: std::sync::mpsc::Receiver<()>,
    }

    impl Avisador {
        pub fn nuevo() -> Avisador {
            let (tx, rx) = std::sync::mpsc::channel::<crate::escritorio::AvisoEscritorio>();
            let (tx_globo, globos) = std::sync::mpsc::channel();
            let (tx_pulsado, pulsados) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                // SAFETY: inicializa COM en este hilo (multihilo), como pide WinRT.
                unsafe {
                    windows_sys::Win32::System::Com::CoInitializeEx(std::ptr::null(), windows_sys::Win32::System::Com::COINIT_MULTITHREADED as u32);
                }
                // La identidad de los avisos se registra con el primero (sin avisos, nada).
                let mut nativos: Option<bool> = None;
                for a in rx {
                    let mut estado = 5;
                    // SAFETY: un entero de salida.
                    unsafe {
                        windows_sys::Win32::UI::Shell::SHQueryUserNotificationState(&mut estado);
                    }
                    if !crate::escritorio::pasa_aviso(estado, a.error) {
                        continue;
                    }
                    let nativos = *nativos.get_or_insert_with(|| registrar_identidad().is_ok());
                    if !nativos || mostrar(&a, tx_pulsado.clone()).is_err() {
                        let _ = tx_globo.send(a);
                    }
                }
            });
            Avisador { tx, globos, pulsados }
        }

        pub fn avisar(&self, a: crate::escritorio::AvisoEscritorio) {
            let _ = self.tx.send(a);
        }
    }

    // ---------- La ventana ----------

    /// Ya abierta: se trae al frente y este proceso no abre otra.
    fn ya_abierta() -> bool {
        use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
        use windows_sys::Win32::System::Threading::CreateMutexW;
        use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE};
        let nombre = ancho(if crate::agent::test_mode() { "Local\\ResguardoAgenteVentanaPruebas" } else { "Local\\ResguardoAgenteVentana" });
        // El mutex vive mientras viva el proceso (no se cierra a propósito).
        // SAFETY: nombre terminado en cero.
        let existe = unsafe {
            let h = CreateMutexW(std::ptr::null(), 0, nombre.as_ptr());
            h.is_null() || GetLastError() == ERROR_ALREADY_EXISTS
        };
        if existe {
            let t = ancho(TITULO);
            // SAFETY: título terminado en cero; si no la encuentra, no hace nada.
            unsafe {
                let w = FindWindowW(std::ptr::null(), t.as_ptr());
                if !w.is_null() {
                    ShowWindow(w, SW_RESTORE);
                    SetForegroundWindow(w);
                }
            }
        }
        existe
    }

    fn leer(nombre: &str) -> Value {
        std::fs::read(crate::agent::agent_dir().join(nombre)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null)
    }

    enum Evento {
        Script(String),
        Pedido(String),
    }

    /// La prueba guardada mientras la ventana está desbloqueada.
    #[derive(Default)]
    struct Desbloqueo {
        prueba: Option<(String, Instant)>,
    }

    impl Desbloqueo {
        fn prueba(&mut self) -> Option<String> {
            match &self.prueba {
                Some((p, t)) if t.elapsed() < Duration::from_secs(DESBLOQUEO_S) => {
                    let p = p.clone();
                    self.prueba = Some((p.clone(), Instant::now()));
                    Some(p)
                }
                _ => {
                    self.prueba = None;
                    None
                }
            }
        }
    }

    /// Lo que pide la página (en otro hilo: Argon2id y restic tardan).
    fn atender(p: &Value, desbloqueo: &Mutex<Desbloqueo>) -> Result<Value, String> {
        use crate::ipc_local as ipc;
        match p["op"].as_str().unwrap_or("") {
            "hola" => ipc::pedir(&json!({ "op": "hola" })),
            "copiar" => {
                for clave in p["claves"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                    let (repo, plan) = clave.split_once('#').ok_or("Copia desconocida.")?;
                    crate::agent::request_backup(repo, plan)?;
                }
                Ok(json!({ "mensaje": "Copia pedida: empieza en unos segundos." }))
            }
            "abrir_consola" => {
                // Solo la dirección que dice el servicio (https://), nunca una de la página.
                let url = leer(crate::bandeja::ARCHIVO)["consola"].as_str().filter(|u| u.starts_with("https://")).map(str::to_string).ok_or("Sin consola.")?;
                let (verbo, u) = (ancho("open"), ancho(&url));
                // SAFETY: cadenas terminadas en cero que viven durante la llamada.
                unsafe {
                    windows_sys::Win32::UI::Shell::ShellExecuteW(
                        std::ptr::null_mut(),
                        verbo.as_ptr(),
                        u.as_ptr(),
                        std::ptr::null(),
                        std::ptr::null(),
                        windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
                    );
                }
                Ok(Value::Null)
            }
            "desbloquear" => {
                let hola = ipc::pedir(&json!({ "op": "hola" }))?;
                let sal = hola["sal_equipo"].as_str().ok_or("Este equipo aún no tiene clave de administración.")?;
                let prueba = ipc::prueba(p["clave"].as_str().unwrap_or(""), sal)?;
                let r = ipc::pedir(&json!({ "op": "comprobar", "reto": hola["reto"], "prueba": prueba }))?;
                desbloqueo.lock().map_err(|_| "Ocupado.")?.prueba = Some((prueba, Instant::now()));
                Ok(r)
            }
            // ¿Sigue desbloqueada? (Al volver a «Ajustes» no se pide otra vez la clave.)
            "desbloqueada" => Ok(json!({ "abierta": desbloqueo.lock().map_err(|_| "Ocupado.")?.prueba().is_some() })),
            "bloquear" => {
                desbloqueo.lock().map_err(|_| "Ocupado.")?.prueba = None;
                Ok(Value::Null)
            }
            "crear_clave" => {
                let clave = p["clave"].as_str().unwrap_or("");
                let mut datos = ipc::datos_clave_nueva(clave)?;
                let hola = ipc::pedir(&json!({ "op": "hola" }))?;
                let sal = datos["sal_equipo"].as_str().unwrap_or_default().to_string();
                datos["op"] = json!("crear_clave");
                datos["reto"] = hola["reto"].clone();
                let r = ipc::pedir(&datos)?;
                // Ya desbloqueada con la clave que se acaba de poner.
                desbloqueo.lock().map_err(|_| "Ocupado.")?.prueba = Some((ipc::prueba(clave, &sal)?, Instant::now()));
                Ok(r)
            }
            "cambiar_clave" => {
                // La clave nueva se deriva aquí (como la consola): al servicio solo van el verificador y K_cfg.
                let prueba = desbloqueo.lock().map_err(|_| "Ocupado.")?.prueba().ok_or("bloqueada")?;
                let clave = p["clave"].as_str().unwrap_or("");
                let datos = ipc::datos_clave_nueva(clave)?;
                let sal = datos["sal_equipo"].as_str().unwrap_or_default().to_string();
                let r = ipc::pedir_con_prueba("cambiar_clave", &prueba, datos)?;
                desbloqueo.lock().map_err(|_| "Ocupado.")?.prueba = Some((ipc::prueba(clave, &sal)?, Instant::now()));
                Ok(r)
            }
            "nube_autorizar" => {
                // `rclone authorize` como el usuario (abre su navegador); el token va al servicio con la clave.
                let tipo = p["tipo"].as_str().unwrap_or("");
                if !matches!(tipo, "dropbox" | "drive") {
                    return Err("Tipo de nube no admitido.".into());
                }
                let prueba = desbloqueo.lock().map_err(|_| "Ocupado.")?.prueba().ok_or("bloqueada")?;
                let conf = carpeta_usuario().join("rclone-vacio.conf");
                let _ = std::fs::create_dir_all(carpeta_usuario());
                let salida = std::process::Command::new(crate::nube::comprobar_binario()?)
                    .args(["authorize", tipo, "--config"])
                    .arg(&conf)
                    .stdin(std::process::Stdio::null())
                    .output()
                    .map_err(|e| format!("No se pudo ejecutar rclone: {e}"))?;
                let _ = std::fs::remove_file(&conf);
                if !salida.status.success() {
                    return Err("No se terminó de dar permiso (¿se cerró el navegador?).".into());
                }
                let token = crate::nube::token_de_salida(&String::from_utf8_lossy(&salida.stdout)).ok_or("No llegó el permiso de la nube.")?;
                ipc::pedir_con_prueba("conectar_nube", &prueba, json!({ "tipo": tipo, "nombre": p["nombre"], "token": token }))
            }
            "servicio" => {
                let op = p["que"].as_str().unwrap_or("");
                if !OPS_CON_CLAVE.contains(&op) {
                    return Err("Operación no válida.".into());
                }
                let prueba = desbloqueo.lock().map_err(|_| "Ocupado.")?.prueba().ok_or("bloqueada")?;
                let r = ipc::pedir_con_prueba(op, &prueba, p["cuerpo"].clone());
                if r.as_ref().is_err_and(|e| e == ipc::CLAVE_INCORRECTA) {
                    // La clave cambió (desde la consola): hay que volver a escribirla.
                    desbloqueo.lock().map_err(|_| "Ocupado.")?.prueba = None;
                }
                r
            }
            otra => Err(format!("Operación desconocida: «{otra}».")),
        }
    }

    pub fn run() -> Result<(), String> {
        use tao::dpi::LogicalSize;
        use tao::event::{Event, WindowEvent};
        use tao::event_loop::{ControlFlow, EventLoopBuilder};
        use tao::window::{Icon, WindowBuilder};
        if crate::platform::is_elevated() && !crate::agent::test_mode() {
            // Un navegador como administrador en la sesión del usuario: no.
            return Err("La ventana no se abre como administrador.".into());
        }
        if ya_abierta() {
            return Ok(());
        }
        let event_loop = EventLoopBuilder::<Evento>::with_user_event().build();
        let lado = 64;
        let icono =
            Icon::from_rgba(crate::bandeja::pintar(crate::bandeja::Variante { nivel: crate::bandeja::Nivel::AlDia, octavos: None }, lado), lado, lado).ok();
        let window = WindowBuilder::new()
            .with_title(TITULO)
            .with_inner_size(LogicalSize::new(460.0, 720.0))
            .with_min_inner_size(LogicalSize::new(380.0, 520.0))
            .with_window_icon(icono)
            .build(&event_loop)
            .map_err(|e| format!("No se pudo abrir la ventana: {e}"))?;
        let datos = carpeta_usuario().join("WebView2");
        let _ = std::fs::create_dir_all(&datos);
        let mut contexto = wry::WebContext::new(Some(datos));
        let proxy = event_loop.create_proxy();
        let p_ipc = proxy.clone();
        // Solo en desarrollo: forzar el tema para las capturas (RESGUARDO_VENTANA_TEMA=claro|oscuro).
        let tema = if cfg!(debug_assertions) { std::env::var("RESGUARDO_VENTANA_TEMA").ok() } else { None };
        let inicio = match tema.as_deref() {
            Some("oscuro") => "window.__resguardoTema='dark';",
            Some("claro") => "window.__resguardoTema='light';",
            _ => "",
        };
        let webview = wry::WebViewBuilder::new_with_web_context(&mut contexto)
            .with_custom_protocol("resguardo".into(), |_, req| {
                use wry::http::Response;
                match archivo(req.uri().path()) {
                    Some((datos, tipo)) => Response::builder()
                        .header("Content-Type", tipo)
                        .header("Content-Security-Policy", CSP)
                        .header("X-Content-Type-Options", "nosniff")
                        .body(std::borrow::Cow::Borrowed(datos))
                        .unwrap_or_else(|_| Response::new(std::borrow::Cow::Borrowed(&[][..]))),
                    None => Response::builder().status(404).body(std::borrow::Cow::Borrowed(&[][..])).unwrap_or_else(|_| Response::new(std::borrow::Cow::Borrowed(&[][..]))),
                }
            })
            .with_url(ORIGEN)
            .with_initialization_script(format!("window.__resguardoVersion={:?};{inicio}", crate::version_programa()))
            .with_background_color((15, 15, 17, 255))
            .with_devtools(cfg!(debug_assertions))
            // La página no navega a ninguna parte ni abre ventanas: la consola, por «abrir_consola».
            .with_navigation_handler(|url| url.starts_with(ORIGEN) || url == "about:blank")
            .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
            .with_ipc_handler(move |req| {
                let _ = p_ipc.send_event(Evento::Pedido(req.body().clone()));
            })
            .build(&window)
            .map_err(|e| {
                format!("No se pudo abrir la ventana (¿falta WebView2?): {e}. La bandeja sigue funcionando; para la ventana, instala «Microsoft Edge WebView2 Runtime».")
            })?;
        // Los datos, cada segundo y solo si cambian.
        let p_datos = proxy.clone();
        let viva = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let viva_hilo = viva.clone();
        std::thread::spawn(move || {
            let mut ultimo = String::new();
            while viva_hilo.load(std::sync::atomic::Ordering::Relaxed) {
                let d = json!({
                    "bandeja": leer(crate::bandeja::ARCHIVO),
                    "ventana": leer(crate::escritorio::ARCHIVO_VENTANA),
                    "ahora": chrono::Local::now().to_rfc3339(),
                });
                let mut sin_hora = d.clone();
                sin_hora["ahora"] = Value::Null;
                let huella = sin_hora.to_string();
                if huella != ultimo {
                    ultimo = huella;
                    if p_datos.send_event(Evento::Script(format!("window.__resguardo&&window.__resguardo.datos({d})"))).is_err() {
                        break;
                    }
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        });
        let desbloqueo = Arc::new(Mutex::new(Desbloqueo::default()));
        event_loop.run(move |evento, _, control_flow| {
            // Sin pisar la salida: tras cerrar pueden llegar aún datos en la misma vuelta.
            if *control_flow != ControlFlow::Exit {
                *control_flow = ControlFlow::Wait;
            }
            match evento {
                Event::UserEvent(Evento::Script(js)) => {
                    let _ = webview.evaluate_script(&js);
                }
                Event::UserEvent(Evento::Pedido(cuerpo)) => {
                    let (proxy, desbloqueo) = (proxy.clone(), desbloqueo.clone());
                    std::thread::spawn(move || {
                        let p: Value = serde_json::from_str(&cuerpo).unwrap_or(Value::Null);
                        let r = match atender(&p, &desbloqueo) {
                            Ok(d) => json!({ "ok": true, "datos": d }),
                            Err(e) => json!({ "ok": false, "error": e }),
                        };
                        let _ = proxy.send_event(Evento::Script(format!("window.__resguardo&&window.__resguardo.respuesta({},{r})", p["id"])));
                    });
                }
                Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                    viva.store(false, std::sync::atomic::Ordering::Relaxed);
                    *control_flow = ControlFlow::Exit;
                }
                _ => {}
            }
        });
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn la_pagina_esta_dentro() {
        let (html, tipo) = super::archivo("/").expect("index.html");
        assert!(tipo.starts_with("text/html"));
        assert!(std::str::from_utf8(html).unwrap().contains("Resguardo"));
        assert!(super::archivo("/../Cargo.toml").is_none());
        assert!(super::archivo("/no-existe.js").is_none());
    }
}
