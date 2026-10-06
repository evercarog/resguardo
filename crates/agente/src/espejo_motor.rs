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
//!
//! Verificación sin contraseñas (§3d): los archivos de restic de `data/`,
//! `index/`, `snapshots/` y `keys/` se llaman como el SHA-256 de su
//! contenido. Antes de copiar uno se comprueba (uno dañado en el almacén no
//! se propaga) y, una vez al día, se comprueba una parte de lo que ya está en
//! el destino, siguiendo donde se quedó la vez anterior. En una carpeta, lo
//! dañado del espejo se repara con el del almacén si ese está bien.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Lo que se ha modificado hace menos de esto se deja para la próxima vuelta.
pub const RECIENTE: Duration = Duration::from_secs(10 * 60);
/// Sufijo de lo que se está copiando a una carpeta (se renombra al terminar).
pub const SUFIJO_TEMPORAL: &str = ".tmp-espejo";

/// Lo que se espera entre dos verificaciones del destino por rotación.
pub const CADA_VERIFICACION: Duration = Duration::from_secs(20 * 3600);

/// El hash de un archivo de restic por su nombre (`data/ab/<hash>`,
/// `index|snapshots|keys/<hash>`), si tiene esa forma. `config` y lo demás, no.
pub fn hash_del_nombre(rel: &str) -> Option<&str> {
    let p: Vec<&str> = rel.split('/').collect();
    let n = p.len();
    let nombre = *p.last()?;
    if nombre.len() != 64 || !nombre.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return None;
    }
    let en_data = n >= 3 && p[n - 3] == "data" && p[n - 2].len() == 2 && nombre.starts_with(p[n - 2]);
    let en_otra = n >= 2 && matches!(p[n - 2], "index" | "snapshots" | "keys");
    (en_data || en_otra).then_some(nombre)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// El SHA-256 de un archivo (leído por trozos).
pub fn sha256_de(p: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(p)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize()))
}

/// ¿Está bien el archivo `p` (relativo `rel`)? `None` si su nombre no es un hash
/// (no se puede saber); un archivo que no se puede leer, mal.
fn esta_bien(p: &Path, rel: &str) -> Option<bool> {
    hash_del_nombre(rel).map(|h| sha256_de(p).is_ok_and(|x| x == h))
}

/// Lo que se configura de una vuelta.
#[derive(Clone, Debug, Default)]
pub struct Opciones {
    /// % de los archivos del destino que se comprueban (0: ninguno).
    pub verificar_pct: u8,
}

/// Lo que una vuelta recuerda para la siguiente (en la carpeta privada del agente).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Estado {
    /// El último archivo comprobado del destino (la rotación sigue desde ahí).
    #[serde(default)]
    pub cursor: Option<String>,
    /// Cuándo se comprobó el destino por última vez (RFC 3339).
    #[serde(default)]
    pub verificado: Option<String>,
}

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
    /// §3d: archivos del almacén cuyo contenido no cuadra con su nombre (no se copian).
    pub danados_origen: Vec<String>,
    /// §3d: archivos del espejo dañados que se han vuelto a copiar bien.
    pub reparados: u64,
    /// §3d: archivos del espejo comprobados en esta vuelta (por rotación).
    pub verificados: u64,
    /// §3d: archivos del espejo dañados que no se han podido reparar.
    pub mal_destino: Vec<String>,
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

/// Una vuelta: copia al destino lo que le falta del origen (dentro del alcance)
/// y comprueba una parte del destino. `avance(leído, escrito)`: lo copiado
/// hasta ahora (carpeta) o los ritmos de rclone (nube).
pub fn vuelta(
    origen: &Path,
    lado: &Lado,
    alcance: &Alcance,
    op: &Opciones,
    estado: &mut Estado,
    avance: &mut dyn FnMut(Option<u64>, Option<u64>),
) -> Result<Resumen, String> {
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
            Some(l) if *l == a.len => r.iguales += 1,
            // Los archivos de restic no cambian: otro tamaño es un daño, en el
            // origen o aquí. Con su nombre se sabe cuál (§3d).
            Some(_) => match esta_bien(&origen.join(&a.rel), &a.rel) {
                None => r.distintos += 1,
                Some(false) => r.danados_origen.push(a.rel.clone()),
                Some(true) => match lado {
                    Lado::Carpeta(d) => {
                        reparar(origen, d, &a.rel)?;
                        r.reparados += 1;
                    }
                    Lado::Nube { .. } => r.mal_destino.push(a.rel.clone()),
                },
            },
            None => copiar.push(a),
        }
    }
    match lado {
        Lado::Carpeta(d) => copiar_a_carpeta(origen, d, &copiar, &mut r, avance)?,
        Lado::Nube { nube, carpeta, trabajo, limite_kib } => {
            // Antes de subir, cada archivo se comprueba: uno dañado no sale del almacén.
            let mut rels: Vec<&str> = Vec::new();
            for a in &copiar {
                if esta_bien(&origen.join(&a.rel), &a.rel) == Some(false) {
                    r.danados_origen.push(a.rel.clone());
                } else {
                    rels.push(a.rel.as_str());
                }
            }
            let (n, bytes) = crate::nube::copiar_lista(nube, origen, carpeta, &rels, *limite_kib, trabajo, avance)?;
            r.copiados += n;
            r.bytes += bytes;
        }
    }
    verificar(origen, lado, &destino, op, estado, &mut r)?;
    Ok(r)
}

/// Los archivos del destino que tocan esta vez: el `pct` % de los que tienen
/// nombre de hash (al menos uno), siguiendo por orden después de `cursor`.
pub fn rotacion<'a>(candidatos: &[&'a str], pct: u8, cursor: Option<&str>) -> Vec<&'a str> {
    let n = candidatos.len();
    if pct == 0 || n == 0 {
        return Vec::new();
    }
    let k = (n * usize::from(pct.min(100))).div_ceil(100).clamp(1, n);
    let inicio = cursor.map_or(0, |c| candidatos.iter().position(|x| *x > c).unwrap_or(0));
    (0..k).map(|i| candidatos[(inicio + i) % n]).collect()
}

/// §3d: una vez al día, el `verificar_pct` % de lo que ya estaba en el destino.
fn verificar(origen: &Path, lado: &Lado, destino: &BTreeMap<String, u64>, op: &Opciones, estado: &mut Estado, r: &mut Resumen) -> Result<(), String> {
    let toca = estado
        .verificado
        .as_deref()
        .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
        .is_none_or(|v| chrono::Utc::now().signed_duration_since(v).to_std().unwrap_or_default() >= CADA_VERIFICACION);
    if op.verificar_pct == 0 || !toca {
        return Ok(());
    }
    let candidatos: Vec<&str> = destino.keys().map(String::as_str).filter(|k| hash_del_nombre(k).is_some()).collect();
    let tocan = rotacion(&candidatos, op.verificar_pct, estado.cursor.as_deref());
    if tocan.is_empty() {
        return Ok(());
    }
    let malos: Vec<&str> = match lado {
        Lado::Carpeta(d) => tocan.iter().copied().filter(|rel| esta_bien(&d.join(rel), rel) == Some(false)).collect(),
        Lado::Nube { nube, carpeta, trabajo, .. } => {
            let hashes = crate::nube::hashes(nube, carpeta, &tocan, trabajo)?;
            tocan.iter().copied().filter(|rel| hashes.get(*rel).map(String::as_str) != hash_del_nombre(rel)).collect()
        }
    };
    for rel in malos {
        match lado {
            // Se repara con el del almacén, si está y está bien.
            Lado::Carpeta(d) if esta_bien(&origen.join(rel), rel) == Some(true) => {
                reparar(origen, d, rel)?;
                r.reparados += 1;
            }
            _ => r.mal_destino.push(rel.to_string()),
        }
    }
    r.verificados = tocan.len() as u64;
    estado.cursor = tocan.last().map(|s| s.to_string());
    estado.verificado = Some(chrono::Utc::now().to_rfc3339());
    Ok(())
}

/// Copia `src` a `dst` calculando su SHA-256 a la vez. Devuelve el hash.
fn copiar_con_hash(src: &Path, dst: &Path) -> std::io::Result<String> {
    use std::io::{Read, Write};
    let mut f = std::fs::File::open(src)?;
    let mut o = std::fs::File::create(dst)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        o.write_all(&buf[..n])?;
    }
    o.sync_all()?;
    Ok(hex(&h.finalize()))
}

/// El temporal de un archivo del destino (se renombra al terminar).
fn temporal(dest: &Path) -> PathBuf {
    dest.with_file_name(format!("{}{SUFIJO_TEMPORAL}", dest.file_name().unwrap_or_default().to_string_lossy()))
}

/// Vuelve a copiar un archivo dañado del espejo con el del almacén (ya comprobado).
fn reparar(origen: &Path, destino: &Path, rel: &str) -> Result<(), String> {
    if let Some(p) = enlace_en_el_camino(destino, rel, &mut HashSet::new()) {
        return Err(format!("{} es un enlace: el espejo no escribe a través de enlaces.", p.display()));
    }
    let dest = destino.join(rel);
    let tmp = temporal(&dest);
    let _ = std::fs::remove_file(&tmp);
    match copiar_con_hash(&origen.join(rel), &tmp) {
        Ok(h) if Some(h.as_str()) == hash_del_nombre(rel) => std::fs::rename(&tmp, &dest).map_err(|e| format!("No se pudo reparar {}: {e}", dest.display())),
        Ok(_) => {
            let _ = std::fs::remove_file(&tmp);
            Err(format!("no se pudo reparar {rel}: el del almacén cambió mientras se copiaba."))
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(error_al_copiar(&e, destino))
        }
    }
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
        let tmp = temporal(&dest);
        // Un temporal que ya estuviera (o un enlace con su nombre) se quita antes.
        if std::fs::symlink_metadata(&tmp).is_ok() {
            std::fs::remove_file(&tmp).map_err(|e| format!("No se pudo quitar {}: {e}", tmp.display()))?;
        }
        let hash = match copiar_con_hash(&origen.join(&a.rel), &tmp) {
            Ok(h) => h,
            Err(err) => {
                // Lo copiado a medias no se queda (en un disco lleno, ocuparía lo poco que queda).
                let _ = std::fs::remove_file(&tmp);
                return Err(error_al_copiar(&err, destino));
            }
        };
        // §3d: si su contenido no cuadra con su nombre, está dañado en el almacén: no se propaga.
        if hash_del_nombre(&a.rel).is_some_and(|h| h != hash) {
            let _ = std::fs::remove_file(&tmp);
            r.danados_origen.push(a.rel.clone());
            continue;
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

    /// Un archivo con nombre de hash de verdad (su SHA-256) y fecha antigua.
    fn escribir(raiz: &Path, carpeta: &str, contenido: &[u8]) -> String {
        let h = hex(&Sha256::digest(contenido));
        let rel = if carpeta.ends_with("data") { format!("{carpeta}/{}/{h}", &h[..2]) } else { format!("{carpeta}/{h}") };
        let p = raiz.join(&rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, contenido).unwrap();
        std::fs::File::options().write(true).open(&p).unwrap().set_modified(SystemTime::now() - Duration::from_secs(3600)).unwrap();
        rel
    }
    fn estropear(p: &Path) {
        let mut b = std::fs::read(p).unwrap();
        b[0] ^= 0xff;
        std::fs::write(p, b).unwrap();
        std::fs::File::options().write(true).open(p).unwrap().set_modified(SystemTime::now() - Duration::from_secs(3600)).unwrap();
    }

    #[test]
    fn nombres_con_hash_y_rotacion() {
        let h = "ab".to_string() + &"0".repeat(62);
        assert_eq!(hash_del_nombre(&format!("ana/r/data/ab/{h}")), Some(h.as_str()));
        assert_eq!(hash_del_nombre(&format!("ana/r/snapshots/{h}")), Some(h.as_str()));
        assert_eq!(hash_del_nombre(&format!("r/keys/{h}")), Some(h.as_str()));
        assert_eq!(hash_del_nombre(&format!("ana/r/data/cd/{h}")), None, "la carpeta de data es el principio del hash");
        assert_eq!(hash_del_nombre("ana/r/config"), None);
        assert_eq!(hash_del_nombre(&format!("ana/r/otra/{h}")), None);
        assert_eq!(hash_del_nombre(&format!("ana/r/index/{}", h.to_uppercase())), None);
        let c = ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"];
        assert_eq!(rotacion(&c, 0, None), Vec::<&str>::new());
        assert_eq!(rotacion(&c, 5, None), vec!["a"], "al menos uno");
        assert_eq!(rotacion(&c, 20, None), vec!["a", "b"]);
        assert_eq!(rotacion(&c, 20, Some("b")), vec!["c", "d"], "sigue donde lo dejó");
        assert_eq!(rotacion(&c, 30, Some("i")), vec!["j", "a", "b"], "y vuelve a empezar");
        assert_eq!(rotacion(&c, 20, Some("zz")), vec!["a", "b"]);
        assert_eq!(rotacion(&c, 100, None).len(), 10);
    }

    /// §3d: los nombres de los archivos de restic son el SHA-256 de su contenido
    /// (comprobado con el restic que acompaña al agente).
    #[test]
    fn nombres_de_restic_son_su_sha256() {
        if crate::restic::version().is_err() {
            eprintln!("Sin restic: se salta la prueba.");
            return;
        }
        let _l = crate::restic::tests::real_repo_lock();
        let base = std::env::temp_dir().join(format!("resguardo-espejo-restic-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("datos")).unwrap();
        std::fs::write(base.join("datos/facturas.txt"), "factura 1\n".repeat(5000)).unwrap();
        let acc = crate::restic::Access::new(base.join("repo").display().to_string(), "clave de prueba del espejo");
        for args in [vec!["init"], vec!["backup", base.join("datos").to_str().unwrap()]] {
            let out = crate::restic::run_raw(&acc, &args, Duration::from_secs(120)).unwrap();
            assert_eq!(out.code, Some(0), "{}", out.stderr);
        }
        let todos = listar_carpeta(&base.join("repo"), &Alcance::Todos).unwrap();
        let mut por_carpeta = std::collections::BTreeMap::new();
        for a in &todos {
            if let Some(h) = hash_del_nombre(&a.rel) {
                assert_eq!(sha256_de(&base.join("repo").join(&a.rel)).unwrap(), h, "{}", a.rel);
                *por_carpeta.entry(a.rel.split('/').next().unwrap().to_string()).or_insert(0) += 1;
            }
        }
        assert_eq!(por_carpeta.keys().cloned().collect::<Vec<_>>(), vec!["data", "index", "keys", "snapshots"]);
        assert!(todos.iter().any(|a| a.rel == "config") && hash_del_nombre("config").is_none());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// §3d: un archivo dañado en el almacén no se copia; uno dañado en el espejo se encuentra y se repara.
    #[test]
    fn no_propaga_lo_danado_y_repara_el_espejo() {
        let base = std::env::temp_dir().join(format!("resguardo-espejo-hash-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (o, d) = (base.join("origen"), base.join("destino"));
        let bueno = escribir(&o, "ana/r/data", b"paquete bueno");
        let malo = escribir(&o, "ana/r/data", b"paquete que se estropea");
        let snap = escribir(&o, "ana/r/snapshots", b"version");
        estropear(&o.join(&malo));
        let op = Opciones { verificar_pct: 100 };
        let mut est = Estado::default();
        let r = vuelta(&o, &Lado::Carpeta(&d), &Alcance::Todos, &op, &mut est, &mut |_, _| {}).unwrap();
        assert_eq!((r.copiados, r.danados_origen.clone()), (2, vec![malo.clone()]));
        assert!(d.join(&bueno).is_file() && !d.join(&malo).exists(), "lo dañado no se propaga");
        assert!(std::fs::read_dir(d.join("ana/r/data")).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().ends_with(SUFIJO_TEMPORAL)));
        assert!(crate::espejo::texto_de(&r).unwrap_err().contains("dañados en el almacén"));
        // El espejo se estropea (mismo tamaño): la comprobación de cada día lo encuentra y lo repara.
        std::fs::remove_file(o.join(&malo)).unwrap();
        estropear(&d.join(&bueno));
        let mut est = Estado::default();
        let r = vuelta(&o, &Lado::Carpeta(&d), &Alcance::Todos, &op, &mut est, &mut |_, _| {}).unwrap();
        assert_eq!((r.verificados, r.reparados, r.mal_destino.len()), (2, 1, 0));
        assert_eq!(std::fs::read(d.join(&bueno)).unwrap(), b"paquete bueno");
        assert!(est.verificado.is_some() && est.cursor.is_some());
        // Ya comprobado hoy: la siguiente vuelta no vuelve a leer el destino.
        let r = vuelta(&o, &Lado::Carpeta(&d), &Alcance::Todos, &op, &mut est, &mut |_, _| {}).unwrap();
        assert_eq!(r.verificados, 0);
        // Otro tamaño en el espejo y el del almacén bien: se repara al momento.
        std::fs::write(d.join(&snap), b"x").unwrap();
        let r = vuelta(&o, &Lado::Carpeta(&d), &Alcance::Todos, &op, &mut est, &mut |_, _| {}).unwrap();
        assert_eq!((r.reparados, r.distintos), (1, 0));
        assert_eq!(std::fs::read(d.join(&snap)).unwrap(), b"version");
        // Sin el del almacén (ya podado) no se puede reparar: queda dicho.
        estropear(&d.join(&snap));
        std::fs::remove_file(o.join(&snap)).unwrap();
        let mut est = Estado::default();
        let r = vuelta(&o, &Lado::Carpeta(&d), &Alcance::Todos, &op, &mut est, &mut |_, _| {}).unwrap();
        assert_eq!(r.mal_destino, vec![snap.clone()]);
        assert!(crate::espejo::texto_de(&r).unwrap_err().contains("no se han podido reparar"));
        let _ = std::fs::remove_dir_all(&base);
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
        let r = vuelta(&o, &Lado::Carpeta(&d), &alcance, &Opciones::default(), &mut Estado::default(), &mut |_, _| {}).unwrap();
        assert_eq!((r.copiados, r.faltan_repos.clone()), (4, vec!["ya/no-esta".to_string()]));
        assert!(d.join("ana/contabilidad/data/ab/abcd").is_file() && d.join("srv/data/cd/cdef").is_file());
        assert!(!d.join("ana/fotos").exists(), "lo no elegido no se copia");
        // Con todos, entra lo que faltaba.
        let r = vuelta(&o, &Lado::Carpeta(&d), &Alcance::Todos, &Opciones::default(), &mut Estado::default(), &mut |_, _| {}).unwrap();
        assert_eq!((r.copiados, r.iguales), (1, 4));
        let _ = std::fs::remove_dir_all(&base);
    }
}
