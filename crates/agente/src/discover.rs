//! Dentro de un destino: qué repositorios hay (fase 2 de docs/destinos.md) y
//! clonar un repositorio a otro destino con `restic copy`.

use crate::restic::{self, Access};
use crate::store::Repo;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Un repositorio encontrado (o conocido) en un destino.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Found {
    /// Ubicación de restic, lista para «Usar este».
    pub location: String,
    /// Ruta dentro del destino, para mostrar («portatil», «clientes/altamar»).
    pub path: String,
    /// Repositorio de este equipo que ya lo usa.
    pub repo_id: Option<String>,
    /// Encontrado al listar (true) o solo recordado (false).
    pub listed: bool,
}

/// Forma comparable de una ubicación (barras, mayúsculas en Windows, sin barra final).
fn comparable(location: &str) -> String {
    let l = location.trim().replace('\\', "/");
    let l = l.trim_end_matches('/');
    if l.as_bytes().get(1) == Some(&b':') || l.starts_with("//") || cfg!(windows) && !l.contains("://") {
        l.to_lowercase()
    } else {
        l.to_string()
    }
}

/// Carpeta base de un destino local o de red: la carpeta padre de un repositorio suyo.
pub fn local_base(location: &str) -> Option<PathBuf> {
    let p = Path::new(location.trim());
    if crate::places::kind(location) != "local" {
        return None;
    }
    let parent = p.parent().filter(|x| !x.as_os_str().is_empty()).unwrap_or(p);
    Some(parent.to_path_buf())
}

/// ¿Es una carpeta un repositorio de restic? (`config` y `keys/`).
fn is_repo_dir(dir: &Path) -> bool {
    dir.join("config").is_file() && dir.join("keys").is_dir()
}

/// Repositorios en una carpeta: ella misma y hasta dos niveles por debajo.
pub fn scan_local(base: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if is_repo_dir(base) {
        out.push(base.to_path_buf());
        return out;
    }
    let subdirs = |d: &Path| -> Vec<PathBuf> {
        let Ok(rd) = std::fs::read_dir(d) else { return Vec::new() };
        let mut v: Vec<PathBuf> = rd.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.is_dir()).collect();
        v.sort();
        v.truncate(200);
        v
    };
    for d1 in subdirs(base) {
        if is_repo_dir(&d1) {
            out.push(d1);
            continue;
        }
        for d2 in subdirs(&d1) {
            if is_repo_dir(&d2) {
                out.push(d2);
            }
        }
    }
    out
}

/// Lo que hay en un destino: lo encontrado al listarlo (si se puede), lo que
/// este equipo ya usa y lo que recuerda de antes. `template` es un
/// repositorio del destino (para sus credenciales de nube).
pub fn scan(place: &crate::places::Place, repos: &[Repo], template: Option<&Repo>) -> Result<(Vec<Found>, Option<String>), String> {
    let mut found: Vec<Found> = Vec::new();
    let mut note = None;
    let mine = |loc: &str| repos.iter().find(|r| comparable(&r.location) == comparable(loc)).map(|r| r.id.clone());
    let push = |found: &mut Vec<Found>, location: String, path: String, listed: bool| {
        if let Some(f) = found.iter_mut().find(|f| comparable(&f.location) == comparable(&location)) {
            f.listed |= listed;
            return;
        }
        found.push(Found { repo_id: mine(&location), location, path, listed });
    };

    if let Some(t) = template {
        match crate::places::kind(&t.location) {
            "local" => {
                if let Some(base) = local_base(&t.location) {
                    if base.is_dir() {
                        for dir in scan_local(&base) {
                            let rel = dir.strip_prefix(&base).map(|p| p.display().to_string()).unwrap_or_default();
                            push(&mut found, dir.display().to_string(), if rel.is_empty() { ".".into() } else { rel }, true);
                        }
                    } else {
                        note = Some("No se encuentra la carpeta del destino. ¿Está conectado el disco?".to_string());
                    }
                }
            }
            "s3" => match crate::store::cloud_creds(t)? {
                Some(c) => {
                    let bucket =
                        crate::s3list::Bucket::from_location(&t.location, c.region.as_deref(), &c.key_id, &c.key_secret).ok_or("Ubicación de S3 no válida.")?;
                    match crate::s3list::find_repos(&bucket) {
                        Ok(prefixes) => {
                            let base = format!("s3:{}/{}", bucket.endpoint, bucket.bucket);
                            for p in prefixes {
                                let location = if p.is_empty() { base.clone() } else { format!("{base}/{p}") };
                                push(&mut found, location, if p.is_empty() { ".".into() } else { p }, true);
                            }
                        }
                        Err(e) => note = Some(e),
                    }
                }
                None => note = Some("Este destino no tiene claves de nube guardadas: no se puede listar.".into()),
            },
            _ => note = Some("Este tipo de destino no permite listar sus repositorios: aquí están los que Resguardo conoce.".into()),
        }
    }
    // Lo que ya usa este equipo y lo que recuerda.
    for r in repos.iter().filter(|r| r.place_id.as_deref() == Some(place.id.as_str())) {
        push(&mut found, r.location.clone(), relative(&r.location, template), false);
    }
    for loc in &place.known {
        push(&mut found, loc.clone(), relative(loc, template), false);
    }
    found.sort_by_key(|f| f.path.to_lowercase());
    Ok((found, note))
}

/// Ruta de un repositorio dentro de su destino, para mostrar.
pub fn relative(location: &str, template: Option<&Repo>) -> String {
    let l = location.trim().trim_end_matches(['/', '\\']);
    if crate::places::kind(l) == "local" {
        if let Some(base) = template.and_then(|t| local_base(&t.location)) {
            if let Ok(rel) = Path::new(l).strip_prefix(&base) {
                let s = rel.display().to_string();
                return if s.is_empty() { ".".into() } else { s };
            }
        }
        return Path::new(l).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_else(|| l.to_string());
    }
    // Nube y servidores: lo que va tras el bucket o el servidor.
    let after_scheme = l.split_once("://").map_or(l.split_once(':').map_or(l, |x| x.1), |x| x.1);
    let mut parts = after_scheme.split('/').filter(|s| !s.is_empty());
    let _host = parts.next();
    let rest: Vec<&str> = parts.collect();
    if l.starts_with("s3:") {
        return if rest.len() <= 1 { ".".into() } else { rest[1..].join("/") };
    }
    if l.starts_with("b2:") || l.starts_with("azure:") || l.starts_with("gs:") {
        return l.splitn(3, ':').nth(2).map(|p| p.trim_start_matches('/').to_string()).filter(|p| !p.is_empty()).unwrap_or_else(|| ".".into());
    }
    if l.starts_with("sftp:") {
        return l.rsplit(':').next().unwrap_or(l).to_string();
    }
    if rest.is_empty() {
        ".".into()
    } else {
        rest.join("/")
    }
}

// ---------- Clonar ----------

/// Variables de la nube que comparten origen y destino al copiar (restic usa un solo juego).
fn conflict(src: &Access, dest: &Access) -> bool {
    src.env.iter().any(|(k, v)| dest.env.iter().any(|(k2, v2)| k == k2 && v != v2))
}

#[derive(Serialize, Clone)]
pub struct CloneProgress {
    pub from: String,
    pub stage: String,
    pub done: usize,
    pub total: usize,
}

pub const PROGRESS_EVENT: &str = "clone-progress";

pub fn job_key(from: &str) -> String {
    format!("{from}:clone")
}

/// Crea un repositorio nuevo en `dest` con los parámetros de troceado del
/// origen (la deduplicación sigue funcionando entre los dos) y copia todas
/// sus versiones. Devuelve cuántas versiones copió.
pub fn clone(jobs: &crate::jobs::Jobs, from_id: &str, src: &Access, dest: &Access, mut report: impl FnMut(CloneProgress)) -> Result<usize, String> {
    if conflict(src, dest) {
        return Err("No se puede clonar entre dos cuentas de nube distintas a la vez: restic usa una sola clave para las dos. \
                    Clona primero a un disco o a un servidor, y desde ahí a la otra nube."
            .into());
    }
    let mut both = dest.clone();
    for kv in &src.env {
        if !both.env.iter().any(|(k, _)| *k == kv.0) {
            both.env.push(kv.clone());
        }
    }
    both.env.push(("RESTIC_FROM_PASSWORD".into(), src.password.clone()));
    if both.cacert.is_none() {
        both.cacert = src.cacert.clone();
    }
    let from = crate::tasks::location_with_auth(&src.location, src.rest_auth.as_ref());

    // El de llegada no puede existir ya: clonar nunca escribe en un repositorio ajeno.
    let probe = restic::run_raw(dest, &["cat", "config", "--no-lock"], restic::CHECK_TIMEOUT)?;
    if probe.code == Some(0) {
        return Err("Ya hay un repositorio en esa ubicación. Elige otra ruta.".into());
    }
    let total = restic::snapshots(src)?.len();
    report(CloneProgress { from: from_id.into(), stage: "Creando el repositorio nuevo…".into(), done: 0, total });
    let out = restic::run_raw(&both, &["init", "--from-repo", &from, "--copy-chunker-params"], restic::CHECK_TIMEOUT)?;
    if out.code != Some(0) {
        return Err(restic::exit_error(out.code, &out.stderr));
    }

    report(CloneProgress { from: from_id.into(), stage: "Copiando las versiones…".into(), done: 0, total });
    let args: Vec<String> = vec!["copy".into(), "--from-repo".into(), from, "--retry-lock".into(), "30m".into()];
    let mut done = 0usize;
    let mut pending = String::new();
    let out = crate::jobs::run_stream(jobs, &job_key(from_id), &both, &args, |chunk| {
        pending.push_str(&String::from_utf8_lossy(chunk));
        while let Some(i) = pending.find('\n') {
            let line: String = pending.drain(..=i).collect();
            let l = line.trim().to_lowercase();
            if l.starts_with("snapshot ") && l.contains(" saved") {
                done += 1;
                report(CloneProgress { from: from_id.into(), stage: "Copiando las versiones…".into(), done, total });
            }
        }
        true
    })?;
    if out.cancelled {
        return Err("Clonado detenido. Lo ya copiado se queda en el repositorio nuevo; puedes quitarlo o volver a clonar más tarde.".into());
    }
    if out.exit_code != Some(0) {
        return Err(restic::exit_error(out.exit_code, &out.stderr));
    }
    Ok(done.max(total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rutas_dentro_del_destino() {
        assert_eq!(relative("s3:https://s3.x.backblazeb2.com/copias/portatil", None), "portatil");
        assert_eq!(relative("s3:https://s3.x.backblazeb2.com/copias/a/b/", None), "a/b");
        assert_eq!(relative("s3:https://s3.x.backblazeb2.com/copias", None), ".");
        assert_eq!(relative("rest:https://nas:8000/ana/portatil/", None), "ana/portatil");
        assert_eq!(relative("b2:copias:clientes/altamar", None), "clientes/altamar");
        assert_eq!(relative("sftp:ana@nas:/volume1/restic", None), "/volume1/restic");
    }

    #[test]
    fn encuentra_repositorios_en_carpetas() {
        let base = std::env::temp_dir().join(format!("resguardo-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        for d in ["uno", "clientes/altamar", "clientes/siigo/datos", "vacia"] {
            std::fs::create_dir_all(base.join(d)).unwrap();
        }
        for d in ["uno", "clientes/altamar"] {
            std::fs::write(base.join(d).join("config"), b"x").unwrap();
            std::fs::create_dir_all(base.join(d).join("keys")).unwrap();
        }
        let got: Vec<String> = scan_local(&base).iter().map(|p| p.strip_prefix(&base).unwrap().display().to_string().replace('\\', "/")).collect();
        assert_eq!(got, ["clientes/altamar", "uno"]);
        let _ = std::fs::remove_dir_all(base);
    }

    /// Clonado real. Requiere RESGUARDO_TEST_REPO y RESGUARDO_TEST_PASSWORD; si no, se omite.
    #[test]
    fn clona_un_repositorio_real() {
        let (Ok(location), Ok(password)) = (std::env::var("RESGUARDO_TEST_REPO"), std::env::var("RESGUARDO_TEST_PASSWORD")) else {
            eprintln!("omitido: define RESGUARDO_TEST_REPO y RESGUARDO_TEST_PASSWORD");
            return;
        };
        // Las demás pruebas reales añaden versiones a ese repositorio: que no cambie mientras se cuenta.
        let _real = crate::restic::tests::real_repo_lock();
        let src = Access::new(location, password);
        let dir = std::env::temp_dir().join(format!("resguardo-clon-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let dest = Access::new(dir.display().to_string(), "otra-contraseña-larga");
        let jobs = crate::jobs::Jobs::default();
        let mut events = 0;
        let copied = clone(&jobs, "prueba", &src, &dest, |_| events += 1).expect("debería clonar");
        let (a, b) = (restic::snapshots(&src).unwrap().len(), restic::snapshots(&dest).unwrap().len());
        assert_eq!(a, b, "todas las versiones");
        assert!(copied >= b && events >= 2);
        // Nunca sobre un repositorio que ya existe.
        assert!(clone(&jobs, "prueba", &src, &dest, |_| {}).unwrap_err().contains("Ya hay un repositorio"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn no_mezcla_dos_cuentas_de_nube() {
        let mut a = Access::new("s3:x/a", "p");
        let mut b = Access::new("s3:x/b", "q");
        a.env.push(("AWS_ACCESS_KEY_ID".into(), "uno".into()));
        b.env.push(("AWS_ACCESS_KEY_ID".into(), "dos".into()));
        assert!(conflict(&a, &b));
        b.env[0].1 = "uno".into();
        assert!(!conflict(&a, &b));
    }
}
