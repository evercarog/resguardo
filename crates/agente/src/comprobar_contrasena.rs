//! 14bbb (plan 0.8.0, B4): «Comprobar la contraseña» de un repositorio.
//!
//! La consola manda la orden `comprobar_contrasena { repo }` con la contraseña escrita en
//! `autorizacion.clave_repo`, sellada solo para este equipo (como las demás órdenes que la
//! piden). Antes de llegar aquí, `servidor_v2::comprobar` la compara con la que guarda el
//! equipo y cuenta los fallos para el bloqueo por intentos (contra adivinar). Aquí se
//! prueba **de verdad**: el motor de copias abre el repositorio con ella (`cat config`,
//! sin bloquearlo ni cambiar nada) y también sus copias externas y derivadas que usan la
//! misma contraseña. Los espejos son el mismo repositorio (los mismos archivos): se abren
//! con la misma, lo dice la consola.
//!
//! No reduce la protección: sin espera. La contraseña no se guarda ni se escribe en ningún
//! registro: va al motor por su variable de entorno, como siempre. Solo queda la fecha y
//! el resultado (`privado/contrasenas-comprobadas.json`), que van en el resumen
//! (`repositorios[].contrasena_comprobada`) y en la salud de la protección.

use resguardo_motor::restic::{self, Access};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

const ARCHIVO: &str = "contrasenas-comprobadas.json";
/// Como mucho, copias externas o derivadas que se prueban (más no tiene un repositorio).
const MAX_COPIAS: usize = 16;

/// Lo que dijo el motor de copias al abrir un repositorio con la contraseña.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Abre {
    Si,
    /// La contraseña no abre el repositorio.
    No,
    /// No se pudo comprobar (sin red, destino apagado…): el motivo, sin rutas ni secretos.
    SinRespuesta(String),
}

/// Abre el repositorio sin bloquearlo ni cambiar nada.
pub fn probar(acc: &Access) -> Abre {
    match restic::run_raw(acc, &["cat", "config", "--no-lock"], restic::CHECK_TIMEOUT) {
        Ok(o) if o.code == Some(0) => Abre::Si,
        Ok(o) if o.code == Some(12) || o.stderr.contains("wrong password") || o.stderr.contains("no key found") => Abre::No,
        Ok(o) => Abre::SinRespuesta(crate::web::public_message(&restic::exit_error(o.code, &o.stderr))),
        Err(e) => Abre::SinRespuesta(crate::web::public_message(&e)),
    }
}

/// Una copia externa o derivada del repositorio.
pub struct Copia {
    pub nombre: String,
    /// `None`: usa otra contraseña (no se prueba aquí: se comprueba con su propio kit).
    pub acceso: Option<Result<Access, String>>,
}

/// Lo que se comprobó.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comprobacion {
    pub repo: Abre,
    /// (nombre, `None` si usa otra contraseña, o lo que dijo).
    pub copias: Vec<(String, Option<Abre>)>,
}

impl Comprobacion {
    /// La frase para la consola.
    pub fn mensaje(&self, nombre: &str) -> String {
        let mut m = match &self.repo {
            Abre::Si => format!("La contraseña abre «{nombre}»: tu kit sirve."),
            Abre::No => format!(
                "La contraseña es la que tiene guardada el equipo, pero no abre «{nombre}». Revisa el repositorio cuanto antes: las copias nuevas podrían estar fallando."
            ),
            Abre::SinRespuesta(e) => format!("Es la contraseña de «{nombre}», pero no se pudo abrir el repositorio para comprobarlo: {e}"),
        };
        let (mut abren, mut no, mut sin, mut otra) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for (n, r) in &self.copias {
            match r {
                Some(Abre::Si) => abren.push(format!("«{n}»")),
                Some(Abre::No) => no.push(format!("«{n}»")),
                Some(Abre::SinRespuesta(_)) => sin.push(format!("«{n}»")),
                None => otra.push(format!("«{n}»")),
            }
        }
        let lista = |v: &[String]| match v {
            [] => String::new(),
            [a] => a.clone(),
            _ => format!("{} y {}", v[..v.len() - 1].join(", "), v[v.len() - 1]),
        };
        if !abren.is_empty() {
            m.push_str(&format!(" También abre la copia en {}.", lista(&abren)));
        }
        if !no.is_empty() {
            m.push_str(&format!(" No abre la copia en {}, que debería usar la misma.", lista(&no)));
        }
        if !sin.is_empty() {
            m.push_str(&format!(" No se pudo llegar a la copia en {}.", lista(&sin)));
        }
        if !otra.is_empty() {
            m.push_str(&format!(" La copia en {} usa otra contraseña: compruébala con su kit.", lista(&otra)));
        }
        m
    }

    /// El detalle para la consola (sin rutas ni secretos).
    pub fn detalle(&self) -> Value {
        let txt = |a: &Abre| match a {
            Abre::Si => "abre",
            Abre::No => "no_abre",
            Abre::SinRespuesta(_) => "sin_respuesta",
        };
        json!({
            "resultado": txt(&self.repo),
            "copias": self.copias.iter().map(|(n, r)| json!({ "nombre": n, "resultado": r.as_ref().map(txt).unwrap_or("otra_contrasena") })).collect::<Vec<_>>(),
        })
    }

    /// ¿Termina bien la orden? Solo si el repositorio abre y ninguna copia con la misma dice que no.
    pub fn bien(&self) -> bool {
        self.repo == Abre::Si && !self.copias.iter().any(|(_, r)| r == &Some(Abre::No))
    }
}

/// Prueba el repositorio y sus copias con la misma contraseña.
pub fn comprobar(repo: &Access, copias: Vec<Copia>) -> Comprobacion {
    let r = probar(repo);
    let copias = copias
        .into_iter()
        .take(MAX_COPIAS)
        .map(|c| {
            let res = c.acceso.map(|a| match a {
                Ok(acc) => probar(&acc),
                Err(e) => Abre::SinRespuesta(crate::web::public_message(&e)),
            });
            (c.nombre, res)
        })
        .collect();
    Comprobacion { repo: r, copias }
}

// ---------------------------------------------------------------------------
// La fecha y el resultado (para el resumen y la salud de la protección)
// ---------------------------------------------------------------------------

/// Lo que queda de cada repositorio (por su id).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Anotada {
    /// RFC 3339 de la última comprobación que abrió el repositorio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bien: Option<String>,
    /// RFC 3339 y resultado de la última (`abre`, `no_abre`, `sin_respuesta`).
    pub ultima: String,
    pub resultado: String,
}

fn ruta() -> std::path::PathBuf {
    crate::agent::private_dir().join(ARCHIVO)
}

pub fn leer() -> HashMap<String, Anotada> {
    std::fs::read(ruta()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Anota la comprobación de `repo` (sin la contraseña: solo fecha y resultado).
pub fn anotar(repo: &str, c: &Comprobacion, ahora: &str) {
    static CERROJO: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _g = CERROJO.lock().unwrap_or_else(|e| e.into_inner());
    let mut todas = leer();
    let resultado = c.detalle()["resultado"].as_str().unwrap_or("").to_string();
    let a = todas.entry(repo.to_string()).or_default();
    a.ultima = ahora.to_string();
    if c.repo == Abre::Si {
        a.bien = Some(ahora.to_string());
    }
    a.resultado = resultado;
    if crate::agent::prepare_dir().is_ok() {
        if let Ok(t) = serde_json::to_vec(&todas) {
            let tmp = ruta().with_extension("tmp");
            if std::fs::write(&tmp, t).is_ok() {
                let _ = std::fs::rename(&tmp, ruta());
            }
        }
    }
}

/// `repositorios[].contrasena_comprobada` del resumen: `{ bien?, ultima, resultado }` o nada.
pub fn resumen(repo: &str, todas: &HashMap<String, Anotada>) -> Value {
    todas.get(repo).map(|a| json!(a)).unwrap_or(Value::Null)
}

// ---------------------------------------------------------------------------
// La orden
// ---------------------------------------------------------------------------

/// Las copias externas y derivadas del repositorio `repo_id` del agente, con su acceso si
/// usan la misma contraseña. `nombre_destino`: el nombre de un destino del equipo por su id.
pub fn copias_de(repo_id: &str, nombre_destino: &dyn Fn(&str) -> Option<String>) -> Vec<Copia> {
    let config = crate::agent::load_config();
    let Some(r) = config.repos.iter().find(|r| r.id == repo_id) else { return Vec::new() };
    let Some(secreto) = crate::agent::load_secrets().ok().and_then(|mut s| s.remove(repo_id)) else { return Vec::new() };
    let nombre = |o: &crate::tasks::Offsite, por_defecto: String| {
        o.provider.strip_prefix("destino:").and_then(nombre_destino).or_else(|| o.target_name.clone().filter(|n| !n.trim().is_empty())).unwrap_or(por_defecto)
    };
    let misma = |p: &Option<String>| p.as_deref().is_none_or(|x| x == secreto.password);
    let mut v = Vec::new();
    if let Some(o) = &r.offsite {
        v.push(Copia {
            nombre: nombre(o, "la copia externa".into()),
            acceso: misma(&secreto.offsite_password).then(|| crate::tasks::acceso_de_copia_externa(o, &secreto)),
        });
    }
    for d in &r.derived {
        let s = crate::tasks::derived_secret(&secreto, &d.id);
        v.push(Copia {
            nombre: nombre(&d.offsite, d.id.clone()),
            acceso: misma(&s.offsite_password).then(|| crate::tasks::acceso_de_copia_externa(&d.offsite, &s)),
        });
    }
    v
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn c(repo: Abre, copias: Vec<(&str, Option<Abre>)>) -> Comprobacion {
        Comprobacion { repo, copias: copias.into_iter().map(|(n, r)| (n.to_string(), r)).collect() }
    }

    #[test]
    fn mensajes() {
        let m = c(Abre::Si, vec![]).mensaje("Contabilidad");
        assert_eq!(m, "La contraseña abre «Contabilidad»: tu kit sirve.");
        let x = c(Abre::Si, vec![("Nube Oficina", Some(Abre::Si)), ("Disco E", Some(Abre::Si)), ("Otra", None)]);
        let m = x.mensaje("Contabilidad");
        assert!(m.contains("También abre la copia en «Nube Oficina» y «Disco E».") && m.contains("«Otra» usa otra contraseña"), "{m}");
        assert!(x.bien());
        let x = c(Abre::Si, vec![("Nube Oficina", Some(Abre::No))]);
        assert!(!x.bien() && x.mensaje("C").contains("No abre la copia en «Nube Oficina»"));
        let x = c(Abre::No, vec![]);
        assert!(!x.bien() && x.mensaje("C").contains("no abre «C»"));
        let x = c(Abre::SinRespuesta("No se pudo conectar.".into()), vec![("B2", Some(Abre::SinRespuesta("x".into())))]);
        assert!(!x.bien() && x.mensaje("C").contains("No se pudo conectar.") && x.mensaje("C").contains("No se pudo llegar a la copia en «B2»"));
        assert_eq!(x.detalle(), json!({ "resultado": "sin_respuesta", "copias": [{ "nombre": "B2", "resultado": "sin_respuesta" }] }));
        // Nunca la palabra del motor.
        for x in [c(Abre::Si, vec![]), c(Abre::No, vec![]), c(Abre::SinRespuesta("y".into()), vec![])] {
            assert!(!x.mensaje("C").to_lowercase().contains("restic"));
        }
    }

    #[test]
    fn anota_fecha_y_resultado_sin_la_contrasena() {
        let _g = crate::restic::tests::real_repo_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("RESGUARDO_AGENT_DIR", dir.path());
        anotar("r1", &c(Abre::Si, vec![]), "2026-10-13T10:00:00+00:00");
        anotar("r1", &c(Abre::SinRespuesta("x".into()), vec![]), "2026-10-14T10:00:00+00:00");
        let t = leer();
        let a = &t["r1"];
        assert_eq!(a.bien.as_deref(), Some("2026-10-13T10:00:00+00:00"));
        assert_eq!((a.ultima.as_str(), a.resultado.as_str()), ("2026-10-14T10:00:00+00:00", "sin_respuesta"));
        assert_eq!(resumen("r1", &t)["bien"], "2026-10-13T10:00:00+00:00");
        assert!(resumen("otro", &t).is_null());
        std::env::remove_var("RESGUARDO_AGENT_DIR");
    }
}
