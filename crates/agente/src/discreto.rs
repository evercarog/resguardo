//! «Modo discreto»: mientras se trabaja (unos días y horas), lo que hace el
//! agente con restic (copias, subidas a la nube, verificaciones y pruebas de
//! restauración) va con prioridad baja para no molestar: CPU por debajo de lo
//! normal, E/S de disco baja y memoria de prioridad baja. Opcionalmente, con la
//! subida a destinos remotos limitada.
//!
//! El horario está en `agent.json` (solo lo cambia un administrador). El agente
//! lo mira antes de empezar cada copia o tarea (`restic::set_discreet`); lo que
//! hace la app a mano («Copiar ahora», restaurar) no cambia.

use chrono::{DateTime, Datelike, Duration, Local, NaiveTime, Timelike};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Discreet {
    /// Días de la semana: 0 = lunes … 6 = domingo.
    pub days: Vec<u8>,
    /// «07:00».
    pub from: String,
    /// «19:00». Si es anterior a `from`, el horario pasa la medianoche.
    pub to: String,
    /// Límite de subida a destinos remotos mientras dura, en KiB/s (None: sin límite).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upload_kib: Option<u32>,
}

impl Default for Discreet {
    /// Lunes a sábado, de 07:00 a 19:00, sin límite de subida.
    fn default() -> Self {
        Discreet { days: vec![0, 1, 2, 3, 4, 5], from: "07:00".into(), to: "19:00".into(), upload_kib: None }
    }
}

fn parse(t: &str) -> Result<NaiveTime, String> {
    NaiveTime::parse_from_str(t.trim(), "%H:%M").map_err(|_| format!("Hora no válida: «{t}» (usa HH:MM, p. ej. 07:00)."))
}

/// Límites del límite de subida (KiB/s): de 64 KiB/s a 1 GiB/s.
pub const MIN_UPLOAD_KIB: u32 = 64;
pub const MAX_UPLOAD_KIB: u32 = 1024 * 1024;

impl Discreet {
    pub fn validate(&self) -> Result<(), String> {
        if self.days.is_empty() {
            return Err("Elige al menos un día.".into());
        }
        if self.days.iter().any(|d| *d > 6) {
            return Err("Día no válido.".into());
        }
        if parse(&self.from)? == parse(&self.to)? {
            return Err("La hora de inicio y la de fin no pueden ser la misma.".into());
        }
        if let Some(k) = self.upload_kib {
            if !(MIN_UPLOAD_KIB..=MAX_UPLOAD_KIB).contains(&k) {
                return Err(format!("El límite de subida debe estar entre {MIN_UPLOAD_KIB} KB/s y 1 GB/s."));
            }
        }
        Ok(())
    }

    /// ¿Estamos dentro del horario?
    pub fn active(&self, now: DateTime<Local>) -> bool {
        let (Ok(from), Ok(to)) = (parse(&self.from), parse(&self.to)) else { return false };
        let t = NaiveTime::from_hms_opt(now.hour(), now.minute(), now.second()).unwrap_or_default();
        let day = |d: DateTime<Local>| self.days.contains(&(d.weekday().num_days_from_monday() as u8));
        if from < to {
            day(now) && t >= from && t < to
        } else {
            // Pasa la medianoche (p. ej. 22:00–06:00): cuenta el día en que empieza.
            (day(now) && t >= from) || (day(now - Duration::days(1)) && t < to)
        }
    }
}

// Destinos remotos y prioridad baja de los procesos: en el motor (los usa restic).
#[cfg(test)]
use resguardo_motor::proceso::{after_spawn, is_remote, lower};
#[cfg(all(windows, test))]
use resguardo_motor::proceso::{priorities, IO_PRIORITY_LOW};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
    }

    #[test]
    fn horario_por_defecto() {
        let d = Discreet::default();
        d.validate().unwrap();
        // 2026-10-01 es jueves; 2026-10-04, domingo.
        assert!(d.active(at(2026, 10, 1, 7, 0)));
        assert!(d.active(at(2026, 10, 1, 18, 59)));
        assert!(!d.active(at(2026, 10, 1, 19, 0)));
        assert!(!d.active(at(2026, 10, 1, 6, 59)));
        assert!(d.active(at(2026, 10, 3, 12, 0)), "sábado");
        assert!(!d.active(at(2026, 10, 4, 12, 0)), "domingo");
    }

    #[test]
    fn horario_que_pasa_la_medianoche() {
        let d = Discreet { days: vec![4], from: "22:00".into(), to: "06:00".into(), upload_kib: None };
        // Viernes 2 de octubre a las 23:00 y el sábado a las 05:00: sí. El sábado a las 23:00: no.
        assert!(d.active(at(2026, 10, 2, 23, 0)));
        assert!(d.active(at(2026, 10, 3, 5, 0)));
        assert!(!d.active(at(2026, 10, 3, 23, 0)));
        assert!(!d.active(at(2026, 10, 2, 5, 0)));
    }

    #[test]
    fn validacion() {
        let ok = Discreet::default();
        assert!(Discreet { days: vec![], ..ok.clone() }.validate().is_err());
        assert!(Discreet { days: vec![7], ..ok.clone() }.validate().is_err());
        assert!(Discreet { to: "07:00".into(), ..ok.clone() }.validate().is_err());
        assert!(Discreet { from: "7".into(), ..ok.clone() }.validate().is_err());
        assert!(Discreet { upload_kib: Some(10), ..ok.clone() }.validate().is_err());
        assert!(Discreet { upload_kib: Some(2048), ..ok }.validate().is_ok());
    }

    #[test]
    fn destinos_remotos() {
        assert!(is_remote("rest:https://servidor:8000/ana/"));
        assert!(is_remote("s3:s3.amazonaws.com/bucket"));
        assert!(is_remote("b2:bucket:/ruta"));
        assert!(is_remote("sftp:ana@nas:/copias"));
        assert!(!is_remote(r"D:\Copias"));
        assert!(!is_remote(r"\\nas\copias"));
        assert!(!is_remote("/mnt/copias"));
    }

    /// Lo que se mide de verdad: un proceso lanzado así queda con CPU por
    /// debajo de lo normal, E/S baja y memoria de prioridad baja.
    #[test]
    #[cfg(windows)]
    fn el_proceso_queda_con_prioridad_baja() {
        use windows_sys::Win32::System::Threading::{BELOW_NORMAL_PRIORITY_CLASS, MEMORY_PRIORITY_LOW, NORMAL_PRIORITY_CLASS};
        let ping = |low: bool| {
            let mut cmd = std::process::Command::new(crate::platform::system_tool("ping.exe"));
            cmd.args(["-n", "3", "127.0.0.1"]).stdout(std::process::Stdio::null());
            if low {
                lower(&mut cmd);
            }
            let child = cmd.spawn().unwrap();
            if low {
                after_spawn(&child);
            }
            child
        };
        let mut normal = ping(false);
        let mut low = ping(true);
        let (cpu, io, mem) = priorities(&low);
        let (ncpu, nio, _) = priorities(&normal);
        for c in [&mut low, &mut normal] {
            let _ = c.kill();
            let _ = c.wait();
        }
        // La clase de CPU la pone CreateProcess: se comprueba siempre.
        assert_eq!(cpu, BELOW_NORMAL_PRIORITY_CLASS);
        // La E/S y la memoria se cambian después (NtSetInformationProcess). En
        // algunos entornos (p. ej. los ejecutores de GitHub Actions, donde la
        // propia prueba ya corre con prioridad baja o sin ese permiso) no se
        // pueden cambiar o no hay diferencia con un proceso normal: ahí se
        // omite, salvo con RESGUARDO_PRUEBA_PRIORIDAD_ESTRICTA (en un equipo
        // normal sí se cumple, y así se comprobó al desarrollarlo).
        let padre_normal = unsafe {
            use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetPriorityClass};
            GetPriorityClass(GetCurrentProcess()) == NORMAL_PRIORITY_CLASS
        };
        let estricta = std::env::var("RESGUARDO_PRUEBA_PRIORIDAD_ESTRICTA").is_ok();
        if !estricta && (!padre_normal || nio != 2) {
            eprintln!("omitido en parte: este entorno no ejecuta los procesos con prioridad normal (padre normal: {padre_normal}, E/S normal: {nio})");
            return;
        }
        assert_eq!(io, IO_PRIORITY_LOW);
        assert_eq!(mem, MEMORY_PRIORITY_LOW);
        assert_eq!(ncpu, NORMAL_PRIORITY_CLASS);
        assert_eq!(nio, 2, "E/S normal");
    }
}
