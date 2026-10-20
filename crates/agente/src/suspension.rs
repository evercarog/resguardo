//! 15ll: que el equipo no se suspenda por inactividad mientras copia.
//!
//! Mientras haya una copia, un espejo, una copia derivada, una verificación, una prueba o una
//! restauración en marcha, el servicio pide al sistema que no se duerma **por inactividad**:
//!
//! - **Windows:** `PowerCreateRequest` + `PowerSetRequest(PowerRequestSystemRequired)`, que
//!   vale desde un servicio (`powercfg /requests` lo enseña con su motivo). No impide suspender
//!   a mano ni cerrar la tapa (eso no se debe impedir).
//! - **Linux:** un inhibidor de systemd `idle` (`systemd-inhibit --what=idle … sleep infinity`)
//!   mientras haga falta. Se pide solo `idle` y no `sleep`: un inhibidor `sleep` bloquearía
//!   también suspender a mano (pediría la contraseña de administrador), y eso no se quiere.
//!   Sin `systemd-inhibit` (otro init), no se hace nada.
//!
//! Lo suelta en cuanto no queda nada en marcha. Una copia con `permitir_suspension` (equipos
//! con batería, en su configuración) no cuenta.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// `admite`: `config.copias[].permitir_suspension` y `evitando_suspension` en el progreso.
pub const ADMITE: &str = "sin_suspension";

/// Cada cuánto se mira si hace falta.
const CADA: Duration = Duration::from_secs(15);

/// ¿Está pedido ahora mismo? (Para el progreso: «Evitando la suspensión mientras copia».)
static EVITANDO: AtomicBool = AtomicBool::new(false);

/// ¿Está el servicio evitando ahora que el equipo se suspenda?
pub fn evitando() -> bool {
    EVITANDO.load(Ordering::Relaxed)
}

/// ¿Hace falta? Algo en marcha, salvo las copias que dejan suspender el equipo.
pub fn hace_falta(actividades: &[crate::escritorio::Actividad], operaciones: usize, permiten: &[String]) -> bool {
    operaciones > 0 || actividades.iter().any(|a| !(a.tipo == "copia" && a.parar.as_ref().is_some_and(|p| permiten.contains(p))))
}

/// Las copias (su clave de «Parar», `copia:<repo>:<copia>`) que dejan suspender el equipo.
fn copias_que_permiten() -> Vec<String> {
    let Some(v) = crate::servidor_v2::cargar() else { return Vec::new() };
    let Some(copias) = v.config_v1.as_ref().and_then(|c| c["copias"].as_array().cloned()) else { return Vec::new() };
    copias.iter().filter(|k| k["permitir_suspension"] == true).filter_map(|k| Some(format!("copia:{}:{}", k["repo"].as_str()?, k["id"].as_str()?))).collect()
}

/// En el servicio: un hilo que pide o suelta «sistema necesario» según lo que haya en marcha.
pub fn hilo() {
    std::thread::spawn(|| {
        let mut pedida: Option<Peticion> = None;
        loop {
            let (config, estado, tareas) = (crate::agent::load_config(), crate::agent::load_state(), crate::tasks::load_state());
            let act = crate::escritorio::actividades_de(&config, &estado, &tareas, chrono::Local::now());
            let ops = crate::progreso_v2::ops::lista().len();
            let falta = hace_falta(&act, ops, &if act.iter().any(|a| a.tipo == "copia") { copias_que_permiten() } else { Vec::new() });
            match (falta, pedida.is_some()) {
                (true, false) => match Peticion::pedir() {
                    Ok(p) => {
                        pedida = Some(p);
                        EVITANDO.store(true, Ordering::Relaxed);
                    }
                    Err(e) => {
                        // Una vez por cada vez que haría falta (no en cada vuelta).
                        crate::agent::log(&format!("AVISO: no se pudo pedir al sistema que no se suspenda mientras copia: {e}"));
                        pedida = Some(Peticion::nula());
                    }
                },
                (false, true) => {
                    pedida = None;
                    EVITANDO.store(false, Ordering::Relaxed);
                }
                _ => {}
            }
            std::thread::sleep(CADA);
        }
    });
}

/// La petición al sistema mientras vive (se suelta al soltarla).
struct Peticion {
    #[cfg(windows)]
    h: Option<isize>,
    #[cfg(unix)]
    hijo: Option<std::process::Child>,
}

impl Peticion {
    /// Una que no pide nada (no se pudo: para no reintentarlo en cada vuelta).
    fn nula() -> Peticion {
        Peticion {
            #[cfg(windows)]
            h: None,
            #[cfg(unix)]
            hijo: None,
        }
    }

    #[cfg(windows)]
    fn pedir() -> Result<Peticion, String> {
        use windows_sys::Win32::System::Power::{PowerCreateRequest, PowerRequestSystemRequired, PowerSetRequest};
        use windows_sys::Win32::System::SystemServices::POWER_REQUEST_CONTEXT_VERSION;
        use windows_sys::Win32::System::Threading::{POWER_REQUEST_CONTEXT_SIMPLE_STRING, REASON_CONTEXT, REASON_CONTEXT_0};
        let mut motivo: Vec<u16> = "Resguardo: copia de seguridad en marcha".encode_utf16().chain(std::iter::once(0)).collect();
        let ctx = REASON_CONTEXT {
            Version: POWER_REQUEST_CONTEXT_VERSION,
            Flags: POWER_REQUEST_CONTEXT_SIMPLE_STRING,
            Reason: REASON_CONTEXT_0 { SimpleReasonString: motivo.as_mut_ptr() },
        };
        // SAFETY: estructura local con una cadena terminada en 0 que vive durante la llamada
        // (Windows la copia). El handle se cierra al soltar la petición.
        unsafe {
            let h = PowerCreateRequest(&ctx);
            if h.is_null() || h as isize == -1 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            if PowerSetRequest(h, PowerRequestSystemRequired) == 0 {
                let e = std::io::Error::last_os_error().to_string();
                windows_sys::Win32::Foundation::CloseHandle(h);
                return Err(e);
            }
            Ok(Peticion { h: Some(h as isize) })
        }
    }

    #[cfg(unix)]
    fn pedir() -> Result<Peticion, String> {
        let prog = ["/usr/bin/systemd-inhibit", "/bin/systemd-inhibit"].into_iter().find(|p| std::path::Path::new(p).is_file()).ok_or("sin systemd-inhibit")?;
        let hijo = std::process::Command::new(prog)
            .args(["--what=idle", "--who=Resguardo Agente", "--why=Copia de seguridad en marcha", "--mode=block", "sleep", "infinity"])
            .env("PATH", resguardo_motor::proceso::path_del_sistema())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(Peticion { hijo: Some(hijo) })
    }

    #[cfg(not(any(windows, unix)))]
    fn pedir() -> Result<Peticion, String> {
        Err("este sistema no lo admite".into())
    }
}

impl Drop for Peticion {
    fn drop(&mut self) {
        #[cfg(windows)]
        if let Some(h) = self.h.take() {
            use windows_sys::Win32::System::Power::{PowerClearRequest, PowerRequestSystemRequired};
            let h = h as windows_sys::Win32::Foundation::HANDLE;
            // SAFETY: el handle es el que dio `PowerCreateRequest` y solo se cierra aquí.
            unsafe {
                PowerClearRequest(h, PowerRequestSystemRequired);
                windows_sys::Win32::Foundation::CloseHandle(h);
            }
        }
        #[cfg(unix)]
        if let Some(mut h) = self.hijo.take() {
            let _ = h.kill();
            let _ = h.wait();
        }
    }
}

/// 15ll (2): las tarjetas de red de este equipo con «Permitir que el equipo apague este
/// dispositivo para ahorrar energía» (Windows; su nombre, sin nada más). Solo las físicas y
/// presentes. En Linux no se sabe sin privilegios raros: vacío.
#[cfg(windows)]
pub fn red_con_ahorro() -> Vec<String> {
    const CLASE: &str = "SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e972-e325-11ce-bfc1-08002be10318}";
    const NCF_PHYSICAL: u32 = 0x4;
    const NCF_HIDDEN: u32 = 0x8;
    // «PnPCapabilities» con 0x10: el sistema no puede apagarla (casilla quitada).
    const NO_APAGAR: u32 = 0x10;
    let presentes = adaptadores_presentes();
    let mut out = Vec::new();
    for i in 0..200 {
        let k = format!("{CLASE}\\{i:04}");
        let Some(guid) = reg_texto(&k, "NetCfgInstanceId") else { continue };
        if !presentes.contains(&guid.to_ascii_lowercase()) {
            continue;
        }
        let c = reg_numero(&k, "Characteristics").unwrap_or(0);
        if c & NCF_PHYSICAL == 0 || c & NCF_HIDDEN != 0 {
            continue;
        }
        if reg_numero(&k, "PnPCapabilities").unwrap_or(0) & NO_APAGAR == 0 {
            if let Some(n) = reg_texto(&k, "DriverDesc") {
                out.push(n);
            }
        }
        if out.len() >= 8 {
            break;
        }
    }
    out
}

#[cfg(not(windows))]
pub fn red_con_ahorro() -> Vec<String> {
    Vec::new()
}

/// Los GUID (en minúsculas, con llaves) de las tarjetas Ethernet y Wi-Fi presentes.
#[cfg(windows)]
fn adaptadores_presentes() -> Vec<String> {
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetAdaptersAddresses, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST, IP_ADAPTER_ADDRESSES_LH,
    };
    const AF_UNSPEC: u32 = 0;
    const ETHERNET: u32 = 6;
    const WIFI: u32 = 71;
    let mut tam: u32 = 32 * 1024;
    let mut buf: Vec<u64> = vec![0; (tam as usize) / 8];
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    // SAFETY: búfer propio alineado a 8 con su tamaño; si no cabe, se repite con el que pide.
    let mut r = unsafe { GetAdaptersAddresses(AF_UNSPEC, flags, std::ptr::null(), buf.as_mut_ptr().cast(), &mut tam) };
    if r == 111 {
        buf = vec![0; (tam as usize).div_ceil(8)];
        // SAFETY: como arriba, con el tamaño que pidió.
        r = unsafe { GetAdaptersAddresses(AF_UNSPEC, flags, std::ptr::null(), buf.as_mut_ptr().cast(), &mut tam) };
    }
    if r != 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut p = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
    // SAFETY: lista enlazada que escribió el sistema dentro de `buf` (vivo hasta el final).
    unsafe {
        while !p.is_null() {
            let a = &*p;
            if (a.IfType == ETHERNET || a.IfType == WIFI) && !a.AdapterName.is_null() {
                let n = std::ffi::CStr::from_ptr(a.AdapterName.cast()).to_string_lossy().to_ascii_lowercase();
                out.push(n);
            }
            p = a.Next;
        }
    }
    out
}

#[cfg(windows)]
fn reg_ancho(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn reg_texto(clave: &str, valor: &str) -> Option<String> {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    let (c, v) = (reg_ancho(clave), reg_ancho(valor));
    let mut buf = [0u16; 256];
    let mut tam = std::mem::size_of_val(&buf) as u32;
    // SAFETY: cadenas terminadas en 0 y búfer propio con su tamaño en bytes.
    let r = unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, c.as_ptr(), v.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut tam) };
    if r != 0 {
        return None;
    }
    let fin = buf.iter().position(|&x| x == 0).unwrap_or(buf.len());
    crate::datos_basicos::limpio(&String::from_utf16_lossy(&buf[..fin]), 80)
}

#[cfg(windows)]
fn reg_numero(clave: &str, valor: &str) -> Option<u32> {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD};
    let (c, v) = (reg_ancho(clave), reg_ancho(valor));
    let mut n: u32 = 0;
    let mut tam = std::mem::size_of::<u32>() as u32;
    // SAFETY: cadenas terminadas en 0 y un u32 propio con su tamaño.
    let r = unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, c.as_ptr(), v.as_ptr(), RRF_RT_REG_DWORD, std::ptr::null_mut(), (&mut n as *mut u32).cast(), &mut tam) };
    (r == 0).then_some(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::escritorio::Actividad;

    #[test]
    fn solo_si_hay_algo_que_no_deja_suspender() {
        let copia = |k: &str| Actividad { tipo: "copia".into(), parar: Some(k.into()), ..Default::default() };
        let espejo = Actividad { tipo: "espejo".into(), parar: Some("espejo:x".into()), ..Default::default() };
        assert!(!hace_falta(&[], 0, &[]));
        assert!(hace_falta(&[], 1, &[]), "una operación del servicio (traer el historial, retención)");
        assert!(hace_falta(&[copia("copia:r:k")], 0, &[]));
        // La copia de un portátil que deja suspender: no; con un espejo a la vez, sí.
        let permite = vec!["copia:r:k".to_string()];
        assert!(!hace_falta(&[copia("copia:r:k")], 0, &permite));
        assert!(hace_falta(&[copia("copia:r:k"), espejo], 0, &permite));
        assert!(hace_falta(&[copia("copia:r:otra")], 0, &permite));
    }

    #[test]
    fn la_peticion_se_pide_y_se_suelta() {
        // En Windows, de verdad (no hace falta ser administrador); en Linux, solo si hay
        // systemd-inhibit y una sesión de systemd (en un contenedor puede no haberla).
        let r = Peticion::pedir();
        #[cfg(windows)]
        if let Err(e) = &r {
            panic!("en Windows tiene que poder pedirse: {e}");
        }
        drop(r);
    }

    #[test]
    fn tarjetas_de_red_sin_romper() {
        // Solo que no falle ni devuelva nombres vacíos (lo que haya depende del equipo).
        assert!(red_con_ahorro().iter().all(|n| !n.trim().is_empty()));
    }
}
