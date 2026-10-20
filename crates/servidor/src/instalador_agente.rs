//! El instalador genérico del agente de Windows que el servidor sirve en
//! «Descargar instalador listo» (v1.17, `api::instaladores`).
//!
//! Resguardo Server para Windows lo trae junto al programa
//! (`agente/Resguardo-Agente-setup.exe`). En Linux (y en cualquier servidor) se
//! pone en la carpeta de datos con `resguardo-server poner-instalador-agente
//! <archivo.exe>`, que lo comprueba y lo deja solo para el usuario del servidor:
//! [`poner`]. Ver docs/servidor-linux.md («El instalador listo de Windows»).

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// El nombre del instalador genérico.
pub const NOMBRE: &str = "Resguardo-Agente-setup.exe";
/// Más pequeño no es un instalador del agente (lleva el agente y restic dentro).
pub const MIN_BYTES: u64 = 64 * 1024;
/// Se lee entero en memoria en cada «instalador listo»: un tope razonable.
pub const MAX_BYTES: u64 = 300 * 1024 * 1024;

/// Dónde lo busca el servidor en su carpeta de datos.
pub fn en_datos(datos: &Path) -> PathBuf {
    datos.join("agente").join(NOMBRE)
}

/// El instalador que sirve el servidor: el de `--instalador-agente` (o
/// `RESGUARDO_INSTALADOR_AGENTE`) si se da; si no, el de la carpeta de datos si
/// está, o el que viene junto al programa (Resguardo Server para Windows). Sin
/// ninguno, el de la carpeta de datos: si se pone después, vale sin reiniciar.
pub fn elegir(explicito: Option<PathBuf>, datos: &Path, junto_al_programa: Option<PathBuf>) -> PathBuf {
    if let Some(p) = explicito {
        return p;
    }
    let propio = en_datos(datos);
    if propio.is_file() {
        return propio;
    }
    junto_al_programa.filter(|p| p.is_file()).unwrap_or(propio)
}

/// Comprueba que `bytes` parece el instalador genérico: un ejecutable de
/// Windows (MZ y cabecera PE), de un tamaño razonable y sin la cola de un
/// instalador ya preparado para un equipo (ese lleva un código de un solo uso).
pub fn comprobar(bytes: &[u8]) -> Result<(), String> {
    let n = bytes.len() as u64;
    if n < MIN_BYTES {
        return Err(format!("Demasiado pequeño para ser el instalador del agente ({n} bytes)."));
    }
    if n > MAX_BYTES {
        return Err(format!("Demasiado grande ({} MB; como mucho {} MB).", n / (1024 * 1024), MAX_BYTES / (1024 * 1024)));
    }
    let pe = bytes.get(0x3c..0x40).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    let es_pe = bytes.starts_with(b"MZ") && pe.and_then(|o| bytes.get(o..o + 4)).is_some_and(|f| f == b"PE\0\0");
    if !es_pe {
        return Err("No es un ejecutable de Windows (Resguardo-Agente-setup.exe).".into());
    }
    if bytes.ends_with(resguardo_protocolo::instalador::FIN) {
        return Err("Es un instalador ya preparado para un equipo (lleva su código dentro). Usa el instalador genérico, Resguardo-Agente-setup.exe.".into());
    }
    Ok(())
}

/// SHA-256 en hexadecimal (minúsculas, como `sha256sum`).
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Lo que dejó [`poner`].
#[derive(Debug)]
pub struct Puesto {
    pub ruta: PathBuf,
    pub sha256: String,
    pub bytes: u64,
}

/// Comprueba `archivo` (y su SHA-256, si se da) y lo deja en la carpeta de
/// datos: carpeta `agente/` 0700 y archivo 0600, del dueño de la carpeta de
/// datos (el usuario del servicio). Reemplaza el anterior de una vez (rename).
/// Puesto a mano no se sabe su versión: se olvida la de [`de_publicacion`].
pub fn poner(datos: &Path, archivo: &Path, sha256: Option<&str>) -> Result<Puesto, String> {
    if !datos.is_dir() {
        return Err(format!("No existe la carpeta de datos {}: ¿está instalado el servidor? (o usa --datos)", datos.display()));
    }
    let meta = std::fs::metadata(archivo).map_err(|e| format!("No se pudo abrir {}: {e}", archivo.display()))?;
    if !meta.is_file() || meta.len() > MAX_BYTES {
        return Err(format!("{} no es un archivo válido (o pasa de {} MB).", archivo.display(), MAX_BYTES / (1024 * 1024)));
    }
    let bytes = std::fs::read(archivo).map_err(|e| format!("No se pudo leer {}: {e}", archivo.display()))?;
    let p = poner_bytes(datos, &bytes, sha256)?;
    let _ = std::fs::remove_file(marca(datos));
    Ok(p)
}

/// Comprueba `bytes` (y su SHA-256, si se da) y los deja como el instalador del servidor.
fn poner_bytes(datos: &Path, bytes: &[u8], sha256: Option<&str>) -> Result<Puesto, String> {
    comprobar(bytes)?;
    let suma = sha256_hex(bytes);
    if let Some(esperada) = sha256 {
        let esperada = esperada.trim().to_ascii_lowercase();
        if esperada != suma {
            return Err(format!("El SHA-256 no coincide: el archivo tiene {suma}, se esperaba {esperada}. No se pone."));
        }
    }
    let carpeta = datos.join("agente");
    // Como root, nunca a través de un enlace que haya podido dejar el usuario del servicio.
    if std::fs::symlink_metadata(&carpeta).is_ok_and(|m| !m.is_dir()) {
        return Err(format!("{} no es una carpeta (¿un enlace?): quítalo y repite.", carpeta.display()));
    }
    std::fs::create_dir_all(&carpeta).map_err(|e| format!("No se pudo crear {}: {e}", carpeta.display()))?;
    let destino = en_datos(datos);
    let tmp = carpeta.join(format!(".{NOMBRE}.{}", uuid::Uuid::new_v4().simple()));
    let escrito = escribir_solo_dueno(&tmp, bytes).and_then(|()| {
        #[cfg(unix)]
        dar_al_dueno(datos, &[carpeta.as_path(), tmp.as_path()]);
        std::fs::rename(&tmp, &destino).map_err(|e| e.to_string())
    });
    if let Err(e) = escrito {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("No se pudo dejar el instalador en {}: {e}", destino.display()));
    }
    Ok(Puesto { ruta: destino, sha256: suma, bytes: bytes.len() as u64 })
}

// ---------------------------------------------------------------------------
// 14dd: el instalador, de la publicación firmada que ya tiene el servidor
// ---------------------------------------------------------------------------

/// La versión del instalador puesto desde una publicación firmada (al lado, en `agente/`).
/// Sin ella, el instalador vino con el servidor o se puso a mano (versión desconocida).
pub fn marca(datos: &Path) -> PathBuf {
    datos.join("agente").join("instalador-version.json")
}

/// La versión del instalador de la carpeta de datos, si se puso desde una publicación.
pub fn version_puesta(datos: &Path) -> Option<String> {
    let t = std::fs::read_to_string(marca(datos)).ok()?;
    let v: serde_json::Value = serde_json::from_str(&t).ok()?;
    let version = v["version"].as_str()?;
    resguardo_protocolo::publicacion::Version::leer(version).map(|_| version.to_string())
}

/// 15x: la versión de un instalador por su SHA-256, si es el de Windows de alguna de las
/// publicaciones firmadas que tiene el servidor (`conocidas`).
pub fn version_por_huella(sha256: &str, conocidas: &[crate::publicaciones::Guardada]) -> Option<String> {
    conocidas
        .iter()
        .find(|g| {
            g.manifiesto.archivos.iter().any(|a| a.plataforma == "windows-x86_64" && a.tipo == "instalador-nsis" && a.sha256.eq_ignore_ascii_case(sha256))
        })
        .map(|g| g.manifiesto.version.clone())
}

/// La versión anotada al ponerlo desde una publicación, solo si el archivo sigue siendo ese
/// (mismo SHA-256): si alguien lo cambió a mano después, ya no se sabe.
fn version_anotada(datos: &Path, sha256: &str) -> Option<String> {
    let v = version_puesta(datos)?;
    let t = std::fs::read_to_string(marca(datos)).ok()?;
    let j: serde_json::Value = serde_json::from_str(&t).ok()?;
    match j["sha256"].as_str() {
        Some(h) if !h.eq_ignore_ascii_case(sha256) => None,
        _ => Some(v),
    }
}

/// 15x: qué instalador sirve el servidor.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Servido {
    /// Su versión, si se sabe.
    pub version: Option<String>,
    /// De dónde se sabe: `publicacion` (se puso desde una), `huella` (su SHA-256 es el de una
    /// publicación firmada que tiene el servidor) o `desconocida` (puesto a mano o el que trae
    /// el servidor, sin publicación conocida con su SHA-256).
    pub origen: &'static str,
}

/// 15x: la versión del instalador que está en `ruta` (con SHA-256 `sha256`).
pub fn servido(datos: &Path, ruta: &Path, sha256: &str, conocidas: &[crate::publicaciones::Guardada]) -> Servido {
    if ruta == en_datos(datos) {
        if let Some(v) = version_anotada(datos, sha256) {
            return Servido { version: Some(v), origen: "publicacion" };
        }
    }
    match version_por_huella(sha256, conocidas) {
        Some(v) => Servido { version: Some(v), origen: "huella" },
        None => Servido { version: None, origen: "desconocida" },
    }
}

/// SHA-256 de un archivo, recordado mientras no cambia (ruta, tamaño y fecha): el instalador
/// se lee entero (decenas de MB) y se pregunta a menudo.
pub fn huella_archivo(ruta: &Path) -> Option<(String, u64)> {
    type Recordada = (PathBuf, u64, Option<std::time::SystemTime>, String);
    static HUELLA: std::sync::Mutex<Option<Recordada>> = std::sync::Mutex::new(None);
    let meta = std::fs::metadata(ruta).ok().filter(|m| m.is_file())?;
    let (n, cuando) = (meta.len(), meta.modified().ok());
    if let Some((r, n0, c0, h)) = HUELLA.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        if r == ruta && *n0 == n && *c0 == cuando {
            return Some((h.clone(), n));
        }
    }
    let bytes = std::fs::read(ruta).ok()?;
    let n = bytes.len() as u64;
    let h = sha256_hex(&bytes);
    *HUELLA.lock().unwrap_or_else(|e| e.into_inner()) = Some((ruta.to_path_buf(), n, cuando, h.clone()));
    Some((h, n))
}

/// Lo que hizo [`de_publicacion`].
#[derive(Debug, PartialEq, Eq)]
pub enum DePublicacion {
    /// Puesto el de esa versión.
    Puesto { version: String, bytes: u64 },
    /// Ya estaba el de esa versión (o uno más nuevo).
    AlDia { version: String },
    /// Hay otro puesto a mano (o el que trae el servidor) y no se pidió cambiarlo.
    OtroAMano,
}

/// 14dd: pone como instalador del agente el de Windows de la publicación firmada `g`
/// (el `instalador-nsis` de `windows-x86_64` que lista su manifiesto, ya bajado y con su
/// SHA-256 comprobado), con las mismas comprobaciones que `poner-instalador-agente`.
/// Solo si no hay ninguno o el que hay es de una versión anterior; uno puesto a mano
/// (sin versión conocida) solo se cambia con `forzar` («Ponerlo ahora»). 15x: uno puesto a
/// mano cuyo SHA-256 es el de una publicación de `conocidas` cuenta con esa versión (y se
/// cambia solo si es más viejo).
pub fn de_publicacion(
    datos: &Path,
    g: &crate::publicaciones::Guardada,
    conocidas: &[crate::publicaciones::Guardada],
    forzar: bool,
) -> Result<DePublicacion, String> {
    use resguardo_protocolo::publicacion as p;
    let m = &g.manifiesto;
    let a = m
        .archivos
        .iter()
        .find(|a| a.plataforma == "windows-x86_64" && a.tipo == "instalador-nsis")
        .ok_or_else(|| format!("La versión {} no trae el instalador de Windows.", m.version))?;
    if g.faltan.contains(&a.nombre) {
        return Err(format!("Aún falta bajar {} de la versión {}.", a.nombre, m.version));
    }
    let hay = en_datos(datos).is_file();
    let actual = if hay { huella_archivo(&en_datos(datos)).map(|(h, _)| servido(datos, &en_datos(datos), &h, conocidas).version) } else { None };
    match actual.flatten() {
        Some(v) if hay && p::comparar(&v, &m.version).is_some_and(|o| o.is_ge()) => return Ok(DePublicacion::AlDia { version: v }),
        None if hay && !forzar => return Ok(DePublicacion::OtroAMano),
        _ => {}
    }
    let ruta = g.dir.join(&a.nombre);
    let meta = std::fs::symlink_metadata(&ruta).map_err(|e| format!("No se pudo abrir {}: {e}", a.nombre))?;
    if !meta.is_file() || meta.len() != a.tamano || meta.len() > MAX_BYTES {
        return Err(format!("{} no tiene el tamaño del manifiesto firmado.", a.nombre));
    }
    let bytes = std::fs::read(&ruta).map_err(|e| format!("No se pudo leer {}: {e}", a.nombre))?;
    // El SHA-256 firmado, otra vez sobre lo que se va a servir.
    let puesto = poner_bytes(datos, &bytes, Some(&a.sha256))?;
    let texto = serde_json::json!({ "version": m.version, "sha256": puesto.sha256, "archivo": a.nombre }).to_string();
    let tmp = marca(datos).with_extension(format!("{}.tmp", uuid::Uuid::new_v4().simple()));
    escribir_solo_dueno(&tmp, texto.as_bytes()).and_then(|()| std::fs::rename(&tmp, marca(datos)).map_err(|e| e.to_string())).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("No se pudo anotar la versión del instalador: {e}")
    })?;
    Ok(DePublicacion::Puesto { version: m.version.clone(), bytes: puesto.bytes })
}

fn escribir_solo_dueno(ruta: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    let mut f = o.open(ruta).map_err(|e| e.to_string())?;
    f.write_all(bytes).and_then(|()| f.sync_all()).map_err(|e| e.to_string())
}

/// En Linux, como root (sudo): la carpeta `agente/` (0700) y el archivo pasan a
/// ser del dueño de la carpeta de datos, para que el servicio pueda leerlos.
#[cfg(unix)]
fn dar_al_dueno(datos: &Path, rutas: &[&Path]) {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let Ok(m) = std::fs::metadata(datos) else { return };
    for p in rutas {
        if std::fs::symlink_metadata(p).is_ok_and(|x| x.file_type().is_symlink()) {
            continue;
        }
        if p.is_dir() {
            let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o700));
        }
        let _ = std::os::unix::fs::lchown(p, Some(m.uid()), Some(m.gid()));
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un ejecutable de Windows mínimo (MZ, e_lfanew y «PE\0\0») de `n` bytes.
    fn exe_falso(n: usize) -> Vec<u8> {
        let mut v = vec![0u8; n];
        v[..2].copy_from_slice(b"MZ");
        v[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        v[0x80..0x84].copy_from_slice(b"PE\0\0");
        v
    }

    #[test]
    fn solo_un_instalador_generico() {
        assert!(comprobar(&exe_falso(MIN_BYTES as usize)).is_ok());
        assert!(comprobar(&exe_falso(1024)).unwrap_err().contains("pequeño"));
        let mut sin_pe = exe_falso(MIN_BYTES as usize);
        sin_pe[0x80] = b'X';
        assert!(comprobar(&sin_pe).unwrap_err().contains("ejecutable"));
        let mut texto = vec![b'a'; MIN_BYTES as usize];
        texto[0x3c..0x40].copy_from_slice(&[0xff; 4]);
        assert!(comprobar(&texto).is_err());
        let mut preparado = exe_falso(MIN_BYTES as usize);
        preparado.extend_from_slice(resguardo_protocolo::instalador::FIN);
        assert!(comprobar(&preparado).unwrap_err().contains("ya preparado"));
    }

    #[test]
    fn poner_lo_deja_en_la_carpeta_de_datos() {
        let dir = tempfile::tempdir().unwrap();
        let datos = dir.path().join("datos");
        let exe = dir.path().join("setup.exe");
        std::fs::write(&exe, exe_falso(100_000)).unwrap();
        // Sin carpeta de datos, no.
        assert!(poner(&datos, &exe, None).is_err());
        std::fs::create_dir(&datos).unwrap();
        let suma = sha256_hex(&std::fs::read(&exe).unwrap());
        assert!(poner(&datos, &exe, Some(&"0".repeat(64))).unwrap_err().contains("no coincide"));
        assert!(!en_datos(&datos).exists());
        let p = poner(&datos, &exe, Some(&suma.to_uppercase())).unwrap();
        assert_eq!((p.ruta.clone(), p.sha256.clone(), p.bytes), (en_datos(&datos), suma, 100_000));
        assert_eq!(std::fs::read(&p.ruta).unwrap(), std::fs::read(&exe).unwrap());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&p.ruta).unwrap().permissions().mode() & 0o777, 0o600);
        }
        // Otra vez: lo reemplaza, sin restos.
        std::fs::write(&exe, exe_falso(200_000)).unwrap();
        assert_eq!(poner(&datos, &exe, None).unwrap().bytes, 200_000);
        assert_eq!(std::fs::read_dir(datos.join("agente")).unwrap().count(), 1);
    }

    #[test]
    fn elegir_prefiere_lo_explicito_y_luego_la_carpeta_de_datos() {
        let dir = tempfile::tempdir().unwrap();
        let junto = dir.path().join("junto.exe");
        let explicito = dir.path().join("otro.exe");
        // Sin nada: la carpeta de datos (para ponerlo después sin reiniciar).
        assert_eq!(elegir(None, dir.path(), Some(junto.clone())), en_datos(dir.path()));
        std::fs::write(&junto, b"x").unwrap();
        assert_eq!(elegir(None, dir.path(), Some(junto.clone())), junto);
        std::fs::create_dir_all(dir.path().join("agente")).unwrap();
        std::fs::write(en_datos(dir.path()), b"x").unwrap();
        assert_eq!(elegir(None, dir.path(), Some(junto.clone())), en_datos(dir.path()));
        assert_eq!(elegir(Some(explicito.clone()), dir.path(), Some(junto)), explicito);
    }
}
