//! Bitácora del equipo (docs/api-servidor.md, §8, «Historial del equipo»):
//! lo que el agente cuenta a la consola, guardado también en el propio equipo
//! **para siempre**, para dárselo a una consola nueva (otro servidor, uno
//! restaurado o el mismo tras volver a vincular). Así la consola nueva ve la
//! Historia, los informes y los avisos de antes, no solo lo que pase desde ese día.
//!
//! - Una línea JSON por cosa que pasa: cada vuelta de una copia (con sus
//!   cifras y el resultado de sus ganchos), cada verificación, prueba de
//!   restauración o copia externa, cada vuelta del espejo, los avisos que
//!   levantó el equipo y (v1.45) cada vuelta de la retención con las versiones
//!   que quitó (`retencion_registro.rs`; con la lista, solo las 50 últimas).
//! - Solo lo que ya se enseña en la consola: sin rutas (los mensajes pasan por
//!   `web::public_message`), sin nombres de archivos y sin secretos.
//! - Un archivo por mes (`privado/bitacora/AAAA-MM.jsonl`), en el que solo se
//!   añade. Una vez al día se compacta (cada archivo se reescribe en uno
//!   temporal y se cambia de nombre: un corte a medias no rompe nada):
//!   - los últimos 12 meses, con todo el detalle;
//!   - lo anterior, cada vuelta de copia se resume en **una entrada por copia y
//!     día** (`resumen_dia`: correctas, fallidas, sin cambios, duración total,
//!     datos añadidos y el último error), que se guarda para siempre;
//!   - avisos, verificaciones, pruebas, copias externas y espejo se guardan
//!     para siempre (son pocos);
//!   - tope de seguridad de 50 MB: si se pasa, se resumen antes también meses
//!     más recientes (los más antiguos primero) y, solo si aun así no cabe, se
//!     quitan los meses más antiguos (y se anota en el registro).
//! - Cada entrada tiene su id (los resúmenes, uno fijo por copia y día): el
//!   servidor la guarda una sola vez, así que subirla dos veces no la repite.
//!
//! El servidor dice en `hola` (y en `tomar`) hasta dónde la tiene
//! (`historial.ultima`). Si no tiene nada (consola nueva), el equipo la sube
//! entera, **de lo más reciente a lo más antiguo** y en tandas; hasta terminar
//! una subida entera a ese servidor, la repite (sin duplicar nada). Después,
//! solo lo posterior. Un servidor anterior no dice nada y no se sube nada.

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Meses con todo el detalle; lo anterior se resume por copia y día.
pub const MESES_DETALLE: i64 = 12;
/// Tope de seguridad de toda la bitácora.
pub const MAX_BYTES: u64 = 50 * 1024 * 1024;
/// Entradas por petición al servidor (su límite).
pub const POR_SUBIDA: usize = 500;
/// Y como mucho tanto por petición (el límite del servidor es 1 MiB).
pub const BYTES_POR_SUBIDA: usize = 900 * 1024;
/// Lo que se vuelve a mirar antes de la última hora que tiene el servidor (relojes
/// y procesos que escriben a la vez); lo repetido no cuenta dos veces.
const MARGEN_MIN: i64 = 10;
const CARPETA: &str = "bitacora";
/// Meses ya resumidos y servidores a los que se subió entera.
const ESTADO: &str = "estado.json";

pub fn carpeta() -> PathBuf {
    crate::agent::private_dir().join(CARPETA)
}

#[derive(Serialize, Deserialize, Default)]
struct Estado {
    /// Meses («AAAA-MM») ya resumidos por día.
    #[serde(default)]
    resumidos: Vec<String>,
    /// Última compactación (día, «AAAA-MM-DD»).
    #[serde(default)]
    compactada: Option<String>,
    /// Servidores (identidad|equipo) a los que ya se subió entera.
    #[serde(default)]
    completas: Vec<String>,
}

fn leer_estado(dir: &Path) -> Estado {
    std::fs::read(dir.join(ESTADO)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn guardar_estado(dir: &Path, e: &Estado) {
    if let Ok(b) = serde_json::to_vec_pretty(e) {
        let _ = escribir_entero(&dir.join(ESTADO), &b);
    }
}

/// Escribe un archivo entero (temporal + cambio de nombre).
fn escribir_entero(path: &Path, contenido: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension(format!("tmp-{}", uuid::Uuid::new_v4().simple()));
    std::fs::write(&tmp, contenido)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

fn corto(m: &str) -> String {
    crate::web::public_message(m).chars().take(240).collect()
}

fn resultado(r: &str) -> &'static str {
    match r {
        "ok" => "ok",
        "warning" => "aviso",
        _ => "fallo",
    }
}

fn hora_de(v: &Value) -> Option<DateTime<Local>> {
    DateTime::parse_from_rfc3339(v.get("hora")?.as_str()?).ok().map(|t| t.with_timezone(&Local))
}

fn mes_de(t: DateTime<Local>) -> String {
    format!("{:04}-{:02}", t.year(), t.month())
}

/// Los archivos de la bitácora («AAAA-MM.jsonl»), del mes más antiguo al más reciente.
fn segmentos(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut v: Vec<(String, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            let mes = n.strip_suffix(".jsonl")?.to_string();
            (mes.len() == 7 && NaiveDate::parse_from_str(&format!("{mes}-01"), "%Y-%m-%d").is_ok()).then(|| (mes, e.path()))
        })
        .collect();
    v.sort();
    v
}

fn leer_segmento(path: &Path) -> Vec<Value> {
    let texto = std::fs::read(path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    texto.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).collect()
}

/// Añade una entrada a la bitácora de `dir` (con id y hora si no los trae). Nunca
/// falla la operación que la origina: si no se puede escribir, se pierde esa línea.
pub fn anotar_en(dir: &Path, tipo: &str, mut campos: Value) {
    let Some(obj) = campos.as_object_mut() else { return };
    obj.insert("tipo".into(), json!(tipo));
    obj.entry("id").or_insert_with(|| json!(uuid::Uuid::new_v4().to_string()));
    obj.entry("hora").or_insert_with(|| json!(Local::now().to_rfc3339()));
    // Lo que no aporta (vacío) no se guarda.
    obj.retain(|_, v| !v.is_null());
    let Some(hora) = hora_de(&campos) else { return };
    let Ok(linea) = serde_json::to_string(&campos) else { return };
    let _ = std::fs::create_dir_all(dir);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join(format!("{}.jsonl", mes_de(hora)))) {
        let _ = writeln!(f, "{linea}");
    }
}

/// Añade una entrada a la bitácora del agente (y la compacta si toca, una vez al día).
pub fn anotar(tipo: &str, campos: Value) {
    let dir = carpeta();
    anotar_en(&dir, tipo, campos);
    compactar_si_toca(&dir, Local::now());
}

/// Toda la bitácora, de lo más antiguo a lo más reciente.
pub fn leer_de(dir: &Path) -> Vec<Value> {
    let mut v: Vec<Value> = segmentos(dir).iter().flat_map(|(_, p)| leer_segmento(p)).collect();
    v.sort_by_key(|e| hora_de(e).map(|t| t.timestamp()).unwrap_or(0));
    v
}

pub fn leer() -> Vec<Value> {
    leer_de(&carpeta())
}

/// Resume las vueltas de copia de un mes: una entrada por copia y día. Lo demás queda igual.
pub fn resumir(entradas: Vec<Value>) -> Vec<Value> {
    let mut otras = Vec::new();
    // (día, repo, copia) → resumen
    let mut dias: BTreeMap<(String, String, String), Value> = BTreeMap::new();
    for e in entradas {
        let Some(hora) = hora_de(&e).filter(|_| e["tipo"] == "copia") else {
            otras.push(e);
            continue;
        };
        let (dia, repo, copia) =
            (hora.format("%Y-%m-%d").to_string(), e["repo"].as_str().unwrap_or("").to_string(), e["copia"].as_str().unwrap_or("").to_string());
        let r = dias.entry((dia.clone(), repo.clone(), copia.clone())).or_insert_with(|| {
            json!({
                "id": format!("dia-{repo}-{copia}-{dia}"), "tipo": "resumen_dia", "dia": dia, "hora": e["hora"], "repo": repo, "copia": copia,
                "ok": 0, "fallidas": 0, "sin_cambios": 0, "duracion_s": 0, "anadido": 0,
            })
        });
        let campo = match e["resultado"].as_str() {
            Some("fallo") => "fallidas",
            Some("sin_cambios") => "sin_cambios",
            _ => "ok",
        };
        r[campo] = json!(r[campo].as_u64().unwrap_or(0) + 1);
        r["duracion_s"] = json!(r["duracion_s"].as_u64().unwrap_or(0) + e["duracion_s"].as_u64().unwrap_or(0));
        r["anadido"] = json!(r["anadido"].as_u64().unwrap_or(0) + e["anadido"].as_u64().unwrap_or(0));
        if hora_de(r).is_none_or(|t| hora >= t) {
            r["hora"] = e["hora"].clone();
        }
        if campo == "fallidas" {
            if let Some(m) = e["mensaje"].as_str() {
                r["ultimo_error"] = json!(m);
            }
        }
    }
    otras.extend(dias.into_values());
    otras.sort_by_key(|e| hora_de(e).map(|t| t.timestamp()).unwrap_or(0));
    otras
}

fn reescribir(path: &Path, entradas: &[Value]) -> bool {
    let mut texto = String::new();
    for e in entradas {
        if let Ok(l) = serde_json::to_string(e) {
            texto.push_str(&l);
            texto.push('\n');
        }
    }
    escribir_entero(path, texto.as_bytes()).is_ok()
}

fn tamano(dir: &Path) -> u64 {
    segmentos(dir).iter().filter_map(|(_, p)| std::fs::metadata(p).ok()).map(|m| m.len()).sum()
}

/// Compacta la bitácora de `dir` (ver arriba). Solo toca meses cerrados (nunca el
/// actual, en el que se sigue escribiendo). Devuelve lo que hizo, para el registro.
pub fn compactar_en(dir: &Path, ahora: DateTime<Local>, max_bytes: u64) -> Vec<String> {
    let mut notas = Vec::new();
    let mut estado = leer_estado(dir);
    let actual = mes_de(ahora);
    let limite = mes_de(ahora - Duration::days(MESES_DETALLE * 31));
    fn resumir_mes(mes: &str, path: &Path, estado: &mut Estado) {
        if reescribir(path, &resumir(leer_segmento(path))) {
            estado.resumidos.push(mes.to_string());
        }
    }
    // 0. Las vueltas de la retención: la lista de versiones, solo en las más recientes.
    compactar_retenciones(dir, &actual);
    // 1. Más de 12 meses: una entrada por copia y día.
    for (mes, path) in segmentos(dir) {
        if mes < limite && !estado.resumidos.contains(&mes) {
            resumir_mes(&mes, &path, &mut estado);
        }
    }
    // 2. Por encima del tope: también los meses cerrados más recientes, de los más antiguos.
    for (mes, path) in segmentos(dir) {
        if tamano(dir) <= max_bytes {
            break;
        }
        if mes < actual && !estado.resumidos.contains(&mes) {
            resumir_mes(&mes, &path, &mut estado);
            notas.push(format!("Bitácora: más de {} MB; el mes {mes} queda resumido por día.", max_bytes / (1024 * 1024)));
        }
    }
    // 3. Si aun así no cabe: fuera los meses más antiguos (nunca el actual).
    for (mes, path) in segmentos(dir) {
        if tamano(dir) <= max_bytes || mes >= actual {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            estado.resumidos.retain(|m| *m != mes);
            notas.push(format!("Bitácora: más de {} MB aun resumida; se quitó el mes {mes}.", max_bytes / (1024 * 1024)));
        }
    }
    estado.compactada = Some(ahora.format("%Y-%m-%d").to_string());
    guardar_estado(dir, &estado);
    notas
}

/// Las vueltas de la retención con la lista de versiones quitadas: solo las
/// [`crate::retencion_registro::DETALLADAS`] más recientes; las anteriores se
/// quedan con sus cifras. Solo se reescriben meses cerrados (en el actual se
/// sigue escribiendo). Devuelve cuántas compactó.
pub fn compactar_retenciones(dir: &Path, actual: &str) -> usize {
    use crate::retencion_registro::{compactar, con_detalle, DETALLADAS};
    let segs = segmentos(dir);
    let mut con: Vec<(i64, String)> = segs
        .iter()
        .flat_map(|(_, p)| leer_segmento(p))
        .filter(con_detalle)
        .filter_map(|e| Some((hora_de(&e)?.timestamp(), e["id"].as_str()?.to_string())))
        .collect();
    if con.len() <= DETALLADAS {
        return 0;
    }
    con.sort_by(|a, b| b.cmp(a));
    let viejas: HashSet<String> = con.into_iter().skip(DETALLADAS).map(|(_, id)| id).collect();
    let mut n = 0;
    for (_, path) in segs.iter().filter(|(mes, _)| mes.as_str() < actual) {
        let mut l = leer_segmento(path);
        let mut cambia = 0;
        for e in l.iter_mut().filter(|e| con_detalle(e) && e["id"].as_str().is_some_and(|id| viejas.contains(id))) {
            compactar(e);
            cambia += 1;
        }
        if cambia > 0 && reescribir(path, &l) {
            n += cambia;
        }
    }
    n
}

/// Las tandas de una subida: como mucho [`POR_SUBIDA`] entradas y [`BYTES_POR_SUBIDA`]
/// (el servidor no acepta peticiones de más de 1 MiB; las vueltas de la retención
/// pueden ocupar hasta 96 KiB).
pub fn tandas(entradas: &[Value]) -> Vec<&[Value]> {
    let mut out = Vec::new();
    let (mut desde, mut bytes) = (0, 0);
    for (i, e) in entradas.iter().enumerate() {
        let b = e.to_string().len() + 1;
        if i > desde && (i - desde >= POR_SUBIDA || bytes + b > BYTES_POR_SUBIDA) {
            out.push(&entradas[desde..i]);
            (desde, bytes) = (i, 0);
        }
        bytes += b;
    }
    if desde < entradas.len() {
        out.push(&entradas[desde..]);
    }
    out
}

/// Compacta una vez al día (lo mira cada proceso que anota).
pub fn compactar_si_toca(dir: &Path, ahora: DateTime<Local>) {
    static HOY: Mutex<String> = Mutex::new(String::new());
    let hoy = ahora.format("%Y-%m-%d").to_string();
    {
        let mut h = HOY.lock().unwrap_or_else(|e| e.into_inner());
        if *h == hoy {
            return;
        }
        *h = hoy.clone();
    }
    if leer_estado(dir).compactada.as_deref() == Some(hoy.as_str()) {
        return;
    }
    for n in compactar_en(dir, ahora, MAX_BYTES) {
        crate::agent::log(&n);
    }
}

/// Lo que va a la bitácora desde una línea del historial del agente
/// (`history::append` lo llama con cada una): vueltas de las copias,
/// verificaciones, pruebas de restauración, copias externas y cambios
/// inusuales. El resto (pausas, configuración…) no sale del equipo.
pub fn desde_historial(e: &crate::history::Entry) {
    if let Some((tipo, campos)) = entrada_de_historial(e) {
        anotar(tipo, campos);
    }
}

pub fn entrada_de_historial(e: &crate::history::Entry) -> Option<(&'static str, Value)> {
    let comun = |tipo: &'static str| {
        (
            tipo,
            json!({ "hora": e.finished, "repo": e.repo_id, "resultado": resultado(&e.result), "mensaje": Some(corto(&e.message)).filter(|m| !m.is_empty()) }),
        )
    };
    Some(match e.kind.as_str() {
        "backup" => {
            let duracion =
                DateTime::parse_from_rfc3339(&e.started).ok().zip(DateTime::parse_from_rfc3339(&e.finished).ok()).map(|(a, b)| (b - a).num_seconds().max(0));
            let ganchos: Vec<Value> = e.ganchos.iter().map(|g| json!({ "tipo": g.tipo, "estado": g.estado, "mensaje": corto(&g.mensaje) })).collect();
            (
                "copia",
                json!({
                    "hora": e.finished,
                    "repo": e.repo_id,
                    "copia": e.plan_id,
                    "resultado": if e.unchanged { "sin_cambios" } else { resultado(&e.result) },
                    "mensaje": Some(corto(&e.message)).filter(|m| !m.is_empty()),
                    "duracion_s": duracion,
                    "anadido": e.data_added,
                    "archivos_nuevos": e.files_new,
                    "archivos_cambiados": e.files_changed,
                    "reintento": e.origin == "retry",
                    "ganchos": Some(ganchos).filter(|g| !g.is_empty()),
                }),
            )
        }
        "verify" => comun("verificacion"),
        "restore_test" => comun("prueba_restauracion"),
        "offsite" => comun("externa"),
        "guard" if e.result == "warning" => {
            ("aviso", json!({ "hora": e.finished, "repo": e.repo_id, "aviso": "cambio_inusual", "mensaje": corto(&e.message) }))
        }
        _ => return None,
    })
}

/// Una vuelta del espejo del Servidor de copias.
pub fn espejo(hora: &str, resultado_texto: &str) {
    let fallo = resultado_texto.starts_with("ERROR");
    anotar("espejo", json!({ "hora": hora, "resultado": if fallo { "fallo" } else { "ok" }, "mensaje": corto(resultado_texto) }));
}

/// Un aviso que el equipo manda al servidor (p. ej. «intentos_fallidos»).
pub fn aviso(tipo: &str, mensaje: &str) {
    anotar("aviso", json!({ "aviso": tipo, "mensaje": mensaje.chars().filter(|c| !c.is_control()).take(500).collect::<String>() }));
}

/// Lo que falta en el servidor, **de lo más reciente a lo más antiguo**: todo si
/// `ultima` es `None` (o si aún no se terminó una subida entera a ese servidor),
/// si no lo posterior a `ultima`; sin lo ya subido en este proceso.
pub fn pendientes(entradas: Vec<Value>, ultima: Option<&str>, ya: &HashSet<String>) -> Vec<Value> {
    let desde = ultima.and_then(|u| DateTime::parse_from_rfc3339(u).ok()).map(|t| t.with_timezone(&Local) - Duration::minutes(MARGEN_MIN));
    let mut v: Vec<Value> = entradas
        .into_iter()
        .filter(|v| desde.is_none_or(|d| hora_de(v).is_some_and(|t| t > d)))
        .filter(|v| v["id"].as_str().is_some_and(|id| !ya.contains(id)))
        .collect();
    v.sort_by_key(|e| std::cmp::Reverse(hora_de(e).map(|t| t.timestamp()).unwrap_or(0)));
    v
}

/// `historial.ultima` de un `hola` o de `tomar`: `None` si el servidor no lo
/// admite (anterior a v1.23); `Some(None)` si no tiene nada de este equipo.
pub fn ultima_del_servidor(m: &Value) -> Option<Option<String>> {
    let h = m.get("historial")?.as_object()?;
    Some(h.get("ultima").and_then(Value::as_str).map(str::to_string))
}

/// Lo que el proceso recuerda de las subidas a un servidor (identidad y equipo):
/// los ids ya subidos (para no repetir el margen en cada vuelta) y, si no faltaba
/// nada, cuántas entradas había y la `ultima` de entonces (mientras no cambien no
/// hace falta mirar más: el sondeo es cada minuto). Otro servidor, o uno que tiene
/// menos que antes (restaurado de una copia), empieza de cero.
#[derive(Default)]
struct Subidas {
    servidor: String,
    ultima: Option<i64>,
    ids: HashSet<String>,
    al_dia: Option<(u64, Option<String>)>,
}

/// Una por servidor (v1.35: con varias consolas, cada una lleva su cuenta).
static SUBIDAS: Mutex<Option<std::collections::HashMap<String, Subidas>>> = Mutex::new(None);

/// Sube lo que falte en el servidor (en tandas de 500, lo más reciente primero).
/// Devuelve la nueva `ultima` del servidor. Solo si el servidor lo admite (`ultima`
/// de su `hola`).
pub fn subir(v: &crate::servidor_v2::Vinculo, ultima: Option<&str>) -> Result<Option<String>, String> {
    subir_en(&carpeta(), v, ultima)
}

pub fn subir_en(dir: &Path, v: &crate::servidor_v2::Vinculo, ultima: Option<&str>) -> Result<Option<String>, String> {
    let servidor = format!("{}|{}", v.identidad, v.equipo_id);
    let clave_proceso = format!("{servidor}|{}", dir.display());
    let marca = ultima.and_then(|u| DateTime::parse_from_rfc3339(u).ok()).map(|t| t.timestamp());
    let huella = (tamano(dir), ultima.map(str::to_string));
    let mut estado = leer_estado(dir);
    // Un servidor sin nada de este equipo (consola nueva, o vaciada): otra subida entera.
    if ultima.is_none() && estado.completas.contains(&servidor) {
        estado.completas.retain(|s| *s != servidor);
        guardar_estado(dir, &estado);
    }
    let entera = !estado.completas.contains(&servidor);
    let ya = {
        let mut g = SUBIDAS.lock().unwrap_or_else(|e| e.into_inner());
        let s = g.get_or_insert_with(Default::default).entry(clave_proceso.clone()).or_default();
        if s.servidor != clave_proceso || marca < s.ultima {
            *s = Subidas { servidor: clave_proceso.clone(), ..Default::default() };
        }
        s.ultima = marca;
        if !entera && s.al_dia.as_ref() == Some(&huella) {
            return Ok(huella.1);
        }
        if entera {
            HashSet::new()
        } else {
            s.ids.clone()
        }
    };
    let falta = pendientes(leer_de(dir), if entera { None } else { ultima }, &ya);
    let mut nueva = ultima.map(str::to_string);
    for tanda in tandas(&falta) {
        let (codigo, r) = crate::servidor_v2::llamar(v, "POST", "/api/agente/historial", Some(&json!({ "entradas": tanda })))?;
        if !(200..300).contains(&codigo) {
            return Err(format!("El servidor no aceptó el historial ({codigo})."));
        }
        if let Some(u) = r["ultima"].as_str() {
            nueva = Some(u.to_string());
        }
        let mut g = SUBIDAS.lock().unwrap_or_else(|e| e.into_inner());
        let s = g.get_or_insert_with(Default::default).entry(clave_proceso.clone()).or_default();
        if s.servidor == clave_proceso {
            if s.ids.len() > 100_000 {
                s.ids.clear();
            }
            s.ids.extend(tanda.iter().filter_map(|e| e["id"].as_str().map(str::to_string)));
            s.ultima = nueva.as_deref().and_then(|u| DateTime::parse_from_rfc3339(u).ok()).map(|t| t.timestamp()).or(s.ultima);
        }
    }
    if entera {
        // Terminada: desde ahora, solo lo posterior a lo que tenga.
        let mut estado = leer_estado(dir);
        if !estado.completas.contains(&servidor) {
            estado.completas.push(servidor.clone());
            guardar_estado(dir, &estado);
        }
    }
    if falta.is_empty() {
        let mut g = SUBIDAS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = g.as_mut().and_then(|m| m.get_mut(&clave_proceso)).filter(|s| s.servidor == clave_proceso) {
            s.al_dia = Some((tamano(dir), nueva.clone()));
        }
    } else {
        crate::agent::log(&format!("Historial del equipo: {} entradas enviadas al servidor.", falta.len()));
    }
    Ok(nueva)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> PathBuf {
        std::env::temp_dir().join(format!("bitacora-{}", uuid::Uuid::new_v4()))
    }

    fn entrada(kind: &str, result: &str, hace: Duration) -> crate::history::Entry {
        let fin = Local::now() - hace;
        crate::history::Entry {
            kind: kind.into(),
            origin: "agent".into(),
            repo_id: "r1".into(),
            plan_id: Some("docs".into()),
            started: (fin - Duration::seconds(90)).to_rfc3339(),
            finished: fin.to_rfc3339(),
            result: result.into(),
            message: r"No se pudo leer C:\Users\Ana\secreto.txt".into(),
            data_added: Some(1234),
            ganchos: vec![crate::ganchos::ResultadoGancho { tipo: "sqlserver".into(), estado: "ok".into(), mensaje: r"Volcado en D:\Volcados\x.bak".into() }],
            ..Default::default()
        }
    }

    #[test]
    fn guarda_lo_que_cuenta_y_sin_rutas() {
        let d = dir();
        for (k, r) in
            [("backup", "error"), ("verify", "ok"), ("restore_test", "warning"), ("offsite", "ok"), ("guard", "warning"), ("pause", "info"), ("config", "info")]
        {
            if let Some((tipo, campos)) = entrada_de_historial(&entrada(k, r, Duration::hours(1))) {
                anotar_en(&d, tipo, campos);
            }
        }
        let l = leer_de(&d);
        let tipos: Vec<&str> = l.iter().map(|v| v["tipo"].as_str().unwrap()).collect();
        assert_eq!(tipos, ["copia", "verificacion", "prueba_restauracion", "externa", "aviso"], "pausas y configuración no salen del equipo");
        assert_eq!((l[0]["resultado"].as_str(), l[0]["duracion_s"].as_i64(), l[0]["anadido"].as_u64()), (Some("fallo"), Some(90), Some(1234)));
        assert_eq!(l[0]["ganchos"][0]["tipo"], "sqlserver");
        assert_eq!(l[2]["resultado"], "aviso");
        assert_eq!(l[4]["aviso"], "cambio_inusual");
        let texto: String = segmentos(&d).iter().map(|(_, p)| std::fs::read_to_string(p).unwrap()).collect();
        assert!(!texto.contains("Users") && !texto.contains("Volcados"), "sin rutas: {texto}");
        let ids: HashSet<&str> = l.iter().map(|v| v["id"].as_str().unwrap()).collect();
        assert_eq!(ids.len(), 5, "cada una con su id");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn para_siempre_con_lo_antiguo_resumido_por_dia() {
        let d = dir();
        // Hora fija (mediodía): las tres vueltas de 1 a 3 h antes caen el mismo día
        // pase lo que pase con el reloj (con «ahora» de verdad, entre las 00:00 y las
        // 03:00 se partían en dos días y el resumen salía doble).
        let ahora = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap().and_hms_opt(12, 0, 0).unwrap().and_local_timezone(Local).single().unwrap();
        let hace = |dias: i64, horas: i64| (ahora - Duration::days(dias) - Duration::hours(horas)).to_rfc3339();
        // Hace dos años: tres vueltas de la misma copia el mismo día, una fallida, más un aviso y una verificación.
        for (h, res, dur, anadido) in [(1, "ok", 60, 100), (2, "fallo", 5, 0), (3, "sin_cambios", 10, 0)] {
            anotar_en(
                &d,
                "copia",
                json!({ "hora": hace(730, h), "repo": "r1", "copia": "docs", "resultado": res, "duracion_s": dur, "anadido": anadido, "mensaje": if res == "fallo" { "Disco lleno" } else { "" } }),
            );
        }
        anotar_en(&d, "aviso", json!({ "hora": hace(729, 0), "aviso": "intentos_fallidos", "mensaje": "5 intentos" }));
        anotar_en(&d, "verificacion", json!({ "hora": hace(728, 0), "repo": "r1", "resultado": "ok" }));
        // Hace dos meses: con detalle.
        anotar_en(&d, "copia", json!({ "hora": hace(60, 0), "repo": "r1", "copia": "docs", "resultado": "ok", "duracion_s": 30 }));
        assert!(compactar_en(&d, ahora, MAX_BYTES).is_empty());
        let l = leer_de(&d);
        let tipos: Vec<&str> = l.iter().map(|v| v["tipo"].as_str().unwrap()).collect();
        assert_eq!(tipos, ["resumen_dia", "aviso", "verificacion", "copia"], "{l:?}");
        let r = &l[0];
        assert_eq!((r["ok"].as_u64(), r["fallidas"].as_u64(), r["sin_cambios"].as_u64()), (Some(1), Some(1), Some(1)));
        assert_eq!((r["duracion_s"].as_u64(), r["anadido"].as_u64(), r["ultimo_error"].as_str()), (Some(75), Some(100), Some("Disco lleno")));
        assert!(r["id"].as_str().unwrap().starts_with("dia-r1-docs-"), "id fijo: resumirlo otra vez no lo duplica en el servidor");
        // Otra vez: nada cambia (ya resumido).
        compactar_en(&d, ahora, MAX_BYTES);
        assert_eq!(leer_de(&d), l);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn tope_de_seguridad() {
        let d = dir();
        let ahora = Local::now();
        let relleno = "x".repeat(1000);
        // 6 meses cerrados con detalle (~200 KB cada uno) y el actual.
        for m in 1..=6 {
            for i in 0..200 {
                anotar_en(
                    &d,
                    "copia",
                    json!({ "hora": (ahora - Duration::days(31 * m) - Duration::minutes(i)).to_rfc3339(), "repo": "r1", "copia": "docs", "resultado": "ok", "mensaje": relleno }),
                );
            }
        }
        anotar_en(&d, "copia", json!({ "repo": "r1", "copia": "docs", "resultado": "ok", "mensaje": relleno }));
        let antes = tamano(&d);
        // Con un tope de 600 KB: se resumen los meses más antiguos primero, sin quitar nada.
        let notas = compactar_en(&d, ahora, 600 * 1024);
        assert!(tamano(&d) <= 600 * 1024 && tamano(&d) < antes, "{notas:?}");
        assert!(notas.iter().all(|n| n.contains("resumido")), "{notas:?}");
        let l = leer_de(&d);
        assert!(l.iter().any(|v| v["tipo"] == "resumen_dia") && l.iter().any(|v| v["tipo"] == "copia"));
        assert_eq!(l.last().unwrap()["tipo"], "copia", "lo más reciente sigue con detalle");
        // Con un tope ridículo: fuera los meses más antiguos (y se dice), nunca el actual.
        let notas = compactar_en(&d, ahora, 1);
        assert!(notas.iter().any(|n| n.contains("se quitó")), "{notas:?}");
        assert_eq!(segmentos(&d).last().unwrap().0, mes_de(ahora));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn retenciones_con_detalle_solo_las_ultimas() {
        let d = dir();
        let ahora = NaiveDate::from_ymd_opt(2026, 10, 5).unwrap().and_hms_opt(12, 0, 0).unwrap().and_local_timezone(Local).single().unwrap();
        // 60 vueltas de la retención en meses cerrados (una cada 3 días hacia atrás) y 2 este mes.
        for i in 0..62i64 {
            let hora = if i < 2 { ahora - Duration::hours(i + 1) } else { ahora - Duration::days(3 * i + 5) };
            anotar_en(
                &d,
                "retencion",
                json!({ "id": format!("ret-{i:03}"), "hora": hora.to_rfc3339(), "origen": "equipo", "repo": "r1", "quitadas": 2, "versiones": [["a1b2c3d4", 1, 0, null, 0]], "grupos": [{ "copia": "docs" }], "motivos": ["cupo:diarias"] }),
            );
        }
        assert!(compactar_en(&d, ahora, MAX_BYTES).is_empty());
        let l = leer_de(&d);
        let con: Vec<&str> = l.iter().filter(|e| crate::retencion_registro::con_detalle(e)).map(|e| e["id"].as_str().unwrap()).collect();
        assert_eq!(con.len(), crate::retencion_registro::DETALLADAS);
        assert!(con.contains(&"ret-000") && con.contains(&"ret-049") && !con.contains(&"ret-050"), "{con:?}");
        let vieja = l.iter().find(|e| e["id"] == "ret-061").unwrap();
        assert_eq!((vieja["compactada"].as_bool(), vieja["quitadas"].as_u64(), vieja.get("versiones")), (Some(true), Some(2), None), "las cifras se quedan");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn tandas_por_numero_y_por_tamano() {
        let pequena = json!({ "id": "x", "tipo": "copia" });
        let grande = json!({ "id": "y", "tipo": "retencion", "relleno": "z".repeat(90 * 1024) });
        let v: Vec<Value> = (0..1200).map(|_| pequena.clone()).collect();
        assert_eq!(tandas(&v).iter().map(|t| t.len()).collect::<Vec<_>>(), [500, 500, 200]);
        let v: Vec<Value> = (0..25).map(|_| grande.clone()).collect();
        let t = tandas(&v);
        assert!(t.iter().all(|t| t.iter().map(|e| e.to_string().len()).sum::<usize>() <= BYTES_POR_SUBIDA), "{}", t.len());
        assert_eq!(t.iter().map(|t| t.len()).sum::<usize>(), 25);
        assert!(tandas(&[]).is_empty());
    }

    #[test]
    fn solo_lo_que_falta_y_lo_reciente_primero() {
        let h = |min: i64, id: &str| json!({ "id": id, "hora": (Local::now() - Duration::minutes(min)).to_rfc3339(), "tipo": "copia" });
        let todas = vec![h(600, "a"), h(120, "b"), h(5, "c")];
        let nada = HashSet::new();
        // Un servidor sin nada de este equipo (consola nueva): todo, lo más reciente primero.
        let p = pendientes(todas.clone(), None, &nada);
        assert_eq!(p.iter().map(|v| v["id"].as_str().unwrap()).collect::<Vec<_>>(), ["c", "b", "a"]);
        // Tiene hasta hace una hora: lo posterior (con margen).
        let ultima = (Local::now() - Duration::minutes(60)).to_rfc3339();
        let p = pendientes(todas.clone(), Some(&ultima), &nada);
        assert_eq!(p.iter().map(|v| v["id"].as_str().unwrap()).collect::<Vec<_>>(), ["c"]);
        // Lo ya subido en este proceso no se repite.
        assert!(pendientes(todas, Some(&ultima), &HashSet::from(["c".to_string()])).is_empty());
        // Qué dice el servidor.
        assert_eq!(ultima_del_servidor(&json!({ "t": "hola" })), None, "servidor anterior: no se sube nada");
        assert_eq!(ultima_del_servidor(&json!({ "historial": { "ultima": null } })), Some(None));
        assert_eq!(ultima_del_servidor(&json!({ "historial": { "ultima": "x" } })), Some(Some("x".into())));
    }
}
