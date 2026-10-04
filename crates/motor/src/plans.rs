//! Planes de copia: cada repositorio puede tener varios, cada uno con sus
//! carpetas, exclusiones, etiquetas y horario (días de la semana y horas).
//!
//! Ejemplo: «Laboral» de lunes a sábado cada hora en punto de 7:00 a 19:00, y
//! «Domingo» solo el domingo a las 23:00 con otras exclusiones y etiquetas.
//!
//! El agente gestionado (≥ 0.7.9) admite además horarios hechos de varias
//! reglas (`mode: "rules"`): «cada 10 minutos de 8:00 a 18:00 de lunes a
//! viernes» y «el día 1 de cada mes a las 23:00», por ejemplo. Toca cuando
//! toca cualquiera de ellas.

use chrono::{DateTime, Datelike, Duration, MappedLocalTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use serde::{Deserialize, Serialize};

/// Margen para pequeños ajustes del reloj (sincronización NTP).
const CLOCK_SLACK_MIN: i64 = 10;

/// Separación mínima entre el comienzo de dos copias programadas del mismo
/// plan. Si una copia empezó a las 12:58 (a mano, o tarde), la de las 13:00
/// espera a las 13:03.
pub const MIN_GAP_MIN: i64 = 5;

/// Intervalos de menos de una hora que se pueden elegir (en minutos). Por
/// encima, horas enteras (60, 120 … 1440).
pub const MINUTE_STEPS: [u32; 5] = [5, 10, 15, 20, 30];

/// Como mucho, reglas en un horario.
const MAX_RULES: usize = 20;
/// «Cada N días»: como mucho, una vez al año.
const MAX_EVERY_DAYS: u32 = 365;
/// Cuántos días se mira hacia atrás o hacia delante como mucho para buscar
/// la última o la próxima vez (un «cada 365 días» que empieza dentro de un año
/// cabe de sobra).
const MAX_HORIZON_DAYS: i64 = 800;

/// ¿Una marca de tiempo guardada (`earlier`) está en el futuro respecto a
/// `now`? Pasa si el reloj del equipo estuvo adelantado y se corrigió (o se
/// cambió a mano). Sin esta comprobación, el agente esperaría a esa fecha
/// futura para volver a copiar.
pub fn clock_went_back<Tz: TimeZone>(earlier: DateTime<Tz>, now: DateTime<Tz>) -> bool {
    earlier > now + Duration::minutes(CLOCK_SLACK_MIN)
}

/// Una regla de un horario por reglas. Los días van de 0 = lunes a 6 = domingo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScheduleRule {
    /// A estas horas, los días elegidos.
    At { days: Vec<u8>, times: Vec<String> },
    /// Cada `every_min` minutos (5, 10, 15, 20, 30 o horas enteras) de `from`
    /// a `to` (las dos incluidas), los días elegidos.
    Every { days: Vec<u8>, every_min: u32, from: String, to: String },
    /// Cada `every` días contando desde `start` (AAAA-MM-DD, la primera vez), a la hora `time`.
    EveryDays { every: u32, start: String, time: String },
    /// El día `day` de cada mes (1 a 28; -1 = el último día del mes), a la hora `time`.
    Monthly { day: i8, time: String },
}

fn one() -> u32 {
    1
}

fn parse_time(t: &str) -> Result<NaiveTime, String> {
    NaiveTime::parse_from_str(t.trim(), "%H:%M").map_err(|_| format!("Hora no válida: «{t}» (usa HH:MM, p. ej. 13:00)."))
}

fn parse_date(d: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(d.trim(), "%Y-%m-%d").map_err(|_| format!("Fecha no válida: «{d}» (usa AAAA-MM-DD)."))
}

fn validate_days(days: &[u8]) -> Result<(), String> {
    if days.is_empty() {
        return Err("Elige al menos un día.".into());
    }
    if days.iter().any(|d| *d > 6) {
        return Err("Día de la semana no válido.".into());
    }
    Ok(())
}

/// ¿Es un intervalo que se puede elegir? 5, 10, 15, 20 o 30 minutos, o de 1 a 24 horas enteras.
pub fn valid_every_min(m: u32) -> bool {
    MINUTE_STEPS.contains(&m) || (m.is_multiple_of(60) && (60..=24 * 60).contains(&m))
}

/// Horas de `from` a `to` (las dos incluidas) cada `step` minutos.
fn times_between(from: NaiveTime, to: NaiveTime, step_min: u32) -> Vec<NaiveTime> {
    let step = Duration::minutes(step_min.max(1) as i64);
    let mut t = from;
    let mut v = Vec::new();
    // 24 h cada 5 min son 288 veces: el tope solo protege de un paso absurdo.
    while t <= to && v.len() < 300 {
        v.push(t);
        let (next, wrapped) = t.overflowing_add_signed(step);
        if wrapped != 0 {
            break;
        }
        t = next;
    }
    v
}

fn is_last_day_of_month(date: NaiveDate) -> bool {
    date.succ_opt().is_none_or(|n| n.month() != date.month())
}

impl ScheduleRule {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            ScheduleRule::At { days, times } => {
                validate_days(days)?;
                if times.is_empty() {
                    return Err("Añade al menos una hora.".into());
                }
                if times.len() > 48 {
                    return Err("Demasiadas horas (máximo 48 al día).".into());
                }
                for t in times {
                    parse_time(t)?;
                }
            }
            ScheduleRule::Every { days, every_min, from, to } => {
                validate_days(days)?;
                if !valid_every_min(*every_min) {
                    return Err("El intervalo debe ser de 5, 10, 15, 20 o 30 minutos, o de 1 a 24 horas.".into());
                }
                let (from, to) = (parse_time(from)?, parse_time(to)?);
                if to < from {
                    return Err("La hora final debe ser posterior a la inicial.".into());
                }
            }
            ScheduleRule::EveryDays { every, start, time } => {
                if !(1..=MAX_EVERY_DAYS).contains(every) {
                    return Err(format!("«Cada N días» debe estar entre 1 y {MAX_EVERY_DAYS}."));
                }
                parse_date(start)?;
                parse_time(time)?;
            }
            ScheduleRule::Monthly { day, time } => {
                if !(*day == -1 || (1..=28).contains(day)) {
                    return Err("El día del mes debe estar entre 1 y 28, o ser el último.".into());
                }
                parse_time(time)?;
            }
        }
        Ok(())
    }

    /// Horas (locales) en que toca ese día.
    fn times_on(&self, date: NaiveDate) -> Vec<NaiveTime> {
        let weekday_ok = |days: &[u8]| days.contains(&(date.weekday().num_days_from_monday() as u8));
        match self {
            ScheduleRule::At { days, times } if weekday_ok(days) => times.iter().filter_map(|t| parse_time(t).ok()).collect(),
            ScheduleRule::Every { days, every_min, from, to } if weekday_ok(days) => match (parse_time(from), parse_time(to)) {
                (Ok(from), Ok(to)) => times_between(from, to, *every_min),
                _ => vec![],
            },
            ScheduleRule::EveryDays { every, start, time } => match (parse_date(start), parse_time(time)) {
                (Ok(start), Ok(time)) if date >= start && (date - start).num_days() % (*every).max(1) as i64 == 0 => vec![time],
                _ => vec![],
            },
            ScheduleRule::Monthly { day, time } => {
                let hit = if *day == -1 { is_last_day_of_month(date) } else { *day > 0 && date.day() == *day as u32 };
                match parse_time(time) {
                    Ok(t) if hit => vec![t],
                    _ => vec![],
                }
            }
            _ => vec![],
        }
    }

    /// En la hora que se repite al atrasar el reloj (cambio de horario),
    /// ¿toca en las dos? Solo los intervalos de minutos: «cada 10 minutos»
    /// sigue cada 10 minutos; «a las 2:30» se hace una sola vez.
    fn both_on_repeat(&self) -> bool {
        matches!(self, ScheduleRule::Every { every_min, .. } if *every_min < 60)
    }

    /// Cuántos días hay que mirar (atrás o adelante, desde `date`) para estar
    /// seguros de encontrar una vez, si la hay.
    fn horizon_days(&self, date: NaiveDate) -> i64 {
        match self {
            ScheduleRule::At { .. } | ScheduleRule::Every { .. } => 8,
            ScheduleRule::EveryDays { every, start, .. } => {
                let wait = parse_date(start).map(|s| (s - date).num_days().max(0)).unwrap_or(0);
                wait + *every as i64 + 1
            }
            ScheduleRule::Monthly { .. } => 62,
        }
    }
}

/// El momento real de una hora local. Si no existe (al adelantar el reloj,
/// las 2:30 se saltan), el primer minuto que sí existe después; si existe dos
/// veces (al atrasarlo), la primera, o las dos con `both`.
fn resolve<Tz: TimeZone>(tz: &Tz, local: NaiveDateTime, both: bool) -> Vec<DateTime<Tz>> {
    match tz.from_local_datetime(&local) {
        MappedLocalTime::Single(t) => vec![t],
        MappedLocalTime::Ambiguous(a, b) if both => vec![a, b],
        MappedLocalTime::Ambiguous(a, _) => vec![a],
        MappedLocalTime::None => (1..=6 * 60).find_map(|m| tz.from_local_datetime(&(local + Duration::minutes(m))).earliest()).into_iter().collect(),
    }
}

/// Los momentos en que toca alguna regla ese día (ordenados, sin repetir).
fn slots_on<Tz: TimeZone>(rules: &[ScheduleRule], tz: &Tz, date: NaiveDate) -> Vec<DateTime<Tz>> {
    let mut v: Vec<DateTime<Tz>> = Vec::new();
    for r in rules {
        let both = r.both_on_repeat();
        for t in r.times_on(date) {
            v.extend(resolve(tz, date.and_time(t), both));
        }
    }
    v.sort();
    v.dedup();
    v
}

fn horizon(rules: &[ScheduleRule], date: NaiveDate) -> i64 {
    rules.iter().map(|r| r.horizon_days(date)).max().unwrap_or(8).min(MAX_HORIZON_DAYS)
}

/// Cuándo se copia un plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanSchedule {
    /// Días de la semana: 0 = lunes … 6 = domingo. (Con `mode: "rules"`, cada regla lleva los suyos.)
    #[serde(default)]
    pub days: Vec<u8>,
    /// "at": a las horas de `times` · "every": cada `every_hours` horas entre `from` y `to` ·
    /// "rules": cuando toque cualquiera de las `rules` (agente gestionado ≥ 0.7.9).
    pub mode: String,
    #[serde(default)]
    pub times: Vec<String>,
    #[serde(default = "one")]
    pub every_hours: u32,
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub to: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<ScheduleRule>,
}

impl PlanSchedule {
    /// Un horario hecho de reglas.
    pub fn from_rules(rules: Vec<ScheduleRule>) -> Self {
        PlanSchedule { days: vec![], mode: "rules".into(), times: vec![], every_hours: 1, from: String::new(), to: String::new(), rules }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.mode == "rules" {
            if self.rules.is_empty() {
                return Err("Añade al menos una regla al horario.".into());
            }
            if self.rules.len() > MAX_RULES {
                return Err(format!("Demasiadas reglas en el horario (máximo {MAX_RULES})."));
            }
            return self.rules.iter().try_for_each(ScheduleRule::validate);
        }
        validate_days(&self.days)?;
        match self.mode.as_str() {
            "at" => {
                if self.times.is_empty() {
                    return Err("Añade al menos una hora.".into());
                }
                if self.times.len() > 48 {
                    return Err("Demasiadas horas (máximo 48 al día).".into());
                }
                for t in &self.times {
                    parse_time(t)?;
                }
            }
            "every" => {
                if !(1..=24).contains(&self.every_hours) {
                    return Err("El intervalo debe estar entre 1 y 24 horas.".into());
                }
                let (from, to) = (parse_time(&self.from)?, parse_time(&self.to)?);
                if to < from {
                    return Err("La hora final debe ser posterior a la inicial.".into());
                }
            }
            _ => return Err("Tipo de horario no válido.".into()),
        }
        Ok(())
    }

    /// El horario como reglas (los modos «at» y «every» son una regla cada uno).
    pub fn effective_rules(&self) -> Vec<ScheduleRule> {
        match self.mode.as_str() {
            "rules" => self.rules.clone(),
            "at" => vec![ScheduleRule::At { days: self.days.clone(), times: self.times.clone() }],
            _ => vec![ScheduleRule::Every {
                days: self.days.clone(),
                every_min: self.every_hours.clamp(1, 24) * 60,
                from: self.from.clone(),
                to: self.to.clone(),
            }],
        }
    }

    /// Horas del día en que toca (ordenadas, sin repetir). Con reglas, las de
    /// las reglas por días de la semana («cada N días» y «cada mes» no cuentan).
    pub fn times_of_day(&self) -> Vec<NaiveTime> {
        let mut out: Vec<NaiveTime> = Vec::new();
        for r in self.effective_rules() {
            match r {
                ScheduleRule::At { times, .. } => out.extend(times.iter().filter_map(|t| parse_time(t).ok())),
                ScheduleRule::Every { every_min, from, to, .. } => {
                    if let (Ok(from), Ok(to)) = (parse_time(&from), parse_time(&to)) {
                        out.extend(times_between(from, to, every_min));
                    }
                }
                _ => {}
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// Último momento programado que ya pasó (hasta 8 días atrás; más con
    /// «cada N días» o «cada mes»).
    pub fn latest_slot<Tz: TimeZone>(&self, now: DateTime<Tz>) -> Option<DateTime<Tz>> {
        let rules = self.effective_rules();
        let tz = now.timezone();
        let today = now.naive_local().date();
        (0..=horizon(&rules, today)).find_map(|back| slots_on(&rules, &tz, today - Duration::days(back)).into_iter().rev().find(|s| *s <= now))
    }

    /// Próximo momento programado después de `after` (hasta 8 días adelante;
    /// más con «cada N días» o «cada mes»).
    pub fn next_slot<Tz: TimeZone>(&self, after: DateTime<Tz>) -> Option<DateTime<Tz>> {
        let rules = self.effective_rules();
        let tz = after.timezone();
        let today = after.naive_local().date();
        (0..=horizon(&rules, today)).find_map(|fwd| slots_on(&rules, &tz, today + Duration::days(fwd)).into_iter().find(|s| *s > after))
    }

    /// ¿Toca copia? `since` es cuándo empezó la última copia del plan (o
    /// cuándo se activó).
    /// - Si el equipo estuvo apagado (o dormido) a la hora prevista, se copia
    ///   al encenderlo: una sola vez, aunque se hayan perdido varias.
    /// - Si `since` está en el futuro (el reloj estuvo adelantado y se
    ///   corrigió), no se espera a que llegue esa fecha: toca en la siguiente
    ///   hora programada que pase, como si no hubiera copia.
    /// - Entre el comienzo de dos copias pasan al menos [`MIN_GAP_MIN`] minutos.
    pub fn is_due<Tz: TimeZone>(&self, since: DateTime<Tz>, now: DateTime<Tz>) -> bool {
        let Some(slot) = self.latest_slot(now.clone()) else { return false };
        if clock_went_back(since.clone(), now.clone()) {
            return true;
        }
        since < slot && now.signed_duration_since(since) >= Duration::minutes(MIN_GAP_MIN)
    }

    /// Mayor separación entre dos copias seguidas (en horas, redondeada hacia
    /// arriba): a lo largo de dos semanas, o de dos años con «cada N días» o
    /// «cada mes». La web la usa para saber cuándo una copia va con retraso
    /// sin dar falsas alarmas por las noches o los fines de semana sin copias.
    pub fn max_gap_hours(&self) -> u32 {
        let rules = self.effective_rules();
        // Un lunes de un año bisiesto (o la primera vez de un «cada N días» posterior).
        let mut first = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        for r in &rules {
            if let ScheduleRule::EveryDays { start, .. } = r {
                if let Ok(s) = parse_date(start) {
                    first = first.max(s);
                }
            }
        }
        let long = rules.iter().any(|r| matches!(r, ScheduleRule::EveryDays { .. } | ScheduleRule::Monthly { .. }));
        let days = if long { 2 * 366 + 62 } else { 14 };
        let mut prev: Option<NaiveDateTime> = None;
        let mut gap = 0i64;
        for d in 0..days {
            // Un «cada N días» que empieza al final del calendario: hasta donde llegue.
            let Some(date) = first.checked_add_signed(Duration::days(d)) else { break };
            let mut ts: Vec<NaiveDateTime> = rules.iter().flat_map(|r| r.times_on(date)).map(|t| date.and_time(t)).collect();
            ts.sort();
            for t in ts {
                if let Some(p) = prev {
                    gap = gap.max((t - p).num_seconds());
                }
                prev = Some(t);
            }
        }
        if gap > 0 {
            ((gap as f64) / 3600.0).ceil().max(1.0) as u32
        } else {
            24 * 7
        }
    }
}

/// Un plan de copia de un repositorio (lo que se guarda en la app).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub excludes: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Sin horario: solo se copia a mano («Copiar ahora»).
    #[serde(default)]
    pub schedule: Option<PlanSchedule>,
    /// Solo guardar una versión si algo cambió (`restic backup --skip-if-unchanged`).
    /// Los planes anteriores no lo tienen: siguen guardando siempre.
    #[serde(default)]
    pub skip_unchanged: bool,
    /// Ganchos de plantilla (ganchos.rs). Solo los usa el agente gestionado.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ganchos: Vec<crate::ganchos::Gancho>,
}

/// Id del plan creado a partir de la configuración de versiones anteriores.
pub const LEGACY_PLAN: &str = "principal";

/// Clave de un plan en el estado del agente y en las solicitudes.
pub fn plan_key(repo_id: &str, plan_id: &str) -> String {
    format!("{repo_id}#{plan_id}")
}

impl Plan {
    pub fn validate(&self) -> Result<(), String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("El plan necesita un nombre.".into());
        }
        if name.chars().count() > 60 {
            return Err("El nombre del plan es demasiado largo (máximo 60 caracteres).".into());
        }
        if self.id.is_empty() || !self.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err("Identificador de plan no válido.".into());
        }
        if self.paths.is_empty() {
            return Err(format!("El plan «{name}» no tiene carpetas para copiar."));
        }
        if self.tags.len() > 10 {
            return Err("Demasiadas etiquetas (máximo 10).".into());
        }
        for t in &self.tags {
            if t.is_empty() || t.chars().count() > 40 || t.contains(',') || t.chars().any(char::is_whitespace) {
                return Err(format!("Etiqueta no válida: «{t}» (sin espacios ni comas, hasta 40 caracteres)."));
            }
        }
        if let Some(s) = &self.schedule {
            s.validate()?;
        }
        crate::ganchos::validar(&self.ganchos)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, Local};

    fn at(s: &str) -> DateTime<Local> {
        Local.from_local_datetime(&chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()).unwrap()
    }

    /// Lunes a sábado, cada hora en punto de 7:00 a 19:00.
    fn laboral() -> PlanSchedule {
        PlanSchedule {
            days: vec![0, 1, 2, 3, 4, 5],
            mode: "every".into(),
            times: vec![],
            every_hours: 1,
            from: "07:00".into(),
            to: "19:00".into(),
            rules: vec![],
        }
    }

    #[test]
    fn cada_hora_en_punto_de_lunes_a_sabado() {
        let s = laboral();
        assert_eq!(s.times_of_day().len(), 13);
        // 2026-09-28 es lunes.
        assert!(!s.is_due(at("2026-09-28 13:00"), at("2026-09-28 13:59")));
        assert!(s.is_due(at("2026-09-28 13:00"), at("2026-09-28 14:00")));
        // Fuera de horario no toca.
        assert!(!s.is_due(at("2026-09-28 19:00"), at("2026-09-28 23:30")));
        // El domingo (2026-10-04) no hay copias: la última fue el sábado a las 19:00.
        assert!(!s.is_due(at("2026-10-03 19:00"), at("2026-10-04 13:00")));
        assert_eq!(s.next_slot(at("2026-10-03 19:30")), Some(at("2026-10-05 07:00")));
        // Mayor hueco: del sábado 19:00 al lunes 7:00.
        assert_eq!(s.max_gap_hours(), 36);
    }

    #[test]
    fn reloj_adelantado_y_corregido() {
        let s = laboral();
        // La última copia quedó anotada en 2030 (reloj adelantado); ya se corrigió.
        assert!(s.is_due(at("2030-01-07 10:00"), at("2026-09-28 14:05")));
        // Un ajuste pequeño del reloj (NTP) no cuenta: sigue la regla de siempre.
        assert!(!s.is_due(at("2026-09-28 14:08"), at("2026-09-28 14:05")));
        assert!(clock_went_back(at("2026-09-28 14:16"), at("2026-09-28 14:05")));
        assert!(!clock_went_back(at("2026-09-28 14:14"), at("2026-09-28 14:05")));
    }

    #[test]
    fn horas_concretas_y_equipo_apagado() {
        let s = PlanSchedule {
            days: vec![6],
            mode: "at".into(),
            times: vec!["23:00".into()],
            every_hours: 1,
            from: String::new(),
            to: String::new(),
            rules: vec![],
        };
        assert!(s.is_due(at("2026-09-27 23:00"), at("2026-10-04 23:00")));
        // Apagado el domingo a esa hora: se copia al encenderlo el lunes.
        assert!(s.is_due(at("2026-09-27 23:00"), at("2026-10-05 08:00")));
        assert_eq!(s.max_gap_hours(), 168);
    }

    #[test]
    fn validaciones() {
        assert!(laboral().validate().is_ok());
        let mut s = laboral();
        s.days.clear();
        assert!(s.validate().is_err());
        let mut s = laboral();
        s.to = "06:00".into();
        assert!(s.validate().is_err());
        let plan = Plan {
            id: "a".into(),
            name: "Laboral".into(),
            paths: vec!["S:\\".into()],
            excludes: vec![],
            tags: vec!["siigo horaria".into()],
            schedule: None,
            skip_unchanged: false,
            ganchos: vec![],
        };
        assert!(plan.validate().is_err(), "etiqueta con espacio");
    }

    #[test]
    fn plan_anterior_guarda_siempre() {
        // Un plan guardado por una versión anterior (sin `skip_unchanged`).
        let old = r#"{"id":"a","name":"Laboral","paths":["S:\\"],"excludes":[],"tags":[],"schedule":null}"#;
        let plan: Plan = serde_json::from_str(old).unwrap();
        assert!(!plan.skip_unchanged, "los planes existentes no cambian de comportamiento");
        let json = serde_json::to_value(Plan { skip_unchanged: true, ..plan }).unwrap();
        assert_eq!(json["skip_unchanged"], true);
    }

    // ---------- Horarios por reglas (agente gestionado ≥ 0.7.9) ----------

    fn nd(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    /// Una zona sin cambios de hora (UTC−5): las pruebas dan lo mismo en cualquier equipo.
    fn fx(s: &str) -> DateTime<FixedOffset> {
        FixedOffset::west_opt(5 * 3600).unwrap().from_local_datetime(&nd(s)).unwrap()
    }

    fn cada(every_min: u32, days: &[u8], from: &str, to: &str) -> ScheduleRule {
        ScheduleRule::Every { days: days.to_vec(), every_min, from: from.into(), to: to.into() }
    }

    const LUN_VIE: [u8; 5] = [0, 1, 2, 3, 4];

    #[test]
    fn cada_10_minutos_en_horario_laboral() {
        let s = PlanSchedule::from_rules(vec![cada(10, &LUN_VIE, "08:00", "18:00")]);
        assert!(s.validate().is_ok());
        assert_eq!(s.times_of_day().len(), 61);
        // 2026-10-05 es lunes.
        assert!(!s.is_due(fx("2026-10-05 10:00"), fx("2026-10-05 10:09")));
        assert!(s.is_due(fx("2026-10-05 10:00"), fx("2026-10-05 10:10")));
        assert_eq!(s.next_slot(fx("2026-10-05 10:00")), Some(fx("2026-10-05 10:10")));
        // El viernes a las 18:00 la última; la siguiente, el lunes a las 8:00.
        assert_eq!(s.next_slot(fx("2026-10-02 18:00")), Some(fx("2026-10-05 08:00")));
        assert!(!s.is_due(fx("2026-10-02 18:00"), fx("2026-10-03 12:00")), "el sábado no toca");
        // Equipo dormido de 9:00 a 13:47: una sola copia al despertar, no 28.
        assert!(s.is_due(fx("2026-10-05 09:00"), fx("2026-10-05 13:47")));
        assert_eq!(s.latest_slot(fx("2026-10-05 13:47")), Some(fx("2026-10-05 13:40")));
        assert!(!s.is_due(fx("2026-10-05 13:47"), fx("2026-10-05 13:49")));
        // La de las 13:50 espera a que pasen 5 minutos desde que empezó la anterior.
        assert!(!s.is_due(fx("2026-10-05 13:47"), fx("2026-10-05 13:50")));
        assert!(s.is_due(fx("2026-10-05 13:47"), fx("2026-10-05 13:52")));
        // Del viernes a las 18:00 al lunes a las 8:00.
        assert_eq!(s.max_gap_hours(), 62);
    }

    #[test]
    fn cada_5_minutos_todo_el_dia() {
        let s = PlanSchedule::from_rules(vec![cada(5, &[0, 1, 2, 3, 4, 5, 6], "00:00", "23:55")]);
        assert!(s.validate().is_ok());
        assert_eq!(s.times_of_day().len(), 288);
        assert_eq!(s.next_slot(fx("2026-12-31 23:55")), Some(fx("2027-01-01 00:00")));
        assert_eq!(s.max_gap_hours(), 1);
    }

    #[test]
    fn cada_n_dias_desde_una_fecha() {
        let s = PlanSchedule::from_rules(vec![ScheduleRule::EveryDays { every: 3, start: "2026-10-01".into(), time: "23:00".into() }]);
        assert!(s.validate().is_ok());
        // Antes de empezar no hay ninguna anterior; la primera, el día de inicio.
        assert_eq!(s.latest_slot(fx("2026-10-01 22:00")), None);
        assert_eq!(s.next_slot(fx("2026-09-20 10:00")), Some(fx("2026-10-01 23:00")));
        assert_eq!(s.next_slot(fx("2026-10-02 10:00")), Some(fx("2026-10-04 23:00")));
        // Apagado varios días: una copia al encender (la del 7), y la siguiente el 10.
        assert!(s.is_due(fx("2026-10-01 23:01"), fx("2026-10-08 09:00")));
        assert_eq!(s.latest_slot(fx("2026-10-08 09:00")), Some(fx("2026-10-07 23:00")));
        assert!(!s.is_due(fx("2026-10-08 09:00"), fx("2026-10-10 22:59")));
        assert!(s.is_due(fx("2026-10-08 09:00"), fx("2026-10-10 23:00")));
        assert_eq!(s.max_gap_hours(), 72);
        // Cruza el fin de año sin perder la cuenta.
        let semanal = PlanSchedule::from_rules(vec![ScheduleRule::EveryDays { every: 7, start: "2026-12-28".into(), time: "08:00".into() }]);
        assert_eq!(semanal.next_slot(fx("2027-01-01 00:00")), Some(fx("2027-01-04 08:00")));
        // Un año entre copias, empezando dentro de 300 días: también se encuentra.
        let anual = PlanSchedule::from_rules(vec![ScheduleRule::EveryDays { every: 365, start: "2027-08-01".into(), time: "08:00".into() }]);
        assert_eq!(anual.next_slot(fx("2026-10-05 00:00")), Some(fx("2027-08-01 08:00")));
        assert_eq!(anual.next_slot(fx("2027-08-01 08:00")), Some(fx("2028-07-31 08:00")), "2028 es bisiesto");
        assert_eq!(anual.max_gap_hours(), 365 * 24);
    }

    #[test]
    fn un_dia_de_cada_mes_y_el_ultimo() {
        let uno = PlanSchedule::from_rules(vec![ScheduleRule::Monthly { day: 1, time: "23:00".into() }]);
        assert_eq!(uno.next_slot(fx("2026-10-02 00:00")), Some(fx("2026-11-01 23:00")));
        // Apagado el día 1: se copia al encender, una vez.
        assert!(uno.is_due(fx("2026-09-01 23:00"), fx("2026-10-03 09:00")));
        assert!(!uno.is_due(fx("2026-10-03 09:00"), fx("2026-10-31 23:59")));
        assert_eq!(uno.max_gap_hours(), 31 * 24);

        let ultimo = PlanSchedule::from_rules(vec![ScheduleRule::Monthly { day: -1, time: "22:00".into() }]);
        assert_eq!(ultimo.next_slot(fx("2027-02-10 00:00")), Some(fx("2027-02-28 22:00")));
        assert_eq!(ultimo.next_slot(fx("2028-02-10 00:00")), Some(fx("2028-02-29 22:00")), "bisiesto");
        assert_eq!(ultimo.next_slot(fx("2028-02-28 23:00")), Some(fx("2028-02-29 22:00")));
        assert_eq!(ultimo.next_slot(fx("2026-04-30 22:00")), Some(fx("2026-05-31 22:00")));
        assert_eq!(ultimo.next_slot(fx("2026-12-31 22:30")), Some(fx("2027-01-31 22:00")));
        assert_eq!(ultimo.latest_slot(fx("2026-07-15 12:00")), Some(fx("2026-06-30 22:00")));
        assert_eq!(ultimo.max_gap_hours(), 31 * 24);

        // El 28 existe todos los meses (por eso no se ofrecen el 29, 30 ni 31).
        let d28 = PlanSchedule::from_rules(vec![ScheduleRule::Monthly { day: 28, time: "08:00".into() }]);
        assert_eq!(d28.next_slot(fx("2027-02-01 00:00")), Some(fx("2027-02-28 08:00")));
        for (day, ok) in [(1, true), (28, true), (-1, true), (0, false), (29, false), (31, false), (-2, false)] {
            assert_eq!(ScheduleRule::Monthly { day, time: "08:00".into() }.validate().is_ok(), ok, "día {day}");
        }
    }

    #[test]
    fn reglas_combinadas() {
        let s = PlanSchedule::from_rules(vec![cada(10, &LUN_VIE, "08:00", "18:00"), ScheduleRule::Monthly { day: 1, time: "23:00".into() }]);
        assert!(s.validate().is_ok());
        // Viernes 30 de octubre por la tarde: la próxima, el domingo 1 a las 23:00; después, el lunes a las 8:00.
        assert_eq!(s.next_slot(fx("2026-10-30 18:30")), Some(fx("2026-11-01 23:00")));
        assert_eq!(s.next_slot(fx("2026-11-01 23:00")), Some(fx("2026-11-02 08:00")));
        assert!(s.is_due(fx("2026-10-30 18:00"), fx("2026-11-01 23:00")));
        assert_eq!(s.max_gap_hours(), 62);
        // Una misma hora en dos reglas cuenta una vez.
        let dos = PlanSchedule::from_rules(vec![
            cada(10, &LUN_VIE, "08:00", "18:00"),
            ScheduleRule::At { days: vec![0], times: vec!["08:00".into(), "12:00".into()] },
        ]);
        let tz = FixedOffset::west_opt(5 * 3600).unwrap();
        assert_eq!(slots_on(&dos.effective_rules(), &tz, NaiveDate::from_ymd_opt(2026, 10, 5).unwrap()).len(), 61);
        // Con el reloj atrasado (la última quedó en 2030), toca en la siguiente que pase.
        assert!(s.is_due(fx("2030-01-01 00:00"), fx("2026-10-05 10:05")));
    }

    /// Una zona con cambio de hora genérico (UTC+1 en invierno y UTC+2 en
    /// verano; el 29-03-2026 las 2:00 pasan a ser las 3:00, y el 25-10-2026
    /// las 3:00 vuelven a ser las 2:00), sin depender de la zona del equipo.
    #[derive(Clone, Copy, Debug)]
    struct Verano;

    impl Verano {
        fn inv() -> FixedOffset {
            FixedOffset::east_opt(3600).unwrap()
        }
        fn ver() -> FixedOffset {
            FixedOffset::east_opt(7200).unwrap()
        }
    }

    impl TimeZone for Verano {
        type Offset = FixedOffset;
        fn from_offset(_: &FixedOffset) -> Self {
            Verano
        }
        fn offset_from_local_date(&self, _: &NaiveDate) -> MappedLocalTime<FixedOffset> {
            MappedLocalTime::Single(Verano::inv())
        }
        fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> MappedLocalTime<FixedOffset> {
            if *local >= nd("2026-03-29 02:00") && *local < nd("2026-03-29 03:00") {
                MappedLocalTime::None
            } else if *local >= nd("2026-10-25 02:00") && *local < nd("2026-10-25 03:00") {
                MappedLocalTime::Ambiguous(Verano::ver(), Verano::inv())
            } else if *local >= nd("2026-03-29 03:00") && *local < nd("2026-10-25 02:00") {
                MappedLocalTime::Single(Verano::ver())
            } else {
                MappedLocalTime::Single(Verano::inv())
            }
        }
        fn offset_from_utc_date(&self, _: &NaiveDate) -> FixedOffset {
            Verano::inv()
        }
        fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> FixedOffset {
            if *utc >= nd("2026-03-29 01:00") && *utc < nd("2026-10-25 01:00") {
                Verano::ver()
            } else {
                Verano::inv()
            }
        }
    }

    fn v(s: &str) -> DateTime<Verano> {
        Verano.from_local_datetime(&nd(s)).earliest().unwrap()
    }

    #[test]
    fn cambio_de_hora() {
        let todos = [0, 1, 2, 3, 4, 5, 6];
        let a_las_230 = PlanSchedule::from_rules(vec![ScheduleRule::At { days: todos.to_vec(), times: vec!["02:30".into()] }]);
        // Al adelantar el reloj, las 2:30 no existen: se copia a las 3:00 (no se pierde el día).
        assert_eq!(a_las_230.next_slot(v("2026-03-29 01:00")), Some(v("2026-03-29 03:00")));
        assert!(a_las_230.is_due(v("2026-03-28 02:30"), v("2026-03-29 03:05")));
        // Al atrasarlo, las 2:30 pasan dos veces: una sola copia.
        let primera = v("2026-10-25 02:30");
        let segunda_240 = Verano.from_local_datetime(&nd("2026-10-25 02:40")).latest().unwrap();
        assert!(a_las_230.is_due(v("2026-10-24 02:30"), primera));
        assert!(!a_las_230.is_due(primera, segunda_240));
        assert_eq!(a_las_230.next_slot(primera), Some(v("2026-10-26 02:30")));

        let tz = Verano;
        let cada10 = vec![cada(10, &todos, "00:00", "06:00")];
        // 37 veces de 0:00 a 6:00; al adelantar, las seis de 2:00 a 2:50 se juntan en las 3:00.
        assert_eq!(slots_on(&cada10, &tz, NaiveDate::from_ymd_opt(2026, 3, 29).unwrap()).len(), 31);
        // Al atrasar, «cada 10 minutos» sigue cada 10 minutos en la hora repetida.
        let otono = slots_on(&cada10, &tz, NaiveDate::from_ymd_opt(2026, 10, 25).unwrap());
        assert_eq!(otono.len(), 43);
        assert!(otono.windows(2).all(|w| w[1].signed_duration_since(w[0]) == Duration::minutes(10)));
        let s = PlanSchedule::from_rules(cada10);
        assert!(s.is_due(primera, segunda_240), "en la hora repetida sigue copiando");
        assert_eq!(s.next_slot(segunda_240), Some(Verano.from_local_datetime(&nd("2026-10-25 02:50")).latest().unwrap()));
    }

    #[test]
    fn validacion_de_reglas() {
        for (m, ok) in
            [(5, true), (10, true), (30, true), (7, false), (45, false), (60, true), (90, false), (120, true), (1440, true), (1500, false), (0, false)]
        {
            assert_eq!(cada(m, &[0], "08:00", "18:00").validate().is_ok(), ok, "cada {m} min");
        }
        assert!(cada(10, &[], "08:00", "18:00").validate().is_err(), "sin días");
        assert!(cada(10, &[7], "08:00", "18:00").validate().is_err(), "día 7 no existe (0 = lunes)");
        assert!(cada(10, &[0], "18:00", "08:00").validate().is_err(), "hasta antes que desde");
        let dias = |every: u32, start: &str| ScheduleRule::EveryDays { every, start: start.into(), time: "08:00".into() }.validate().is_ok();
        assert!(dias(1, "2026-10-01") && dias(365, "2026-10-01"));
        assert!(!dias(0, "2026-10-01") && !dias(366, "2026-10-01") && !dias(3, "2026-13-01") && !dias(3, "1/10/2026"));
        assert!(ScheduleRule::At { days: vec![0], times: vec![] }.validate().is_err());
        assert!(PlanSchedule::from_rules(vec![]).validate().is_err());
        assert!(PlanSchedule::from_rules(vec![cada(10, &[0], "08:00", "18:00"); 21]).validate().is_err());
        assert!(PlanSchedule::from_rules(vec![cada(10, &[0], "08:00", "18:00"); 20]).validate().is_ok());
    }

    #[test]
    fn horario_por_reglas_en_json() {
        // Los planes de siempre se guardan igual (sin `rules`).
        let viejo: PlanSchedule = serde_json::from_str(r#"{"days":[0],"mode":"at","times":["13:00"]}"#).unwrap();
        assert!(viejo.rules.is_empty());
        assert!(serde_json::to_value(&viejo).unwrap().get("rules").is_none());
        let nuevo: PlanSchedule = serde_json::from_str(
            r#"{"mode":"rules","rules":[{"kind":"every","days":[0],"every_min":10,"from":"08:00","to":"18:00"},{"kind":"monthly","day":-1,"time":"23:00"},{"kind":"every_days","every":3,"start":"2026-10-01","time":"23:00"}]}"#,
        )
        .unwrap();
        assert!(nuevo.validate().is_ok());
        assert_eq!(nuevo.rules.len(), 3);
        let otra_vez: PlanSchedule = serde_json::from_value(serde_json::to_value(&nuevo).unwrap()).unwrap();
        assert_eq!(otra_vez, nuevo);
    }
}
