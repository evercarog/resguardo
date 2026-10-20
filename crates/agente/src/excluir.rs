//! 0.8.1 (15y): «No copiar» fuera del motor de copias.
//!
//! - **«No copiar archivos de más de N MB», con excepciones por tipo** (p. ej. siempre
//!   `*.pst` y `*.bak`). El motor de copias tiene `--exclude-larger-than`, pero sin
//!   excepciones. Lo más honesto: antes de copiar, el agente recorre las carpetas (sin
//!   seguir enlaces y saltando lo que ya excluyen las reglas), apunta los archivos de más
//!   de N MB que no son de un tipo exceptuado y los deja fuera **por su ruta exacta** (un
//!   archivo de exclusiones en la carpeta privada del agente). Así sabe exactamente qué se
//!   dejó fuera por tamaño y lo dice en la versión (cuántos, cuánto y de qué tipos, sin
//!   rutas). Si el recorrido no termina a tiempo, copia **sin** el límite y lo dice (copiar
//!   de más es mejor que dejar fuera sin saberlo).
//! - **Vista previa** (`vista_exclusiones`): cuánto deja fuera cada grupo de «No copiar»
//!   en unas carpetas, sin copiar nada, con un tope de tiempo.
//!
//! Las reglas se interpretan como las del motor de copias: `*`, `?` y `[...]` dentro de un
//! nombre, `**` para cualquier número de carpetas; una regla que empieza por `/` o por una
//! unidad (`C:`) va desde la raíz y las demás valen en cualquier sitio de la ruta (`*.tmp`,
//! `Downloads/*.exe`); si una regla encaja con una carpeta, todo lo de dentro queda fuera.
//! En Windows, sin distinguir mayúsculas (como `--iexclude`). Es una aproximación para
//! contar: lo que de verdad deja fuera las reglas sigue siendo cosa del motor.

use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Un MB, como lo cuenta la consola (MiB).
pub const MB: u64 = 1024 * 1024;
/// Como mucho, el límite de tamaño (1 TB).
pub const MAX_MB: u32 = 1_000_000;
/// Como mucho, excepciones al límite de tamaño.
pub const MAX_EXCEPCIONES: usize = 50;
/// Como mucho, grupos que se previsualizan de una vez.
pub const MAX_GRUPOS_VISTA: usize = 60;
/// El recorrido de antes de copiar, como mucho (si no termina, se copia sin el límite).
pub const TOPE_ANTES_DE_COPIAR: std::time::Duration = std::time::Duration::from_secs(15 * 60);
/// La vista previa, como mucho.
pub const TOPE_VISTA: std::time::Duration = std::time::Duration::from_secs(60);

/// Un conjunto de reglas de «No copiar», listas para comparar con rutas.
#[derive(Debug, Clone, Default)]
pub struct Reglas {
    patrones: Vec<Patron>,
}

#[derive(Debug, Clone)]
struct Patron {
    /// Desde la raíz (`/…` o `C:…`); si no, en cualquier sitio de la ruta.
    anclado: bool,
    partes: Vec<String>,
}

/// Sin distinguir mayúsculas en Windows (como `--iexclude`).
fn normal(s: &str) -> String {
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s.to_string()
    }
}

/// Los trozos de una ruta o de una regla (por `/` o `\`), sin vacíos ni `.`.
fn trozos(s: &str) -> Vec<String> {
    s.split(['/', '\\']).filter(|x| !x.is_empty() && *x != ".").map(normal).collect()
}

impl Reglas {
    pub fn nuevas<S: AsRef<str>>(reglas: &[S]) -> Self {
        let patrones = reglas
            .iter()
            .map(|r| r.as_ref().trim())
            .filter(|r| !r.is_empty() && !r.starts_with('#'))
            .map(|r| {
                let b = r.as_bytes();
                let unidad = b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':';
                Patron { anclado: r.starts_with('/') || r.starts_with('\\') || unidad, partes: trozos(r) }
            })
            .filter(|p| !p.partes.is_empty())
            .collect();
        Reglas { patrones }
    }

    pub fn vacias(&self) -> bool {
        self.patrones.is_empty()
    }

    /// ¿Deja fuera esta ruta (o alguna de las carpetas en las que está)?
    pub fn excluye(&self, ruta: &Path) -> bool {
        self.excluye_trozos(&trozos(&ruta.to_string_lossy()))
    }

    fn excluye_trozos(&self, ruta: &[String]) -> bool {
        self.patrones.iter().any(|p| if p.anclado { encaja(&p.partes, ruta) } else { (0..ruta.len()).any(|i| encaja(&p.partes, &ruta[i..])) })
    }

    /// Solo por el nombre del archivo (para las excepciones por tipo: `*.pst`).
    pub fn por_nombre(&self, ruta: &Path) -> bool {
        let t = trozos(&ruta.to_string_lossy());
        self.excluye_trozos(&t) || t.last().is_some_and(|n| self.patrones.iter().any(|p| p.partes.len() == 1 && comodin(p.partes[0].as_bytes(), n.as_bytes())))
    }
}

/// ¿Encajan las partes de la regla con el principio de la ruta? (Si encaja una carpeta,
/// también lo de dentro.) `**` vale por cero o más carpetas.
fn encaja(pat: &[String], ruta: &[String]) -> bool {
    match pat.split_first() {
        None => true,
        Some((p, resto)) if p == "**" => (0..=ruta.len()).any(|i| encaja(resto, &ruta[i..])),
        Some((p, resto)) => match ruta.split_first() {
            Some((r, rr)) => comodin(p.as_bytes(), r.as_bytes()) && encaja(resto, rr),
            None => false,
        },
    }
}

/// `*`, `?` y `[...]` (con rangos y `^`/`!` para negar) dentro de un nombre.
fn comodin(p: &[u8], t: &[u8]) -> bool {
    // Por caracteres (no bytes) para que `?` valga por una letra con tilde.
    let p: Vec<char> = String::from_utf8_lossy(p).chars().collect();
    let t: Vec<char> = String::from_utf8_lossy(t).chars().collect();
    comodin_c(&p, &t)
}

fn comodin_c(p: &[char], t: &[char]) -> bool {
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut estrella, mut desde) = (None::<usize>, 0usize);
    while ti < t.len() {
        if pi < p.len() {
            match p[pi] {
                '*' => {
                    estrella = Some(pi);
                    desde = ti;
                    pi += 1;
                    continue;
                }
                '?' => {
                    pi += 1;
                    ti += 1;
                    continue;
                }
                '[' => {
                    if let Some((ok, fin)) = clase(&p[pi..], t[ti]) {
                        if ok {
                            pi += fin;
                            ti += 1;
                            continue;
                        }
                    } else if t[ti] == '[' {
                        // Un `[` sin cerrar vale por sí mismo.
                        pi += 1;
                        ti += 1;
                        continue;
                    }
                }
                c if c == t[ti] => {
                    pi += 1;
                    ti += 1;
                    continue;
                }
                _ => {}
            }
        }
        match estrella {
            Some(e) => {
                pi = e + 1;
                desde += 1;
                ti = desde;
            }
            None => return false,
        }
    }
    p[pi..].iter().all(|c| *c == '*')
}

/// Una clase `[...]` al principio de `p`: si encaja `c` y cuánto ocupa (o `None` si no se cierra).
fn clase(p: &[char], c: char) -> Option<(bool, usize)> {
    let mut i = 1;
    let negada = matches!(p.get(1), Some('^') | Some('!'));
    if negada {
        i += 1;
    }
    let mut ok = false;
    let mut primero = true;
    while i < p.len() {
        if p[i] == ']' && !primero {
            return Some((ok != negada, i + 1));
        }
        primero = false;
        if i + 2 < p.len() && p[i + 1] == '-' && p[i + 2] != ']' {
            if p[i] <= c && c <= p[i + 2] {
                ok = true;
            }
            i += 3;
        } else {
            if p[i] == c {
                ok = true;
            }
            i += 1;
        }
    }
    None
}

/// Una ruta exacta como regla del archivo de exclusiones (los comodines, entre corchetes;
/// en Linux, además, la barra invertida). `None` si no se puede escribir en una línea.
pub fn escapar(ruta: &str) -> Option<String> {
    if ruta.contains(['\n', '\r']) || ruta.trim() != ruta || ruta.starts_with('#') {
        return None;
    }
    let mut out = String::with_capacity(ruta.len() + 8);
    for c in ruta.chars() {
        match c {
            '*' => out.push_str("[*]"),
            '?' => out.push_str("[?]"),
            '[' => out.push_str("[[]"),
            // El motor expande variables de entorno en el archivo: `$` entre corchetes no.
            '$' => out.push_str("[$]"),
            '\\' if !cfg!(windows) => out.push_str("\\\\"),
            c => out.push(c),
        }
    }
    Some(out)
}

/// La extensión de un archivo en minúsculas («.pst»), o «sin extensión».
fn tipo_de(ruta: &Path) -> String {
    ruta.extension().map(|e| format!(".{}", e.to_string_lossy().to_lowercase())).filter(|e| e.chars().count() <= 12).unwrap_or_else(|| "sin extensión".into())
}

/// Recorre las carpetas sin seguir enlaces, saltando lo que deja fuera `excluir`, y llama a
/// `visita` con cada archivo y su tamaño. `false` si se pasó del tope (el recorrido queda a medias).
pub fn recorrer(carpetas: &[String], excluir: &Reglas, tope: Instant, visita: &mut dyn FnMut(&Path, u64)) -> bool {
    let mut pila: Vec<PathBuf> = carpetas.iter().map(PathBuf::from).collect();
    let mut n = 0u64;
    while let Some(p) = pila.pop() {
        n += 1;
        if n.is_multiple_of(256) && Instant::now() > tope {
            return false;
        }
        if excluir.excluye(&p) {
            continue;
        }
        let Ok(m) = std::fs::symlink_metadata(&p) else { continue };
        if m.file_type().is_symlink() {
            continue;
        }
        if m.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&p) {
                pila.extend(rd.flatten().map(|e| e.path()));
            }
        } else if m.is_file() {
            visita(&p, m.len());
        }
    }
    true
}

/// Lo que «No copiar archivos de más de N MB» deja fuera en unas carpetas.
#[derive(Debug, Default)]
pub struct FueraPorTamano {
    /// Las rutas, para el archivo de exclusiones (solo en el equipo).
    pub rutas: Vec<String>,
    pub archivos: u64,
    pub bytes: u64,
    /// Cuántos de cada tipo.
    pub tipos: BTreeMap<String, u64>,
    /// ¿Terminó el recorrido a tiempo?
    pub completo: bool,
}

/// Recorre las carpetas y apunta lo que pasa de `limite_mb` y no es de un tipo exceptuado.
pub fn fuera_por_tamano(carpetas: &[String], exclusiones: &[String], limite_mb: u32, excepto: &[String], tope: Instant) -> FueraPorTamano {
    let reglas = Reglas::nuevas(exclusiones);
    let excepciones = Reglas::nuevas(excepto);
    let limite = u64::from(limite_mb) * MB;
    let mut f = FueraPorTamano::default();
    f.completo = recorrer(carpetas, &reglas, tope, &mut |p, tam| {
        if tam > limite && !excepciones.por_nombre(p) {
            if let Some(r) = escapar(&p.to_string_lossy()) {
                f.rutas.push(r);
                f.archivos += 1;
                f.bytes += tam;
                *f.tipos.entry(tipo_de(p)).or_default() += 1;
            }
        }
    });
    f
}

/// «3 archivos (12,4 GB: 2 .mp4 y 1 .iso)», sin rutas.
pub fn texto_fuera(f: &FueraPorTamano) -> String {
    let mut tipos: Vec<(&String, &u64)> = f.tipos.iter().collect();
    tipos.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    let mut partes: Vec<String> = tipos.iter().take(4).map(|(t, n)| format!("{n} {t}")).collect();
    if tipos.len() > 4 {
        partes.push("otros".into());
    }
    let lista = match partes.len() {
        0 => String::new(),
        1 => partes[0].clone(),
        k => format!("{} y {}", partes[..k - 1].join(", "), partes[k - 1]),
    };
    let cuantos = if f.archivos == 1 { "1 archivo".to_string() } else { format!("{} archivos", f.archivos) };
    format!("{cuantos} ({}{}{lista})", crate::tasks::human_bytes(f.bytes), if lista.is_empty() { "" } else { ": " })
}

/// Valida una lista de reglas que llega de fuera (excepciones, reglas de un grupo).
pub fn reglas_validas(reglas: &[String], max: usize, que: &str) -> Result<(), String> {
    if reglas.len() > max {
        return Err(format!("{que}: como mucho {max}."));
    }
    for r in reglas {
        let t = r.trim();
        if t.is_empty() || t.chars().count() > 200 || t.chars().any(char::is_control) {
            return Err(format!("{que}: cada regla, de 1 a 200 caracteres y sin caracteres de control."));
        }
    }
    Ok(())
}

/// `vista_exclusiones { carpetas, grupos: [{ id, reglas }], sueltas?, mayores_mb?, excepto? }`
/// (con la clave de administración: recorre carpetas del equipo; nunca copia nada): cuánto
/// deja fuera cada grupo, las reglas sueltas y el límite de tamaño, con un tope de tiempo.
/// Responde, sellado para la consola, `{ total, grupos: [{ id, archivos, bytes }], sueltas,
/// tamano, quedan, completo }` (cuentas, sin rutas).
pub fn vista_previa(c: &Value, tope: Instant) -> Result<Value, String> {
    let lista =
        |v: &Value, k: &str| -> Vec<String> { v[k].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default() };
    let carpetas = lista(c, "carpetas");
    if carpetas.is_empty() || carpetas.len() > 100 {
        return Err("Elige de 1 a 100 carpetas.".into());
    }
    for d in &carpetas {
        if d.chars().any(char::is_control) || !Path::new(d).is_absolute() {
            return Err("Cada carpeta, una ruta completa.".into());
        }
    }
    let grupos: Vec<(String, Reglas)> = c["grupos"]
        .as_array()
        .map(|a| {
            a.iter()
                .take(MAX_GRUPOS_VISTA)
                .map(|g| (g["id"].as_str().unwrap_or_default().chars().take(64).collect::<String>(), Reglas::nuevas(&lista(g, "reglas"))))
                .collect()
        })
        .unwrap_or_default();
    let sueltas_v = lista(c, "sueltas");
    reglas_validas(&sueltas_v, 500, "Las reglas sueltas")?;
    let sueltas = Reglas::nuevas(&sueltas_v);
    let limite = c["mayores_mb"].as_u64().filter(|n| (1..=u64::from(MAX_MB)).contains(n)).map(|n| n * MB);
    let excepto = Reglas::nuevas(&lista(c, "excepto"));
    let mut total = (0u64, 0u64);
    let mut por_grupo = vec![(0u64, 0u64); grupos.len()];
    let mut en_sueltas = (0u64, 0u64);
    let mut por_tamano = (0u64, 0u64);
    let mut quedan = (0u64, 0u64);
    let completo = recorrer(&carpetas, &Reglas::default(), tope, &mut |p, tam| {
        total.0 += 1;
        total.1 += tam;
        let mut fuera = false;
        for (i, (_, r)) in grupos.iter().enumerate() {
            if r.excluye(p) {
                por_grupo[i].0 += 1;
                por_grupo[i].1 += tam;
                fuera = true;
            }
        }
        if sueltas.excluye(p) {
            en_sueltas.0 += 1;
            en_sueltas.1 += tam;
            fuera = true;
        }
        if !fuera && limite.is_some_and(|l| tam > l) && !excepto.por_nombre(p) {
            por_tamano.0 += 1;
            por_tamano.1 += tam;
            fuera = true;
        }
        if !fuera {
            quedan.0 += 1;
            quedan.1 += tam;
        }
    });
    let par = |x: (u64, u64)| json!({ "archivos": x.0, "bytes": x.1 });
    Ok(json!({
        "total": par(total),
        "grupos": grupos.iter().zip(por_grupo).map(|((id, _), x)| json!({ "id": id, "archivos": x.0, "bytes": x.1 })).collect::<Vec<_>>(),
        "sueltas": par(en_sueltas),
        "tamano": limite.map(|_| par(por_tamano)),
        "quedan": par(quedan),
        "completo": completo,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: &[&str]) -> Reglas {
        Reglas::nuevas(x)
    }

    #[test]
    fn reglas_como_el_motor() {
        let p = |s: &str| PathBuf::from(s);
        assert!(r(&["*.tmp"]).excluye(&p("/home/ana/docs/a.tmp")));
        assert!(!r(&["*.tmp"]).excluye(&p("/home/ana/docs/a.tmpx")));
        assert!(r(&["Downloads/*.exe"]).excluye(&p("/home/ana/Downloads/setup.exe")));
        assert!(!r(&["Downloads/*.exe"]).excluye(&p("/home/ana/Programas/erp.exe")));
        // Una carpeta deja fuera lo de dentro.
        assert!(r(&["node_modules"]).excluye(&p("/srv/app/node_modules/x/y.js")));
        assert!(r(&["/srv/cache"]).excluye(&p("/srv/cache/a/b")));
        assert!(!r(&["/srv/cache"]).excluye(&p("/otra/srv/cache/a")));
        assert!(r(&["**/tmp/*.log"]).excluye(&p("/a/b/tmp/x.log")));
        assert!(r(&["~$*"]).excluye(&p("/d/~$informe.docx")));
        assert!(r(&["foto?.jpg"]).excluye(&p("/d/foto1.jpg")) && !r(&["foto?.jpg"]).excluye(&p("/d/foto10.jpg")));
        assert!(r(&["*.[ms]p3"]).excluye(&p("/d/a.mp3")) && !r(&["*.[!m]p3"]).excluye(&p("/d/a.mp3")));
        assert!(r(&["Thumbs.db"]).excluye(&p("/d/e/Thumbs.db")));
        assert!(!r(&[]).excluye(&p("/d")));
        // Con tildes.
        assert!(r(&["Música"]).excluye(&p("/home/ana/Música/a.mp3")));
        assert!(r(&["?úsica"]).excluye(&p("/home/ana/Música")));
    }

    #[cfg(windows)]
    #[test]
    fn en_windows_sin_mayusculas() {
        assert!(r(&["*.TMP"]).excluye(Path::new(r"C:\Datos\a.tmp")));
        assert!(r(&[r"C:\Users\*\AppData"]).excluye(Path::new(r"c:\users\ana\appdata\local\x")));
    }

    #[test]
    fn escapar_rutas_exactas() {
        assert_eq!(escapar("/d/a*b?[c]$HOME.txt").unwrap(), "/d/a[*]b[?][[]c][$]HOME.txt");
        assert!(escapar("/d/a\nb").is_none());
        assert!(escapar("#raro").is_none());
        // Escapada, encaja solo consigo misma.
        let e = escapar("/d/a*b").unwrap();
        assert!(r(&[e.as_str()]).excluye(Path::new("/d/a*b")) && !r(&[e.as_str()]).excluye(Path::new("/d/axxb")));
    }

    #[test]
    fn fuera_por_tamano_y_vista_previa() {
        let d = std::env::temp_dir().join(format!("resguardo-excluir-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(d.join("Música")).unwrap();
        std::fs::create_dir_all(d.join("trabajo")).unwrap();
        let grande = vec![0u8; 3 * MB as usize];
        std::fs::write(d.join("Música").join("a.mp3"), &grande).unwrap();
        std::fs::write(d.join("trabajo").join("correo.pst"), &grande).unwrap();
        std::fs::write(d.join("trabajo").join("video.mp4"), &grande).unwrap();
        std::fs::write(d.join("trabajo").join("hoja.xlsx"), b"poco").unwrap();
        std::fs::write(d.join("trabajo").join("x.tmp"), b"poco").unwrap();
        let carpetas = vec![d.to_string_lossy().to_string()];
        let lejos = Instant::now() + std::time::Duration::from_secs(60);
        // Más de 1 MB, salvo los .pst; la música ya está fuera por otra regla.
        let f = fuera_por_tamano(&carpetas, &["Música".into()], 1, &["*.pst".into()], lejos);
        assert!(f.completo);
        assert_eq!((f.archivos, f.bytes), (1, 3 * MB));
        assert_eq!(f.tipos.get(".mp4"), Some(&1));
        assert!(f.rutas[0].ends_with("video.mp4"));
        assert_eq!(texto_fuera(&f), "1 archivo (3 MB: 1 .mp4)");
        // La vista previa: cada grupo, las sueltas y el tamaño, sin rutas.
        let c = json!({ "carpetas": carpetas, "grupos": [{ "id": "musica", "reglas": ["*.mp3"] }, { "id": "temporales", "reglas": ["*.tmp"] }], "sueltas": [], "mayores_mb": 1, "excepto": ["*.pst"] });
        let v = vista_previa(&c, lejos).unwrap();
        assert_eq!(v["total"]["archivos"], 5);
        assert_eq!(v["grupos"][0], json!({ "id": "musica", "archivos": 1, "bytes": 3 * MB }));
        assert_eq!(v["grupos"][1]["archivos"], 1);
        assert_eq!(v["tamano"], json!({ "archivos": 1, "bytes": 3 * MB }));
        assert_eq!(v["quedan"]["archivos"], 2);
        assert_eq!(v["completo"], true);
        assert!(!v.to_string().contains("trabajo"), "sin rutas");
        // Con el tope ya pasado no cuenta como completo (y no falla).
        let ya = vista_previa(&c, Instant::now()).unwrap();
        assert!(ya["total"]["archivos"].as_u64().unwrap() <= 5);
        assert!(vista_previa(&json!({ "carpetas": [] }), lejos).is_err());
        assert!(vista_previa(&json!({ "carpetas": ["relativa"] }), lejos).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
