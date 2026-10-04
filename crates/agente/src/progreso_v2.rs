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
        // v1.36: lo que restic lee del disco y lo que sube o escribe en el destino (bytes/s,
        // medido en el proceso) y archivos por segundo, para las gráficas en vivo.
        "lectura": if con_cifras { r.read_bps } else { None },
        "subida": if con_cifras { r.upload_bps } else { None },
        "archivos_s": if con_cifras { r.files_per_s } else { None },
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
        // Lo que restic lee y sube o escribe de verdad (bytes/s, de sus contadores de E/S).
        "lectura": t.read_bps,
        "subida": t.upload_bps,
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

/// Lo que lee y escribe un proceso hijo (restic o rclone), por segundo, de sus
/// contadores de E/S (`platform::io_proceso`), como en las copias: la lectura y
/// la subida reales de una verificación, una copia externa o un espejo a la nube.
#[derive(Debug, Default)]
pub struct RitmoIo {
    /// El proceso, sus contadores y cuándo se leyeron.
    antes: Option<(u32, (u64, u64), Instant)>,
    pub lectura: Option<u64>,
    pub escritura: Option<u64>,
}

/// Entre dos muestras, al menos esto (menos da ritmos con mucho ruido).
const RITMO_CADA: Duration = Duration::from_secs(2);

impl RitmoIo {
    /// Una muestra del proceso `pid` (si hay uno): los ritmos se actualizan, suavizados,
    /// cada [`RITMO_CADA`] mientras sea el mismo proceso. Devuelve (lectura, escritura).
    pub fn medir(&mut self, pid: Option<u32>) -> (Option<u64>, Option<u64>) {
        let ahora = Instant::now();
        if let Some(io) = pid.and_then(|p| crate::platform::io_proceso(p).map(|x| (p, x))) {
            self.anotar(io.0, io.1, ahora);
        }
        (self.lectura, self.escritura)
    }

    /// Lo mismo, con los contadores ya leídos (para probarlo).
    pub fn anotar(&mut self, pid: u32, (l1, e1): (u64, u64), ahora: Instant) {
        match self.antes {
            Some((p0, (l0, e0), t0)) if p0 == pid => {
                let s = ahora.saturating_duration_since(t0);
                if s < RITMO_CADA {
                    return;
                }
                let s = s.as_secs_f64();
                self.lectura = crate::agent::smoothed_rate(self.lectura, l1.saturating_sub(l0), s);
                self.escritura = crate::agent::smoothed_rate(self.escritura, e1.saturating_sub(e0), s);
            }
            // Otro proceso (restic lanza varios en una tarea): se empieza a contar desde aquí.
            _ => {}
        }
        self.antes = Some((pid, (l1, e1), ahora));
    }
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
            read_bps: None,
            upload_bps: None,
            files_per_s: None,
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
        r.read_bps = Some(3_000);
        r.upload_bps = Some(900);
        r.files_per_s = Some(12);
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
        assert_eq!((t["lectura"].as_u64(), t["subida"].as_u64(), t["archivos_s"].as_u64()), (Some(3_000), Some(900), Some(12)));
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
        assert!(v["lectura"].is_null() && v["subida"].is_null(), "sin contadores, sin ritmos (la consola los deduce de los bytes)");
        let v = tarea_larga(&crate::tasks::RunningTask { read_bps: Some(5_000), upload_bps: Some(4_000), ..t });
        assert_eq!((v["lectura"].as_u64(), v["subida"].as_u64()), (Some(5_000), Some(4_000)));
    }

    #[test]
    fn ritmo_de_los_contadores_de_un_proceso() {
        let mut r = RitmoIo::default();
        let t0 = Instant::now();
        r.anotar(7, (1_000, 500), t0);
        assert_eq!((r.lectura, r.escritura), (None, None), "la primera muestra no sabe el ritmo");
        // Antes de 2 s, nada nuevo.
        r.anotar(7, (2_000, 600), t0 + Duration::from_secs(1));
        assert_eq!(r.lectura, None);
        r.anotar(7, (5_000, 2_500), t0 + Duration::from_secs(4));
        assert_eq!((r.lectura, r.escritura), (Some(1_000), Some(500)));
        // Otro proceso (otro restic de la misma tarea): empieza a contar de nuevo, sin saltos.
        r.anotar(8, (10, 10), t0 + Duration::from_secs(6));
        assert_eq!((r.lectura, r.escritura), (Some(1_000), Some(500)));
        r.anotar(8, (2_010, 10), t0 + Duration::from_secs(8));
        assert_eq!((r.lectura, r.escritura), (Some(1_000), Some(300)), "suavizado: 0,6 × antes + 0,4 × ahora");
        // Sin proceso, se queda lo último.
        assert_eq!(r.medir(None), (Some(1_000), Some(300)));
        // El propio proceso (siempre existe): mide sin fallar.
        let _ = r.medir(Some(std::process::id()));
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
