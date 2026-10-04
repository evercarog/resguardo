//! Copia de la consola (docs/api-servidor.md, §2, «Copia de la consola», y
//! docs/agente-gestionado.md, «Si pierdes la consola», camino 2).
//!
//! Lo que hace que una consola se pueda restaurar **con la misma identidad**
//! (y que los agentes la reconozcan sin volver a vincularse): las bases de
//! datos (`control.db` y `clientes/*.db`), `identidad.key`, la autoridad TLS
//! (`tls/`), los paquetes de cliente guardados y este mismo ajuste.
//!
//! - Cada noche (a la hora fijada) y al arrancar una versión nueva (antes de
//!   tocar la base de datos), el servidor hace una instantánea coherente de
//!   cada base de datos con `VACUUM INTO` (el servidor sigue funcionando
//!   mientras) y lo empaqueta todo **cifrado** para la clave pública de la
//!   «clave de respaldo de la consola» (`resguardo_protocolo::respaldo_consola`).
//!   El servidor nunca tiene la clave: la consola deriva la pública en el navegador.
//! - Deja el archivo en `<datos>/respaldos/` y conserva los N más recientes.
//!   Para tenerlo fuera de la máquina, el agente de esa misma máquina lo copia
//!   como una copia más (carpeta `<datos>/respaldos`, «Consola de Resguardo»),
//!   o un almacén lo recibe como cualquier otra copia: se reutiliza todo lo que
//!   ya hay (restic, retención, copia externa, espejo).
//! - `resguardo-server restaurar-respaldo <archivo>` lo descifra con la clave y
//!   deja la carpeta de datos como estaba (con su identidad).
//!
//! Formato del contenido (antes de cifrar): por cada archivo,
//! `u16 big-endian (largo de la ruta) ‖ ruta (UTF-8, con «/») ‖ u64 big-endian (largo) ‖ datos`,
//! y al final un `u16` a cero.

use resguardo_protocolo::respaldo_consola as rc;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const ARCHIVO_AJUSTES: &str = "respaldo-consola.json";
pub const CARPETA: &str = "respaldos";
pub const EXTENSION: &str = "resguardo-consola";
const FIN: u16 = 0;

/// Lo que guarda el servidor del ajuste (sin la clave: solo su pública y su sal).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Ajustes {
    #[serde(default)]
    pub activo: bool,
    /// Clave pública X25519 de la clave de respaldo (base64).
    #[serde(default)]
    pub publica: Option<String>,
    /// Sal de la clave de respaldo (base64).
    #[serde(default)]
    pub sal: Option<String>,
    /// Cuándo se puso la clave (RFC 3339).
    #[serde(default)]
    pub clave_puesta: Option<String>,
    /// Copias que se conservan (las más recientes).
    #[serde(default = "conservar_por_defecto")]
    pub conservar: u32,
    /// Hora de la copia diaria («HH:MM», hora local del servidor).
    #[serde(default = "hora_por_defecto")]
    pub hora: String,
    #[serde(default)]
    pub ultima: Option<Ultima>,
    /// Versión del servidor de la última copia correcta (para hacer otra al actualizar).
    #[serde(default)]
    pub version: Option<String>,
    /// Último intento de la copia diaria (RFC 3339) y si salió bien.
    #[serde(default)]
    pub diaria: Option<String>,
    #[serde(default)]
    pub diaria_ok: bool,
}

fn conservar_por_defecto() -> u32 {
    7
}
fn hora_por_defecto() -> String {
    "03:30".into()
}

impl Default for Ajustes {
    fn default() -> Self {
        Self {
            activo: false,
            publica: None,
            sal: None,
            clave_puesta: None,
            conservar: 7,
            hora: hora_por_defecto(),
            ultima: None,
            version: None,
            diaria: None,
            diaria_ok: false,
        }
    }
}

/// Cómo fue la última copia.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Ultima {
    pub cuando: String,
    pub ok: bool,
    pub mensaje: String,
    /// Nombre del archivo (sin carpeta).
    #[serde(default)]
    pub archivo: Option<String>,
    #[serde(default)]
    pub bytes: Option<u64>,
    /// «diaria», «manual» o «actualizacion».
    #[serde(default)]
    pub motivo: String,
}

pub fn leer(datos: &Path) -> Ajustes {
    std::fs::read(datos.join(ARCHIVO_AJUSTES)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn guardar(datos: &Path, a: &Ajustes) -> Result<(), String> {
    crate::identidad::escribir_privado(&datos.join(ARCHIVO_AJUSTES), &serde_json::to_vec_pretty(a).map_err(|e| e.to_string())?)
}

pub fn carpeta(datos: &Path) -> PathBuf {
    datos.join(CARPETA)
}

/// Las copias que hay en la carpeta, de la más reciente a la más antigua: (nombre, bytes).
pub fn copias(datos: &Path) -> Vec<(String, u64)> {
    let mut v: Vec<(String, u64)> = std::fs::read_dir(carpeta(datos))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            (n.starts_with("consola-") && n.ends_with(&format!(".{EXTENSION}"))).then(|| (n, e.metadata().map(|m| m.len()).unwrap_or(0)))
        })
        .collect();
    // El nombre lleva la fecha (consola-AAAAMMDD-HHMMSS-mmm): ordenar por nombre es por fecha.
    v.sort_by(|a, b| b.0.cmp(&a.0));
    v
}

// ---------- Hacer la copia ----------

/// Una instantánea coherente de una base de datos SQLite (con el servidor en marcha).
fn instantanea(origen: &Path, destino: &Path) -> Result<(), String> {
    let _ = std::fs::remove_file(destino);
    // Una conexión aparte (la del servidor sigue a lo suyo); VACUUM INTO solo lee el origen.
    let c = rusqlite::Connection::open(origen).map_err(|e| format!("{}: {e}", origen.display()))?;
    c.busy_timeout(std::time::Duration::from_secs(30)).map_err(|e| e.to_string())?;
    c.execute("VACUUM INTO ?1", [destino.to_string_lossy().as_ref()]).map_err(|e| format!("{}: {e}", origen.display()))?;
    Ok(())
}

fn escribir_entrada(w: &mut impl Write, ruta: &str, archivo: &Path) -> Result<(), String> {
    let mut f = std::fs::File::open(archivo).map_err(|e| format!("{}: {e}", archivo.display()))?;
    let largo = f.metadata().map_err(|e| e.to_string())?.len();
    w.write_all(&(ruta.len() as u16).to_be_bytes()).map_err(|e| e.to_string())?;
    w.write_all(ruta.as_bytes()).map_err(|e| e.to_string())?;
    w.write_all(&largo.to_be_bytes()).map_err(|e| e.to_string())?;
    let copiados = std::io::copy(&mut (&mut f).take(largo), w).map_err(|e| e.to_string())?;
    if copiados != largo {
        return Err(format!("{} cambió mientras se copiaba.", archivo.display()));
    }
    Ok(())
}

/// Archivos de una carpeta (sin subcarpetas ni ocultos), por nombre.
fn archivos_de(dir: &Path, filtro: impl Fn(&str) -> bool) -> Vec<(String, PathBuf)> {
    let mut v: Vec<(String, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
        .filter(|(n, _)| !n.starts_with('.') && nombre_seguro(n) && filtro(n))
        .collect();
    v.sort();
    v
}

fn nombre_seguro(n: &str) -> bool {
    !n.is_empty() && n.len() <= 120 && !n.starts_with('.') && n.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// Solo una copia a la vez (la diaria, «Hacer ahora» y la de la actualización).
static EN_MARCHA: Mutex<()> = Mutex::new(());

/// Carpeta temporal (en la de datos) con las instantáneas en claro de las bases de datos.
const PREFIJO_INSTANTANEA: &str = ".respaldo-instantanea-";
/// Una instantánea más vieja que esto es de una copia que se cortó (otro proceso, como
/// `hacer-respaldo`, puede estar haciendo una ahora: esa no se toca).
const INSTANTANEA_VIEJA: std::time::Duration = std::time::Duration::from_secs(6 * 3600);

/// Quita las instantáneas en claro que dejó una copia cortada: las de ahora
/// (en la carpeta de datos) y las de versiones anteriores (en `respaldos`, que
/// se copia tal cual), estas siempre.
fn limpiar_instantaneas(datos: &Path) {
    let viejas = |dir: &Path, prefijo: &str, siempre: bool| {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            let es_dir = e.file_type().is_ok_and(|t| t.is_dir());
            let vieja = siempre || e.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).is_some_and(|d| d > INSTANTANEA_VIEJA);
            if n.starts_with(prefijo) && es_dir && vieja {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    };
    viejas(datos, PREFIJO_INSTANTANEA, false);
    viejas(&carpeta(datos), ".instantanea-", true);
}

/// Hace una copia ahora y la anota en los ajustes. `identidad`: la del servidor
/// (firma la copia; ver `resguardo_protocolo::respaldo_consola`).
pub fn hacer(datos: &Path, identidad: &ed25519_dalek::SigningKey, motivo: &str) -> Result<Ultima, String> {
    let _uno = EN_MARCHA.lock().unwrap_or_else(|e| e.into_inner());
    let ajustes = leer(datos);
    let resultado = hacer_sin_anotar(datos, identidad, &ajustes);
    let ahora = chrono::Local::now().to_rfc3339();
    let ultima = match &resultado {
        Ok((archivo, bytes)) => Ultima {
            cuando: ahora,
            ok: true,
            mensaje: format!("Copia de la consola hecha ({}).", tamano(*bytes)),
            archivo: Some(archivo.clone()),
            bytes: Some(*bytes),
            motivo: motivo.into(),
        },
        Err(e) => Ultima { cuando: ahora, ok: false, mensaje: e.clone(), archivo: None, bytes: None, motivo: motivo.into() },
    };
    // Se relee: los ajustes pudieron cambiar mientras se hacía.
    let mut a = leer(datos);
    a.ultima = Some(ultima.clone());
    if motivo == "diaria" {
        a.diaria = Some(ultima.cuando.clone());
        a.diaria_ok = ultima.ok;
    }
    if ultima.ok {
        a.version = Some(env!("CARGO_PKG_VERSION").into());
    }
    guardar(datos, &a)?;
    resultado.map(|_| ultima)
}

fn tamano(b: u64) -> String {
    match b {
        b if b >= 1 << 30 => format!("{:.1} GB", b as f64 / (1u64 << 30) as f64).replace('.', ","),
        b if b >= 1 << 20 => format!("{:.1} MB", b as f64 / (1u64 << 20) as f64).replace('.', ","),
        b => format!("{} KB", b.div_ceil(1024)),
    }
}

fn hacer_sin_anotar(datos: &Path, identidad: &ed25519_dalek::SigningKey, a: &Ajustes) -> Result<(String, u64), String> {
    let (Some(publica), Some(sal)) = (a.publica.as_deref(), a.sal.as_deref()) else {
        return Err("Falta poner la clave de respaldo de la consola.".into());
    };
    if !rc::publica_valida(publica) {
        return Err("La clave pública guardada no es válida: vuelve a poner la clave de respaldo.".into());
    }
    let dir = carpeta(datos);
    std::fs::create_dir_all(&dir).map_err(|e| format!("No se pudo crear {}: {e}", dir.display()))?;
    limpiar_instantaneas(datos);
    // Las instantáneas van EN CLARO: fuera de `respaldos`, que el agente de esta
    // máquina copia tal cual (allí solo debe haber archivos cifrados).
    let temporal = datos.join(format!("{PREFIJO_INSTANTANEA}{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&temporal).map_err(|e| e.to_string())?;
    let r = (|| {
        // 1. Instantáneas de las bases de datos (el servidor sigue con ellas mientras).
        let mut entradas: Vec<(String, PathBuf)> = Vec::new();
        let control = temporal.join("control.db");
        instantanea(&datos.join("control.db"), &control)?;
        entradas.push(("control.db".into(), control));
        for (n, p) in archivos_de(&datos.join("clientes"), |n| n.ends_with(".db")) {
            let destino = temporal.join(format!("cliente-{n}"));
            instantanea(&p, &destino)?;
            entradas.push((format!("clientes/{n}"), destino));
        }
        // 2. Identidad, autoridad TLS, paquetes guardados y este ajuste.
        let identidad_key = datos.join("identidad.key");
        if !identidad_key.is_file() {
            return Err("No está identidad.key en la carpeta de datos.".to_string());
        }
        entradas.push(("identidad.key".into(), identidad_key));
        entradas.extend(archivos_de(&datos.join("tls"), |_| true).into_iter().map(|(n, p)| (format!("tls/{n}"), p)));
        entradas.extend(archivos_de(&datos.join("paquetes"), |_| true).into_iter().map(|(n, p)| (format!("paquetes/{n}"), p)));
        if datos.join(ARCHIVO_AJUSTES).is_file() {
            entradas.push((ARCHIVO_AJUSTES.into(), datos.join(ARCHIVO_AJUSTES)));
        }
        // 3. Cifrado, a un archivo temporal que se cambia de nombre al terminar.
        let ahora = chrono::Local::now();
        let nombre = format!("consola-{}.{EXTENSION}", ahora.format("%Y%m%d-%H%M%S-%3f"));
        let parcial = dir.join(format!(".{nombre}.parcial"));
        let f = std::fs::File::create(&parcial).map_err(|e| format!("No se pudo crear {}: {e}", parcial.display()))?;
        let mut c = rc::Cifrador::nuevo(std::io::BufWriter::new(f), sal, publica, &ahora.to_rfc3339(), identidad, env!("CARGO_PKG_VERSION"))?;
        for (ruta, p) in &entradas {
            escribir_entrada(&mut c, ruta, p)?;
        }
        c.write_all(&FIN.to_be_bytes()).map_err(|e| e.to_string())?;
        let f = c.terminar()?.into_inner().map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        drop(f);
        let final_ = dir.join(&nombre);
        std::fs::rename(&parcial, &final_).map_err(|e| e.to_string())?;
        let bytes = std::fs::metadata(&final_).map(|m| m.len()).unwrap_or(0);
        Ok((nombre, bytes))
    })();
    let _ = std::fs::remove_dir_all(&temporal);
    if r.is_err() {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            if e.file_name().to_string_lossy().ends_with(".parcial") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let r = r?;
    // 4. Solo las N más recientes.
    for (n, _) in copias(datos).into_iter().skip(a.conservar.max(1) as usize) {
        let _ = std::fs::remove_file(dir.join(n));
    }
    Ok(r)
}

/// ¿Toca la copia diaria? (activa, con clave, pasada la hora y sin una correcta hoy).
pub fn toca(a: &Ajustes, ahora: chrono::DateTime<chrono::Local>) -> bool {
    if !a.activo || a.publica.is_none() {
        return false;
    }
    let Ok(h) = chrono::NaiveTime::parse_from_str(&a.hora, "%H:%M") else { return false };
    if ahora.time() < h {
        return false;
    }
    let hoy = ahora.format("%Y-%m-%d").to_string();
    // Una al día: si hoy ya hubo una (bien, o mal hace menos de una hora), no.
    match &a.diaria {
        Some(d) if d.starts_with(&hoy) => {
            !a.diaria_ok && chrono::DateTime::parse_from_rfc3339(d).is_ok_and(|t| ahora.signed_duration_since(t) > chrono::Duration::hours(1))
        }
        _ => true,
    }
}

/// La tarea de cada pocos minutos del servidor.
pub fn si_toca(datos: &Path, identidad: &ed25519_dalek::SigningKey) {
    if toca(&leer(datos), chrono::Local::now()) {
        match hacer(datos, identidad, "diaria") {
            Ok(u) => println!("{}", u.mensaje),
            Err(e) => eprintln!("Copia de la consola: {e}"),
        }
    }
}

/// Al arrancar una versión nueva, antes de abrir (y quizá cambiar) la base de datos.
pub fn antes_de_actualizar(datos: &Path) -> Option<Result<Ultima, String>> {
    let a = leer(datos);
    if !a.activo || a.publica.is_none() || a.version.as_deref().is_none_or(|v| v == env!("CARGO_PKG_VERSION")) || !datos.join("control.db").is_file() {
        return None;
    }
    Some(crate::identidad::identidad(datos).and_then(|identidad| hacer(datos, &identidad, "actualizacion")))
}

// ---------- Restaurar ----------

/// Lo que se restauró.
#[derive(Debug)]
pub struct Restaurado {
    pub identidad: String,
    pub creado: String,
    pub version: String,
    /// ¿Iba firmada (versión 2)? Las de antes de 0.7.11 no.
    pub firmada: bool,
    pub archivos: usize,
    /// Dónde quedó lo que había antes (con `reemplazar`).
    pub antes: Option<PathBuf>,
}

pub(crate) fn ruta_valida(r: &str) -> bool {
    match r.split_once('/') {
        None => matches!(r, "control.db" | "identidad.key" | ARCHIVO_AJUSTES),
        Some((dir, n)) => matches!(dir, "clientes" | "tls" | "paquetes") && nombre_seguro(n),
    }
}

/// Lee el contenido descifrado y escribe cada archivo en `destino` (sin salir de ahí).
struct Desempaquetador {
    destino: PathBuf,
    cab: Vec<u8>,
    actual: Option<(std::fs::File, u64)>,
    terminado: bool,
    archivos: usize,
}

impl Desempaquetador {
    fn error(m: impl Into<String>) -> std::io::Error {
        std::io::Error::new(std::io::ErrorKind::InvalidData, m.into())
    }
}

impl Write for Desempaquetador {
    fn write(&mut self, mut datos: &[u8]) -> std::io::Result<usize> {
        let total = datos.len();
        while !datos.is_empty() {
            if self.terminado {
                return Err(Self::error("Copia dañada (sobran datos)."));
            }
            if let Some((f, falta)) = self.actual.as_mut() {
                let n = (*falta).min(datos.len() as u64) as usize;
                f.write_all(&datos[..n])?;
                *falta -= n as u64;
                datos = &datos[n..];
                if *falta == 0 {
                    let (f, _) = self.actual.take().expect("hay archivo");
                    f.sync_all()?;
                }
                continue;
            }
            // Cabecera de una entrada: u16 + ruta + u64.
            self.cab.push(datos[0]);
            datos = &datos[1..];
            if self.cab.len() < 2 {
                continue;
            }
            let largo = u16::from_be_bytes([self.cab[0], self.cab[1]]) as usize;
            if largo == FIN as usize {
                self.terminado = true;
                continue;
            }
            if largo > 300 {
                return Err(Self::error("Copia dañada (ruta)."));
            }
            if self.cab.len() < 2 + largo + 8 {
                continue;
            }
            let ruta = std::str::from_utf8(&self.cab[2..2 + largo]).map_err(|_| Self::error("Copia dañada (ruta)."))?.to_string();
            if !ruta_valida(&ruta) {
                return Err(Self::error(format!("Copia dañada (ruta no esperada: {ruta}).")));
            }
            let bytes = u64::from_be_bytes(self.cab[2 + largo..2 + largo + 8].try_into().expect("8 bytes"));
            self.cab.clear();
            let path = self.destino.join(&ruta);
            if let Some(p) = path.parent() {
                std::fs::create_dir_all(p)?;
            }
            let f = crear_privado(&path)?;
            self.archivos += 1;
            if bytes == 0 {
                f.sync_all()?;
            } else {
                self.actual = Some((f, bytes));
            }
        }
        Ok(total)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Pasa a un `Desempaquetador` el contenido (descifrado) en esos trozos (las
/// pruebas de propiedades): (archivos, terminado).
#[cfg(test)]
pub(crate) fn desempaquetar_para_pruebas<'a>(destino: &Path, trozos: impl Iterator<Item = &'a [u8]>) -> std::io::Result<(usize, bool)> {
    let mut d = Desempaquetador { destino: destino.to_path_buf(), cab: Vec::new(), actual: None, terminado: false, archivos: 0 };
    for t in trozos {
        d.write_all(t)?;
    }
    Ok((d.archivos, d.terminado))
}

fn crear_privado(path: &Path) -> std::io::Result<std::fs::File> {
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    o.open(path)
}

/// Lo que forma la carpeta de datos de un servidor (lo que se aparta al reemplazar).
const DEL_SERVIDOR: &[&str] =
    &["control.db", "control.db-wal", "control.db-shm", "identidad.key", "tls", "clientes", "paquetes", ARCHIVO_AJUSTES, "codigo-arranque.txt", "relevos"];

/// Restaura una copia en la carpeta de datos `datos`. Primero lo descifra todo
/// aparte (con una clave equivocada, un archivo dañado o una firma que no es de
/// su identidad no se toca nada); si ya hay un servidor ahí, solo con
/// `reemplazar` (lo de antes se aparta, no se borra). La carpeta `respaldos` se
/// queda como está. `esperada`: la identidad (base64) que la persona confirmó
/// (`--confiar-en` o en la terminal); si la copia es de otra, no se sigue.
pub fn restaurar(archivo: &Path, clave: &str, datos: &Path, reemplazar: bool, esperada: &str) -> Result<Restaurado, String> {
    let abrir = || std::fs::File::open(archivo).map(std::io::BufReader::new).map_err(|e| format!("No se pudo abrir {}: {e}", archivo.display()));
    let (cab, _) = rc::leer_cabecera(&mut abrir()?)?;
    if cab.identidad != esperada {
        return Err(format!("La copia es de otro servidor (identidad {}), no de la que se confirmó ({}).", rc::huella(&cab.identidad), rc::huella(esperada)));
    }
    let hay_servidor = datos.join("control.db").exists() || datos.join("identidad.key").exists();
    if hay_servidor && !reemplazar {
        return Err(format!(
            "Ya hay un servidor en {}. Para el servicio y repite con --reemplazar (lo que hay se aparta, no se borra), o usa otra carpeta con --datos.",
            datos.display()
        ));
    }
    let secreto = rc::secreto(clave, &cab.sal)?;
    std::fs::create_dir_all(datos).map_err(|e| format!("No se pudo crear {}: {e}", datos.display()))?;
    let aparte = datos.join(format!(".restaurando-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&aparte).map_err(|e| e.to_string())?;
    let mut d = Desempaquetador { destino: aparte.clone(), cab: Vec::new(), actual: None, terminado: false, archivos: 0 };
    let r = rc::descifrar(abrir()?, &secreto, &mut d).and_then(|c| {
        if c.cabecera.identidad != esperada {
            return Err("La copia cambió mientras se restauraba.".to_string());
        }
        if d.terminado && d.actual.is_none() && aparte.join("control.db").is_file() && aparte.join("identidad.key").is_file() {
            Ok(c)
        } else {
            Err("La copia está incompleta.".to_string())
        }
    });
    // La identidad descifrada tiene que ser la que dice la cabecera: se
    // comprueba antes de tocar nada de la carpeta de datos.
    let r = r.and_then(|c| {
        let identidad = identidad_de(&aparte.join("identidad.key"))?;
        if identidad == c.cabecera.identidad {
            Ok((c, identidad))
        } else {
            Err("La identidad de la copia no coincide con la de su cabecera: el archivo está dañado.".to_string())
        }
    });
    let (comprobada, identidad) = match r {
        Ok(c) => c,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&aparte);
            return Err(e);
        }
    };
    // Lo de antes, aparte (no se borra nada).
    let mut movidos: Vec<&str> = Vec::new();
    let antes = if hay_servidor {
        let a = datos.join(format!("antes-de-restaurar-{}", chrono::Local::now().format("%Y%m%d-%H%M%S")));
        std::fs::create_dir_all(&a).map_err(|e| e.to_string())?;
        for n in DEL_SERVIDOR {
            let p = datos.join(n);
            if std::fs::symlink_metadata(&p).is_err() {
                continue;
            }
            if let Err(e) = std::fs::rename(&p, a.join(n)) {
                // Se deja todo como estaba.
                devolver(datos, &a, &movidos);
                let _ = std::fs::remove_dir(&a);
                let _ = std::fs::remove_dir_all(&aparte);
                return Err(format!("No se pudo apartar {} ({e}). ¿Está el servidor en marcha? Para el servicio antes de restaurar.", p.display()));
            }
            movidos.push(n);
        }
        Some(a)
    } else {
        None
    };
    // Lo restaurado, a su sitio. Si algo falla a medias, se quita lo puesto y vuelve lo de antes.
    let mut puestos: Vec<std::ffi::OsString> = Vec::new();
    let colocar = (|| {
        for e in std::fs::read_dir(&aparte).map_err(|e| e.to_string())?.flatten() {
            std::fs::rename(e.path(), datos.join(e.file_name())).map_err(|e| e.to_string())?;
            puestos.push(e.file_name());
        }
        Ok::<(), String>(())
    })();
    if let Err(e) = colocar {
        for n in &puestos {
            quitar(&datos.join(n));
        }
        if let Some(a) = &antes {
            devolver(datos, a, &movidos);
            let _ = std::fs::remove_dir(a);
        }
        let _ = std::fs::remove_dir_all(&aparte);
        return Err(format!("No se pudo dejar la copia en {} ({e}): la carpeta de datos queda como estaba.", datos.display()));
    }
    let _ = std::fs::remove_dir_all(&aparte);
    // En Linux, lo restaurado es del usuario del servicio (el dueño de la carpeta de
    // datos). Solo lo restaurado y sin seguir enlaces: lo demás de la carpeta lo pudo
    // dejar ese usuario, y root no debe darle archivos de otros por un enlace.
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let Ok(m) = std::fs::metadata(datos) {
            let (uid, gid) = (m.uid(), m.gid());
            let mut pendientes: Vec<PathBuf> = puestos.iter().map(|n| datos.join(n)).collect();
            while let Some(p) = pendientes.pop() {
                let Ok(meta) = std::fs::symlink_metadata(&p) else { continue };
                if meta.file_type().is_symlink() {
                    continue;
                }
                let _ = std::os::unix::fs::lchown(&p, Some(uid), Some(gid));
                if meta.is_dir() {
                    pendientes.extend(std::fs::read_dir(&p).into_iter().flatten().flatten().map(|e| e.path()));
                }
            }
        }
    }
    Ok(Restaurado {
        identidad,
        creado: comprobada.cabecera.creado,
        version: comprobada.cabecera.version,
        firmada: comprobada.firmada,
        archivos: d.archivos,
        antes,
    })
}

/// La identidad pública de un `identidad.key` (sin crear nada si no está).
fn identidad_de(archivo: &Path) -> Result<String, String> {
    use base64::Engine;
    let t = std::fs::read_to_string(archivo).map_err(|_| "La copia no trae identidad.key.".to_string())?;
    let semilla: [u8; 32] =
        base64::engine::general_purpose::STANDARD.decode(t.trim()).ok().and_then(|v| v.try_into().ok()).ok_or("identidad.key de la copia dañada.")?;
    Ok(crate::identidad::publica(&ed25519_dalek::SigningKey::from_bytes(&semilla)))
}

/// Vuelve a su sitio lo que se había apartado en `a`.
fn devolver(datos: &Path, a: &Path, movidos: &[&str]) {
    for m in movidos {
        let _ = std::fs::rename(a.join(m), datos.join(m));
    }
}

/// Quita un archivo o una carpeta (sin seguir enlaces).
fn quitar(p: &Path) {
    match std::fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() => {
            let _ = std::fs::remove_dir_all(p);
        }
        Ok(_) => {
            let _ = std::fs::remove_file(p);
        }
        Err(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLAVE: &str = "una clave de respaldo bien larga";

    fn servidor_con_datos() -> (tempfile::TempDir, crate::estado::St) {
        let dir = tempfile::tempdir().unwrap();
        crate::identidad::preparar_tls(dir.path(), &[]).unwrap();
        let st = crate::preparar(dir.path(), Default::default()).unwrap();
        st.db.crear_cuenta("ana@ejemplo.com", "Ana", "hash", true).unwrap();
        let c = st.db.crear_cliente("Café del Sur", "c2Fs", 24).unwrap();
        let ctx = crate::almacen::ClienteCtx::autorizado(&c.id);
        st.db.crear_aviso(&ctx, None, "cambio_inusual", "algo").unwrap();
        let sal = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, [5u8; 16]);
        let a = Ajustes { activo: true, publica: Some(rc::publica(CLAVE, &sal).unwrap()), sal: Some(sal), conservar: 2, ..Default::default() };
        guardar(dir.path(), &a).unwrap();
        (dir, st)
    }

    #[test]
    fn copia_y_restaura_con_la_misma_identidad_y_cuentas() {
        let (dir, st) = servidor_con_datos();
        let u = hacer(dir.path(), &st.identidad, "manual").unwrap();
        assert!(u.ok && u.bytes.unwrap() > 0, "{u:?}");
        let archivo = carpeta(dir.path()).join(u.archivo.as_ref().unwrap());
        // Sin la clave no se ve nada de dentro.
        let crudo = std::fs::read(&archivo).unwrap();
        assert!(!crudo.windows(7).any(|w| w == b"CREATE ") && !crudo.windows(14).any(|w| w == b"ana@ejemplo.co"));
        assert_eq!(leer(dir.path()).ultima.unwrap(), u);
        assert_eq!(leer(dir.path()).version.as_deref(), Some(env!("CARGO_PKG_VERSION")));

        // Restaurar en una carpeta vacía: la misma identidad, las mismas cuentas y clientes.
        let nueva = tempfile::tempdir().unwrap();
        let destino = nueva.path().join("datos");
        // Con otra clave, nada (y la carpeta sigue sin servidor).
        let id = st.identidad_pub.as_str();
        assert!(restaurar(&archivo, "otra clave cualquiera larga", &destino, false, id).unwrap_err().contains("no es la de esta copia"));
        assert!(!destino.join("control.db").exists());
        // Si se esperaba otra identidad, ni se descifra.
        let otra = crate::identidad::publica(&ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]));
        assert!(restaurar(&archivo, CLAVE, &destino, false, &otra).unwrap_err().contains("otro servidor"));
        assert!(!destino.exists() || !destino.join("control.db").exists());
        let r = restaurar(&archivo, CLAVE, &destino, false, id).unwrap();
        assert_eq!(r.identidad, st.identidad_pub);
        assert!(r.firmada, "las copias nuevas van firmadas");
        assert!(r.antes.is_none());
        let st2 = crate::preparar(&destino, Default::default()).unwrap();
        assert_eq!(st2.identidad_pub, st.identidad_pub, "los agentes la reconocen");
        assert_eq!(st2.huella_ca, st.huella_ca, "misma autoridad TLS");
        assert!(st2.db.cuenta_por_correo("ana@ejemplo.com").unwrap().is_some());
        let clientes = st2.db.todos_los_clientes().unwrap();
        assert_eq!(clientes.len(), 1);
        assert_eq!(st2.db.avisos(&clientes[0], false).unwrap().len(), 1, "y los datos de cada cliente");
        assert!(leer(&destino).activo, "la copia diaria sigue puesta");
        drop(st2);

        // Encima de un servidor: solo con «reemplazar», y lo de antes se aparta.
        assert!(restaurar(&archivo, CLAVE, &destino, false, id).unwrap_err().contains("--reemplazar"));
        let r = restaurar(&archivo, CLAVE, &destino, true, id).unwrap();
        assert!(r.antes.as_ref().unwrap().join("control.db").is_file());
    }

    /// Una copia cuya identidad no cuadra con su cabecera no deja nada a medias:
    /// el servidor que había sigue entero en su sitio.
    #[test]
    fn identidad_que_no_cuadra_no_toca_la_carpeta() {
        let (dir, st) = servidor_con_datos();
        // Firmada (bien) por otra identidad, pero con el identidad.key de este servidor dentro.
        let otra = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]);
        let mala = hacer(dir.path(), &otra, "manual").unwrap();
        let archivo = carpeta(dir.path()).join(mala.archivo.unwrap());
        let antes = std::fs::read(dir.path().join("identidad.key")).unwrap();
        drop(st);
        let e = restaurar(&archivo, CLAVE, dir.path(), true, &crate::identidad::publica(&otra)).unwrap_err();
        assert!(e.contains("no coincide"), "{e}");
        assert_eq!(std::fs::read(dir.path().join("identidad.key")).unwrap(), antes);
        assert!(dir.path().join("control.db").is_file());
        let sobras: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("antes-de-restaurar") || n.starts_with(".restaurando"))
            .collect();
        assert!(sobras.is_empty(), "{sobras:?}");
    }

    /// Una copia de antes de 0.7.11 (versión 1, sin firma) se sigue restaurando.
    #[test]
    fn restaura_copias_v1_sin_firma() {
        let (dir, st) = servidor_con_datos();
        let u = hacer(dir.path(), &st.identidad, "manual").unwrap();
        let archivo = carpeta(dir.path()).join(u.archivo.unwrap());
        let a = leer(dir.path());
        let secreto = rc::secreto(CLAVE, a.sal.as_deref().unwrap()).unwrap();
        let mut contenido = Vec::new();
        rc::descifrar(std::fs::File::open(&archivo).unwrap(), &secreto, &mut contenido).unwrap();
        let mut c = rc::Cifrador::nuevo_v1_sin_firma(
            Vec::new(),
            a.sal.as_deref().unwrap(),
            a.publica.as_deref().unwrap(),
            "2026-10-01T03:30:00+02:00",
            &st.identidad_pub,
            "0.7.10",
        )
        .unwrap();
        c.write_all(&contenido).unwrap();
        let v1 = dir.path().join("antigua.resguardo-consola");
        std::fs::write(&v1, c.terminar().unwrap()).unwrap();
        let destino = tempfile::tempdir().unwrap();
        let r = restaurar(&v1, CLAVE, destino.path(), false, &st.identidad_pub).unwrap();
        assert!(!r.firmada);
        assert_eq!((r.identidad.as_str(), r.version.as_str()), (st.identidad_pub.as_str(), "0.7.10"));
    }

    /// Las instantáneas en claro de las bases de datos no quedan nunca en `respaldos`
    /// (la carpeta que copia el agente): allí solo hay archivos cifrados.
    #[test]
    fn respaldos_solo_con_archivos_cifrados() {
        let (dir, st) = servidor_con_datos();
        // Lo que dejó una versión anterior cortada a medias.
        let vieja = carpeta(dir.path()).join(".instantanea-0123");
        std::fs::create_dir_all(&vieja).unwrap();
        std::fs::write(vieja.join("control.db"), b"SQLite format 3").unwrap();
        hacer(dir.path(), &st.identidad, "manual").unwrap();
        for e in std::fs::read_dir(carpeta(dir.path())).unwrap().flatten() {
            assert!(e.file_type().unwrap().is_file(), "{:?}", e.file_name());
            let crudo = std::fs::read(e.path()).unwrap();
            assert!(crudo.starts_with(rc::MAGIA), "{:?}", e.file_name());
        }
        let tmp: Vec<_> =
            std::fs::read_dir(dir.path()).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with(PREFIJO_INSTANTANEA)).collect();
        assert!(tmp.is_empty(), "la instantánea se borra al terminar");
    }

    #[test]
    fn conserva_las_mas_recientes_y_dice_si_falta_la_clave() {
        let (dir, st) = servidor_con_datos();
        for _ in 0..3 {
            hacer(dir.path(), &st.identidad, "manual").unwrap();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(copias(dir.path()).len(), 2, "conservar: 2");
        let mut a = leer(dir.path());
        a.publica = None;
        guardar(dir.path(), &a).unwrap();
        assert!(hacer(dir.path(), &st.identidad, "manual").unwrap_err().contains("clave de respaldo"));
        assert!(!leer(dir.path()).ultima.unwrap().ok);
    }

    #[test]
    fn el_servidor_sigue_funcionando_durante_la_copia() {
        let (dir, st) = servidor_con_datos();
        let c = st.db.todos_los_clientes().unwrap()[0].clone();
        let parar = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (st2, c2, p2) = (st.clone(), c.clone(), parar.clone());
        let escritor = std::thread::spawn(move || {
            let mut n = 0;
            while !p2.load(std::sync::atomic::Ordering::SeqCst) || n < 20 {
                st2.db.crear_aviso(&c2, None, "cambio_inusual", &format!("aviso {n}")).unwrap();
                n += 1;
            }
            n
        });
        for _ in 0..3 {
            hacer(dir.path(), &st.identidad, "manual").unwrap();
        }
        parar.store(true, std::sync::atomic::Ordering::SeqCst);
        // El servidor escribió todo el rato sin errores (ni «database is locked»).
        assert!(escritor.join().unwrap() >= 20);
        assert_eq!(copias(dir.path()).len(), 2);
    }

    #[test]
    fn la_diaria_una_vez_al_dia_pasada_la_hora() {
        let t = |s: &str| chrono::DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&chrono::Local);
        let ahora = chrono::Local::now();
        let hoy = |h: &str| t(&format!("{}T{h}:00{}", ahora.format("%Y-%m-%d"), ahora.format("%:z")));
        let mut a = Ajustes { activo: true, publica: Some("x".into()), ..Default::default() };
        assert!(!toca(&a, hoy("03:00")), "antes de las 03:30");
        assert!(toca(&a, hoy("03:31")));
        (a.diaria, a.diaria_ok) = (Some(hoy("03:31").to_rfc3339()), true);
        assert!(!toca(&a, hoy("10:00")), "ya hecha hoy");
        a.ultima = Some(Ultima { cuando: hoy("09:00").to_rfc3339(), ok: true, mensaje: String::new(), archivo: None, bytes: None, motivo: "manual".into() });
        assert!(!toca(&a, hoy("10:00")), "una a mano no cambia la diaria");
        a.diaria_ok = false;
        assert!(!toca(&a, hoy("04:00")) && toca(&a, hoy("05:00")), "si falló, otra vez pasada una hora");
        a.activo = false;
        assert!(!toca(&a, hoy("05:00")));
        assert!(ruta_valida("clientes/abc.db") && ruta_valida("tls/ca.key") && !ruta_valida("../x") && !ruta_valida("clientes/../x") && !ruta_valida("otra/x"));
    }
}
