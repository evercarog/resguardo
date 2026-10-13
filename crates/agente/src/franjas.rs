//! Plan 0.8.0, F4 (14t): límite de velocidad según la hora en los espejos y en las
//! copias derivadas o externas (p. ej. 2 MB/s de 8:00 a 18:00 los laborables y sin
//! límite el resto).
//!
//! Una lista de franjas, cada una con sus días, su hora de empezar y de terminar
//! (el mismo día) y su límite en KiB/s (0: sin límite). Fuera de todas, el límite de
//! siempre (el que ya tuviera, o ninguno).
//!
//! - **restic** (copias derivadas y externas): `--limit-upload` con el de la franja en
//!   la que **empieza** cada vuelta (restic no cambia de límite a mitad).
//! - **rclone** (espejos a una nube): `--bwlimit` con su horario propio
//!   («Mon-08:00,2048K Mon-18:00,off …»), que cambia el límite a su hora aunque la
//!   subida sea larga.

use chrono::{DateTime, Datelike, Local, NaiveTime};
use serde::{Deserialize, Serialize};

/// Franjas como mucho.
pub const MAX_FRANJAS: usize = 8;
/// El límite más bajo que se admite (KiB/s): menos no sirve para nada.
pub const MIN_KIB: u32 = 64;
/// Y el más alto (unos 10 GB/s: en la práctica, sin límite).
pub const MAX_KIB: u32 = 10_000_000;

/// Una franja: los días (1 = lunes … 7 = domingo), de `desde` a `hasta` («HH:MM»,
/// el mismo día, `desde` antes que `hasta`) y su límite (KiB/s; 0, sin límite).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Franja {
    pub dias: Vec<u8>,
    pub desde: String,
    pub hasta: String,
    pub kib: u32,
}

fn hora(h: &str) -> Option<NaiveTime> {
    (h.len() == 5).then(|| NaiveTime::parse_from_str(h, "%H:%M").ok()).flatten()
}

/// Comprueba las franjas: como mucho [`MAX_FRANJAS`], días de 1 a 7, horas válidas
/// con `desde` antes que `hasta`, límites de [`MIN_KIB`] a [`MAX_KIB`] (o 0) y sin
/// que dos se pisen el mismo día.
pub fn validar(franjas: &[Franja]) -> Result<(), String> {
    if franjas.len() > MAX_FRANJAS {
        return Err(format!("Como mucho {MAX_FRANJAS} franjas de velocidad."));
    }
    let mut tramos: Vec<(u8, NaiveTime, NaiveTime)> = Vec::new();
    for f in franjas {
        if f.dias.is_empty() || f.dias.iter().any(|d| !(1..=7).contains(d)) {
            return Err("Cada franja de velocidad necesita sus días (1 = lunes … 7 = domingo).".into());
        }
        let (Some(a), Some(b)) = (hora(&f.desde), hora(&f.hasta)) else {
            return Err("Las horas de las franjas de velocidad son HH:MM.".into());
        };
        if a >= b {
            return Err(format!("La franja de {} a {} tiene que empezar antes de terminar (el mismo día).", f.desde, f.hasta));
        }
        if f.kib != 0 && !(MIN_KIB..=MAX_KIB).contains(&f.kib) {
            return Err(format!("El límite de cada franja va de {MIN_KIB} KiB/s a {MAX_KIB} KiB/s (o sin límite)."));
        }
        for d in &f.dias {
            if tramos.iter().any(|(e, x, y)| e == d && a < *y && *x < b) {
                return Err("Dos franjas de velocidad se pisan el mismo día.".into());
            }
            tramos.push((*d, a, b));
        }
    }
    Ok(())
}

/// El límite (KiB/s) a esta hora: el de la franja en la que está o, fuera de todas,
/// `por_defecto`. `None`: sin límite.
pub fn limite_ahora(franjas: &[Franja], por_defecto: Option<u32>, ahora: DateTime<Local>) -> Option<u32> {
    let dia = ahora.weekday().number_from_monday() as u8;
    let t = ahora.time();
    let f = franjas.iter().find(|f| f.dias.contains(&dia) && hora(&f.desde).is_some_and(|a| a <= t) && hora(&f.hasta).is_some_and(|b| t < b));
    match f {
        Some(f) => (f.kib > 0).then_some(f.kib),
        None => por_defecto.filter(|k| *k > 0),
    }
}

const DIAS_RCLONE: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

fn valor_rclone(kib: Option<u32>) -> String {
    match kib.filter(|k| *k > 0) {
        Some(k) => format!("{k}K"),
        None => "off".into(),
    }
}

/// El `--bwlimit` de rclone: sin franjas, el límite de siempre (`None` si no hay);
/// con franjas, su horario semanal («Mon-00:00,off Mon-08:00,2048K Mon-18:00,off …»).
pub fn bwlimit(franjas: &[Franja], por_defecto: Option<u32>) -> Option<String> {
    if franjas.is_empty() {
        return por_defecto.filter(|k| *k > 0).map(|k| format!("{k}K"));
    }
    let mut partes = Vec::new();
    for (i, nombre) in DIAS_RCLONE.iter().enumerate() {
        let dia = i as u8 + 1;
        let mut puntos: Vec<(NaiveTime, Option<u32>)> = vec![(NaiveTime::MIN, por_defecto)];
        let mut del_dia: Vec<&Franja> = franjas.iter().filter(|f| f.dias.contains(&dia)).collect();
        del_dia.sort_by_key(|f| hora(&f.desde));
        for f in del_dia {
            let (Some(a), Some(b)) = (hora(&f.desde), hora(&f.hasta)) else { continue };
            puntos.retain(|(t, _)| *t != a);
            puntos.push((a, Some(f.kib)));
            puntos.push((b, por_defecto));
        }
        puntos.sort_by_key(|p| p.0);
        // Sin repetir el mismo valor seguido ni dos puntos a la misma hora (gana el último).
        let mut limpios: Vec<(NaiveTime, Option<u32>)> = Vec::new();
        for p in puntos {
            if let Some(u) = limpios.last_mut().filter(|u| u.0 == p.0) {
                *u = p;
                continue;
            }
            if limpios.last().map(|u| valor_rclone(u.1)) != Some(valor_rclone(p.1)) {
                limpios.push(p);
            }
        }
        for (t, k) in limpios {
            partes.push(format!("{nombre}-{},{}", t.format("%H:%M"), valor_rclone(k)));
        }
    }
    Some(partes.join(" "))
}

/// En palabras: «2048 KiB/s de 08:00 a 18:00 (lun–vie)».
pub fn texto(franjas: &[Franja]) -> String {
    franjas
        .iter()
        .map(|f| {
            let lim = if f.kib == 0 { "sin límite".to_string() } else { format!("{} KiB/s", f.kib) };
            format!("{lim} de {} a {}", f.desde, f.hasta)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn oficina() -> Franja {
        Franja { dias: vec![1, 2, 3, 4, 5], desde: "08:00".into(), hasta: "18:00".into(), kib: 2048 }
    }

    #[test]
    fn franjas_validas() {
        validar(&[oficina()]).unwrap();
        validar(&[oficina(), Franja { dias: vec![6], desde: "09:00".into(), hasta: "13:00".into(), kib: 0 }]).unwrap();
        for mala in [
            vec![Franja { dias: vec![], ..oficina() }],
            vec![Franja { dias: vec![8], ..oficina() }],
            vec![Franja { desde: "18:00".into(), hasta: "08:00".into(), ..oficina() }],
            vec![Franja { desde: "8:00".into(), ..oficina() }],
            vec![Franja { kib: 10, ..oficina() }],
            vec![oficina(), Franja { dias: vec![5], desde: "17:00".into(), hasta: "20:00".into(), kib: 512 }],
            (0..9).map(|_| Franja { dias: vec![7], desde: "00:00".into(), hasta: "00:01".into(), kib: 0 }).collect(),
        ] {
            assert!(validar(&mala).is_err(), "{mala:?}");
        }
    }

    #[test]
    fn el_limite_de_la_hora_en_que_empieza() {
        // Lunes 5 de octubre de 2026.
        let lunes = |h: u32, m: u32| Local.with_ymd_and_hms(2026, 10, 5, h, m, 0).unwrap();
        let f = [oficina()];
        assert_eq!(limite_ahora(&f, None, lunes(10, 0)), Some(2048));
        assert_eq!(limite_ahora(&f, None, lunes(8, 0)), Some(2048), "desde la hora de empezar");
        assert_eq!(limite_ahora(&f, None, lunes(18, 0)), None, "hasta la de terminar");
        assert_eq!(limite_ahora(&f, Some(8192), lunes(22, 0)), Some(8192), "fuera, el de siempre");
        let sabado = Local.with_ymd_and_hms(2026, 10, 10, 10, 0, 0).unwrap();
        assert_eq!(limite_ahora(&f, None, sabado), None);
        // Una franja «sin límite» quita también el de siempre.
        let libre = [Franja { kib: 0, ..oficina() }];
        assert_eq!(limite_ahora(&libre, Some(512), lunes(10, 0)), None);
    }

    #[test]
    fn horario_de_rclone() {
        assert_eq!(bwlimit(&[], None), None);
        assert_eq!(bwlimit(&[], Some(512)).as_deref(), Some("512K"));
        let s = bwlimit(&[oficina()], None).unwrap();
        assert!(s.starts_with("Mon-00:00,off Mon-08:00,2048K Mon-18:00,off Tue-00:00,off"), "{s}");
        assert!(s.ends_with("Sat-00:00,off Sun-00:00,off"), "{s}");
        // Con un límite de siempre y una franja que empieza a medianoche.
        let s = bwlimit(&[Franja { dias: vec![7], desde: "00:00".into(), hasta: "06:00".into(), kib: 0 }], Some(1024)).unwrap();
        assert!(s.contains("Sun-00:00,off Sun-06:00,1024K"), "{s}");
        assert!(s.starts_with("Mon-00:00,1024K Tue-00:00,1024K"), "{s}");
    }
}
