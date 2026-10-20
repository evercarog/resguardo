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

/// Los discos de `/proc/self/mountinfo`: (punto de montaje, sistema de archivos), uno por
/// dispositivo (los subvolúmenes de btrfs o los conjuntos de datos de un mismo grupo ZFS
/// cuentan una vez, con su punto más corto). Sin lo que montan los contenedores ni snap.
pub fn discos_linux(mountinfo: &str) -> Vec<(String, String)> {
    let sin_escapes = |x: &str| x.replace(r"\040", " ").replace(r"\011", "\t").replace(r"\134", "\\");
    let mut vistos: Vec<(String, String, String)> = Vec::new();
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
            None => vistos.push((clave, punto, tipo.to_string())),
        }
    }
    vistos.sort_by(|a, b| a.1.cmp(&b.1));
    vistos.into_iter().take(MAX_DISCOS).map(|(_, p, t)| (p, t)).collect()
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
}

struct Cache {
    fijos: Value,
    leido_fijos: Instant,
    discos: Vec<Disco>,
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
            // 15ll (2): tarjetas de red que el sistema puede apagar para ahorrar energía.
            "red_ahorro": Some(crate::suspension::red_con_ahorro()).filter(|l| !l.is_empty()),
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
            out.push(Disco { punto: format!("{letra}:"), total: e.total, libre: e.libre, fs: crate::espacio::sistema_archivos(&ruta) });
        }
        out
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
            .filter_map(|(punto, fs)| {
                let e = crate::espacio::del_volumen(std::path::Path::new(&punto))?;
                Some(Disco { punto, total: e.total, libre: e.libre, fs: Some(fs) })
            })
            .collect()
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
    let c = g.get_or_insert_with(|| Cache { fijos: Value::Null, leido_fijos: ahora, discos: Vec::new(), leido_discos: ahora });
    if c.fijos.is_null() || ahora.duration_since(c.leido_fijos) >= CADA_FIJOS {
        let mut f = sistema::fijos();
        f["leido"] = json!(chrono::Local::now().to_rfc3339());
        c.fijos = f;
        c.leido_fijos = ahora;
        c.discos = sistema::discos();
        c.leido_discos = ahora;
    } else if ahora.duration_since(c.leido_discos) >= CADA_DISCOS {
        c.discos = sistema::discos();
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
        .map(|(d, u)| json!({ "punto": d.punto.chars().take(MAX_TEXTO).collect::<String>(), "total": d.total, "libre": d.libre, "sistema_archivos": d.fs, "en_uso": u }))
        .collect::<Vec<_>>()
        .into();
    r
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
    }
}
