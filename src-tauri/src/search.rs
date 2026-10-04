//! «Buscar un archivo» en todas las versiones de un destino (`restic find --json`).
//!
//! restic escribe un único array JSON (`[{...},{...}]`) sin saltos de línea,
//! pero lo va escribiendo según recorre las versiones: se separa por elementos
//! a medida que llega para mostrar resultados mientras busca.

use crate::jobs::{self, Jobs};
use crate::restic::{self, Access};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Runtime};

/// Evento con los resultados de cada versión en cuanto se encuentran.
pub const PROGRESS_EVENT: &str = "search-progress";

/// Máximo de coincidencias en total: con un patrón muy amplio («*») restic
/// devolvería todos los archivos de todas las versiones.
pub const MAX_MATCHES: usize = 5000;

pub fn job_key(repo_id: &str) -> String {
    format!("{repo_id}:search")
}

#[derive(Deserialize, Clone, Debug)]
pub struct Request {
    pub pattern: String,
    /// Buscar solo en estas versiones (vacío: en todas).
    #[serde(default)]
    pub snapshots: Vec<String>,
    /// Solo versiones que incluyan estas rutas (las carpetas de una copia).
    #[serde(default)]
    pub paths: Vec<String>,
    /// Solo versiones con estas etiquetas (las de una copia).
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Match {
    pub path: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub mtime: Option<String>,
}

/// Coincidencias de una versión, tal como las da restic.
#[derive(Deserialize, Debug)]
struct SnapshotHits {
    snapshot: String,
    #[serde(default)]
    matches: Vec<Match>,
}

#[derive(Serialize, Clone)]
struct Progress<'a> {
    repo_id: &'a str,
    snapshot: &'a str,
    matches: &'a [Match],
}

#[derive(Serialize, Debug)]
pub struct Outcome {
    /// Versiones con alguna coincidencia.
    pub snapshots: usize,
    pub matches: usize,
    /// Se paró al llegar a `MAX_MATCHES`.
    pub truncated: bool,
}

/// Patrón de restic a partir de lo que escribe el usuario: sin comodines se
/// busca como parte del nombre (`factura` → `*factura*`); una ruta de Windows
/// se pasa al formato de las versiones (`C:\Users\Ana` → `/C/Users/Ana`).
pub fn normalize_pattern(input: &str) -> Result<String, String> {
    let p = input.trim().trim_matches('"').replace('\\', "/");
    if p.is_empty() {
        return Err("Escribe el nombre (o parte del nombre) del archivo que buscas.".into());
    }
    if p.len() > 500 {
        return Err("El texto de búsqueda es demasiado largo.".into());
    }
    let b = p.as_bytes();
    let p = if b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic() { format!("/{}{}", (b[0] as char).to_ascii_uppercase(), &p[2..]) } else { p };
    let p = p.trim_end_matches('/').to_string();
    if p.is_empty() {
        return Err("Escribe el nombre (o parte del nombre) del archivo que buscas.".into());
    }
    if p.contains(['*', '?', '[']) || p.contains('/') {
        Ok(p)
    } else {
        Ok(format!("*{p}*"))
    }
}

pub fn find_args(req: &Request, pattern: &str) -> Result<Vec<String>, String> {
    let mut args: Vec<String> = vec!["find".into(), "--json".into(), "--no-lock".into()];
    // En Windows los nombres no distinguen mayúsculas.
    if cfg!(windows) {
        args.push("--ignore-case".into());
    }
    for s in &req.snapshots {
        if !restic::valid_snapshot_id(s) {
            return Err("ID de versión no válido.".into());
        }
        args.push("--snapshot".into());
        args.push(s.clone());
    }
    for p in &req.paths {
        args.push("--path".into());
        args.push(p.clone());
    }
    if !req.tags.is_empty() {
        args.push("--tag".into());
        args.push(req.tags.join(","));
    }
    args.push("--".into());
    args.push(pattern.into());
    Ok(args)
}

/// Separa los elementos de primer nivel de un array JSON que llega por trozos.
#[derive(Default)]
pub struct ArraySplitter {
    depth: u32,
    in_string: bool,
    escaped: bool,
    current: Vec<u8>,
}

impl ArraySplitter {
    /// Añade bytes y devuelve los elementos completos que se cerraron.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        for &c in bytes {
            if self.depth >= 2 {
                self.current.push(c);
            }
            if self.in_string {
                if self.escaped {
                    self.escaped = false;
                } else if c == b'\\' {
                    self.escaped = true;
                } else if c == b'"' {
                    self.in_string = false;
                }
                continue;
            }
            match c {
                b'"' => self.in_string = true,
                b'[' | b'{' => {
                    self.depth += 1;
                    // Empieza un elemento del array exterior.
                    if self.depth == 2 {
                        self.current.clear();
                        self.current.push(c);
                    }
                }
                b']' | b'}' => {
                    self.depth = self.depth.saturating_sub(1);
                    if self.depth == 1 {
                        out.push(std::mem::take(&mut self.current));
                    }
                }
                _ => {}
            }
        }
        out
    }
}

/// Busca y bloquea hasta que termina. Los resultados se emiten como `PROGRESS_EVENT`.
pub fn run<R: Runtime>(app: &AppHandle<R>, jobs: &Jobs, repo_id: &str, access: &Access, req: &Request) -> Result<Outcome, String> {
    let pattern = normalize_pattern(&req.pattern)?;
    let args = find_args(req, &pattern)?;
    let mut splitter = ArraySplitter::default();
    let mut outcome = Outcome { snapshots: 0, matches: 0, truncated: false };
    let out = jobs::run_stream(jobs, &job_key(repo_id), access, &args, |chunk| {
        for item in splitter.feed(chunk) {
            let Ok(mut hits) = serde_json::from_slice::<SnapshotHits>(&item) else { continue };
            if hits.matches.is_empty() {
                continue;
            }
            let room = MAX_MATCHES - outcome.matches;
            if hits.matches.len() > room {
                hits.matches.truncate(room);
                outcome.truncated = true;
            }
            outcome.snapshots += 1;
            outcome.matches += hits.matches.len();
            let _ = app.emit(PROGRESS_EVENT, Progress { repo_id, snapshot: &hits.snapshot, matches: &hits.matches });
            if outcome.matches >= MAX_MATCHES {
                outcome.truncated = true;
                return false;
            }
        }
        true
    })?;
    if out.cancelled {
        return Err("Búsqueda cancelada.".into());
    }
    if !outcome.truncated && out.exit_code != Some(0) {
        return Err(restic::exit_error(out.exit_code, &out.stderr));
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patrones() {
        assert_eq!(normalize_pattern("factura").unwrap(), "*factura*");
        assert_eq!(normalize_pattern("  factura_123.xlsx ").unwrap(), "*factura_123.xlsx*");
        assert_eq!(normalize_pattern("*.xlsx").unwrap(), "*.xlsx");
        assert_eq!(normalize_pattern(r"C:\Users\Ana\informe.docx").unwrap(), "/C/Users/Ana/informe.docx");
        assert_eq!(normalize_pattern(r#""d:\fotos\""#).unwrap(), "/D/fotos");
        assert!(normalize_pattern("   ").is_err());
        assert!(normalize_pattern("/").is_err());
    }

    #[test]
    fn argumentos() {
        let req = Request {
            pattern: "--help".into(),
            snapshots: vec!["da37898f".into()],
            paths: vec![r"C:\Users\Ana".into()],
            tags: vec!["diaria".into(), "casa".into()],
        };
        let pattern = normalize_pattern(&req.pattern).unwrap();
        let args = find_args(&req, &pattern).unwrap();
        assert!(args.windows(2).any(|w| w == ["--snapshot", "da37898f"]));
        assert!(args.windows(2).any(|w| w == ["--path", r"C:\Users\Ana"]));
        assert!(args.windows(2).any(|w| w == ["--tag", "diaria,casa"]));
        // El patrón va tras «--»: nunca se toma como opción.
        assert_eq!(args[args.len() - 2..], ["--", "*--help*"]);
        let bad = Request { snapshots: vec!["--x".into()], ..req };
        assert!(find_args(&bad, "x").is_err());
    }

    #[test]
    fn separa_el_array_por_trozos() {
        let json = br#"[{"matches":[{"path":"/C/a [1]{x}.txt","type":"file","size":10,"mtime":"2026-09-29T00:25:11-05:00"}],"hits":1,"snapshot":"aa"},{"matches":[{"path":"/C/b \"q\" \\ ].txt","type":"file","size":3}],"hits":1,"snapshot":"bb"}]"#;
        // Trozos de 7 bytes: los elementos se parten por cualquier sitio.
        let mut s = ArraySplitter::default();
        let items: Vec<Vec<u8>> = json.chunks(7).flat_map(|c| s.feed(c)).collect();
        assert_eq!(items.len(), 2);
        let a: SnapshotHits = serde_json::from_slice(&items[0]).unwrap();
        let b: SnapshotHits = serde_json::from_slice(&items[1]).unwrap();
        assert_eq!(a.snapshot, "aa");
        assert_eq!(a.matches[0].path, "/C/a [1]{x}.txt");
        assert_eq!(a.matches[0].size, Some(10));
        assert_eq!(b.matches[0].path, r#"/C/b "q" \ ].txt"#);
        assert_eq!(b.matches[0].mtime, None);
        assert!(s.feed(b"").is_empty());
        // Sin resultados: «[]».
        assert!(ArraySplitter::default().feed(b"[]\n").is_empty());
    }

    /// Búsqueda real en el repo de prueba.
    #[test]
    fn busqueda_real() {
        let _real = crate::restic::tests::real_repo_lock();
        let (Ok(location), Ok(pw), Ok(data)) =
            (std::env::var("RESGUARDO_TEST_REPO"), std::env::var("RESGUARDO_TEST_PASSWORD"), std::env::var("RESGUARDO_TEST_DATA"))
        else {
            return;
        };
        let access = Access::new(location, pw);
        let snaps = restic::snapshots(&access).unwrap();
        let newest = snaps.iter().max_by(|a, b| a.time.cmp(&b.time)).unwrap();
        let dir = restic::tests::to_snapshot_path(&data);
        let first = restic::list_dir(&access, &newest.short_id, &dir).unwrap().into_iter().find(|e| e.kind == "file").unwrap();
        let req = Request { pattern: first.name.to_uppercase(), snapshots: vec![newest.short_id.clone()], paths: vec![], tags: vec![] };
        let jobs = Jobs::default();
        let mut s = ArraySplitter::default();
        let mut found = Vec::new();
        let mut args = find_args(&req, &normalize_pattern(&req.pattern).unwrap()).unwrap();
        // Caché propia: en Windows, dos restic escribiendo a la vez en la misma
        // caché chocan (los tests de copias usan el mismo repositorio en paralelo).
        let cache = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("cache-busqueda");
        args.splice(1..1, ["--cache-dir".to_string(), cache.to_string_lossy().into_owned()]);
        let out = jobs::run_stream(&jobs, "t:search", &access, &args, |chunk| {
            for item in s.feed(chunk) {
                let hits: SnapshotHits = serde_json::from_slice(&item).unwrap();
                found.extend(hits.matches);
            }
            true
        })
        .unwrap();
        assert_eq!(out.exit_code, Some(0), "{}", out.stderr);
        assert!(found.iter().any(|m| m.path.ends_with(&format!("/{}", first.name))), "{found:?}");
        assert!(jobs.is_empty());
    }
}
