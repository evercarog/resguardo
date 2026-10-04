//! Qué cambió entre dos snapshots (`restic diff --json`).

use crate::restic::{self, Access};
use serde::{Deserialize, Serialize};

/// Máximo de cambios que se envían a la interfaz (el resumen cuenta todos).
const MAX_CHANGES: usize = 5000;

#[derive(Serialize)]
pub struct Change {
    pub path: String,
    /// "added", "removed", "modified", "metadata", "type" u "other".
    pub kind: &'static str,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Side {
    #[serde(default)]
    pub files: u64,
    #[serde(default)]
    pub dirs: u64,
    #[serde(default)]
    pub bytes: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Stats {
    #[serde(default)]
    pub changed_files: u64,
    #[serde(default)]
    pub added: Side,
    #[serde(default)]
    pub removed: Side,
}

#[derive(Serialize)]
pub struct Diff {
    pub changes: Vec<Change>,
    /// Cambios totales (puede ser mayor que `changes.len()` si se recortó).
    pub total: usize,
    pub stats: Stats,
}

/// Modificador de restic → tipo de cambio. `M` = contenido, `U` = metadatos,
/// `T` = cambió de tipo (p. ej. archivo → enlace). Pueden venir combinados.
fn kind(modifier: &str) -> &'static str {
    match modifier {
        "+" => "added",
        "-" => "removed",
        m if m.contains('M') => "modified",
        m if m.contains('T') => "type",
        m if m.contains('U') => "metadata",
        _ => "other",
    }
}

pub fn diff(access: &Access, from: &str, to: &str) -> Result<Diff, String> {
    if !restic::valid_snapshot_id(from) || !restic::valid_snapshot_id(to) {
        return Err("ID de versión no válido.".into());
    }
    let out = restic::run_with(access, &["diff", "--json", "--no-lock", from, to], restic::STATS_TIMEOUT, None)?;

    let mut changes = Vec::new();
    let mut total = 0;
    let mut stats = Stats::default();
    for line in String::from_utf8_lossy(&out).lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        match v["message_type"].as_str() {
            Some("change") => {
                total += 1;
                if changes.len() < MAX_CHANGES {
                    changes.push(Change { path: v["path"].as_str().unwrap_or_default().to_string(), kind: kind(v["modifier"].as_str().unwrap_or_default()) });
                }
            }
            Some("statistics") => stats = serde_json::from_value(v).unwrap_or_default(),
            _ => {}
        }
    }
    Ok(Diff { changes, total, stats })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tipos_de_cambio() {
        assert_eq!(kind("+"), "added");
        assert_eq!(kind("-"), "removed");
        assert_eq!(kind("M"), "modified");
        assert_eq!(kind("MU"), "modified");
        assert_eq!(kind("U"), "metadata");
        assert_eq!(kind("T"), "type");
    }

    #[test]
    fn diferencias_reales() {
        let (Ok(repo), Ok(pw)) = (std::env::var("RESGUARDO_TEST_REPO"), std::env::var("RESGUARDO_TEST_PASSWORD")) else {
            return;
        };
        let access = Access::new(repo, pw);
        let mut snaps = restic::snapshots(&access).unwrap();
        snaps.sort_by(|a, b| a.time.cmp(&b.time));
        let (first, last) = (&snaps[0], snaps.last().unwrap());
        let d = diff(&access, &first.short_id, &last.short_id).unwrap();
        // El repositorio de prueba debe tener cambios entre la primera copia y la última.
        assert!(!d.changes.is_empty(), "sin cambios entre la primera y la última copia");
        let known = ["added", "removed", "modified", "metadata", "type"];
        assert!(d.changes.iter().all(|c| known.contains(&c.kind) && !c.path.is_empty()));
        assert_eq!(d.total, d.changes.len());
        assert!(diff(&access, "--help", &last.short_id).is_err());
    }
}
