//! Prueba de restauración: cada cierto tiempo, el agente restaura de verdad
//! unos cuantos archivos de una versión y comprueba que salen enteros.
//!
//! La verificación (`restic check`) dice que el repositorio está sano; la
//! prueba de restauración demuestra que se puede recuperar. Los archivos se
//! restauran en la carpeta privada del agente (solo SYSTEM y Administradores)
//! con `--verify` (restic comprueba el contenido) y se borran siempre al
//! terminar: son archivos de otras personas.

use crate::agent::{self, RunRecord, Schedule};
use crate::restic::{self, Access};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Una prueba puede tardar (leer una versión grande con `ls`, descargar de la nube).
const TEST_TIMEOUT: Duration = Duration::from_secs(4 * 3600);
/// Nombre de las carpetas de la prueba dentro de la carpeta privada.
const DIR_PREFIX: &str = "prueba-restauracion";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RestoreTest {
    pub schedule: Schedule,
    /// Archivos que se restauran cada vez.
    #[serde(default = "default_files")]
    pub files: u32,
    /// Tamaño máximo de todos ellos juntos, en MB.
    #[serde(default = "default_max_mb")]
    pub max_mb: u32,
    /// RFC 3339: cuándo se activó (referencia para la primera vez).
    pub enabled_at: String,
}

fn default_files() -> u32 {
    20
}
fn default_max_mb() -> u32 {
    200
}

impl RestoreTest {
    pub fn validate(&self) -> Result<(), String> {
        self.schedule.validate_task()?;
        if !(1..=500).contains(&self.files) {
            return Err("Entre 1 y 500 archivos.".into());
        }
        if !(1..=10_240).contains(&self.max_mb) {
            return Err("Entre 1 MB y 10 GB.".into());
        }
        Ok(())
    }
}

/// Un archivo de la versión (de `restic ls --json`).
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// Ruta dentro de la versión (`/C/Users/ana/x.docx`).
    pub path: String,
    pub size: u64,
}

/// Generador pseudoaleatorio sencillo (no hace falta más: solo elige muestras).
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }
    pub fn from_clock() -> Self {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(42);
        Self::new(n ^ (std::process::id() as u64).rotate_left(32))
    }
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next() % n
        }
    }
}

/// Muestreo por tamaños: tres reservas (pequeños, medianos y grandes) para
/// que la prueba no se quede solo con archivos diminutos.
pub struct Sampler {
    buckets: [Vec<Candidate>; 3],
    seen: [u64; 3],
    capacity: usize,
    max_file: u64,
}

impl Sampler {
    pub fn new(capacity: usize, max_bytes: u64) -> Self {
        Self { buckets: Default::default(), seen: [0; 3], capacity: capacity.max(1), max_file: max_bytes }
    }

    fn bucket(size: u64) -> usize {
        match size {
            0..=1_048_575 => 0,          // < 1 MB
            1_048_576..=20_971_519 => 1, // < 20 MB
            _ => 2,
        }
    }

    /// Un archivo más (algoritmo R: cada uno tiene la misma probabilidad de quedarse).
    pub fn offer(&mut self, c: Candidate, rng: &mut Rng) {
        // Vacíos, más grandes que todo el presupuesto o con nombres que no
        // caben en una lista de patrones (saltos de línea): no sirven.
        if c.size == 0 || c.size > self.max_file || c.path.contains(['\n', '\r']) {
            return;
        }
        let b = Self::bucket(c.size);
        self.seen[b] += 1;
        if self.buckets[b].len() < self.capacity {
            self.buckets[b].push(c);
        } else {
            let j = rng.below(self.seen[b]) as usize;
            if j < self.capacity {
                self.buckets[b][j] = c;
            }
        }
    }

    /// Elige hasta `n` archivos que quepan en `max_bytes`, alternando tamaños.
    pub fn pick(mut self, n: usize, max_bytes: u64, rng: &mut Rng) -> Vec<Candidate> {
        for b in &mut self.buckets {
            // Barajar cada reserva.
            for i in (1..b.len()).rev() {
                let j = rng.below(i as u64 + 1) as usize;
                b.swap(i, j);
            }
        }
        let mut out = Vec::new();
        let mut total = 0u64;
        let mut progress = true;
        while out.len() < n && progress {
            progress = false;
            for b in [2usize, 1, 0] {
                if out.len() >= n {
                    break;
                }
                // El primero de esta reserva que aún quepa.
                if let Some(pos) = self.buckets[b].iter().position(|c| total + c.size <= max_bytes) {
                    let c = self.buckets[b].remove(pos);
                    total += c.size;
                    out.push(c);
                    progress = true;
                }
            }
        }
        out
    }
}

/// Qué versión probar: la más reciente o, una de cada dos veces, una al azar
/// de los últimos 30 días.
pub fn choose_snapshot<'a>(snaps: &'a [restic::Snapshot], now: DateTime<Local>, rng: &mut Rng) -> Option<&'a restic::Snapshot> {
    let mut sorted: Vec<&restic::Snapshot> = snaps.iter().collect();
    sorted.sort_by(|a, b| b.time.cmp(&a.time));
    let latest = *sorted.first()?;
    if rng.below(2) == 0 {
        return Some(latest);
    }
    let recent: Vec<&restic::Snapshot> = sorted
        .iter()
        .copied()
        .filter(|s| DateTime::parse_from_rfc3339(&s.time).is_ok_and(|t| now.signed_duration_since(t) <= chrono::Duration::days(30)))
        .collect();
    if recent.is_empty() {
        return Some(latest);
    }
    Some(recent[rng.below(recent.len() as u64) as usize])
}

/// Patrón de restic para un archivo: los comodines (`*`, `?`, `[`) no se
/// pueden escapar en Windows, así que se cambian por `?` (un carácter).
pub fn include_pattern(path: &str) -> String {
    path.chars().map(|c| if matches!(c, '*' | '?' | '[' | ']') { '?' } else { c }).collect()
}

/// Dónde queda restaurado un archivo de la versión dentro de `target`.
pub fn restored_path(target: &Path, snapshot_path: &str) -> PathBuf {
    let mut p = target.to_path_buf();
    for part in snapshot_path.split('/').filter(|s| !s.is_empty()) {
        p.push(part);
    }
    p
}

/// Carpeta de trabajo dentro de `base` (la carpeta privada del agente).
fn work_dir(base: &Path) -> PathBuf {
    base.join(DIR_PREFIX)
}

fn include_file(base: &Path) -> PathBuf {
    base.join(format!("{DIR_PREFIX}-incluir.txt"))
}

/// Quita el atributo de solo lectura (restic lo restaura tal cual) para poder
/// borrar. No entra en enlaces: solo se toca lo que hay dentro de `dir`.
#[allow(clippy::permissions_set_readonly_false)] // en Windows solo quita el atributo
pub fn clear_readonly(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            clear_readonly(&entry.path());
        }
        if let Ok(meta) = entry.metadata() {
            let mut perms = meta.permissions();
            if perms.readonly() {
                perms.set_readonly(false);
                let _ = fs::set_permissions(entry.path(), perms);
            }
        }
    }
}

/// Borra lo que haya quedado de una prueba anterior (p. ej. si se cortó).
/// Son archivos de otras personas: si no se pueden borrar, se anota.
pub fn clean_leftovers(base: &Path) {
    let dir = work_dir(base);
    if fs::symlink_metadata(&dir).is_ok() && fs::remove_dir_all(&dir).is_err() {
        clear_readonly(&dir);
        if let Err(e) = fs::remove_dir_all(&dir) {
            agent::log(&format!("AVISO: no se pudieron borrar los archivos de la prueba de restauración: {e}"));
        }
    }
    let _ = fs::remove_file(include_file(base));
}

/// Al salir (bien, mal o por un error), se borra todo lo restaurado.
struct Cleanup(PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        clean_leftovers(&self.0);
    }
}

/// «archivo .docx de 2,1 MB»: sin el nombre ni la carpeta (son de otras personas).
pub fn describe(c: &Candidate) -> String {
    let name = c.path.rsplit('/').next().unwrap_or_default();
    let ext = name.rsplit_once('.').map(|(_, e)| e).filter(|e| !e.is_empty() && e.len() <= 8 && e.chars().all(|ch| ch.is_ascii_alphanumeric()));
    match ext {
        Some(e) => format!("archivo .{} de {}", e.to_lowercase(), crate::tasks::human_bytes(c.size)),
        None => format!("archivo de {}", crate::tasks::human_bytes(c.size)),
    }
}

/// «20 archivos (85 MB) de la versión del 30/09 18:00 restaurados y comprobados».
pub fn ok_message(files: usize, bytes: u64, snapshot_time: &str) -> String {
    let when = DateTime::parse_from_rfc3339(snapshot_time).map(|t| t.with_timezone(&Local).format("%d/%m %H:%M").to_string()).unwrap_or_default();
    let what = if files == 1 { "1 archivo".to_string() } else { format!("{files} archivos") };
    format!("{what} ({}) de la versión del {when} restaurados y comprobados.", crate::tasks::human_bytes(bytes))
}

/// Progreso: qué está haciendo y, al restaurar, el porcentaje.
pub type Report<'a> = &'a mut dyn FnMut(&str, Option<f64>);

/// Hace la prueba con estos datos de acceso, en la carpeta privada del agente.
pub fn run(access: &Access, cfg: &RestoreTest, report: Report) -> RunRecord {
    run_in(access, cfg, &agent::private_dir(), report)
}

/// `base`: carpeta (sin acceso para los usuarios) donde se restaura y se borra.
fn run_in(access: &Access, cfg: &RestoreTest, base: &Path, report: Report) -> RunRecord {
    let mut record = RunRecord { started: Local::now().to_rfc3339(), ..Default::default() };
    // Los mensajes se guardan en el estado del agente (lo leen todos los
    // usuarios del equipo) y van a la web: sin rutas ni nombres de archivos.
    let fail = |mut r: RunRecord, msg: String| {
        r.result = "error".into();
        r.message = crate::web::public_message(&msg);
        r.finished = Local::now().to_rfc3339();
        r
    };
    clean_leftovers(base);
    let _cleanup = Cleanup(base.to_path_buf());
    let mut rng = Rng::from_clock();

    report("Eligiendo la versión…", None);
    let snaps = match restic::snapshots(access) {
        Ok(s) => s,
        Err(e) => return fail(record, e),
    };
    let Some(snap) = choose_snapshot(&snaps, Local::now(), &mut rng).cloned() else {
        return fail(record, "Este repositorio todavía no tiene versiones que probar.".into());
    };

    // Los archivos de la versión, sin guardarlos todos en memoria.
    report("Eligiendo los archivos…", None);
    let max_bytes = cfg.max_mb as u64 * 1024 * 1024;
    let mut sampler = Sampler::new(cfg.files as usize * 3, max_bytes);
    let mut on_line = |line: &str| {
        if !line.contains(r#""struct_type":"node""#) {
            return;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { return };
        if v["type"] != "file" {
            return;
        }
        if let (Some(path), Some(size)) = (v["path"].as_str(), v["size"].as_u64()) {
            sampler.offer(Candidate { path: path.to_string(), size }, &mut rng);
        }
    };
    match restic::run_raw_lines(access, &["ls", "--json", "--no-lock", &snap.id], TEST_TIMEOUT, &mut on_line) {
        Ok(out) if out.code == Some(0) => {}
        Ok(out) => return fail(record, restic::exit_error(out.code, &out.stderr)),
        Err(e) => return fail(record, e),
    }
    let mut rng = Rng::from_clock();
    let chosen = sampler.pick(cfg.files as usize, max_bytes, &mut rng);
    if chosen.is_empty() {
        return fail(record, format!("La versión del {} no tiene archivos que quepan en {} MB.", snap.time.chars().take(16).collect::<String>(), cfg.max_mb));
    }
    let total: u64 = chosen.iter().map(|c| c.size).sum();

    // Restaurar (en la carpeta privada) con verificación del contenido.
    let target = work_dir(base);
    if let Err(e) = fs::create_dir_all(&target) {
        return fail(record, format!("No se pudo preparar la carpeta de la prueba: {e}"));
    }
    let patterns: String = chosen.iter().map(|c| include_pattern(&c.path) + "\n").collect();
    if let Err(e) = fs::write(include_file(base), patterns) {
        return fail(record, format!("No se pudo preparar la prueba: {e}"));
    }
    report(&format!("Restaurando {} archivos ({})…", chosen.len(), crate::tasks::human_bytes(total)), Some(0.0));
    let target_s = target.to_string_lossy().into_owned();
    let include_s = include_file(base).to_string_lossy().into_owned();
    let mut on_restore = |line: &str| {
        if line.contains(r#""message_type":"status""#) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(p) = v["percent_done"].as_f64() {
                    report("Restaurando y comprobando…", Some(p));
                }
            }
        }
    };
    let args = ["restore", "--json", "--no-lock", "--verify", "--target", &target_s, "--include-file", &include_s, &snap.id];
    match restic::run_raw_lines(access, &args, TEST_TIMEOUT, &mut on_restore) {
        Ok(out) if out.code == Some(0) => {}
        Ok(out) => {
            // Con --json, los errores de un archivo llegan como «error» con su ruta.
            let detail = String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                .find(|v| v["message_type"] == "error")
                .map(|v| format!("No se pudo restaurar un archivo de la muestra: {}", v["error"]["message"].as_str().unwrap_or("")));
            return fail(record, detail.unwrap_or_else(|| restic::exit_error(out.code, &out.stderr)));
        }
        Err(e) => return fail(record, e),
    }

    // Cada archivo, en su sitio y con su tamaño (el contenido lo comprobó --verify).
    report("Comprobando los archivos…", Some(1.0));
    for c in &chosen {
        let p = restored_path(&target, &c.path);
        match fs::metadata(&p) {
            Ok(m) if m.len() == c.size => {}
            Ok(m) => return fail(record, format!("Un archivo ({}) se restauró con {} bytes en lugar de {}.", describe(c), m.len(), c.size)),
            Err(e) => return fail(record, format!("Un archivo ({}) no se restauró: {e}", describe(c))),
        }
    }
    record.result = "ok".into();
    record.message = ok_message(chosen.len(), total, &snap.time);
    record.files_new = Some(chosen.len() as u64);
    record.data_added = Some(total);
    record.snapshot_id = Some(snap.id.clone());
    record.finished = Local::now().to_rfc3339();
    record
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(path: &str, size: u64) -> Candidate {
        Candidate { path: path.into(), size }
    }

    #[test]
    fn muestra_con_tamanos_variados_dentro_del_presupuesto() {
        let mut rng = Rng::new(7);
        let max = 200 * 1024 * 1024;
        let mut s = Sampler::new(60, max);
        for i in 0..5000u64 {
            let size = match i % 10 {
                0 => 50 * 1024 * 1024,    // grandes
                1..=3 => 5 * 1024 * 1024, // medianos
                _ => 10_000 + i,          // pequeños
            };
            s.offer(cand(&format!("/C/datos/{i}.bin"), size), &mut rng);
        }
        s.offer(cand("/C/vacio.txt", 0), &mut rng);
        s.offer(cand("/C/enorme.iso", max + 1), &mut rng);
        s.offer(cand("/C/raro\nnombre", 10), &mut rng);
        let picked = s.pick(20, max, &mut rng);
        assert_eq!(picked.len(), 20);
        let total: u64 = picked.iter().map(|c| c.size).sum();
        assert!(total <= max, "dentro del presupuesto");
        assert!(picked.iter().any(|c| c.size >= 20 * 1024 * 1024), "alguno grande");
        assert!(picked.iter().any(|c| c.size < 1024 * 1024), "alguno pequeño");
        assert!(picked.iter().all(|c| c.size > 0 && !c.path.contains('\n')));
        let mut paths: Vec<&str> = picked.iter().map(|c| c.path.as_str()).collect();
        paths.dedup();
        assert_eq!(paths.len(), 20, "sin repetir");
    }

    #[test]
    fn pocos_archivos_o_presupuesto_justo() {
        let mut rng = Rng::new(3);
        let mut s = Sampler::new(60, 1000);
        s.offer(cand("/a", 600), &mut rng);
        s.offer(cand("/b", 600), &mut rng);
        s.offer(cand("/c", 300), &mut rng);
        let picked = s.pick(20, 1000, &mut rng);
        assert!(picked.iter().map(|c| c.size).sum::<u64>() <= 1000);
        assert_eq!(picked.len(), 2, "uno de 600 y el de 300");
    }

    #[test]
    fn descripcion_sin_nombre() {
        let d = describe(&cand("/C/Users/ana/Nóminas/secreto marzo.XLSX", 2_200_000));
        assert!(d.starts_with("archivo .xlsx de "), "{d}");
        assert!(!d.contains("secreto") && !d.contains("ana"));
        assert!(describe(&cand("/C/x/sin-extension", 10)).starts_with("archivo de "));
        assert!(describe(&cand("/C/x/raro.tar.gz;rm", 10)).starts_with("archivo de "));
    }

    #[test]
    fn limpieza_con_archivos_de_solo_lectura() {
        let base = std::env::temp_dir().join(format!("resguardo-limpieza-{}", std::process::id()));
        let dir = work_dir(&base);
        fs::create_dir_all(dir.join("C").join("x")).unwrap();
        let file = dir.join("C").join("x").join("a.txt");
        fs::write(&file, "x").unwrap();
        let mut perms = fs::metadata(&file).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&file, perms).unwrap();
        clean_leftovers(&base);
        assert!(!dir.exists(), "lo restaurado se borra aunque sea de solo lectura");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn patrones_rutas_y_mensaje() {
        assert_eq!(include_pattern("/C/datos/informe [v2]*.docx"), "/C/datos/informe ?v2??.docx");
        let p = restored_path(Path::new(r"C:\ProgramData\Resguardo\privado\prueba-restauracion"), "/C/Users/ana/x.txt");
        assert!(p.ends_with(Path::new("C").join("Users").join("ana").join("x.txt")));
        let msg = ok_message(20, 85 * 1024 * 1024, "2026-09-30T18:00:00-05:00");
        assert!(msg.starts_with("20 archivos (85 MB) de la versión del "), "{msg}");
        assert!(msg.ends_with("restaurados y comprobados."));
        assert!(RestoreTest { schedule: Schedule::Weekly { weekday: 5, time: "04:00".into() }, files: 0, max_mb: 200, enabled_at: String::new() }
            .validate()
            .is_err());
        let old: RestoreTest = serde_json::from_str(r#"{"schedule":{"kind":"weekly","weekday":5,"time":"04:00"},"enabled_at":"x"}"#).unwrap();
        assert_eq!((old.files, old.max_mb), (20, 200));
    }

    #[test]
    fn elige_la_version() {
        let snap = |id: &str, time: &str| restic::Snapshot {
            id: id.into(),
            short_id: id.into(),
            time: time.into(),
            hostname: String::new(),
            username: None,
            paths: vec![],
            tags: vec![],
            excludes: vec![],
            parent: None,
            program_version: None,
            summary: None,
            original: None,
        };
        let now = DateTime::parse_from_rfc3339("2026-10-01T12:00:00-05:00").unwrap().with_timezone(&Local);
        let snaps = vec![snap("viejo", "2026-01-01T00:00:00-05:00"), snap("nuevo", "2026-09-30T18:00:00-05:00"), snap("medio", "2026-09-20T18:00:00-05:00")];
        let mut rng = Rng::new(11);
        for _ in 0..50 {
            let s = choose_snapshot(&snaps, now, &mut rng).unwrap();
            assert_ne!(s.id, "viejo", "solo de los últimos 30 días");
        }
        assert!(choose_snapshot(&[], now, &mut rng).is_none());
    }

    /// Prueba real con el repositorio de prueba (en una carpeta temporal propia).
    #[test]
    fn prueba_real() {
        let _real = crate::restic::tests::real_repo_lock();
        let (Ok(location), Ok(pw)) = (std::env::var("RESGUARDO_TEST_REPO"), std::env::var("RESGUARDO_TEST_PASSWORD")) else {
            return;
        };
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("prueba-restauracion-test");
        fs::create_dir_all(&base).unwrap();
        let access = Access::new(location, pw);
        let cfg = RestoreTest { schedule: Schedule::Weekly { weekday: 5, time: "04:00".into() }, files: 5, max_mb: 50, enabled_at: String::new() };
        let mut stages = Vec::new();
        let r = run_in(&access, &cfg, &base, &mut |s, _| stages.push(s.to_string()));
        assert_eq!(r.result, "ok", "{}", r.message);
        assert!(r.message.contains("restaurados y comprobados"), "{}", r.message);
        assert!(r.files_new.unwrap_or(0) > 0);
        assert!(!work_dir(&base).exists() && !include_file(&base).exists(), "se borra al terminar");
        assert!(stages.iter().any(|s| s.starts_with("Restaurando")), "{stages:?}");
        let _ = fs::remove_dir_all(&base);
    }
}
