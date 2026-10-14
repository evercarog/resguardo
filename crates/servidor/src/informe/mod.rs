//! Informe ejecutivo programado (plan 0.8.0, G1; plan-mejoras 14g): un PDF por
//! cliente y periodo (semanal o mensual) que sale por correo a los destinatarios
//! que elija el cliente, con el logo del prestador.
//!
//! - [`datos`]: lo que cuenta (del historial de los equipos, sin rutas ni secretos).
//! - [`pdf`]: el PDF, escrito aquí mismo (pdf-writer; sin navegador).
//! - Aquí: lo que se guarda (el programa de cada cliente, el prestador del servidor y
//!   la regla 3-2-1-1-0 que calculó la consola), los periodos, cuándo toca y la cola.
//!
//! Se guarda en los valores del servidor (`informe:…`), como los ajustes de las
//! notificaciones: sin cambios en el esquema. El correo sale por la cola de las
//! notificaciones (reintentos, registro de envíos), con el PDF adjunto: el PDF se
//! escribe al enviarlo, con los datos que se prepararon al ponerlo en la cola.

pub mod datos;
pub mod pdf;

use crate::almacen::{Almacen, ClienteCtx, Ts, R};
use crate::notificaciones::{self as notif, ajustes as notif_ajustes, local, Mensaje, Severidad};
use base64::Engine;
use chrono::{Datelike, Duration, NaiveDate, Timelike};
pub use datos::{EstadoParte, Informe, Periodo};
use serde::{Deserialize, Serialize};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// Destinatarios como mucho por cliente.
pub const MAX_DESTINATARIOS: usize = 20;
/// Copias como mucho en la regla que manda la consola.
pub const MAX_COPIAS_REGLA: usize = 5_000;
/// El nombre del prestador, como mucho.
pub const MAX_NOMBRE_PRESTADOR: usize = 80;

// ---------- Lo que se guarda ----------

/// Qué logo lleva el informe de un cliente.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Logo {
    /// El del prestador (el del servidor); sin él, ninguno.
    #[default]
    Prestador,
    /// El de la marca del cliente (v1.32); sin él, el del prestador.
    Cliente,
    Ninguno,
}

/// El informe programado de un cliente.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Programa {
    #[serde(default)]
    pub activo: bool,
    #[serde(default)]
    pub periodo: Periodo,
    /// El semanal sale ese día (1 = lunes … 7 = domingo), con la semana anterior entera;
    /// el mensual, el día 1 con el mes anterior. A la hora de los resúmenes del servidor.
    #[serde(default = "lunes")]
    pub dia_semana: u8,
    #[serde(default)]
    pub destinatarios: Vec<String>,
    #[serde(default)]
    pub logo: Logo,
    #[serde(default)]
    pub actualizado: Ts,
    #[serde(default)]
    pub por: String,
}

fn lunes() -> u8 {
    1
}

impl Default for Programa {
    fn default() -> Self {
        Self { activo: false, periodo: Periodo::Semanal, dia_semana: 1, destinatarios: Vec::new(), logo: Logo::Prestador, actualizado: 0, por: String::new() }
    }
}

/// El prestador (lo pone el propietario del servidor): su nombre y su logo (PNG).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Prestador {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nombre: Option<String>,
    /// El PNG en base64.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub huella: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actualizado: Option<Ts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub por: Option<String>,
}

/// Una copia en la regla que calculó la consola: el equipo, la copia y cómo está cada parte.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CopiaFoto {
    pub equipo: String,
    pub copia: String,
    pub partes: [EstadoParte; 5],
}

/// La regla 3-2-1-1-0 del cliente tal como la calculó la consola (que es quien ve todos los
/// equipos y el catálogo de destinos; `consola/src/lib/regla321.ts`). Solo ids y estados:
/// los nombres y el texto de lo que falta se ponen aquí al hacer el informe.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FotoRegla {
    pub calculada: Ts,
    #[serde(default)]
    pub por: String,
    pub copias: Vec<CopiaFoto>,
}

/// La última vez que salió (o se mandó a mano).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Ultimo {
    pub cuando: Ts,
    pub periodo: Periodo,
    pub desde: Ts,
    pub hasta: Ts,
    pub destinatarios: usize,
    #[serde(default)]
    pub a_mano: bool,
    #[serde(default)]
    pub por: Option<String>,
}

fn clave_programa(c: &str) -> String {
    format!("informe:programa:{c}")
}
fn clave_regla(c: &str) -> String {
    format!("informe:regla:{c}")
}
fn clave_enviado(c: &str) -> String {
    format!("informe:enviado:{c}")
}
fn clave_ultimo(c: &str) -> String {
    format!("informe:ultimo:{c}")
}
const CLAVE_PRESTADOR: &str = "informe:prestador";

fn leer<T: serde::de::DeserializeOwned>(db: &dyn Almacen, clave: &str) -> R<Option<T>> {
    Ok(db.valor(clave)?.filter(|v| !v.is_empty()).and_then(|v| serde_json::from_str(&v).ok()))
}

fn guardar<T: Serialize>(db: &dyn Almacen, clave: &str, v: &T) -> R<()> {
    db.poner_valor(clave, &serde_json::to_string(v).map_err(|e| e.to_string())?)
}

pub fn programa(db: &dyn Almacen, cliente: &str) -> R<Programa> {
    Ok(leer(db, &clave_programa(cliente))?.unwrap_or_default())
}

/// Guarda el programa. Al encenderlo (o cambiar de semanal a mensual), el periodo que acaba
/// de terminar cuenta como enviado: el primero sale al terminar el siguiente, no uno
/// atrasado nada más encenderlo. Cambiar solo los destinatarios no se salta ninguno.
pub fn guardar_programa(db: &dyn Almacen, cliente: &str, previo: &Programa, p: &Programa, ahora: Ts) -> R<()> {
    guardar(db, &clave_programa(cliente), p)?;
    if p.activo && (!previo.activo || previo.periodo != p.periodo) {
        let (desde, _) = periodo_anterior(p.periodo, ahora);
        db.poner_valor(&clave_enviado(cliente), &clave_periodo(p.periodo, desde))?;
    }
    Ok(())
}

pub fn prestador(db: &dyn Almacen) -> R<Prestador> {
    Ok(leer(db, CLAVE_PRESTADOR)?.unwrap_or_default())
}

pub fn guardar_prestador(db: &dyn Almacen, p: &Prestador) -> R<()> {
    guardar(db, CLAVE_PRESTADOR, p)
}

pub fn foto_regla(db: &dyn Almacen, cliente: &str) -> R<Option<FotoRegla>> {
    leer(db, &clave_regla(cliente))
}

pub fn guardar_foto_regla(db: &dyn Almacen, cliente: &str, f: &FotoRegla) -> R<()> {
    guardar(db, &clave_regla(cliente), f)
}

pub fn ultimo(db: &dyn Almacen, cliente: &str) -> R<Option<Ultimo>> {
    leer(db, &clave_ultimo(cliente))
}

// ---------- Validar ----------

/// Limpia los destinatarios: direcciones de correo válidas, sin repetir, como mucho [`MAX_DESTINATARIOS`].
pub fn valida_destinatarios(xs: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for x in xs {
        let d = x.trim().to_string();
        if d.is_empty() {
            continue;
        }
        let ok = d.len() <= 254 && !d.chars().any(|c| c.is_control() || c.is_whitespace() || "<>,;\"".contains(c)) && d.parse::<lettre::Address>().is_ok();
        if !ok {
            return Err(format!("«{}» no es una dirección de correo válida.", d.chars().take(60).collect::<String>()));
        }
        if !out.iter().any(|o| o.eq_ignore_ascii_case(&d)) {
            out.push(d);
        }
    }
    if out.len() > MAX_DESTINATARIOS {
        return Err(format!("Como mucho {MAX_DESTINATARIOS} destinatarios."));
    }
    Ok(out)
}

/// Un id de equipo o de copia tal como llega de la consola: corto y sin nada raro.
pub fn id_valido(s: &str) -> bool {
    !s.is_empty() && s.len() <= 100 && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_.:".contains(c))
}

/// El nombre del prestador: texto corto, sin caracteres de control.
pub fn valida_nombre_prestador(n: &str) -> Result<Option<String>, String> {
    let n = n.trim();
    if n.is_empty() {
        return Ok(None);
    }
    if n.chars().count() > MAX_NOMBRE_PRESTADOR || n.chars().any(char::is_control) {
        return Err(format!("El nombre, como mucho {MAX_NOMBRE_PRESTADOR} caracteres y sin saltos de línea."));
    }
    Ok(Some(n.to_string()))
}

// ---------- Periodos ----------

fn medianoche(d: NaiveDate) -> Ts {
    notif::de_local(d.and_hms_opt(0, 0, 0).unwrap_or_default())
}

/// La medianoche siguiente a `t` (hora del servidor; con el cambio de hora, igual).
pub fn manana(t: Ts) -> Ts {
    let d = local(t).date();
    medianoche(d.succ_opt().unwrap_or(d))
}

fn inicio_semana(d: NaiveDate) -> NaiveDate {
    d - Duration::days(d.weekday().num_days_from_monday() as i64)
}

fn inicio_mes(d: NaiveDate) -> NaiveDate {
    d.with_day(1).unwrap_or(d)
}

/// El último periodo entero antes de `ahora`: la semana (de lunes a domingo) o el mes anterior.
pub fn periodo_anterior(p: Periodo, ahora: Ts) -> (Ts, Ts) {
    let hoy = local(ahora).date();
    match p {
        Periodo::Semanal => {
            let fin = inicio_semana(hoy);
            (medianoche(fin - Duration::days(7)), medianoche(fin))
        }
        Periodo::Mensual => {
            let fin = inicio_mes(hoy);
            let ini = inicio_mes(fin.pred_opt().unwrap_or(fin));
            (medianoche(ini), medianoche(fin))
        }
    }
}

/// El periodo en curso: desde el lunes o el día 1 hasta el final de la semana o del mes.
pub fn periodo_actual(p: Periodo, ahora: Ts) -> (Ts, Ts) {
    let hoy = local(ahora).date();
    match p {
        Periodo::Semanal => {
            let ini = inicio_semana(hoy);
            (medianoche(ini), medianoche(ini + Duration::days(7)))
        }
        Periodo::Mensual => {
            let ini = inicio_mes(hoy);
            let sig = (ini + Duration::days(32)).with_day(1).unwrap_or(ini);
            (medianoche(ini), medianoche(sig))
        }
    }
}

/// Cómo se llama un periodo para no enviarlo dos veces: «semanal:2026-W41», «mensual:2026-09».
pub fn clave_periodo(p: Periodo, desde: Ts) -> String {
    let d = local(desde).date();
    match p {
        Periodo::Semanal => format!("semanal:{}", d.format("%G-W%V")),
        Periodo::Mensual => format!("mensual:{}", d.format("%Y-%m")),
    }
}

/// ¿Toca enviar ya el del periodo anterior? El semanal, a partir del día elegido de esta
/// semana; el mensual, desde el día 1. Los dos a partir de la hora de los resúmenes (si el
/// servidor estaba apagado, sale en cuanto vuelve, dentro de la misma semana o mes).
pub fn toca(p: &Programa, hora_resumen: &str, ahora: Ts) -> bool {
    let t = local(ahora);
    let minutos = t.hour() * 60 + t.minute();
    let hora = notif::reglas::minutos(hora_resumen).unwrap_or(8 * 60);
    let dia = match p.periodo {
        Periodo::Semanal => t.weekday().number_from_monday(),
        Periodo::Mensual => t.day(),
    };
    let elegido = match p.periodo {
        Periodo::Semanal => p.dia_semana.clamp(1, 7) as u32,
        Periodo::Mensual => 1,
    };
    dia > elegido || (dia == elegido && minutos >= hora)
}

// ---------- Enviar ----------

/// El logo que lleva el informe de un cliente (PNG ya comprobado), según su programa.
pub fn logo(db: &dyn Almacen, cliente: &str, cual: Logo) -> Option<Vec<u8>> {
    let de_b64 = |b: &str| B64.decode(b).ok().filter(|b| crate::api::marca::png_valido(b).is_ok());
    let del_prestador = || prestador(db).ok().and_then(|p| p.logo).and_then(|b| de_b64(&b));
    match cual {
        Logo::Ninguno => None,
        Logo::Prestador => del_prestador(),
        Logo::Cliente => crate::api::marca::leer(db, cliente).ok().and_then(|m| m.logo).and_then(|b| de_b64(&b)).or_else(del_prestador),
    }
}

/// Pone en la cola el informe de un periodo para los destinatarios del programa (o los que se
/// digan). Devuelve a cuántos, o por qué no se puede (en palabras).
#[allow(clippy::too_many_arguments)]
pub fn encolar(
    db: &dyn Almacen,
    cliente: &str,
    periodo: Periodo,
    desde: Ts,
    hasta: Ts,
    destinatarios: &[String],
    ahora: Ts,
    a_mano: Option<&str>,
) -> R<Result<usize, String>> {
    if destinatarios.is_empty() {
        return Ok(Err("Añade al menos un destinatario.".into()));
    }
    let ajustes = notif_ajustes::ajustes(db)?;
    let propios = notif_ajustes::ajustes_cliente(db, cliente)?;
    let Some((ambito, canal)) = notif::correo_de(&ajustes, &propios, cliente) else {
        return Ok(Err("No hay un canal de correo encendido: ponlo en Notificaciones (del servidor o de este cliente).".into()));
    };
    let Some(inf) = datos::preparar(db, cliente, periodo, desde, hasta, ahora)? else { return Ok(Err("El cliente no existe.".into())) };
    let m = Mensaje::Informe(Box::new(inf));
    for d in destinatarios {
        notif::encolar_uno(db, &ambito, Some(cliente), &canal.id, d, None, Severidad::Informativo, None, &m, ahora)?;
    }
    guardar(
        db,
        &clave_ultimo(cliente),
        &Ultimo { cuando: ahora, periodo, desde, hasta, destinatarios: destinatarios.len(), a_mano: a_mano.is_some(), por: a_mano.map(Into::into) },
    )?;
    Ok(Ok(destinatarios.len()))
}

/// Los informes programados que tocan (lo llama la tarea de las notificaciones).
pub fn si_toca(db: &dyn Almacen, ahora: Ts) -> R<()> {
    let ajustes = notif_ajustes::ajustes(db)?;
    for ctx in db.todos_los_clientes()? {
        let c = ctx.id();
        let p = programa(db, c)?;
        if !p.activo || p.destinatarios.is_empty() || !toca(&p, &ajustes.hora_resumen, ahora) {
            continue;
        }
        let (desde, hasta) = periodo_anterior(p.periodo, ahora);
        let clave = clave_periodo(p.periodo, desde);
        if db.valor(&clave_enviado(c))?.as_deref() == Some(clave.as_str()) {
            continue;
        }
        // Se anota antes: si algo falla, no se repite cada 10 s (queda en el registro de envíos).
        db.poner_valor(&clave_enviado(c), &clave)?;
        match encolar(db, c, p.periodo, desde, hasta, &p.destinatarios, ahora, None)? {
            Ok(_) => {}
            Err(motivo) => {
                let ctx = ClienteCtx::autorizado(c);
                db.crear_aviso(&ctx, None, "informe_no_enviado", &format!("No se pudo enviar el informe {}: {motivo}", p.periodo.adjetivo()))?;
            }
        }
    }
    Ok(())
}

/// El nombre del adjunto: sin el del cliente (solo letras, números y guiones).
pub fn nombre_archivo(i: &Informe) -> String {
    let d = local(i.desde).date();
    match i.periodo {
        Periodo::Semanal => format!("informe-semanal-{}.pdf", d.format("%Y-%m-%d")),
        Periodo::Mensual => format!("informe-mensual-{}.pdf", d.format("%Y-%m")),
    }
}

#[cfg(test)]
mod tests;
