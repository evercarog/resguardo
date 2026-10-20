//! 0.8.1 (15kk): apagar o hibernar el equipo al terminar una copia.
//!
//! - Se pide con «Copiar ahora» (para esa vez: `copiar_ahora { copia, al_terminar }`, que
//!   entonces lleva la clave de administración) o como opción de la copia
//!   (`config.copias[].al_terminar`). Solo `apagar` o `hibernar`.
//! - Solo si la copia terminó **bien o con avisos** (si falla, no: para poder mirar), y
//!   cuando ya no queda nada que hacer en esa vuelta: ni otras copias que tocan, ni pasos
//!   encadenados, ni tareas (derivadas, verificaciones…) pendientes o en marcha.
//! - Antes, **2 minutos de aviso**: Windows enseña su cuenta atrás (`shutdown /s /t 120`) y
//!   el icono de Resguardo avisa y ofrece «No apagar», que la para (`shutdown /a`, que el
//!   propio usuario puede hacer) y deja `solicitudes\no-apagar.json` para que el servicio
//!   lo apunte. Hibernar se hace en la vuelta siguiente pasados los 2 minutos.
//! - Un equipo que guarda copias de otros (almacén) solo con confirmación expresa.
//! - Queda en el historial de todas las consolas (aviso `equipo_apagado`).
//! - **Nunca** apaga en las pruebas ni con `RESGUARDO_PRUEBA_APAGADO=1` (solo lo apunta).

use chrono::{DateTime, Duration, Local};
use serde::{Deserialize, Serialize};

/// Segundos de aviso antes de apagar.
pub const AVISO_S: i64 = 120;
/// Variable que hace que nunca se apague (solo se apunta): para las pruebas a mano.
pub const PRUEBA: &str = "RESGUARDO_PRUEBA_APAGADO";
/// Lo que deja la bandeja para decir que alguien pulsó «No apagar».
pub const NO_APAGAR: &str = "no-apagar.json";

pub fn modo_valido(m: &str) -> bool {
    matches!(m, "apagar" | "hibernar")
}

/// «apagar» / «hibernar» → lo que se dice.
pub fn verbo(m: &str) -> &'static str {
    if m == "hibernar" {
        "hibernará"
    } else {
        "se apagará"
    }
}

/// Un apagado pedido para cuando termine una copia (`apagar.json`, carpeta privada).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Pedido {
    pub modo: String,
    /// `repo#plan`.
    pub clave: String,
    /// Quién lo pidió (de la consola), si se sabe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub por: Option<String>,
    /// Cuándo se pidió (RFC 3339).
    pub pedido: String,
}

/// El apagado en marcha (en state.json y en la bandeja).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct EnMarcha {
    pub modo: String,
    /// Cuándo se apaga (RFC 3339).
    pub hasta: String,
    /// La copia que lo pidió.
    pub nombre: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub por: Option<String>,
    /// Cuándo empezó la cuenta atrás (las cancelaciones de antes no cuentan).
    #[serde(default)]
    pub desde: String,
}

fn archivo() -> std::path::PathBuf {
    crate::agent::private_dir().join("apagar.json")
}

/// Los pedidos de «Copiar ahora» (que la vuelta del agente recoge).
pub fn pedidos() -> Vec<Pedido> {
    std::fs::read_to_string(archivo()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// Apunta un pedido (sustituye al de esa misma copia).
pub fn pedir(p: Pedido) -> Result<(), String> {
    let mut l = pedidos();
    l.retain(|x| x.clave != p.clave);
    l.push(p);
    l.truncate(20);
    std::fs::write(archivo(), serde_json::to_string(&l).map_err(|e| e.to_string())?).map_err(|e| format!("No se pudo apuntar el apagado: {e}"))
}

/// Quita el pedido de una copia (y lo devuelve).
pub fn tomar(clave: &str) -> Option<Pedido> {
    let mut l = pedidos();
    let i = l.iter().position(|x| x.clave == clave)?;
    let p = l.remove(i);
    let _ = std::fs::write(archivo(), serde_json::to_string(&l).unwrap_or_else(|_| "[]".into()));
    Some(p)
}

/// ¿Solo apuntarlo, sin apagar? (Pruebas.)
pub fn de_prueba() -> bool {
    cfg!(test) || crate::agent::test_mode() || std::env::var(PRUEBA).is_ok_and(|v| v == "1")
}

/// Empieza la cuenta atrás (apagar) o nada todavía (hibernar: en la vuelta siguiente).
pub fn empezar(e: &EnMarcha) -> Result<(), String> {
    if de_prueba() || e.modo != "apagar" {
        return Ok(());
    }
    let mensaje = format!("Resguardo apagará el equipo en 2 minutos: terminó la copia «{}». Para no apagarlo: icono de Resguardo → «No apagar».", e.nombre);
    #[cfg(windows)]
    {
        let exe = std::path::Path::new(&std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join("System32").join("shutdown.exe");
        let c: String = mensaje.chars().take(500).collect();
        lanzar(std::process::Command::new(exe).args(["/s", "/t", &AVISO_S.to_string(), "/d", "p:0:0", "/c", &c]))
    }
    #[cfg(unix)]
    {
        lanzar(std::process::Command::new("/sbin/shutdown").args(["-h", "+2", &mensaje]))
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = mensaje;
        Err("Este sistema no se apaga desde Resguardo.".into())
    }
}

/// Hiberna ya (pasada la cuenta atrás).
pub fn hibernar_ya() -> Result<(), String> {
    if de_prueba() {
        return Ok(());
    }
    #[cfg(windows)]
    {
        let exe = std::path::Path::new(&std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join("System32").join("shutdown.exe");
        lanzar(std::process::Command::new(exe).arg("/h"))
    }
    #[cfg(unix)]
    {
        lanzar(std::process::Command::new("systemctl").arg("hibernate"))
    }
    #[cfg(not(any(windows, unix)))]
    {
        Err("Este sistema no hiberna desde Resguardo.".into())
    }
}

/// Para la cuenta atrás de apagar (también lo hace la bandeja, como el usuario).
pub fn cancelar() {
    if de_prueba() {
        return;
    }
    #[cfg(windows)]
    {
        let exe = std::path::Path::new(&std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join("System32").join("shutdown.exe");
        let _ = lanzar(std::process::Command::new(exe).arg("/a"));
    }
    #[cfg(unix)]
    {
        let _ = lanzar(std::process::Command::new("/sbin/shutdown").arg("-c"));
    }
}

#[allow(dead_code)]
fn lanzar(c: &mut std::process::Command) -> Result<(), String> {
    let st = c
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("No se pudo apagar: {e}"))?;
    if st.success() {
        Ok(())
    } else {
        Err("El sistema no dejó apagar el equipo.".into())
    }
}

/// ¿Pulsó alguien «No apagar» después de empezar la cuenta atrás?
pub fn cancelado(e: &EnMarcha) -> bool {
    let Ok(m) = std::fs::symlink_metadata(crate::agent::requests_dir().join(NO_APAGAR)) else { return false };
    if !m.is_file() {
        return false;
    }
    let desde = DateTime::parse_from_rfc3339(&e.desde).ok();
    m.modified().ok().map(DateTime::<chrono::Utc>::from).is_some_and(|t| desde.is_none_or(|d| t >= d))
}

/// La cuenta atrás de un pedido.
pub fn en_marcha(p: &Pedido, nombre: &str, ahora: DateTime<Local>) -> EnMarcha {
    EnMarcha {
        modo: p.modo.clone(),
        hasta: (ahora + Duration::seconds(AVISO_S)).to_rfc3339(),
        nombre: nombre.to_string(),
        por: p.por.clone(),
        desde: ahora.to_rfc3339(),
    }
}

/// ¿Ya pasó la cuenta atrás?
pub fn vencida(e: &EnMarcha, ahora: DateTime<Local>) -> bool {
    DateTime::parse_from_rfc3339(&e.hasta).is_ok_and(|h| ahora >= h.with_timezone(&Local))
}

/// Lo que queda en el historial de todas las consolas.
pub fn texto_historial(e: &EnMarcha) -> String {
    let por = e.por.as_deref().map(|p| format!(", pedido por {p}")).unwrap_or_default();
    format!("El equipo {} al terminar la copia «{}»{por}. Quien lo usaba tuvo 2 minutos para pararlo con «No apagar».", verbo(&e.modo), e.nombre)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn cuenta_atras_y_textos() {
        assert!(modo_valido("apagar") && modo_valido("hibernar") && !modo_valido("reiniciar") && !modo_valido(""));
        let ahora = Local.with_ymd_and_hms(2026, 10, 20, 21, 0, 0).unwrap();
        let p = Pedido { modo: "apagar".into(), clave: "r#k".into(), por: Some("Ana".into()), pedido: ahora.to_rfc3339() };
        let e = en_marcha(&p, "Documentos", ahora);
        assert!(!vencida(&e, ahora + Duration::seconds(60)) && vencida(&e, ahora + Duration::seconds(AVISO_S)));
        assert!(texto_historial(&e).contains("se apagará al terminar la copia «Documentos», pedido por Ana"));
        // En las pruebas nunca se apaga de verdad.
        assert!(de_prueba());
        assert!(empezar(&e).is_ok() && hibernar_ya().is_ok());
    }
}
