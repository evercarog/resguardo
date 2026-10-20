//! 0.8.1 (15gg): repositorios **sueltos** en el disco de un almacén (p. ej. los de un
//! rest-server viejo en el mismo equipo) y «Dar a un equipo».
//!
//! - **Buscar** (sin abrirlos ni leer nada de dentro): en la carpeta de cada zona, las
//!   carpetas que tienen `config`, `data`, `index`, `keys` y `snapshots` y **no** están en
//!   la carpeta de ninguno de sus equipos (el primer nivel no es un usuario de esa zona),
//!   hasta dos niveles; además, en las carpetas que se le indiquen (`sueltos_en`, fuera del
//!   almacén: solo se enseñan, no se mueven). Nunca sigue enlaces. Van en el resumen del
//!   almacén (`guarda_copias.sueltos`), solo con su carpeta relativa, cuántas versiones
//!   tiene (cuántos archivos hay en `snapshots/`) y la fecha del más reciente.
//! - **Dar a un equipo** (`guarda_copias { dar_suelto: { zona?, ruta, usuario, carpeta } }`,
//!   con la clave de administración): lo mueve dentro de la misma zona a
//!   `<usuario>/<carpeta>` con un `rename` (mismo disco, sin copiar nada). Si el sistema no
//!   puede (otro disco debajo de la carpeta), no hace nada y lo dice. Después, el equipo lo
//!   adopta con su contraseña (`adoptar_repositorio`, la consola lo ofrece seguido).

use crate::server::ServerConfig;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Como mucho, repositorios sueltos que se enseñan.
pub const MAX_SUELTOS: usize = 100;
/// Como mucho, carpetas de más donde buscar.
pub const MAX_CARPETAS_EXTRA: usize = 10;

/// ¿Es la carpeta de un repositorio? Solo mira que estén sus partes (sin abrirlo).
pub fn es_repo(d: &Path) -> bool {
    !crate::platform::is_reparse_point(d) && d.join("config").is_file() && ["data", "index", "keys", "snapshots"].iter().all(|x| d.join(x).is_dir())
}

/// Subcarpetas (sin enlaces), ordenadas y como mucho 200.
fn subcarpetas(d: &Path) -> Vec<(String, PathBuf)> {
    let Ok(rd) = std::fs::read_dir(d) else { return Vec::new() };
    let mut v: Vec<(String, PathBuf)> = rd
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()) && !crate::platform::is_reparse_point(&e.path()))
        .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
        .collect();
    v.sort();
    v.truncate(200);
    v
}

/// Cuántas versiones (archivos en `snapshots/`) y la fecha de la más reciente.
fn versiones(d: &Path) -> (usize, Option<String>) {
    let Ok(rd) = std::fs::read_dir(d.join("snapshots")) else { return (0, None) };
    let mut n = 0usize;
    let mut ultima: Option<std::time::SystemTime> = None;
    for e in rd.flatten().take(1_000_000) {
        let Ok(m) = e.metadata() else { continue };
        if !m.is_file() {
            continue;
        }
        n += 1;
        if let Ok(t) = m.modified() {
            ultima = Some(ultima.map_or(t, |u| u.max(t)));
        }
    }
    (n, ultima.map(|t| chrono::DateTime::<chrono::Local>::from(t).to_rfc3339()))
}

/// Las zonas del almacén: (id o `None` para la principal, carpeta, usuarios).
fn zonas(c: &ServerConfig) -> Vec<(Option<String>, String, Vec<String>)> {
    std::iter::once((None, c.path.clone(), c.users.iter().map(|u| u.name.clone()).collect()))
        .chain(c.zonas.iter().map(|z| (Some(z.id.clone()), z.path.clone(), z.users.iter().map(|u| u.name.clone()).collect())))
        .collect()
}

/// Los repositorios sueltos de una carpeta: (ruta relativa con `/`, carpeta).
fn sueltos_en(raiz: &Path, usuarios: &[String]) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    for (a, da) in subcarpetas(raiz) {
        // La carpeta de un equipo (o la de un renombre a medias) no es suelta.
        if usuarios.iter().any(|u| u.eq_ignore_ascii_case(&a)) || a.starts_with('.') {
            continue;
        }
        if es_repo(&da) {
            out.push((a, da));
            continue;
        }
        for (b, db) in subcarpetas(&da) {
            if es_repo(&db) {
                out.push((format!("{a}/{b}"), db));
            }
        }
        if out.len() >= MAX_SUELTOS {
            break;
        }
    }
    out
}

/// Lo que va en el resumen del almacén (`guarda_copias.sueltos`), o `null` si no hay ninguno.
pub fn resumen(c: &ServerConfig) -> Value {
    let mut l: Vec<Value> = Vec::new();
    for (zona, raiz, usuarios) in zonas(c) {
        for (ruta, d) in sueltos_en(Path::new(&raiz), &usuarios) {
            let (n, ultima) = versiones(&d);
            l.push(json!({ "zona": zona, "ruta": ruta, "versiones": n, "ultima": ultima }));
        }
    }
    for extra in c.sueltos_en.iter().take(MAX_CARPETAS_EXTRA) {
        let raiz = Path::new(extra);
        if crate::platform::hay_enlace_en_el_camino(raiz) {
            continue;
        }
        let mut aqui: Vec<(String, PathBuf)> = if es_repo(raiz) { vec![(String::new(), raiz.to_path_buf())] } else { sueltos_en(raiz, &[]) };
        aqui.truncate(MAX_SUELTOS);
        for (ruta, d) in aqui {
            let (n, ultima) = versiones(&d);
            // Fuera del almacén: con su ruta entera (la escribió la consola), y no se mueve.
            l.push(json!({ "fuera": true, "ruta": d.to_string_lossy(), "rel": ruta, "versiones": n, "ultima": ultima }));
        }
    }
    l.truncate(MAX_SUELTOS);
    if l.is_empty() {
        Value::Null
    } else {
        Value::Array(l)
    }
}

/// `guarda_copias { sueltos_en: [rutas] }` (clave de administración): dónde buscar además
/// (fuera de la carpeta del almacén; vacía, ninguna). Rutas completas, que existan, sin
/// enlaces y que no sean del sistema.
pub fn poner_busqueda(c: &mut ServerConfig, v: &Value) -> Result<String, String> {
    let rutas: Vec<String> = v
        .as_array()
        .ok_or("Carpetas donde buscar no válidas.")?
        .iter()
        .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
        .collect();
    if rutas.len() > MAX_CARPETAS_EXTRA {
        return Err(format!("Como mucho {MAX_CARPETAS_EXTRA} carpetas donde buscar."));
    }
    for r in &rutas {
        let p = Path::new(r);
        if r.chars().any(char::is_control) || !p.is_absolute() || !p.is_dir() {
            return Err(format!("«{r}» no es una carpeta de este equipo."));
        }
        if crate::platform::hay_enlace_en_el_camino(p) || crate::server::carpeta_del_sistema(p) {
            return Err(format!("«{r}» no vale: es un enlace o una carpeta del sistema."));
        }
    }
    c.sueltos_en = rutas;
    Ok(match c.sueltos_en.len() {
        0 => "Ya no se buscan repositorios fuera de la carpeta del almacén.".to_string(),
        1 => "Se buscan repositorios también en 1 carpeta más.".to_string(),
        n => format!("Se buscan repositorios también en {n} carpetas más."),
    })
}

/// Un trozo de ruta válido (ni vacío, ni `.`/`..`, ni separadores ni caracteres de control).
fn trozo_valido(t: &str) -> bool {
    !t.is_empty()
        && t != "."
        && t != ".."
        && t.chars().count() <= 120
        && !t.chars().any(|c| c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
}

/// `guarda_copias { dar_suelto: { zona?, ruta, usuario, carpeta } }`: mueve el repositorio
/// suelto `ruta` de esa zona a `<usuario>/<carpeta>` (mismo disco, `rename`). Devuelve el
/// mensaje y el detalle `{ ruta: "<usuario>/<carpeta>" }`.
pub fn dar(c: &ServerConfig, d: &Value) -> Result<(String, Value), String> {
    let zona = d["zona"].as_str().filter(|z| !z.is_empty() && *z != "principal");
    let raiz = PathBuf::from(c.carpeta_zona(zona).ok_or("Esa zona ya no está en este almacén.")?);
    let usuarios: Vec<String> = match zona {
        None => c.users.iter().map(|u| u.name.clone()).collect(),
        Some(z) => c.zonas.iter().find(|x| x.id == z).map(|x| x.users.iter().map(|u| u.name.clone()).collect()).unwrap_or_default(),
    };
    let ruta = d["ruta"].as_str().unwrap_or_default().trim().trim_matches('/').to_string();
    let trozos: Vec<&str> = ruta.split('/').collect();
    if trozos.is_empty() || trozos.len() > 2 || !trozos.iter().all(|t| trozo_valido(t)) {
        return Err("La carpeta del repositorio suelto no es válida.".into());
    }
    // Solo uno de los que se ven como sueltos (no la carpeta de otro equipo ni algo de fuera).
    if !sueltos_en(&raiz, &usuarios).iter().any(|(r, _)| *r == ruta) {
        return Err("Ese repositorio ya no está suelto en el almacén (puede que ya se moviera).".into());
    }
    let usuario = d["usuario"].as_str().unwrap_or_default();
    if !usuarios.iter().any(|u| u == usuario) {
        return Err("Ese equipo aún no tiene acceso a esta zona del almacén: dale acceso primero.".into());
    }
    let carpeta = d["carpeta"].as_str().unwrap_or_default();
    if !crate::nombres::carpeta_valida(carpeta) {
        return Err("El nombre de la carpeta nueva: letras minúsculas, cifras y guiones.".into());
    }
    let origen = raiz.join(&ruta);
    let dir_usuario = raiz.join(usuario);
    let destino = dir_usuario.join(carpeta);
    if crate::platform::hay_enlace_en_el_camino(&origen) || crate::platform::hay_enlace_en_el_camino(&dir_usuario) || !es_repo(&origen) {
        return Err("No se mueve: hay un enlace en el camino o ya no parece un repositorio.".into());
    }
    if destino.exists() {
        return Err(format!("Ya hay una carpeta «{carpeta}» en la del equipo: elige otro nombre."));
    }
    if !dir_usuario.exists() {
        std::fs::create_dir(&dir_usuario).map_err(|e| format!("No se pudo crear la carpeta del equipo: {e}"))?;
    }
    if crate::platform::is_reparse_point(&dir_usuario) {
        return Err("La carpeta del equipo es un enlace: no se mueve nada.".into());
    }
    std::fs::rename(&origen, &destino).map_err(|e| {
        format!("No se pudo mover sin copiar ({e}). Si la carpeta está en otro disco que la del almacén, muévela a mano a la del equipo y adóptala desde allí.")
    })?;
    let nueva = format!("{usuario}/{carpeta}");
    crate::agent::log(&format!("Repositorio suelto «{ruta}» movido a «{nueva}» en el almacén."));
    Ok((
        format!("«{ruta}» movido a la carpeta del equipo («{nueva}»), sin copiar nada. Ahora el equipo puede adoptarlo con su contraseña."),
        json!({ "ruta": nueva }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{ServerUser, Zona};

    fn repo(d: &Path) {
        for x in ["data", "index", "keys", "snapshots"] {
            std::fs::create_dir_all(d.join(x)).unwrap();
        }
        std::fs::write(d.join("config"), b"x").unwrap();
        std::fs::write(d.join("snapshots").join("a1"), b"x").unwrap();
    }

    #[test]
    fn buscar_y_dar_a_un_equipo() {
        let base = std::env::temp_dir().join(format!("resguardo-sueltos-{}", uuid::Uuid::new_v4()));
        let (d, e) = (base.join("d"), base.join("e"));
        // Uno de un equipo (no suelto), uno viejo en la raíz y otro dos niveles más abajo; y uno en otra zona.
        repo(&d.join("pc-01-3f2a").join("docs"));
        repo(&d.join("viejo"));
        repo(&d.join("rest-antiguo").join("ventas"));
        repo(&e.join("otro"));
        std::fs::create_dir_all(d.join("no-es-repo").join("nada")).unwrap();
        let c = ServerConfig {
            enabled: true,
            path: d.display().to_string(),
            users: vec![ServerUser { name: "pc-01-3f2a".into(), ..Default::default() }],
            zonas: vec![Zona {
                id: "z0a0b0c".into(),
                nombre: "Disco E".into(),
                path: e.display().to_string(),
                port: 8002,
                users: vec![],
                creada: String::new(),
            }],
            ..Default::default()
        };
        let r = resumen(&c);
        let rutas: Vec<(&str, Option<&str>)> = r.as_array().unwrap().iter().map(|x| (x["ruta"].as_str().unwrap(), x["zona"].as_str())).collect();
        assert_eq!(rutas, vec![("rest-antiguo/ventas", None), ("viejo", None), ("otro", Some("z0a0b0c"))]);
        assert_eq!(r[0]["versiones"], 1);
        // Dar a un equipo: dentro de su carpeta, con otro nombre, sin copiar.
        let mut pedido = json!({ "ruta": "rest-antiguo/ventas", "usuario": "pc-01-3f2a", "carpeta": "ventas" });
        let (m, det) = dar(&c, &pedido).unwrap();
        assert!(m.contains("pc-01-3f2a/ventas"), "{m}");
        assert_eq!(det["ruta"], "pc-01-3f2a/ventas");
        assert!(es_repo(&d.join("pc-01-3f2a").join("ventas")) && !d.join("rest-antiguo").join("ventas").exists());
        // Ya no está suelto; ni a un equipo sin acceso, ni a una carpeta que ya existe, ni fuera.
        assert!(dar(&c, &pedido).unwrap_err().contains("ya no está suelto"));
        pedido = json!({ "ruta": "viejo", "usuario": "otro-equipo", "carpeta": "viejo" });
        assert!(dar(&c, &pedido).unwrap_err().contains("acceso"));
        pedido = json!({ "ruta": "viejo", "usuario": "pc-01-3f2a", "carpeta": "docs" });
        assert!(dar(&c, &pedido).unwrap_err().contains("Ya hay"));
        for mala in ["../fuera", "pc-01-3f2a/docs", "", "a/b/c"] {
            assert!(dar(&c, &json!({ "ruta": mala, "usuario": "pc-01-3f2a", "carpeta": "x" })).is_err(), "{mala}");
        }
        assert!(dar(&c, &json!({ "ruta": "viejo", "usuario": "pc-01-3f2a", "carpeta": "Con Espacios" })).is_err());
        // Buscar también fuera: se enseña con su ruta entera y no se mueve.
        let fuera = base.join("fuera");
        repo(&fuera.join("antiguo"));
        let mut c2 = c.clone();
        poner_busqueda(&mut c2, &json!([fuera.display().to_string()])).unwrap();
        let r2 = resumen(&c2);
        assert!(r2.as_array().unwrap().iter().any(|x| x["fuera"] == true && x["rel"] == "antiguo"));
        assert!(poner_busqueda(&mut c2, &json!(["relativa"])).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }
}
