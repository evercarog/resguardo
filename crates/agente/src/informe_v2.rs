//! Agente v2: el informe detallado de cada repositorio para la consola
//! (docs/api-servidor.md, §6 «Informe del agente», `repos[]`), para que pueda
//! mostrar lo mismo que la app de escritorio: la lista de versiones, las
//! ejecuciones de los últimos 60 días, el espacio, la verificación, la prueba
//! de restauración y la salud de la protección.
//!
//! Solo metadatos: nada de rutas ni nombres de archivos (los mensajes pasan
//! por `web::public_message`, que quita las rutas).
//!
//! Las versiones y el espacio salen de restic, que puede tardar: se guardan
//! en `informe-cache.json` y se refrescan en otro hilo (las versiones cuando
//! termina una copia o cada 6 h; el espacio una vez al día). El informe usa
//! siempre lo que haya en caché, así que nunca espera a restic. Dos
//! excepciones (v1.30): al terminar una copia, el canal relee sus versiones
//! antes del informe inmediato (`refrescar_tras_copias`, con un tope), y la
//! pista `refrescar` del servidor (el almacén aplicó la retención) las relee
//! en otro hilo (`pista_refrescar`).

use crate::gestion_v2::{self as g, RepoV2};
use crate::servidor_v2::Vinculo;
use chrono::{DateTime, Duration, Local};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// Días que se cuentan hacia atrás.
pub const DIAS: i64 = 60;
pub const MAX_VERSIONES: usize = 500;
pub const MAX_EJECUCIONES: usize = 400;
/// El servidor acepta informes de hasta 256 KiB: se deja margen.
pub const MAX_INFORME_BYTES: usize = 200 * 1024;
const ARCHIVO: &str = "informe-cache.json";

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
struct CacheRepo {
    /// Cuándo se leyeron las versiones (RFC 3339) y con qué última copia.
    #[serde(default)]
    versiones_leidas: Option<String>,
    #[serde(default)]
    huella: String,
    #[serde(default)]
    versiones: Vec<Value>,
    #[serde(default)]
    espacio: Option<Value>,
    #[serde(default)]
    espacio_leido: Option<String>,
    /// Cuántas versiones tiene en total (no solo las de 60 días).
    #[serde(default)]
    total_versiones: Option<usize>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
struct Cache {
    #[serde(default)]
    repos: HashMap<String, CacheRepo>,
}

fn reciente(t: Option<&str>, max: Duration, ahora: DateTime<Local>) -> bool {
    t.and_then(|t| DateTime::parse_from_rfc3339(t).ok()).is_some_and(|t| ahora.signed_duration_since(t.with_timezone(&Local)) < max)
}

fn corto(m: &str) -> String {
    crate::web::public_message(m).chars().take(160).collect()
}

/// «ok», «aviso» o «fallo» de un resultado del agente.
fn resultado(r: &str) -> &'static str {
    match r {
        "ok" => "ok",
        "warning" => "aviso",
        _ => "fallo",
    }
}

/// Lo último que terminó en un repositorio (para saber si hay versiones nuevas).
fn huella(repo: &str) -> String {
    crate::agent::load_state().runs.iter().filter(|(k, _)| k.starts_with(&format!("{repo}#"))).map(|(k, r)| format!("{k}={};", r.finished)).collect()
}

/// Huella de lo que cambia el informe: las vueltas terminadas (y la que está
/// en marcha) y cuándo se releyeron las versiones. Si cambia, el canal manda
/// el informe enseguida y la consola ve terminar una copia sin esperar 5 min.
pub fn huella_informe() -> String {
    let st = crate::agent::load_state();
    let vueltas = vueltas_de(&st);
    let cache: Cache = crate::agent::read_json(ARCHIVO);
    let mut leidas: Vec<String> = cache.repos.iter().map(|(k, c)| format!("{k}={}", c.versiones_leidas.as_deref().unwrap_or(""))).collect();
    leidas.sort();
    format!("{vueltas}|{:?}|{}|{}", st.running.is_some(), leidas.join(";"), huella_otras())
}

fn vueltas_de(st: &crate::agent::AgentState) -> String {
    let mut vueltas: Vec<String> = st.runs.iter().map(|(k, r)| format!("{k}={}", r.finished)).collect();
    vueltas.sort();
    vueltas.join(";")
}

/// Solo las vueltas de copia terminadas: si cambia, acaba de terminar una copia.
pub fn huella_vueltas() -> String {
    vueltas_de(&crate::agent::load_state())
}

/// Lo demás que cuentan el informe y el resumen y no es una copia: lo que
/// terminó de las tareas (verificar, copia externa, prueba de restauración), la
/// retención del almacén (resultado, versiones, clave) y el espejo. Sin esto,
/// «Verificar ahora» o «aplicar la retención en el almacén» no llegaban a la
/// consola hasta el informe de cada 5 minutos.
fn huella_otras() -> String {
    let tareas = crate::tasks::load_state();
    let mut terminadas: Vec<String> = tareas.runs.iter().map(|(k, r)| format!("{k}={}", r.finished)).collect();
    terminadas.sort();
    let mut retenciones: Vec<String> = crate::retencion_almacen::cargar()
        .iter()
        .map(|e| format!("{}/{}={:?}{:?}{:?}{:?}", e.usuario, e.repo, e.ultima, e.resultado, e.versiones, e.clave_ok))
        .collect();
    retenciones.sort();
    let espejo = crate::server::load().espejo.map(|e| format!("{:?}{:?}", e.ultima, e.resultado)).unwrap_or_default();
    // v1.47: terminó una operación del servicio (traer el historial, aplicar la retención): su
    // entrada del historial del equipo va con el próximo informe, a todas las consolas, ya.
    let ops = crate::progreso_v2::ops::terminadas();
    // v1.4x: cambió la lista de órdenes en espera: el resumen va ya a todas las consolas.
    let espera = crate::espera_v2::cambios();
    format!("{}|{:?}|{}|{espejo}|{ops}|{espera}", terminadas.join(";"), tareas.running.is_some(), retenciones.join(";"))
}

/// Una versión de restic como la ve la consola (sin rutas).
pub fn version_de(s: &resguardo_motor::restic::Snapshot, copia: Option<&str>) -> Value {
    let sm = s.summary.as_ref();
    let duracion = sm.and_then(|m| {
        let (a, b) = (DateTime::parse_from_rfc3339(m.backup_start.as_deref()?).ok()?, DateTime::parse_from_rfc3339(m.backup_end.as_deref()?).ok()?);
        Some((b - a).num_seconds().max(0))
    });
    json!({
        "id": s.id.chars().take(8).collect::<String>(),
        "hora": s.time,
        "copia": copia,
        "total_bytes": sm.and_then(|m| m.total_bytes_processed),
        "anadido": sm.and_then(|m| m.data_added),
        "anadido_empaquetado": sm.and_then(|m| m.data_added_packed),
        "archivos_nuevos": sm.and_then(|m| m.files_new),
        "archivos_cambiados": sm.and_then(|m| m.files_changed),
        "archivos_sin_cambios": sm.and_then(|m| m.files_unmodified),
        "duracion_s": duracion,
        "etiquetas": s.tags,
    })
}

/// Copia (plan) de cada versión, por el historial del agente.
pub(crate) fn copias_por_version() -> HashMap<String, String> {
    crate::history::read(&crate::history::agent_file()).into_iter().filter(|e| e.kind == "backup").filter_map(|e| Some((e.snapshot_id?, e.plan_id?))).collect()
}

/// Versiones de los últimos 60 días, la más reciente primero (como mucho 500).
pub fn versiones(snaps: &[resguardo_motor::restic::Snapshot], copias: &HashMap<String, String>, ahora: DateTime<Local>) -> Vec<Value> {
    let desde = ahora - Duration::days(DIAS);
    let mut v: Vec<&resguardo_motor::restic::Snapshot> =
        snaps.iter().filter(|s| DateTime::parse_from_rfc3339(&s.time).is_ok_and(|t| t.with_timezone(&Local) >= desde)).collect();
    v.sort_by(|a, b| b.time.cmp(&a.time));
    v.into_iter().take(MAX_VERSIONES).map(|s| version_de(s, copias.get(&s.id).map(String::as_str))).collect()
}

/// Las versiones y el espacio de un repositorio se vuelven a leer en el
/// próximo informe (p. ej. tras traer el historial de otro repositorio, que
/// no es una copia y no cambia la huella).
pub fn invalidar(repo: &str) {
    let _candado = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let mut cache: Cache = crate::agent::read_json(ARCHIVO);
    if let Some(c) = cache.repos.get_mut(repo) {
        c.versiones_leidas = None;
        c.espacio_leido = None;
        let _ = crate::agent::write_json(ARCHIVO, &cache);
    }
}

static REFRESCANDO: AtomicBool = AtomicBool::new(false);
/// Leer, cambiar y escribir la caché, de uno en uno (el hilo de fondo, el
/// refresco tras una copia y el de una pista del servidor pueden coincidir).
static CACHE: Mutex<()> = Mutex::new(());

/// Vuelve a leer de restic lo que toque de un repositorio y lo guarda en la caché.
/// Las versiones, si `forzar` o si hay vueltas nuevas o tienen más de 6 h; el
/// espacio, si `espacio` y (`forzar` o tiene más de un día).
fn refrescar_repo(v: &Vinculo, r: &RepoV2, copias: &HashMap<String, String>, forzar: bool, espacio: bool) {
    let Ok(acc) = g::acceso(v, &r.id) else { return };
    let ahora = Local::now();
    let h = huella(&r.id);
    let (leer_versiones, leer_espacio) = {
        let cache: Cache = crate::agent::read_json(ARCHIVO);
        let c = cache.repos.get(&r.id);
        (
            forzar || c.is_none_or(|c| c.huella != h || !reciente(c.versiones_leidas.as_deref(), Duration::hours(6), ahora)),
            espacio && (forzar || c.is_none_or(|c| !reciente(c.espacio_leido.as_deref(), Duration::hours(24), ahora))),
        )
    };
    // restic, sin el candado (puede tardar).
    let snaps = if leer_versiones { resguardo_motor::restic::snapshots(&acc).ok() } else { None };
    let stats = leer_espacio.then(|| resguardo_motor::restic::stats(&acc));
    let _candado = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let mut cache: Cache = crate::agent::read_json(ARCHIVO);
    let c = cache.repos.entry(r.id.clone()).or_default();
    if let Some(s) = snaps {
        c.versiones = versiones(&s, copias, ahora);
        c.total_versiones = Some(s.len());
        c.versiones_leidas = Some(ahora.to_rfc3339());
        c.huella = h;
    }
    if let Some(st) = stats {
        if let Ok(st) = st {
            c.espacio = Some(json!({
                "en_disco_bytes": st.total_size, "sin_comprimir": st.total_uncompressed_size,
                "ratio": (st.compression_ratio > 0.0).then_some(st.compression_ratio), "leido": ahora.to_rfc3339(),
            }));
        }
        // Aunque falle, no se reintenta en cada informe: mañana otra vez.
        c.espacio_leido = Some(ahora.to_rfc3339());
    }
    let _ = crate::agent::write_json(ARCHIVO, &cache);
}

/// Refresca la caché (en otro hilo) si toca. Nunca bloquea al informe.
pub fn refrescar_si_toca(v: &Vinculo) {
    let ahora = Local::now();
    let cache: Cache = crate::agent::read_json(ARCHIVO);
    let pendientes: Vec<RepoV2> = v
        .repos_v2
        .iter()
        .filter(|r| {
            let c = cache.repos.get(&r.id);
            let versiones = c.is_none_or(|c| c.huella != huella(&r.id) || !reciente(c.versiones_leidas.as_deref(), Duration::hours(6), ahora));
            let espacio = c.is_none_or(|c| !reciente(c.espacio_leido.as_deref(), Duration::hours(24), ahora));
            versiones || espacio
        })
        .cloned()
        .collect();
    if pendientes.is_empty() || REFRESCANDO.swap(true, Ordering::SeqCst) {
        return;
    }
    let v = v.clone();
    std::thread::spawn(move || {
        let copias = copias_por_version();
        for r in pendientes {
            refrescar_repo(&v, &r, &copias, false, true);
        }
        REFRESCANDO.store(false, Ordering::SeqCst);
    });
}

/// Repositorios con vueltas terminadas que la caché aún no cuenta (acaba de terminar una copia).
fn con_vueltas_nuevas(v: &Vinculo) -> Vec<RepoV2> {
    let cache: Cache = crate::agent::read_json(ARCHIVO);
    v.repos_v2
        .iter()
        .filter(|r| {
            let h = huella(&r.id);
            !h.is_empty() && cache.repos.get(&r.id).is_none_or(|c| c.huella != h)
        })
        .cloned()
        .collect()
}

/// v1.30: al terminar una copia, sus versiones se releen ya (como mucho
/// `limite`) para que el informe que sale enseguida las lleve: antes llegaban
/// en el siguiente, 15–35 s después, y la consola enseñaba «0 versiones» con
/// la copia ya terminada. Devuelve si releyó algo.
pub fn refrescar_tras_copias(v: &Vinculo, limite: std::time::Duration) -> bool {
    let pendientes = con_vueltas_nuevas(v);
    if pendientes.is_empty() {
        return false;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let v = v.clone();
    std::thread::spawn(move || {
        let copias = copias_por_version();
        for r in pendientes {
            refrescar_repo(&v, &r, &copias, false, false);
        }
        let _ = tx.send(());
    });
    // Si restic tarda más, sigue en su hilo y llega en el informe siguiente.
    let _ = rx.recv_timeout(limite);
    true
}

/// Entre dos pistas del servidor para el mismo repositorio, como poco.
const ENTRE_PISTAS: std::time::Duration = std::time::Duration::from_secs(60);

/// v1.30: pista del servidor (`{ "t": "refrescar", "repo" }`): el almacén
/// acaba de aplicar la retención en ese repositorio, así que sus versiones y
/// su espacio cambiaron. Solo se vuelven a leer (nada se borra ni se cambia),
/// de un repositorio de este equipo y como mucho una vez por minuto: una pista
/// falsa o repetida no hace más que leer. Devuelve si la atendió.
pub fn pista_refrescar(v: &Vinculo, repo: &str) -> bool {
    static ULTIMAS: Mutex<Option<HashMap<String, std::time::Instant>>> = Mutex::new(None);
    let Some(r) = v.repos_v2.iter().find(|r| r.id == repo).cloned() else { return false };
    {
        let mut u = ULTIMAS.lock().unwrap_or_else(|e| e.into_inner());
        let u = u.get_or_insert_with(HashMap::new);
        let ahora = std::time::Instant::now();
        if u.get(repo).is_some_and(|t| ahora.duration_since(*t) < ENTRE_PISTAS) {
            return false;
        }
        u.insert(repo.to_string(), ahora);
    }
    let v = v.clone();
    std::thread::spawn(move || refrescar_repo(&v, &r, &copias_por_version(), true, true));
    true
}

/// Lo que el resumen en claro dice de un repositorio (de la caché, sin esperar
/// a restic): cuántas versiones tiene, lo que ocupa la última y cuándo fue.
pub fn resumen_repo(repo: &str) -> Value {
    let cache: Cache = crate::agent::read_json(ARCHIVO);
    let Some(c) = cache.repos.get(repo) else { return json!({}) };
    let ultima = c.versiones.first();
    json!({
        "versiones": c.total_versiones.or((!c.versiones.is_empty()).then_some(c.versiones.len())),
        "bytes": ultima.and_then(|v| v["total_bytes"].as_u64()),
        "ultima_version": ultima.and_then(|v| v["hora"].as_str()),
    })
}

/// Ejecuciones de las copias de un repositorio (últimos 60 días, la más reciente primero).
pub fn ejecuciones(historial: &[crate::history::Entry], repo: &str, ahora: DateTime<Local>) -> Vec<Value> {
    let desde = ahora - Duration::days(DIAS);
    let mut v: Vec<&crate::history::Entry> = historial
        .iter()
        .filter(|e| e.kind == "backup" && e.repo_id == repo)
        .filter(|e| DateTime::parse_from_rfc3339(&e.finished).is_ok_and(|t| t.with_timezone(&Local) >= desde))
        .collect();
    v.sort_by(|a, b| b.finished.cmp(&a.finished));
    v.into_iter()
        .take(MAX_EJECUCIONES)
        .map(|e| {
            // v1.12: cuánto duró y qué añadió cada vuelta (para el detalle de una copia).
            let duracion =
                DateTime::parse_from_rfc3339(&e.started).ok().zip(DateTime::parse_from_rfc3339(&e.finished).ok()).map(|(a, b)| (b - a).num_seconds().max(0));
            json!({
                "hora": e.finished,
                "copia": e.plan_id,
                "resultado": if e.unchanged { "sin_cambios" } else { resultado(&e.result) },
                "mensaje_corto": corto(&e.message),
                "duracion_s": duracion,
                "anadido": e.data_added,
                "archivos_nuevos": e.files_new,
                "archivos_cambiados": e.files_changed,
                "reintento": e.origin == "retry",
            })
        })
        .collect()
}

fn tarea(t: &crate::tasks::TasksState, kind: &str, repo: &str) -> Value {
    match t.runs.get(&crate::tasks::key(kind, repo)) {
        Some(r) => json!({ "ultima": r.finished, "resultado": resultado(&r.result), "mensaje_corto": corto(&r.message) }),
        None => Value::Null,
    }
}

fn estado(s: &crate::protection::State) -> &'static str {
    match s {
        crate::protection::State::Ok => "ok",
        crate::protection::State::Warn => "aviso",
        crate::protection::State::Bad => "fallo",
        crate::protection::State::Unknown => "desconocido",
    }
}

/// Salud de la protección, con las mismas reglas que la app de escritorio.
fn proteccion(
    v: &Vinculo,
    r: &RepoV2,
    cfg: &crate::agent::AgentConfig,
    st: &crate::agent::AgentState,
    t: &crate::tasks::TasksState,
    ahora: DateTime<Local>,
) -> Value {
    let location = g::acceso(v, &r.id).map(|a| a.location).unwrap_or_default();
    let mut f = crate::protection::agent_facts(&r.id, &location, cfg, st, t, &crate::tasks::load_guard(), ahora);
    // Gestionado: la consola muestra el kit al crear el repositorio y no deja
    // seguir sin confirmar que se guardó; la retención es la del servidor.
    f.kit_ok = true;
    f.kit_stale = false;
    f.has_retention = r.retencion.is_some();
    let p = crate::protection::evaluate(&f, ahora);
    json!({
        "puntuacion": p.score,
        "total": p.total,
        "items": p.items.iter().map(|i| json!({ "id": i.id, "estado": estado(&i.state), "etiqueta": i.label, "detalle": crate::web::public_message(&i.detail) })).collect::<Vec<_>>(),
    })
}

/// `repos[]` del informe (sin bloquear: versiones y espacio de la caché).
pub fn repos(v: &Vinculo) -> Vec<Value> {
    refrescar_si_toca(v);
    let ahora = Local::now();
    let cache: Cache = crate::agent::read_json(ARCHIVO);
    let historial = crate::history::read(&crate::history::agent_file());
    let (cfg, st, t) = (crate::agent::load_config(), crate::agent::load_state(), crate::tasks::load_state());
    v.repos_v2
        .iter()
        .map(|r| {
            let c = cache.repos.get(&r.id).cloned().unwrap_or_default();
            json!({
                "id": r.id,
                "nombre": r.nombre,
                "solo_lectura": r.solo_lectura,
                "versiones": c.versiones,
                "versiones_leidas": c.versiones_leidas,
                "ejecuciones": ejecuciones(&historial, &r.id, ahora),
                "espacio": c.espacio,
                "verificacion": tarea(&t, "verify", &r.id),
                "prueba_restauracion": tarea(&t, "restore_test", &r.id),
                "externa": tarea(&t, "offsite", &r.id),
                "proteccion": proteccion(v, r, &cfg, &st, &t, ahora),
            })
        })
        .collect()
}

/// Recorta el informe hasta que quepa: primero las ejecuciones más antiguas, luego las versiones.
pub fn acotar(informe: &mut Value) {
    for _ in 0..12 {
        if informe.to_string().len() <= MAX_INFORME_BYTES {
            return;
        }
        let Some(repos) = informe["repos"].as_array_mut() else { return };
        for r in repos.iter_mut() {
            for campo in ["ejecuciones", "versiones"] {
                if let Some(l) = r[campo].as_array_mut() {
                    let n = l.len() / 2;
                    l.truncate(n);
                }
            }
            r["recortado"] = json!(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(id: &str, hace_dias: i64) -> resguardo_motor::restic::Snapshot {
        serde_json::from_value(json!({
            "id": id, "short_id": &id[..8], "time": (Local::now() - Duration::days(hace_dias)).to_rfc3339(), "hostname": "h",
            "paths": ["C:\\Users\\Ana\\Documentos"], "tags": ["resguardo-agente"],
            "summary": { "backup_start": "2026-10-02T10:00:00-05:00", "backup_end": "2026-10-02T10:01:30-05:00",
                         "files_new": 2, "files_changed": 1, "files_unmodified": 40, "data_added": 1000, "data_added_packed": 600, "total_bytes_processed": 50000 }
        }))
        .unwrap()
    }

    #[test]
    fn la_huella_cambia_con_tareas_y_retencion_del_almacen() {
        // RESGUARDO_AGENT_DIR es de todo el proceso: como las demás pruebas que lo cambian, con el candado.
        let _real = crate::restic::tests::real_repo_lock();
        let dir = std::env::temp_dir().join(format!("resguardo-huella-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("privado")).unwrap();
        std::env::set_var("RESGUARDO_AGENT_DIR", &dir);
        let h0 = huella_informe();
        assert_eq!(huella_informe(), h0, "sin cambios, la misma");
        let verificada =
            |cuando: &str| json!({ "runs": { "verify:r1": { "started": cuando, "finished": cuando, "result": "ok", "message": "" } } }).to_string();
        std::fs::write(dir.join("tasks.json"), verificada("2026-10-04T10:00:00-05:00")).unwrap();
        let h1 = huella_informe();
        assert_ne!(h1, h0, "terminó una verificación («Verificar ahora»)");
        std::fs::write(dir.join("tasks.json"), verificada("2026-10-04T11:00:00-05:00")).unwrap();
        let h2 = huella_informe();
        assert_ne!(h2, h1, "y otra");
        let mut e: crate::retencion_almacen::Entrada = serde_json::from_value(json!({
            "usuario": "ana", "repo": "siigo", "clave": "x".repeat(32), "retencion": { "diarias": 7 },
            "horario": { "dias": [7], "hora": "03:00" }, "desde": "2026-10-01T00:00:00-05:00"
        }))
        .unwrap();
        crate::retencion_almacen::escribir(std::slice::from_ref(&e)).unwrap();
        let h3 = huella_informe();
        assert_ne!(h3, h2, "una regla nueva en el almacén");
        e.ultima = Some("2026-10-04T12:00:00-05:00".into());
        e.resultado = Some("ok".into());
        e.versiones = Some(4);
        crate::retencion_almacen::escribir(&[e]).unwrap();
        assert_ne!(huella_informe(), h3, "el almacén aplicó la retención");
        std::env::remove_var("RESGUARDO_AGENT_DIR");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn una_pista_de_un_repositorio_ajeno_no_hace_nada() {
        assert!(!pista_refrescar(&Vinculo::default(), "no-es-mio"));
    }

    #[test]
    fn versiones_sin_rutas_y_de_60_dias() {
        let snaps = vec![snap("aaaaaaaa11111111", 1), snap("bbbbbbbb22222222", 100), snap("cccccccc33333333", 0)];
        let copias = HashMap::from([("aaaaaaaa11111111".to_string(), "docs".to_string())]);
        let v = versiones(&snaps, &copias, Local::now());
        assert_eq!(v.len(), 2, "la de hace 100 días no entra");
        assert_eq!(v[0]["id"], "cccccccc", "la más reciente primero");
        assert_eq!(v[1]["copia"], "docs");
        assert_eq!((v[1]["anadido"].as_u64(), v[1]["archivos_sin_cambios"].as_u64(), v[1]["duracion_s"].as_i64()), (Some(1000), Some(40), Some(90)));
        assert!(!serde_json::to_string(&v).unwrap().contains("Users"), "sin rutas");
    }

    #[test]
    fn ejecuciones_con_sin_cambios_y_fallos() {
        let e = |fin: DateTime<Local>, result: &str, unchanged: bool, msg: &str| crate::history::Entry {
            kind: "backup".into(),
            repo_id: "r1".into(),
            plan_id: Some("docs".into()),
            started: (fin - Duration::seconds(90)).to_rfc3339(),
            finished: fin.to_rfc3339(),
            data_added: Some(1234),
            result: result.into(),
            unchanged,
            message: msg.into(),
            ..Default::default()
        };
        let ahora = Local::now();
        let h = vec![
            e(ahora - Duration::hours(1), "ok", true, "Sin cambios"),
            e(ahora - Duration::hours(2), "error", false, r"No se pudo leer C:\Users\Ana\secreto.txt"),
            e(ahora - Duration::days(90), "ok", false, "vieja"),
        ];
        let v = ejecuciones(&h, "r1", ahora);
        assert_eq!(v.len(), 2);
        assert_eq!((v[0]["resultado"].as_str(), v[1]["resultado"].as_str()), (Some("sin_cambios"), Some("fallo")));
        assert!(!v[1]["mensaje_corto"].as_str().unwrap().contains("Users"), "sin rutas");
        assert_eq!((v[0]["duracion_s"].as_i64(), v[0]["anadido"].as_u64()), (Some(90), Some(1234)));
        assert_eq!(v[0]["reintento"], false);
    }

    #[test]
    fn el_informe_se_acota() {
        let grande: Vec<Value> =
            (0..2000).map(|i| json!({ "id": format!("{i:08}"), "hora": "2026-10-02T10:00:00-05:00", "relleno": "x".repeat(100) })).collect();
        let mut inf = json!({ "repos": [{ "versiones": grande.clone(), "ejecuciones": grande }] });
        acotar(&mut inf);
        assert!(inf.to_string().len() <= MAX_INFORME_BYTES);
        assert_eq!(inf["repos"][0]["recortado"], true);
    }
}
