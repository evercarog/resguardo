//! 14uu (plan 0.8.0, D3): los datos básicos del equipo para sus consolas
//! (`resumen.datos_basicos`): el sistema exacto, el procesador, la memoria, los
//! discos, el fabricante y el modelo, y el último arranque.
//!
//! Se leen al arrancar y una vez al día (los discos, cada 6 horas: su espacio
//! libre sirve para el aviso «disco casi lleno»), y se guardan en memoria: el
//! resumen se hace a menudo y esto no debe costar nada. Son lecturas baratas
//! (el registro en Windows; `/proc`, `/sys` y `/etc/os-release` en Linux).
//!
//! **Nada** de números de serie, usuarios, programas instalados ni nombres de
//! red. De los discos, solo la letra (Windows) o el punto de montaje (Linux),
//! su tamaño, lo libre, el sistema de archivos y si guarda algo de las copias.
//!
//! Plan 0.8.1 (C4, 15d, docs/infraestructura.md): lo que el equipo ve de su
//! almacenamiento, para que la consola **proponga** la infraestructura y la
//! persona la confirme (`almacenamiento`): los discos que ve (en Linux, los de
//! `/sys/block`; en Windows, `\\.\PhysicalDriveN` con su bus y su modelo), los
//! conjuntos (`mdadm` de `/proc/mdstat`, los grupos de ZFS por el origen de sus
//! montajes y su estado de `/proc/spl/kstat/zfs`, Storage Spaces y controladoras
//! RAID por el bus del disco que ven) y en qué conjunto está cada disco
//! (`discos[].volumen`). Dentro de una máquina virtual o un contenedor no se ven
//! los discos del anfitrión, pero sí, p. ej., el grupo de ZFS (`tank`) de un
//! montaje. Solo lecturas, sin ejecutar programas ni números de serie.

use serde_json::{json, Value};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Cada cuánto se vuelve a leer lo que casi no cambia (sistema, procesador…).
const CADA_FIJOS: Duration = Duration::from_secs(24 * 3600);
/// Cada cuánto se vuelve a medir el espacio de los discos.
const CADA_DISCOS: Duration = Duration::from_secs(6 * 3600);
/// Como mucho tantos discos (Windows tiene 26 letras; en Linux, los primeros).
pub const MAX_DISCOS: usize = 32;
/// Largo máximo de cada texto.
pub const MAX_TEXTO: usize = 120;

/// Un texto presentable: sin caracteres de control, espacios juntados y recortado.
pub fn limpio(s: &str, max: usize) -> Option<String> {
    let t: String = s.split_whitespace().collect::<Vec<_>>().join(" ").chars().filter(|c| !c.is_control()).take(max).collect();
    let t = t.trim().to_string();
    // Los fabricantes de placas dejan textos de relleno.
    let vacio = matches!(
        t.to_ascii_lowercase().as_str(),
        "" | "to be filled by o.e.m." | "default string" | "system product name" | "system manufacturer" | "not specified" | "none" | "o.e.m."
    );
    (!vacio).then_some(t)
}

// ---------- Sistema ----------

/// El nombre de Windows corregido: en Windows 11, `ProductName` sigue diciendo
/// «Windows 10» (Microsoft no lo cambió); se sabe por la compilación (≥ 22000).
pub fn nombre_windows(producto: &str, compilacion: Option<u32>) -> String {
    let p = producto.trim();
    if compilacion.is_some_and(|b| b >= 22000) && !p.contains("Server") {
        if let Some(resto) = p.strip_prefix("Windows 10") {
            return format!("Windows 11{resto}");
        }
    }
    p.to_string()
}

/// `PRETTY_NAME`, `ID` y `VERSION_ID` de `/etc/os-release` (sin comillas).
pub fn os_release(texto: &str) -> (Option<String>, Option<String>, Option<String>) {
    let campo = |k: &str| {
        texto.lines().find_map(|l| {
            let v = l.trim().strip_prefix(k)?.strip_prefix('=')?;
            limpio(v.trim().trim_matches('"').trim_matches('\''), MAX_TEXTO)
        })
    };
    (campo("PRETTY_NAME"), campo("ID").map(|s| s.to_ascii_lowercase()), campo("VERSION_ID"))
}

// ---------- Linux: /proc ----------

/// El nombre del procesador en `/proc/cpuinfo` (x86: `model name`; algunos ARM: `Model` o `Hardware`).
pub fn cpu_linux(cpuinfo: &str) -> Option<String> {
    ["model name", "Model", "Hardware", "cpu model"].iter().find_map(|k| {
        cpuinfo.lines().find_map(|l| {
            let (a, b) = l.split_once(':')?;
            (a.trim() == *k).then(|| limpio(b, MAX_TEXTO)).flatten()
        })
    })
}

/// `MemTotal` de `/proc/meminfo`, en bytes.
pub fn memoria_linux(meminfo: &str) -> Option<u64> {
    let l = meminfo.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kb: u64 = l.split_whitespace().nth(1)?.parse().ok()?;
    kb.checked_mul(1024)
}

/// `btime` de `/proc/stat`: el arranque, en segundos Unix.
pub fn arranque_linux(stat: &str) -> Option<i64> {
    stat.lines().find_map(|l| l.strip_prefix("btime ")).and_then(|v| v.trim().parse().ok()).filter(|t: &i64| *t > 0)
}

/// Sistemas de archivos de disco de verdad (no `tmpfs`, `overlay`, `squashfs`, de red…).
pub fn fs_real(tipo: &str) -> bool {
    matches!(
        tipo,
        "ext2" | "ext3" | "ext4" | "xfs" | "btrfs" | "zfs" | "f2fs" | "jfs" | "reiserfs" | "bcachefs" | "vfat" | "exfat" | "ntfs" | "ntfs3" | "fuseblk"
    )
}

/// El conjunto de discos que hay detrás de un montaje, por su origen: el grupo de ZFS
/// (`tank/datos` → `tank`), un RAID de `mdadm` (`/dev/md0` → `md0`) o un volumen de
/// LVM o cifrado (`/dev/mapper/vg-datos` → `vg-datos`). Solo nombres (sin rutas).
pub fn volumen_de(tipo: &str, fuente: &str) -> Option<String> {
    let v = if tipo == "zfs" {
        fuente.split('/').next().unwrap_or(fuente).to_string()
    } else if let Some(md) = fuente.strip_prefix("/dev/").filter(|x| x.starts_with("md")) {
        md.split('/').next_back().unwrap_or(md).to_string()
    } else {
        fuente.strip_prefix("/dev/mapper/")?.to_string()
    };
    limpio(&v, 64).filter(|x| x.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':')))
}

/// Los discos de `/proc/self/mountinfo`: (punto de montaje, sistema de archivos, conjunto),
/// uno por dispositivo (los subvolúmenes de btrfs o los conjuntos de datos de un mismo grupo
/// ZFS cuentan una vez, con su punto más corto). Sin lo que montan los contenedores ni snap.
pub fn discos_linux(mountinfo: &str) -> Vec<(String, String, Option<String>)> {
    let sin_escapes = |x: &str| x.replace(r"\040", " ").replace(r"\011", "\t").replace(r"\134", "\\");
    let mut vistos: Vec<(String, String, String, Option<String>)> = Vec::new();
    for l in mountinfo.lines() {
        let Some((antes, despues)) = l.split_once(" - ") else { continue };
        let campos: Vec<&str> = antes.split(' ').collect();
        let (Some(dispositivo), Some(punto)) = (campos.get(2), campos.get(4)) else { continue };
        let punto = sin_escapes(punto);
        let mut d = despues.split(' ');
        let (Some(tipo), Some(fuente)) = (d.next(), d.next()) else { continue };
        if !fs_real(tipo) || !punto.starts_with('/') {
            continue;
        }
        let ruido = ["/snap/", "/var/lib/docker/", "/var/lib/containers/", "/run/", "/proc/", "/sys/", "/dev/"];
        if ruido.iter().any(|r| punto.starts_with(r)) || punto == "/boot/efi" {
            continue;
        }
        // Un grupo de ZFS: todos sus conjuntos de datos comparten el espacio libre.
        let clave = if tipo == "zfs" { format!("zfs:{}", fuente.split('/').next().unwrap_or(fuente)) } else { dispositivo.to_string() };
        match vistos.iter_mut().find(|v| v.0 == clave) {
            Some(v) if punto.len() < v.1.len() => v.1 = punto,
            Some(_) => {}
            None => vistos.push((clave, punto, tipo.to_string(), volumen_de(tipo, fuente))),
        }
    }
    vistos.sort_by(|a, b| a.1.cmp(&b.1));
    vistos.into_iter().take(MAX_DISCOS).map(|(_, p, t, v)| (p, t, v)).collect()
}

/// Como mucho tantos discos físicos y conjuntos.
pub const MAX_FISICOS: usize = 32;

/// Un conjunto de discos que ve el equipo (un RAID de `mdadm`, un grupo de ZFS, Storage
/// Spaces o una controladora RAID vista como un disco).
#[derive(Clone, Debug, PartialEq)]
pub struct Conjunto {
    pub id: String,
    /// `mdadm`, `zfs`, `storage_spaces` o `raid` (controladora).
    pub tipo: &'static str,
    /// `raid0`, `raid1`, `raid5`, `raid6`, `raid10`… si se sabe (en ZFS no se ve sin `zpool`).
    pub forma: Option<String>,
    /// Cuántos discos tiene, si se sabe.
    pub discos: Option<u32>,
    /// `bien`, `degradado` o `reconstruyendo`, si se sabe.
    pub estado: Option<&'static str>,
    pub total: Option<u64>,
}

/// Los RAID de `/proc/mdstat`: nombre, nivel, discos, estado (`[UU]` bien, `[U_]` degradado,
/// `recovery`/`resync` reconstruyendo) y tamaño.
pub fn mdstat(texto: &str) -> Vec<Conjunto> {
    let mut out: Vec<Conjunto> = Vec::new();
    for l in texto.lines() {
        let t = l.trim();
        if let Some((nombre, resto)) = t.split_once(" : ").filter(|(n, _)| n.starts_with("md") && !n.contains(' ')) {
            let mut p = resto.split_whitespace();
            let _activo = p.next();
            let nivel = p.next().filter(|x| x.starts_with("raid") || *x == "linear").map(str::to_string);
            let discos = p.filter(|x| x.contains('[')).count() as u32;
            out.push(Conjunto {
                id: nombre.to_string(),
                tipo: "mdadm",
                forma: nivel,
                discos: (discos > 0).then_some(discos),
                estado: Some("bien"),
                total: None,
            });
        } else if let Some(c) = out.last_mut() {
            if t.contains(" blocks") {
                c.total = t.split_whitespace().next().and_then(|n| n.parse::<u64>().ok()).and_then(|b| b.checked_mul(1024));
                if let Some(marca) = t.rsplit('[').next().and_then(|x| x.strip_suffix(']')) {
                    if marca.contains('_') {
                        c.estado = Some("degradado");
                    }
                }
            } else if t.contains("recovery") || t.contains("resync") || t.contains("reshape") {
                c.estado = Some("reconstruyendo");
            }
        }
    }
    out.truncate(MAX_FISICOS);
    out
}

/// El estado de un grupo de ZFS por `/proc/spl/kstat/zfs/<grupo>/state` (`ONLINE`, `DEGRADED`…).
pub fn estado_zfs(texto: &str) -> Option<&'static str> {
    match texto.trim().to_ascii_uppercase().as_str() {
        "ONLINE" => Some("bien"),
        "DEGRADED" | "FAULTED" | "UNAVAIL" | "SUSPENDED" => Some("degradado"),
        _ => None,
    }
}

/// ¿Es un disco de verdad lo que hay en `/sys/block/<nombre>`? (No los bucles, la memoria,
/// los de LVM o cifrado ni los propios RAID de `mdadm`.)
pub fn disco_de_bloque(nombre: &str) -> bool {
    ["sd", "nvme", "vd", "xvd", "hd", "mmcblk"].iter().any(|p| nombre.starts_with(p)) && !nombre.contains("boot") && !nombre.contains("rpmb")
}

/// Un disco que ve el equipo (sin número de serie).
#[derive(Clone, Debug, PartialEq)]
pub struct DiscoFisico {
    pub id: String,
    pub total: u64,
    /// `ssd` o `hdd`, si se sabe.
    pub clase: Option<&'static str>,
    /// `sata`, `sas`, `nvme`, `usb`, `raid`, `spaces`, `virtual`, `iscsi`… si se sabe.
    pub bus: Option<&'static str>,
    pub modelo: Option<String>,
    pub extraible: bool,
}

/// ¿Está `ruta` en el disco montado en `punto` (o con letra `punto`)? Con varios puntos,
/// el más largo que la contiene es el suyo: se elige fuera.
fn dentro(punto: &str, ruta: &str) -> bool {
    if punto.len() == 2 && punto.ends_with(':') {
        return crate::espacio::letra_unidad(ruta).is_some_and(|l| l.eq_ignore_ascii_case(punto));
    }
    (punto == "/" && ruta.starts_with('/')) || ruta == punto || ruta.starts_with(&format!("{}/", punto.trim_end_matches('/')))
}

/// Los puntos de los discos que guardan algo de las copias (carpetas que se copian,
/// destinos locales, la carpeta del almacén). Cada ruta, en el disco más concreto.
pub fn en_uso(puntos: &[String], rutas: &[String]) -> Vec<bool> {
    let mut uso = vec![false; puntos.len()];
    for r in rutas {
        let mejor = puntos.iter().enumerate().filter(|(_, p)| dentro(p, r)).max_by_key(|(_, p)| p.len()).map(|(i, _)| i);
        if let Some(i) = mejor {
            uso[i] = true;
        }
    }
    uso
}

// ---------- Lectura ----------

#[derive(Clone, Debug, PartialEq)]
struct Disco {
    punto: String,
    total: u64,
    libre: u64,
    fs: Option<String>,
    /// C4: el conjunto de discos en el que está (grupo de ZFS, RAID…), si se sabe.
    volumen: Option<String>,
}

/// C4: lo que el equipo ve de su almacenamiento (`almacenamiento` del resumen).
#[derive(Clone, Debug, Default, PartialEq)]
struct Almacenamiento {
    discos: Vec<DiscoFisico>,
    conjuntos: Vec<Conjunto>,
}

impl Almacenamiento {
    fn json(&self) -> Value {
        if self.discos.is_empty() && self.conjuntos.is_empty() {
            return Value::Null;
        }
        json!({
            "discos": self.discos.iter().take(MAX_FISICOS).map(|d| json!({
                "id": d.id, "total": d.total, "clase": d.clase, "bus": d.bus, "modelo": d.modelo, "extraible": d.extraible,
            })).collect::<Vec<_>>(),
            "conjuntos": self.conjuntos.iter().take(MAX_FISICOS).map(|c| json!({
                "id": c.id, "tipo": c.tipo, "forma": c.forma, "discos": c.discos, "estado": c.estado, "total": c.total,
            })).collect::<Vec<_>>(),
        })
    }
}

struct Cache {
    fijos: Value,
    leido_fijos: Instant,
    discos: Vec<Disco>,
    almacenamiento: Almacenamiento,
    leido_discos: Instant,
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);

#[cfg(windows)]
mod sistema {
    use super::*;
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ};

    fn ancho(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Un valor de texto de `HKLM\<clave>` (o `None`).
    fn texto(clave: &str, valor: &str) -> Option<String> {
        let (c, v) = (ancho(clave), ancho(valor));
        let mut buf = [0u16; 512];
        let mut tam = std::mem::size_of_val(&buf) as u32;
        // SAFETY: cadenas terminadas en 0 y búfer propio con su tamaño en bytes.
        let r = unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, c.as_ptr(), v.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut tam) };
        if r != 0 {
            return None;
        }
        let fin = buf.iter().position(|&x| x == 0).unwrap_or(buf.len());
        limpio(&String::from_utf16_lossy(&buf[..fin]), MAX_TEXTO)
    }

    /// Un valor DWORD de `HKLM\<clave>`.
    fn numero(clave: &str, valor: &str) -> Option<u32> {
        let (c, v) = (ancho(clave), ancho(valor));
        let mut n: u32 = 0;
        let mut tam = std::mem::size_of::<u32>() as u32;
        // SAFETY: cadenas terminadas en 0 y un u32 propio con su tamaño.
        let r =
            unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, c.as_ptr(), v.as_ptr(), RRF_RT_REG_DWORD, std::ptr::null_mut(), (&mut n as *mut u32).cast(), &mut tam) };
        (r == 0).then_some(n)
    }

    pub fn fijos() -> Value {
        const CV: &str = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion";
        const BIOS: &str = "HARDWARE\\DESCRIPTION\\System\\BIOS";
        const CPU: &str = "HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0";
        let build_txt = texto(CV, "CurrentBuild").or_else(|| texto(CV, "CurrentBuildNumber"));
        let build = build_txt.as_deref().and_then(|b| b.parse::<u32>().ok());
        let ubr = numero(CV, "UBR");
        let nombre = texto(CV, "ProductName").map(|p| nombre_windows(&p, build));
        let compilacion = build_txt.map(|b| match ubr {
            Some(u) => format!("{b}.{u}"),
            None => b,
        });
        let memoria = {
            use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
            // SAFETY: estructura C de enteros: todo ceros es válido; se pone su tamaño.
            let mut m: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
            m.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
            // SAFETY: puntero a una estructura propia con `dwLength` puesto.
            (unsafe { GlobalMemoryStatusEx(&mut m) } != 0).then_some(m.ullTotalPhys)
        };
        // SAFETY: sin argumentos.
        let ms = unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() };
        let arranque = chrono::Local::now() - chrono::Duration::milliseconds(i64::try_from(ms).unwrap_or(0));
        json!({
            "sistema": {
                "familia": "windows",
                "nombre": nombre,
                "variante": texto(CV, "EditionID"),
                "version": texto(CV, "DisplayVersion").or_else(|| texto(CV, "ReleaseId")),
                "compilacion": compilacion,
            },
            "procesador": { "nombre": texto(CPU, "ProcessorNameString"), "nucleos": nucleos() },
            "memoria": memoria,
            "fabricante": texto(BIOS, "SystemManufacturer"),
            "modelo": texto(BIOS, "SystemProductName"),
            "arranque": arranque.to_rfc3339(),
        })
    }

    pub fn discos() -> Vec<Disco> {
        use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
        const FIJO: u32 = 3;
        // SAFETY: sin argumentos.
        let mapa = unsafe { GetLogicalDrives() };
        let mut out = Vec::new();
        for i in 0..26u32 {
            if mapa & (1 << i) == 0 {
                continue;
            }
            let letra = char::from(b'A' + i as u8);
            let raiz = ancho(&format!("{letra}:\\"));
            // SAFETY: cadena terminada en 0, viva durante la llamada.
            if unsafe { GetDriveTypeW(raiz.as_ptr()) } != FIJO {
                continue;
            }
            let ruta = format!("{letra}:\\");
            let Some(e) = crate::espacio::del_volumen(std::path::Path::new(&ruta)) else { continue };
            // C4: en qué disco está (el primero de sus trozos): `disco1`, que es un conjunto si
            // ese disco es de Storage Spaces o de una controladora RAID.
            let volumen = discos_del_volumen(letra).first().map(|n| format!("disco{n}"));
            out.push(Disco { punto: format!("{letra}:"), total: e.total, libre: e.libre, fs: crate::espacio::sistema_archivos(&ruta), volumen });
        }
        out
    }

    /// Abre un dispositivo (`\\.\D:`, `\\.\PhysicalDrive0`) sin pedir lectura (solo para
    /// preguntarle cosas) o con lectura. Devuelve el manejador o `None`.
    fn abrir(nombre: &str, leer: bool) -> Option<windows_sys::Win32::Foundation::HANDLE> {
        use windows_sys::Win32::Foundation::{GENERIC_READ, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
        let n = ancho(nombre);
        // SAFETY: cadena terminada en 0; sin plantilla ni seguridad.
        let h = unsafe {
            CreateFileW(
                n.as_ptr(),
                if leer { GENERIC_READ } else { 0 },
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        (h != INVALID_HANDLE_VALUE && !h.is_null()).then_some(h)
    }

    fn cerrar(h: windows_sys::Win32::Foundation::HANDLE) {
        // SAFETY: un manejador propio, abierto con `abrir`.
        unsafe { windows_sys::Win32::Foundation::CloseHandle(h) };
    }

    /// Los números de disco en los que está un volumen (`IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS`).
    fn discos_del_volumen(letra: char) -> Vec<u32> {
        use windows_sys::Win32::Storage::FileSystem::IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS;
        use windows_sys::Win32::System::IO::DeviceIoControl;
        let Some(h) = abrir(&format!("\\\\.\\{letra}:"), false) else { return Vec::new() };
        // Sitio para 16 trozos (un volumen repartido en varios discos dinámicos).
        let mut buf = [0u64; 1 + 16 * 3];
        let mut dev = 0u32;
        // SAFETY: búfer propio con su tamaño en bytes; sin entrada.
        let ok = unsafe {
            DeviceIoControl(
                h,
                IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
                std::ptr::null(),
                0,
                buf.as_mut_ptr().cast(),
                std::mem::size_of_val(&buf) as u32,
                &mut dev,
                std::ptr::null_mut(),
            )
        };
        cerrar(h);
        if ok == 0 || (dev as usize) < 8 {
            return Vec::new();
        }
        // VOLUME_DISK_EXTENTS: u32 (y relleno hasta 8) y luego trozos de 24 bytes (u32 + relleno, i64, i64).
        let bytes: &[u8] = &bytemuck_u8(&buf)[..(dev as usize).min(std::mem::size_of_val(&buf))];
        let n = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        let mut out = Vec::new();
        for i in 0..n.min(16) {
            let o = 8 + i * 24;
            if o + 4 > bytes.len() {
                break;
            }
            let d = u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
            if !out.contains(&d) {
                out.push(d);
            }
        }
        out
    }

    fn bytemuck_u8(x: &[u64]) -> &[u8] {
        // SAFETY: u64 no tiene relleno y u8 se alinea con todo: es la misma memoria vista en bytes.
        unsafe { std::slice::from_raw_parts(x.as_ptr().cast::<u8>(), std::mem::size_of_val(x)) }
    }

    /// El bus en palabras para la consola.
    fn bus(b: i32) -> Option<&'static str> {
        use windows_sys::Win32::Storage::FileSystem::{
            BusTypeFileBackedVirtual, BusTypeNvme, BusTypeRAID, BusTypeSas, BusTypeSata, BusTypeSpaces, BusTypeUsb, BusTypeVirtual, BusTypeiScsi,
        };
        Some(match b {
            x if x == BusTypeUsb => "usb",
            x if x == BusTypeRAID => "raid",
            x if x == BusTypeiScsi => "iscsi",
            x if x == BusTypeSas => "sas",
            x if x == BusTypeSata => "sata",
            x if x == BusTypeVirtual || x == BusTypeFileBackedVirtual => "virtual",
            x if x == BusTypeSpaces => "spaces",
            x if x == BusTypeNvme => "nvme",
            _ => return None,
        })
    }

    /// Lo que se ve de `\\.\PhysicalDriveN`: su bus, su modelo (sin el número de serie),
    /// si es extraíble y su tamaño.
    fn disco_fisico(n: u32) -> Option<DiscoFisico> {
        use windows_sys::Win32::System::Ioctl::{
            PropertyStandardQuery, StorageDeviceProperty, GET_LENGTH_INFORMATION, IOCTL_DISK_GET_LENGTH_INFO, IOCTL_STORAGE_QUERY_PROPERTY,
            STORAGE_DEVICE_DESCRIPTOR, STORAGE_PROPERTY_QUERY,
        };
        use windows_sys::Win32::System::IO::DeviceIoControl;
        let nombre = format!("\\\\.\\PhysicalDrive{n}");
        let h = abrir(&nombre, false)?;
        let q = STORAGE_PROPERTY_QUERY { PropertyId: StorageDeviceProperty, QueryType: PropertyStandardQuery, AdditionalParameters: [0] };
        let mut buf = [0u64; 128];
        let mut dev = 0u32;
        // SAFETY: entrada y salida propias con sus tamaños en bytes.
        let ok = unsafe {
            DeviceIoControl(
                h,
                IOCTL_STORAGE_QUERY_PROPERTY,
                (&q as *const STORAGE_PROPERTY_QUERY).cast(),
                std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
                buf.as_mut_ptr().cast(),
                std::mem::size_of_val(&buf) as u32,
                &mut dev,
                std::ptr::null_mut(),
            )
        };
        cerrar(h);
        if ok == 0 || (dev as usize) < std::mem::size_of::<STORAGE_DEVICE_DESCRIPTOR>() {
            return None;
        }
        // SAFETY: el búfer está alineado a 8 y tiene al menos un descriptor entero (comprobado arriba).
        let d: &STORAGE_DEVICE_DESCRIPTOR = unsafe { &*(buf.as_ptr().cast::<STORAGE_DEVICE_DESCRIPTOR>()) };
        let bytes = &bytemuck_u8(&buf)[..(dev as usize).min(std::mem::size_of_val(&buf))];
        let texto = |o: u32| -> Option<String> {
            let o = o as usize;
            if o == 0 || o >= bytes.len() {
                return None;
            }
            let fin = bytes[o..].iter().position(|&b| b == 0).map_or(bytes.len(), |p| o + p);
            limpio(&String::from_utf8_lossy(&bytes[o..fin]), 60)
        };
        // El modelo es fabricante + producto; el número de serie (`SerialNumberOffset`) nunca se lee.
        let modelo = match (texto(d.VendorIdOffset), texto(d.ProductIdOffset)) {
            (Some(v), Some(p)) if !p.to_lowercase().starts_with(&v.to_lowercase()) => limpio(&format!("{v} {p}"), 60),
            (_, Some(p)) => Some(p),
            (v, None) => v,
        };
        let total = abrir(&nombre, true)
            .map(|h| {
                let mut l = GET_LENGTH_INFORMATION { Length: 0 };
                let mut dev = 0u32;
                // SAFETY: salida propia con su tamaño.
                let ok = unsafe {
                    DeviceIoControl(
                        h,
                        IOCTL_DISK_GET_LENGTH_INFO,
                        std::ptr::null(),
                        0,
                        (&mut l as *mut GET_LENGTH_INFORMATION).cast(),
                        std::mem::size_of::<GET_LENGTH_INFORMATION>() as u32,
                        &mut dev,
                        std::ptr::null_mut(),
                    )
                };
                cerrar(h);
                if ok != 0 {
                    u64::try_from(l.Length).unwrap_or(0)
                } else {
                    0
                }
            })
            .unwrap_or(0);
        Some(DiscoFisico { id: format!("disco{n}"), total, clase: None, bus: bus(d.BusType), modelo, extraible: d.RemovableMedia })
    }

    pub fn almacenamiento() -> Almacenamiento {
        let mut a = Almacenamiento::default();
        let mut fallos = 0;
        for n in 0..64u32 {
            match disco_fisico(n) {
                Some(d) => {
                    fallos = 0;
                    // Un disco de Storage Spaces o de una controladora RAID es, para la consola, un conjunto.
                    let tipo = match d.bus {
                        Some("spaces") => Some("storage_spaces"),
                        Some("raid") => Some("raid"),
                        _ => None,
                    };
                    if let Some(tipo) = tipo {
                        a.conjuntos.push(Conjunto { id: d.id.clone(), tipo, forma: None, discos: None, estado: None, total: (d.total > 0).then_some(d.total) });
                    }
                    a.discos.push(d);
                    if a.discos.len() >= MAX_FISICOS {
                        break;
                    }
                }
                // Los números pueden tener huecos: se para tras 8 seguidos que no están.
                None => {
                    fallos += 1;
                    if fallos >= 8 {
                        break;
                    }
                }
            }
        }
        a
    }
}

#[cfg(target_os = "linux")]
mod sistema {
    use super::*;

    fn leer(p: &str) -> Option<String> {
        std::fs::read_to_string(p).ok()
    }

    pub fn fijos() -> Value {
        let (bonito, id, version) = leer("/etc/os-release").or_else(|| leer("/usr/lib/os-release")).map(|t| os_release(&t)).unwrap_or_default();
        json!({
            "sistema": {
                "familia": "linux",
                "nombre": bonito,
                "id": id,
                "version": version,
                "kernel": leer("/proc/sys/kernel/osrelease").and_then(|k| limpio(&k, MAX_TEXTO)),
            },
            "procesador": { "nombre": leer("/proc/cpuinfo").and_then(|c| cpu_linux(&c)), "nucleos": nucleos() },
            "memoria": leer("/proc/meminfo").and_then(|m| memoria_linux(&m)),
            "fabricante": leer("/sys/class/dmi/id/sys_vendor").and_then(|s| limpio(&s, MAX_TEXTO)),
            "modelo": leer("/sys/class/dmi/id/product_name").and_then(|s| limpio(&s, MAX_TEXTO)),
            "arranque": leer("/proc/stat").and_then(|s| arranque_linux(&s)).and_then(|t| chrono::DateTime::from_timestamp(t, 0)).map(|d| d.with_timezone(&chrono::Local).to_rfc3339()),
        })
    }

    pub fn discos() -> Vec<Disco> {
        let Some(info) = leer("/proc/self/mountinfo") else { return Vec::new() };
        discos_linux(&info)
            .into_iter()
            .filter_map(|(punto, fs, volumen)| {
                let e = crate::espacio::del_volumen(std::path::Path::new(&punto))?;
                Some(Disco { punto, total: e.total, libre: e.libre, fs: Some(fs), volumen })
            })
            .collect()
    }

    /// C4: los discos de `/sys/block`, los RAID de `/proc/mdstat` y los grupos de ZFS de los
    /// montajes (`discos`), con su estado si el equipo lo ve (en el anfitrión).
    pub fn almacenamiento(discos: &[Disco]) -> Almacenamiento {
        let mut a = Almacenamiento::default();
        if let Ok(rd) = std::fs::read_dir("/sys/block") {
            let mut nombres: Vec<String> =
                rd.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| disco_de_bloque(n)).collect();
            nombres.sort();
            for n in nombres.into_iter().take(MAX_FISICOS) {
                let base = format!("/sys/block/{n}");
                let total = leer(&format!("{base}/size")).and_then(|s| s.trim().parse::<u64>().ok()).and_then(|s| s.checked_mul(512)).unwrap_or(0);
                if total == 0 {
                    continue;
                }
                let clase = leer(&format!("{base}/queue/rotational")).map(|r| if r.trim() == "1" { "hdd" } else { "ssd" });
                let modelo = leer(&format!("{base}/device/model")).and_then(|m| limpio(&m, 60));
                let extraible = leer(&format!("{base}/removable")).is_some_and(|r| r.trim() == "1");
                let bus = if n.starts_with("nvme") {
                    Some("nvme")
                } else if n.starts_with("vd") || n.starts_with("xvd") {
                    Some("virtual")
                } else {
                    None
                };
                a.discos.push(DiscoFisico { id: n, total, clase, bus, modelo, extraible });
            }
        }
        if let Some(t) = leer("/proc/mdstat") {
            a.conjuntos.extend(mdstat(&t));
        }
        let mut grupos: Vec<String> = Vec::new();
        for d in discos.iter().filter(|d| d.fs.as_deref() == Some("zfs")) {
            if let Some(g) = d.volumen.as_ref().filter(|g| !grupos.contains(g)) {
                grupos.push(g.clone());
                let estado = leer(&format!("/proc/spl/kstat/zfs/{g}/state")).and_then(|s| estado_zfs(&s));
                a.conjuntos.push(Conjunto { id: g.clone(), tipo: "zfs", forma: None, discos: None, estado, total: Some(d.total) });
            }
        }
        a.conjuntos.truncate(MAX_FISICOS);
        a
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
mod sistema {
    use super::*;
    pub fn fijos() -> Value {
        json!({ "procesador": { "nucleos": nucleos() } })
    }
    pub fn discos() -> Vec<Disco> {
        Vec::new()
    }
}

/// Núcleos lógicos que ve el proceso.
fn nucleos() -> Option<usize> {
    std::thread::available_parallelism().ok().map(|n| n.get())
}

/// `datos_basicos` para el resumen. `rutas`: las carpetas que se copian, los destinos
/// locales y la carpeta del almacén (solo para marcar sus discos; no salen del equipo).
pub fn resumen(rutas: &[String]) -> Value {
    let ahora = Instant::now();
    let mut g = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let c = g.get_or_insert_with(|| Cache {
        fijos: Value::Null,
        leido_fijos: ahora,
        discos: Vec::new(),
        almacenamiento: Almacenamiento::default(),
        leido_discos: ahora,
    });
    if c.fijos.is_null() || ahora.duration_since(c.leido_fijos) >= CADA_FIJOS {
        let mut f = sistema::fijos();
        f["leido"] = json!(chrono::Local::now().to_rfc3339());
        c.fijos = f;
        c.leido_fijos = ahora;
        c.discos = sistema::discos();
        c.almacenamiento = leer_almacenamiento(&c.discos);
        c.leido_discos = ahora;
    } else if ahora.duration_since(c.leido_discos) >= CADA_DISCOS {
        c.discos = sistema::discos();
        c.almacenamiento = leer_almacenamiento(&c.discos);
        c.leido_discos = ahora;
    }
    let puntos: Vec<String> = c.discos.iter().map(|d| d.punto.clone()).collect();
    let uso = en_uso(&puntos, rutas);
    let mut r = c.fijos.clone();
    r["discos"] = c
        .discos
        .iter()
        .zip(uso)
        .take(MAX_DISCOS)
        .map(|(d, u)| {
            let mut x = json!({ "punto": d.punto.chars().take(MAX_TEXTO).collect::<String>(), "total": d.total, "libre": d.libre, "sistema_archivos": d.fs, "en_uso": u });
            // C4: el conjunto de discos en el que está (solo si se sabe).
            if let Some(v) = &d.volumen {
                x["volumen"] = json!(v);
            }
            x
        })
        .collect::<Vec<_>>()
        .into();
    let a = c.almacenamiento.json();
    if !a.is_null() {
        r["almacenamiento"] = a;
    }
    r
}

/// C4: lo que se ve del almacenamiento (cada sistema a su manera).
#[allow(unused_variables)]
fn leer_almacenamiento(discos: &[Disco]) -> Almacenamiento {
    #[cfg(windows)]
    {
        sistema::almacenamiento()
    }
    #[cfg(target_os = "linux")]
    {
        sistema::almacenamiento(discos)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Almacenamiento::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_11_se_corrige_por_la_compilacion() {
        assert_eq!(nombre_windows("Windows 10 Pro", Some(22631)), "Windows 11 Pro");
        assert_eq!(nombre_windows("Windows 10 Enterprise", Some(26100)), "Windows 11 Enterprise");
        assert_eq!(nombre_windows("Windows 10 Pro", Some(19045)), "Windows 10 Pro");
        assert_eq!(nombre_windows("Windows Server 2025 Standard", Some(26100)), "Windows Server 2025 Standard");
        assert_eq!(nombre_windows("Windows 10 Pro", None), "Windows 10 Pro");
    }

    #[test]
    fn os_release_de_ubuntu_y_debian() {
        let u = "NAME=\"Ubuntu\"\nVERSION_ID=\"22.04\"\nPRETTY_NAME=\"Ubuntu 22.04.4 LTS\"\nID=ubuntu\nID_LIKE=debian\n";
        assert_eq!(os_release(u), (Some("Ubuntu 22.04.4 LTS".into()), Some("ubuntu".into()), Some("22.04".into())));
        let d = "PRETTY_NAME=\"Debian GNU/Linux 12 (bookworm)\"\nVERSION_ID=\"12\"\nID=debian\n";
        assert_eq!(os_release(d).2.as_deref(), Some("12"));
        assert_eq!(os_release(""), (None, None, None));
    }

    #[test]
    fn proc_de_linux() {
        assert_eq!(cpu_linux("processor\t: 0\nmodel name\t: Intel(R) Xeon(R)   CPU E5-2620\n").as_deref(), Some("Intel(R) Xeon(R) CPU E5-2620"));
        assert_eq!(cpu_linux("Hardware\t: BCM2835\n").as_deref(), Some("BCM2835"));
        assert_eq!(cpu_linux("flags: x"), None);
        assert_eq!(memoria_linux("MemTotal:       16314240 kB\nMemFree: 1 kB\n"), Some(16314240 * 1024));
        assert_eq!(memoria_linux("nada"), None);
        assert_eq!(arranque_linux("cpu 1 2\nbtime 1760000000\n"), Some(1_760_000_000));
        assert_eq!(arranque_linux("btime x"), None);
    }

    #[test]
    fn textos_de_relleno_fuera() {
        assert_eq!(limpio("To Be Filled By O.E.M.", 50), None);
        assert_eq!(limpio("  Dell   Inc.\n", 50).as_deref(), Some("Dell Inc."));
        assert_eq!(limpio("abcdef", 3).as_deref(), Some("abc"));
        assert_eq!(limpio("a\u{7}b", 10).as_deref(), Some("ab"));
    }

    #[test]
    fn discos_de_linux_sin_ruido() {
        let info = "\
22 1 8:1 / / rw - ext4 /dev/sda1 rw
23 22 0:21 / /dev/shm rw - tmpfs tmpfs rw
24 22 8:17 / /srv/copias rw - xfs /dev/sdb1 rw
25 22 0:30 / /var/lib/docker/overlay2/x/merged rw - overlay overlay rw
26 22 7:0 / /snap/core/1 ro - squashfs /dev/loop0 ro
27 22 0:40 /@home /home rw - btrfs /dev/sdc1 rw
28 22 0:40 /@ /mnt/raiz rw - btrfs /dev/sdc1 rw
29 22 0:50 / /tank rw - zfs tank rw
30 29 0:51 / /tank/datos rw - zfs tank/datos rw
31 22 8:2 / /boot/efi rw - vfat /dev/sda2 rw
32 22 0:60 / /mnt/red rw - nfs4 srv:/x rw
33 22 8:33 / /mnt/con\\040espacio rw - ext4 /dev/sdd1 rw
";
        let d = discos_linux(info);
        let puntos: Vec<&str> = d.iter().map(|x| x.0.as_str()).collect();
        assert_eq!(puntos, vec!["/", "/home", "/mnt/con espacio", "/srv/copias", "/tank"]);
        assert_eq!(d[3].1, "xfs");
        // C4: el grupo de ZFS se sabe por el origen del montaje.
        assert_eq!(d[4].2.as_deref(), Some("tank"));
        assert_eq!(d[3].2, None);
    }

    #[test]
    fn conjuntos_de_discos() {
        assert_eq!(volumen_de("zfs", "rpool/data/subvol-101-disk-0").as_deref(), Some("rpool"));
        assert_eq!(volumen_de("ext4", "/dev/md0").as_deref(), Some("md0"));
        assert_eq!(volumen_de("ext4", "/dev/md/datos").as_deref(), Some("datos"));
        assert_eq!(volumen_de("xfs", "/dev/mapper/vg0-copias").as_deref(), Some("vg0-copias"));
        assert_eq!(volumen_de("ext4", "/dev/sda1"), None);
        let m = "\
Personalities : [raid1] [raid6] [raid5] [raid4]
md0 : active raid1 sdb1[1] sda1[0]
      976630464 blocks super 1.2 [2/2] [UU]
      bitmap: 0/8 pages [0KB], 65536KB chunk

md1 : active raid5 sdc[0] sdd[1] sde[3](F)
      1953260544 blocks super 1.2 level 5, 512k chunk, algorithm 2 [3/2] [UU_]

md2 : active raid10 sdf[0] sdg[1] sdh[2] sdi[3]
      100 blocks super 1.2 [4/4] [UUUU]
      [==>..................]  recovery = 12.6% (1/2) finish=1.0min speed=1K/sec

unused devices: <none>
";
        let c = mdstat(m);
        assert_eq!(c.len(), 3);
        assert_eq!(
            (c[0].id.as_str(), c[0].forma.as_deref(), c[0].discos, c[0].estado, c[0].total),
            ("md0", Some("raid1"), Some(2), Some("bien"), Some(976630464 * 1024))
        );
        assert_eq!((c[1].forma.as_deref(), c[1].discos, c[1].estado), (Some("raid5"), Some(3), Some("degradado")));
        assert_eq!((c[2].forma.as_deref(), c[2].estado), (Some("raid10"), Some("reconstruyendo")));
        assert_eq!(estado_zfs("ONLINE\n"), Some("bien"));
        assert_eq!(estado_zfs("DEGRADED"), Some("degradado"));
        assert_eq!(estado_zfs("raro"), None);
        assert!(disco_de_bloque("sda") && disco_de_bloque("nvme0n1") && disco_de_bloque("vda"));
        assert!(
            !disco_de_bloque("loop0") && !disco_de_bloque("dm-0") && !disco_de_bloque("md0") && !disco_de_bloque("zram0") && !disco_de_bloque("mmcblk0boot0")
        );
        let a = Almacenamiento {
            discos: vec![DiscoFisico { id: "sda".into(), total: 10, clase: Some("ssd"), bus: None, modelo: Some("Disco X".into()), extraible: false }],
            conjuntos: c,
        };
        let j = a.json();
        assert_eq!(j["discos"][0]["id"], "sda");
        assert_eq!(j["conjuntos"][1]["estado"], "degradado");
        assert!(j.to_string().len() < 8 * 1024);
        assert_eq!(Almacenamiento::default().json(), Value::Null);
    }

    #[test]
    fn discos_en_uso() {
        let puntos = vec!["/".to_string(), "/srv/copias".to_string()];
        assert_eq!(en_uso(&puntos, &["/srv/copias/almacen".into()]), vec![false, true]);
        assert_eq!(en_uso(&puntos, &["/home/ana".into()]), vec![true, false]);
        assert_eq!(en_uso(&puntos, &["/srv/copiasotro".into()]), vec![true, false]);
        let letras = vec!["C:".to_string(), "D:".to_string()];
        assert_eq!(en_uso(&letras, &[r"d:\Datos".into(), r"\\nas\x".into()]), vec![false, true]);
        assert_eq!(en_uso(&letras, &[]), vec![false, false]);
    }

    #[test]
    fn resumen_de_este_equipo() {
        let r = resumen(&[]);
        assert!(r["leido"].is_string());
        assert!(r["discos"].is_array());
        assert!(r["discos"].as_array().unwrap().len() <= MAX_DISCOS);
        // Pequeño: cabe de sobra en el resumen (64 KiB).
        assert!(r.to_string().len() < 8 * 1024, "{r}");
        #[cfg(any(windows, target_os = "linux"))]
        {
            assert!(r["sistema"]["familia"].is_string(), "{r}");
            assert!(r["procesador"]["nucleos"].as_u64().unwrap_or(0) >= 1);
        }
        // La segunda vez sale de la memoria (lo mismo).
        assert_eq!(resumen(&[])["leido"], r["leido"]);
        // C4: lo que se ve del almacenamiento, si algo, sin números de serie.
        if let Some(a) = r.get("almacenamiento") {
            assert!(a["discos"].is_array() && a["conjuntos"].is_array(), "{a}");
            assert!(!a.to_string().to_lowercase().contains("serial"));
        }
    }
}
