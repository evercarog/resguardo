//! «Qué cambió» entre dos versiones, archivo por archivo, con tamaños:
//! `restic diff --json` (qué rutas son nuevas, cambiaron o se borraron) y
//! `restic ls --json` de las dos versiones (el tamaño antes y después de cada
//! una de esas rutas). Solo se leen índices de carpetas: no se descargan datos.
//!
//! También: la versión anterior «de la misma copia» (mismas carpetas y mismo
//! equipo) y las versiones en las que está un archivo (`restic find`).
//! Los nombres de archivo son datos del cliente: quien llama los manda cifrados
//! a la consola (sesión `explorar`), nunca al servidor.

use crate::restic::{self, Access, Snapshot};
use serde::Serialize;
use std::collections::HashMap;
use std::time::Duration;

/// Como mucho tantos cambios con su ruta (los recuentos cuentan todos).
pub const MAX_CAMBIOS: usize = 20_000;
/// `diff` y cada `ls` en un repositorio grande y remoto pueden tardar.
pub const TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// Como mucho tantas versiones de un archivo.
pub const MAX_VERSIONES_ARCHIVO: usize = 500;

/// Un archivo que cambió. `tipo`: `nuevo`, `cambiado` (contenido), `borrado`,
/// `metadatos` (solo fechas o permisos) u `otro` (p. ej. pasó de archivo a enlace).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Cambio {
    pub ruta: String,
    pub tipo: &'static str,
    /// Tamaño en la versión nueva (`None`: no está en ella o no se midió).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    /// Tamaño en la versión anterior.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_antes: Option<u64>,
}

/// Recuentos de todo el cambio (aunque la lista se recorte).
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Resumen {
    pub nuevos: u64,
    pub cambiados: u64,
    pub borrados: u64,
    pub metadatos: u64,
    pub otros: u64,
    pub carpetas_nuevas: u64,
    pub carpetas_borradas: u64,
    /// Datos nuevos y quitados según restic (`statistics.added/removed.bytes`).
    pub bytes_anadidos: Option<u64>,
    pub bytes_quitados: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Diferencias {
    pub desde: String,
    pub hasta: String,
    pub resumen: Resumen,
    pub cambios: Vec<Cambio>,
    /// Había más cambios que [`MAX_CAMBIOS`]: la lista solo trae los primeros.
    pub recortado: bool,
    /// Con los tamaños de cada archivo (`ls` de las dos versiones).
    pub con_tamanos: bool,
}

/// Modificador de restic → tipo. `+` nuevo, `-` borrado, `M` contenido,
/// `T` cambió de tipo, `U` solo metadatos (pueden venir juntos: `MU`).
pub fn tipo_de(modificador: &str) -> &'static str {
    match modificador {
        "+" => "nuevo",
        "-" => "borrado",
        m if m.contains('M') => "cambiado",
        m if m.contains('T') => "otro",
        m if m.contains('U') => "metadatos",
        _ => "otro",
    }
}

/// Lee la salida de `restic diff --json`: los cambios de archivos (las
/// carpetas, que acaban en `/`, solo se cuentan), como mucho `max`.
pub fn leer_diff(salida: &str, max: usize) -> (Vec<Cambio>, Resumen, bool) {
    let mut cambios = Vec::new();
    let mut r = Resumen::default();
    let mut recortado = false;
    for linea in salida.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(linea) else { continue };
        match v["message_type"].as_str() {
            Some("change") => {
                let ruta = v["path"].as_str().unwrap_or_default();
                let tipo = tipo_de(v["modifier"].as_str().unwrap_or_default());
                if ruta.is_empty() {
                    continue;
                }
                if ruta.ends_with('/') {
                    match tipo {
                        "nuevo" => r.carpetas_nuevas += 1,
                        "borrado" => r.carpetas_borradas += 1,
                        _ => {}
                    }
                    continue;
                }
                match tipo {
                    "nuevo" => r.nuevos += 1,
                    "borrado" => r.borrados += 1,
                    "cambiado" => r.cambiados += 1,
                    "metadatos" => r.metadatos += 1,
                    _ => r.otros += 1,
                }
                if cambios.len() < max {
                    cambios.push(Cambio { ruta: ruta.to_string(), tipo, bytes: None, bytes_antes: None });
                } else {
                    recortado = true;
                }
            }
            Some("statistics") => {
                r.bytes_anadidos = v["added"]["bytes"].as_u64();
                r.bytes_quitados = v["removed"]["bytes"].as_u64();
            }
            _ => {}
        }
    }
    (cambios, r, recortado)
}

/// Un archivo (ruta, tamaño) de una línea de `restic ls --json`, sin
/// deserializar las que no son archivos.
fn archivo_de_ls(linea: &str) -> Option<(String, u64)> {
    crate::sizes::archivo_de_ls(linea)
}

/// Pone a cada cambio su tamaño, recorriendo `ls` de una versión.
fn medir(acc: &Access, version: &str, indice: &HashMap<String, usize>, cambios: &mut [Cambio], antes: bool) -> Result<(), String> {
    let mut f = |linea: &str| {
        if let Some((ruta, tam)) = archivo_de_ls(linea) {
            if let Some(&i) = indice.get(&ruta) {
                if antes {
                    cambios[i].bytes_antes = Some(tam);
                } else {
                    cambios[i].bytes = Some(tam);
                }
            }
        }
    };
    let out = restic::run_raw_lines(acc, &["ls", "--json", "--no-lock", version], TIMEOUT, &mut f)?;
    if out.code != Some(0) {
        return Err(restic::exit_error(out.code, &out.stderr));
    }
    Ok(())
}

/// Qué cambió de `desde` a `hasta` (ids de versión). Con `con_tamanos`, el
/// tamaño de cada archivo antes y después (dos `ls`: tarda más).
pub fn diferencias(acc: &Access, desde: &str, hasta: &str, con_tamanos: bool) -> Result<Diferencias, String> {
    if !restic::valid_snapshot_id(desde) || !restic::valid_snapshot_id(hasta) {
        return Err("Versión no válida.".into());
    }
    let out = restic::run_with(acc, &["diff", "--json", "--no-lock", desde, hasta], TIMEOUT, None)?;
    let (mut cambios, resumen, recortado) = leer_diff(&String::from_utf8_lossy(&out), MAX_CAMBIOS);
    if con_tamanos && !cambios.is_empty() {
        let indice: HashMap<String, usize> = cambios.iter().enumerate().map(|(i, c)| (c.ruta.clone(), i)).collect();
        if cambios.iter().any(|c| c.tipo != "nuevo") {
            medir(acc, desde, &indice, &mut cambios, true)?;
        }
        if cambios.iter().any(|c| c.tipo != "borrado") {
            medir(acc, hasta, &indice, &mut cambios, false)?;
        }
    }
    Ok(Diferencias { desde: desde.to_string(), hasta: hasta.to_string(), resumen, cambios, recortado, con_tamanos })
}

/// ¿Es `s` el mismo id que `id` (completo o abreviado)?
fn mismo_id(s: &Snapshot, id: &str) -> bool {
    s.id == id || s.short_id == id || (id.len() >= 8 && s.id.starts_with(id))
}

/// La versión anterior a `hasta` de la misma copia: mismo equipo y mismas
/// carpetas, la más reciente antes que ella. `None` si es la primera.
pub fn anterior<'a>(versiones: &'a [Snapshot], hasta: &str) -> Option<&'a Snapshot> {
    let v = versiones.iter().find(|s| mismo_id(s, hasta))?;
    let mut carpetas = v.paths.clone();
    carpetas.sort();
    let t = instante(&v.time);
    versiones
        .iter()
        .filter(|s| {
            instante(&s.time) < t && s.hostname == v.hostname && {
                let mut p = s.paths.clone();
                p.sort();
                p == carpetas
            }
        })
        .max_by_key(|s| instante(&s.time))
}

/// La hora de una versión, comparable aunque cambie el huso (verano, otro equipo).
fn instante(t: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(t).ok().map(|x| x.with_timezone(&chrono::Utc))
}

/// Una ruta de archivo dentro de una versión (`/C/Users/Ana/x.txt`): absoluta,
/// sin `.`, `..`, NUL ni caracteres de control, hasta 4096 caracteres.
pub fn ruta_valida(ruta: &str) -> bool {
    ruta.starts_with('/')
        && ruta.len() > 1
        && ruta.chars().count() <= 4096
        && !ruta.chars().any(char::is_control)
        && !ruta.ends_with('/')
        && ruta[1..].split('/').all(|p| !p.is_empty() && p != "." && p != "..")
}

/// Patrón de `restic find` para una ruta exacta: sin la `/` del principio (con
/// ella, restic no encuentra nada en Windows) y con `*` en lugar de los
/// caracteres especiales de los patrones (`* ? [ ] \`). Coincide con más rutas
/// de las pedidas: el resultado se filtra luego por la ruta exacta.
pub fn patron_find(ruta: &str) -> String {
    ruta.trim_start_matches('/').chars().map(|c| if "*?[]\\".contains(c) { '*' } else { c }).collect()
}

/// Un archivo en una versión.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EnVersion {
    pub version: String,
    pub cuando: String,
    pub bytes: Option<u64>,
    pub modificado: Option<String>,
}

/// Lee `restic find --json` y deja solo la ruta exacta, con la hora de cada versión.
pub fn leer_find(salida: &[u8], ruta: &str, versiones: &[Snapshot]) -> Vec<EnVersion> {
    let v: serde_json::Value = serde_json::from_slice(salida).unwrap_or(serde_json::Value::Null);
    let mut out = Vec::new();
    for snap in v.as_array().into_iter().flatten() {
        let id = snap["snapshot"].as_str().unwrap_or_default();
        let Some(s) = versiones.iter().find(|s| mismo_id(s, id)) else { continue };
        let Some(m) = snap["matches"].as_array().into_iter().flatten().find(|m| m["path"].as_str() == Some(ruta) && m["type"] != "dir") else {
            continue;
        };
        out.push(EnVersion {
            version: s.short_id.clone(),
            cuando: s.time.clone(),
            bytes: m["size"].as_u64(),
            modificado: m["mtime"].as_str().map(str::to_string),
        });
    }
    out.sort_by_key(|x| std::cmp::Reverse(instante(&x.cuando)));
    out.truncate(MAX_VERSIONES_ARCHIVO);
    out
}

/// Las versiones en las que está el archivo `ruta`, la más reciente primero.
pub fn versiones_de_archivo(acc: &Access, ruta: &str) -> Result<Vec<EnVersion>, String> {
    if !ruta_valida(ruta) {
        return Err("Ruta no válida dentro de la versión.".into());
    }
    let versiones = restic::snapshots(acc)?;
    let patron = patron_find(ruta);
    let out = restic::run_with(acc, &["find", "--json", "--no-lock", "--", &patron], TIMEOUT, None)?;
    Ok(leer_find(&out, ruta, &versiones))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = r#"{"message_type":"change","path":"/datos/a.txt","modifier":"M"}
{"message_type":"change","path":"/datos/nueva/","modifier":"+"}
{"message_type":"change","path":"/datos/nueva/n.txt","modifier":"+"}
{"message_type":"change","path":"/datos/sub/b.txt","modifier":"-"}
{"message_type":"change","path":"/datos/c.txt","modifier":"U"}
{"message_type":"change","path":"/datos/d","modifier":"T"}
{"message_type":"statistics","source_snapshot":"429dd59e","target_snapshot":"2cbbdbca","changed_files":1,"added":{"files":1,"dirs":1,"others":0,"data_blobs":2,"tree_blobs":4,"bytes":4584},"removed":{"files":1,"dirs":0,"others":0,"data_blobs":2,"tree_blobs":3,"bytes":3773}}
"#;

    #[test]
    fn tipos_de_cambio() {
        assert_eq!(tipo_de("+"), "nuevo");
        assert_eq!(tipo_de("-"), "borrado");
        assert_eq!(tipo_de("M"), "cambiado");
        assert_eq!(tipo_de("MU"), "cambiado");
        assert_eq!(tipo_de("U"), "metadatos");
        assert_eq!(tipo_de("T"), "otro");
        assert_eq!(tipo_de("?"), "otro");
    }

    #[test]
    fn lee_la_salida_de_diff() {
        let (c, r, recortado) = leer_diff(DIFF, 100);
        assert!(!recortado);
        assert_eq!(
            c.iter().map(|x| (x.ruta.as_str(), x.tipo)).collect::<Vec<_>>(),
            [
                ("/datos/a.txt", "cambiado"),
                ("/datos/nueva/n.txt", "nuevo"),
                ("/datos/sub/b.txt", "borrado"),
                ("/datos/c.txt", "metadatos"),
                ("/datos/d", "otro")
            ]
        );
        assert_eq!((r.nuevos, r.cambiados, r.borrados, r.metadatos, r.otros, r.carpetas_nuevas), (1, 1, 1, 1, 1, 1));
        assert_eq!((r.bytes_anadidos, r.bytes_quitados), (Some(4584), Some(3773)));
        // Recortada: los recuentos siguen siendo los de todo.
        let (c, r, recortado) = leer_diff(DIFF, 2);
        assert!(recortado);
        assert_eq!(c.len(), 2);
        assert_eq!(r.nuevos + r.cambiados + r.borrados + r.metadatos + r.otros, 5);
        // Basura y líneas a medias: se ignoran.
        let (c, _, _) = leer_diff("no es json\n{\"message_type\":\"change\"}\n", 10);
        assert!(c.is_empty());
    }

    fn snap(id: &str, t: &str, host: &str, paths: &[&str]) -> Snapshot {
        serde_json::from_value(serde_json::json!({
            "id": format!("{id}{}", "0".repeat(56)), "short_id": id, "time": t, "hostname": host, "paths": paths
        }))
        .unwrap()
    }

    #[test]
    fn anterior_de_la_misma_copia() {
        let vs = vec![
            snap("aaaaaaaa", "2026-10-01T10:00:00Z", "CAJA", &["C:\\Datos"]),
            snap("bbbbbbbb", "2026-10-02T10:00:00Z", "CAJA", &["C:\\Otra"]),
            snap("cccccccc", "2026-10-03T10:00:00Z", "OTRO", &["C:\\Datos"]),
            snap("dddddddd", "2026-10-04T10:00:00Z", "CAJA", &["C:\\Datos"]),
        ];
        assert_eq!(anterior(&vs, "dddddddd").map(|s| s.short_id.as_str()), Some("aaaaaaaa"));
        assert_eq!(anterior(&vs, &vs[3].id).map(|s| s.short_id.as_str()), Some("aaaaaaaa"));
        assert!(anterior(&vs, "aaaaaaaa").is_none(), "la primera no tiene anterior");
        assert!(anterior(&vs, "eeeeeeee").is_none());
    }

    #[test]
    fn rutas_y_patrones() {
        assert!(ruta_valida("/C/Users/Ana/Informe [v2].docx"));
        for mal in ["", "/", "C/x", "/C/../x", "/C/./x", "/C//x", "/C/x/", "/C/x\0", "/C/\nx"] {
            assert!(!ruta_valida(mal), "{mal:?}");
        }
        assert!(!ruta_valida(&format!("/{}", "a".repeat(4096))));
        assert_eq!(patron_find("/C/Users/Ana/Informe [v2]*.docx"), "C/Users/Ana/Informe *v2**.docx");
        assert_eq!(patron_find("/home/a\\b?.txt"), "home/a*b*.txt");
    }

    #[test]
    fn lee_find_solo_la_ruta_exacta() {
        let vs = vec![snap("aaaaaaaa", "2026-10-01T10:00:00Z", "CAJA", &["/x"]), snap("bbbbbbbb", "2026-10-02T10:00:00Z", "CAJA", &["/x"])];
        let out = serde_json::json!([
            { "snapshot": vs[0].id, "matches": [
                { "path": "/x/a.txt", "type": "file", "size": 2, "mtime": "2026-10-01T09:00:00Z" },
                { "path": "/otra/x/a.txt", "type": "file", "size": 9 } ] },
            { "snapshot": vs[1].id, "matches": [ { "path": "/x/a.txt", "type": "file", "size": 5 } ] },
            { "snapshot": "ffffffff", "matches": [ { "path": "/x/a.txt", "type": "file", "size": 1 } ] }
        ]);
        let r = leer_find(out.to_string().as_bytes(), "/x/a.txt", &vs);
        assert_eq!(r.len(), 2);
        assert_eq!((r[0].version.as_str(), r[0].bytes), ("bbbbbbbb", Some(5)));
        assert_eq!((r[1].version.as_str(), r[1].bytes, r[1].modificado.as_deref()), ("aaaaaaaa", Some(2), Some("2026-10-01T09:00:00Z")));
        assert!(leer_find(b"basura", "/x/a.txt", &vs).is_empty());
    }

    /// Con restic de verdad (el del PATH), en un repositorio temporal: dos
    /// versiones, lo nuevo, lo cambiado, lo borrado y sus tamaños; la anterior
    /// de la misma copia; y las versiones de un archivo (con corchetes en el nombre).
    #[test]
    fn diferencias_reales() {
        if restic::version().is_err() {
            eprintln!("omitido: no hay restic");
            return;
        }
        let _g = restic::tests::real_repo_lock();
        let base = std::env::temp_dir().join(format!("resguardo-dif-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let datos = base.join("datos");
        std::fs::create_dir_all(datos.join("sub")).unwrap();
        std::fs::write(datos.join("a.txt"), "a").unwrap();
        std::fs::write(datos.join("sub").join("b.txt"), "bb").unwrap();
        std::fs::write(datos.join("[raro] c.txt"), "ccc").unwrap();
        let acc = Access::new(base.join("repo").display().to_string(), "contraseña de prueba");
        let lim = Duration::from_secs(120);
        assert_eq!(restic::run_raw(&acc, &["init"], lim).unwrap().code, Some(0));
        let d = datos.display().to_string();
        assert_eq!(restic::run_raw(&acc, &["backup", &d], lim).unwrap().code, Some(0));
        std::fs::write(datos.join("a.txt"), "aaaa").unwrap();
        std::fs::remove_file(datos.join("sub").join("b.txt")).unwrap();
        std::fs::create_dir_all(datos.join("nueva")).unwrap();
        std::fs::write(datos.join("nueva").join("n.txt"), "nnnnnn").unwrap();
        std::fs::write(datos.join("[raro] c.txt"), "cccc").unwrap();
        assert_eq!(restic::run_raw(&acc, &["backup", &d], lim).unwrap().code, Some(0));
        let mut vs = restic::snapshots(&acc).unwrap();
        vs.sort_by(|a, b| a.time.cmp(&b.time));
        let (v1, v2) = (&vs[0], &vs[1]);
        assert_eq!(anterior(&vs, &v2.short_id).map(|s| s.id.clone()), Some(v1.id.clone()));

        let r = diferencias(&acc, &v1.short_id, &v2.short_id, true).unwrap();
        let por = |fin: &str| r.cambios.iter().find(|c| c.ruta.ends_with(fin)).unwrap_or_else(|| panic!("{fin}: {:?}", r.cambios));
        assert_eq!((por("/a.txt").tipo, por("/a.txt").bytes_antes, por("/a.txt").bytes), ("cambiado", Some(1), Some(4)));
        assert_eq!((por("/n.txt").tipo, por("/n.txt").bytes_antes, por("/n.txt").bytes), ("nuevo", None, Some(6)));
        assert_eq!((por("/b.txt").tipo, por("/b.txt").bytes_antes, por("/b.txt").bytes), ("borrado", Some(2), None));
        assert_eq!((r.resumen.nuevos, r.resumen.borrados, r.resumen.carpetas_nuevas), (1, 1, 1));
        assert!(r.resumen.cambiados >= 2 && !r.recortado && r.con_tamanos);
        // Sin tamaños: lo mismo, más rápido.
        let s = diferencias(&acc, &v1.short_id, &v2.short_id, false).unwrap();
        assert_eq!(s.cambios.len(), r.cambios.len());
        assert!(s.cambios.iter().all(|c| c.bytes.is_none() && c.bytes_antes.is_none()));
        assert!(diferencias(&acc, "--help", &v2.short_id, true).is_err());

        let raro = por("c.txt").ruta.clone();
        let h = versiones_de_archivo(&acc, &raro).unwrap();
        assert_eq!(h.iter().map(|x| x.bytes).collect::<Vec<_>>(), [Some(4), Some(3)], "{h:?}");
        assert_eq!(h[0].version, v2.short_id);
        let b = versiones_de_archivo(&acc, &por("/b.txt").ruta).unwrap();
        assert_eq!(b.iter().map(|x| x.version.as_str()).collect::<Vec<_>>(), [v1.short_id.as_str()]);
        assert!(versiones_de_archivo(&acc, "/../x").is_err());
        let _ = std::fs::remove_dir_all(&base);
    }
}
