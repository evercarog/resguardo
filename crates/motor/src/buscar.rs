//! «Buscar archivos» en todas las versiones de un repositorio: un solo
//! `restic find --json` sobre las versiones elegidas (por fechas), leído a
//! medida que llega (sin guardar la salida entera), agrupado por archivo y con
//! límites de coincidencias y de tiempo.
//!
//! El texto se busca en el **nombre** del archivo, sin distinguir mayúsculas.
//! Los caracteres especiales de los patrones de restic (`* ? [ ] \`) no se
//! pueden escapar igual en Windows y en Linux (en Windows `\` es separador),
//! así que se cambian por `*` y luego se filtra por el texto exacto: el patrón
//! encuentra de más, nunca de menos. restic también da los archivos de dentro
//! de una carpeta cuyo nombre coincide; el filtro los quita.
//!
//! Los nombres de archivo son datos del cliente: quien llama los manda
//! cifrados a la consola (sesión `explorar`), nunca al servidor.

use crate::diferencias::EnVersion;
use crate::restic::{self, Access, Snapshot};
use serde::de::{self, DeserializeSeed, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufReader, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Como mucho tantas coincidencias (un archivo en una versión) por búsqueda.
pub const MAX_COINCIDENCIAS: usize = 2000;
/// Como mucho tantas versiones de un mismo archivo en el resultado.
pub const MAX_VERSIONES_POR_ARCHIVO: usize = 500;
/// Como mucho se busca en tantas versiones (las más recientes del rango).
pub const MAX_VERSIONES: usize = 1000;
/// Tiempo máximo de una búsqueda: pasado, se da lo encontrado hasta entonces.
pub const TIEMPO: Duration = Duration::from_secs(5 * 60);
/// Largo del texto que se busca (en caracteres).
pub const TEXTO_MIN: usize = 2;
pub const TEXTO_MAX: usize = 100;

/// Por qué el resultado no es completo.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Motivo {
    /// Se llegó al máximo de coincidencias.
    Limite,
    /// Se acabó el tiempo.
    Tiempo,
}

/// Un archivo encontrado y las versiones en las que está (la más reciente primero).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Encontrado {
    pub ruta: String,
    pub versiones: Vec<EnVersion>,
    /// Está en más versiones de las que se dan ([`MAX_VERSIONES_POR_ARCHIVO`]).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub recortado: bool,
}

/// El resultado de una búsqueda.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Busqueda {
    pub texto: String,
    /// Los archivos, el que estuvo más recientemente primero.
    pub archivos: Vec<Encontrado>,
    /// Coincidencias (archivo y versión) que trae el resultado.
    pub coincidencias: usize,
    /// En cuántas versiones se buscó y cuántas había en el rango de fechas.
    pub versiones_buscadas: usize,
    pub versiones_en_rango: usize,
    pub recortado: bool,
    pub motivo: Option<Motivo>,
}

/// El texto que se busca, comprobado: sin espacios alrededor, de 2 a 100
/// caracteres, sin caracteres de control ni separadores de carpeta.
pub fn texto_valido(texto: &str) -> Result<String, String> {
    let t = texto.trim();
    let n = t.chars().count();
    if !(TEXTO_MIN..=TEXTO_MAX).contains(&n) {
        return Err(format!("Escribe qué buscar (de {TEXTO_MIN} a {TEXTO_MAX} caracteres)."));
    }
    if t.chars().any(|c| c.is_control() || c == '/' || c == '\\') {
        return Err("Busca por el nombre del archivo (sin / ni \\ ni caracteres de control).".into());
    }
    Ok(t.to_string())
}

/// El patrón de `restic find` para un texto (ya comprobado): `*texto*` con
/// `*` en lugar de `* ? [ ] \`. Nunca empieza por `-` (empieza por `*`).
pub fn patron(texto: &str) -> String {
    let medio: String = texto.chars().map(|c| if "*?[]\\".contains(c) { '*' } else { c }).collect();
    format!("*{medio}*")
}

/// ¿Contiene el nombre el texto, sin distinguir mayúsculas? (`aguja` ya en minúsculas).
pub fn nombre_coincide(ruta: &str, aguja: &str) -> bool {
    let nombre = ruta.rsplit('/').next().unwrap_or(ruta);
    nombre.to_lowercase().contains(aguja)
}

/// Un momento, comparable aunque cambie el huso.
pub type Instante = chrono::DateTime<chrono::Utc>;

/// Una fecha (RFC 3339).
fn instante(t: &str) -> Option<Instante> {
    chrono::DateTime::parse_from_rfc3339(t).ok().map(|x| x.with_timezone(&chrono::Utc))
}

/// Desde y hasta cuándo (sin uno de ellos: sin límite por ese lado).
pub type Rango = (Option<Instante>, Option<Instante>);

/// Comprueba el rango de fechas (`desde` y `hasta`, RFC 3339, opcionales).
pub fn rango(desde: Option<&str>, hasta: Option<&str>) -> Result<Rango, String> {
    let leer = |x: Option<&str>| match x.filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) if s.len() <= 40 => instante(s).map(Some).ok_or_else(|| "Fecha no válida.".to_string()),
        Some(_) => Err("Fecha no válida.".to_string()),
    };
    let (d, h) = (leer(desde)?, leer(hasta)?);
    if let (Some(d), Some(h)) = (d, h) {
        if d > h {
            return Err("La fecha «desde» es posterior a «hasta».".into());
        }
    }
    Ok((d, h))
}

/// Las versiones del rango, la más reciente primero.
pub fn versiones_del_rango(mut vs: Vec<Snapshot>, desde: Option<Instante>, hasta: Option<Instante>) -> Vec<Snapshot> {
    vs.retain(|s| match instante(&s.time) {
        Some(t) => desde.is_none_or(|d| t >= d) && hasta.is_none_or(|h| t <= h),
        None => desde.is_none() && hasta.is_none(),
    });
    vs.sort_by_key(|s| std::cmp::Reverse(instante(&s.time)));
    vs
}

// ---------------------------------------------------------------------------
// Lectura de `restic find --json` a medida que llega
// ---------------------------------------------------------------------------

/// Lo que se va encontrando.
struct Acumulador<'a> {
    aguja: String,
    versiones: &'a [Snapshot],
    max: usize,
    coincidencias: usize,
    lleno: bool,
    por_ruta: HashMap<String, Vec<EnVersion>>,
}

impl Acumulador<'_> {
    /// Añade lo encontrado en una versión (si es de las pedidas); devuelve cuántas.
    fn anadir(&mut self, snapshot: &str, encontrados: Vec<Coincidencia>) -> usize {
        let Some(s) = self.versiones.iter().find(|s| s.id == snapshot || (snapshot.len() >= 8 && s.id.starts_with(snapshot))) else { return 0 };
        let n = encontrados.len();
        for c in encontrados {
            self.por_ruta.entry(c.path).or_default().push(EnVersion {
                version: s.short_id.clone(),
                cuando: s.time.clone(),
                bytes: c.size,
                modificado: c.mtime,
            });
        }
        n
    }
}

/// Un elemento de `matches` (solo lo que hace falta; el resto se ignora).
#[derive(Deserialize)]
struct Coincidencia {
    path: String,
    #[serde(rename = "type", default)]
    tipo: String,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    mtime: Option<String>,
}

/// Se llegó al máximo: se deja de leer (no es un error de la salida).
const LLENO: &str = "lleno";

struct Raiz<'x, 'a>(&'x mut Acumulador<'a>);

impl<'de> DeserializeSeed<'de> for Raiz<'_, '_> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for Raiz<'_, '_> {
    type Value = ();
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("la lista de versiones de restic find")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while seq.next_element_seed(Instantanea(&mut *self.0))?.is_some() {
            if self.0.lleno {
                return Err(de::Error::custom(LLENO));
            }
        }
        Ok(())
    }
}

/// `{ "matches": [...], "hits": n, "snapshot": "id" }` (el id llega al final).
struct Instantanea<'x, 'a>(&'x mut Acumulador<'a>);

impl<'de> DeserializeSeed<'de> for Instantanea<'_, '_> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for Instantanea<'_, '_> {
    type Value = ();
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("una versión de restic find")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let mut encontrados = Vec::new();
        let mut snapshot = String::new();
        while let Some(k) = map.next_key::<String>()? {
            match k.as_str() {
                "matches" => map.next_value_seed(Lista { acum: &mut *self.0, encontrados: &mut encontrados })?,
                "snapshot" => snapshot = map.next_value::<String>()?,
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        self.0.coincidencias += self.0.anadir(&snapshot, encontrados);
        Ok(())
    }
}

struct Lista<'x, 'y, 'a> {
    acum: &'x mut Acumulador<'a>,
    encontrados: &'y mut Vec<Coincidencia>,
}

impl<'de> DeserializeSeed<'de> for Lista<'_, '_, '_> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for Lista<'_, '_, '_> {
    type Value = ();
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("las coincidencias de una versión")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while let Some(c) = seq.next_element::<Coincidencia>()? {
            // Solo archivos cuyo nombre lleva el texto (no lo de dentro de una carpeta que se llama así).
            if c.tipo != "file" || !nombre_coincide(&c.path, &self.acum.aguja) {
                continue;
            }
            if self.acum.coincidencias + self.encontrados.len() >= self.acum.max {
                // Se termina de leer esta versión (su id llega al final) y se para.
                self.acum.lleno = true;
                continue;
            }
            self.encontrados.push(c);
        }
        Ok(())
    }
}

/// Lee la salida de `restic find --json` (de `entrada`, a medida que llega).
/// Devuelve si se llenó (entonces se dejó de leer) y lo encontrado, agrupado.
fn leer(entrada: impl Read, aguja: &str, versiones: &[Snapshot], max: usize) -> (Result<(), String>, bool, usize, HashMap<String, Vec<EnVersion>>) {
    let mut acum = Acumulador { aguja: aguja.to_lowercase(), versiones, max, coincidencias: 0, lleno: false, por_ruta: HashMap::new() };
    let mut d = serde_json::Deserializer::from_reader(BufReader::with_capacity(64 * 1024, entrada));
    let r = Raiz(&mut acum).deserialize(&mut d).map_err(|e| e.to_string());
    let r = if acum.lleno { Ok(()) } else { r };
    (r, acum.lleno, acum.coincidencias, acum.por_ruta)
}

/// Agrupa lo encontrado: cada archivo con sus versiones (la más reciente
/// primero, como mucho [`MAX_VERSIONES_POR_ARCHIVO`]) y los archivos por la
/// versión más reciente en la que están (y por ruta).
fn agrupar(por_ruta: HashMap<String, Vec<EnVersion>>) -> Vec<Encontrado> {
    let mut out: Vec<Encontrado> = por_ruta
        .into_iter()
        .map(|(ruta, mut versiones)| {
            versiones.sort_by_key(|v| std::cmp::Reverse(instante(&v.cuando)));
            let recortado = versiones.len() > MAX_VERSIONES_POR_ARCHIVO;
            versiones.truncate(MAX_VERSIONES_POR_ARCHIVO);
            Encontrado { ruta, versiones, recortado }
        })
        .collect();
    out.sort_by(|a, b| {
        let ultima = |x: &Encontrado| x.versiones.first().and_then(|v| instante(&v.cuando));
        ultima(b).cmp(&ultima(a)).then_with(|| a.ruta.cmp(&b.ruta))
    });
    out
}

/// Busca `texto` en el nombre de los archivos de las versiones de `desde` a
/// `hasta` (RFC 3339; sin ellas, todas), como mucho `max` coincidencias
/// (1 a [`MAX_COINCIDENCIAS`]) y `limite` de tiempo: pasado, se da lo
/// encontrado (`recortado`, `motivo: tiempo`).
pub fn buscar(acc: &Access, texto: &str, desde: Option<&str>, hasta: Option<&str>, max: usize, limite: Duration) -> Result<Busqueda, String> {
    let texto = texto_valido(texto)?;
    let (d, h) = rango(desde, hasta)?;
    let max = max.clamp(1, MAX_COINCIDENCIAS);
    let mut versiones = versiones_del_rango(restic::snapshots(acc)?, d, h);
    let versiones_en_rango = versiones.len();
    versiones.truncate(MAX_VERSIONES);
    let mut b = Busqueda {
        texto: texto.clone(),
        archivos: Vec::new(),
        coincidencias: 0,
        versiones_buscadas: versiones.len(),
        versiones_en_rango,
        recortado: false,
        motivo: None,
    };
    if versiones.is_empty() {
        return Ok(b);
    }
    // Las versiones, de la más reciente a la más antigua: si se llega al
    // máximo, lo que falta es lo más antiguo.
    let mut args: Vec<String> = ["find", "--json", "--no-lock", "--ignore-case"].iter().map(|s| s.to_string()).collect();
    for v in &versiones {
        if !restic::valid_snapshot_id(&v.short_id) {
            continue;
        }
        args.push("--snapshot".into());
        args.push(v.short_id.clone());
    }
    args.push("--".into());
    args.push(patron(&texto));

    let mut hijo = restic::spawn(acc, &args)?;
    let salida = hijo.stdout.take().ok_or("No se pudo leer la salida de restic.")?;
    let mut errores = hijo.stderr.take().ok_or("No se pudo leer la salida de restic.")?;
    let hilo_err = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = (&mut errores).take(64 * 1024).read_to_end(&mut buf);
        let _ = std::io::copy(&mut errores, &mut std::io::sink());
        String::from_utf8_lossy(&buf).into_owned()
    });
    // Un vigilante corta restic si se pasa del tiempo (la lectura acaba entonces sola).
    let hijo = Arc::new(Mutex::new(hijo));
    let (hecho, vencido) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)));
    let vigilante = {
        let (hijo, hecho, vencido) = (hijo.clone(), hecho.clone(), vencido.clone());
        std::thread::spawn(move || {
            let inicio = Instant::now();
            while !hecho.load(Ordering::Relaxed) {
                if inicio.elapsed() >= limite {
                    vencido.store(true, Ordering::Relaxed);
                    if let Ok(mut h) = hijo.lock() {
                        restic::kill_tree(&mut h);
                    }
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        })
    };
    let (leido, lleno, coincidencias, por_ruta) = leer(salida, &texto, &versiones, max);
    hecho.store(true, Ordering::Relaxed);
    let _ = vigilante.join();
    let estado = {
        let mut h = hijo.lock().map_err(|_| "No se pudo esperar a restic.")?;
        if lleno {
            restic::kill_tree(&mut h);
        }
        h.wait().ok()
    };
    let stderr = hilo_err.join().unwrap_or_default();
    let vencido = vencido.load(Ordering::Relaxed);
    if !lleno && !vencido {
        let codigo = estado.and_then(|s| s.code());
        if codigo != Some(0) {
            return Err(restic::exit_error(codigo, &stderr));
        }
        leido.map_err(|e| format!("No se entendió la respuesta de restic: {e}"))?;
    }
    b.coincidencias = coincidencias;
    b.archivos = agrupar(por_ruta);
    b.recortado = lleno || vencido || b.archivos.iter().any(|a| a.recortado);
    b.motivo = if vencido {
        Some(Motivo::Tiempo)
    } else if lleno {
        Some(Motivo::Limite)
    } else {
        None
    };
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(id: &str, t: &str) -> Snapshot {
        serde_json::from_value(serde_json::json!({
            "id": format!("{id}{}", "0".repeat(56)), "short_id": id, "time": t, "hostname": "CAJA", "paths": ["/x"]
        }))
        .unwrap()
    }

    #[test]
    fn texto_y_patron() {
        assert_eq!(texto_valido("  factura ").unwrap(), "factura");
        for mal in ["", " ", "a", "a/b", "a\\b", "x\0y", "fa\nctura"] {
            assert!(texto_valido(mal).is_err(), "{mal:?}");
        }
        assert!(texto_valido(&"a".repeat(TEXTO_MAX)).is_ok());
        assert!(texto_valido(&"a".repeat(TEXTO_MAX + 1)).is_err());
        assert!(texto_valido(&"ñ".repeat(TEXTO_MAX)).is_ok(), "se cuentan caracteres, no bytes");
        // Los caracteres especiales de los patrones pasan a `*` (no se escapan: en Windows `\` separa carpetas).
        assert_eq!(patron("factura"), "*factura*");
        assert_eq!(patron("Factura [1]"), "*Factura *1**");
        assert_eq!(patron("¿qué? *.txt"), "*¿qué* *.txt*");
        assert_eq!(patron("-rf"), "*-rf*", "nunca empieza por «-» (no es una opción)");
        assert!(nombre_coincide("/C/Datos/Factura [1].txt", "factura [1]"));
        assert!(nombre_coincide("/C/Datos/INFORME.PDF", "informe"));
        assert!(!nombre_coincide("/C/Facturas/otro.txt", "factura"), "solo el nombre, no la carpeta");
        assert!(!nombre_coincide("/C/Datos/Factura 1.txt", "factura [1]"), "el patrón encuentra de más; el filtro no");
    }

    #[test]
    fn rango_de_fechas() {
        assert_eq!(rango(None, None).unwrap(), (None, None));
        assert_eq!(rango(Some(""), Some("")).unwrap(), (None, None));
        assert!(rango(Some("2026-10-01T00:00:00Z"), Some("2026-10-02T00:00:00-05:00")).is_ok());
        assert!(rango(Some("ayer"), None).is_err());
        assert!(rango(Some("2026-10-01"), None).is_err(), "con hora y huso");
        assert!(rango(Some("2026-10-03T00:00:00Z"), Some("2026-10-02T00:00:00Z")).is_err());
        let vs = vec![snap("aaaaaaaa", "2026-10-01T10:00:00Z"), snap("cccccccc", "2026-10-03T10:00:00Z"), snap("bbbbbbbb", "2026-10-02T10:00:00-05:00")];
        let todas = versiones_del_rango(vs.clone(), None, None);
        assert_eq!(todas.iter().map(|s| s.short_id.as_str()).collect::<Vec<_>>(), ["cccccccc", "bbbbbbbb", "aaaaaaaa"]);
        let (d, h) = rango(Some("2026-10-02T00:00:00Z"), Some("2026-10-02T23:59:59Z")).unwrap();
        assert_eq!(versiones_del_rango(vs, d, h).iter().map(|s| s.short_id.as_str()).collect::<Vec<_>>(), ["bbbbbbbb"]);
    }

    fn salida(n_versiones: usize, por_version: usize) -> (Vec<Snapshot>, String) {
        let vs: Vec<Snapshot> =
            (0..n_versiones).map(|i| snap(&format!("{:08x}", 0xa000_0000u32 + i as u32), &format!("2026-10-{:02}T10:00:00Z", 1 + i % 28))).collect();
        let objs: Vec<serde_json::Value> = vs
            .iter()
            .map(|s| {
                let mut m: Vec<serde_json::Value> = (0..por_version)
                    .map(|j| serde_json::json!({ "path": format!("/x/Factura-{j}.txt"), "type": "file", "size": j, "mtime": "2026-10-01T09:00:00Z", "permissions": "-rw-r--r--", "uid": 0 }))
                    .collect();
                // Una carpeta que se llama como el texto y lo de dentro: no cuentan.
                m.push(serde_json::json!({ "path": "/x/Facturas", "type": "dir" }));
                m.push(serde_json::json!({ "path": "/x/Facturas/otro.pdf", "type": "file", "size": 1 }));
                serde_json::json!({ "matches": m, "hits": m.len(), "snapshot": s.id })
            })
            .collect();
        (vs, serde_json::Value::Array(objs).to_string())
    }

    #[test]
    fn lee_y_agrupa_por_archivo() {
        let (vs, out) = salida(3, 2);
        let (r, lleno, n, por_ruta) = leer(out.as_bytes(), "FACTURA", &vs, 100);
        assert!(r.is_ok() && !lleno);
        assert_eq!(n, 6);
        let g = agrupar(por_ruta);
        assert_eq!(g.iter().map(|a| a.ruta.as_str()).collect::<Vec<_>>(), ["/x/Factura-0.txt", "/x/Factura-1.txt"]);
        assert_eq!(g[1].versiones.len(), 3);
        assert_eq!(g[1].versiones[0].cuando, "2026-10-03T10:00:00Z", "la más reciente primero");
        assert_eq!((g[1].versiones[0].bytes, g[1].versiones[0].modificado.as_deref()), (Some(1), Some("2026-10-01T09:00:00Z")));
        // Versiones que no se pidieron (o basura): se ignoran.
        let (r, _, n, _) = leer(out.as_bytes(), "factura", &vs[..1], 100);
        assert!(r.is_ok());
        assert_eq!(n, 2, "solo las de las versiones pedidas");
        let (r, _, _, _) = leer(&b"no es json"[..], "factura", &vs, 100);
        assert!(r.is_err());
        let (r, _, n, _) = leer(&b"[]"[..], "factura", &vs, 100);
        assert!(r.is_ok() && n == 0);
    }

    #[test]
    fn para_al_llegar_al_maximo() {
        let (vs, out) = salida(50, 100);
        let (r, lleno, n, por_ruta) = leer(out.as_bytes(), "factura", &vs, 250);
        assert!(r.is_ok() && lleno);
        assert_eq!(n, 250);
        assert_eq!(por_ruta.values().map(Vec::len).sum::<usize>(), 250);
        // Un archivo en más versiones de las que se dan: recortado.
        let (vs, out) = salida(MAX_VERSIONES_POR_ARCHIVO + 3, 1);
        let (_, lleno, _, por_ruta) = leer(out.as_bytes(), "factura", &vs, MAX_COINCIDENCIAS);
        assert!(!lleno);
        let g = agrupar(por_ruta);
        assert!(g[0].recortado && g[0].versiones.len() == MAX_VERSIONES_POR_ARCHIVO);
    }

    /// No guarda la salida entera: lee un millón de coincidencias que no
    /// pasan el filtro sin que el resultado crezca.
    #[test]
    fn salida_grande_sin_guardarla() {
        struct Generador {
            hecho: usize,
            buf: Vec<u8>,
            pos: usize,
        }
        impl Read for Generador {
            fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
                if self.pos >= self.buf.len() {
                    self.buf.clear();
                    self.pos = 0;
                    match self.hecho {
                        0 => self.buf.extend_from_slice(br#"[{"matches":["#),
                        n if n <= 1_000_000 => {
                            let coma = if n > 1 { "," } else { "" };
                            self.buf.extend_from_slice(format!(r#"{coma}{{"path":"/x/Facturas/a-{n}.bin","type":"file","size":{n}}}"#).as_bytes());
                        }
                        1_000_001 => self.buf.extend_from_slice(format!(r#"],"hits":1000000,"snapshot":"a0000000{}"}}]"#, "0".repeat(56)).as_bytes()),
                        _ => return Ok(0),
                    }
                    self.hecho += 1;
                }
                let n = out.len().min(self.buf.len() - self.pos);
                out[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
                self.pos += n;
                Ok(n)
            }
        }
        let vs = vec![snap("a0000000", "2026-10-01T10:00:00Z")];
        let (r, lleno, n, por_ruta) = leer(Generador { hecho: 0, buf: Vec::new(), pos: 0 }, "factura", &vs, 10);
        assert!(r.is_ok(), "{r:?}");
        assert!(!lleno && n == 0 && por_ruta.is_empty());
    }

    /// Con restic de verdad (el del PATH): un archivo en dos versiones con
    /// otro tamaño, corchetes en el nombre, mayúsculas, el rango de fechas, el
    /// máximo y lo de dentro de una carpeta que se llama como el texto.
    #[test]
    fn buscar_con_restic_de_verdad() {
        if restic::version().is_err() {
            eprintln!("omitido: no hay restic");
            return;
        }
        let _g = restic::tests::real_repo_lock();
        let base = std::env::temp_dir().join(format!("resguardo-buscar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let datos = base.join("datos");
        std::fs::create_dir_all(datos.join("Facturas")).unwrap();
        std::fs::write(datos.join("Factura [1].txt"), "uno").unwrap();
        std::fs::write(datos.join("Facturas").join("otro.pdf"), "x").unwrap();
        std::fs::write(datos.join("notas.md"), "n").unwrap();
        let acc = Access::new(base.join("repo").display().to_string(), "contraseña de prueba");
        let lim = Duration::from_secs(120);
        assert_eq!(restic::run_raw(&acc, &["init"], lim).unwrap().code, Some(0));
        let d = datos.display().to_string();
        assert_eq!(restic::run_raw(&acc, &["backup", &d], lim).unwrap().code, Some(0));
        std::fs::write(datos.join("Factura [1].txt"), "uno y dos").unwrap();
        std::fs::write(datos.join("factura-2.TXT"), "otra").unwrap();
        assert_eq!(restic::run_raw(&acc, &["backup", &d], lim).unwrap().code, Some(0));
        let mut vs = restic::snapshots(&acc).unwrap();
        vs.sort_by(|a, b| a.time.cmp(&b.time));

        let b = buscar(&acc, "FACTURA", None, None, MAX_COINCIDENCIAS, TIEMPO).unwrap();
        assert!(!b.recortado && b.motivo.is_none(), "{b:?}");
        assert_eq!((b.versiones_buscadas, b.versiones_en_rango, b.coincidencias), (2, 2, 3));
        let nombres: Vec<&str> = b.archivos.iter().map(|a| a.ruta.rsplit('/').next().unwrap()).collect();
        assert_eq!(nombres, ["Factura [1].txt", "factura-2.TXT"], "{b:?}");
        let f1 = &b.archivos[0];
        assert_eq!(f1.versiones.iter().map(|v| v.bytes).collect::<Vec<_>>(), [Some(9), Some(3)]);
        assert_eq!(f1.versiones[0].version, vs[1].short_id);
        // Con corchetes: se encuentra igual (el patrón los cambia por `*`).
        let b = buscar(&acc, "a [1]", None, None, MAX_COINCIDENCIAS, TIEMPO).unwrap();
        assert_eq!(b.archivos.len(), 1);
        // Solo la primera versión (por fechas).
        let hasta = vs[0].time.clone();
        let b = buscar(&acc, "factura", None, Some(&hasta), MAX_COINCIDENCIAS, TIEMPO).unwrap();
        assert_eq!((b.versiones_buscadas, b.archivos.len()), (1, 1));
        // El máximo.
        let b = buscar(&acc, "factura", None, None, 1, TIEMPO).unwrap();
        assert_eq!((b.coincidencias, b.recortado, b.motivo), (1, true, Some(Motivo::Limite)));
        // Nada.
        let b = buscar(&acc, "no-existe-esto", None, None, MAX_COINCIDENCIAS, TIEMPO).unwrap();
        assert!(b.archivos.is_empty() && !b.recortado);
        // Sin versiones en el rango: ni se llama a find.
        let b = buscar(&acc, "factura", Some("2000-01-01T00:00:00Z"), Some("2000-01-02T00:00:00Z"), MAX_COINCIDENCIAS, TIEMPO).unwrap();
        assert_eq!((b.versiones_buscadas, b.versiones_en_rango), (0, 0));
        assert!(buscar(&acc, "x", None, None, 10, TIEMPO).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }
}
