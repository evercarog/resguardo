//! «Lo que más ocupa» en una versión: las carpetas y los archivos más grandes,
//! a partir de `restic ls --json` (recorre la versión entera, sin descargar
//! datos: solo el índice de carpetas).

use crate::restic::{self, Access};
use serde::Serialize;
use std::collections::HashMap;
use std::time::Duration;

/// Recorrer una versión grande en un servidor remoto puede tardar.
const TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SizeItem {
    /// Ruta dentro de la versión (formato de restic: `/C/Users/…`).
    pub path: String,
    pub size: u64,
    /// Archivos que contiene (carpetas) o 1 (archivos).
    pub files: u64,
}

#[derive(Debug, Serialize)]
pub struct Largest {
    pub total_size: u64,
    pub total_files: u64,
    pub folders: Vec<SizeItem>,
    pub files: Vec<SizeItem>,
}

/// Un archivo (ruta, tamaño) de una línea de `restic ls --json`.
pub(crate) fn archivo_de_ls(line: &str) -> Option<(String, u64)> {
    if !line.contains(r#""type":"file""#) {
        return None;
    }
    let v = serde_json::from_str::<serde_json::Value>(line).ok()?;
    Some((v["path"].as_str()?.to_string(), v["size"].as_u64()?))
}

/// Calcula las carpetas y archivos más grandes de una lista (ruta, tamaño).
pub(crate) fn summarize(items: impl Iterator<Item = (String, u64)>, limit: usize) -> Largest {
    let mut total_size = 0u64;
    let mut total_files = 0u64;
    let mut dirs: HashMap<String, (u64, u64)> = HashMap::new();
    let mut files: Vec<SizeItem> = Vec::new();
    for (path, size) in items {
        // Los tamaños vienen del repositorio: sumados sin desbordar.
        total_size = total_size.saturating_add(size);
        total_files += 1;
        // Suma el archivo a cada carpeta que lo contiene.
        let mut end = path.len();
        while let Some(slash) = path[..end].rfind('/') {
            if slash == 0 {
                break;
            }
            let e = dirs.entry(path[..slash].to_string()).or_default();
            e.0 = e.0.saturating_add(size);
            e.1 += 1;
            end = slash;
        }
        files.push(SizeItem { path, size, files: 1 });
        // Solo se conservan los más grandes (sin guardar la lista entera).
        if files.len() > limit * 4 {
            files.sort_unstable_by_key(|f| std::cmp::Reverse(f.size));
            files.truncate(limit);
        }
    }
    files.sort_unstable_by_key(|f| std::cmp::Reverse(f.size));
    files.truncate(limit);

    // Carpetas: se omiten las que son solo «envoltorio» de otra (una
    // subcarpeta suya ocupa ≥ 90 %), para mostrar las que de verdad pesan.
    let mut biggest_child: HashMap<String, u64> = HashMap::new();
    for (path, (size, _)) in &dirs {
        if let Some(slash) = path.rfind('/') {
            if slash > 0 {
                let parent = path[..slash].to_string();
                let e = biggest_child.entry(parent).or_default();
                *e = (*e).max(*size);
            }
        }
    }
    let mut folders: Vec<SizeItem> = dirs
        .into_iter()
        .filter(|(path, (size, _))| biggest_child.get(path).is_none_or(|child| (*child as f64) < *size as f64 * 0.9))
        .map(|(path, (size, files))| SizeItem { path, size, files })
        .collect();
    folders.sort_unstable_by(|a, b| b.size.cmp(&a.size).then(a.path.cmp(&b.path)));
    folders.truncate(limit);
    Largest { total_size, total_files, folders, files }
}

pub fn largest(access: &Access, snapshot: &str, limit: usize) -> Result<Largest, String> {
    if !restic::valid_snapshot_id(snapshot) {
        return Err("Versión no válida.".into());
    }
    let mut items: Vec<(String, u64)> = Vec::new();
    let mut on_line = |line: &str| items.extend(archivo_de_ls(line));
    let out = restic::run_raw_lines(access, &["ls", "--json", "--no-lock", snapshot], TIMEOUT, &mut on_line)?;
    if out.code != Some(0) {
        return Err(restic::exit_error(out.code, &out.stderr));
    }
    Ok(summarize(items.into_iter(), limit.clamp(5, 200)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carpetas_y_archivos_mas_grandes() {
        let items = vec![
            ("/S/SIIWI01/BckRebuild/a.bak".to_string(), 10_000),
            ("/S/SIIWI01/BckRebuild/b.bak".to_string(), 9_000),
            ("/S/SIIWI01/Datos/x.dat".to_string(), 3_000),
            ("/S/SIIWI01/Datos/y.dat".to_string(), 1_000),
            ("/S/otro.txt".to_string(), 10),
        ];
        let r = summarize(items.into_iter(), 10);
        assert_eq!(r.total_size, 23_010);
        assert_eq!(r.total_files, 5);
        assert_eq!(r.files[0].path, "/S/SIIWI01/BckRebuild/a.bak");
        // «/S» es solo envoltorio de SIIWI01 (≥ 90 %): no aparece. SIIWI01 sí,
        // y dentro BckRebuild con su 82 %.
        assert!(!r.folders.iter().any(|f| f.path == "/S"));
        assert_eq!(r.folders[0].path, "/S/SIIWI01");
        let bck = r.folders.iter().find(|f| f.path == "/S/SIIWI01/BckRebuild").unwrap();
        assert_eq!((bck.size, bck.files), (19_000, 2));
        assert!(r.folders.iter().any(|f| f.path == "/S/SIIWI01/Datos"));
    }

    #[test]
    fn version_real() {
        let (Ok(repo), Ok(pw)) = (std::env::var("RESGUARDO_TEST_REPO"), std::env::var("RESGUARDO_TEST_PASSWORD")) else {
            return;
        };
        let access = Access::new(repo, pw);
        let snaps = restic::snapshots(&access).unwrap();
        let r = largest(&access, &snaps.last().unwrap().id, 10).unwrap();
        assert!(r.total_files > 0 && r.total_size > 0);
        assert!(!r.files.is_empty());
    }
}
