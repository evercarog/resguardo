//! Una vuelta del espejo a un destino (docs/espejo.md, «El motor»): lista el
//! origen dentro del alcance (todos los repositorios del almacén o algunos),
//! lista el destino y copia lo que falta. Sirve igual para una carpeta del
//! equipo y para una nube conectada con rclone (nube.rs), y es la pieza que
//! reutilizará el «espejo» de un repositorio como paso de una cadena (tarea 7).
//!
//! Reglas de siempre: nunca se copian los `locks` ni lo modificado hace menos
//! de [`RECIENTE`]; nunca se sigue un enlace; un archivo que ya está con el
//! mismo tamaño no se vuelve a copiar y, con otro, no se reemplaza (se avisa).
//! No necesita ninguna contraseña: no abre nada.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Lo que se ha modificado hace menos de esto se deja para la próxima vuelta.
pub const RECIENTE: Duration = Duration::from_secs(10 * 60);
/// Sufijo de lo que se está copiando a una carpeta (se renombra al terminar).
pub const SUFIJO_TEMPORAL: &str = ".tmp-espejo";

/// Qué repositorios del almacén van a un destino (§3f).
#[derive(Clone, Debug, PartialEq)]
pub enum Alcance {
    /// Todo lo que guarda el almacén (también lo que llegue después).
    Todos,
    /// Solo estos: `<usuario>` o `<usuario>/<repo>`, como en `espejo::repos_en`.
    Repos(Vec<String>),
}

/// ¿Un nombre de repositorio del almacén razonable? `<usuario>` o
/// `<usuario>/<repo>`: letras, cifras, `-`, `_` y `.`, sin empezar por punto.
pub fn repo_valido(r: &str) -> bool {
    let partes: Vec<&str> = r.split('/').collect();
    (1..=2).contains(&partes.len())
        && partes
            .iter()
            .all(|p| (1..=100).contains(&p.len()) && !p.starts_with('.') && p.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
}

impl Alcance {
    pub fn de(repos: Option<&[String]>) -> Alcance {
        match repos {
            None => Alcance::Todos,
            Some(l) => Alcance::Repos(l.to_vec()),
        }
    }

    /// Las carpetas que se recorren (relativas, con «/»): "" es todo el almacén.
    /// Sin repetir ni una dentro de otra.
    pub fn raices(&self) -> Vec<String> {
        match self {
            Alcance::Todos => vec![String::new()],
            Alcance::Repos(l) => {
                let mut v: Vec<String> = l.iter().filter(|r| repo_valido(r)).cloned().collect();
                v.sort();
                v.dedup();
                let todas = v.clone();
                v.retain(|r| !todas.iter().any(|o| o != r && r.starts_with(&format!("{o}/"))));
                v
            }
        }
    }

    /// ¿Entra este archivo (ruta relativa con «/») en el alcance?
    pub fn contiene(&self, rel: &str) -> bool {
        match self {
            Alcance::Todos => true,
            Alcance::Repos(_) => self.raices().iter().any(|r| rel.starts_with(&format!("{r}/"))),
        }
    }
}

/// Un archivo del origen o del destino.
#[derive(Clone, Debug, PartialEq)]
pub struct Archivo {
    /// Ruta relativa a la raíz, con «/».
    pub rel: String,
    pub len: u64,
    /// Modificado hace menos de [`RECIENTE`] (solo cuenta en el origen).
    pub reciente: bool,
}

/// ¿Se salta este nombre? Los bloqueos de restic y lo que está a medias.
fn ignorado(nombre: &str) -> bool {
    nombre == "locks" || nombre.ends_with(SUFIJO_TEMPORAL) || nombre.ends_with(".partial")
}

/// Los archivos de una carpeta (el almacén o una carpeta del espejo) dentro
/// del alcance, sin seguir enlaces. Una raíz que no existe no tiene nada.
pub fn listar_carpeta(raiz: &Path, alcance: &Alcance) -> Result<Vec<Archivo>, String> {
    let ahora = SystemTime::now();
    let mut out = Vec::new();
    for r in alcance.raices() {
        let base = if r.is_empty() { raiz.to_path_buf() } else { raiz.join(&r) };
        if !base.is_dir() || crate::platform::is_reparse_point(&base) {
            continue;
        }
        let mut pendientes: Vec<(PathBuf, String)> = vec![(base, r.clone())];
        while let Some((dir, rel)) = pendientes.pop() {
            for e in std::fs::read_dir(&dir).map_err(|e| format!("No se pudo leer {}: {e}", dir.display()))?.flatten() {
                let nombre = e.file_name().to_string_lossy().into_owned();
                if ignorado(&nombre) {
                    continue;
                }
                let Ok(m) = std::fs::symlink_metadata(e.path()) else { continue };
                if m.file_type().is_symlink() || crate::platform::is_reparse_point(&e.path()) {
                    continue;
                }
                let rel_e = if rel.is_empty() { nombre } else { format!("{rel}/{nombre}") };
                if m.is_dir() {
                    pendientes.push((e.path(), rel_e));
                } else if m.is_file() {
                    let reciente = m.modified().ok().and_then(|t| ahora.duration_since(t).ok()).is_none_or(|d| d < RECIENTE);
                    out.push(Archivo { rel: rel_e, len: m.len(), reciente });
                }
            }
        }
    }
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(out)
}

/// Adónde va una vuelta.
pub enum Lado<'a> {
    /// Una carpeta de un disco del equipo (ya comprobada por quien llama).
    Carpeta(&'a Path),
    /// Una nube conectada (nube.rs), en `carpeta` dentro de ella. `trabajo`:
    /// carpeta privada donde dejar las listas de archivos para rclone.
    Nube { nube: &'a crate::nube::Nube, carpeta: &'a str, trabajo: &'a Path, limite_kib: Option<u32> },
}

#[derive(Debug, Default, PartialEq)]
pub struct Resumen {
    pub copiados: u64,
    pub bytes: u64,
    pub iguales: u64,
    pub recientes: u64,
    /// Ya estaban en el espejo con otro tamaño: no se reemplazan.
    pub distintos: u64,
    /// Repositorios elegidos que ya no están en el almacén.
    pub faltan_repos: Vec<String>,
}

/// Lo que hay en el destino (ruta relativa → tamaño), dentro del alcance.
pub fn listar_destino(lado: &Lado, alcance: &Alcance) -> Result<BTreeMap<String, u64>, String> {
    match lado {
        Lado::Carpeta(d) => Ok(listar_carpeta(d, alcance)?.into_iter().map(|a| (a.rel, a.len)).collect()),
        Lado::Nube { nube, carpeta, trabajo, .. } => {
            let mut m = BTreeMap::new();
            for r in alcance.raices() {
                for (rel, len) in crate::nube::listar(nube, carpeta, &r, trabajo)? {
                    let nombre = rel.rsplit('/').next().unwrap_or_default();
                    if !ignorado(nombre) && !rel.split('/').any(|p| p == "locks") {
                        m.insert(rel, len);
                    }
                }
            }
            Ok(m)
        }
    }
}

/// Una vuelta: copia al destino lo que le falta del origen (dentro del alcance).
/// `avance(leído, escrito)`: lo copiado hasta ahora (carpeta) o los ritmos de rclone (nube).
pub fn vuelta(origen: &Path, lado: &Lado, alcance: &Alcance, avance: &mut dyn FnMut(Option<u64>, Option<u64>)) -> Result<Resumen, String> {
    let mut r = Resumen::default();
    if let Alcance::Repos(l) = alcance {
        r.faltan_repos = l.iter().filter(|x| !repo_valido(x) || !origen.join(x.as_str()).join("config").is_file()).cloned().collect();
    }
    let origen_l = listar_carpeta(origen, alcance)?;
    let destino = listar_destino(lado, alcance)?;
    let mut copiar: Vec<&Archivo> = Vec::new();
    for a in &origen_l {
        if a.reciente {
            r.recientes += 1;
            continue;
        }
        match destino.get(&a.rel) {
            // Nunca se reescribe lo que ya está: los archivos de restic no
            // cambian, así que otro tamaño es un daño (en el origen o aquí).
            Some(l) if *l == a.len => r.iguales += 1,
            Some(_) => r.distintos += 1,
            None => copiar.push(a),
        }
    }
    if copiar.is_empty() {
        return Ok(r);
    }
    match lado {
        Lado::Carpeta(d) => copiar_a_carpeta(origen, d, &copiar, &mut r, avance)?,
        Lado::Nube { nube, carpeta, trabajo, limite_kib } => {
            let rels: Vec<&str> = copiar.iter().map(|a| a.rel.as_str()).collect();
            let (n, bytes) = crate::nube::copiar_lista(nube, origen, carpeta, &rels, *limite_kib, trabajo, avance)?;
            r.copiados += n;
            r.bytes += bytes;
        }
    }
    Ok(r)
}

/// ¿Hay un enlace en el camino de `rel` dentro de `destino`? (cada carpeta se mira una vez)
fn enlace_en_el_camino(destino: &Path, rel: &str, vistas: &mut HashSet<PathBuf>) -> Option<PathBuf> {
    let mut p = destino.to_path_buf();
    for parte in rel.split('/') {
        p.push(parte);
        if vistas.contains(&p) {
            continue;
        }
        if crate::platform::is_reparse_point(&p) {
            return Some(p);
        }
        if p.is_dir() {
            vistas.insert(p.clone());
        }
    }
    None
}

fn copiar_a_carpeta(
    origen: &Path,
    destino: &Path,
    archivos: &[&Archivo],
    r: &mut Resumen,
    avance: &mut dyn FnMut(Option<u64>, Option<u64>),
) -> Result<(), String> {
    let mut vistas = HashSet::new();
    for a in archivos {
        // Nunca a través de un enlace (desviaría lo que escribe SYSTEM).
        if let Some(p) = enlace_en_el_camino(destino, &a.rel, &mut vistas) {
            return Err(format!("{} es un enlace: el espejo no escribe a través de enlaces.", p.display()));
        }
        let dest = destino.join(&a.rel);
        if let Some(p) = dest.parent() {
            std::fs::create_dir_all(p).map_err(|e| format!("No se pudo crear {}: {e}", p.display()))?;
        }
        let tmp = dest.with_file_name(format!("{}{SUFIJO_TEMPORAL}", dest.file_name().unwrap_or_default().to_string_lossy()));
        // Un temporal que ya estuviera (o un enlace con su nombre) se quita antes.
        if std::fs::symlink_metadata(&tmp).is_ok() {
            std::fs::remove_file(&tmp).map_err(|e| format!("No se pudo quitar {}: {e}", tmp.display()))?;
        }
        if let Err(err) = std::fs::copy(origen.join(&a.rel), &tmp) {
            // Lo copiado a medias no se queda (en un disco lleno, ocuparía lo poco que queda).
            let _ = std::fs::remove_file(&tmp);
            return Err(error_al_copiar(&err, destino));
        }
        std::fs::rename(&tmp, &dest).map_err(|e| format!("No se pudo terminar {}: {e}", dest.display()))?;
        r.copiados += 1;
        r.bytes += a.len;
        avance(Some(r.bytes), None);
    }
    Ok(())
}

/// El motivo de un archivo que no se pudo copiar al espejo, con qué hacer.
pub fn error_al_copiar(e: &std::io::Error, destino: &Path) -> String {
    let lleno = e.kind() == std::io::ErrorKind::StorageFull
        || e.raw_os_error() == Some(if cfg!(windows) { 112 } else { 28 })
        || resguardo_motor::restic::sin_espacio(&e.to_string().to_lowercase());
    if lleno {
        format!(
            "no queda espacio en el disco del espejo ({}). Libera espacio en él o elige otra carpeta con más sitio; \
             lo que ya está en el espejo se conserva y lo que falta se copiará en la próxima vuelta.",
            destino.display()
        )
    } else {
        format!("no se pudo copiar a {}: {e}", destino.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nombres_y_alcance() {
        for bien in ["ana", "ana/contabilidad", "srv-01/sql_2.bak"] {
            assert!(repo_valido(bien), "{bien}");
        }
        for mal in ["", "..", "ana/..", "ana/.oculto", "a/b/c", "ana\\x", "ana/", "/ana", "c:x", "ana/con espacio"] {
            assert!(!repo_valido(mal), "{mal}");
        }
        let a = Alcance::Repos(vec!["ana/fotos".into(), "ana".into(), "srv/sql".into(), "srv/sql".into(), "../x".into()]);
        assert_eq!(a.raices(), vec!["ana", "srv/sql"], "sin repetir, sin una dentro de otra y sin nombres raros");
        assert!(a.contiene("ana/fotos/data/ab/abc") && a.contiene("srv/sql/config"));
        assert!(!a.contiene("srv/sqlite/config") && !a.contiene("srv/config") && !a.contiene("anabel/x"));
        assert!(Alcance::Todos.contiene("cualquier/cosa"));
        assert_eq!(Alcance::de(None), Alcance::Todos);
    }

    /// §3f: solo los repositorios elegidos, cada uno en su carpeta.
    #[test]
    fn solo_los_repositorios_elegidos() {
        let base = std::env::temp_dir().join(format!("resguardo-espejo-alcance-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (o, d) = (base.join("origen"), base.join("destino"));
        let viejo = SystemTime::now() - Duration::from_secs(3600);
        for f in ["ana/contabilidad/config", "ana/contabilidad/data/ab/abcd", "ana/fotos/config", "srv/config", "srv/data/cd/cdef"] {
            std::fs::create_dir_all(o.join(f).parent().unwrap()).unwrap();
            std::fs::write(o.join(f), f.as_bytes()).unwrap();
            std::fs::File::options().write(true).open(o.join(f)).unwrap().set_modified(viejo).unwrap();
        }
        let alcance = Alcance::Repos(vec!["ana/contabilidad".into(), "srv".into(), "ya/no-esta".into()]);
        let r = vuelta(&o, &Lado::Carpeta(&d), &alcance, &mut |_, _| {}).unwrap();
        assert_eq!((r.copiados, r.faltan_repos.clone()), (4, vec!["ya/no-esta".to_string()]));
        assert!(d.join("ana/contabilidad/data/ab/abcd").is_file() && d.join("srv/data/cd/cdef").is_file());
        assert!(!d.join("ana/fotos").exists(), "lo no elegido no se copia");
        // Con todos, entra lo que faltaba.
        let r = vuelta(&o, &Lado::Carpeta(&d), &Alcance::Todos, &mut |_, _| {}).unwrap();
        assert_eq!((r.copiados, r.iguales), (1, 4));
        let _ = std::fs::remove_dir_all(&base);
    }
}
