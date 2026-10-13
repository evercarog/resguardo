//! Restaurar mejor (plan 0.8.0, bloque E: 13a, 13b, 13c, 13d, 13e, 14l, 14m).
//!
//! - **En otra carpeta** (`restaurar { destino: "nueva", carpeta }`, con la clave
//!   de administración): solo una carpeta **nueva o vacía** de un disco del equipo,
//!   nunca del sistema, de los programas ni de Resguardo, sin `..` ni enlaces en
//!   su camino. Nunca se sobrescribe nada. Vale también para una unidad entera.
//! - **En un .zip** (`destino: "zip"`, `carpeta`, `nombre?`, `contrasena_zip?`,
//!   también con la clave de administración): el .zip se crea nuevo en la
//!   carpeta elegida (las mismas reglas; si ya hay uno con ese nombre, otro
//!   nombre). Con contraseña, en **AES-256** (nunca ZipCrypto). Se escribe
//!   directamente desde `restic dump`: nada sin cifrar pasa por el disco.
//!   La descarga al navegador con contraseña usa lo mismo (`descargar`).
//! - **Desde otro sitio** (`desde`: `externa` o el id de una copia derivada): se
//!   explora y se restaura con el acceso que ya tiene el equipo a esa copia.
//! - **Registro** (13d, 14l): cada restauración deja una entrada `restauracion`
//!   en la bitácora (la ven todas las consolas): quién, desde qué consola, cuándo,
//!   cuánto tardó, qué versión, desde dónde, a dónde (el tipo, nunca la ruta),
//!   cuántos elementos, archivos y bytes, el motivo y el resultado. Sin rutas.
//! - **Restauración real** (13e): una correcta que restauró algo cuenta como
//!   prueba de restauración del repositorio (`restauraciones.json`).

use resguardo_motor::restic;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

/// Largo de la contraseña de un .zip (en caracteres).
pub const ZIP_CONTRASENA_MIN: usize = 8;
pub const ZIP_CONTRASENA_MAX: usize = 128;
/// Motivos de una restauración (14l).
pub const MOTIVOS: &[&str] = &["borrado", "dano", "prueba", "otro"];

// ---------- Carpeta nueva o vacía ----------

/// Una carpeta **nueva o vacía** donde restaurar (13a). Si no existe, se crea
/// (solo ella: la de arriba tiene que existir y valer). Si existe, tiene que ser
/// una carpeta vacía. Nunca del sistema, de los programas ni de Resguardo, ni
/// con enlaces en su camino (ver [`crate::sesiones_v2::carpeta_destino`]).
pub fn carpeta_nueva_o_vacia(d: &str, dueno: Option<&str>) -> Result<PathBuf, String> {
    crate::sesiones_v2::forma_de_carpeta(d)?;
    let p = Path::new(d);
    if crate::platform::hay_enlace_en_el_camino(p) {
        return Err("Esa carpeta es un enlace o está dentro de uno: elige una carpeta normal.".into());
    }
    if crate::sesiones_v2::es_del_sistema(d) {
        return Err(crate::sesiones_v2::MAL_DESTINO.into());
    }
    match std::fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() => {
            // Que exista y esté bien (también su ruta real), y vacía.
            crate::sesiones_v2::carpeta_destino(d)?;
            let vacia = std::fs::read_dir(p).map_err(|e| format!("No se pudo mirar la carpeta: {e}"))?.next().is_none();
            if !vacia {
                return Err("Esa carpeta no está vacía: elige una carpeta nueva o vacía (nunca se sobrescribe nada).".into());
            }
            Ok(p.to_path_buf())
        }
        Ok(_) => Err("Ya hay un archivo con ese nombre: elige una carpeta nueva o vacía.".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let padre = p.parent().filter(|x| !x.as_os_str().is_empty()).ok_or(crate::sesiones_v2::MAL_DESTINO)?;
            crate::sesiones_v2::carpeta_destino(&padre.display().to_string())
                .map_err(|_| "La carpeta de arriba no existe o no vale: elige otra (solo se crea una carpeta, la última).".to_string())?;
            match crate::platform::crear_carpeta_nueva(p, dueno) {
                Ok(()) => Ok(p.to_path_buf()),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err("Esa carpeta acaba de aparecer: vuelve a intentarlo con otra.".into()),
                Err(e) => Err(format!("No se pudo crear la carpeta: {e}")),
            }
        }
        Err(e) => Err(format!("No se pudo mirar la carpeta: {e}")),
    }
}

/// El nombre de un .zip: el pedido (sin caracteres que Windows no admite y con
/// `.zip`) o «Restaurado AAAA-MM-DD HHMM.zip».
pub fn nombre_zip(pedido: Option<&str>, sello: &str) -> Result<String, String> {
    let base = match pedido.map(str::trim).filter(|s| !s.is_empty()) {
        Some(n) => {
            let n = n.strip_suffix(".zip").or_else(|| n.strip_suffix(".ZIP")).unwrap_or(n).trim_end_matches(['.', ' ']);
            if n.is_empty() || n.chars().count() > 120 || n.chars().any(|c| c.is_control() || "<>:\"/\\|?*".contains(c)) || n.starts_with('.') {
                return Err(r#"Ese nombre no vale para el .zip: hasta 120 caracteres, sin \ / : * ? " < > |."#.into());
            }
            if crate::sesiones_v2::es_dispositivo(n) {
                return Err("Ese nombre no vale para el .zip.".into());
            }
            n.to_string()
        }
        None => format!("Restaurado {sello}"),
    };
    Ok(format!("{base}.zip"))
}

/// Crea el .zip **nuevo** en `carpeta` (si ya hay uno con ese nombre, «… (2).zip»…).
pub fn crear_zip_nuevo(carpeta: &Path, nombre: &str) -> Result<(PathBuf, std::fs::File), String> {
    let base = nombre.strip_suffix(".zip").unwrap_or(nombre);
    for n in 1..=50 {
        let p = if n == 1 { carpeta.join(nombre) } else { carpeta.join(format!("{base} ({n}).zip")) };
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&p) {
            Ok(f) => return Ok((p, f)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("No se pudo crear el .zip: {e}")),
        }
    }
    Err("Ya hay demasiados .zip con ese nombre: elige otro.".into())
}

/// La contraseña de un .zip (8 a 128 caracteres, sin caracteres de control).
pub fn contrasena_zip(c: &Value) -> Result<Option<String>, String> {
    match c.get("contrasena_zip") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.is_empty() => Ok(None),
        Some(Value::String(s)) => {
            let n = s.chars().count();
            if !(ZIP_CONTRASENA_MIN..=ZIP_CONTRASENA_MAX).contains(&n) || s.chars().any(char::is_control) {
                return Err(format!("La contraseña del .zip tiene que tener entre {ZIP_CONTRASENA_MIN} y {ZIP_CONTRASENA_MAX} caracteres."));
            }
            Ok(Some(s.clone()))
        }
        Some(_) => Err("Contraseña del .zip no válida.".into()),
    }
}

// ---------- Cifras de lo restaurado ----------

/// Lo que se restauró (para el registro y la prueba de restauración real).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Cifras {
    pub archivos: u64,
    pub bytes: u64,
}

impl Cifras {
    fn sumar(&mut self, o: Cifras) {
        self.archivos += o.archivos;
        self.bytes += o.bytes;
    }
}

/// Lo que dice una línea `--json` de `restic restore` (estado o resumen):
/// archivos y bytes restaurados y sus totales.
pub fn cifras_de_restore(l: &Value) -> Option<(Cifras, Option<Cifras>)> {
    if !matches!(l["message_type"].as_str(), Some("status" | "summary")) {
        return None;
    }
    let hechas = Cifras { archivos: l["files_restored"].as_u64().unwrap_or(0), bytes: l["bytes_restored"].as_u64().unwrap_or(0) };
    let total = match (l["total_files"].as_u64(), l["total_bytes"].as_u64()) {
        (None, None) => None,
        (a, b) => Some(Cifras { archivos: a.unwrap_or(0), bytes: b.unwrap_or(0) }),
    };
    Some((hechas, total))
}

/// Restaura `rutas` de `version` en `carpeta` (ya comprobada: nueva o vacía).
/// Una sola carpeta de origen: dentro, tal cual; varias: una subcarpeta por cada
/// una (con su nombre). Una unidad entera (`/C`) o una carpeta de la raíz va
/// en su propia subcarpeta (`C`). Nunca sobrescribe (`--overwrite never`).
pub fn restaurar_en_carpeta(
    acc: &restic::Access,
    version: &str,
    rutas: &[String],
    carpeta: &Path,
    guarda: &crate::escritorio::en_marcha::Guarda,
) -> Result<Cifras, String> {
    let grupos = por_carpeta_o_raiz(rutas)?;
    let mut usados = Vec::new();
    let mut total = Cifras::default();
    for (padre, nombres) in &grupos {
        let destino = if grupos.len() == 1 {
            carpeta.to_path_buf()
        } else {
            let d = carpeta.join(crate::sesiones_v2::nombre_de_grupo(padre, &mut usados));
            std::fs::create_dir(&d).map_err(|e| format!("No se pudo crear una carpeta para restaurar: {e}"))?;
            d
        };
        let origen = if padre.is_empty() { version.to_string() } else { format!("{version}:{padre}") };
        let destino_txt = destino.display().to_string();
        let mut args: Vec<String> = vec!["restore".into(), origen, "--target".into(), destino_txt, "--overwrite".into(), "never".into(), "--json".into()];
        for n in nombres {
            args.extend(["--include".into(), format!("/{n}")]);
        }
        let base = total;
        let mut ultima = Cifras::default();
        let mut al_avanzar = |l: &str| {
            if let Some((hechas, t)) = serde_json::from_str::<Value>(l).ok().as_ref().and_then(cifras_de_restore) {
                ultima = hechas;
                guarda.cifras(
                    Some(base.bytes + hechas.bytes),
                    t.map(|t| base.bytes + t.bytes),
                    Some(base.archivos + hechas.archivos),
                    t.map(|t| base.archivos + t.archivos),
                );
            }
        };
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = restic::run_raw_lines(acc, &refs, Duration::from_secs(48 * 3600), &mut al_avanzar)?;
        if out.code != Some(0) {
            return Err(restic::exit_error(out.code, &out.stderr));
        }
        total.sumar(ultima);
    }
    Ok(total)
}

/// Las rutas por su carpeta de arriba (`""` para lo que cuelga de la raíz: una
/// unidad entera en Windows o una carpeta de `/` en Linux).
pub fn por_carpeta_o_raiz(rutas: &[String]) -> Result<Vec<(String, Vec<String>)>, String> {
    let mut grupos: Vec<(String, Vec<String>)> = Vec::new();
    for ruta in rutas {
        let (padre, nombre) = partes_o_raiz(ruta)?;
        match grupos.iter_mut().find(|(p, _)| *p == padre) {
            Some((_, n)) => {
                if !n.contains(&nombre) {
                    n.push(nombre)
                }
            }
            None => grupos.push((padre, vec![nombre])),
        }
    }
    Ok(grupos)
}

/// Carpeta de arriba y nombre (la carpeta puede ser `""`: la raíz). Nunca `/` sola.
pub fn partes_o_raiz(ruta: &str) -> Result<(String, String), String> {
    let r = restic::snapshot_dir(ruta)?;
    let (padre, nombre) = r.rsplit_once('/').ok_or("Ruta no válida.")?;
    if nombre.is_empty() {
        return Err("Elige qué restaurar: una carpeta, un archivo o una unidad.".into());
    }
    Ok((padre.to_string(), nombre.to_string()))
}

// ---------- .zip desde `restic dump` ----------

/// ¿Es `ruta` una carpeta de la versión? (`restic ls --json`, solo ese nivel.)
fn es_carpeta(acc: &restic::Access, version: &str, ruta: &str) -> Result<bool, String> {
    let out = restic::run_raw(acc, &["ls", "--json", version, ruta], Duration::from_secs(3600))?;
    if out.code != Some(0) {
        return Err(restic::exit_error(out.code, &out.stderr));
    }
    for l in String::from_utf8_lossy(&out.stdout).lines() {
        if let Ok(v) = serde_json::from_str::<Value>(l) {
            if v["message_type"] == "node" && v["path"].as_str() == Some(ruta) {
                return Ok(v["type"] == "dir");
            }
        }
    }
    Err("Uno de los elementos elegidos ya no está en esa versión.".into())
}

/// Ejecuta `restic dump …` y da su salida a `leer` (stderr en otro hilo, para
/// que restic no se pare si se llena). Si `leer` falla, se para restic.
fn con_dump<T>(acc: &restic::Access, args: &[String], leer: impl FnOnce(&mut dyn Read) -> Result<T, String>) -> Result<T, String> {
    let (mut hijo, _vuelta) = restic::spawn_vuelta(acc, args)?;
    let mut stderr = hijo.stderr.take();
    let errores = std::thread::spawn(move || {
        let mut t = String::new();
        if let Some(e) = stderr.as_mut() {
            let _ = e.read_to_string(&mut t);
        }
        t
    });
    let mut salida = hijo.stdout.take().ok_or("No se pudo leer la versión.")?;
    let r = leer(&mut salida);
    if r.is_err() {
        let _ = hijo.kill();
    }
    let estado = hijo.wait().map_err(|e| e.to_string())?;
    let err = errores.join().unwrap_or_default();
    let v = r?;
    if !estado.success() {
        return Err(restic::exit_error(estado.code(), &err));
    }
    Ok(v)
}

/// El nombre dentro del .zip de una entrada del tar de `restic dump` de una
/// carpeta (sus rutas van desde la raíz, sin la `/` del principio): sin las
/// carpetas de arriba (`padre`), con `arriba` en lugar del nombre elegido (por
/// si dos elegidos se llaman igual) y sin `.`, `..` ni `\`. `None` si no es de
/// lo elegido.
pub fn nombre_en_zip(entrada: &str, padre: &str, nombre: &str, arriba: &str) -> Option<String> {
    let partes: Vec<&str> = entrada.split('/').filter(|s| !s.is_empty() && *s != ".").collect();
    if partes.iter().any(|s| *s == ".." || s.contains('\\') || s.contains('\0')) {
        return None;
    }
    let de_arriba: Vec<&str> = padre.split('/').filter(|s| !s.is_empty()).collect();
    let resto = partes.strip_prefix(de_arriba.as_slice())?;
    let (primero, dentro) = resto.split_first()?;
    if *primero != nombre {
        return None;
    }
    Some(std::iter::once(arriba).chain(dentro.iter().copied()).collect::<Vec<_>>().join("/"))
}

/// La hora de un archivo para el .zip (la local; el formato del .zip no tiene zona).
fn hora_zip(segundos: u64) -> Option<zip::DateTime> {
    use chrono::{Datelike, Timelike};
    let t = chrono::DateTime::from_timestamp(i64::try_from(segundos).ok()?, 0)?.with_timezone(&chrono::Local).naive_local();
    zip::DateTime::from_date_and_time(u16::try_from(t.year()).ok()?, t.month() as u8, t.day() as u8, t.hour() as u8, t.minute() as u8, t.second() as u8).ok()
}

/// Escribe en `w` un .zip con `rutas` de `version`, leído con `restic dump`
/// (nada pasa por el disco sin cifrar). Con `contrasena`, cada archivo en AES-256.
pub fn zip_de_rutas<W: Write + Seek>(
    acc: &restic::Access,
    version: &str,
    rutas: &[String],
    w: W,
    contrasena: Option<&str>,
    guarda: Option<&crate::escritorio::en_marcha::Guarda>,
) -> Result<(W, Cifras), String> {
    let mut z = zip::ZipWriter::new(w);
    let base = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).large_file(true);
    let opciones = |hora: Option<zip::DateTime>| {
        let o = match hora {
            Some(h) => base.last_modified_time(h),
            None => base,
        };
        match contrasena {
            Some(c) => o.with_aes_encryption(zip::AesMode::Aes256, c),
            None => o,
        }
    };
    let mut usados: Vec<String> = Vec::new();
    let mut cifras = Cifras::default();
    for ruta in rutas {
        let r = restic::snapshot_dir(ruta)?;
        let (padre, nombre) = partes_o_raiz(&r)?;
        // Si dos elegidos se llaman igual (de carpetas distintas), el segundo «nombre (2)».
        let arriba = crate::sesiones_v2::nombre_de_grupo(&format!("/{nombre}"), &mut usados);
        let avisar = |c: &Cifras| {
            if let Some(g) = guarda {
                g.cifras(Some(c.bytes), None, Some(c.archivos), None);
            }
        };
        if es_carpeta(acc, version, &r)? {
            let args: Vec<String> = vec!["dump".into(), "--no-lock".into(), "--archive".into(), "tar".into(), version.into(), r.clone()];
            con_dump(acc, &args, |salida| {
                let mut tar = tar::Archive::new(salida);
                for e in tar.entries().map_err(|e| format!("No se pudo leer la versión: {e}"))? {
                    let mut e = e.map_err(|e| format!("No se pudo leer la versión: {e}"))?;
                    let ruta_tar = String::from_utf8_lossy(&e.path_bytes()).into_owned();
                    let Some(dentro) = nombre_en_zip(&ruta_tar, &padre, &nombre, &arriba) else { continue };
                    let tipo = e.header().entry_type();
                    let hora = e.header().mtime().ok().and_then(hora_zip);
                    if tipo.is_dir() {
                        z.add_directory(format!("{dentro}/"), opciones(hora)).map_err(|e| e.to_string())?;
                    } else if tipo.is_file() {
                        z.start_file(dentro, opciones(hora)).map_err(|e| e.to_string())?;
                        cifras.bytes += std::io::copy(&mut e, &mut z).map_err(|e| format!("No se pudo escribir el .zip: {e}"))?;
                        cifras.archivos += 1;
                        avisar(&cifras);
                    }
                    // Enlaces y lo demás no van en el .zip (como al descargar).
                }
                Ok(())
            })?;
        } else {
            let args: Vec<String> = vec!["dump".into(), "--no-lock".into(), version.into(), r.clone()];
            z.start_file(arriba.clone(), opciones(None)).map_err(|e| e.to_string())?;
            let n = con_dump(acc, &args, |salida| std::io::copy(salida, &mut z).map_err(|e| format!("No se pudo escribir el .zip: {e}")))?;
            cifras.bytes += n;
            cifras.archivos += 1;
            avisar(&cifras);
        }
    }
    let w = z.finish().map_err(|e| format!("No se pudo terminar el .zip: {e}"))?;
    Ok((w, cifras))
}

// ---------- Quién, desde dónde y el registro ----------

/// Quién pide la restauración y desde dónde (para el registro y el progreso).
#[derive(Debug, Clone, Default)]
pub struct Quien {
    /// El nombre de quien la mandó (como lo dice su consola), si se sabe.
    pub por: Option<String>,
    /// El nombre de esa consola en el equipo (nunca su dirección); `None` en la ventana del equipo.
    pub consola: Option<String>,
    /// El enlace de esa consola (para saber si la empezó otra).
    pub enlace: String,
    /// La orden (su id en el servidor), si vino de una.
    pub orden: Option<String>,
}

/// Texto corto y limpio que llega de fuera (sin controles).
fn limpio(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

/// El motivo de la restauración (14l) que manda la consola: `{ tipo, notas? }`.
pub fn motivo(c: &Value) -> Option<Value> {
    let m = c.get("motivo")?;
    let tipo = m["tipo"].as_str().filter(|t| MOTIVOS.contains(t))?;
    let notas = m["notas"].as_str().map(|n| limpio(n, 500)).filter(|n| !n.is_empty());
    Some(json!({ "tipo": tipo, "notas": notas }))
}

/// El id de la entrada del registro: la de la orden si vino de una (así cada consola
/// puede añadirle su motivo), si no, una nueva.
pub fn id_registro(q: &Quien) -> String {
    match q.orden.as_deref().filter(|o| !o.is_empty() && o.len() <= 50 && o.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')) {
        Some(o) => format!("restauracion-{o}"),
        None => format!("restauracion-{}", uuid::Uuid::new_v4()),
    }
}

/// La entrada `restauracion` de la bitácora (sin rutas ni secretos).
#[allow(clippy::too_many_arguments)]
pub fn entrada(
    repo: &str,
    c: &Value,
    destino: &str,
    q: &Quien,
    inicio: chrono::DateTime<chrono::Local>,
    fin: chrono::DateTime<chrono::Local>,
    r: &Result<Cifras, String>,
    desde_nombre: Option<&str>,
) -> Value {
    let version: String = c["version"].as_str().unwrap_or("").chars().take(8).collect();
    let elementos = c["rutas"].as_array().map_or(0, Vec::len);
    let desde = match c["desde"].as_str() {
        None | Some("") => "principal",
        Some("externa") => "externa",
        Some(_) => "derivada",
    };
    let (resultado, mensaje, cifras) = match r {
        Ok(x) => ("ok", None, Some(*x)),
        Err(e) => ("fallo", Some(crate::web::public_message(e)), None),
    };
    json!({
        "id": id_registro(q),
        "hora": fin.to_rfc3339(),
        "inicio": inicio.to_rfc3339(),
        "duracion_s": (fin - inicio).num_seconds().max(0),
        "repo": repo,
        "version": version,
        "elementos": elementos,
        "archivos": cifras.map(|x| x.archivos),
        "bytes": cifras.map(|x| x.bytes),
        "destino": destino,
        "cifrado": (destino == "zip" || destino == "descarga") && c["contrasena_zip"].as_str().is_some_and(|s| !s.is_empty()),
        "desde": desde,
        "desde_nombre": desde_nombre.map(|n| limpio(n, 80)),
        "resultado": resultado,
        "mensaje": mensaje,
        "motivo": motivo(c),
        "por": q.por.as_deref().map(|p| limpio(p, 80)).filter(|p| !p.is_empty()),
        "consola": q.consola.as_deref().map(|p| limpio(p, 60)).filter(|p| !p.is_empty()),
        "orden": q.orden.as_deref().filter(|o| o.len() <= 50),
        "en_el_equipo": q.orden.is_none(),
    })
}

/// La última restauración real correcta de cada repositorio (13e).
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Reales {
    #[serde(default)]
    pub repos: HashMap<String, Real>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Real {
    pub inicio: String,
    pub fin: String,
    #[serde(default)]
    pub archivos: u64,
    #[serde(default)]
    pub bytes: u64,
}

const ARCHIVO_REALES: &str = "restauraciones.json";
static REALES: Mutex<()> = Mutex::new(());

pub fn reales() -> Reales {
    crate::agent::read_json(ARCHIVO_REALES)
}

/// La restauración real correcta de un repositorio, si hay.
pub fn real_de(repo: &str) -> Option<Real> {
    reales().repos.get(repo).cloned()
}

/// ¿Cuenta como prueba de restauración? Terminó bien y restauró algo.
pub fn cuenta_como_prueba(r: &Result<Cifras, String>) -> bool {
    r.as_ref().is_ok_and(|c| c.archivos > 0 || c.bytes > 0)
}

/// Anota lo hecho: la bitácora (todas las consolas) y, si cuenta, la prueba real.
#[allow(clippy::too_many_arguments)]
pub fn anotar(
    repo: &str,
    c: &Value,
    destino: &str,
    q: &Quien,
    inicio: chrono::DateTime<chrono::Local>,
    r: &Result<Cifras, String>,
    desde_nombre: Option<&str>,
) {
    let fin = chrono::Local::now();
    crate::bitacora::anotar("restauracion", entrada(repo, c, destino, q, inicio, fin, r, desde_nombre));
    if cuenta_como_prueba(r) && c["desde"].as_str().is_none_or(str::is_empty) {
        let Ok(_g) = REALES.lock() else { return };
        let mut todas = reales();
        let x = r.as_ref().copied().unwrap_or_default();
        todas.repos.insert(repo.to_string(), Real { inicio: inicio.to_rfc3339(), fin: fin.to_rfc3339(), archivos: x.archivos, bytes: x.bytes });
        if let Err(e) = crate::agent::write_json(ARCHIVO_REALES, &todas) {
            crate::agent::log(&format!("No se pudo anotar la restauración real: {e}"));
        }
    }
}

/// El acceso para restaurar o explorar: el del repositorio o, con `desde`, el de
/// su copia externa (`externa`) o una copia derivada (su id) (14m). Devuelve
/// también el nombre del destino de esa copia, si se sabe.
pub fn acceso_desde(principal: impl FnOnce() -> Result<restic::Access, String>, repo: &str, c: &Value) -> Result<(restic::Access, Option<String>), String> {
    match c["desde"].as_str().filter(|d| !d.is_empty()) {
        None => Ok((principal()?, None)),
        Some(d) if d == "externa" || crate::gestion_v2::id_derivada_valido(d) => crate::tasks::acceso_copia(repo, d),
        Some(_) => Err("Copia de origen no válida.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Con restic de verdad (sin él, se omite): en una carpeta nueva (y no en una
    /// que ya tiene algo), en un .zip con y sin contraseña, y lo que queda en el
    /// registro y como prueba de restauración real. Todo en una carpeta temporal.
    #[test]
    fn restaurar_en_carpeta_nueva_y_en_zip_con_restic() {
        let _real = crate::restic::tests::real_repo_lock();
        if restic::version().is_err() {
            return;
        }
        let base = std::env::temp_dir().join(format!("resguardo-restaurar-v2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::env::set_var("RESGUARDO_AGENT_DIR", base.join("agente"));
        let datos = base.join("datos");
        std::fs::create_dir_all(datos.join("sub")).unwrap();
        std::fs::write(datos.join("a.txt"), b"hola").unwrap();
        std::fs::write(datos.join("sub").join("b.txt"), b"adios").unwrap();
        let acc = restic::Access {
            location: base.join("repo").display().to_string(),
            password: "contraseña de prueba".into(),
            rest_auth: None,
            cacert: None,
            env: Vec::new(),
        };
        let d = datos.display().to_string();
        assert_eq!(restic::run_raw(&acc, &["init"], Duration::from_secs(120)).unwrap().code, Some(0));
        assert_eq!(restic::run_raw(&acc, &["backup", &d], Duration::from_secs(120)).unwrap().code, Some(0));
        let ls = restic::run_raw(&acc, &["ls", "--json", "latest"], Duration::from_secs(120)).unwrap();
        let nodos: Vec<Value> = String::from_utf8_lossy(&ls.stdout).lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
        let version = nodos.iter().find(|n| n["message_type"] == "snapshot").and_then(|n| n["id"].as_str()).unwrap().to_string();
        let ruta = nodos.iter().find(|n| n["name"] == "datos" && n["type"] == "dir").and_then(|n| n["path"].as_str()).unwrap().to_string();
        let ruta_a = format!("{ruta}/a.txt");

        // 13a: en una carpeta nueva.
        let nueva = base.join("restaurado");
        let c = json!({ "repo": "r1", "version": version, "rutas": [ruta], "destino": "nueva", "carpeta": nueva.display().to_string() });
        crate::sesiones_v2::restaurar(&acc, &c, None).unwrap();
        assert_eq!(std::fs::read(nueva.join("datos").join("sub").join("b.txt")).unwrap(), b"adios");
        // Ya no está vacía: no se sobrescribe nada.
        let e = crate::sesiones_v2::restaurar(&acc, &c, None).unwrap_err();
        assert!(e.contains("no está vacía"), "{e}");
        // La de arriba tiene que existir (solo se crea la última).
        let lejos =
            json!({ "repo": "r1", "version": version, "rutas": [ruta], "destino": "nueva", "carpeta": base.join("no").join("existe").display().to_string() });
        assert!(crate::sesiones_v2::restaurar(&acc, &lejos, None).is_err());

        // 13b: en un .zip con contraseña (AES-256) y sin ella.
        let zips = base.join("zips");
        let c = json!({ "repo": "r1", "version": version, "rutas": [ruta, ruta_a], "destino": "zip", "carpeta": zips.display().to_string(),
                        "nombre": "Prueba", "contrasena_zip": "una clave larga" });
        crate::sesiones_v2::restaurar(&acc, &c, None).unwrap();
        let mut z = zip::ZipArchive::new(std::fs::File::open(zips.join("Prueba.zip")).unwrap()).unwrap();
        let mut leido = String::new();
        z.by_name_decrypt("datos/sub/b.txt", b"una clave larga").unwrap().read_to_string(&mut leido).unwrap();
        assert_eq!(leido, "adios");
        // El archivo suelto, con otro nombre (dos elegidos se llaman distinto aquí: «datos» y «a.txt»).
        leido.clear();
        z.by_name_decrypt("a.txt", b"una clave larga").unwrap().read_to_string(&mut leido).unwrap();
        assert_eq!(leido, "hola");
        assert!(z.by_name_decrypt("datos/a.txt", b"otra clave").is_err(), "con otra contraseña no se abre");
        // Otra vez con el mismo nombre: otro .zip, nunca encima.
        let sin = json!({ "repo": "r1", "version": version, "rutas": [ruta_a], "destino": "zip", "carpeta": zips.display().to_string(), "nombre": "Prueba" });
        crate::sesiones_v2::restaurar(&acc, &sin, None).unwrap();
        let mut z2 = zip::ZipArchive::new(std::fs::File::open(zips.join("Prueba (2).zip")).unwrap()).unwrap();
        leido.clear();
        z2.by_name("a.txt").unwrap().read_to_string(&mut leido).unwrap();
        assert_eq!(leido, "hola");

        // 13d: en el registro, sin rutas ni la contraseña; 13e: cuenta como prueba.
        let registro: Vec<Value> = crate::bitacora::leer().into_iter().filter(|e| e["tipo"] == "restauracion").collect();
        assert_eq!(registro.len(), 5, "{registro:?}");
        let texto = serde_json::to_string(&registro).unwrap();
        assert!(!texto.contains("una clave larga") && !texto.contains(&d) && !texto.contains("datos"), "{texto}");
        assert!(registro.iter().any(|e| e["destino"] == "zip" && e["cifrado"] == true && e["resultado"] == "ok" && e["archivos"] == 3));
        assert!(registro.iter().any(|e| e["destino"] == "nueva" && e["resultado"] == "fallo"));
        assert!(real_de("r1").is_some_and(|r| r.archivos > 0));

        std::env::remove_var("RESGUARDO_AGENT_DIR");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn nombres_dentro_del_zip() {
        // `restic dump` de /C/Users/ana/Docs da «C/Users/ana/Docs/…».
        assert_eq!(nombre_en_zip("C/Users/ana/Docs/a.txt", "/C/Users/ana", "Docs", "Docs").as_deref(), Some("Docs/a.txt"));
        assert_eq!(nombre_en_zip("C/Users/ana/Docs/", "/C/Users/ana", "Docs", "Docs").as_deref(), Some("Docs"));
        assert_eq!(nombre_en_zip("C/Users/ana/Docs/sub/b", "/C/Users/ana", "Docs", "Docs (2)").as_deref(), Some("Docs (2)/sub/b"));
        // Una unidad entera.
        assert_eq!(nombre_en_zip("C/x/y.txt", "", "C", "C").as_deref(), Some("C/x/y.txt"));
        // Lo que no es de lo elegido, o con «..» o «\», fuera.
        assert_eq!(nombre_en_zip("C/Users/ana/Otra/a.txt", "/C/Users/ana", "Docs", "Docs"), None);
        assert_eq!(nombre_en_zip("C/Users/ana/Docs/../../x", "/C/Users/ana", "Docs", "Docs"), None);
        assert_eq!(nombre_en_zip(r"C/Users/ana/Docs/a\..\b", "/C/Users/ana", "Docs", "Docs"), None);
        assert_eq!(nombre_en_zip("D/Users/ana/Docs/a", "/C/Users/ana", "Docs", "Docs"), None);
    }

    #[test]
    fn nombres_del_zip() {
        assert_eq!(nombre_zip(None, "2026-10-13 0900").unwrap(), "Restaurado 2026-10-13 0900.zip");
        assert_eq!(nombre_zip(Some("Facturas.zip"), "x").unwrap(), "Facturas.zip");
        assert_eq!(nombre_zip(Some("  Facturas  "), "x").unwrap(), "Facturas.zip");
        for mal in ["a/b", r"a\b", "..", ".oculto", "CON", "nul.zip", "a:b", "a*"] {
            assert!(nombre_zip(Some(mal), "x").is_err(), "{mal}");
        }
    }

    #[test]
    fn contrasena_del_zip() {
        assert_eq!(contrasena_zip(&json!({})).unwrap(), None);
        assert_eq!(contrasena_zip(&json!({ "contrasena_zip": "" })).unwrap(), None);
        assert_eq!(contrasena_zip(&json!({ "contrasena_zip": "una buena clave" })).unwrap().as_deref(), Some("una buena clave"));
        assert!(contrasena_zip(&json!({ "contrasena_zip": "corta" })).is_err());
        assert!(contrasena_zip(&json!({ "contrasena_zip": "con\ncontrol!!" })).is_err());
        assert!(contrasena_zip(&json!({ "contrasena_zip": 12345678 })).is_err());
        assert!(contrasena_zip(&json!({ "contrasena_zip": "x".repeat(129) })).is_err());
    }

    #[test]
    fn cifras_de_las_lineas_de_restore() {
        let (h, t) =
            cifras_de_restore(&json!({ "message_type": "status", "files_restored": 3, "bytes_restored": 30, "total_files": 10, "total_bytes": 100 })).unwrap();
        assert_eq!((h, t), (Cifras { archivos: 3, bytes: 30 }, Some(Cifras { archivos: 10, bytes: 100 })));
        assert!(cifras_de_restore(&json!({ "message_type": "verbose_status" })).is_none());
        assert!(cuenta_como_prueba(&Ok(Cifras { archivos: 1, bytes: 0 })));
        assert!(!cuenta_como_prueba(&Ok(Cifras::default())));
        assert!(!cuenta_como_prueba(&Err("x".into())));
    }

    #[test]
    fn rutas_por_carpeta_y_raices() {
        let g = por_carpeta_o_raiz(&["/C/a/x".into(), "/C/a/y".into(), "/C".into(), "/D".into(), "/C/a/x".into()]).unwrap();
        assert_eq!(g, vec![("/C/a".to_string(), vec!["x".to_string(), "y".to_string()]), (String::new(), vec!["C".to_string(), "D".to_string()])]);
        assert!(por_carpeta_o_raiz(&["/".into()]).is_err());
        assert!(por_carpeta_o_raiz(&["/C/../x".into()]).is_err());
    }

    #[test]
    fn la_entrada_del_registro_no_lleva_rutas_ni_secretos() {
        let c = json!({ "repo": "r1", "version": "0123456789abcdef", "rutas": ["/C/Users/ana/Secreto.txt", "/C/x"], "contrasena_zip": "una clave muy buena",
                        "motivo": { "tipo": "borrado", "notas": "Lo borró\u{7} sin querer" } });
        let q = Quien { por: Some("Ana".into()), consola: Some("Oficina".into()), enlace: "e".into(), orden: Some("abc-123".into()) };
        let t0 = chrono::Local::now();
        let e = entrada("r1", &c, "zip", &q, t0 - chrono::Duration::seconds(90), t0, &Ok(Cifras { archivos: 4, bytes: 1000 }), None);
        let s = e.to_string();
        assert!(!s.contains("Secreto") && !s.contains("Users") && !s.contains("una clave muy buena"), "{s}");
        assert_eq!(e["id"], "restauracion-abc-123");
        assert_eq!((e["version"].as_str(), e["elementos"].as_u64(), e["duracion_s"].as_i64()), (Some("01234567"), Some(2), Some(90)));
        assert_eq!((e["cifrado"].as_bool(), e["desde"].as_str(), e["resultado"].as_str()), (Some(true), Some("principal"), Some("ok")));
        assert_eq!(e["motivo"], json!({ "tipo": "borrado", "notas": "Lo borró sin querer" }));
        // Un motivo que no existe no se guarda; un fallo, con su mensaje sin rutas.
        let c2 = json!({ "version": "x", "rutas": [], "motivo": { "tipo": "inventado" }, "desde": "deriv-1" });
        let e2 = entrada("r1", &c2, "nueva", &Quien::default(), t0, t0, &Err(r#"No se pudo escribir en C:\Users\ana\x"#.into()), Some("Nube"));
        assert!(e2["motivo"].is_null() && e2["desde"] == "derivada" && e2["resultado"] == "fallo" && e2["en_el_equipo"] == true);
        assert!(!e2.to_string().contains("ana"), "{e2}");
    }
}
