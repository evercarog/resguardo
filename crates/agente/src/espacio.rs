//! Espacio libre y total del volumen de una carpeta (v1.31, «¿Cuándo se
//! llena?» de la consola): el del almacén (`guarda_copias.espacio`) y el de
//! cada carpeta del espejo (`guarda_copias.espejo.destinos[].espacio`).
//!
//! Solo dos números y cuándo se leyeron: nada de rutas. Es barato (una
//! llamada al sistema: `GetDiskFreeSpaceExW` en Windows, `statvfs` en Linux),
//! así que se lee al hacer el resumen. Si la carpeta aún no existe se mide la
//! primera de sus carpetas superiores que exista (el mismo volumen).

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Bytes libres (para quien escribe) y totales de un volumen o de una nube.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Espacio {
    pub libre: u64,
    pub total: u64,
}

impl Espacio {
    /// `{ libre, total, leido }` para el resumen (`leido`: ahora o cuando se midió).
    pub fn json(&self, leido: &str) -> serde_json::Value {
        serde_json::json!({ "libre": self.libre, "total": self.total, "leido": leido })
    }
}

/// El espacio del volumen donde está (o estará) `ruta`. `None` si no se puede leer.
pub fn del_volumen(ruta: &Path) -> Option<Espacio> {
    let mut p = ruta;
    // La primera carpeta que exista (una carpeta del espejo aún sin crear).
    while !p.exists() {
        p = p.parent()?;
    }
    medir(p).filter(|e| e.total > 0 && e.libre <= e.total)
}

/// `{ libre, total, leido }` del volumen de `ruta` (o `null`).
pub fn json_de(ruta: &str) -> serde_json::Value {
    if ruta.trim().is_empty() {
        return serde_json::Value::Null;
    }
    del_volumen(Path::new(ruta)).map_or(serde_json::Value::Null, |e| e.json(&chrono::Local::now().to_rfc3339()))
}

#[cfg(windows)]
fn medir(p: &Path) -> Option<Espacio> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let ancho: Vec<u16> = p.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let (mut libre, mut total, mut _todo_libre) = (0u64, 0u64, 0u64);
    // SAFETY: cadena terminada en 0 y punteros a u64 válidos durante la llamada.
    let ok = unsafe { GetDiskFreeSpaceExW(ancho.as_ptr(), &mut libre, &mut total, &mut _todo_libre) };
    (ok != 0).then_some(Espacio { libre, total })
}

// Los campos de `statvfs` son u64 en Linux de 64 bits y u32 en otros
// sistemas: `as u64` hace falta en unos y en otros es un «cast innecesario»
// (y `u64::from`, una «conversión inútil»).
#[cfg(unix)]
#[allow(clippy::unnecessary_cast)]
fn medir(p: &Path) -> Option<Espacio> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(p.as_os_str().as_bytes()).ok()?;
    // SAFETY: `statvfs` es una estructura C de enteros: todo ceros es válido.
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: cadena C válida y estructura propia.
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    let bloque = if s.f_frsize > 0 { s.f_frsize as u64 } else { s.f_bsize as u64 };
    Some(Espacio { libre: (s.f_bavail as u64).saturating_mul(bloque), total: (s.f_blocks as u64).saturating_mul(bloque) })
}

#[cfg(not(any(windows, unix)))]
fn medir(_: &Path) -> Option<Espacio> {
    None
}

// ---------- Qué disco es (v1.41) ----------

/// Qué es el volumen de un destino local (v1.41, «¿Dónde se guardan?» de la
/// consola): la unidad («D:», solo en Windows), si es un disco extraíble
/// (USB, tarjeta) y si en realidad es una carpeta de la red. Nada de rutas:
/// solo la letra de la unidad, para decir «en este mismo equipo (D:)».
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Disco {
    pub unidad: Option<String>,
    /// `None`: no se sabe (la consola avisa igual).
    pub extraible: Option<bool>,
    /// Una carpeta compartida de otra máquina (`\\servidor\copias`, NFS, SMB…).
    pub red: bool,
}

impl Disco {
    /// Los campos para `resumen.destinos[]`.
    pub fn json(&self) -> serde_json::Value {
        serde_json::json!({ "unidad": self.unidad, "extraible": self.extraible, "red": self.red })
    }
}

/// ¿Es una ruta de red de Windows (`\\servidor\recurso`, `\\?\UNC\…`)?
pub fn ruta_unc(ruta: &str) -> bool {
    let r = ruta.trim().replace('/', "\\");
    if let Some(resto) = r.strip_prefix(r"\\?\").or_else(|| r.strip_prefix(r"\\.\")) {
        return resto.len() >= 4 && resto[..4].eq_ignore_ascii_case(r"UNC\");
    }
    r.starts_with(r"\\")
}

/// La letra de unidad de una ruta de Windows («D:»), también tras `\\?\`.
pub fn letra_unidad(ruta: &str) -> Option<String> {
    let r = ruta.trim();
    let r = r.strip_prefix(r"\\?\").unwrap_or(r);
    let mut c = r.chars();
    match (c.next(), c.next()) {
        (Some(l), Some(':')) if l.is_ascii_alphabetic() => Some(format!("{}:", l.to_ascii_uppercase())),
        _ => None,
    }
}

/// Qué disco es el de `ruta` (un destino local). Si no se puede saber, lo que se sepa.
pub fn disco_de(ruta: &str) -> Disco {
    if ruta.trim().is_empty() {
        return Disco::default();
    }
    if ruta_unc(ruta) {
        return Disco { unidad: None, extraible: Some(false), red: true };
    }
    disco_sistema(ruta)
}

#[cfg(windows)]
fn disco_sistema(ruta: &str) -> Disco {
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
    // GetDriveTypeW: 2 extraíble, 3 fijo, 4 de red, 5 CD/DVD.
    const EXTRAIBLE: u32 = 2;
    const FIJO: u32 = 3;
    const RED: u32 = 4;
    const CD: u32 = 5;
    let Some(unidad) = letra_unidad(ruta) else { return Disco::default() };
    let raiz: Vec<u16> = format!("{unidad}\\").encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: cadena terminada en 0, viva durante la llamada.
    let tipo = unsafe { GetDriveTypeW(raiz.as_ptr()) };
    match tipo {
        RED => Disco { unidad: Some(unidad), extraible: Some(false), red: true },
        EXTRAIBLE | CD => Disco { unidad: Some(unidad), extraible: Some(true), red: false },
        // Un disco duro USB dice «fijo»: se mira por qué bus va.
        FIJO => {
            let extraible = bus_extraible(&unidad);
            Disco { unidad: Some(unidad), extraible, red: false }
        }
        _ => Disco { unidad: Some(unidad), extraible: None, red: false },
    }
}

/// ¿Va el volumen `D:` por USB, FireWire o una tarjeta? (`IOCTL_STORAGE_QUERY_PROPERTY`). `None`: no se pudo saber.
#[cfg(windows)]
fn bus_extraible(unidad: &str) -> Option<bool> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
    use windows_sys::Win32::System::Ioctl::{
        PropertyStandardQuery, StorageDeviceProperty, IOCTL_STORAGE_QUERY_PROPERTY, STORAGE_DEVICE_DESCRIPTOR, STORAGE_PROPERTY_QUERY,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;
    // STORAGE_BUS_TYPE: 4 FireWire, 7 USB, 12 SD, 13 MMC.
    const EXTRAIBLES: [i32; 4] = [4, 7, 12, 13];
    let dispositivo: Vec<u16> = format!(r"\\.\{unidad}").encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: sin permisos de lectura ni escritura (solo consultar propiedades); cadena terminada en 0.
    let h = unsafe { CreateFileW(dispositivo.as_ptr(), 0, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null(), OPEN_EXISTING, 0, std::ptr::null_mut()) };
    if h == INVALID_HANDLE_VALUE || h.is_null() {
        return None;
    }
    let consulta = STORAGE_PROPERTY_QUERY { PropertyId: StorageDeviceProperty, QueryType: PropertyStandardQuery, AdditionalParameters: [0] };
    // El descriptor y lo que le sigue (cadenas del fabricante): con 1 KiB basta.
    let mut buf = [0u64; 128];
    let mut leidos = 0u32;
    // SAFETY: búferes propios del tamaño indicado; el identificador es válido hasta cerrarlo.
    let ok = unsafe {
        DeviceIoControl(
            h,
            IOCTL_STORAGE_QUERY_PROPERTY,
            (&consulta as *const STORAGE_PROPERTY_QUERY).cast(),
            std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            buf.as_mut_ptr().cast(),
            std::mem::size_of_val(&buf) as u32,
            &mut leidos,
            std::ptr::null_mut(),
        )
    };
    // SAFETY: el identificador se abrió aquí y no se usa después.
    unsafe { CloseHandle(h) };
    if ok == 0 || (leidos as usize) < std::mem::size_of::<STORAGE_DEVICE_DESCRIPTOR>() {
        return None;
    }
    // SAFETY: el búfer (alineado a 8) tiene al menos un descriptor entero escrito por el sistema.
    let d = unsafe { &*(buf.as_ptr() as *const STORAGE_DEVICE_DESCRIPTOR) };
    Some(d.RemovableMedia || EXTRAIBLES.contains(&d.BusType))
}

#[cfg(target_os = "linux")]
fn disco_sistema(ruta: &str) -> Disco {
    let mut p = Path::new(ruta);
    while !p.exists() {
        match p.parent() {
            Some(x) => p = x,
            None => return Disco::default(),
        }
    }
    let Ok(real) = std::fs::canonicalize(p) else { return Disco::default() };
    let Ok(info) = std::fs::read_to_string("/proc/self/mountinfo") else { return Disco::default() };
    let Some((tipo, fuente)) = montaje_de(&info, &real.to_string_lossy()) else { return Disco::default() };
    if es_fs_de_red(&tipo) {
        return Disco { unidad: None, extraible: Some(false), red: true };
    }
    let extraible = fuente.strip_prefix("/dev/").and_then(|_| {
        let dev = std::fs::canonicalize(&fuente).ok()?;
        let nombre = dev.file_name()?.to_string_lossy().to_string();
        let sys = std::fs::canonicalize(format!("/sys/class/block/{nombre}")).ok()?;
        let sys = sys.to_string_lossy();
        Some(sys.contains("/usb") || sys.contains("/mmc"))
    });
    Disco { unidad: None, extraible, red: false }
}

#[cfg(not(any(windows, target_os = "linux")))]
fn disco_sistema(_: &str) -> Disco {
    Disco::default()
}

/// Sistemas de archivos de red (`/proc/self/mountinfo`).
#[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]
fn es_fs_de_red(tipo: &str) -> bool {
    matches!(tipo, "nfs" | "nfs4" | "cifs" | "smb3" | "smbfs" | "fuse.sshfs" | "9p" | "afs" | "ceph" | "glusterfs" | "fuse.rclone")
}

/// El sistema de archivos y la fuente del montaje que contiene `ruta` (el de
/// punto de montaje más largo), leídos de `/proc/self/mountinfo`.
#[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]
fn montaje_de(mountinfo: &str, ruta: &str) -> Option<(String, String)> {
    // «\040» es un espacio en los campos de mountinfo.
    let sin_escapes = |x: &str| x.replace(r"\040", " ").replace(r"\011", "\t").replace(r"\134", "\\");
    let dentro = |punto: &str| punto == "/" || ruta == punto || ruta.starts_with(&format!("{}/", punto.trim_end_matches('/')));
    mountinfo
        .lines()
        .filter_map(|l| {
            let (antes, despues) = l.split_once(" - ")?;
            let punto = sin_escapes(antes.split(' ').nth(4)?);
            let mut d = despues.split(' ');
            let (tipo, fuente) = (d.next()?.to_string(), sin_escapes(d.next()?));
            dentro(&punto).then_some((punto.len(), tipo, fuente))
        })
        .max_by_key(|x| x.0)
        .map(|(_, t, f)| (t, f))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letra_de_unidad_y_rutas_de_red() {
        assert_eq!(letra_unidad(r"d:\Resguardo").as_deref(), Some("D:"));
        assert_eq!(letra_unidad(r"\\?\E:\Copias").as_deref(), Some("E:"));
        assert_eq!(letra_unidad("/mnt/disco"), None);
        assert!(ruta_unc(r"\\servidor\copias"));
        assert!(ruta_unc("//servidor/copias"));
        assert!(ruta_unc(r"\\?\UNC\servidor\copias"));
        assert!(!ruta_unc(r"\\?\D:\Copias"));
        assert!(!ruta_unc(r"D:\Copias"));
        let d = disco_de(r"\\nas\copias");
        assert!(d.red && d.extraible == Some(false) && d.unidad.is_none());
        assert_eq!(disco_de(""), Disco::default());
        assert_eq!(d.json()["red"], true);
    }

    #[test]
    fn el_montaje_mas_largo_manda() {
        let info = "22 1 8:2 / / rw,relatime - ext4 /dev/sda2 rw\n\
                    40 22 8:17 / /media/copias\\040usb rw - vfat /dev/sdb1 rw\n\
                    41 22 0:50 / /mnt/nas rw - nfs4 nas:/copias rw\n";
        assert_eq!(montaje_de(info, "/srv/resguardo"), Some(("ext4".into(), "/dev/sda2".into())));
        assert_eq!(montaje_de(info, "/media/copias usb/repo"), Some(("vfat".into(), "/dev/sdb1".into())));
        assert_eq!(montaje_de(info, "/mnt/nas"), Some(("nfs4".into(), "nas:/copias".into())));
        assert_eq!(montaje_de(info, "/mnt/nasa"), Some(("ext4".into(), "/dev/sda2".into())));
        assert!(es_fs_de_red("nfs4") && es_fs_de_red("cifs") && !es_fs_de_red("ext4"));
    }

    /// En Windows, la unidad del sistema: un disco fijo, ni de red ni extraíble (en una máquina normal).
    #[cfg(windows)]
    #[test]
    fn la_unidad_del_sistema_es_fija() {
        let sistema = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        let d = disco_de(&format!(r"{sistema}\Windows"));
        assert_eq!(d.unidad.as_deref(), Some(sistema.to_uppercase().as_str()));
        assert!(!d.red);
    }

    #[test]
    fn mide_el_volumen_de_una_carpeta_que_aun_no_existe() {
        let tmp = std::env::temp_dir();
        let e = del_volumen(&tmp.join("resguardo-no-existe").join("tampoco")).expect("el volumen del temporal");
        assert!(e.total > 0 && e.libre <= e.total);
        let j = e.json("2026-10-04T10:00:00+02:00");
        assert_eq!(j["total"], e.total);
        assert_eq!(json_de(""), serde_json::Value::Null);
    }
}
