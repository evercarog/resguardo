//! Caché del espacio en disco de cada repositorio.
//!
//! `restic stats` recorre todos los snapshots: se guarda el resultado junto
//! con una huella de la lista de snapshots y solo se recalcula cuando esa
//! lista cambia (una copia nueva, un `forget`…). No contiene secretos.

use crate::restic::RepoStats;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    key: u64,
    computed_at: u64,
    stats: RepoStats,
}

#[derive(Serialize)]
pub struct CachedStats {
    pub stats: RepoStats,
    /// Segundos desde 1970 (UTC) en que se calculó.
    pub computed_at: u64,
    /// true si no hizo falta ejecutar restic.
    pub cached: bool,
}

pub struct StatsCache {
    path: PathBuf,
    lock: Mutex<()>,
}

/// Huella de la lista de snapshots (el orden no importa).
pub fn key(snapshot_ids: &[String]) -> u64 {
    let mut ids: Vec<&String> = snapshot_ids.iter().collect();
    ids.sort();
    let mut h = DefaultHasher::new();
    ids.hash(&mut h);
    h.finish()
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl StatsCache {
    pub fn new(config_dir: PathBuf) -> Self {
        Self { path: config_dir.join("stats-cache.json"), lock: Mutex::new(()) }
    }

    fn read(&self) -> HashMap<String, Entry> {
        fs::read(&self.path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    fn write(&self, map: &HashMap<String, Entry>) {
        if let (Some(dir), Ok(json)) = (self.path.parent(), serde_json::to_vec(map)) {
            let _ = fs::create_dir_all(dir);
            let tmp = self.path.with_extension("json.tmp");
            if fs::write(&tmp, json).is_ok() {
                let _ = fs::rename(&tmp, &self.path);
            }
        }
    }

    pub fn get(&self, repo_id: &str, key: u64) -> Option<CachedStats> {
        let _guard = self.lock.lock().unwrap();
        self.read().remove(repo_id).filter(|e| e.key == key).map(|e| CachedStats { stats: e.stats, computed_at: e.computed_at, cached: true })
    }

    pub fn put(&self, repo_id: &str, key: u64, stats: RepoStats) -> CachedStats {
        let _guard = self.lock.lock().unwrap();
        let mut map = self.read();
        let computed_at = now();
        map.insert(repo_id.to_string(), Entry { key, computed_at, stats: stats.clone() });
        self.write(&map);
        CachedStats { stats, computed_at, cached: false }
    }

    pub fn remove(&self, repo_id: &str) {
        let _guard = self.lock.lock().unwrap();
        let mut map = self.read();
        if map.remove(repo_id).is_some() {
            self.write(&map);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_huella_no_depende_del_orden() {
        let a = vec!["b".to_string(), "a".to_string()];
        let b = vec!["a".to_string(), "b".to_string()];
        assert_eq!(key(&a), key(&b));
        assert_ne!(key(&a), key(&["a".to_string()]));
    }

    #[test]
    fn guarda_y_recupera() {
        let dir = std::env::temp_dir().join(format!("resguardo-cache-{}", std::process::id()));
        let cache = StatsCache::new(dir.clone());
        let stats = RepoStats {
            total_size: 10,
            total_uncompressed_size: 20,
            compression_ratio: 2.0,
            compression_space_saving: 50.0,
            total_blob_count: 3,
            snapshots_count: 1,
        };
        cache.put("r", 42, stats);
        assert!(cache.get("r", 42).is_some_and(|c| c.cached && c.stats.total_size == 10));
        assert!(cache.get("r", 43).is_none(), "otra lista de snapshots invalida la caché");
        cache.remove("r");
        assert!(cache.get("r", 42).is_none());
        let _ = fs::remove_dir_all(dir);
    }
}
