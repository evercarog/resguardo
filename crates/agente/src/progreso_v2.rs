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
        "verify_offsite" | crate::tasks::VERIFY_DERIVADA => "verificar_externa",
        "restore_test" => "prueba_restauracion",
        _ => "copia_externa",
    }
}

/// Una tarea larga (verificar, copia externa, prueba de restauración).
pub fn tarea_larga(t: &crate::tasks::RunningTask) -> Value {
    tarea_larga_para(t, "")
}

/// [`tarea_larga`] como la ve la consola `enlace`: 14p, con quién la pidió con «Ahora» y
/// desde qué consola (`otra_consola` si no fue esta; nunca su dirección).
pub fn tarea_larga_para(t: &crate::tasks::RunningTask, enlace: &str) -> Value {
    let mut v = tarea_larga_sin_quien(t);
    if let Some(e) = t.enlace.as_deref() {
        let otra = e != enlace;
        v["por"] = t.por.clone().into();
        v["otra_consola"] = otra.into();
        if otra {
            v["consola"] = t.consola.clone().into();
        }
    }
    v
}

/// La derivada de una tarea (`derivada-<id>`, `verify_derivada-<id>`), si es de una.
fn derivada_de(kind: &str) -> Option<&str> {
    kind.split_once('-').filter(|(k, _)| *k == crate::tasks::DERIVADA || *k == crate::tasks::VERIFY_DERIVADA).map(|(_, d)| d)
}

fn tarea_larga_sin_quien(t: &crate::tasks::RunningTask) -> Value {
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
        // Plan 0.8.1 (C2, 15cc): de qué copia derivada es (el mapa anima su flecha).
        "derivada": derivada_de(&t.kind),
    })
}

/// 14p: el tipo para la consola de una tarea pedida con «Ahora» (`verify`, `derivada-<id>`…).
fn tipo_pedida(pedida: &str) -> &'static str {
    match pedida.split_once('-').map_or(pedida, |(k, _)| k) {
        "verify" => "verificar",
        "verify_offsite" | crate::tasks::VERIFY_DERIVADA => "verificar_externa",
        "restore_test" => "prueba_restauracion",
        _ => "copia_externa",
    }
}

/// 14p: algo pedido con «Ahora» que aún no ha empezado («en cola»), como lo ve la consola `enlace`.
pub fn tarea_en_cola(repo: &str, pedida: &str, quien: &crate::tasks::PedidaPor, enlace: &str) -> Value {
    let mut v = json!({ "tipo": tipo_pedida(pedida), "repo": repo, "fase": "en_cola", "etapa": "En cola", "derivada": derivada_de(pedida) });
    if let Some(e) = quien.enlace.as_deref() {
        let otra = e != enlace;
        v["por"] = quien.por.clone().into();
        v["otra_consola"] = otra.into();
        if otra {
            v["consola"] = quien.consola.clone().into();
        }
    }
    v
}

/// 14p: la vuelta de un espejo en marcha (del registro del servicio), sin rutas: el trabajo,
/// quién lo hace, lo copiado de lo que hay que copiar, el ritmo y lo que queda.
pub fn tarea_espejo(a: &crate::escritorio::Actividad, enlace: &str) -> Option<Value> {
    let id = a.espejo.as_deref().filter(|_| a.tipo == "espejo" || a.tipo == "nube")?;
    let otra = a.enlace.as_deref().is_some_and(|e| e != enlace);
    Some(json!({
        "tipo": "espejo",
        "repo": id,
        "espejo": id,
        "nombre": a.espejo_nombre,
        "quien": a.quien,
        "primera": a.primera.then_some(true),
        "fase": if a.bytes_total.is_none() { "preparando" } else { "subiendo" },
        "etapa": if a.primera { "Primera subida" } else { "Subiendo lo nuevo" },
        "porcentaje": a.porcentaje,
        "bytes": a.bytes,
        "bytes_total": a.bytes_total,
        "velocidad": a.velocidad,
        "lectura": a.lectura,
        "subida": a.subida.or(a.velocidad),
        "quedan_s": a.quedan_s,
        "empezo": a.empezo,
        "actualizado": chrono::Local::now().to_rfc3339(),
        "por": a.por,
        "otra_consola": a.enlace.is_some().then_some(otra),
        "consola": if otra { a.consola.clone() } else { None },
    }))
}

/// 14p: los espejos con «Hacer ahora» pendiente que no están en marcha («en cola»).
fn espejos_en_cola(en_marcha: &[crate::escritorio::Actividad], enlace: &str) -> Vec<Value> {
    let corriendo: Vec<&str> = en_marcha.iter().filter_map(|a| a.espejo.as_deref()).collect();
    let mut trabajos = crate::espejo_trabajos::cargar_equipo().trabajos;
    let c = crate::server::load();
    if c.enabled {
        trabajos.extend(c.espejo.map(|e| e.trabajos_efectivos()).unwrap_or_default());
    }
    trabajos
        .iter()
        .filter(|t| t.activo && t.estado.pedido_ahora && !corriendo.contains(&t.id.as_str()))
        .map(|t| {
            let p = t.estado.pedido.clone().unwrap_or_default();
            let otra = !p.enlace.is_empty() && p.enlace != enlace;
            json!({
                "tipo": "espejo", "repo": t.id, "espejo": t.id, "nombre": t.nombre, "quien": t.quien,
                "primera": t.nunca_bien().then_some(true), "fase": "en_cola", "etapa": "En cola",
                "por": p.por, "otra_consola": (!p.enlace.is_empty()).then_some(otra), "consola": if otra { p.consola } else { None },
            })
        })
        .collect()
}

/// Lo que está en marcha ahora mismo en este equipo (vacío si nada), tal como
/// se le cuenta a la consola de `v`: a todas las consolas lo mismo, salvo quién
/// empezó cada operación (`otra_consola`, ver [`tarea_operacion`]).
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
    let enlace = v.map(Vinculo::id_enlace).unwrap_or_default();
    if let Some(t) = crate::tasks::load_state().live_running() {
        out.push(tarea_larga_para(t, &enlace));
    }
    // v1.47: lo que corre dentro del servicio por una orden (traer el historial, también
    // al «Mover a otro sitio…»; aplicar la retención) y las restauraciones.
    out.extend(ops::lista().iter().map(|o| tarea_operacion(o, &enlace)));
    let actividades = crate::escritorio::en_marcha::actividades();
    out.extend(actividades.iter().filter_map(|a| tarea_restauracion(a, &enlace)));
    // 14p: los espejos en marcha y, después, lo pedido con «Ahora» que aún no empezó.
    out.extend(actividades.iter().filter_map(|a| tarea_espejo(a, &enlace)));
    if out.len() < MAX_TAREAS {
        out.extend(crate::tasks::en_cola(&crate::agent::load_config()).iter().map(|(r, p, q)| tarea_en_cola(r, p, q, &enlace)));
        out.extend(espejos_en_cola(&actividades, &enlace));
    }
    out.truncate(MAX_TAREAS);
    out
}

/// Una restauración en marcha (del registro del servicio), sin rutas: el repositorio y las cifras.
/// 13c/13f: también archivos, ritmo, lo que queda y quién la empezó y desde qué consola
/// (`otra_consola` si no fue la consola `enlace`; `en_el_equipo` si fue desde su ventana).
pub fn tarea_restauracion(a: &crate::escritorio::Actividad, enlace: &str) -> Option<Value> {
    let repo = a.clave.strip_prefix("restauracion:").filter(|_| a.tipo == "restauracion")?;
    let otra = a.enlace.as_deref().is_some_and(|e| e != enlace);
    Some(json!({
        "tipo": "restauracion",
        "repo": repo,
        "fase": if a.bytes.is_none() { "preparando" } else { "en_marcha" },
        "etapa": "Restaurando",
        "porcentaje": a.porcentaje,
        "bytes": a.bytes,
        "bytes_total": a.bytes_total,
        "archivos": a.archivos,
        "archivos_total": a.archivos_total,
        "velocidad": a.velocidad,
        "subida": a.velocidad,
        "quedan_s": a.quedan_s,
        "empezo": a.empezo,
        "por": a.por,
        "otra_consola": otra,
        "consola": if otra { a.consola.clone() } else { None },
        "en_el_equipo": a.enlace.is_none(),
    }))
}

/// Una operación del servicio como la ve la consola `enlace`. `otra_consola`:
/// la empezó otra de las consolas del equipo (esta solo la enseña, sin poder
/// llevarla); `consola`, el nombre que esa consola tiene en el equipo (nunca su
/// dirección), si tiene uno.
pub fn tarea_operacion(o: &ops::Operacion, enlace: &str) -> Value {
    // 14p: sin consola (la actualización del agente la decide el propio equipo), de nadie.
    let otra = !o.enlace.is_empty() && o.enlace != enlace;
    let porcentaje = match (o.hechas.or(o.bytes), o.total.or(o.bytes_total)) {
        (Some(h), Some(t)) if t > 0 => Some(((h as f64 / t as f64).clamp(0.0, 1.0) * 1000.0).round() / 1000.0),
        _ => None,
    };
    json!({
        "tipo": o.tipo,
        "repo": o.repo,
        "origen": o.origen,
        "nombre": o.nombre,
        "nombre_origen": o.nombre_origen,
        "mover": o.mover.then_some(true),
        "paso": o.paso,
        "fase": if (o.tipo == "historial" && o.hechas.is_none()) || (o.tipo == "actualizacion" && o.bytes.is_none()) { "preparando" } else { "en_marcha" },
        "etapa": (!o.etapa.is_empty()).then(|| o.etapa.clone()),
        "porcentaje": porcentaje,
        "versiones": o.hechas.filter(|_| o.tipo == "historial"),
        "versiones_total": o.total.filter(|_| o.tipo == "historial"),
        // 14p: en bytes (la actualización del agente, la retención del almacén).
        "bytes": o.bytes,
        "bytes_total": o.bytes_total,
        "velocidad": o.velocidad,
        "subida": o.velocidad,
        "quedan_s": o.quedan_s,
        "por": o.por,
        "otra_consola": otra,
        "consola": if otra { o.consola.clone() } else { None },
        "empezo": o.empezo,
        "actualizado": o.actualizado,
    })
}

/// v1.47: las operaciones largas que corren dentro del servicio por una orden
/// (traer el historial —también los pasos de «Mover a otro sitio…»— y aplicar
/// la retención). Solo en memoria: si el servicio se reinicia, la operación se
/// cortó con él (y su orden queda fallida, `largas`). Cada canal las cuenta a
/// su consola con el progreso de siempre, así que TODAS las consolas del
/// equipo las ven, no solo la que mandó la orden.
pub mod ops {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Mutex;

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct Operacion {
        pub id: u64,
        /// `historial` o `retencion`.
        pub tipo: &'static str,
        /// El repositorio en el que se hace (al traer el historial, el que lo recibe).
        pub repo: String,
        /// Al traer el historial de otro repositorio de este equipo, ese.
        pub origen: Option<String>,
        /// El nombre del repositorio (el que le puso la consola).
        pub nombre: Option<String>,
        /// El nombre del de `origen`.
        pub nombre_origen: Option<String>,
        /// Es un paso de «Mover a otro sitio…».
        pub mover: bool,
        /// Qué paso del movimiento: `historial` (todo) o `ultimo` (lo copiado mientras tanto).
        pub paso: Option<String>,
        /// La consola que la empezó (id del enlace) y su nombre en el equipo (sin dirección).
        pub enlace: String,
        pub consola: Option<String>,
        pub etapa: String,
        pub hechas: Option<u64>,
        pub total: Option<u64>,
        /// 14p: lo hecho y lo que hay que hacer en bytes (bajar una versión del agente…).
        pub bytes: Option<u64>,
        pub bytes_total: Option<u64>,
        /// Bytes por segundo (media desde que empezó) y segundos que faltan.
        pub velocidad: Option<u64>,
        pub quedan_s: Option<u64>,
        /// Quién la pidió (como lo dice su consola), si se sabe.
        pub por: Option<String>,
        pub empezo: String,
        pub actualizado: String,
    }

    impl Operacion {
        /// Una operación de `tipo` en `repo` que manda la consola de `v` (su nombre en el equipo, nunca su dirección).
        pub fn de_consola(v: &crate::servidor_v2::Vinculo, tipo: &'static str, repo: &str, etapa: &str) -> Self {
            Operacion {
                tipo,
                repo: repo.to_string(),
                nombre: v.repos_v2.iter().find(|r| r.id == repo).map(|r| r.nombre.clone()),
                enlace: v.id_enlace(),
                consola: Some(v.nombre_consola.trim().chars().take(60).collect::<String>()).filter(|n| !n.is_empty()),
                etapa: etapa.to_string(),
                ..Default::default()
            }
        }
    }

    static OPS: Mutex<Vec<Operacion>> = Mutex::new(Vec::new());
    static CONTADOR: AtomicU64 = AtomicU64::new(1);

    /// Mientras vive, la operación cuenta como en marcha (al soltarlo, se quita).
    #[derive(Debug)]
    pub struct Op(u64);

    pub fn empezar(mut o: Operacion) -> Op {
        o.id = CONTADOR.fetch_add(1, Ordering::Relaxed);
        let ahora = chrono::Local::now().to_rfc3339();
        o.empezo.clone_from(&ahora);
        o.actualizado = ahora;
        let id = o.id;
        OPS.lock().unwrap_or_else(|e| e.into_inner()).push(o);
        Op(id)
    }

    impl Op {
        /// 14p: cómo va en bytes (y de ahí el ritmo medio y lo que queda).
        pub fn bytes(&self, etapa: &str, hechos: u64, total: Option<u64>) {
            let mut l = OPS.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(o) = l.iter_mut().find(|o| o.id == self.0) {
                o.etapa = crate::web::public_message(etapa).chars().take(120).collect();
                o.bytes = Some(hechos);
                o.bytes_total = total;
                let lleva = chrono::DateTime::parse_from_rfc3339(&o.empezo).map(|e| chrono::Local::now().signed_duration_since(e)).ok();
                let s = lleva.map(|d| d.num_milliseconds() as f64 / 1000.0).unwrap_or(0.0);
                o.velocidad = (s >= 2.0).then(|| (hechos as f64 / s).round() as u64);
                o.quedan_s = match (total, o.velocidad) {
                    (Some(t), Some(v)) if v > 0 && t > hechos => Some((t - hechos) / v),
                    _ => None,
                };
                o.actualizado = chrono::Local::now().to_rfc3339();
            }
        }

        /// Cómo va: qué hace (en palabras, sin rutas) y, si se saben, cuántas de cuántas.
        pub fn avance(&self, etapa: &str, hechas: Option<u64>, total: Option<u64>) {
            let mut l = OPS.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(o) = l.iter_mut().find(|o| o.id == self.0) {
                o.etapa = crate::web::public_message(etapa).chars().take(120).collect();
                o.hechas = hechas;
                o.total = total;
                o.actualizado = chrono::Local::now().to_rfc3339();
            }
        }
    }

    /// Cuántas han terminado en este proceso (para la huella del informe: al terminar
    /// una, el informe y el historial del equipo salen enseguida hacia todas las consolas).
    static TERMINADAS: AtomicU64 = AtomicU64::new(0);

    pub fn terminadas() -> u64 {
        TERMINADAS.load(Ordering::Relaxed)
    }

    impl Drop for Op {
        fn drop(&mut self) {
            OPS.lock().unwrap_or_else(|e| e.into_inner()).retain(|o| o.id != self.0);
            TERMINADAS.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Las que están en marcha ahora.
    pub fn lista() -> Vec<Operacion> {
        OPS.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
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

/// Entre dos lecturas de lo que está en marcha, al menos esto (leer son unos pocos archivos pequeños).
pub const MIRAR_CADA: Duration = Duration::from_secs(1);

/// Decide cuándo mandar el progreso: cada `cada` mientras hay algo en marcha, una
/// vez más (vacío) cuando termina, para que la consola lo quite enseguida, y 14p
/// **en cuanto algo empieza, termina o sale de la cola** (sin esperar a `cada`: una
/// verificación de unos segundos se veía empezar y terminar entre dos mensajes).
/// Como mucho un mensaje por [`MIRAR_CADA`].
#[derive(Default)]
pub struct Emisor {
    ultimo: Option<Instant>,
    mirado: Option<Instant>,
    activo: bool,
    /// Qué había en marcha en lo último que se mandó (tipo, repositorio, copia y si estaba en cola).
    claves: Vec<String>,
    /// El servidor no lo admite (anterior a v1.25): no se manda más.
    apagado: bool,
}

/// Qué tareas son, sin sus cifras.
fn claves(t: &[Value]) -> Vec<String> {
    t.iter().map(|x| format!("{}|{}|{}|{}", x["tipo"], x["repo"], x["copia"], x["fase"] == "en_cola")).collect()
}

impl Emisor {
    pub fn desactivar(&mut self) {
        self.apagado = true;
    }

    /// Las tareas que toca mandar ahora, o `None` si aún no toca o no hay nada nuevo que decir.
    pub fn toca(&mut self, cada: Duration, ahora: Instant, leer: impl FnOnce() -> Vec<Value>) -> Option<Vec<Value>> {
        if self.apagado || self.mirado.is_some_and(|m| ahora.saturating_duration_since(m) < MIRAR_CADA.min(cada)) {
            return None;
        }
        self.mirado = Some(ahora);
        let tareas = leer();
        let nuevas = claves(&tareas);
        let toca = self.ultimo.is_none_or(|u| ahora.saturating_duration_since(u) >= cada) || nuevas != self.claves;
        if !toca {
            return None;
        }
        self.ultimo = Some(ahora);
        self.claves = nuevas;
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

    /// v1.47: una operación del servicio (traer el historial al mover un repositorio) se
    /// cuenta a TODAS las consolas; las que no la empezaron la ven como de otra consola,
    /// con su nombre (nunca su dirección), y al soltarla deja de contarse.
    #[test]
    fn operaciones_a_todas_las_consolas() {
        let mut a = Vinculo { modo: "gestionado".into(), url: "https://192.168.1.20:8443".into(), nombre_consola: "Oficina".into(), ..Default::default() };
        a.repos_v2.push(crate::gestion_v2::RepoV2 { id: "prueba-ops-r1".into(), nombre: "Contabilidad".into(), ..Default::default() });
        let mut b = a.clone();
        b.enlace_id = "otra-consola-de-prueba".into();
        b.nombre_consola = "En línea".into();
        let op = ops::Operacion {
            origen: Some("prueba-ops-r1".into()),
            nombre_origen: Some("Contabilidad".into()),
            mover: true,
            paso: Some("historial".into()),
            ..ops::Operacion::de_consola(&a, "historial", "prueba-ops-r1", "Moviéndose a otro sitio: preparando…")
        };
        assert_eq!(op.consola.as_deref(), Some("Oficina"));
        let g = ops::empezar(op);
        let de = |v: &Vinculo| tareas(Some(v)).into_iter().find(|t| t["tipo"] == "historial" && t["repo"] == "prueba-ops-r1").expect("en marcha");
        let t = de(&b);
        assert_eq!((t["fase"].as_str(), t["mover"].as_bool(), t["paso"].as_str()), (Some("preparando"), Some(true), Some("historial")));
        assert_eq!((t["otra_consola"].as_bool(), t["consola"].as_str()), (Some(true), Some("Oficina")), "la otra consola sabe quién la empezó");
        assert!(!t.to_string().contains("192.168"), "nunca la dirección de la consola: {t}");
        let t = de(&a);
        assert_eq!((t["otra_consola"].as_bool(), t["consola"].as_str()), (Some(false), None), "la que la empezó la lleva ella");
        g.avance(r"Trayendo el historial de C:\Users\Ana\x", Some(56), Some(255));
        let t = de(&b);
        assert_eq!((t["fase"].as_str(), t["versiones"].as_u64(), t["versiones_total"].as_u64()), (Some("en_marcha"), Some(56), Some(255)));
        assert_eq!(t["porcentaje"].as_f64(), Some(0.22));
        assert!(!t["etapa"].as_str().unwrap().contains("Ana"), "sin rutas: {}", t["etapa"]);
        assert_eq!(t["origen"], "prueba-ops-r1");
        drop(g);
        assert!(!tareas(Some(&b)).iter().any(|t| t["repo"] == "prueba-ops-r1"), "terminó: ya no se cuenta");
        // Sin nombre en el equipo, ninguno (no se cambia por la dirección).
        let sin =
            ops::Operacion::de_consola(&Vinculo { nombre_consola: " ".into(), url: "https://10.0.0.1".into(), ..Default::default() }, "retencion", "r", "x");
        assert_eq!(sin.consola, None);
        let t = tarea_operacion(&sin, "otra");
        assert_eq!((t["fase"].as_str(), t["otra_consola"].as_bool(), t["consola"].is_null()), (Some("en_marcha"), Some(true), true));
        assert!(t["versiones"].is_null(), "solo el historial cuenta versiones");
    }

    #[test]
    fn restauracion_sin_rutas() {
        let a = crate::escritorio::Actividad {
            tipo: "restauracion".into(),
            clave: "restauracion:r9".into(),
            nombre: "Copias".into(),
            bytes: Some(10),
            bytes_total: Some(40),
            porcentaje: Some(0.25),
            ..Default::default()
        };
        let t = tarea_restauracion(&a, "e1").unwrap();
        assert_eq!(
            (t["tipo"].as_str(), t["repo"].as_str(), t["fase"].as_str(), t["porcentaje"].as_f64()),
            (Some("restauracion"), Some("r9"), Some("en_marcha"), Some(0.25))
        );
        assert!(tarea_restauracion(&crate::escritorio::Actividad { tipo: "espejo".into(), clave: "espejo:1".into(), ..Default::default() }, "e1").is_none());
        // 13f: desde la ventana del equipo; desde esta consola; desde otra (con su nombre, nunca su dirección).
        assert_eq!((t["en_el_equipo"].as_bool(), t["otra_consola"].as_bool()), (Some(true), Some(false)));
        let b = crate::escritorio::Actividad { enlace: Some("e2".into()), consola: Some("Oficina".into()), por: Some("Ana".into()), archivos: Some(3), ..a };
        let de_esta = tarea_restauracion(&b, "e2").unwrap();
        assert_eq!((de_esta["otra_consola"].as_bool(), de_esta["consola"].is_null(), de_esta["por"].as_str()), (Some(false), true, Some("Ana")));
        let de_otra = tarea_restauracion(&b, "e1").unwrap();
        assert_eq!((de_otra["otra_consola"].as_bool(), de_otra["consola"].as_str(), de_otra["archivos"].as_u64()), (Some(true), Some("Oficina"), Some(3)));
    }

    /// 14p: la vuelta de un espejo, a todas las consolas: su trabajo, quién lo hace, si es la
    /// primera subida, las cifras y quién la pidió (nunca la dirección de su consola ni rutas).
    #[test]
    fn espejo_en_marcha_a_todas_las_consolas() {
        let a = crate::escritorio::Actividad {
            tipo: "nube".into(),
            clave: "nube:t1".into(),
            nombre: "B2 Oficina".into(),
            espejo: Some("t1".into()),
            espejo_nombre: Some("Todo a la nube".into()),
            quien: Some("almacen".into()),
            primera: true,
            bytes: Some(30),
            bytes_total: Some(120),
            porcentaje: Some(0.25),
            velocidad: Some(10),
            subida: Some(12),
            quedan_s: Some(9),
            ..Default::default()
        };
        let t = tarea_espejo(&a, "e1").unwrap();
        assert_eq!(
            (t["tipo"].as_str(), t["repo"].as_str(), t["espejo"].as_str(), t["nombre"].as_str()),
            (Some("espejo"), Some("t1"), Some("t1"), Some("Todo a la nube"))
        );
        assert_eq!(
            (t["fase"].as_str(), t["primera"].as_bool(), t["porcentaje"].as_f64(), t["subida"].as_u64()),
            (Some("subiendo"), Some(true), Some(0.25), Some(12))
        );
        assert!(t["otra_consola"].is_null(), "por su horario: de nadie");
        // Pedido con «Hacer ahora» desde otra consola: su nombre, nunca su dirección.
        let b = crate::escritorio::Actividad {
            enlace: Some("e2".into()),
            consola: Some("Oficina".into()),
            por: Some("Ana".into()),
            bytes_total: None,
            ..a.clone()
        };
        let t = tarea_espejo(&b, "e1").unwrap();
        assert_eq!(
            (t["otra_consola"].as_bool(), t["consola"].as_str(), t["por"].as_str(), t["fase"].as_str()),
            (Some(true), Some("Oficina"), Some("Ana"), Some("preparando"))
        );
        assert_eq!(tarea_espejo(&b, "e2").unwrap()["consola"], Value::Null, "la que lo pidió no se nombra a sí misma");
        // Una restauración no es un espejo (y al revés).
        assert!(tarea_espejo(&crate::escritorio::Actividad { tipo: "restauracion".into(), espejo: None, ..Default::default() }, "e1").is_none());
        assert!(tarea_restauracion(&a, "e1").is_none());
    }

    /// 14p: lo pedido con «Ahora» que aún no empezó, «en cola», con quién lo pidió.
    #[test]
    fn en_cola_con_quien_la_pidio() {
        let q = crate::tasks::PedidaPor { por: Some("Ana".into()), consola: Some("Oficina".into()), enlace: Some("e2".into()) };
        let t = tarea_en_cola("r1", "verify", &q, "e1");
        assert_eq!((t["tipo"].as_str(), t["fase"].as_str(), t["etapa"].as_str()), (Some("verificar"), Some("en_cola"), Some("En cola")));
        assert_eq!((t["otra_consola"].as_bool(), t["consola"].as_str(), t["por"].as_str()), (Some(true), Some("Oficina"), Some("Ana")));
        let t = tarea_en_cola("r1", "verify_derivada-d1", &crate::tasks::PedidaPor::default(), "e1");
        assert_eq!((t["tipo"].as_str(), t["derivada"].as_str(), t["otra_consola"].is_null()), (Some("verificar_externa"), Some("d1"), true));
        assert_eq!(tarea_en_cola("r1", "restore_test", &q, "e2")["consola"], Value::Null);
        assert_eq!(tarea_en_cola("r1", "derivada-d1", &q, "e2")["tipo"], "copia_externa");
        // La tarea larga ya en marcha, con quién la pidió.
        let r = crate::tasks::RunningTask {
            repo_id: "r1".into(),
            kind: "verify".into(),
            enlace: Some("e2".into()),
            consola: Some("Oficina".into()),
            por: Some("Ana".into()),
            ..Default::default()
        };
        let t = tarea_larga_para(&r, "e1");
        assert_eq!((t["otra_consola"].as_bool(), t["consola"].as_str(), t["por"].as_str()), (Some(true), Some("Oficina"), Some("Ana")));
        assert!(tarea_larga_para(&crate::tasks::RunningTask { enlace: None, ..r }, "e1")["otra_consola"].is_null());
    }

    /// 14p: la actualización del agente, en bytes (con su ritmo y lo que queda), de nadie.
    #[test]
    fn actualizacion_en_bytes() {
        let g = ops::empezar(ops::Operacion { tipo: "actualizacion", nombre: Some("Resguardo Agente 9.9.9".into()), ..Default::default() });
        let t = tareas(None).into_iter().find(|t| t["tipo"] == "actualizacion").expect("en marcha");
        assert_eq!((t["fase"].as_str(), t["otra_consola"].as_bool()), (Some("preparando"), Some(false)));
        g.bytes(r"Bajando C:\Windows\x", 25, Some(100));
        let t = tareas(None).into_iter().find(|t| t["tipo"] == "actualizacion").expect("en marcha");
        assert_eq!(
            (t["fase"].as_str(), t["bytes"].as_u64(), t["bytes_total"].as_u64(), t["porcentaje"].as_f64()),
            (Some("en_marcha"), Some(25), Some(100), Some(0.25))
        );
        assert!(!t["etapa"].as_str().unwrap().contains("Windows"), "sin rutas");
        assert!(t["versiones"].is_null());
        drop(g);
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

    /// 14p: lo que empieza o termina se manda enseguida (como mucho uno por segundo), sin esperar a `cada`.
    #[test]
    fn emisor_enseguida_al_empezar_y_al_terminar() {
        let mut e = Emisor::default();
        let t0 = Instant::now();
        let cada = Duration::from_secs(5);
        let ver = || vec![json!({ "tipo": "verificar", "repo": "r", "fase": "en_marcha" })];
        let cola = || vec![json!({ "tipo": "verificar", "repo": "r", "fase": "en_cola" })];
        assert_eq!(e.toca(cada, t0, Vec::new), None);
        assert_eq!(e.toca(cada, t0 + Duration::from_millis(500), cola), None, "antes de un segundo no se mira");
        assert_eq!(e.toca(cada, t0 + Duration::from_secs(1), cola).map(|x| x.len()), Some(1), "en cola: enseguida");
        assert_eq!(e.toca(cada, t0 + Duration::from_secs(2), cola), None, "lo mismo: hasta `cada`");
        assert_eq!(e.toca(cada, t0 + Duration::from_secs(3), ver).map(|x| x.len()), Some(1), "empieza: enseguida");
        assert_eq!(e.toca(cada, t0 + Duration::from_secs(4), Vec::new), Some(vec![]), "termina: enseguida");
        assert_eq!(e.toca(cada, t0 + Duration::from_secs(10), Vec::new), None);
    }

    #[test]
    fn emisor_cada_cierto_tiempo_y_uno_vacio_al_terminar() {
        let mut e = Emisor::default();
        let t0 = Instant::now();
        let cada = Duration::from_secs(5);
        let una = || vec![json!({ "tipo": "copia" })];
        assert_eq!(e.toca(cada, t0, Vec::new), None, "nada en marcha: nada que mandar");
        assert_eq!(e.toca(cada, t0 + Duration::from_secs(1), una).map(|x| x.len()), Some(1), "empieza: enseguida");
        assert_eq!(e.toca(cada, t0 + Duration::from_secs(5), una), None, "lo mismo: aún no toca");
        assert_eq!(e.toca(cada, t0 + Duration::from_secs(6), una).map(|x| x.len()), Some(1));
        assert_eq!(e.toca(cada, t0 + Duration::from_secs(11), una).map(|x| x.len()), Some(1));
        assert_eq!(e.toca(cada, t0 + Duration::from_secs(15), Vec::new), Some(vec![]), "terminó: uno vacío");
        assert_eq!(e.toca(cada, t0 + Duration::from_secs(20), Vec::new), None);
    }
}
