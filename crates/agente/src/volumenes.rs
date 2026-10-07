//! Cuándo se ve conectado el disco de un destino local (0.7.26, bloque 2:
//! la marca «Aislado» de docs/regla-3-2-1.md).
//!
//! Un medio aislado (discos USB que se rotan) casi nunca está conectado, así
//! que la consola no puede adivinar si la rotación se cumple: lo **ve el
//! agente**. Cada vez que hace el resumen (y al terminar una copia) mira qué
//! volumen tiene la carpeta de cada destino local y apunta cuándo lo vio:
//!
//! - **Windows**: el GUID del volumen (`\\?\Volume{…}\`); si no, su número de serie.
//! - **Linux**: el UUID del sistema de archivos (`/dev/disk/by-uuid`).
//!
//! Al resumen solo va un **resumen** de ese identificador (SHA-256, 16 cifras
//! hexadecimales) y la hora: nunca la ruta, la letra ni el número de serie.
//! Con varios discos que se rotan para el mismo destino, se guarda una lista
//! corta (los 8 más recientes) para que se vea la rotación entre A y B.
//!
//! Una carpeta que no existe (el disco no está puesto) no cuenta como vista:
//! en Linux, el punto de montaje vacío está en el disco del sistema.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Mutex;

const ARCHIVO: &str = "volumenes.json";
/// Discos que se recuerdan por destino.
const MAX_POR_DESTINO: usize = 8;
/// Destinos que se recuerdan.
const MAX_DESTINOS: usize = 64;
/// No se vuelve a escribir si se vio hace menos de esto (el resumen se hace a menudo).
const REFRESCO_SEGUNDOS: i64 = 3600;

static CANDADO: Mutex<()> = Mutex::new(());

/// Un disco visto: el resumen de su identificador y cuándo.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Visto {
    pub id: String,
    pub visto: String,
}

/// Lo apuntado: por destino (`destino:<id>`, `espejo:<resumen>`), los discos vistos.
#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
pub struct Registro {
    #[serde(default)]
    pub destinos: BTreeMap<String, Vec<Visto>>,
}

fn parse(t: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    chrono::DateTime::parse_from_rfc3339(t).ok()
}

/// El resumen de un identificador de volumen (nunca el identificador).
pub fn resumen_id(crudo: &str) -> String {
    use sha2::{Digest, Sha256};
    let h = Sha256::digest(format!("resguardo-volumen:{}", crudo.trim().to_lowercase()).as_bytes());
    h.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

impl Registro {
    /// Apunta que `id` (ya resumido) se vio `ahora` en el destino `clave`. Devuelve si cambió algo.
    pub fn apuntar(&mut self, clave: &str, id: &str, ahora: &str) -> bool {
        let Some(t) = parse(ahora) else { return false };
        if !self.destinos.contains_key(clave) && self.destinos.len() >= MAX_DESTINOS {
            // El destino que lleva más sin verse deja sitio.
            let viejo = self.destinos.iter().min_by_key(|(_, l)| l.iter().filter_map(|v| parse(&v.visto)).max()).map(|(k, _)| k.clone());
            if let Some(k) = viejo {
                self.destinos.remove(&k);
            }
        }
        let l = self.destinos.entry(clave.to_string()).or_default();
        if let Some(v) = l.iter_mut().find(|v| v.id == id) {
            if parse(&v.visto).is_some_and(|p| (t - p).num_seconds() < REFRESCO_SEGUNDOS && p <= t) {
                return false;
            }
            v.visto = ahora.to_string();
        } else {
            l.push(Visto { id: id.to_string(), visto: ahora.to_string() });
        }
        l.sort_by(|a, b| parse(&b.visto).cmp(&parse(&a.visto)));
        l.truncate(MAX_POR_DESTINO);
        true
    }

    /// Al terminar una copia: el disco `id` se vio `ahora` en los destinos que ya lo conocían.
    pub fn tocar(&mut self, id: &str, ahora: &str) -> bool {
        let claves: Vec<String> = self.destinos.iter().filter(|(_, l)| l.iter().any(|v| v.id == id)).map(|(k, _)| k.clone()).collect();
        claves.iter().fold(false, |c, k| self.apuntar(k, id, ahora) || c)
    }

    /// `{ ultima_conexion, volumenes: [{ id, visto }] }` de un destino (el más reciente primero), o `null`.
    pub fn json(&self, clave: &str) -> Value {
        let Some(l) = self.destinos.get(clave).filter(|l| !l.is_empty()) else { return Value::Null };
        json!({ "ultima_conexion": l.first().map(|v| v.visto.clone()), "volumenes": l })
    }
}

fn cargar() -> Registro {
    crate::agent::read_json(ARCHIVO)
}

/// Para el resumen: mira el disco de la carpeta del destino `clave` ahora y
/// devuelve lo apuntado (`null` si nunca se vio). Nunca falla.
pub fn resumen(clave: &str, ruta: &str) -> Value {
    let Ok(_g) = CANDADO.lock() else { return Value::Null };
    let mut r = cargar();
    if let Some(id) = id_de(ruta) {
        if r.apuntar(clave, &id, &chrono::Local::now().to_rfc3339()) {
            let _ = crate::agent::write_json(ARCHIVO, &r);
        }
    }
    r.json(clave)
}

/// Al terminar una copia en `ruta` (un repositorio local): su disco se vio ahora.
pub fn tocar(ruta: &str) {
    if !es_ruta_local(ruta) {
        return;
    }
    let Some(id) = id_de(ruta) else { return };
    let Ok(_g) = CANDADO.lock() else { return };
    let mut r = cargar();
    if r.tocar(&id, &chrono::Local::now().to_rfc3339()) {
        let _ = crate::agent::write_json(ARCHIVO, &r);
    }
}

/// ¿Es la ruta de una carpeta del equipo (no `rest:`, `s3:`, `rclone:`…)?
pub fn es_ruta_local(ruta: &str) -> bool {
    let r = ruta.trim();
    crate::espacio::letra_unidad(r).is_some() || r.starts_with('/')
}

/// El resumen del identificador del volumen de `ruta`, si la carpeta existe y no es de la red.
pub fn id_de(ruta: &str) -> Option<String> {
    let r = ruta.trim();
    if r.is_empty() || crate::espacio::ruta_unc(r) || !std::path::Path::new(r).exists() {
        return None;
    }
    id_sistema(r).filter(|x| !x.trim().is_empty()).map(|x| resumen_id(&x))
}

#[cfg(windows)]
fn id_sistema(ruta: &str) -> Option<String> {
    use windows_sys::Win32::Storage::FileSystem::{GetVolumeInformationW, GetVolumeNameForVolumeMountPointW, GetVolumePathNameW};
    let ancho: Vec<u16> = ruta.encode_utf16().chain(std::iter::once(0)).collect();
    let mut raiz = [0u16; 512];
    // SAFETY: cadena terminada en 0 y búfer propio del tamaño indicado.
    if unsafe { GetVolumePathNameW(ancho.as_ptr(), raiz.as_mut_ptr(), raiz.len() as u32) } == 0 {
        return None;
    }
    let mut guid = [0u16; 64];
    // SAFETY: `raiz` termina en 0 y en barra (lo pone el sistema); búfer propio de 64 (pide 50).
    if unsafe { GetVolumeNameForVolumeMountPointW(raiz.as_ptr(), guid.as_mut_ptr(), guid.len() as u32) } != 0 {
        let fin = guid.iter().position(|&c| c == 0).unwrap_or(guid.len());
        let g = String::from_utf16_lossy(&guid[..fin]);
        if !g.is_empty() {
            return Some(format!("guid:{g}"));
        }
    }
    let mut serie = 0u32;
    // SAFETY: solo se pide el número de serie, en una variable propia.
    let ok = unsafe {
        GetVolumeInformationW(raiz.as_ptr(), std::ptr::null_mut(), 0, &mut serie, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), 0)
    };
    (ok != 0 && serie != 0).then(|| format!("serie:{serie:08x}"))
}

#[cfg(target_os = "linux")]
fn id_sistema(ruta: &str) -> Option<String> {
    let real = std::fs::canonicalize(ruta).ok()?;
    let info = std::fs::read_to_string("/proc/self/mountinfo").ok()?;
    let (tipo, fuente) = crate::espacio::montaje_de(&info, &real.to_string_lossy())?;
    if crate::espacio::es_fs_de_red(&tipo) || !fuente.starts_with("/dev/") {
        return None;
    }
    let dev = std::fs::canonicalize(&fuente).ok()?;
    std::fs::read_dir("/dev/disk/by-uuid").ok()?.flatten().find_map(|e| {
        let destino = std::fs::canonicalize(e.path()).ok()?;
        (destino == dev).then(|| format!("uuid:{}", e.file_name().to_string_lossy()))
    })
}

#[cfg(not(any(windows, target_os = "linux")))]
fn id_sistema(_: &str) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: &str = "2026-10-01T10:00:00-05:00";

    #[test]
    fn resumen_sin_el_identificador() {
        let a = resumen_id("guid:\\\\?\\Volume{1234-ABCD}\\");
        assert_eq!(a.len(), 16);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(!a.contains("1234"));
        assert_eq!(a, resumen_id("GUID:\\\\?\\volume{1234-abcd}\\"), "sin distinguir mayúsculas");
        assert_ne!(a, resumen_id("uuid:5f2c"));
    }

    #[test]
    fn rotacion_entre_dos_discos() {
        let mut r = Registro::default();
        assert_eq!(r.json("destino:d1"), Value::Null, "sin datos");
        assert!(r.apuntar("destino:d1", "aaaa", T0));
        // El mismo disco al rato: no se reescribe.
        assert!(!r.apuntar("destino:d1", "aaaa", "2026-10-01T10:30:00-05:00"));
        // Una semana después se pone el otro disco.
        assert!(r.apuntar("destino:d1", "bbbb", "2026-10-08T10:00:00-05:00"));
        let j = r.json("destino:d1");
        assert_eq!(j["ultima_conexion"], "2026-10-08T10:00:00-05:00");
        assert_eq!(j["volumenes"][0]["id"], "bbbb");
        assert_eq!(j["volumenes"][1], json!({ "id": "aaaa", "visto": T0 }));
        // Al terminar una copia en el disco A, se apunta en los destinos que lo conocen.
        assert!(r.tocar("aaaa", "2026-10-15T10:00:00-05:00"));
        assert!(!r.tocar("cccc", "2026-10-15T10:00:00-05:00"), "un disco que no se conoce no se apunta");
        assert_eq!(r.json("destino:d1")["volumenes"][0]["id"], "aaaa");
        // Como mucho 8 discos por destino.
        for i in 0..12 {
            r.apuntar("destino:d1", &format!("d{i}"), &format!("2026-11-{:02}T10:00:00-05:00", i + 1));
        }
        assert_eq!(r.destinos["destino:d1"].len(), MAX_POR_DESTINO);
        assert_eq!(r.destinos["destino:d1"][0].id, "d11");
        assert!(!r.apuntar("destino:d1", "x", "no es una fecha"));
    }

    #[test]
    fn rutas() {
        assert!(es_ruta_local(r"E:\Copias\documentos") && es_ruta_local("/media/usb/copias"));
        assert!(!es_ruta_local("rest:https://almacen:8000/x/") && !es_ruta_local("s3:cubo/x") && !es_ruta_local("rclone:dropbox:x"));
        assert_eq!(id_de(r"\\nas\copias"), None, "la red no se mira");
        assert_eq!(id_de(""), None);
        let tmp = std::env::temp_dir();
        assert_eq!(id_de(&tmp.join("resguardo-no-existe-vol").to_string_lossy()), None, "una carpeta que no existe no cuenta como vista");
        // La carpeta temporal existe: en Windows y Linux suele tener identificador; si lo tiene, es un resumen.
        if let Some(id) = id_de(&tmp.to_string_lossy()) {
            assert_eq!(id.len(), 16);
        }
    }
}
