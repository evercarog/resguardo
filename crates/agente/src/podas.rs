//! B3 (plan 0.8.2, 15yy): lo que quitó la poda del propio equipo.
//!
//! La retención que aplica el propio almacén (`retencion_almacen.rs`: `forget` y
//! `prune` en local) o el propio equipo en un repositorio de sus discos
//! (`gestion_v2::aplicar_retencion_por`) borra y reempaqueta archivos del
//! repositorio. El espejo de ese repositorio ve después que faltan de golpe en el
//! origen y, sin saber por qué, salta su freno (`espejo_motor::Freno`): «Espejo
//! fallido», aunque lo nuevo se subiera bien.
//!
//! Aquí se anota, por repositorio, **qué archivos quitó esa poda**: la lista de
//! `data/`, `index/`, `snapshots/` y `keys/` justo antes y justo después, y la
//! diferencia. El espejo no cuenta esos archivos para su freno (siguen su
//! retención con el borrado diferido de siempre). Lo que falte por otra razón
//! (un disco dañado, alguien borrando a mano, un repositorio quitado) no está en
//! esta lista y sigue frenando igual.
//!
//! No hay secretos: rutas del propio equipo y nombres de archivo (hashes). Se
//! guarda en la carpeta privada del agente y nunca sale del equipo.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use std::path::Path;
use std::sync::Mutex;

const ARCHIVO: &str = "podas.json";
/// Lo que se recuerda una poda: un espejo parado más tiempo vuelve al freno de siempre.
pub const VIDA_DIAS: i64 = 180;
/// Podas recordadas por repositorio (las más recientes).
pub const MAX_POR_REPO: usize = 30;
/// Archivos recordados en total (los de las podas más viejas se olvidan antes).
pub const MAX_ARCHIVOS: usize = 2_000_000;
/// Para decir «tras la poda del día …»: las de estos últimos días.
pub const RECIENTE_DIAS: i64 = 14;

/// Las carpetas de un repositorio que la poda puede tocar (`config` nunca).
const CARPETAS: [&str; 4] = ["data", "index", "snapshots", "keys"];

/// Una poda de un repositorio: cuándo y qué archivos quitó (relativos a su carpeta, con «/»).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Poda {
    /// La carpeta del repositorio ([`clave`]).
    pub carpeta: String,
    /// Cuándo terminó (RFC 3339).
    pub fecha: String,
    pub archivos: Vec<String>,
}

/// Lo que se sabe de las podas de un origen del espejo: los archivos (relativos al
/// origen) que quitó alguna poda y la fecha (AAAA-MM-DD) de la más reciente.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Podado {
    pub rels: HashSet<String>,
    pub ultima: Option<String>,
}

/// Una ruta escrita siempre igual: con «/» y sin `\\?\` ni «/» al final.
pub fn clave(p: &Path) -> String {
    let s = p.display().to_string().replace('\\', "/");
    s.strip_prefix("//?/").unwrap_or(&s).trim_end_matches('/').to_string()
}

/// Para comparar dos [`clave`]: en Windows, sin distinguir mayúsculas (como sus rutas).
fn comparable(s: &str) -> String {
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s.to_string()
    }
}

/// Los archivos que puede tocar una poda en el repositorio de `carpeta` (relativos,
/// con «/»), sin seguir enlaces y sin los bloqueos. `None` si no se pudo leer.
pub fn listar(carpeta: &Path) -> Option<BTreeSet<String>> {
    if !carpeta.join("config").is_file() {
        return None;
    }
    let mut out = BTreeSet::new();
    for c in CARPETAS {
        let base = carpeta.join(c);
        if !base.is_dir() {
            continue;
        }
        let mut pendientes = vec![(base, c.to_string())];
        while let Some((dir, rel)) = pendientes.pop() {
            for e in std::fs::read_dir(&dir).ok()?.flatten() {
                let Ok(m) = std::fs::symlink_metadata(e.path()) else { continue };
                if m.file_type().is_symlink() || crate::platform::is_reparse_point(&e.path()) {
                    continue;
                }
                let r = format!("{rel}/{}", e.file_name().to_string_lossy());
                if m.is_dir() {
                    pendientes.push((e.path(), r));
                } else if m.is_file() {
                    out.insert(r);
                }
            }
        }
    }
    Some(out)
}

/// Anota en `lista` una poda de `carpeta` (lo que había antes y no después), y olvida
/// lo viejo. Sin nada quitado no se anota nada.
pub fn anotar_en(lista: &mut Vec<Poda>, carpeta: &Path, antes: &BTreeSet<String>, despues: &BTreeSet<String>, ahora: chrono::DateTime<chrono::Local>) {
    let quitados: Vec<String> = antes.difference(despues).cloned().collect();
    if !quitados.is_empty() {
        lista.push(Poda { carpeta: clave(carpeta), fecha: ahora.to_rfc3339(), archivos: quitados });
    }
    podar_lista(lista, ahora);
}

/// Fuera lo de hace más de [`VIDA_DIAS`], lo que pase de [`MAX_POR_REPO`] por repositorio
/// y, si aún son demasiados archivos, las podas más viejas.
fn podar_lista(lista: &mut Vec<Poda>, ahora: chrono::DateTime<chrono::Local>) {
    let limite = ahora - chrono::Duration::days(VIDA_DIAS);
    lista.retain(|p| chrono::DateTime::parse_from_rfc3339(&p.fecha).is_ok_and(|f| f > limite));
    lista.sort_by(|a, b| b.fecha.cmp(&a.fecha));
    let mut por_repo: std::collections::HashMap<String, usize> = Default::default();
    lista.retain(|p| {
        let n = por_repo.entry(p.carpeta.clone()).or_default();
        *n += 1;
        *n <= MAX_POR_REPO
    });
    let mut total = 0usize;
    lista.retain(|p| {
        total += p.archivos.len();
        total <= MAX_ARCHIVOS
    });
    lista.reverse();
}

/// Lo que sale de `lista` para un espejo cuyo origen es `origen`: los archivos que quitó
/// la poda de cualquier repositorio que esté dentro (o que sea el propio origen),
/// relativos al origen, y la fecha de la poda más reciente de los últimos días.
pub fn de_origen_en(lista: &[Poda], origen: &Path, ahora: chrono::DateTime<chrono::Local>) -> Podado {
    let o = comparable(&clave(origen));
    let mut r = Podado::default();
    let reciente = ahora - chrono::Duration::days(RECIENTE_DIAS);
    for p in lista {
        let c = comparable(&p.carpeta);
        let prefijo = if c == o {
            String::new()
        } else if c.starts_with(&format!("{o}/")) {
            // La ruta relativa como la escribe el espejo (sus carpetas tal cual): de la
            // carpeta tal como se anotó, salvo si al compararla cambió de largo.
            let n = o.chars().count() + 1;
            let resto: String = if c.chars().count() == p.carpeta.chars().count() { p.carpeta.chars().skip(n).collect() } else { c.chars().skip(n).collect() };
            format!("{resto}/")
        } else {
            continue;
        };
        r.rels.extend(p.archivos.iter().map(|a| format!("{prefijo}{a}")));
        if let Ok(f) = chrono::DateTime::parse_from_rfc3339(&p.fecha) {
            let f = f.with_timezone(&chrono::Local);
            if f > reciente {
                let dia = f.format("%Y-%m-%d").to_string();
                if r.ultima.as_ref().is_none_or(|u| dia > *u) {
                    r.ultima = Some(dia);
                }
            }
        }
    }
    r
}

static CANDADO: Mutex<()> = Mutex::new(());

fn archivo() -> std::path::PathBuf {
    crate::agent::private_dir().join(ARCHIVO)
}

fn leer() -> Vec<Poda> {
    std::fs::read(archivo()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn escribir(lista: &[Poda]) -> Result<(), String> {
    crate::agent::prepare_dir()?;
    let tmp = archivo().with_extension("json.tmp");
    let _ = std::fs::remove_file(&tmp);
    crate::agent::write_new(&tmp, &serde_json::to_vec(lista).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, archivo()).map_err(|e| e.to_string())
}

/// Anota (guardado) una poda de `carpeta`.
pub fn anotar(carpeta: &Path, antes: &BTreeSet<String>, despues: &BTreeSet<String>) {
    let _g = CANDADO.lock().unwrap_or_else(|e| e.into_inner());
    let mut lista = leer();
    let n = antes.difference(despues).count();
    anotar_en(&mut lista, carpeta, antes, despues, chrono::Local::now());
    match escribir(&lista) {
        Ok(()) if n > 0 => crate::agent::log(&format!("Poda: {n} archivos quitados por la retención, anotados para los espejos.")),
        Ok(()) => {}
        Err(e) => crate::agent::log(&format!("Poda: no se pudo anotar lo que quitó la retención ({e}); sus espejos la tomarán por un borrado inesperado.")),
    }
}

/// Lo que se sabe de las podas para un espejo que copia desde `origen`.
pub fn de_origen(origen: &Path) -> Podado {
    let _g = CANDADO.lock().unwrap_or_else(|e| e.into_inner());
    de_origen_en(&leer(), origen, chrono::Local::now())
}

/// Hace `f` (la poda del repositorio de `carpeta`) anotando lo que quitó. Si la carpeta
/// no es un repositorio en un disco de este equipo (una nube, un servidor), solo `f`.
/// Se anota también si la poda falla a medias: lo que borró ya no está.
pub fn con_registro<T>(carpeta: &Path, f: impl FnOnce() -> T) -> T {
    let antes = listar(carpeta);
    let r = f();
    if let (Some(antes), Some(despues)) = (antes, listar(carpeta)) {
        anotar(carpeta, &antes, &despues);
    }
    r
}

/// La carpeta de un repositorio si su dirección es un disco de este equipo.
pub fn carpeta_local(direccion: &str) -> Option<std::path::PathBuf> {
    let d = direccion.trim();
    if d.is_empty()
        || d.contains("://")
        || d.starts_with("rest:")
        || d.starts_with("s3:")
        || d.starts_with("b2:")
        || d.starts_with("sftp:")
        || d.starts_with("rclone:")
    {
        return None;
    }
    let p = std::path::PathBuf::from(d.strip_prefix("local:").unwrap_or(d));
    p.join("config").is_file().then_some(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conjunto(l: &[&str]) -> BTreeSet<String> {
        l.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn anota_lo_que_quito_y_lo_da_relativo_al_origen() {
        let ahora = chrono::Local::now();
        let mut lista = Vec::new();
        let raiz = Path::new("/almacen/zona");
        let repo = raiz.join("pc-01-3f2a/contabilidad");
        anotar_en(&mut lista, &repo, &conjunto(&["data/ab/ab1", "data/cd/cd1", "index/i1"]), &conjunto(&["data/ab/ab1", "data/ef/ef1", "index/i2"]), ahora);
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].archivos, vec!["data/cd/cd1".to_string(), "index/i1".to_string()]);
        // Desde la zona (espejo del almacén): con la carpeta del repositorio delante.
        let p = de_origen_en(&lista, raiz, ahora);
        assert!(p.rels.contains("pc-01-3f2a/contabilidad/data/cd/cd1"));
        assert!(p.rels.contains("pc-01-3f2a/contabilidad/index/i1"));
        assert!(!p.rels.contains("pc-01-3f2a/contabilidad/data/ab/ab1"));
        assert_eq!(p.ultima, Some(ahora.format("%Y-%m-%d").to_string()));
        // Desde el propio repositorio (espejo del equipo): tal cual.
        let p = de_origen_en(&lista, &repo, ahora);
        assert!(p.rels.contains("data/cd/cd1"));
        // Desde otra carpeta (que solo empieza igual): nada.
        assert!(de_origen_en(&lista, Path::new("/almacen/zona2"), ahora).rels.is_empty());
        assert!(de_origen_en(&lista, Path::new("/almacen/zo"), ahora).rels.is_empty());
        // Sin nada quitado no se anota.
        anotar_en(&mut lista, &repo, &conjunto(&["a"]), &conjunto(&["a"]), ahora);
        assert_eq!(lista.len(), 1);
    }

    #[test]
    fn olvida_lo_viejo_y_no_crece_sin_fin() {
        let ahora = chrono::Local::now();
        let mut lista =
            vec![Poda { carpeta: "/r".into(), fecha: (ahora - chrono::Duration::days(VIDA_DIAS + 1)).to_rfc3339(), archivos: vec!["data/x".into()] }];
        anotar_en(&mut lista, Path::new("/r"), &conjunto(&["data/y"]), &BTreeSet::new(), ahora);
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].archivos, vec!["data/y".to_string()]);
        for i in 0..(MAX_POR_REPO + 5) {
            anotar_en(&mut lista, Path::new("/r"), &conjunto(&[&format!("data/{i}")]), &BTreeSet::new(), ahora + chrono::Duration::seconds(i as i64));
        }
        assert_eq!(lista.len(), MAX_POR_REPO);
        // Se quedan las más recientes.
        assert!(lista.iter().any(|p| p.archivos == vec![format!("data/{}", MAX_POR_REPO + 4)]));
        // Una poda de hace más de unos días cuenta para el freno, pero no se dice «tras la poda del…».
        let vieja =
            vec![Poda { carpeta: "/r".into(), fecha: (ahora - chrono::Duration::days(RECIENTE_DIAS + 2)).to_rfc3339(), archivos: vec!["data/z".into()] }];
        let p = de_origen_en(&vieja, Path::new("/r"), ahora);
        assert!(p.rels.contains("data/z"));
        assert_eq!(p.ultima, None);
    }

    #[test]
    fn lista_solo_lo_que_toca_la_poda() {
        let base = std::env::temp_dir().join(format!("resguardo-podas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        for d in ["data/ab", "index", "snapshots", "keys", "locks"] {
            std::fs::create_dir_all(base.join(d)).unwrap();
        }
        assert_eq!(listar(&base), None, "sin config no es un repositorio");
        for f in ["config", "data/ab/ab12", "index/i1", "snapshots/s1", "keys/k1", "locks/l1"] {
            std::fs::write(base.join(f), b"x").unwrap();
        }
        assert_eq!(listar(&base).unwrap(), conjunto(&["data/ab/ab12", "index/i1", "keys/k1", "snapshots/s1"]));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn solo_carpetas_de_este_equipo() {
        assert_eq!(carpeta_local("rest:https://192.0.2.10:8000/pc-01/"), None);
        assert_eq!(carpeta_local("s3:https://s3.example.com/b"), None);
        assert_eq!(carpeta_local("rclone:nube:carpeta"), None);
        assert_eq!(carpeta_local(""), None);
        // Una carpeta que no es un repositorio, tampoco.
        assert_eq!(carpeta_local(&std::env::temp_dir().display().to_string()), None);
    }

    #[test]
    fn rutas_comparables() {
        assert_eq!(clave(Path::new("/a/b/")), "/a/b");
        #[cfg(windows)]
        {
            assert_eq!(clave(Path::new(r"\\?\D:\Copias\PC-01")), "D:/Copias/PC-01");
            // Sin distinguir mayúsculas, pero con la ruta relativa como está en el disco.
            let ahora = chrono::Local::now();
            let lista = vec![Poda { carpeta: "D:/Copias/PC-01/Conta".into(), fecha: ahora.to_rfc3339(), archivos: vec!["data/x".into()] }];
            assert!(de_origen_en(&lista, Path::new(r"d:\copias"), ahora).rels.contains("PC-01/Conta/data/x"));
        }
    }
}
