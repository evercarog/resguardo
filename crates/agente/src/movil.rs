//! 0.8.1 (15ii): copias para equipos que entran y salen (portátiles, equipos de campo).
//!
//! - **«Además, al volver a conectarse»** (`config.copias[].al_reconectar` → el plan, con
//!   `on_reconnect`): en cada vuelta se mira si el destino responde. Cuando vuelve a responder
//!   tras no hacerlo y la última copia buena es más vieja que lo que separa dos veces de su
//!   horario, se copia **una vez** tras unos minutos de calma. Se suma al horario: con el
//!   equipo siempre conectado no hace nada.
//! - Los **reintentos** de una copia cortada por la red no se gastan mientras el destino no
//!   responde: esperan a que vuelva (antes, 2 intentos en 30 minutos y se rendía).
//! - **No empezar** con la batería por debajo de un % ni con una conexión medida (datos del
//!   móvil) salvo que se permita (`config.movil`). Solo lo que se hace solo: «Copiar ahora»
//!   se hace igual.
//! - **«Sale de la oficina»** con su margen (`config.movil.sale_oficina`, `margen_dias`): una
//!   copia que falla por no llegar al destino no cuenta como fallida mientras no pase el
//!   margen («Fuera de la oficina: sin conexión desde …»); pasado, sí, con «Lleva N días sin
//!   conectarse».

use chrono::{DateTime, Duration, Local};
use serde::{Deserialize, Serialize};
use std::net::{TcpStream, ToSocketAddrs};

/// Los minutos de calma tras volver a conectarse antes de copiar.
pub const CALMA_MIN: i64 = 5;
/// Como mucho, días de margen de un equipo que sale de la oficina.
pub const MAX_MARGEN_DIAS: u32 = 90;
/// El margen si no se dice.
pub const MARGEN_DEFECTO: u32 = 7;

/// `config.movil` (con la clave de administración, como el resto de la configuración).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Movil {
    /// «Sale de la oficina»: no cuenta como fallida una copia sin conexión hasta pasar el margen.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub sale_oficina: bool,
    /// Días sin copia antes de avisar (1 a 90; sin él, 7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margen_dias: Option<u32>,
    /// No empezar una copia sola con la batería por debajo de este % (0 o sin él: no se mira).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bateria_min: Option<u8>,
    /// Permitir empezar con una conexión medida (datos del móvil). Sin él, no.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub permitir_medida: bool,
}

impl Movil {
    pub fn valida(&self) -> Result<(), String> {
        if self.margen_dias.is_some_and(|d| !(1..=MAX_MARGEN_DIAS).contains(&d)) {
            return Err(format!("El margen de un equipo que sale de la oficina va de 1 a {MAX_MARGEN_DIAS} días."));
        }
        if self.bateria_min.is_some_and(|b| b > 95) {
            return Err("La batería mínima va de 0 % a 95 %.".into());
        }
        Ok(())
    }
    pub fn margen(&self) -> u32 {
        self.margen_dias.unwrap_or(MARGEN_DEFECTO).clamp(1, MAX_MARGEN_DIAS)
    }
    /// Lo que se guarda en el agente para decidir si empieza (agent.json).
    pub fn energia(&self) -> Option<Energia> {
        let e = Energia { bateria_min: self.bateria_min.filter(|b| *b > 0), sin_medida: !self.permitir_medida };
        (e.bateria_min.is_some() || e.sin_medida).then_some(e)
    }
}

/// Lo que mira el agente antes de empezar una copia sola (en agent.json).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Energia {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bateria_min: Option<u8>,
    /// No empezar con una conexión medida.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub sin_medida: bool,
}

/// La batería: (%, enchufado). `None`: sin batería o no se sabe.
pub fn bateria() -> Option<(u8, bool)> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
        // SAFETY: GetSystemPowerStatus solo escribe en la estructura que se le pasa.
        let mut s: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
        if unsafe { GetSystemPowerStatus(&mut s) } == 0 {
            return None;
        }
        // 128: sin batería; 255: desconocido.
        if s.BatteryFlag & 128 != 0 || s.BatteryFlag == 255 || s.BatteryLifePercent > 100 {
            return None;
        }
        Some((s.BatteryLifePercent, s.ACLineStatus == 1))
    }
    #[cfg(target_os = "linux")]
    {
        bateria_linux(std::path::Path::new("/sys/class/power_supply"))
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        None
    }
}

/// En Linux: la primera batería de `/sys/class/power_supply` y si hay corriente.
#[cfg(any(target_os = "linux", test))]
pub fn bateria_linux(base: &std::path::Path) -> Option<(u8, bool)> {
    let leer = |p: std::path::PathBuf| std::fs::read_to_string(p).ok().map(|s| s.trim().to_string());
    let mut pct = None;
    let mut enchufado = false;
    for e in std::fs::read_dir(base).ok()?.flatten() {
        let d = e.path();
        match leer(d.join("type")).as_deref() {
            Some("Battery") if pct.is_none() => {
                pct = leer(d.join("capacity")).and_then(|c| c.parse::<u8>().ok()).filter(|c| *c <= 100);
                if matches!(leer(d.join("status")).as_deref(), Some("Charging" | "Full")) {
                    enchufado = true;
                }
            }
            Some("Mains") if leer(d.join("online")).as_deref() == Some("1") => enchufado = true,
            _ => {}
        }
    }
    pct.map(|p| (p, enchufado))
}

/// ¿Es medida la conexión a internet? `None`: no se sabe.
pub fn medida() -> Option<bool> {
    #[cfg(windows)]
    {
        use windows::Networking::Connectivity::{NetworkCostType, NetworkInformation};
        let p = NetworkInformation::GetInternetConnectionProfile().ok()?;
        let c = p.GetConnectionCost().ok()?;
        let t = c.NetworkCostType().ok()?;
        Some(t == NetworkCostType::Fixed || t == NetworkCostType::Variable || c.Roaming().unwrap_or(false))
    }
    #[cfg(target_os = "linux")]
    {
        // NetworkManager (si lo hay): 1 = sí, 3 = probablemente sí.
        let out = std::process::Command::new("busctl")
            .args(["get-property", "org.freedesktop.NetworkManager", "/org/freedesktop/NetworkManager", "org.freedesktop.NetworkManager", "Metered"])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()?;
        let s = String::from_utf8_lossy(&out.stdout);
        let n: u32 = s.split_whitespace().nth(1)?.parse().ok()?;
        Some(n == 1 || n == 3)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        None
    }
}

/// Por qué no empieza ahora una copia sola (o `None` si puede).
pub fn por_que_no(e: &Energia, bat: Option<(u8, bool)>, med: Option<bool>) -> Option<String> {
    if let (Some(min), Some((pct, false))) = (e.bateria_min, bat) {
        if pct < min {
            return Some(format!("la batería está al {pct} % (por debajo del {min} %) y el equipo no está enchufado"));
        }
    }
    if e.sin_medida && med == Some(true) {
        return Some("la conexión es medida (datos del móvil)".into());
    }
    None
}

/// ¿Responde el destino de un repositorio? Solo lo que se puede mirar sin credenciales:
/// un servidor por su puerto (3 s) o una carpeta. `None`: no se sabe (una nube, S3…).
pub fn alcanzable(ubicacion: &str) -> Option<bool> {
    let u = ubicacion.trim();
    let resto = u.strip_prefix("rest:").unwrap_or(u);
    if let Some(sin) = resto.strip_prefix("https://").or_else(|| resto.strip_prefix("http://")) {
        let por_defecto = if resto.starts_with("https://") { 443 } else { 80 };
        let anfitrion = sin.split('/').next()?.rsplit('@').next()?;
        let con_puerto =
            if anfitrion.starts_with('[') || anfitrion.matches(':').count() == 1 { anfitrion.to_string() } else { format!("{anfitrion}:{por_defecto}") };
        return Some(conecta(&con_puerto));
    }
    if let Some(s) = u.strip_prefix("sftp:") {
        let host = s.split(':').next()?.rsplit('@').next()?;
        return Some(conecta(&format!("{host}:22")));
    }
    let b = u.as_bytes();
    let local = u.starts_with('/') || u.starts_with("\\\\") || (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':');
    local.then(|| std::path::Path::new(u).exists())
}

fn conecta(dir: &str) -> bool {
    let Ok(addrs) = dir.to_socket_addrs() else { return false };
    addrs.take(3).any(|a| TcpStream::connect_timeout(&a, std::time::Duration::from_secs(3)).is_ok())
}

/// ¿Falló por no llegar al destino? (sin conexión, o se cortó al guardar).
pub fn es_sin_conexion(mensaje: &str) -> bool {
    let m = mensaje.to_lowercase();
    resguardo_motor::errores::es_caso(&m, "sin_conexion") || resguardo_motor::errores::es_caso(&m, "conexion_al_guardar")
}

/// Lo que se recuerda de cada plan con «al volver a conectarse» (en state.json).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Reconexion {
    /// Si respondía el destino la última vez que se miró.
    #[serde(default)]
    pub respondia: Option<bool>,
    /// Cuándo toca la copia de «volvió a conectarse» (RFC 3339), si toca.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toca: Option<String>,
}

/// Con lo que se ve ahora (`ahora_responde`), cómo queda la reconexión de un plan:
/// si el destino vuelve a responder y la última copia buena (`ultima_buena`) es más vieja
/// que `hueco_h` horas (lo que separa dos veces de su horario), toca copiar tras la calma.
pub fn mirar(r: &Reconexion, ahora_responde: Option<bool>, ultima_buena: Option<DateTime<Local>>, hueco_h: i64, ahora: DateTime<Local>) -> Reconexion {
    let Some(responde) = ahora_responde else { return r.clone() };
    let mut n = Reconexion { respondia: Some(responde), toca: r.toca.clone() };
    if !responde {
        // Se fue otra vez antes de la calma: se vuelve a esperar a que vuelva.
        n.toca = None;
        return n;
    }
    let vuelve = r.respondia == Some(false);
    let vieja = ultima_buena.is_none_or(|t| ahora.signed_duration_since(t) > Duration::hours(hueco_h.max(1)));
    if vuelve && vieja && n.toca.is_none() {
        n.toca = Some((ahora + Duration::minutes(CALMA_MIN)).to_rfc3339());
    }
    n
}

/// ¿Toca ya la copia de «volvió a conectarse»?
pub fn toca(r: &Reconexion, ahora: DateTime<Local>) -> bool {
    r.respondia == Some(true) && r.toca.as_deref().and_then(|t| DateTime::parse_from_rfc3339(t).ok()).is_some_and(|t| ahora >= t.with_timezone(&Local))
}

/// 15ii: cómo se cuenta una copia que falló, en un equipo que sale de la oficina: `None`
/// si no es el caso (otro fallo, o no sale de la oficina); si no, (estado, mensaje):
/// «aviso» dentro del margen y «fallo» pasado.
pub fn estado_fuera(m: &Movil, mensaje: &str, ultima_buena: Option<DateTime<Local>>, ahora: DateTime<Local>) -> Option<(&'static str, String)> {
    if !m.sale_oficina || !es_sin_conexion(mensaje) {
        return None;
    }
    let dias = ultima_buena.map(|t| ahora.signed_duration_since(t).num_days().max(0));
    let margen = i64::from(m.margen());
    Some(match dias {
        Some(d) if d <= margen => (
            "aviso",
            format!(
                "Fuera de la oficina: sin conexión con el destino. Última copia buena hace {}; se copia al volver (se avisa pasados {margen} días).",
                dias_texto(d)
            ),
        ),
        Some(d) => ("fallo", format!("Lleva {} sin conectarse con el destino (sale de la oficina; se avisa pasados {margen} días).", dias_texto(d))),
        None => ("fallo", "Todavía sin ninguna copia buena: el destino no responde (sale de la oficina).".into()),
    })
}

fn dias_texto(d: i64) -> String {
    match d {
        0 => "menos de un día".into(),
        1 => "1 día".into(),
        d => format!("{d} días"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn t(h: i64) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 20, 9, 0, 0).unwrap() + Duration::hours(h)
    }

    #[test]
    fn volver_a_conectarse() {
        let r0 = Reconexion::default();
        // Primera vez que se mira: no hay vuelta.
        let r1 = mirar(&r0, Some(true), Some(t(-48)), 24, t(0));
        assert_eq!(r1.toca, None);
        // Se va y vuelve: con la última buena de hace 2 días y un horario diario, toca tras la calma.
        let r2 = mirar(&r1, Some(false), Some(t(-48)), 24, t(1));
        let r3 = mirar(&r2, Some(true), Some(t(-48)), 24, t(2));
        assert!(r3.toca.is_some() && !toca(&r3, t(2)));
        assert!(toca(&r3, t(2) + Duration::minutes(CALMA_MIN)));
        // Vuelve pero la última buena es reciente: nada.
        let r4 = mirar(&r2, Some(true), Some(t(-3)), 24, t(2));
        assert_eq!(r4.toca, None);
        // Se va antes de la calma: se anula.
        assert_eq!(mirar(&r3, Some(false), None, 24, t(2)).toca, None);
        // Sin saberlo (una nube), nada cambia.
        assert_eq!(mirar(&r3, None, None, 24, t(2)), r3);
    }

    #[test]
    fn energia_y_conexion_medida() {
        let e = Movil { bateria_min: Some(30), ..Default::default() }.energia().unwrap();
        assert!(e.sin_medida, "sin permitirlo, la medida no");
        assert!(por_que_no(&e, Some((20, false)), Some(false)).unwrap().contains("20 %"));
        assert!(por_que_no(&e, Some((20, true)), Some(false)).is_none(), "enchufado sí");
        assert!(por_que_no(&e, None, None).is_none(), "sin batería ni saberlo, sí");
        assert!(por_que_no(&e, Some((80, false)), Some(true)).unwrap().contains("medida"));
        let permitir = Movil { permitir_medida: true, ..Default::default() };
        assert!(permitir.energia().is_none());
        assert!(Movil { margen_dias: Some(0), ..Default::default() }.valida().is_err());
        assert!(Movil { bateria_min: Some(99), ..Default::default() }.valida().is_err());
    }

    #[test]
    fn bateria_en_linux() {
        let d = std::env::temp_dir().join(format!("resguardo-bateria-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(d.join("BAT0")).unwrap();
        std::fs::create_dir_all(d.join("AC")).unwrap();
        std::fs::write(d.join("BAT0").join("type"), "Battery\n").unwrap();
        std::fs::write(d.join("BAT0").join("capacity"), "42\n").unwrap();
        std::fs::write(d.join("BAT0").join("status"), "Discharging\n").unwrap();
        std::fs::write(d.join("AC").join("type"), "Mains\n").unwrap();
        std::fs::write(d.join("AC").join("online"), "0\n").unwrap();
        assert_eq!(bateria_linux(&d), Some((42, false)));
        std::fs::write(d.join("AC").join("online"), "1\n").unwrap();
        assert_eq!(bateria_linux(&d), Some((42, true)));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn destinos_que_responden() {
        assert_eq!(alcanzable("s3:https://s3.ejemplo.com/b"), None);
        assert_eq!(alcanzable("rclone:nube:carpeta"), None);
        let d = std::env::temp_dir();
        assert_eq!(alcanzable(&d.display().to_string()), Some(true));
        assert_eq!(alcanzable(&d.join("no-existe-nunca-81d").display().to_string()), Some(false));
        // Un puerto local cerrado no responde (sin red de verdad).
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let p = l.local_addr().unwrap().port();
        assert_eq!(alcanzable(&format!("rest:https://127.0.0.1:{p}/pc-01/")), Some(true));
        drop(l);
    }

    #[test]
    fn sale_de_la_oficina() {
        let m = Movil { sale_oficina: true, margen_dias: Some(7), ..Default::default() };
        let red = "Fatal: unable to open config file: Stat: Get \"https://192.0.2.10:8000/x/config\": dial tcp 192.0.2.10:8000: connectex: A connection attempt failed";
        assert!(es_sin_conexion(red));
        let (e, msg) = estado_fuera(&m, red, Some(t(-24 * 3)), t(0)).unwrap();
        assert_eq!(e, "aviso");
        assert!(msg.contains("Fuera de la oficina") && msg.contains("3 días"), "{msg}");
        let (e, msg) = estado_fuera(&m, red, Some(t(-24 * 10)), t(0)).unwrap();
        assert_eq!(e, "fallo");
        assert!(msg.contains("Lleva 10 días sin conectarse"), "{msg}");
        // Otro fallo, o un equipo que no sale: como siempre.
        assert!(estado_fuera(&m, "Fatal: wrong password or no key found", Some(t(-1)), t(0)).is_none());
        assert!(estado_fuera(&Movil::default(), red, Some(t(-1)), t(0)).is_none());
    }
}
