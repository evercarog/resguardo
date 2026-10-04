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
pub fn poner(datos: &Path, archivo: &Path, sha256: Option<&str>) -> Result<Puesto, String> {
    if !datos.is_dir() {
        return Err(format!("No existe la carpeta de datos {}: ¿está instalado el servidor? (o usa --datos)", datos.display()));
    }
    let meta = std::fs::metadata(archivo).map_err(|e| format!("No se pudo abrir {}: {e}", archivo.display()))?;
    if !meta.is_file() || meta.len() > MAX_BYTES {
        return Err(format!("{} no es un archivo válido (o pasa de {} MB).", archivo.display(), MAX_BYTES / (1024 * 1024)));
    }
    let bytes = std::fs::read(archivo).map_err(|e| format!("No se pudo leer {}: {e}", archivo.display()))?;
    comprobar(&bytes)?;
    let suma = sha256_hex(&bytes);
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
    let escrito = escribir_solo_dueno(&tmp, &bytes).and_then(|()| {
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
