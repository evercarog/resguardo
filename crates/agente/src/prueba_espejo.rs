//! Prueba de restauración desde un espejo (plan 0.8.0, E4, 14j).
//!
//! La prueba de restauración de siempre (`restore_test.rs`: unos archivos al azar de
//! una versión, restaurados con `--verify` en la carpeta privada, comparados y
//! borrados) pero **desde la copia de un espejo**, para saber que ese destino sirve de
//! verdad el día que haga falta. La hace el **equipo dueño del repositorio**, que tiene
//! su contraseña: el almacén nunca la recibe. Desde dónde:
//!
//! - `trabajo`: un espejo que hace el propio equipo (`espejo_trabajos`, `quien: equipo`),
//!   a otra carpeta suya (`<carpeta>/<repo>`) o a una nube conectada en él;
//! - `nube` + `carpeta`: una nube conectada en este equipo y la carpeta del repositorio en
//!   ella (p. ej. el espejo del almacén en la misma cuenta de B2 o Dropbox);
//! - `carpeta`: la carpeta del repositorio en un disco de este equipo (el disco del espejo
//!   conectado aquí, o el espejo del almacén cuando el almacén es este mismo equipo).
//!
//! Órdenes (dentro de `probar_restauracion`, inofensiva: solo lee y borra lo restaurado):
//! `{ repo, espejo: Prueba }` la hace ahora; `{ repo, programar_espejo: Prueba }` la guarda
//! con `cada_dias` (p. ej. cada 30); `{ repo, quitar_espejo: id }` la quita. El resultado va
//! en el informe (`repos[].pruebas_espejo[]`, de ahí el aviso `prueba_fallida`) y en la
//! bitácora como una `prueba_restauracion` con `espejo: { id, nombre }`. Sin rutas en
//! lo que sale del equipo.

use crate::espejo_trabajos as et;
use crate::servidor_v2::Vinculo;
use resguardo_motor::restic::Access;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// `resumen.admite`.
pub const ADMITE: &str = "prueba_espejo";
const ARCHIVO: &str = "pruebas-espejo.json";
/// Pruebas programadas por repositorio.
pub const MAX_POR_REPO: usize = 8;
/// Cada cuántos días, como mucho.
pub const MAX_DIAS: u32 = 92;
/// La primera programada, de madrugada (después de la prueba de siempre, a las 04:00).
const HORA_PRIMERA: u32 = 5;

/// Desde dónde se prueba (una de las tres formas).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Prueba {
    /// `[a-z0-9-]`, la elige la consola (una por espejo).
    pub id: String,
    #[serde(default)]
    pub nombre: String,
    /// La clave del destino en la consola (opaca; solo se devuelve para enlazarla).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destino: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trabajo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nube: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carpeta: Option<String>,
    /// Programada: cada cuántos días.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cada_dias: Option<u32>,
}

/// Una programada, con desde cuándo cuenta su horario.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Programada {
    pub prueba: Prueba,
    /// RFC 3339: el comienzo de la última vuelta (o la referencia para la primera).
    pub desde: String,
}

/// El resultado de la última vuelta de una prueba (sin rutas).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Ultima {
    pub nombre: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destino: Option<String>,
    pub inicio: String,
    pub fin: String,
    /// `ok` o `fallo`.
    pub resultado: String,
    pub mensaje: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archivos: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Guardado {
    /// repo → programadas.
    #[serde(default)]
    pub programadas: BTreeMap<String, Vec<Programada>>,
    /// `<repo>|<id>` → la última vuelta.
    #[serde(default)]
    pub ultimas: BTreeMap<String, Ultima>,
}

static ARCHIVO_CERROJO: Mutex<()> = Mutex::new(());
static EN_MARCHA: AtomicBool = AtomicBool::new(false);

pub fn cargar() -> Guardado {
    crate::agent::read_json(ARCHIVO)
}

/// Cambia lo guardado (se vuelve a leer: otra vuelta pudo cambiarlo).
fn cambiar<T>(f: impl FnOnce(&mut Guardado) -> Result<T, String>) -> Result<T, String> {
    let _g = ARCHIVO_CERROJO.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = cargar();
    let r = f(&mut g)?;
    crate::agent::write_json(ARCHIVO, &g)?;
    Ok(r)
}

fn id_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn texto(t: &str, max: usize) -> Option<String> {
    let t = t.trim();
    (t.chars().count() <= max && !t.chars().any(char::is_control)).then(|| t.to_string())
}

/// Comprueba lo que manda la consola (todo es de fuera).
pub fn validar(p: &mut Prueba) -> Result<(), String> {
    if !id_valido(&p.id) {
        return Err("Prueba no válida.".into());
    }
    p.nombre = texto(&p.nombre, 80).ok_or("Nombre de la prueba no válido.")?;
    p.destino = match p.destino.as_deref() {
        Some(d) => Some(texto(d, 160).ok_or("Destino no válido.")?).filter(|d| !d.is_empty()),
        None => None,
    };
    match (p.trabajo.is_some(), p.nube.is_some(), p.carpeta.is_some()) {
        (true, false, false) => {
            let t = p.trabajo.as_deref().unwrap_or_default();
            if t.is_empty() || t.len() > 64 || !t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                return Err("Espejo no válido.".into());
            }
        }
        (false, true, true) => {
            let n = p.nube.as_deref().and_then(|n| texto(n, 80)).filter(|n| !n.is_empty()).ok_or("Nube no válida.")?;
            p.nube = Some(n);
            let c = p.carpeta.as_deref().unwrap_or_default();
            if !crate::nube::carpeta_remota_valida(c) {
                return Err("Escribe la carpeta del repositorio dentro de la nube (sin «..»).".into());
            }
            p.carpeta = Some(c.trim().trim_matches('/').to_string());
        }
        (false, false, true) => {
            let c = p.carpeta.as_deref().unwrap_or_default().trim().to_string();
            crate::platform::carpeta_local_valida(&c)?;
            p.carpeta = Some(c);
        }
        _ => return Err("Di desde dónde: un espejo del equipo, una nube conectada o una carpeta de un disco.".into()),
    }
    if p.cada_dias.is_some_and(|d| !(1..=MAX_DIAS).contains(&d)) {
        return Err(format!("Programada: de cada día a cada {MAX_DIAS} días."));
    }
    Ok(())
}

/// El acceso de restic a la copia del espejo, con la contraseña del repositorio.
pub fn acceso(v: &Vinculo, repo: &str, p: &Prueba) -> Result<Access, String> {
    let base = crate::gestion_v2::acceso(v, repo)?;
    let (location, env) = if let Some(t) = &p.trabajo {
        let e = et::cargar_equipo();
        let t = e.trabajos.iter().find(|x| &x.id == t && x.es_equipo()).ok_or("Ese espejo del equipo ya no está.")?;
        if !et::repos_de(t, &et::repos_locales()).iter().any(|(id, _)| id == repo) {
            return Err("Ese espejo no copia este repositorio.".into());
        }
        match t.adonde.tipo.as_str() {
            "nube" => {
                let n = t.adonde.nube.as_deref().unwrap_or_default();
                (crate::nube::ubicacion_restic(t.adonde.carpeta.trim().trim_matches('/'), repo), crate::nube::entorno_restic(n)?)
            }
            _ => {
                crate::platform::carpeta_local_valida(&t.adonde.carpeta)?;
                (carpeta_repo(&Path::new(t.adonde.carpeta.trim()).join(repo))?, Vec::new())
            }
        }
    } else if let Some(n) = &p.nube {
        let c = p.carpeta.as_deref().unwrap_or_default();
        let (arriba, ultimo) = c.rsplit_once('/').unwrap_or(("", c));
        (crate::nube::ubicacion_restic(arriba, ultimo), crate::nube::entorno_restic(n)?)
    } else {
        let c = p.carpeta.as_deref().ok_or("Falta la carpeta.")?;
        crate::platform::carpeta_local_valida(c)?;
        (carpeta_repo(Path::new(c))?, Vec::new())
    };
    Ok(Access { location, password: base.password, rest_auth: None, cacert: None, env })
}

/// Una carpeta local que tiene una copia de un repositorio (su archivo `config`).
fn carpeta_repo(p: &Path) -> Result<String, String> {
    if !p.join("config").is_file() {
        return Err("En esa carpeta no hay una copia del repositorio (¿está conectado el disco del espejo?).".into());
    }
    Ok(p.display().to_string())
}

fn clave(repo: &str, id: &str) -> String {
    format!("{repo}|{id}")
}

/// La carpeta privada de estas pruebas (aparte de la de la prueba de siempre, que corre en otro proceso).
fn carpeta_trabajo() -> PathBuf {
    crate::agent::private_dir().join("prueba-espejo")
}

/// Hace la prueba (bloquea: en otro hilo) y la anota.
pub fn hacer(v: &Vinculo, repo: &str, p: &Prueba) -> Ultima {
    let inicio = chrono::Local::now();
    let nombre_repo = v.repos_v2.iter().find(|r| r.id == repo).map(|r| r.nombre.clone()).unwrap_or_else(|| repo.to_string());
    let op = crate::progreso_v2::ops::empezar(crate::progreso_v2::ops::Operacion::de_consola(
        v,
        "prueba_restauracion",
        repo,
        &format!("Probando desde «{}»…", p.nombre),
    ));
    let cfg =
        crate::restore_test::RestoreTest { schedule: crate::agent::Schedule::Hours { every: 24 }, files: 20, max_mb: 200, enabled_at: inicio.to_rfc3339() };
    let record = match acceso(v, repo, p) {
        Ok(acc) => {
            let base = carpeta_trabajo();
            let _ = std::fs::create_dir_all(&base);
            crate::restore_test::run_en(&acc, &cfg, &base, &mut |etapa, _| op.avance(etapa, None, None))
        }
        Err(e) => crate::agent::RunRecord {
            started: inicio.to_rfc3339(),
            finished: chrono::Local::now().to_rfc3339(),
            result: "error".into(),
            message: crate::web::public_message(&e),
            ..Default::default()
        },
    };
    drop(op);
    let ok = record.result == "ok";
    let version = record.snapshot_id.as_deref().filter(|s| s.len() >= 8 && s.bytes().all(|c| c.is_ascii_hexdigit())).map(|s| s[..8].to_ascii_lowercase());
    let u = Ultima {
        nombre: p.nombre.clone(),
        destino: p.destino.clone(),
        inicio: record.started.clone(),
        fin: if record.finished.is_empty() { chrono::Local::now().to_rfc3339() } else { record.finished.clone() },
        resultado: if ok { "ok" } else { "fallo" }.into(),
        mensaje: crate::web::public_message(&record.message),
        archivos: record.files_new,
        bytes: record.data_added,
        version,
    };
    let duracion =
        chrono::DateTime::parse_from_rfc3339(&u.fin).ok().zip(chrono::DateTime::parse_from_rfc3339(&u.inicio).ok()).map(|(f, i)| (f - i).num_seconds().max(0));
    crate::agent::log(&format!("{}Prueba de restauración de «{nombre_repo}» desde «{}»: {}", if ok { "" } else { "ERROR: " }, p.nombre, u.mensaje));
    crate::bitacora::anotar(
        "prueba_restauracion",
        json!({
            "hora": u.fin, "repo": repo, "resultado": u.resultado, "mensaje": u.mensaje, "duracion_s": duracion,
            "archivos": u.archivos, "bytes": u.bytes, "version": u.version,
            "espejo": { "id": p.id, "nombre": p.nombre },
        }),
    );
    let (k, u2) = (clave(repo, &p.id), u.clone());
    let _ = cambiar(|g| {
        g.ultimas.insert(k, u2);
        Ok(())
    });
    u
}

/// En otro hilo, de una en una. `Err` si ya hay una en marcha.
fn lanzar(v: Vinculo, repo: String, p: Prueba) -> Result<(), String> {
    if EN_MARCHA.swap(true, Ordering::SeqCst) {
        return Err("Ya hay una prueba desde un espejo en marcha en este equipo: espera a que termine.".into());
    }
    std::thread::spawn(move || {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| hacer(&v, &repo, &p)));
        EN_MARCHA.store(false, Ordering::SeqCst);
    });
    Ok(())
}

fn prueba_de(c: &Value, campo: &str) -> Result<Prueba, String> {
    let mut p: Prueba = serde_json::from_value(c[campo].clone()).map_err(|_| "Prueba no válida.".to_string())?;
    validar(&mut p)?;
    Ok(p)
}

/// `probar_restauracion { repo, espejo }`: ahora.
pub fn pedir_ahora(v: &Vinculo, repo: &str, c: &Value) -> Result<String, String> {
    let p = prueba_de(c, "espejo")?;
    // Lo que se puede saber ya (el espejo existe, la carpeta tiene la copia, la nube está): fuera de la prueba.
    acceso(v, repo, &p)?;
    let nombre = p.nombre.clone();
    lanzar(v.clone(), repo.to_string(), p)?;
    Ok(format!("Prueba desde «{nombre}» pedida: empieza ahora."))
}

/// La referencia de la primera vuelta de una programada: que la primera sea a las 05:00 siguientes.
fn desde_primera(cada_dias: u32, ahora: chrono::DateTime<chrono::Local>) -> String {
    use chrono::TimeZone;
    let hoy = ahora.date_naive().and_hms_opt(HORA_PRIMERA, 0, 0).and_then(|t| chrono::Local.from_local_datetime(&t).earliest());
    let primera = match hoy {
        Some(t) if t > ahora + chrono::Duration::hours(1) => t,
        Some(t) => t + chrono::Duration::days(1),
        None => ahora + chrono::Duration::hours(1),
    };
    (primera - chrono::Duration::days(i64::from(cada_dias))).to_rfc3339()
}

/// `probar_restauracion { repo, programar_espejo }`: la guarda (o la cambia) con su horario.
pub fn programar(v: &Vinculo, repo: &str, c: &Value) -> Result<String, String> {
    let p = prueba_de(c, "programar_espejo")?;
    let dias = p.cada_dias.ok_or("Di cada cuántos días.")?;
    acceso(v, repo, &p)?;
    let nombre = p.nombre.clone();
    cambiar(|g| {
        let l = g.programadas.entry(repo.to_string()).or_default();
        let n = l.len();
        match l.iter_mut().find(|x| x.prueba.id == p.id) {
            // El mismo horario: no vuelve a empezar.
            Some(x) if x.prueba.cada_dias == p.cada_dias => x.prueba = p,
            Some(x) => *x = Programada { desde: desde_primera(dias, chrono::Local::now()), prueba: p },
            None if n >= MAX_POR_REPO => return Err(format!("Como mucho {MAX_POR_REPO} pruebas desde espejos por repositorio.")),
            None => l.push(Programada { desde: desde_primera(dias, chrono::Local::now()), prueba: p }),
        }
        Ok(())
    })?;
    Ok(format!("Prueba desde «{nombre}» programada cada {dias} días."))
}

/// `probar_restauracion { repo, quitar_espejo: id }`.
pub fn quitar(repo: &str, c: &Value) -> Result<String, String> {
    let id = c["quitar_espejo"].as_str().filter(|x| id_valido(x)).ok_or("Prueba no válida.")?.to_string();
    let nombre = cambiar(|g| {
        let l = g.programadas.entry(repo.to_string()).or_default();
        let pos = l.iter().position(|x| x.prueba.id == id).ok_or("Esa prueba ya no está programada.")?;
        let x = l.remove(pos);
        if l.is_empty() {
            g.programadas.remove(repo);
        }
        Ok(x.prueba.nombre)
    })?;
    Ok(format!("La prueba desde «{nombre}» ya no está programada (sus resultados se quedan)."))
}

/// ¿Toca? (cada `cada_dias` desde `desde`).
pub fn toca(p: &Programada, ahora: chrono::DateTime<chrono::Local>) -> bool {
    let Some(d) = p.prueba.cada_dias else { return false };
    chrono::DateTime::parse_from_rfc3339(&p.desde).map_or(true, |t| ahora.signed_duration_since(t) >= chrono::Duration::days(i64::from(d)))
}

/// Lo llama el servicio en cada vuelta: la primera programada que toca, en otro hilo.
pub fn si_toca() {
    if EN_MARCHA.load(Ordering::SeqCst) {
        return;
    }
    let g = cargar();
    if g.programadas.is_empty() {
        return;
    }
    let Some(v) = crate::servidor_v2::cargar() else { return };
    let ahora = chrono::Local::now();
    let Some((repo, p)) = g.programadas.iter().find_map(|(repo, l)| {
        let x = l.iter().find(|x| toca(x, ahora))?;
        v.repos_v2.iter().any(|r| &r.id == repo).then(|| (repo.clone(), x.prueba.clone()))
    }) else {
        return;
    };
    let (r2, id) = (repo.clone(), p.id.clone());
    let _ = cambiar(|g| {
        if let Some(x) = g.programadas.get_mut(&r2).and_then(|l| l.iter_mut().find(|x| x.prueba.id == id)) {
            x.desde = ahora.to_rfc3339();
        }
        Ok(())
    });
    let _ = lanzar(v, repo, p);
}

/// La próxima de una programada.
fn proxima(p: &Programada) -> Option<String> {
    let d = p.prueba.cada_dias?;
    let t = chrono::DateTime::parse_from_rfc3339(&p.desde).ok()?;
    Some((t + chrono::Duration::days(i64::from(d))).to_rfc3339())
}

/// De dónde, en una palabra (sin rutas).
fn forma(p: &Prueba) -> &'static str {
    if p.trabajo.is_some() {
        "espejo_equipo"
    } else if p.nube.is_some() {
        "nube"
    } else {
        "carpeta"
    }
}

/// `repos[].pruebas_espejo` del informe: las programadas y las hechas a mano (sin rutas).
pub fn informe(repo: &str) -> Vec<Value> {
    let g = cargar();
    let prefijo = format!("{repo}|");
    let mut out: Vec<Value> = Vec::new();
    let ultima = |id: &str| g.ultimas.get(&clave(repo, id));
    let datos = |x: &mut Value, u: &Ultima| {
        x["ultima"] = json!(u.fin);
        x["inicio"] = json!(u.inicio);
        x["resultado"] = json!(u.resultado);
        x["mensaje_corto"] = json!(u.mensaje.chars().take(300).collect::<String>());
        x["archivos"] = json!(u.archivos);
        x["bytes"] = json!(u.bytes);
        x["version"] = json!(u.version);
    };
    for p in g.programadas.get(repo).into_iter().flatten() {
        let mut x = json!({ "id": p.prueba.id, "nombre": p.prueba.nombre, "destino": p.prueba.destino, "forma": forma(&p.prueba), "trabajo": p.prueba.trabajo, "nube": p.prueba.nube, "cada_dias": p.prueba.cada_dias, "proxima": proxima(p) });
        if let Some(u) = ultima(&p.prueba.id) {
            datos(&mut x, u);
        }
        out.push(x);
    }
    for (k, u) in g.ultimas.iter().filter(|(k, _)| k.starts_with(&prefijo)) {
        let id = &k[prefijo.len()..];
        if out.iter().any(|x| x["id"] == id) {
            continue;
        }
        let mut x = json!({ "id": id, "nombre": u.nombre, "destino": u.destino });
        datos(&mut x, u);
        out.push(x);
    }
    out.truncate(16);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(v: Value) -> Result<Prueba, String> {
        let mut p: Prueba = serde_json::from_value(v).map_err(|e| e.to_string())?;
        validar(&mut p).map(|_| p)
    }

    #[test]
    fn valida_desde_donde() {
        assert!(p(json!({ "id": "eq-1", "nombre": "Disco E", "trabajo": "t1" })).is_ok());
        let n = p(json!({ "id": "n-1", "nombre": "Nube Sur", "nube": "Nube Sur", "carpeta": "/Resguardo/pc-01/docs/" })).unwrap();
        assert_eq!(n.carpeta.as_deref(), Some("Resguardo/pc-01/docs"));
        let carpeta = if cfg!(windows) { r"E:\Espejo\pc-01\docs" } else { "/srv/espejo/pc-01/docs" };
        assert!(p(json!({ "id": "c-1", "nombre": "Disco", "carpeta": carpeta })).is_ok());
        for mal in [
            json!({ "id": "Mayus", "nombre": "x", "trabajo": "t1" }),
            json!({ "id": "x", "nombre": "x" }),
            json!({ "id": "x", "nombre": "x", "trabajo": "t1", "nube": "n", "carpeta": "a" }),
            json!({ "id": "x", "nombre": "x", "trabajo": "t/1" }),
            json!({ "id": "x", "nombre": "x", "nube": "n", "carpeta": "../fuera" }),
            json!({ "id": "x", "nombre": "x", "carpeta": "relativa" }),
            json!({ "id": "x", "nombre": "x", "carpeta": "\\\\servidor\\recurso" }),
            json!({ "id": "x", "nombre": "a\u{1}b", "trabajo": "t1" }),
            json!({ "id": "x", "nombre": "x", "trabajo": "t1", "cada_dias": 0 }),
            json!({ "id": "x", "nombre": "x", "trabajo": "t1", "cada_dias": 93 }),
        ] {
            assert!(p(mal.clone()).is_err(), "{mal}");
        }
    }

    #[test]
    fn horario_de_las_programadas() {
        let ahora = chrono::Local::now();
        let x = Programada { prueba: Prueba { id: "a".into(), cada_dias: Some(30), ..Default::default() }, desde: desde_primera(30, ahora) };
        assert!(!toca(&x, ahora), "la primera, de madrugada");
        assert!(toca(&x, ahora + chrono::Duration::days(2)));
        let manual = Programada { prueba: Prueba { id: "b".into(), ..Default::default() }, desde: ahora.to_rfc3339() };
        assert!(!toca(&manual, ahora + chrono::Duration::days(400)), "sin `cada_dias`, nunca sola");
        assert!(proxima(&x).is_some());
    }

    /// Con restic de verdad (sin él, se omite): un repositorio, su «espejo» en otra carpeta
    /// (copia de sus archivos, como hace el espejo) y la prueba desde ahí; con la copia
    /// dañada, falla; y lo que va al informe no lleva la ruta.
    #[test]
    fn prueba_desde_una_carpeta_con_restic() {
        let _real = crate::restic::tests::real_repo_lock();
        if resguardo_motor::restic::version().is_err() {
            return;
        }
        let base = std::env::temp_dir().join(format!("resguardo-prueba-espejo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::env::set_var("RESGUARDO_AGENT_DIR", base.join("agente"));
        let datos = base.join("datos");
        std::fs::create_dir_all(&datos).unwrap();
        std::fs::write(datos.join("a.txt"), b"contenido de prueba").unwrap();
        std::fs::write(datos.join("b.bin"), vec![7u8; 300_000]).unwrap();
        let repo = base.join("repo");
        let acc = Access { location: repo.display().to_string(), password: "contraseña de prueba".into(), rest_auth: None, cacert: None, env: Vec::new() };
        let t = std::time::Duration::from_secs(120);
        assert_eq!(resguardo_motor::restic::run_raw(&acc, &["init"], t).unwrap().code, Some(0));
        assert_eq!(resguardo_motor::restic::run_raw(&acc, &["backup", &datos.display().to_string()], t).unwrap().code, Some(0));
        // El «espejo»: los mismos archivos en otra carpeta.
        let espejo = base.join("espejo").join("docs");
        copiar(&repo, &espejo);
        let mut acc_espejo = acc.clone();
        acc_espejo.location = carpeta_repo(&espejo).unwrap();
        let cfg = crate::restore_test::RestoreTest { schedule: crate::agent::Schedule::Hours { every: 24 }, files: 5, max_mb: 50, enabled_at: String::new() };
        let trabajo = base.join("privado");
        let r = crate::restore_test::run_en(&acc_espejo, &cfg, &trabajo, &mut |_, _| {});
        assert_eq!(r.result, "ok", "{}", r.message);
        assert!(r.files_new.unwrap_or(0) > 0);
        // Sin la copia (el disco no está): lo dice sin intentar nada.
        assert!(carpeta_repo(&base.join("no-esta")).unwrap_err().contains("¿está conectado"));
        // La copia dañada: cada paquete de datos cambiado. Falla.
        for e in walk(&espejo.join("data")) {
            std::fs::write(&e, b"danado").unwrap();
        }
        let r = crate::restore_test::run_en(&acc_espejo, &cfg, &trabajo, &mut |_, _| {});
        assert_eq!(r.result, "error", "{}", r.message);
        // Lo guardado para el informe no lleva rutas.
        let _g = cambiar(|g| {
            g.ultimas.insert(
                clave("docs", "c-1"),
                Ultima {
                    nombre: "Disco".into(),
                    resultado: "fallo".into(),
                    mensaje: crate::web::public_message(&r.message),
                    fin: "x".into(),
                    ..Default::default()
                },
            );
            Ok(())
        });
        let inf = informe("docs");
        assert_eq!(inf.len(), 1);
        assert!(!inf[0].to_string().contains(&base.display().to_string()), "{}", inf[0]);
        let _ = std::fs::remove_dir_all(&base);
    }

    fn copiar(de: &Path, a: &Path) {
        std::fs::create_dir_all(a).unwrap();
        for e in std::fs::read_dir(de).unwrap().flatten() {
            let p = a.join(e.file_name());
            if e.file_type().unwrap().is_dir() {
                copiar(&e.path(), &p);
            } else {
                std::fs::copy(e.path(), p).unwrap();
            }
        }
    }

    fn walk(d: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                out.extend(walk(&e.path()));
            } else {
                out.push(e.path());
            }
        }
        out
    }
}
