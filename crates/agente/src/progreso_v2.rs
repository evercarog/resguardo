//! Agente v2: el progreso en vivo de lo que está en marcha (una copia, una
//! verificación, la copia externa…) para la consola (docs/api-servidor.md,
//! §8, mensaje `progreso`).
//!
//! Sale de lo que ya guardan el proceso de copias (`state.json`, cada 3 s) y
//! el de tareas (`tasks.json`). Solo cifras, la fase y los nombres que da la
//! propia consola: nunca rutas ni nombres de archivos (las etapas pasan por
//! `web::public_message`).

use crate::agent::RunningCopy;
use crate::servidor_v2::Vinculo;
use chrono::{DateTime, Local};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// Como mucho, tantas tareas por mensaje (el servidor tampoco admite más).
pub const MAX_TAREAS: usize = 8;
/// Cada cuánto se manda mientras hay algo en marcha, por el canal.
pub const CADA_CANAL: Duration = Duration::from_secs(5);
/// Y por HTTP, cuando el canal no pasa (sondeo).
pub const CADA_SONDEO: Duration = Duration::from_secs(10);

/// Minutos sin noticias tras los que una copia ya no cuenta como en marcha (un
/// proceso que se cortó deja su estado). Restic puede pasar un rato sin dar
/// cifras (esperando a que se libere el repositorio, `--retry-lock 30m`).
const COPIA_VIVA_MIN: i64 = 40;
/// Los ganchos «Antes de copiar» (p. ej. el volcado de una base de datos grande) no dan cifras.
const GANCHOS_VIVOS_MIN: i64 = 180;

fn minutos_desde(t: &str, ahora: DateTime<Local>) -> Option<i64> {
    DateTime::parse_from_rfc3339(t).ok().map(|t| ahora.signed_duration_since(t.with_timezone(&Local)).num_minutes())
}

/// La fase de una copia, según lo último que dijo restic:
/// - `antes_de_copiar`: los ganchos (volcados, comprobaciones);
/// - `preparando`: abre el repositorio, aún sin cifras;
/// - `escaneando`: restic todavía cuenta lo que hay (copia a la vez, pero sin «quedan»);
/// - `subiendo`: ya sabe el total y cuánto falta;
/// - `terminando`: guarda la versión.
pub fn fase_copia(r: &RunningCopy) -> &'static str {
    if r.phase.as_deref() == Some("hooks") {
        return "antes_de_copiar";
    }
    if r.percent.is_none() && r.total_files == 0 && r.files_done == 0 && r.bytes_done == 0 {
        return "preparando";
    }
    match r.percent {
        Some(p) if p >= 0.999 => "terminando",
        _ if r.seconds_remaining.is_none() => "escaneando",
        _ => "subiendo",
    }
}

/// Una copia en marcha como la ve la consola. `None` si su estado es viejo
/// (el proceso se cortó) o no está en la configuración.
pub fn tarea_copia(r: &RunningCopy, nombre: Option<&str>, ahora: DateTime<Local>) -> Option<Value> {
    let ultima = r.updated.as_deref().unwrap_or(&r.started);
    let max = if r.phase.as_deref() == Some("hooks") { GANCHOS_VIVOS_MIN } else { COPIA_VIVA_MIN };
    if minutos_desde(ultima, ahora).is_none_or(|m| m >= max) {
        return None;
    }
    let fase = fase_copia(r);
    let con_cifras = matches!(fase, "escaneando" | "subiendo" | "terminando");
    Some(json!({
        "tipo": "copia",
        "repo": r.repo_id,
        "copia": r.plan_id,
        "nombre": nombre,
        "fase": fase,
        "porcentaje": if con_cifras { r.percent.map(|p| (p.clamp(0.0, 1.0) * 1000.0).round() / 1000.0) } else { None },
        "archivos": con_cifras.then_some(r.files_done),
        "archivos_total": con_cifras.then_some(r.total_files),
        "bytes": con_cifras.then_some(r.bytes_done),
        "bytes_total": con_cifras.then_some(r.total_bytes),
        "velocidad": if fase == "subiendo" || fase == "escaneando" { r.bytes_per_s } else { None },
        "quedan_s": if fase == "subiendo" { r.seconds_remaining } else { None },
        "empezo": r.started,
        "actualizado": r.updated,
    }))
}

/// Tipo de una tarea de `tasks.rs` para la consola.
fn tipo_tarea(kind: &str) -> &'static str {
    match kind {
        "verify" => "verificar",
        "verify_offsite" => "verificar_externa",
        "restore_test" => "prueba_restauracion",
        _ => "copia_externa",
    }
}

/// Una tarea larga (verificar, copia externa, prueba de restauración).
pub fn tarea_larga(t: &crate::tasks::RunningTask) -> Value {
    let etapa: String = crate::web::public_message(&t.stage).chars().take(120).collect();
    let tipo = tipo_tarea(&t.kind);
    json!({
        "tipo": tipo,
        "repo": t.repo_id,
        "fase": if t.percent.is_none() && t.done == 0 { "preparando" } else { "en_marcha" },
        "etapa": (!etapa.is_empty()).then_some(etapa),
        "porcentaje": t.percent.map(|p| (p.clamp(0.0, 1.0) * 1000.0).round() / 1000.0),
        "bytes": t.bytes_done,
        "bytes_total": t.bytes_total,
        "versiones": (tipo == "copia_externa" && t.total.is_some()).then_some(t.done),
        "versiones_total": if tipo == "copia_externa" { t.total } else { None },
        "quedan_s": t.eta_s,
        "empezo": t.started,
        "actualizado": t.updated,
    })
}

/// Lo que está en marcha ahora mismo en este equipo (vacío si nada).
pub fn tareas(v: Option<&Vinculo>) -> Vec<Value> {
    let ahora = Local::now();
    let copias: Vec<crate::gestion_v2::Copia> = v
        .and_then(|v| v.config_v1.as_ref())
        .and_then(|c| serde_json::from_value::<crate::gestion_v2::Configuracion>(c.clone()).ok())
        .map(|c| c.copias)
        .unwrap_or_default();
    let mut out = Vec::new();
    if let Some(r) = crate::agent::load_state().running.as_ref() {
        let nombre = copias.iter().find(|k| k.repo == r.repo_id && Some(&k.id) == r.plan_id.as_ref()).map(|k| k.nombre.as_str());
        out.extend(tarea_copia(r, nombre, ahora));
    }
    if let Some(t) = crate::tasks::load_state().live_running() {
        out.push(tarea_larga(t));
    }
    out.truncate(MAX_TAREAS);
    out
}

/// Decide cuándo mandar el progreso: cada `cada` mientras hay algo en marcha
/// y una vez más (vacío) cuando termina, para que la consola lo quite enseguida.
#[derive(Default)]
pub struct Emisor {
    ultimo: Option<Instant>,
    activo: bool,
    /// El servidor no lo admite (anterior a v1.25): no se manda más.
    apagado: bool,
}

impl Emisor {
    pub fn desactivar(&mut self) {
        self.apagado = true;
    }

    /// Las tareas que toca mandar ahora, o `None` si aún no toca o no hay nada nuevo que decir.
    pub fn toca(&mut self, cada: Duration, ahora: Instant, leer: impl FnOnce() -> Vec<Value>) -> Option<Vec<Value>> {
        if self.apagado || self.ultimo.is_some_and(|u| ahora.saturating_duration_since(u) < cada) {
            return None;
        }
        self.ultimo = Some(ahora);
        let tareas = leer();
        let antes = std::mem::replace(&mut self.activo, !tareas.is_empty());
        (antes || self.activo).then_some(tareas)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn copia() -> RunningCopy {
        RunningCopy {
            repo_id: "r1".into(),
            plan_id: Some("k1".into()),
            started: Local::now().to_rfc3339(),
            percent: None,
            files_done: 0,
            total_files: 0,
            bytes_done: 0,
            total_bytes: 0,
            seconds_remaining: None,
            updated: None,
            phase: None,
            bytes_per_s: None,
        }
    }

    #[test]
    fn fases_de_una_copia() {
        let mut r = copia();
        assert_eq!(fase_copia(&r), "preparando");
        r.phase = Some("hooks".into());
        assert_eq!(fase_copia(&r), "antes_de_copiar");
        r.phase = None;
        r.percent = Some(0.1);
        r.total_files = 300;
        r.files_done = 10;
        r.bytes_done = 1000;
        assert_eq!(fase_copia(&r), "escaneando", "sin «quedan», restic aún cuenta");
        r.seconds_remaining = Some(120);
        assert_eq!(fase_copia(&r), "subiendo");
        r.percent = Some(1.0);
        assert_eq!(fase_copia(&r), "terminando");
    }

    #[test]
    fn tarea_de_copia_sin_rutas_y_solo_si_esta_viva() {
        let ahora = Local::now();
        let mut r = copia();
        r.percent = Some(0.4234);
        r.total_files = 300;
        r.files_done = 120;
        r.bytes_done = 4_000;
        r.total_bytes = 10_000;
        r.seconds_remaining = Some(90);
        r.bytes_per_s = Some(2_000);
        r.updated = Some(ahora.to_rfc3339());
        let t = tarea_copia(&r, Some("Documentos"), ahora).unwrap();
        assert_eq!(t["tipo"], "copia");
        assert_eq!(t["copia"], "k1");
        assert_eq!(t["nombre"], "Documentos");
        assert_eq!(t["fase"], "subiendo");
        assert_eq!(t["porcentaje"], 0.423);
        assert_eq!(t["archivos"], 120);
        assert_eq!(t["bytes_total"], 10_000);
        assert_eq!(t["velocidad"], 2_000);
        assert_eq!(t["quedan_s"], 90);
        // Un estado de hace una hora es de un proceso que se cortó.
        r.updated = Some((ahora - chrono::Duration::minutes(60)).to_rfc3339());
        assert!(tarea_copia(&r, None, ahora).is_none());
        // Mientras preparan, sin cifras.
        let p = tarea_copia(&copia(), None, ahora).unwrap();
        assert_eq!(p["fase"], "preparando");
        assert!(p["porcentaje"].is_null() && p["archivos"].is_null());
    }

    #[test]
    fn tarea_larga_sin_rutas() {
        let t = crate::tasks::RunningTask {
            repo_id: "r1".into(),
            kind: "offsite".into(),
            started: Local::now().to_rfc3339(),
            stage: r"Subiendo C:\Users\Ana\secreto.txt".into(),
            done: 2,
            total: Some(5),
            percent: Some(0.5),
            ..Default::default()
        };
        let v = tarea_larga(&t);
        assert_eq!(v["tipo"], "copia_externa");
        assert_eq!(v["fase"], "en_marcha");
        assert_eq!(v["versiones"], 2);
        assert_eq!(v["versiones_total"], 5);
        assert!(!v["etapa"].as_str().unwrap().contains("Ana"), "sin rutas: {}", v["etapa"]);
    }

    #[test]
    fn emisor_cada_cierto_tiempo_y_uno_vacio_al_terminar() {
        let mut e = Emisor::default();
        let t0 = Instant::now();
        let cada = Duration::from_secs(5);
        let una = || vec![json!({ "tipo": "copia" })];
        assert_eq!(e.toca(cada, t0, Vec::new), None, "nada en marcha: nada que mandar");
        assert_eq!(e.toca(cada, t0 + Duration::from_secs(1), una), None, "aún no toca");
        assert_eq!(e.toca(cada, t0 + Duration::from_secs(5), una).map(|x| x.len()), Some(1));
        assert_eq!(e.toca(cada, t0 + Duration::from_secs(10), una).map(|x| x.len()), Some(1));
        assert_eq!(e.toca(cada, t0 + Duration::from_secs(15), Vec::new), Some(vec![]), "terminó: uno vacío");
        assert_eq!(e.toca(cada, t0 + Duration::from_secs(20), Vec::new), None);
    }
}
