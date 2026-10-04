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

#[cfg(test)]
mod tests {
    use super::*;

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
