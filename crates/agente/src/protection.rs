//! Salud de la protección de un destino: una lista de comprobaciones (copias
//! al día, protección contra borrado, copia externa, verificación, prueba de
//! restauración, kit de recuperación y retención) con su estado.
//!
//! Las reglas viven solo aquí: la app y el informe a la web reúnen los datos
//! (`Facts`) cada uno con lo que tiene a mano y llaman a `evaluate`.

use chrono::{DateTime, Duration, Local};
use serde::{Deserialize, Serialize};

/// Resultado de una tarea (copia, subida, verificación, prueba).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RunFact {
    pub ok: bool,
    /// RFC 3339.
    pub finished: String,
    pub message: String,
}

impl RunFact {
    pub fn from_record(r: &crate::agent::RunRecord) -> Self {
        Self { ok: r.result != "error", finished: r.finished.clone(), message: r.message.clone() }
    }
}

/// Lo que se sabe de un destino.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    /// "local", "rest", "sftp", "s3", "b2", "azure", "gs"…
    pub kind: String,
    /// Destinos que suben aquí su copia externa (nombres).
    pub offsite_sources: Vec<String>,
    /// Está en el agente (copias automáticas o «solo vigilar»).
    pub scheduled: bool,
    pub monitor: bool,
    pub plans: usize,
    pub paused: bool,
    /// Últimas copias automáticas de sus copias (una por copia).
    pub backups: Vec<RunFact>,
    /// REST: ¿servidor de solo añadir? (`None`: sin comprobar).
    pub append_only: Option<bool>,
    /// Nube: el usuario declaró que el bucket tiene bloqueo de objetos.
    pub object_lock: bool,
    pub has_offsite: bool,
    pub offsite_run: Option<RunFact>,
    pub offsite_held: bool,
    /// RFC 3339 de la versión más reciente del destino.
    pub last_snapshot: Option<String>,
    pub has_verify: bool,
    pub verify_run: Option<RunFact>,
    pub has_cloud_verify: bool,
    pub cloud_verify_run: Option<RunFact>,
    pub has_restore_test: bool,
    pub restore_test_run: Option<RunFact>,
    /// Kit guardado y vigente / guardado pero ya no vale (cambió la ubicación).
    pub kit_ok: bool,
    pub kit_stale: bool,
    pub has_retention: bool,
    /// v1.41: un destino local, qué disco es (unidad, extraíble, de la red). `None`: no es local.
    pub disk: Option<crate::espacio::Disco>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Ok,
    Warn,
    Bad,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    /// "copias", "borrado", "externa", "verificacion", "restauracion", "kit", "retencion".
    pub id: String,
    pub state: State,
    /// Título corto.
    pub label: String,
    /// Una línea que explica el estado.
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Protection {
    /// Comprobaciones en verde.
    pub score: usize,
    pub total: usize,
    pub items: Vec<Item>,
}

fn item(id: &str, state: State, label: &str, detail: impl Into<String>) -> Item {
    Item { id: id.into(), state, label: label.into(), detail: detail.into() }
}

fn parse(t: &str) -> Option<DateTime<Local>> {
    DateTime::parse_from_rfc3339(t).ok().map(|d| d.with_timezone(&Local))
}

fn ago(t: &str, now: DateTime<Local>) -> String {
    let Some(t) = parse(t) else { return String::new() };
    let d = now.signed_duration_since(t);
    if d < Duration::hours(1) {
        "hace un momento".into()
    } else if d < Duration::hours(36) {
        format!("hace {} h", d.num_hours())
    } else {
        format!("hace {} días", d.num_days())
    }
}

/// Nombres en una frase: «X», «X y Y».
fn names(list: &[String]) -> String {
    match list {
        [] => String::new(),
        [one] => format!("«{one}»"),
        _ => {
            let (last, rest) = list.split_last().unwrap();
            format!("{} y «{last}»", rest.iter().map(|n| format!("«{n}»")).collect::<Vec<_>>().join(", "))
        }
    }
}

pub fn evaluate(f: &Facts, now: DateTime<Local>) -> Protection {
    let cloud = matches!(f.kind.as_str(), "s3" | "b2" | "azure" | "gs");
    let target = !f.offsite_sources.is_empty() && f.plans == 0;
    let mut items = Vec::new();

    // 1. Copias automáticas activas y al día.
    items.push(if target {
        item("copias", State::Ok, "Copias", format!("Recibe la copia externa de {}.", names(&f.offsite_sources)))
    } else if !f.scheduled {
        item("copias", State::Bad, "Copias automáticas", "Sin copias automáticas: solo se copia cuando alguien lo hace a mano.")
    } else if f.monitor {
        item("copias", State::Ok, "Copias automáticas", "«Solo vigilar»: las hace otro programa y Resguardo las revisa.")
    } else if f.backups.iter().any(|b| !b.ok) {
        item("copias", State::Bad, "Copias automáticas", "La última copia automática de alguna copia falló.")
    } else if f.paused {
        item("copias", State::Warn, "Copias automáticas", "En pausa.")
    } else if f.backups.is_empty() {
        item("copias", State::Warn, "Copias automáticas", "Programadas, todavía sin ninguna hecha.")
    } else {
        item("copias", State::Ok, "Copias automáticas", "Activas y al día.")
    });

    // 2. Protección contra borrado (un ransomware no puede borrar las versiones).
    items.push(match f.kind.as_str() {
        "rest" => match f.append_only {
            Some(true) => item("borrado", State::Ok, "Protegida contra borrado", "El servidor es de solo añadir: desde este equipo no se puede borrar nada."),
            Some(false) => item(
                "borrado",
                State::Warn,
                "Protegida contra borrado",
                "El servidor permite borrar: arranca rest-server con --append-only para que nadie pueda borrar las versiones desde aquí.",
            ),
            None => item("borrado", State::Unknown, "Protegida contra borrado", "Sin comprobar: no se pudo consultar al servidor."),
        },
        _ if cloud && f.object_lock => item("borrado", State::Ok, "Protegida contra borrado", "El bucket tiene bloqueo de objetos (Object Lock)."),
        _ if cloud => item(
            "borrado",
            State::Warn,
            "Protegida contra borrado",
            "Activa el bloqueo de objetos (Object Lock) en el bucket y márcalo aquí para que nadie pueda borrar las versiones.",
        ),
        _ => item(
            "borrado",
            State::Warn,
            "Protegida contra borrado",
            "Un ransomware podría borrarla; usa un servidor de solo añadir o una copia externa con bloqueo.",
        ),
    });

    // 3. Copia externa configurada y al día.
    items.push(if target {
        item("externa", State::Ok, "Copia externa", format!("Este repositorio es la copia externa de {}.", names(&f.offsite_sources)))
    } else if !f.has_offsite {
        item("externa", State::Bad, "Copia externa", "Todas las versiones están en un solo sitio: configura una copia externa (la nube u otro disco).")
    } else if f.offsite_held {
        item("externa", State::Bad, "Copia externa", "Subida frenada por un cambio inusual: revísalo.")
    } else {
        match &f.offsite_run {
            None => item("externa", State::Warn, "Copia externa", "Configurada, todavía sin ninguna subida."),
            Some(r) if !r.ok => item("externa", State::Bad, "Copia externa", format!("La última subida falló: {}", r.message)),
            Some(r) => {
                let behind = match (parse(&r.finished), f.last_snapshot.as_deref().and_then(parse)) {
                    (Some(up), Some(snap)) => snap - up > Duration::hours(24),
                    _ => false,
                };
                if behind {
                    item(
                        "externa",
                        State::Warn,
                        "Copia externa",
                        format!("Atrasada: la última subida fue {} y hay versiones más nuevas.", ago(&r.finished, now)),
                    )
                } else {
                    item("externa", State::Ok, "Copia externa", format!("Al día (última subida {}).", ago(&r.finished, now)))
                }
            }
        }
    });

    // 4. Verificación (del destino y, si la hay, de su copia en la nube).
    let (has_v, run_v) = if target { (f.has_cloud_verify, f.cloud_verify_run.clone()) } else { (f.has_verify, f.verify_run.clone()) };
    items.push(if !has_v {
        item(
            "verificacion",
            State::Warn,
            "Verificación",
            if target { "Sin verificación: prográmala en el origen, en «Copia externa»." } else { "Sin verificación programada." },
        )
    } else if run_v.as_ref().is_some_and(|r| !r.ok) || (!target && f.cloud_verify_run.as_ref().is_some_and(|r| !r.ok)) {
        item("verificacion", State::Bad, "Verificación", "La última verificación encontró errores o no se pudo hacer.")
    } else if let Some(r) = run_v {
        let cloud_note = if !target && f.has_cloud_verify { " (también la copia en la nube)" } else { "" };
        item("verificacion", State::Ok, "Verificación", format!("Sin errores{cloud_note}, {}.", ago(&r.finished, now)))
    } else {
        item("verificacion", State::Warn, "Verificación", "Programada, todavía sin ninguna hecha.")
    });

    // 5. Prueba de restauración (en un destino que solo recibe una copia externa, no aplica).
    if !target {
        items.push(if !f.has_restore_test {
            item("restauracion", State::Warn, "Prueba de restauración", "Sin prueba: nadie comprueba que las copias se puedan recuperar de verdad.")
        } else {
            match &f.restore_test_run {
                None => item("restauracion", State::Warn, "Prueba de restauración", "Programada, todavía sin ninguna hecha."),
                Some(r) if !r.ok => item("restauracion", State::Bad, "Prueba de restauración", format!("La última falló: {}", r.message)),
                Some(r) if parse(&r.finished).is_some_and(|t| now - t > Duration::days(45)) => {
                    item("restauracion", State::Warn, "Prueba de restauración", format!("La última correcta fue {}.", ago(&r.finished, now)))
                }
                Some(r) => item("restauracion", State::Ok, "Prueba de restauración", format!("Correcta, {}.", ago(&r.finished, now))),
            }
        });
    }

    // 6. Kit de recuperación.
    items.push(if f.kit_ok {
        item("kit", State::Ok, "Kit de recuperación", "Guardado.")
    } else if f.kit_stale {
        item("kit", State::Warn, "Kit de recuperación", "La ubicación cambió desde el último kit: guarda uno nuevo.")
    } else {
        item("kit", State::Bad, "Kit de recuperación", "Sin kit: si pierdes este equipo no podrás abrir las copias.")
    });

    // 7. Retención.
    items.push(if !f.has_retention {
        item("retencion", State::Warn, "Retención", "Sin política: el repositorio crece sin fin.")
    } else if f.kind == "rest" && f.append_only == Some(true) {
        item("retencion", State::Ok, "Retención", "Configurada; se aplica en el servidor.")
    } else {
        item("retencion", State::Ok, "Retención", "Configurada.")
    });

    // 8. v1.41: fuera del equipo que protege (solo un destino local; los demás ya lo están).
    // Al final, para no mover las anteriores: quien lea `items` por posición sigue igual.
    if f.kind == "local" && !target {
        let disk = f.disk.clone().unwrap_or_default();
        let unidad = disk.unidad.as_deref().map(|u| format!(" ({u})")).unwrap_or_default();
        items.push(if disk.red {
            item("lugar", State::Ok, "Fuera de este equipo", "En una carpeta de otra máquina de la red.")
        } else if disk.extraible == Some(true) {
            item("lugar", State::Ok, "Fuera de este equipo", format!("En un disco extraíble{unidad}: guárdalo lejos del equipo cuando no copie."))
        } else if f.has_offsite {
            item("lugar", State::Ok, "Fuera de este equipo", format!("En este mismo equipo{unidad}, pero con copia externa."))
        } else {
            item(
                "lugar",
                State::Warn,
                "Fuera de este equipo",
                format!(
                    "En este mismo equipo{unidad}: si se daña o lo cifra un ransomware, se pierden los archivos y las copias. Guárdalas en un almacén de otro equipo o añade una copia externa."
                ),
            )
        });
    }

    Protection { score: items.iter().filter(|i| i.state == State::Ok).count(), total: items.len(), items }
}

// ---------- Datos del agente ----------

/// Lo que sabe el agente de un destino (configuración, últimas ejecuciones,
/// tareas y freno). La app completa después lo suyo (retención, kit…).
pub fn agent_facts(
    repo_id: &str,
    location: &str,
    config: &crate::agent::AgentConfig,
    state: &crate::agent::AgentState,
    tasks: &crate::tasks::TasksState,
    guard: &crate::tasks::GuardState,
    now: DateTime<Local>,
) -> Facts {
    let entry = config.repos.iter().find(|r| r.id == repo_id);
    let task = |kind: &str| tasks.runs.get(&crate::tasks::key(kind, repo_id)).map(RunFact::from_record);
    // Si es destino de una copia externa, su verificación es la de la nube del origen.
    let sources: Vec<&crate::agent::AgentRepo> =
        config.repos.iter().filter(|r| r.offsite.as_ref().is_some_and(|o| o.provider == format!("destino:{repo_id}"))).collect();
    let cloud_verify_src = sources.iter().find(|s| s.offsite.as_ref().is_some_and(|o| o.verify.is_some()));
    let mut f = Facts {
        kind: crate::kit::kind_of(location).into(),
        offsite_sources: sources.iter().map(|s| s.name.clone()).collect(),
        scheduled: entry.is_some(),
        monitor: entry.is_some_and(|e| matches!(e.schedule, crate::agent::Schedule::Monitor { .. })),
        plans: entry.map_or(0, |e| e.plans.len()),
        paused: entry.is_some_and(|e| e.active_pause(now).is_some()),
        backups: entry
            .map(|e| e.plans.iter().filter_map(|p| state.runs.get(&crate::plans::plan_key(repo_id, &p.id))).map(RunFact::from_record).collect())
            .unwrap_or_default(),
        append_only: state.append_only.get(repo_id).and_then(|c| c.append_only),
        has_offsite: entry.is_some_and(|e| e.offsite.is_some()),
        offsite_run: task("offsite"),
        offsite_held: guard.holds.contains_key(repo_id),
        last_snapshot: state.web.snapshots.get(repo_id).and_then(|s| s.last_time.clone()),
        has_verify: entry.is_some_and(|e| e.verify.is_some()),
        verify_run: task("verify"),
        has_restore_test: entry.is_some_and(|e| e.restore_test.is_some()),
        restore_test_run: task("restore_test"),
        has_cloud_verify: entry.is_some_and(|e| e.offsite.as_ref().is_some_and(|o| o.verify.is_some())),
        cloud_verify_run: task("verify_offsite"),
        ..Default::default()
    };
    if f.kind == "local" {
        f.disk = Some(crate::espacio::disco_de(location));
    }
    if f.plans == 0 {
        if let Some(src) = cloud_verify_src {
            f.has_cloud_verify = true;
            f.cloud_verify_run = tasks.runs.get(&crate::tasks::key("verify_offsite", &src.id)).map(RunFact::from_record);
        }
    }
    if let Some(e) = entry {
        f.object_lock = e.object_lock;
        f.has_retention = e.has_retention;
        f.kit_ok = e.kit.as_ref().is_some_and(|k| k.matches(location, None));
        f.kit_stale = e.kit.is_some() && !f.kit_ok;
    }
    f
}

// ---------- ¿Servidor REST de solo añadir? ----------

/// Resultado de la comprobación (se guarda para no repetirla más de una vez al día).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppendOnlyCheck {
    pub checked_at: String,
    /// `None`: no se pudo saber.
    pub append_only: Option<bool>,
}

impl AppendOnlyCheck {
    /// Un resultado vale un día; «no se pudo saber», solo una hora (sin red, o
    /// un agente anterior que no usaba la autoridad propia del almacén).
    pub fn fresh(&self, now: DateTime<Local>) -> bool {
        let vale = if self.append_only.is_some() { Duration::hours(24) } else { Duration::hours(1) };
        parse(&self.checked_at).is_some_and(|t| now - t < vale)
    }
}

pub(crate) fn base64(input: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Dirección HTTP de un objeto inexistente del repositorio (`data/<64 hex al azar>`).
pub fn probe_url(location: &str) -> Option<String> {
    let rest = location.strip_prefix("rest:")?;
    if !(rest.starts_with("http://") || rest.starts_with("https://")) {
        return None;
    }
    // Sin `usuario:contraseña@` en la dirección (van en la cabecera).
    let (scheme, after) = rest.split_once("://")?;
    let (authority, path) = after.split_at(after.find('/').unwrap_or(after.len()));
    let host = authority.rsplit('@').next().unwrap_or(authority);
    let mut rng = crate::restore_test::Rng::from_clock();
    let id: String = (0..4).map(|_| format!("{:016x}", rng.next())).collect();
    Some(format!("{scheme}://{host}{}/data/{id}", path.trim_end_matches('/')))
}

/// ¿El rest-server está en modo `--append-only`? Pide borrar un objeto que no
/// existe (nombre al azar): en solo añadir responde 403 antes de mirar nada;
/// si no, 200 (borrar algo inexistente no es un error). Nunca se borra nada.
/// `None` si no se puede saber (sin red, credenciales…).
///
/// `cacert`: el archivo PEM con la autoridad propia del servidor (la de un
/// almacén, «Este equipo guarda copias», o el `ca_pem` de un destino), la
/// misma que restic usa con `--cacert`. Sin ella, las autoridades del sistema.
pub fn probe_append_only(location: &str, auth: Option<(&str, &str)>, cacert: Option<&str>) -> Option<bool> {
    let url = probe_url(location)?;
    let tls = match cacert {
        Some(path) => tls_con_autoridad(&std::fs::read(path).ok()?)?,
        None => crate::web::tls_sistema(),
    };
    let agent =
        ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(15))).http_status_as_error(false).tls_config(tls).build().new_agent();
    let mut req = agent.delete(&url);
    if let Some((user, pass)) = auth {
        req = req.header("Authorization", format!("Basic {}", base64(format!("{user}:{pass}").as_bytes())));
    }
    let status = req.call().ok()?.status().as_u16();
    estado_solo_anadir(status)
}

/// TLS que solo confía en las autoridades de un PEM (todas las que traiga).
pub(crate) fn tls_con_autoridad(pem: &[u8]) -> Option<ureq::tls::TlsConfig> {
    use ureq::tls::{PemItem, RootCerts, TlsConfig, TlsProvider};
    let certs: Vec<_> = ureq::tls::parse_pem(pem)
        .filter_map(|i| match i {
            Ok(PemItem::Certificate(c)) => Some(c),
            _ => None,
        })
        .collect();
    (!certs.is_empty()).then(|| TlsConfig::builder().provider(TlsProvider::Rustls).root_certs(RootCerts::Specific(std::sync::Arc::new(certs))).build())
}

/// La respuesta de rest-server al borrar un objeto que no existe.
fn estado_solo_anadir(status: u16) -> Option<bool> {
    match status {
        403 => Some(true),
        200..=299 | 404 => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Local> {
        parse("2026-10-01T12:00:00-05:00").unwrap()
    }

    fn ok(t: &str) -> Option<RunFact> {
        Some(RunFact { ok: true, finished: t.into(), message: String::new() })
    }

    fn completo() -> Facts {
        Facts {
            kind: "rest".into(),
            scheduled: true,
            plans: 2,
            backups: vec![ok("2026-10-01T11:00:00-05:00").unwrap()],
            append_only: Some(true),
            has_offsite: true,
            offsite_run: ok("2026-10-01T11:30:00-05:00"),
            last_snapshot: Some("2026-10-01T11:00:00-05:00".into()),
            has_verify: true,
            verify_run: ok("2026-09-28T03:00:00-05:00"),
            has_restore_test: true,
            restore_test_run: ok("2026-09-27T04:00:00-05:00"),
            kit_ok: true,
            has_retention: true,
            ..Default::default()
        }
    }

    #[test]
    fn todo_en_verde() {
        let p = evaluate(&completo(), now());
        assert_eq!((p.score, p.total), (7, 7), "{:#?}", p.items);
        assert_eq!(p.items.iter().find(|i| i.id == "retencion").unwrap().detail, "Configurada; se aplica en el servidor.");
    }

    #[test]
    fn lo_que_falta_se_ve() {
        let mut f = completo();
        f.kind = "local".into();
        f.has_offsite = false;
        f.kit_ok = false;
        f.restore_test_run = Some(RunFact { ok: false, finished: "2026-09-30T04:00:00-05:00".into(), message: "x".into() });
        let p = evaluate(&f, now());
        let state = |id: &str| p.items.iter().find(|i| i.id == id).unwrap().state.clone();
        assert_eq!(state("borrado"), State::Warn);
        assert_eq!(state("externa"), State::Bad);
        assert_eq!(state("kit"), State::Bad);
        assert_eq!(state("restauracion"), State::Bad);
        assert_eq!(p.score, 3);
        // Copia externa atrasada: hay versiones más de 24 h posteriores a la última subida.
        let mut f = completo();
        f.offsite_run = ok("2026-09-28T11:00:00-05:00");
        let p = evaluate(&f, now());
        assert_eq!(p.items.iter().find(|i| i.id == "externa").unwrap().state, State::Warn);
        // REST sin comprobar.
        let mut f = completo();
        f.append_only = None;
        assert_eq!(evaluate(&f, now()).items[1].state, State::Unknown);
    }

    #[test]
    fn copias_en_el_mismo_equipo() {
        let lugar = |f: &Facts| evaluate(f, now()).items.into_iter().find(|i| i.id == "lugar");
        // Fuera del equipo (un servidor, la nube): no hace falta decirlo.
        assert!(lugar(&completo()).is_none());
        assert_eq!(evaluate(&completo(), now()).total, 7);
        // En una carpeta del propio equipo, sin copia externa: aviso, con la unidad.
        let mut f = completo();
        f.kind = "local".into();
        f.has_offsite = false;
        f.disk = Some(crate::espacio::Disco { unidad: Some("D:".into()), extraible: Some(false), red: false });
        let i = lugar(&f).unwrap();
        assert_eq!(i.state, State::Warn);
        assert!(i.detail.starts_with("En este mismo equipo (D:)"), "{}", i.detail);
        assert_eq!(evaluate(&f, now()).total, 8);
        // Sin saber qué disco es (un agente que no pudo mirarlo): también aviso.
        f.disk = None;
        assert_eq!(lugar(&f).unwrap().state, State::Warn);
        // Un disco USB, una carpeta de la red o con copia externa: bien.
        f.disk = Some(crate::espacio::Disco { unidad: Some("E:".into()), extraible: Some(true), red: false });
        assert_eq!(lugar(&f).unwrap().state, State::Ok);
        f.disk = Some(crate::espacio::Disco { unidad: None, extraible: Some(false), red: true });
        assert_eq!(lugar(&f).unwrap().state, State::Ok);
        f.disk = None;
        f.has_offsite = true;
        assert!(lugar(&f).unwrap().detail.contains("con copia externa"));
    }

    #[test]
    fn destino_de_una_copia_externa() {
        let f = Facts {
            kind: "s3".into(),
            offsite_sources: vec!["Siigo".into()],
            object_lock: true,
            has_cloud_verify: true,
            cloud_verify_run: ok("2026-09-30T05:00:00-05:00"),
            kit_ok: true,
            has_retention: true,
            ..Default::default()
        };
        let p = evaluate(&f, now());
        assert_eq!(p.total, 6, "sin prueba de restauración propia");
        assert_eq!(p.score, 6, "{:#?}", p.items);
        assert!(p.items[0].detail.contains("Recibe la copia externa de «Siigo»"));
    }

    #[test]
    fn comprobacion_del_servidor() {
        let u = probe_url("rest:https://ana:clave@nas.local:8000/siigo/").unwrap();
        assert!(u.starts_with("https://nas.local:8000/siigo/data/"), "{u}");
        assert_eq!(u.rsplit('/').next().unwrap().len(), 64);
        assert!(!u.contains("clave") && !u.contains("ana@"));
        assert!(probe_url("s3:x").is_none());
        assert_eq!(base64(b"usuario:clave"), "dXN1YXJpbzpjbGF2ZQ==");
        assert_eq!(base64(b"ab"), "YWI=");
        let c = AppendOnlyCheck { checked_at: "2026-10-01T00:00:00-05:00".into(), append_only: Some(true) };
        assert!(c.fresh(now()));
        assert!(!AppendOnlyCheck { checked_at: "2026-09-29T00:00:00-05:00".into(), append_only: None }.fresh(now()));
        // «No se sabe» se vuelve a mirar a la hora; un resultado, al día.
        assert!(!AppendOnlyCheck { checked_at: "2026-10-01T10:00:00-05:00".into(), append_only: None }.fresh(now()));
        assert!(AppendOnlyCheck { checked_at: "2026-10-01T10:00:00-05:00".into(), append_only: Some(false) }.fresh(now()));
        assert_eq!(
            (estado_solo_anadir(403), estado_solo_anadir(200), estado_solo_anadir(404), estado_solo_anadir(401)),
            (Some(true), Some(false), Some(false), None)
        );
    }

    #[test]
    fn autoridad_propia_del_almacen() {
        let (ca, _, _) = resguardo_motor::tls::generar_ca("Prueba").unwrap();
        assert!(tls_con_autoridad(ca.as_bytes()).is_some());
        assert!(tls_con_autoridad(b"no es un certificado").is_none());
        // Un archivo de autoridad que no existe: no se sabe (no se cae a las del sistema).
        assert_eq!(probe_append_only("rest:https://127.0.0.1:9/x/", None, Some("no-existe.pem")), None);
    }
}
