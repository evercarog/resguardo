//! Las reglas de las notificaciones, sin base de datos ni red (para probarlas
//! sin esperar): gravedad de cada tipo de aviso, agrupar lo repetido, horas
//! de silencio, límite por hora y reintentos.

use super::{Incidente, Severidad, Silencio};
use crate::almacen::Ts;
use chrono::{Duration, NaiveDateTime, NaiveTime};

/// Un problema que sigue abierto se vuelve a contar como nuevo si pasa tanto sin repetirse.
pub const OLVIDAR_S: Ts = 24 * 3600;
/// Mientras sigue abierto, se recuerda como mucho una vez cada tanto («Sigue fallando»).
pub const RECORDAR_S: Ts = 24 * 3600;
/// Un fallo más antiguo que esto (p. ej. al actualizar el servidor) se anota sin avisar.
pub const ANTIGUO_S: Ts = 3 * 24 * 3600;
/// Intentos de entrega antes de darla por fallida.
pub const MAX_INTENTOS: u32 = 8;
/// Envíos por hora y canal (y destinatario) si el propietario no dice otra cosa.
pub const MAX_POR_HORA: u32 = 10;

/// Gravedad de cada tipo de aviso.
pub fn severidad(tipo: &str) -> Severidad {
    match tipo {
        "copia_fallida"
        | "verificacion_fallida"
        | "bloqueo"
        | "intentos_fallidos"
        | "cambio_inusual"
        | "servicio_detenido"
        | "orden_destructiva"
        | "orden_en_espera"
        | "cambio_clave"
        | "auditoria_rehecha" => Severidad::Critico,
        "equipo_sin_contacto"
        | "copia_atrasada"
        | "espejo_fallido"
        | "retencion_fallida"
        | "externa_fallida"
        | "prueba_fallida"
        | "cadena_parada"
        | "orden_no_aplicada"
        | "actualizacion_fallida" => Severidad::Importante,
        _ => Severidad::Informativo,
    }
}

/// v1.52: la gravedad de un aviso de un equipo cuyas etiquetas piden más importancia.
/// Sube hasta la más alta que pidan, pero solo lo que ya es importante o crítico de por sí
/// (lo informativo no se vuelve urgente). Nunca baja.
pub fn severidad_con_etiquetas(base: Severidad, pedidas: &[Severidad]) -> Severidad {
    if base == Severidad::Informativo {
        return base;
    }
    pedidas.iter().copied().fold(base, Severidad::max)
}

/// Qué hacer cuando vuelve a pasar algo (por la clave del incidente).
#[derive(Debug, PartialEq, Eq)]
pub enum Paso {
    /// Nuevo (o volvió después de arreglarse u olvidarse): se avisa.
    Abrir { avisar: bool },
    /// Otra vez lo mismo mientras sigue abierto: se cuenta y, como mucho una vez al día, se recuerda.
    Repetir { recordar: bool },
    /// El mismo informe otra vez (la misma marca): nada.
    Nada,
}

/// `marca`: lo que distingue una vuelta de otra (la hora de la copia que falló), si la hay;
/// `cuando`: cuándo pasó de verdad, si se sabe (un fallo de hace días no avisa).
pub fn al_ocurrir(prev: Option<&Incidente>, ahora: Ts, marca: Option<&str>, cuando: Option<Ts>) -> Paso {
    let reciente = cuando.is_none_or(|c| ahora - c <= ANTIGUO_S);
    // Uno con marca (sale de los informes) sigue abierto hasta que el equipo diga que se
    // arregló, aunque pase más de un día (una copia que falla cada semana); uno sin marca
    // (un aviso suelto) se olvida tras un día sin repetirse.
    let sigue = prev.filter(|p| p.abierto && (p.marca.is_some() || ahora - p.ultimo <= OLVIDAR_S));
    match sigue {
        Some(p) if marca.is_some() && p.marca.as_deref() == marca => Paso::Nada,
        Some(p) => Paso::Repetir { recordar: reciente && p.notificado.is_none_or(|n| ahora - n >= RECORDAR_S) },
        None => Paso::Abrir { avisar: reciente },
    }
}

/// ¿Avisar de que se arregló? Solo si se avisó de que estaba mal.
pub fn al_recuperar(inc: &Incidente) -> bool {
    inc.abierto && inc.notificado.is_some()
}

/// Minutos desde medianoche de «HH:MM».
pub fn minutos(hhmm: &str) -> Option<u32> {
    let t = NaiveTime::parse_from_str(hhmm, "%H:%M").ok()?;
    (hhmm.len() == 5).then_some(())?;
    Some(chrono::Timelike::hour(&t) * 60 + chrono::Timelike::minute(&t))
}

/// ¿Está `ahora` (hora local) dentro de las horas de silencio? `desde` incluido, `hasta` no.
/// Si `desde` > `hasta`, cruza la medianoche (22:00–07:00). Iguales: sin silencio.
pub fn en_silencio(s: &Silencio, ahora: NaiveDateTime) -> bool {
    let (Some(d), Some(h)) = (minutos(&s.desde), minutos(&s.hasta)) else { return false };
    let m = chrono::Timelike::hour(&ahora) * 60 + chrono::Timelike::minute(&ahora);
    match d.cmp(&h) {
        std::cmp::Ordering::Equal => false,
        std::cmp::Ordering::Less => m >= d && m < h,
        std::cmp::Ordering::Greater => m >= d || m < h,
    }
}

/// Cuándo termina el silencio en el que está `ahora` (la próxima vez que dan las `hasta`).
pub fn fin_silencio(s: &Silencio, ahora: NaiveDateTime) -> NaiveDateTime {
    let h = minutos(&s.hasta).unwrap_or(0);
    let hasta = NaiveTime::from_hms_opt(h / 60, h % 60, 0).unwrap_or_default();
    let hoy = ahora.date().and_time(hasta);
    if hoy > ahora {
        hoy
    } else {
        hoy + Duration::days(1)
    }
}

/// Si hay que esperar al final del silencio para mandarlo: hasta cuándo.
pub fn retrasar(s: Option<&Silencio>, sev: Severidad, ahora: NaiveDateTime) -> Option<NaiveDateTime> {
    let s = s?;
    if s.salvo_criticos && sev == Severidad::Critico {
        return None;
    }
    en_silencio(s, ahora).then(|| fin_silencio(s, ahora))
}

/// Espera antes del intento `n` (el primero que falló es el 1): 1, 2, 4, 8, 16, 32 min y
/// después cada hora.
pub fn espera_reintento(n: u32) -> Ts {
    let base: Ts = 60;
    base.saturating_mul(1 << n.saturating_sub(1).min(10)).min(3600)
}

/// Cuántos envíos quedan esta hora a un destino.
pub fn cupo(enviados_ultima_hora: u32, max: u32) -> u32 {
    max.saturating_sub(enviados_ultima_hora)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn inc(abierto: bool, ultimo: Ts, notificado: Option<Ts>, marca: Option<&str>) -> Incidente {
        Incidente {
            clave: "c|e|copia_fallida|k".into(),
            cliente: "c".into(),
            equipo: Some("e".into()),
            tipo: "copia_fallida".into(),
            sujeto: "k".into(),
            severidad: Severidad::Critico,
            titulo: "t".into(),
            mensaje: "m".into(),
            abierto,
            primero: 0,
            ultimo,
            veces: 1,
            marca: marca.map(Into::into),
            notificado,
            cerrado: None,
        }
    }

    fn hora(h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 5).unwrap().and_hms_opt(h, m, 0).unwrap()
    }

    #[test]
    fn agrupa_lo_repetido_y_recuerda_una_vez_al_dia() {
        let t = 1_000_000;
        // Nuevo: se avisa.
        assert_eq!(al_ocurrir(None, t, Some("10:00"), Some(t)), Paso::Abrir { avisar: true });
        // El mismo informe otra vez: nada.
        let abierto = inc(true, t, Some(t), Some("10:00"));
        assert_eq!(al_ocurrir(Some(&abierto), t + 300, Some("10:00"), Some(t)), Paso::Nada);
        // Otra vuelta que falla, una hora después: se cuenta, sin avisar otra vez.
        assert_eq!(al_ocurrir(Some(&abierto), t + 3600, Some("11:00"), Some(t + 3600)), Paso::Repetir { recordar: false });
        // Un día después sigue fallando: se recuerda.
        assert_eq!(al_ocurrir(Some(&abierto), t + RECORDAR_S, Some("x"), Some(t + RECORDAR_S)), Paso::Repetir { recordar: true });
        // Cerrado (se arregló) y vuelve a fallar: nuevo aviso.
        let cerrado = inc(false, t, Some(t), Some("10:00"));
        assert_eq!(al_ocurrir(Some(&cerrado), t + 60, Some("11:00"), Some(t + 60)), Paso::Abrir { avisar: true });
        // Un aviso sin marca, más de un día después: se trata como nuevo.
        let viejo = inc(true, t, Some(t), None);
        assert_eq!(al_ocurrir(Some(&viejo), t + OLVIDAR_S + 1, None, None), Paso::Abrir { avisar: true });
        assert_eq!(al_ocurrir(Some(&viejo), t + 60, None, None), Paso::Repetir { recordar: false });
        // Un fallo de hace una semana (al actualizar el servidor): se anota sin avisar.
        assert_eq!(al_ocurrir(None, t, Some("viejo"), Some(t - 7 * 24 * 3600)), Paso::Abrir { avisar: false });
    }

    #[test]
    fn recuperacion_solo_si_se_aviso() {
        assert!(al_recuperar(&inc(true, 0, Some(0), Some("a"))));
        assert!(!al_recuperar(&inc(true, 0, None, Some("a"))));
        assert!(!al_recuperar(&inc(false, 0, Some(0), Some("a"))));
    }

    #[test]
    fn horas_de_silencio() {
        let noche = Silencio { desde: "22:00".into(), hasta: "07:00".into(), salvo_criticos: true };
        assert!(en_silencio(&noche, hora(23, 30)));
        assert!(en_silencio(&noche, hora(3, 0)));
        assert!(en_silencio(&noche, hora(22, 0)));
        assert!(!en_silencio(&noche, hora(7, 0)));
        assert!(!en_silencio(&noche, hora(12, 0)));
        // Fin del silencio: las 07:00 del día siguiente (o de hoy, de madrugada).
        assert_eq!(fin_silencio(&noche, hora(23, 30)), hora(7, 0) + Duration::days(1));
        assert_eq!(fin_silencio(&noche, hora(3, 0)), hora(7, 0));
        // Los críticos pasan si así se quiso; los demás esperan.
        assert_eq!(retrasar(Some(&noche), Severidad::Critico, hora(23, 0)), None);
        assert_eq!(retrasar(Some(&noche), Severidad::Importante, hora(23, 0)), Some(hora(7, 0) + Duration::days(1)));
        assert_eq!(retrasar(Some(&noche), Severidad::Importante, hora(12, 0)), None);
        let siesta = Silencio { desde: "13:00".into(), hasta: "15:00".into(), salvo_criticos: false };
        assert!(en_silencio(&siesta, hora(14, 59)));
        assert!(!en_silencio(&siesta, hora(15, 0)));
        assert_eq!(retrasar(Some(&siesta), Severidad::Critico, hora(13, 10)), Some(hora(15, 0)));
        let nada = Silencio { desde: "08:00".into(), hasta: "08:00".into(), salvo_criticos: false };
        assert!(!en_silencio(&nada, hora(8, 0)));
        assert_eq!(minutos("7:00"), None);
        assert_eq!(minutos("25:00"), None);
    }

    #[test]
    fn reintentos_y_cupo() {
        assert_eq!(espera_reintento(1), 60);
        assert_eq!(espera_reintento(2), 120);
        assert_eq!(espera_reintento(6), 1920);
        assert_eq!(espera_reintento(7), 3600);
        assert_eq!(espera_reintento(40), 3600);
        assert_eq!(cupo(3, 10), 7);
        assert_eq!(cupo(12, 10), 0);
    }

    #[test]
    fn gravedad_por_etiquetas() {
        use Severidad::*;
        assert_eq!(severidad_con_etiquetas(Importante, &[Critico]), Critico);
        assert_eq!(severidad_con_etiquetas(Critico, &[Importante]), Critico, "nunca baja");
        assert_eq!(severidad_con_etiquetas(Importante, &[]), Importante);
        assert_eq!(severidad_con_etiquetas(Informativo, &[Critico]), Informativo, "lo informativo no sube");
        assert_eq!(severidad_con_etiquetas(Importante, &[Importante, Critico]), Critico);
    }

    #[test]
    fn gravedades() {
        assert_eq!(severidad("copia_fallida"), Severidad::Critico);
        assert_eq!(severidad("orden_en_espera"), Severidad::Critico);
        assert_eq!(severidad("equipo_sin_contacto"), Severidad::Importante);
        assert_eq!(severidad("lo_que_sea"), Severidad::Informativo);
    }
}
