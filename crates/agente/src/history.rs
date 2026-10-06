//! Historial de actividad: una línea JSON por cada copia, verificación o
//! subida externa, con su resultado. Lo escriben el agente (en su carpeta,
//! legible por los usuarios del equipo, como el registro) y la app (copias
//! manuales, en su carpeta de configuración). La app muestra los dos juntos.
//!
//! La app anota también los cambios de configuración de cada destino
//! («config»: quién cambió qué y cuándo, sin secretos) y los kits de
//! recuperación guardados («kit»).

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Tamaño a partir del cual el archivo se rota (se conserva el anterior).
const MAX_BYTES: u64 = 2_000_000;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Entry {
    /// "backup", "verify", "offsite", "pause" (copias automáticas en pausa),
    /// "resume" (reanudadas), "config" (un cambio de configuración) o "kit"
    /// (kit de recuperación guardado).
    pub kind: String,
    /// "agent" (programada o pedida al agente, o el final de una pausa), "retry"
    /// (reintento) o "manual".
    pub origin: String,
    pub repo_id: String,
    pub repo_name: String,
    #[serde(default)]
    pub plan_id: Option<String>,
    #[serde(default)]
    pub plan_name: Option<String>,
    pub started: String,
    pub finished: String,
    /// "ok", "warning", "error" o "info" (pausas: no es un resultado).
    pub result: String,
    pub message: String,
    #[serde(default)]
    pub snapshot_id: Option<String>,
    #[serde(default)]
    pub data_added: Option<u64>,
    #[serde(default)]
    pub files_new: Option<u64>,
    #[serde(default)]
    pub files_changed: Option<u64>,
    /// Copia correcta sin cambios: con «Solo guardar si hay cambios», restic
    /// no creó una versión nueva (result "ok", sin `snapshot_id`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unchanged: bool,
    /// Copia pedida a distancia: desde qué equipo (o «la web»).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_from: Option<String>,
    /// Usuario de Windows que hizo el cambio («config» y «kit»).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// Resultado de los ganchos de la copia («backup»), si tiene.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ganchos: Vec<crate::ganchos::ResultadoGancho>,
}

/// Usuario de Windows que usa la app (para «quién cambió qué»).
pub fn local_user() -> Option<String> {
    std::env::var("USERNAME").or_else(|_| std::env::var("USER")).ok().map(|u| u.trim().to_string()).filter(|u| !u.is_empty())
}

/// Anota un cambio de configuración («config») o un kit guardado («kit») en
/// el historial de la app. `message` es un resumen sin secretos.
pub fn note(config_dir: &Path, kind: &str, repo_id: &str, repo_name: &str, plan: Option<(&str, &str)>, message: impl Into<String>) {
    let now = chrono::Local::now().to_rfc3339();
    append(
        &app_file(config_dir),
        &Entry {
            kind: kind.into(),
            origin: "manual".into(),
            repo_id: repo_id.into(),
            repo_name: repo_name.into(),
            plan_id: plan.map(|p| p.0.to_string()),
            plan_name: plan.map(|p| p.1.to_string()),
            started: now.clone(),
            finished: now,
            result: "info".into(),
            message: message.into(),
            user: local_user(),
            ..Default::default()
        },
    );
}

/// Qué cambió en las copias de un destino: (id, nombre, resumen) por copia
/// creada, eliminada o cambiada. Solo nombres de campos, nunca su contenido.
pub fn plan_changes(old: &[crate::plans::Plan], new: &[crate::plans::Plan]) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for p in new {
        match old.iter().find(|o| o.id == p.id) {
            None => out.push((p.id.clone(), p.name.clone(), format!("Copia «{}» creada.", p.name))),
            Some(o) if o != p => {
                let mut what = Vec::new();
                if o.name != p.name {
                    what.push(format!("nombre («{}» → «{}»)", o.name, p.name));
                }
                if o.paths != p.paths {
                    what.push("carpetas".to_string());
                }
                if o.excludes != p.excludes {
                    what.push("exclusiones".to_string());
                }
                if o.tags != p.tags {
                    what.push("etiquetas".to_string());
                }
                if o.schedule != p.schedule {
                    what.push(match (&o.schedule, &p.schedule) {
                        (None, Some(_)) => "horario (ahora se hace sola)".to_string(),
                        (Some(_), None) => "horario (ahora solo a mano)".to_string(),
                        _ => "horario".to_string(),
                    });
                }
                if o.skip_unchanged != p.skip_unchanged {
                    what.push("«solo guardar si hay cambios»".to_string());
                }
                out.push((p.id.clone(), p.name.clone(), format!("Copia «{}» cambiada: {}.", p.name, what.join(", "))));
            }
            Some(_) => {}
        }
    }
    for o in old.iter().filter(|o| !new.iter().any(|p| p.id == o.id)) {
        out.push((o.id.clone(), o.name.clone(), format!("Copia «{}» eliminada.", o.name)));
    }
    out
}

pub fn agent_file() -> PathBuf {
    crate::agent::agent_dir().join("historial.jsonl")
}

pub fn app_file(config_dir: &Path) -> PathBuf {
    config_dir.join("historial.jsonl")
}

/// Añade una entrada (sin fallar nunca la operación que la origina).
pub fn append(path: &Path, entry: &Entry) {
    // El historial del agente lo leen todos los usuarios del equipo: sin rutas
    // ni contraseñas en los mensajes (el de la app es solo del usuario).
    let sanitized;
    let entry = if path.starts_with(crate::agent::agent_dir()) {
        sanitized = Entry { message: crate::web::local_message(&entry.message), ..entry.clone() };
        &sanitized
    } else {
        entry
    };
    let Ok(line) = serde_json::to_string(entry) else { return };
    if fs::metadata(path).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = fs::rename(path, path.with_extension("old.jsonl"));
    }
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{line}");
    }
    // Lo que cuenta a la consola, también en la bitácora del equipo (para una consola nueva).
    if path == agent_file() {
        crate::bitacora::desde_historial(entry);
    }
}

/// Entradas de un archivo (y del rotado), de la más antigua a la más reciente.
pub fn read(path: &Path) -> Vec<Entry> {
    let text = |p: &Path| fs::read(p).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let all = text(&path.with_extension("old.jsonl")) + &text(path);
    all.lines().filter_map(|l| serde_json::from_str::<Entry>(l).ok()).collect()
}

/// Historial combinado (agente + app), de lo más reciente a lo más antiguo.
pub fn combined(config_dir: &Path, limit: usize) -> Vec<Entry> {
    let mut all = read(&agent_file());
    all.extend(read(&app_file(config_dir)));
    all.sort_by(|a, b| b.finished.cmp(&a.finished));
    all.truncate(limit);
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anota_lee_y_rota() {
        let dir = std::env::temp_dir().join(format!("resguardo-hist-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let file = dir.join("historial.jsonl");
        for i in 0..3 {
            append(
                &file,
                &Entry {
                    kind: "backup".into(),
                    origin: "manual".into(),
                    repo_id: "r".into(),
                    repo_name: "Siigo".into(),
                    finished: format!("2026-09-29T1{i}:00:00-05:00"),
                    result: "ok".into(),
                    ..Default::default()
                },
            );
        }
        fs::write(&file, fs::read_to_string(&file).unwrap() + "línea rota\n").unwrap();
        let got = read(&file);
        assert_eq!(got.len(), 3, "las líneas dañadas se ignoran");
        assert_eq!(got[2].finished, "2026-09-29T12:00:00-05:00");
        let _ = fs::remove_dir_all(dir);
    }

    fn plan(id: &str, name: &str) -> crate::plans::Plan {
        crate::plans::Plan {
            id: id.into(),
            name: name.into(),
            paths: vec![r"C:\Datos".into()],
            excludes: vec![],
            tags: vec![],
            schedule: None,
            skip_unchanged: true,
            ganchos: vec![],
            after: None,
        }
    }

    #[test]
    fn resume_los_cambios_de_las_copias() {
        let old = vec![plan("a", "Laboral"), plan("b", "Domingo")];
        let mut changed = plan("a", "Oficina");
        changed.excludes = vec!["*.tmp".into()];
        let new = vec![changed, plan("c", "Fotos")];
        let got = plan_changes(&old, &new);
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].2, "Copia «Oficina» cambiada: nombre («Laboral» → «Oficina»), exclusiones.");
        assert_eq!(got[1].2, "Copia «Fotos» creada.");
        assert_eq!(got[2].2, "Copia «Domingo» eliminada.");
        assert!(plan_changes(&old, &old).is_empty(), "sin cambios, nada que anotar");
        // Nunca el contenido: ni rutas ni patrones.
        assert!(!got.iter().any(|c| c.2.contains("Datos") || c.2.contains("*.tmp")));
    }

    #[test]
    fn anota_quien_cambia_la_configuracion() {
        let dir = std::env::temp_dir().join(format!("resguardo-hist-cfg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        note(&dir, "config", "r1", "Siigo", Some(("p", "Laboral")), "Copia «Laboral» creada.");
        let got = read(&app_file(&dir));
        assert_eq!(got.len(), 1);
        assert_eq!((got[0].kind.as_str(), got[0].result.as_str()), ("config", "info"));
        assert_eq!(got[0].plan_name.as_deref(), Some("Laboral"));
        assert_eq!(got[0].user, local_user());
        let _ = fs::remove_dir_all(dir);
    }
}
