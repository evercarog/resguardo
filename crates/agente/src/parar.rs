//! 15hh: parar (o pausar) lo que está en marcha: una copia, una tarea larga (verificación,
//! copia derivada, prueba de restauración), un espejo o una restauración. Siempre con la
//! clave de administración (consola, ventana del equipo o `resguardo-agente parar`).
//!
//! Lo que está en marcha se reconoce por su **clave**, la misma que va en su progreso
//! (`parar`): `copia:<repo>:<copia>`, `tarea:<repo>:<pedida>` (`verify`, `derivada-<id>`…),
//! `espejo:<trabajo>`, `nube:<trabajo>` (los del equipo, `…:eq-<trabajo>`) y
//! `restauracion:<repo>`. Pedir que pare deja un archivo en la carpeta privada del agente
//! (solo administradores y el servicio) y, si lo hace este mismo proceso, pone su marca.
//! Quien lo hace lo vigila ([`vigilar`]) y para restic o rclone con esa marca
//! (`restic::con_cancelacion`): nada se congela (las conexiones caducarían y el repositorio
//! quedaría bloqueado), se para y, si es una pausa, sigue sola a su hora.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// `admite`: la orden `parar { clave, pausa? }` y `parar` en el progreso (15hh).
pub const ADMITE: &str = "parar";

/// Lo que se dice cuando restic o rclone se paran por una petición.
pub const PARADO: &str = "Parada a petición.";

/// El resultado de una vuelta de espejo parada a petición empieza así (como «ERROR:» las
/// fallidas): no cuenta como bien ni como fallida.
pub const PREFIJO_PARADA: &str = "PARADA:";

/// Una pausa no puede durar más que esto (luego sigue sola igual).
pub const MAX_PAUSA_DIAS: i64 = 7;

/// Una petición de parar o pausar.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Peticion {
    pub clave: String,
    /// Quién la pidió: lo que dice su consola, o la cuenta del equipo («ana (PC-01)»).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub por: Option<String>,
    /// El nombre de la consola en el equipo (nunca su dirección); `None`: en el propio equipo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consola: Option<String>,
    /// Pausar: sigue sola a esta hora (RFC 3339).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hasta: Option<String>,
    /// Pausar «a mano»: no sigue sola (hasta «Copiar ahora»).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub a_mano: bool,
    /// Cuándo se pidió (RFC 3339).
    pub cuando: String,
}

impl Peticion {
    /// ¿Es una pausa (sigue sola a una hora, o a mano)?
    pub fn pausa(&self) -> bool {
        self.hasta.is_some() || self.a_mano
    }

    /// «por Ana desde la consola «Oficina»» / «en el equipo por ana (PC-01)».
    pub fn quien(&self) -> String {
        let por = self.por.as_deref().map(|p| format!(" por {p}")).unwrap_or_default();
        match self.consola.as_deref() {
            Some(c) => format!("{por} desde la consola «{c}»").trim_start().to_string(),
            None => format!("en el equipo{por}"),
        }
    }

    /// Lo que se anota en el historial y en el registro.
    pub fn texto(&self) -> String {
        if self.a_mano {
            return format!("En pausa hasta reanudarla (pausada {}). Sigue con «Copiar ahora».", self.quien());
        }
        match self.hasta.as_deref().and_then(|h| chrono::DateTime::parse_from_rfc3339(h).ok()) {
            Some(h) => {
                let h = h.with_timezone(&chrono::Local);
                let dia = if h.date_naive() == chrono::Local::now().date_naive() { String::new() } else { format!("el {} ", h.format("%d/%m")) };
                format!("En pausa hasta {dia}las {} (pausada {}). Sigue sola entonces.", h.format("%H:%M"), self.quien())
            }
            None => format!("Parada {}.", self.quien()),
        }
    }
}

/// Las claves que se aceptan: `<tipo>:<lo suyo>`, sin nada raro (van a un nombre de archivo).
pub fn clave_valida(c: &str) -> bool {
    let Some((tipo, resto)) = c.split_once(':') else { return false };
    matches!(tipo, "copia" | "tarea" | "espejo" | "nube" | "restauracion")
        && !resto.is_empty()
        && c.len() <= 200
        && resto.chars().all(|x| x.is_ascii_alphanumeric() || matches!(x, '-' | '_' | ':' | '.'))
}

/// ¿Se puede pausar (seguir sola a una hora)? Por ahora, las copias.
pub fn se_puede_pausar(clave: &str) -> bool {
    clave.starts_with("copia:")
}

fn carpeta() -> std::path::PathBuf {
    crate::agent::private_dir().join("parar")
}

fn ruta(clave: &str) -> std::path::PathBuf {
    let nombre: String = clave.bytes().map(|b| format!("{b:02x}")).collect();
    carpeta().join(format!("{nombre}.json"))
}

/// Las marcas de lo que está en marcha en este proceso (el servicio: espejos y restauraciones).
static EN_PROCESO: Mutex<Vec<(String, Arc<AtomicBool>)>> = Mutex::new(Vec::new());

/// Pide que pare (o se pause) lo de `p.clave`: quien lo hace lo verá en un segundo.
pub fn pedir(p: &Peticion) -> Result<(), String> {
    if !clave_valida(&p.clave) {
        return Err("No se sabe qué parar.".into());
    }
    std::fs::create_dir_all(carpeta()).map_err(|e| format!("No se pudo pedir que pare: {e}"))?;
    let tmp = ruta(&p.clave).with_extension("tmp");
    let datos = serde_json::to_vec(p).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, datos).and_then(|_| std::fs::rename(&tmp, ruta(&p.clave))).map_err(|e| format!("No se pudo pedir que pare: {e}"))?;
    if let Ok(l) = EN_PROCESO.lock() {
        for (_, f) in l.iter().filter(|(c, _)| *c == p.clave) {
            f.store(true, Ordering::Relaxed);
        }
    }
    Ok(())
}

/// La petición pendiente de `clave`, si hay.
pub fn pendiente(clave: &str) -> Option<Peticion> {
    std::fs::read(ruta(clave)).ok().and_then(|b| serde_json::from_slice(&b).ok())
}

/// Mientras vive, lo de `clave` se puede parar: su marca se pone en cuanto llega una
/// petición (de este proceso o de otro). Al soltarlo, la petición se borra.
pub struct Vigilancia {
    clave: String,
    marca: Arc<AtomicBool>,
    fin: Arc<AtomicBool>,
}

/// Empieza a vigilar `clave` (una petición vieja de antes se descarta).
pub fn vigilar(clave: &str) -> Vigilancia {
    let _ = std::fs::remove_file(ruta(clave));
    let marca = Arc::new(AtomicBool::new(false));
    let fin = Arc::new(AtomicBool::new(false));
    if let Ok(mut l) = EN_PROCESO.lock() {
        l.push((clave.to_string(), marca.clone()));
    }
    let (m, f, r) = (marca.clone(), fin.clone(), ruta(clave));
    std::thread::spawn(move || {
        while !f.load(Ordering::Relaxed) {
            if r.is_file() {
                m.store(true, Ordering::Relaxed);
                return;
            }
            std::thread::sleep(Duration::from_millis(700));
        }
    });
    Vigilancia { clave: clave.to_string(), marca, fin }
}

impl Vigilancia {
    /// La marca para `restic::con_cancelacion`.
    pub fn marca(&self) -> &Arc<AtomicBool> {
        &self.marca
    }

    /// ¿Se pidió parar? Con la petición (quién y si es una pausa).
    pub fn peticion(&self) -> Option<Peticion> {
        if !self.marca.load(Ordering::Relaxed) {
            return None;
        }
        Some(pendiente(&self.clave).unwrap_or_else(|| Peticion { clave: self.clave.clone(), cuando: chrono::Local::now().to_rfc3339(), ..Default::default() }))
    }
}

impl Drop for Vigilancia {
    fn drop(&mut self) {
        self.fin.store(true, Ordering::Relaxed);
        if let Ok(mut l) = EN_PROCESO.lock() {
            l.retain(|(_, f)| !Arc::ptr_eq(f, &self.marca));
        }
        let _ = std::fs::remove_file(ruta(&self.clave));
    }
}

/// `parar { clave, pausa?: { hasta } | { a_mano: true } }` (15hh), de una consola, de la
/// ventana o de la terminal: comprueba que eso está en marcha y pide que pare. Devuelve lo
/// que se dice (sin esperar a que pare: lo hace en unos segundos y queda en el historial).
pub fn orden(c: &serde_json::Value, por: Option<&str>, consola: Option<&str>) -> Result<String, String> {
    let clave = c["clave"].as_str().unwrap_or("").to_string();
    if !clave_valida(&clave) {
        return Err("No se sabe qué parar.".into());
    }
    let ahora = chrono::Local::now();
    let (hasta, a_mano) = match c.get("pausa").filter(|p| !p.is_null()) {
        None => (None, false),
        Some(_) if !se_puede_pausar(&clave) => return Err("Esto se puede parar, pero no pausar.".into()),
        Some(p) if p["a_mano"] == true => (None, true),
        Some(p) => {
            let h = p["hasta"]
                .as_str()
                .and_then(|h| chrono::DateTime::parse_from_rfc3339(h).ok())
                .ok_or("Di hasta cuándo pausarla.")?
                .with_timezone(&chrono::Local);
            if h <= ahora + chrono::Duration::minutes(1) {
                return Err("La hora de seguir ya pasó: elige una más tarde.".into());
            }
            if h > ahora + chrono::Duration::days(MAX_PAUSA_DIAS) {
                return Err(format!("Una pausa dura como mucho {MAX_PAUSA_DIAS} días."));
            }
            (Some(h.to_rfc3339()), false)
        }
    };
    let tareas = crate::progreso_v2::tareas(None);
    let t = tareas
        .iter()
        .find(|t| t["parar"].as_str() == Some(clave.as_str()) && t["fase"] != "en_cola")
        .ok_or("Ya no está en marcha: terminó o aún no ha empezado.")?;
    let nombre = t["nombre"].as_str().map(|n| format!(" «{n}»")).unwrap_or_default();
    let limpio = |s: &str, n: usize| -> String { s.chars().filter(|c| !c.is_control()).take(n).collect() };
    let p = Peticion {
        clave,
        por: por.map(|p| limpio(p, 80)).filter(|p| !p.trim().is_empty()),
        consola: consola.map(|c| limpio(c, 60)).filter(|c| !c.trim().is_empty()),
        hasta,
        a_mano,
        cuando: ahora.to_rfc3339(),
    };
    pedir(&p)?;
    crate::agent::log(&format!("{}{nombre}: {} pedido {}.", if p.pausa() { "Pausar" } else { "Parar" }, if p.pausa() { "pausa" } else { "parada" }, p.quien()));
    Ok(if p.pausa() {
        format!("Pausando{nombre}: se para en unos segundos y sigue sola a su hora.")
    } else {
        format!("Parando{nombre}: se para en unos segundos.")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claves() {
        for bien in ["copia:r1:k1", "tarea:r1:derivada-d1", "espejo:nube-b2", "nube:eq-x", "restauracion:repo_1"] {
            assert!(clave_valida(bien), "{bien}");
        }
        for mal in ["", "copia", "copia:", "otra:x", "copia:../x", "copia:a b", "copia:a/b", &format!("copia:{}", "x".repeat(300))] {
            assert!(!clave_valida(mal), "{mal}");
        }
        assert!(se_puede_pausar("copia:r1:k1") && !se_puede_pausar("espejo:x"));
    }

    #[test]
    fn textos() {
        let p = Peticion { clave: "copia:r:k".into(), por: Some("Ana".into()), consola: Some("Oficina".into()), cuando: String::new(), ..Default::default() };
        assert_eq!(p.texto(), "Parada por Ana desde la consola «Oficina».");
        let e = Peticion { por: Some("ana (PC-01)".into()), consola: None, ..p.clone() };
        assert_eq!(e.texto(), "Parada en el equipo por ana (PC-01).");
        let m = Peticion { a_mano: true, ..p.clone() };
        assert!(m.pausa() && m.texto().starts_with("En pausa hasta reanudarla"), "{}", m.texto());
        let h = (chrono::Local::now() + chrono::Duration::hours(2)).to_rfc3339();
        let t = Peticion { hasta: Some(h), ..p };
        assert!(t.pausa() && t.texto().contains("Sigue sola entonces"), "{}", t.texto());
    }

    #[test]
    fn la_marca_se_pone_al_pedirlo_y_se_borra_al_soltarla() {
        let _g = crate::restic::tests::real_repo_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("RESGUARDO_AGENT_DIR", dir.path());
        let v = vigilar("copia:r1:k1");
        assert!(v.peticion().is_none());
        pedir(&Peticion { clave: "copia:r1:k1".into(), por: Some("Ana".into()), cuando: "x".into(), ..Default::default() }).unwrap();
        // En este mismo proceso, al momento.
        let p = v.peticion().expect("pedida");
        assert_eq!(p.por.as_deref(), Some("Ana"));
        drop(v);
        assert!(pendiente("copia:r1:k1").is_none(), "al soltarla, la petición se borra");
        // Otra vigilancia de lo mismo no ve una petición vieja.
        pedir(&Peticion { clave: "copia:r1:k1".into(), cuando: "x".into(), ..Default::default() }).unwrap();
        let v2 = vigilar("copia:r1:k1");
        assert!(v2.peticion().is_none());
        drop(v2);
        std::env::remove_var("RESGUARDO_AGENT_DIR");
    }
}
