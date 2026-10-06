mod account;
mod applock;
mod avisos;
mod backup;
mod diff;
pub(crate) use resguardo_motor::restic;
// El agente y lo que comparte con la app (ver crates/agente).
pub(crate) use resguardo_agente::{
    agent, console, discover, discreto, history, jobs, kit, places, platform, protection, restore_test, server, share, store, tasks, web,
};
pub use resguardo_agente::{
    agent_main, agent_purge, agent_sync_task, agent_tasks_main, agente_main, harden_dll_search, managed_maintenance, poner_version_app, server_main,
    server_stop,
};
mod restore;
pub(crate) use resguardo_motor::plans;
pub(crate) use resguardo_motor::retention;
mod search;
mod stats_cache;
pub(crate) use resguardo_motor::sizes;
mod tray;
mod versiones;

use jobs::Jobs;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use store::{Repo, Store};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};

/// Ejecuta trabajo bloqueante (llamadas a restic) fuera del hilo de la interfaz.
async fn blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn restic_version() -> Result<String, String> {
    blocking(restic::version).await
}

#[tauri::command]
fn list_repos(store: State<'_, Store>) -> Result<Vec<Repo>, String> {
    store.list()
}

use resguardo_motor::restic::has_embedded_password;

fn non_empty(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn add_repo(
    store: State<'_, Store>,
    name: String,
    location: String,
    password: String,
    create: bool,
    rest_username: Option<String>,
    rest_password: Option<String>,
    cacert: Option<String>,
    checks: State<'_, Checks>,
    check_id: Option<String>,
    // Destinos en la nube: claves nuevas o las de otro destino ya guardado.
    cloud_key_id: Option<String>,
    cloud_key_secret: Option<String>,
    cloud_region: Option<String>,
    cloud_from: Option<String>,
    // rest-server: el usuario y la contraseña del servidor de otro repositorio del mismo destino.
    rest_from: Option<String>,
) -> Result<Repo, String> {
    let name = name.trim().to_string();
    let location = location.trim().to_string();
    if name.is_empty() || location.is_empty() {
        return Err("El nombre y la ubicación son obligatorios.".into());
    }
    if password.is_empty() {
        return Err("La contraseña es obligatoria.".into());
    }
    if has_embedded_password(&location) {
        return Err("No incluyas la contraseña del servidor dentro de la ubicación: quedaría guardada en texto \
                    plano. Escríbela en los campos de usuario y contraseña del servidor."
            .into());
    }

    let rest_auth = match (non_empty(rest_from), non_empty(rest_username)) {
        (Some(from), _) => store::access(&store.get(&from)?)?.rest_auth,
        (None, Some(user)) => {
            if !location.starts_with("rest:") {
                return Err("El usuario y la contraseña del servidor solo se usan con servidores REST.".into());
            }
            let pass = rest_password.filter(|p| !p.is_empty()).ok_or("Falta la contraseña del servidor REST.")?;
            Some((user, pass))
        }
        (None, None) => None,
    };
    let cacert = non_empty(cacert);
    if let Some(path) = &cacert {
        if !std::path::Path::new(path).is_file() {
            return Err(format!("No se encontró el certificado: {path}"));
        }
    }

    let cloud = match non_empty(cloud_from) {
        Some(from) => Some(store::cloud_creds(&store.get(&from)?)?.ok_or("Ese repositorio no tiene claves de nube guardadas.")?),
        None => match (non_empty(cloud_key_id), non_empty(cloud_key_secret)) {
            (Some(key_id), Some(key_secret)) => Some(store::CloudCreds { key_id, key_secret, region: non_empty(cloud_region) }),
            (None, None) => None,
            _ => return Err("Faltan el ID o la clave secreta de la nube.".into()),
        },
    };
    let env = cloud.as_ref().map(|c| tasks::cloud_env(&location, c.region.as_deref(), Some(&c.key_id), Some(&c.key_secret))).unwrap_or_default();
    let access = restic::Access { location, password, rest_auth, cacert, env };

    // Comprobar el acceso (o crear el repositorio) antes de guardar nada.
    // Con límite de tiempo y cancelable desde la interfaz (cancel_add_repo).
    let flag = Arc::new(AtomicBool::new(false));
    let key = check_id.unwrap_or_default();
    checks.0.lock().unwrap().insert(key.clone(), flag.clone());
    let check = access.clone();
    let checked = blocking(move || {
        let args: &[&str] = if create { &["init"] } else { &["cat", "config", "--no-lock"] };
        restic::run_with(&check, args, restic::CHECK_TIMEOUT, Some(&flag)).map(|_| ())
    })
    .await;
    let cancelled = checks.0.lock().unwrap().remove(&key).is_some_and(|f| f.load(Ordering::Relaxed));
    if cancelled {
        return Err(restic::CANCELLED.into());
    }
    checked?;

    let created = store.add(name, &access, cloud)?;
    history::note(&store.config_dir(), "config", &created.id, &created.name, None, if create { "Repositorio creado." } else { "Repositorio conectado." });
    Ok(created)
}

/// Comprobaciones de conexión en curso (al añadir repositorios), para poder cancelarlas.
#[derive(Default)]
struct Checks(Mutex<HashMap<String, Arc<AtomicBool>>>);

#[tauri::command]
fn cancel_add_repo(checks: State<'_, Checks>, check_id: String) {
    if let Some(flag) = checks.0.lock().unwrap().get(&check_id) {
        flag.store(true, Ordering::Relaxed);
    }
}

/// Comprueba la contraseña del repositorio antes de una acción protegida.
async fn require_password(repo: &Repo, password: String) -> Result<(), String> {
    let repo = repo.clone();
    blocking(move || store::verify_password(&repo, &password)).await
}

#[tauri::command]
async fn remove_repo(
    store: State<'_, Store>,
    jobs: State<'_, Jobs>,
    cache: State<'_, stats_cache::StatsCache>,
    id: String,
    password: String,
) -> Result<(), String> {
    let repo = store.get(&id)?;
    require_password(&repo, password).await?;
    if jobs.any_for_repo(&id) {
        return Err("Espera a que termine la operación en curso.".into());
    }
    if agent::is_scheduled(&id) {
        if !platform::is_elevated() {
            return Err("Este repositorio tiene copias automáticas y el agente también tiene que olvidarlo: abre Resguardo como \
                        administrador y vuelve a quitarlo."
                .into());
        }
        let unscheduled = repo.clone();
        blocking(move || agent::set_schedule(&unscheduled, None, None)).await.map_err(|e| format!("No se pudieron quitar sus copias automáticas: {e}"))?;
    }
    store.remove(&id)?;
    cache.remove(&id);
    history::note(&store.config_dir(), "config", &repo.id, &repo.name, None, "Repositorio quitado de Resguardo (las versiones guardadas no se tocan).");
    Ok(())
}

#[tauri::command]
async fn list_snapshots(store: State<'_, Store>, id: String) -> Result<Vec<restic::Snapshot>, String> {
    let access = store::access(&store.get(&id)?)?;
    blocking(move || restic::snapshots(&access)).await
}

/// Guarda los planes de copia (qué, con qué etiquetas y cuándo). Cambia la
/// configuración: pide la contraseña. Si el agente ya copia este repositorio,
/// la interfaz ofrece aplicar los cambios (requiere administrador).
#[tauri::command]
async fn set_plans(store: State<'_, Store>, id: String, plans: Vec<plans::Plan>, password: String) -> Result<Repo, String> {
    let repo = store.get(&id)?;
    require_password(&repo, password).await?;
    let updated = store.set_plans(&id, plans)?;
    for (plan_id, plan_name, message) in history::plan_changes(&repo.plans, &updated.plans) {
        history::note(&store.config_dir(), "config", &updated.id, &updated.name, Some((&plan_id, &plan_name)), message);
    }
    Ok(updated)
}

/// Mueve una copia (plan) a otro destino. Cambia la configuración de los
/// dos: pide la contraseña de ambos. Si la app está abierta como
/// administrador y el agente copiaba alguno de los dos, se actualiza también.
#[tauri::command]
async fn move_plan(store: State<'_, Store>, from: String, plan: String, to: String, password_from: String, password_to: String) -> Result<Vec<Repo>, String> {
    let (src, dst) = (store.get(&from)?, store.get(&to)?);
    require_password(&src, password_from).await?;
    require_password(&dst, password_to).await?;
    let old_name = src.plans.iter().find(|p| p.id == plan).map(|p| p.name.clone()).unwrap_or_default();
    let (src, dst) = store.move_plan(&from, &plan, &to)?;
    if let Some(moved) = dst.plans.last() {
        let dir = store.config_dir();
        history::note(&dir, "config", &src.id, &src.name, Some((&plan, &old_name)), format!("Copia «{old_name}» movida a «{}».", dst.name));
        history::note(&dir, "config", &dst.id, &dst.name, Some((&moved.id, &moved.name)), format!("Copia «{}» traída desde «{}».", moved.name, src.name));
    }
    if platform::is_elevated() {
        let (a, b) = (src.clone(), dst.clone());
        let _ = blocking(move || {
            for repo in [a, b] {
                let in_agent = agent::load_config().repos.iter().any(|r| r.id == repo.id && matches!(r.schedule, agent::Schedule::Plans));
                let scheduled = repo.plans.iter().any(|p| p.schedule.is_some());
                if in_agent || scheduled {
                    let access = store::access(&repo)?;
                    // Sin copias programadas: se intenta mantener (si tiene
                    // verificación o copia externa); si no, se quita del agente.
                    if agent::set_schedule(&repo, Some(&access), Some(agent::Schedule::Plans)).is_err() {
                        agent::set_schedule(&repo, Some(&access), None)?;
                    }
                }
            }
            Ok::<(), String>(())
        })
        .await
        .map_err(|e| agent::log(&format!("ERROR: al mover una copia no se pudo actualizar el agente: {e}")));
    }
    Ok(vec![src, dst])
}

/// Plan de la copia manual en curso de cada repositorio (para pasarla al
/// agente si se cierra la app).
#[derive(Default)]
struct ManualPlans(std::sync::Mutex<std::collections::HashMap<String, String>>);

#[tauri::command]
async fn run_backup(app: AppHandle, store: State<'_, Store>, manual: State<'_, ManualPlans>, id: String, plan: String) -> Result<backup::BackupResult, String> {
    let repo = store.get(&id)?;
    let the_plan = repo.plans.iter().find(|p| p.id == plan).cloned().ok_or("Ese plan ya no existe.")?;
    let access = store::access(&repo)?;
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let (repo_id, repo_name, plan_id, plan_name) = (repo.id.clone(), repo.name.clone(), the_plan.id.clone(), the_plan.name.clone());
    let started = chrono::Local::now().to_rfc3339();
    // Si ya hay una copia de este destino en curso, `jobs` la rechazará: no se
    // cambia el plan anotado (se usa para pasarla al agente al cerrar).
    if !app.state::<Jobs>().any_for_repo(&id) {
        manual.0.lock().unwrap().insert(id.clone(), plan);
    }
    let busy = app.state::<Jobs>().any_for_repo(&id);
    let plan_for_note = the_plan.name.clone();
    let result = blocking(move || backup::run(&app, &app.state::<Jobs>(), &repo, &the_plan, &access)).await;
    // Versión nueva: el agente la tiene en cuenta para la copia externa
    // «después de cada copia» y para el freno ante cambios inusuales.
    if let Ok(r) = &result {
        if let Some(s) = r.summary.as_ref().filter(|_| !r.unchanged) {
            if let Some(id) = &s.snapshot_id {
                let (rid, sid, (added, new, changed)) = (repo_id.clone(), id.clone(), (s.data_added, s.files_new, s.files_changed));
                let _ = blocking(move || {
                    tasks::note_manual_backup(&rid, &plan_for_note, &sid, added, (new, changed));
                    Ok(())
                })
                .await;
            }
        }
    }
    // Historial (no si se rechazó por haber otra copia en curso).
    if !busy {
        let (res, message, summary) = match &result {
            Ok(r) if r.incomplete => (
                "warning",
                if r.error_count == 1 {
                    "Copia terminada con 1 archivo que no se pudo leer.".to_string()
                } else {
                    format!("Copia terminada con {} archivos que no se pudieron leer.", r.error_count)
                },
                r.summary.clone(),
            ),
            Ok(r) if r.unchanged => ("ok", backup::UNCHANGED_MESSAGE.to_string(), r.summary.clone()),
            Ok(r) => ("ok", "Copia completada.".to_string(), r.summary.clone()),
            Err(e) => ("error", e.clone(), None),
        };
        history::append(
            &history::app_file(&config_dir),
            &history::Entry {
                kind: "backup".into(),
                origin: "manual".into(),
                repo_id,
                repo_name,
                plan_id: Some(plan_id),
                plan_name: Some(plan_name),
                started,
                finished: chrono::Local::now().to_rfc3339(),
                result: res.into(),
                message,
                snapshot_id: summary.as_ref().and_then(|s| s.snapshot_id.clone()),
                data_added: summary.as_ref().map(|s| s.data_added),
                files_new: summary.as_ref().map(|s| s.files_new),
                files_changed: summary.as_ref().map(|s| s.files_changed),
                unchanged: result.as_ref().is_ok_and(|r| r.unchanged),
                requested_from: None,
                user: history::local_user(),
                ganchos: vec![],
            },
        );
    }
    result
}

#[tauri::command]
fn cancel_backup(jobs: State<'_, Jobs>, id: String) -> Result<(), String> {
    jobs.cancel(&id)
}

/// `restic unlock`: quita solo los bloqueos antiguos (los de operaciones que
/// ya no están en marcha). Nunca `--remove-all`.
#[tauri::command]
async fn unlock_repo(store: State<'_, Store>, id: String) -> Result<(), String> {
    let access = store::access(&store.get(&id)?)?;
    blocking(move || {
        let out = restic::run_raw(&access, &["unlock"], restic::CHECK_TIMEOUT)?;
        if out.code == Some(0) {
            agent::log(&format!("Bloqueos antiguos quitados del repositorio {id}."));
            Ok(())
        } else {
            Err(restic::exit_error(out.code, &out.stderr))
        }
    })
    .await
}

/// Cambia la contraseña guardada de un repositorio cuyo dueño la cambió en
/// otro sitio. Antes comprueba que la nueva lo abre. Devuelve "agente" si
/// también se actualizó la del agente, "agente-admin" si el agente la usa y
/// hace falta abrir la app como administrador, o "app".
#[tauri::command]
async fn update_saved_password(store: State<'_, Store>, id: String, password: String) -> Result<String, String> {
    let repo = store.get(&id)?;
    if password.is_empty() {
        return Err("Escribe la contraseña nueva.".into());
    }
    let mut access = store::access(&repo).unwrap_or_else(|_| restic::Access::new(repo.location.clone(), ""));
    access.password = password.clone();
    blocking(move || restic::run(&access, &["cat", "config", "--no-lock"]).map(|_| ())).await.map_err(|e| {
        if e.contains("Contraseña incorrecta") {
            "Esa contraseña no abre el repositorio: no se ha cambiado nada.".to_string()
        } else {
            e
        }
    })?;
    store::set_password(&id, &password)?;
    let in_agent = agent::load_config().repos.iter().any(|r| r.id == id);
    if !in_agent {
        return Ok("app".into());
    }
    if !platform::is_elevated() {
        return Ok("agente-admin".into());
    }
    blocking(move || agent::update_password(&id, &password)).await?;
    Ok("agente".into())
}

/// Solo comprueba la contraseña (para pedirla antes de lanzar una acción larga).
#[tauri::command]
async fn check_password(store: State<'_, Store>, id: String, password: String) -> Result<(), String> {
    require_password(&store.get(&id)?, password).await
}

#[tauri::command]
async fn list_snapshot_dir(store: State<'_, Store>, id: String, snapshot: String, dir: String) -> Result<Vec<restic::Entry>, String> {
    let access = store::access(&store.get(&id)?)?;
    blocking(move || restic::list_dir(&access, &snapshot, &dir)).await
}

/// Carpetas y archivos más grandes de una versión (solo lectura).
#[tauri::command]
async fn snapshot_largest(store: State<'_, Store>, id: String, snapshot: String, limit: Option<usize>) -> Result<sizes::Largest, String> {
    let access = store::access(&store.get(&id)?)?;
    blocking(move || sizes::largest(&access, &snapshot, limit.unwrap_or(50))).await
}

#[derive(serde::Serialize)]
struct TargetInfo {
    exists: bool,
    empty: bool,
}

/// ¿Existe la carpeta de destino y tiene contenido? Decide si hay algo que reemplazar.
#[tauri::command]
fn inspect_target(path: String) -> TargetInfo {
    match std::fs::read_dir(&path) {
        Ok(mut entries) => TargetInfo { exists: true, empty: entries.next().is_none() },
        Err(_) => TargetInfo { exists: std::path::Path::new(&path).exists(), empty: true },
    }
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn run_restore(
    app: AppHandle,
    store: State<'_, Store>,
    id: String,
    snapshot: String,
    dir: String,
    names: Vec<String>,
    target: String,
    overwrite: bool,
    password: Option<String>,
) -> Result<restore::RestoreResult, String> {
    let repo = store.get(&id)?;
    // Reemplazar archivos existentes es destructivo: exige la contraseña.
    if overwrite {
        require_password(&repo, password.unwrap_or_default()).await?;
    }
    let access = store::access(&repo)?;
    let req = restore::Request { snapshot, dir, names, target, overwrite };
    blocking(move || restore::run(&app, &app.state::<Jobs>(), &repo, &access, &req)).await
}

#[tauri::command]
async fn retention_preview(store: State<'_, Store>, id: String, policy: retention::Policy) -> Result<retention::Preview, String> {
    let access = store::access(&store.get(&id)?)?;
    blocking(move || retention::preview(&access, &policy)).await
}

/// Guardar la política cambia la configuración del repositorio: pide la contraseña.
#[tauri::command]
async fn set_retention(store: State<'_, Store>, id: String, policy: Option<retention::Policy>, password: String) -> Result<Repo, String> {
    let repo = store.get(&id)?;
    require_password(&repo, password).await?;
    let updated = store.set_retention(&id, policy)?;
    let message = match (&repo.retention, &updated.retention) {
        (_, None) => "Retención quitada: se guardan todas las versiones.",
        (None, Some(_)) => "Retención definida.",
        (Some(_), Some(_)) => "Retención cambiada.",
    };
    history::note(&store.config_dir(), "config", &updated.id, &updated.name, None, message);
    // Como administrador, el agente (y la web) saben que tiene retención.
    if platform::is_elevated() {
        let r = updated.clone();
        let _ = blocking(move || agent::sync_meta(&r)).await;
    }
    Ok(updated)
}

/// Espacio en disco. `snapshot_ids` es la lista que ve la interfaz: si no ha
/// cambiado desde el último cálculo se devuelve el resultado guardado sin
/// ejecutar restic. `force` obliga a recalcular.
#[tauri::command]
async fn repo_stats(
    store: State<'_, Store>,
    cache: State<'_, stats_cache::StatsCache>,
    id: String,
    snapshot_ids: Vec<String>,
    force: bool,
) -> Result<stats_cache::CachedStats, String> {
    let key = stats_cache::key(&snapshot_ids);
    if !force {
        if let Some(hit) = cache.get(&id, key) {
            return Ok(hit);
        }
    }
    let access = store::access(&store.get(&id)?)?;
    let stats = blocking(move || restic::stats(&access)).await?;
    Ok(cache.put(&id, key, stats))
}

/// Datos del kit de recuperación de los destinos indicados (todos si no se
/// indica ninguno): sin secretos. Lee el ID de cada repositorio con restic.
#[tauri::command]
async fn recovery_info(store: State<'_, Store>, ids: Option<Vec<String>>) -> Result<Vec<kit::KitEntry>, String> {
    let repos: Vec<Repo> = store.list()?.into_iter().filter(|r| ids.as_ref().is_none_or(|ids| ids.contains(&r.id))).collect();
    let mut out = Vec::new();
    for repo in repos {
        let access = store::access(&repo).ok();
        out.push(blocking(move || Ok(kit::entry(&repo, access.as_ref()))).await?);
    }
    Ok(out)
}

/// «Ya lo guardé en un lugar seguro»: anota el kit de un destino. Cambia la
/// configuración (y la salud de la protección que ve la web): pide la
/// contraseña del destino. Como administrador, también lo sabe el agente.
#[tauri::command]
async fn kit_confirm(store: State<'_, Store>, id: String, config_id: Option<String>, password: String) -> Result<Repo, String> {
    let repo = store.get(&id)?;
    // Cambia la configuración (y lo que ve la web): con la contraseña del destino.
    require_password(&repo, password).await?;
    let status = kit::KitStatus { saved_at: chrono::Local::now().to_rfc3339(), location: repo.location.clone(), config_id };
    let updated = store.set_kit(&id, Some(status.clone()))?;
    let _ = status;
    history::note(&store.config_dir(), "kit", &updated.id, &updated.name, None, "Kit de recuperación guardado.");
    if platform::is_elevated() && agent::is_scheduled(&id) {
        let repo = updated.clone();
        let _ = blocking(move || {
            agent::sync_meta(&repo)?;
            if web::load_link().is_some_and(|l| !l.revoked) {
                let _ = web::report_now();
            }
            Ok(())
        })
        .await;
    }
    Ok(updated)
}

/// Salud de la protección de un destino (ver protection.rs). `last_snapshot`:
/// la versión más reciente que conoce la interfaz.
#[tauri::command]
async fn protection_status(store: State<'_, Store>, id: String, last_snapshot: Option<String>) -> Result<protection::Protection, String> {
    let repo = store.get(&id)?;
    blocking(move || {
        let now = chrono::Local::now();
        let state = agent::load_state();
        let mut f = protection::agent_facts(&repo.id, &repo.location, &agent::load_config(), &state, &tasks::load_state(), &tasks::load_guard(), now);
        // Lo de la app manda: retención, kit, bloqueo de objetos y la comprobación del servidor más reciente.
        f.has_retention = repo.retention.is_some();
        f.object_lock = repo.object_lock;
        f.kit_ok = repo.kit.as_ref().is_some_and(|k| k.matches(&repo.location, None));
        f.kit_stale = repo.kit.is_some() && !f.kit_ok;
        let agent_check = state.append_only.get(&repo.id);
        let newest = match (repo.append_only.as_ref(), agent_check) {
            (Some(a), Some(b)) => Some(if a.checked_at >= b.checked_at { a } else { b }),
            (a, b) => a.or(b),
        };
        f.append_only = newest.and_then(|c| c.append_only);
        if last_snapshot.is_some() {
            f.last_snapshot = last_snapshot;
        }
        Ok(protection::evaluate(&f, now))
    })
    .await
}

/// ¿El servidor REST es de solo añadir? Lo comprueba (sin borrar nada) si
/// hace más de un día de la última vez, o si se fuerza.
#[tauri::command]
async fn probe_append_only(store: State<'_, Store>, id: String, force: Option<bool>) -> Result<Repo, String> {
    let repo = store.get(&id)?;
    if !repo.location.starts_with("rest:") || (!force.unwrap_or(false) && repo.append_only.as_ref().is_some_and(|c| c.fresh(chrono::Local::now()))) {
        return Ok(repo);
    }
    let access = store::access(&repo)?;
    let result = blocking(move || {
        let auth = access.rest_auth.as_ref().map(|(u, p)| (u.as_str(), p.as_str()));
        Ok(protection::probe_append_only(&access.location, auth, access.cacert.as_deref()))
    })
    .await?;
    store.update(&id, |r| r.append_only = Some(protection::AppendOnlyCheck { checked_at: chrono::Local::now().to_rfc3339(), append_only: result }))
}

/// El bucket tiene bloqueo de objetos (lo declara el usuario). Pide la contraseña.
#[tauri::command]
async fn set_object_lock(store: State<'_, Store>, id: String, on: bool, password: String) -> Result<Repo, String> {
    let repo = store.get(&id)?;
    require_password(&repo, password).await?;
    let updated = store.update(&id, |r| r.object_lock = on)?;
    if repo.object_lock != on {
        let message = if on { "Marcado como bucket con bloqueo de objetos." } else { "Ya no se cuenta con bloqueo de objetos." };
        history::note(&store.config_dir(), "config", &updated.id, &updated.name, None, message);
    }
    if platform::is_elevated() {
        let r = updated.clone();
        let _ = blocking(move || agent::sync_meta(&r)).await;
    }
    Ok(updated)
}

/// Destinos (lugares) con sus repositorios (ver docs/destinos.md).
#[tauri::command]
fn list_places(store: State<'_, Store>) -> Result<Vec<places::Place>, String> {
    store.places()
}

/// Cambia el nombre de un destino. Solo es su nombre visible: no toca ningún
/// repositorio, así que no pide contraseña. Como administrador se refleja en
/// el agente (y en la web).
#[tauri::command]
async fn rename_place(store: State<'_, Store>, id: String, name: String) -> Result<places::Place, String> {
    let place = store.rename_place(&id, &name)?;
    if platform::is_elevated() {
        if let Ok(repos) = store.list() {
            let _ = blocking(move || agent::sync_all_meta(&repos).map(|_| ())).await;
        }
    }
    Ok(place)
}

/// Lo que hay en un destino: los repositorios encontrados al listarlo (carpeta
/// o bucket S3), los que ya usa este equipo y los que recuerda. Solo lee.
#[derive(serde::Serialize)]
struct PlaceScan {
    found: Vec<discover::Found>,
    /// Por qué no se pudo listar (o qué se muestra en su lugar).
    note: Option<String>,
}

#[tauri::command]
async fn place_scan(store: State<'_, Store>, id: String) -> Result<PlaceScan, String> {
    let place = store.places()?.into_iter().find(|p| p.id == id).ok_or("Ese destino ya no existe.")?;
    let repos = store.list()?;
    blocking(move || {
        let template = repos.iter().find(|r| r.place_id.as_deref() == Some(place.id.as_str())).cloned();
        let (found, note) = discover::scan(&place, &repos, template.as_ref())?;
        Ok(PlaceScan { found, note })
    })
    .await
}

/// Clona un repositorio a una ubicación nueva de otro destino (o del mismo):
/// crea el repositorio con su contraseña nueva (`password_new`) y copia todas
/// las versiones con `restic copy`. Usa las credenciales de un repositorio de
/// ese destino (`template`). Pide la contraseña del de origen, porque saca sus
/// datos a otro sitio. Al terminar lo añade a Resguardo.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn clone_repo(
    app: AppHandle,
    store: State<'_, Store>,
    from: String,
    template: String,
    location: String,
    name: String,
    password_new: String,
    password: String,
) -> Result<Repo, String> {
    let src_repo = store.get(&from)?;
    require_password(&src_repo, password).await?;
    let name = name.trim().to_string();
    let location = location.trim().to_string();
    if name.is_empty() || location.is_empty() {
        return Err("El nombre y la ubicación son obligatorios.".into());
    }
    if password_new.chars().count() < 12 {
        return Err("La contraseña del repositorio nuevo debe tener al menos 12 caracteres.".into());
    }
    if has_embedded_password(&location) {
        return Err("No incluyas contraseñas dentro de la ubicación.".into());
    }
    let tpl = store.get(&template)?;
    if places::key(&tpl.location) != places::key(&location) {
        return Err("La ubicación nueva no está en ese destino.".into());
    }
    let src = store::access(&src_repo)?;
    let tpl_access = store::access(&tpl)?;
    let cloud = store::cloud_creds(&tpl)?;
    let env = cloud.as_ref().map(|c| tasks::cloud_env(&location, c.region.as_deref(), Some(&c.key_id), Some(&c.key_secret))).unwrap_or_default();
    let dest = restic::Access { location, password: password_new, rest_auth: tpl_access.rest_auth.clone(), cacert: tpl_access.cacert.clone(), env };
    let (handle, from_id, dest2) = (app.clone(), from.clone(), dest.clone());
    let copied = blocking(move || {
        discover::clone(&handle.state::<Jobs>(), &from_id, &src, &dest2, |p| {
            let _ = handle.emit(discover::PROGRESS_EVENT, p);
        })
    })
    .await?;
    let created = store.add(name, &dest, cloud)?;
    history::note(
        &store.config_dir(),
        "config",
        &created.id,
        &created.name,
        None,
        format!("Repositorio creado como clon de «{}» ({copied} versiones copiadas).", src_repo.name),
    );
    Ok(created)
}

#[tauri::command]
fn cancel_clone(jobs: State<'_, Jobs>, from: String) -> Result<(), String> {
    jobs.cancel(&discover::job_key(&from))
}

// ---------- Compartir un destino entre mis equipos (fase 3) ----------

#[derive(serde::Serialize)]
struct ShareStatus {
    /// Es de la nube o un rest-server (se puede compartir).
    shareable: bool,
    shared: bool,
    since: Option<String>,
    /// Entregas: (equipo, cuándo).
    delivered: Vec<(String, String)>,
    /// Este equipo está vinculado a Resguardo Web (hace falta para compartir).
    linked: bool,
    elevated: bool,
}

fn place_of(store: &Store, place_id: &str) -> Result<(places::Place, Vec<Repo>), String> {
    let place = store.places()?.into_iter().find(|p| p.id == place_id).ok_or("Ese destino ya no existe.")?;
    let repos: Vec<Repo> = store.list()?.into_iter().filter(|r| r.place_id.as_deref() == Some(place_id)).collect();
    Ok((place, repos))
}

#[tauri::command]
async fn place_share_status(store: State<'_, Store>, id: String) -> Result<ShareStatus, String> {
    let (_, repos) = place_of(&store, &id)?;
    let shareable = repos.first().is_some_and(|r| matches!(places::kind(&r.location), "s3" | "b2" | "azure" | "gs" | "rest"));
    let linked = web::load_link().is_some_and(|l| !l.revoked);
    let elevated = platform::is_elevated();
    // Solo un administrador puede leer lo que guarda el agente.
    let shared = if elevated { share::load().ok().and_then(|v| v.shared.get(&id).cloned()) } else { None };
    Ok(ShareStatus {
        shareable,
        shared: shared.is_some(),
        since: shared.as_ref().map(|s| s.since.clone()),
        delivered: shared.map(|s| s.delivered).unwrap_or_default(),
        linked,
        elevated,
    })
}

/// Comparte (o deja de compartir) un destino con los equipos del usuario.
/// Pide administrador (el agente guarda las credenciales) y la contraseña de
/// un repositorio del destino (prueba de que se tiene autoridad sobre él).
#[tauri::command]
async fn place_share_set(store: State<'_, Store>, id: String, on: bool, repo: String, password: String) -> Result<ShareStatus, String> {
    agent::require_admin()?;
    let (place, repos) = place_of(&store, &id)?;
    let template = repos.iter().find(|r| r.id == repo).cloned().ok_or("Ese repositorio no está en este destino.")?;
    require_password(&template, password).await?;
    let link = web::load_link().filter(|l| !l.revoked).ok_or("Vincula antes este equipo con Resguardo Web (Ajustes → Este equipo).")?;
    let secret = web::device_secret(&agent::load_secrets()?).ok_or("Falta el secreto del equipo: vuelve a vincularlo con la web.")?;
    let name = place.name.clone();
    let place_id = id.clone();
    let dir = store.config_dir();
    blocking(move || {
        let mut vault = share::load()?;
        if on {
            let (meta, creds) = share::from_repo(&template, &name)?;
            share::ensure_key(&mut vault);
            let share_id = share::publish(&link, &secret, &place_id, Some(&meta))?;
            vault
                .shared
                .insert(place_id.clone(), share::SharedPlace { share_id, meta, creds, since: chrono::Local::now().to_rfc3339(), delivered: Vec::new() });
            share::save(&vault)?;
            history::note(&dir, "config", &template.id, &template.name, None, format!("Destino «{name}» compartido con mis equipos."));
        } else {
            share::publish(&link, &secret, &place_id, None)?;
            vault.shared.remove(&place_id);
            share::save(&vault)?;
            history::note(&dir, "config", &template.id, &template.name, None, format!("Destino «{name}» ya no se comparte."));
        }
        Ok(())
    })
    .await?;
    place_share_status(store, id).await
}

/// Destinos compartidos por mis otros equipos (desde la web, con la sesión de «Todos mis equipos»).
#[tauri::command]
async fn shares_available(app: AppHandle) -> Result<serde_json::Value, String> {
    let link = web::load_link().filter(|l| !l.revoked);
    let mut v = blocking(move || app.state::<account::Account>().shares()).await?;
    v["this_device_id"] = serde_json::json!(link.map(|l| l.device_id));
    Ok(v)
}

/// Pide un destino compartido para este equipo.
#[tauri::command]
async fn share_request(app: AppHandle, share: String) -> Result<serde_json::Value, String> {
    let link = web::load_link()
        .filter(|l| !l.revoked)
        .ok_or("Vincula antes este equipo con Resguardo Web (Ajustes → Este equipo): el destino llega cifrado para él.")?;
    blocking(move || app.state::<account::Account>().request_share(&share, &link.device_id)).await
}

/// Cancela una petición pendiente (mientras el otro equipo aún no la entregó).
#[tauri::command]
async fn share_cancel(app: AppHandle, request: String) -> Result<serde_json::Value, String> {
    blocking(move || app.state::<account::Account>().cancel_share(&request)).await
}

#[derive(serde::Serialize)]
struct ReceivedShare {
    id: String,
    meta: share::ShareMeta,
    base: String,
    from_device: String,
    received_at: String,
}

/// Destinos recibidos de mis otros equipos (sin secretos). Requiere administrador.
#[tauri::command]
async fn shares_received() -> Result<Vec<ReceivedShare>, String> {
    agent::require_admin()?;
    let vault = blocking(share::load).await?;
    let mut out: Vec<ReceivedShare> = vault
        .received
        .into_iter()
        .map(|(id, r)| ReceivedShare { id, base: r.creds.base.clone(), meta: r.meta, from_device: r.from_device, received_at: r.received_at })
        .collect();
    out.sort_by(|a, b| b.received_at.cmp(&a.received_at));
    Ok(out)
}

/// Crea (o conecta) un repositorio en un destino recibido, con sus credenciales.
/// La ubicación tiene que estar dentro del destino. Requiere administrador.
#[tauri::command]
async fn received_share_add(store: State<'_, Store>, id: String, name: String, location: String, password: String, create: bool) -> Result<Repo, String> {
    agent::require_admin()?;
    let vault = blocking(share::load).await?;
    let r = vault.received.get(&id).cloned().ok_or("Ese destino compartido ya no está en este equipo.")?;
    let location = location.trim().to_string();
    if places::key(&location) != places::key(&r.creds.base) {
        return Err("La ubicación tiene que estar dentro del destino compartido.".into());
    }
    if name.trim().is_empty() || password.is_empty() {
        return Err("El nombre y la contraseña son obligatorios.".into());
    }
    let cloud = match (&r.creds.key_id, &r.creds.key_secret) {
        (Some(k), Some(s)) => Some(store::CloudCreds { key_id: k.clone(), key_secret: s.clone(), region: r.creds.region.clone() }),
        _ => None,
    };
    let env = cloud.as_ref().map(|c| tasks::cloud_env(&location, c.region.as_deref(), Some(&c.key_id), Some(&c.key_secret))).unwrap_or_default();
    // Certificado propio del servidor: se guarda junto a la configuración de la app.
    let cacert = match &r.creds.tls_cert_pem {
        Some(pem) => {
            let path = store.config_dir().join(format!("ca-{}.pem", id.chars().filter(|c| c.is_ascii_alphanumeric()).take(16).collect::<String>()));
            std::fs::write(&path, pem).map_err(|e| format!("No se pudo guardar el certificado: {e}"))?;
            Some(path.display().to_string())
        }
        None => None,
    };
    let rest_auth = r.creds.rest_user.clone().zip(r.creds.rest_password.clone());
    let access = restic::Access { location, password, rest_auth, cacert, env };
    let check = access.clone();
    blocking(move || {
        let args: &[&str] = if create { &["init"] } else { &["cat", "config", "--no-lock"] };
        restic::run_with(&check, args, restic::CHECK_TIMEOUT, None).map(|_| ())
    })
    .await?;
    let created = store.add(name.trim().to_string(), &access, cloud)?;
    history::note(
        &store.config_dir(),
        "config",
        &created.id,
        &created.name,
        None,
        format!("Repositorio {} en el destino compartido «{}» (desde «{}»).", if create { "creado" } else { "conectado" }, r.meta.name, r.from_device),
    );
    Ok(created)
}

// ---------- Servidor de copias (fase 4) ----------

#[derive(serde::Serialize)]
struct ServerUserInfo {
    name: String,
    created_at: String,
    repos: Vec<String>,
    /// Se ofrece a mis equipos (entrega cifrada, fase 3).
    shared: bool,
}

#[derive(serde::Serialize)]
struct ServerStatus {
    supported: bool,
    elevated: bool,
    /// None si el binario está y su huella coincide; si no, por qué no.
    binary_problem: Option<String>,
    enabled: bool,
    path: String,
    port: u16,
    local_subnet_only: bool,
    running: bool,
    lan_addresses: Vec<String>,
    tls_sha256: Option<String>,
    users: Vec<ServerUserInfo>,
    linked: bool,
}

fn server_status_now() -> ServerStatus {
    let c = server::load();
    let vault = if platform::is_elevated() { share::load().ok() } else { None };
    let repos = server::repos_by_user(&c);
    ServerStatus {
        supported: cfg!(windows),
        elevated: platform::is_elevated(),
        binary_problem: server::binary_check().err(),
        enabled: c.enabled,
        path: c.path.clone(),
        port: c.port,
        local_subnet_only: c.local_subnet_only,
        running: c.enabled && server::listening(c.port),
        lan_addresses: server::lan_addresses(),
        tls_sha256: c.tls_sha256.clone(),
        users: c
            .users
            .iter()
            .map(|u| ServerUserInfo {
                name: u.name.clone(),
                created_at: u.created_at.clone(),
                repos: repos.iter().find(|(n, _)| n == &u.name).map(|(_, r)| r.clone()).unwrap_or_default(),
                shared: vault.as_ref().is_some_and(|v| v.shared.contains_key(&format!("server:{}", u.name))),
            })
            .collect(),
        linked: web::load_link().is_some_and(|l| !l.revoked),
    }
}

#[tauri::command]
async fn server_status() -> Result<ServerStatus, String> {
    blocking(|| Ok(server_status_now())).await
}

/// Activa (o cambia) el Servidor de copias: carpeta, puerto y si solo se
/// acepta la red local. Genera el certificado, la regla del firewall y la
/// tarea que lo arranca. Requiere administrador.
#[tauri::command]
async fn server_setup(path: String, port: u16, local_subnet_only: bool) -> Result<ServerStatus, String> {
    agent::require_admin()?;
    blocking(move || {
        server::activar(&path, port, local_subnet_only)?;
        Ok(server_status_now())
    })
    .await
}

/// Desactiva el Servidor de copias. Las copias guardadas se quedan en la carpeta.
#[tauri::command]
async fn server_disable() -> Result<ServerStatus, String> {
    agent::require_admin()?;
    blocking(|| {
        server::desactivar()?;
        Ok(server_status_now())
    })
    .await
}

#[derive(serde::Serialize)]
struct NewServerUser {
    status: ServerStatus,
    user: String,
    /// La contraseña, para mostrarla una vez (también llega cifrada a los equipos que la pidan).
    password: String,
    location: String,
}

/// Añade un equipo cliente: su usuario y contraseña (128 bits) y, si este
/// equipo está vinculado, lo ofrece a mis equipos con la entrega cifrada.
#[tauri::command]
async fn server_add_user(name: String) -> Result<NewServerUser, String> {
    agent::require_admin()?;
    blocking(move || {
        let (user, password, location) = server::anadir_equipo(&name)?;
        let c = server::load();
        let ip = server::lan_addresses().into_iter().next().unwrap_or_else(|| "localhost".into());
        // Ofrecerlo a mis equipos (fase 3): el cliente lo pide y le llega cifrado.
        let mut vault = share::load()?;
        let meta = share::ShareMeta {
            kind: "rest".into(),
            host: Some(format!("{ip}:{}", c.port)),
            base: format!("/{user}/"),
            name: format!("Servidor de copias de {} · {user}", web::default_device_name()),
        };
        let creds = share::PlaceCreds {
            kind: "rest".into(),
            base: location.clone(),
            rest_user: Some(user.clone()),
            rest_password: Some(password.clone()),
            tls_cert_pem: std::fs::read_to_string(server::cert_file()).ok(),
            ..Default::default()
        };
        let share_id = match (web::load_link().filter(|l| !l.revoked), web::device_secret(&agent::load_secrets()?)) {
            (Some(link), Some(secret)) => share::publish(&link, &secret, &format!("server:{user}"), Some(&meta)).ok().flatten(),
            _ => None,
        };
        vault
            .shared
            .insert(format!("server:{user}"), share::SharedPlace { share_id, meta, creds, since: chrono::Local::now().to_rfc3339(), delivered: Vec::new() });
        share::save(&vault)?;
        Ok(NewServerUser { status: server_status_now(), user, password, location })
    })
    .await
}

/// Dirección pública de esta red (solo si se pide: consulta un servicio externo).
#[tauri::command]
async fn server_public_ip() -> Result<String, String> {
    blocking(|| {
        let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(8))).build().into();
        let ip = agent
            .get("https://api.ipify.org")
            .call()
            .map_err(|e| format!("No se pudo consultar: {e}"))?
            .body_mut()
            .read_to_string()
            .map_err(|e| e.to_string())?;
        let ip = ip.trim().to_string();
        if ip.parse::<std::net::IpAddr>().is_ok() {
            Ok(ip)
        } else {
            Err("Respuesta inesperada.".into())
        }
    })
    .await
}

/// Quita un equipo cliente: ya no puede entrar. Sus copias se quedan en la carpeta.
#[tauri::command]
async fn server_remove_user(name: String) -> Result<ServerStatus, String> {
    agent::require_admin()?;
    blocking(move || {
        server::quitar_equipo(&name)?;
        let mut vault = share::load()?;
        if vault.shared.remove(&format!("server:{name}")).is_some() {
            if let (Some(link), Some(secret)) = (web::load_link().filter(|l| !l.revoked), web::device_secret(&agent::load_secrets()?)) {
                let _ = share::publish(&link, &secret, &format!("server:{name}"), None);
            }
        }
        share::save(&vault)?;
        agent::log(&format!("Servidor de copias: equipo cliente «{name}» quitado."));
        Ok(server_status_now())
    })
    .await
}

// ---------- Equipos gestionados: consola (fase 5) ----------

#[tauri::command]
async fn managed_list() -> Result<Vec<console::EndpointInfo>, String> {
    agent::require_admin()?;
    blocking(console::list).await
}

#[tauri::command]
async fn managed_pair_start() -> Result<console::PairingStart, String> {
    agent::require_admin()?;
    blocking(console::pair_start).await
}

#[tauri::command]
async fn managed_pair_poll(pairing: String) -> Result<console::PairingPoll, String> {
    agent::require_admin()?;
    blocking(move || console::pair_poll(&pairing)).await
}

#[tauri::command]
async fn managed_pair_confirm(pairing: String) -> Result<console::EndpointInfo, String> {
    agent::require_admin()?;
    blocking(move || console::pair_confirm(&pairing)).await
}

#[tauri::command]
async fn managed_pair_cancel(pairing: String) -> Result<(), String> {
    agent::require_admin()?;
    blocking(move || console::pair_cancel(&pairing)).await
}

#[tauri::command]
async fn managed_set_config(device: String, plans: Vec<plans::Plan>, tray: bool, tray_toasts: bool) -> Result<console::EndpointInfo, String> {
    agent::require_admin()?;
    blocking(move || console::set_config(&device, plans, tray, tray_toasts)).await
}

/// El instalador del agente que acompaña a la app (si esta versión lo incluye).
fn agent_installer(app: &AppHandle) -> Option<std::path::PathBuf> {
    let p = app.path().resource_dir().ok()?.join("agente").join("Resguardo-Agente-setup.exe");
    p.is_file().then_some(p)
}

/// Huella del instalador del agente fijada al compilar la app (`npm run build:todo`).
const AGENT_INSTALLER_SHA256: Option<&str> = option_env!("RESGUARDO_AGENT_INSTALLER_SHA256");

#[derive(serde::Serialize)]
struct AgentInstallerInfo {
    available: bool,
    version: String,
    /// Nombre con el que se guarda.
    file_name: String,
}

#[tauri::command]
fn managed_agent_installer(app: AppHandle) -> AgentInstallerInfo {
    let version = app.package_info().version.to_string();
    AgentInstallerInfo { available: agent_installer(&app).is_some(), file_name: format!("Resguardo-Agente-{version}-setup.exe"), version }
}

#[derive(serde::Serialize)]
struct SavedInstaller {
    path: String,
    sha256: String,
}

/// «Guardar el instalador del agente…»: lo copia (comprobando su huella) a la
/// carpeta elegida, p. ej. una memoria USB.
#[tauri::command]
async fn managed_save_agent_installer(app: AppHandle, folder: String) -> Result<SavedInstaller, String> {
    let source = agent_installer(&app).ok_or("Esta versión de Resguardo no incluye el instalador del agente.")?;
    let name = managed_agent_installer(app.clone()).file_name;
    blocking(move || {
        use sha2::Digest;
        let bytes = std::fs::read(&source).map_err(|e| format!("No se pudo leer el instalador del agente: {e}"))?;
        let sha256: String = sha2::Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
        if let Some(pinned) = AGENT_INSTALLER_SHA256 {
            if !pinned.trim().eq_ignore_ascii_case(&sha256) {
                return Err("El instalador del agente no es el que acompaña a esta versión (la huella no coincide). Reinstala Resguardo.".into());
            }
        }
        let dir = std::path::Path::new(&folder);
        if !dir.is_dir() {
            return Err("Elige una carpeta.".into());
        }
        let dest = dir.join(&name);
        std::fs::write(&dest, &bytes).map_err(|e| format!("No se pudo guardar en {}: {e}", dir.display()))?;
        Ok(SavedInstaller { path: dest.display().to_string(), sha256 })
    })
    .await
}

#[tauri::command]
async fn managed_set_retention(device: String, retention: String) -> Result<console::EndpointInfo, String> {
    agent::require_admin()?;
    blocking(move || console::set_retention(&device, &retention)).await
}

/// Aplica ya la retención de un equipo en el servidor (puede tardar).
#[tauri::command]
async fn managed_prune_now(device: String) -> Result<(), String> {
    agent::require_admin()?;
    blocking(move || console::prune(&device)).await
}

#[tauri::command]
async fn managed_backup_now(device: String, plan: String) -> Result<(), String> {
    agent::require_admin()?;
    blocking(move || console::backup_now(&device, &plan)).await
}

#[tauri::command]
async fn managed_unpair(device: String) -> Result<console::EndpointInfo, String> {
    agent::require_admin()?;
    blocking(move || console::unpair(&device)).await
}

/// Restaurar desde la consola: añade el repositorio del equipo a Resguardo
/// (por localhost) para explorarlo y restaurar como cualquier otro.
#[tauri::command]
async fn managed_open_repo(store: State<'_, Store>, device: String) -> Result<Repo, String> {
    agent::require_admin()?;
    let (name, access) = blocking(move || console::repo_access(&device)).await?;
    if let Some(existing) = store.list()?.into_iter().find(|r| r.location == access.location) {
        return Ok(existing);
    }
    store.add(format!("{name} (equipo gestionado)"), &access, None)
}

/// Cambiar el nombre que se muestra en la app. Es una modificación: pide la contraseña.
#[tauri::command]
async fn rename_repo(store: State<'_, Store>, id: String, name: String, password: String) -> Result<Repo, String> {
    let repo = store.get(&id)?;
    require_password(&repo, password).await?;
    let updated = store.rename(&id, &name)?;
    if updated.name != repo.name {
        history::note(&store.config_dir(), "config", &updated.id, &updated.name, None, format!("Nombre cambiado: «{}» → «{}».", repo.name, updated.name));
    }
    Ok(updated)
}

#[tauri::command]
async fn snapshot_diff(store: State<'_, Store>, id: String, from: String, to: String) -> Result<diff::Diff, String> {
    let access = store::access(&store.get(&id)?)?;
    blocking(move || diff::diff(&access, &from, &to)).await
}

/// Frecuencia esperada de copias (para los avisos). Cambia la configuración: pide la contraseña.
#[tauri::command]
async fn set_expected_interval(store: State<'_, Store>, id: String, hours: Option<u32>, password: String) -> Result<Repo, String> {
    let repo = store.get(&id)?;
    require_password(&repo, password).await?;
    let updated = store.set_expected_hours(&id, hours)?;
    if updated.expected_hours != repo.expected_hours {
        let message = match hours {
            Some(h) => format!("Ritmo esperado de las copias: cada {h} h."),
            None => "Ritmo esperado de las copias: según su historial.".to_string(),
        };
        history::note(&store.config_dir(), "config", &updated.id, &updated.name, None, message);
    }
    Ok(updated)
}

#[derive(serde::Serialize)]
struct AgentInfo {
    /// El modo agente solo existe en Windows por ahora.
    supported: bool,
    /// La app se abrió como administrador (necesario para programar).
    elevated: bool,
    task_installed: bool,
    repos: Vec<agent::AgentRepo>,
    /// «Copias a distancia» activadas en este equipo.
    remote_backup: bool,
    /// «Modo discreto» (None: desactivado).
    discreet: Option<discreto::Discreet>,
    state: agent::AgentState,
    /// Verificaciones y copias externas: últimas ejecuciones y la que esté en curso.
    tasks: tasks::TasksState,
    /// Subidas a la nube frenadas por un cambio inusual, por destino.
    offsite_holds: std::collections::HashMap<String, tasks::Hold>,
}

#[tauri::command]
async fn agent_info() -> Result<AgentInfo, String> {
    blocking(|| {
        let state = agent::load_state();
        // Windows oculta a los usuarios normales las tareas que se ejecutan
        // como sistema: sin administrador, la tarea "no existe" aunque esté.
        // Si el agente se ha ejecutado hace poco, la tarea existe.
        let recent_tick = state
            .last_tick
            .as_deref()
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
            .is_some_and(|t| chrono::Local::now().signed_duration_since(t) < chrono::Duration::minutes(15));
        Ok(AgentInfo {
            supported: cfg!(windows),
            elevated: platform::is_elevated(),
            task_installed: recent_tick || platform::task_installed(),
            remote_backup: agent::load_config().remote_backup,
            discreet: agent::load_config().discreet,
            repos: agent::load_config().repos,
            state,
            tasks: tasks::load_state(),
            offsite_holds: tasks::load_guard().holds,
        })
    })
    .await
}

/// Activa, cambia o desactiva las copias automáticas de un repositorio.
/// Cambia la configuración: pide la contraseña. Requiere administrador.
#[tauri::command]
async fn agent_set_schedule(store: State<'_, Store>, id: String, schedule: Option<agent::Schedule>, password: String) -> Result<AgentInfo, String> {
    let repo = store.get(&id)?;
    require_password(&repo, password).await?;
    let access = if schedule.is_some() { Some(store::access(&repo)?) } else { None };
    let message = match &schedule {
        None => "Copias automáticas desactivadas.",
        Some(agent::Schedule::Plans) => "Copias automáticas activadas según los horarios de sus copias.",
        Some(agent::Schedule::Monitor { .. }) => "«Solo vigilar» activado: las copias las hace otro programa.",
        Some(_) => "Copias automáticas cambiadas.",
    };
    let (dir, rid, rname) = (store.config_dir(), repo.id.clone(), repo.name.clone());
    blocking(move || {
        agent::set_schedule(&repo, access.as_ref(), schedule)?;
        history::note(&dir, "config", &rid, &rname, None, message);
        // Si el equipo está vinculado, la web se entera enseguida (no en 10 minutos).
        if web::load_link().is_some_and(|l| !l.revoked) {
            let _ = web::report_now();
        }
        Ok(())
    })
    .await?;
    agent_info().await
}

/// Datos de la verificación que envía la app.
#[derive(serde::Deserialize)]
struct VerifyInput {
    schedule: agent::Schedule,
    subset_percent: u8,
    /// Verificación rotativa: todo el repositorio cada N verificaciones (0: no).
    #[serde(default)]
    rotate_parts: u32,
}

/// Activa, cambia o quita la verificación programada. Pide la contraseña.
#[tauri::command]
async fn agent_set_verify(store: State<'_, Store>, id: String, verify: Option<VerifyInput>, password: String) -> Result<AgentInfo, String> {
    let repo = store.get(&id)?;
    require_password(&repo, password).await?;
    let (dir, name) = (store.config_dir(), repo.name.clone());
    blocking(move || {
        let had = agent::load_config().repos.iter().any(|r| r.id == id && r.verify.is_some());
        let message = match (had, verify.is_some()) {
            (_, false) => "Verificación quitada.",
            (false, true) => "Verificación programada.",
            (true, true) => "Verificación cambiada.",
        };
        let verify = verify.map(|v| tasks::Verify {
            schedule: v.schedule,
            subset_percent: v.subset_percent,
            enabled_at: chrono::Local::now().to_rfc3339(),
            rotate_parts: v.rotate_parts,
        });
        agent::set_verify(&id, verify)?;
        history::note(&dir, "config", &id, &name, None, message);
        Ok(())
    })
    .await?;
    agent_info().await
}

/// Prueba de restauración que envía la app.
#[derive(serde::Deserialize)]
struct RestoreTestInput {
    schedule: agent::Schedule,
    files: u32,
    max_mb: u32,
}

/// Activa, cambia o quita la prueba de restauración. Pide la contraseña.
#[tauri::command]
async fn agent_set_restore_test(store: State<'_, Store>, id: String, restore_test: Option<RestoreTestInput>, password: String) -> Result<AgentInfo, String> {
    let repo = store.get(&id)?;
    require_password(&repo, password).await?;
    let (dir, name) = (store.config_dir(), repo.name.clone());
    blocking(move || {
        let had = agent::load_config().repos.iter().any(|r| r.id == id && r.restore_test.is_some());
        let message = match (had, restore_test.is_some()) {
            (_, false) => "Prueba de restauración quitada.",
            (false, true) => "Prueba de restauración programada.",
            (true, true) => "Prueba de restauración cambiada.",
        };
        let test = restore_test.map(|t| restore_test::RestoreTest {
            schedule: t.schedule,
            files: t.files,
            max_mb: t.max_mb,
            enabled_at: chrono::Local::now().to_rfc3339(),
        });
        agent::set_restore_test(&id, test)?;
        history::note(&dir, "config", &id, &name, None, message);
        Ok(())
    })
    .await?;
    agent_info().await
}

/// Destino de la copia externa que envía la app.
#[derive(serde::Deserialize)]
struct OffsiteInput {
    /// Otro destino ya guardado en la app (en lugar de ubicación y claves).
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    location: String,
    #[serde(default)]
    provider: String,
    #[serde(default)]
    region: Option<String>,
    schedule: agent::Schedule,
    /// Aplicar en el destino la política de retención del repositorio.
    #[serde(default)]
    apply_retention: bool,
    /// Velocidad máxima de subida en KiB/s (vacío: sin límite).
    #[serde(default)]
    limit_upload_kib: Option<u32>,
    /// Freno ante cambios inusuales (vacío: sin freno).
    #[serde(default)]
    guard: Option<tasks::Guard>,
    /// Verificar también la copia externa (vacío: no).
    #[serde(default)]
    verify: Option<VerifyInput>,
}

impl VerifyInput {
    fn into_verify(self) -> tasks::Verify {
        tasks::Verify {
            schedule: self.schedule,
            subset_percent: self.subset_percent,
            enabled_at: chrono::Local::now().to_rfc3339(),
            rotate_parts: self.rotate_parts,
        }
    }
}

/// Credenciales del destino (solo se envían al crearlo o cambiarlas).
#[derive(serde::Deserialize)]
struct OffsiteCreds {
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    key_id: Option<String>,
    #[serde(default)]
    key_secret: Option<String>,
}

/// Comprueba el destino y, si no existe, lo crea. Pide la contraseña del
/// repositorio de origen. Devuelve "existing" o "created".
#[tauri::command]
async fn offsite_prepare(
    store: State<'_, Store>,
    id: String,
    location: String,
    region: Option<String>,
    creds: OffsiteCreds,
    password: String,
) -> Result<String, String> {
    let repo = store.get(&id)?;
    require_password(&repo, password).await?;
    if has_embedded_password(&location) {
        return Err("No pongas la contraseña dentro de la ubicación: usa los campos de credenciales.".into());
    }
    let src = store::access(&repo)?;
    blocking(move || {
        let dest = restic::Access {
            location: location.trim().to_string(),
            password: creds.password.clone().filter(|p| !p.is_empty()).unwrap_or_else(|| src.password.clone()),
            rest_auth: None,
            cacert: None,
            env: tasks::cloud_env(&location, region.as_deref(), creds.key_id.as_deref(), creds.key_secret.as_deref()),
        };
        tasks::prepare_destination(&src, &dest).map(String::from)
    })
    .await
}

/// ¿El destino `tgt` recibe solo la copia externa de `source`? (Sin copias
/// propias y sin otra copia externa que suba a él.)
fn offsite_target_exclusive(tgt: &Repo, source: &str) -> bool {
    let provider = format!("destino:{}", tgt.id);
    tgt.plans.is_empty()
        && !agent::load_config().repos.iter().any(|r| r.id != source && (r.id == tgt.id || r.offsite.as_ref().is_some_and(|o| o.provider == provider)))
}

/// Activa, cambia o quita la copia externa. Pide la contraseña.
#[tauri::command]
async fn agent_set_offsite(
    store: State<'_, Store>,
    id: String,
    offsite: Option<OffsiteInput>,
    creds: Option<OffsiteCreds>,
    password: String,
    password_target: Option<String>,
) -> Result<AgentInfo, String> {
    let repo = store.get(&id)?;
    require_password(&repo, password).await?;
    let (dir, name) = (store.config_dir(), repo.name.clone());
    // Hacia otro destino de la app: se usan su ubicación y sus credenciales.
    let target = match offsite.as_ref().and_then(|o| non_empty(o.target.clone())) {
        Some(t) if t == id => return Err("El repositorio de la copia externa debe ser otro.".into()),
        Some(t) => {
            let tgt = store.get(&t)?;
            // Se entregan sus credenciales al agente: pide también su contraseña.
            require_password(&tgt, password_target.unwrap_or_default()).await?;
            let acc = store::access(&tgt)?;
            Some((tgt, acc))
        }
        None => None,
    };
    blocking(move || {
        let (offsite, creds) = match (offsite, target) {
            (Some(o), Some((tgt, acc))) => (
                Some(tasks::Offsite {
                    location: tgt.location.clone(),
                    provider: format!("destino:{}", tgt.id),
                    target_name: Some(tgt.name.clone()),
                    region: tgt.cloud_region.clone(),
                    schedule: o.schedule,
                    // La retención propia del destino (la que se edita en ese
                    // destino), no la del origen: p. ej. más ligera en la nube.
                    // Solo si es seguro: que no tenga copias propias ni reciba de
                    // otro origen (si no, borraría también esas versiones).
                    retention: if o.apply_retention && offsite_target_exclusive(&tgt, &id) { tgt.retention.clone().filter(|p| !p.is_empty()) } else { None },
                    limit_upload_kib: o.limit_upload_kib.filter(|k| *k > 0),
                    enabled_at: chrono::Local::now().to_rfc3339(),
                    guard: o.guard,
                    verify: o.verify.map(VerifyInput::into_verify),
                    dest: Default::default(),
                    filtro: None,
                }),
                Some(agent::OffsiteSecrets {
                    password: Some(acc.password.clone()),
                    key_id: None,
                    key_secret: None,
                    location: Some(tasks::location_with_auth(&acc.location, acc.rest_auth.as_ref())),
                    env: acc.env.clone(),
                }),
            ),
            (Some(o), None) => (
                Some(tasks::Offsite {
                    location: o.location.trim().to_string(),
                    provider: o.provider,
                    target_name: None,
                    region: o.region.filter(|r| !r.trim().is_empty()),
                    schedule: o.schedule,
                    retention: if o.apply_retention { repo.retention.clone() } else { None },
                    limit_upload_kib: o.limit_upload_kib.filter(|k| *k > 0),
                    enabled_at: chrono::Local::now().to_rfc3339(),
                    guard: o.guard,
                    verify: o.verify.map(VerifyInput::into_verify),
                    dest: Default::default(),
                    filtro: None,
                }),
                creds.map(|c| agent::OffsiteSecrets { password: c.password, key_id: c.key_id, key_secret: c.key_secret, location: None, env: Vec::new() }),
            ),
            (None, _) => (None, None),
        };
        let had = agent::load_config().repos.iter().any(|r| r.id == id && r.offsite.is_some());
        let message = match (had, &offsite) {
            (_, None) => "Copia externa quitada.".to_string(),
            (false, Some(o)) => format!("Copia externa configurada{}.", o.target_name.as_ref().map(|n| format!(" hacia «{n}»")).unwrap_or_default()),
            (true, Some(_)) => "Copia externa cambiada.".to_string(),
        };
        agent::set_offsite(&id, offsite, creds)?;
        history::note(&dir, "config", &id, &name, None, message);
        Ok(())
    })
    .await?;
    agent_info().await
}

/// Pausa las copias automáticas de un destino hasta `until` (RFC 3339, como
/// mucho a 30 días; `None`: hasta reanudarlas a mano). Lo que esté en marcha
/// termina normalmente. Pide la contraseña. Requiere administrador.
#[tauri::command]
async fn agent_pause(store: State<'_, Store>, id: String, until: Option<String>, password: String) -> Result<AgentInfo, String> {
    let repo = store.get(&id)?;
    require_password(&repo, password).await?;
    blocking(move || {
        agent::set_pause(&id, until.as_deref())?;
        // Si el equipo está vinculado, la web se entera enseguida.
        if web::load_link().is_some_and(|l| !l.revoked) {
            let _ = web::report_now();
        }
        Ok(())
    })
    .await?;
    agent_info().await
}

/// Reanuda las copias automáticas de un destino. Pide la contraseña. Requiere administrador.
#[tauri::command]
async fn agent_resume(store: State<'_, Store>, id: String, password: String) -> Result<AgentInfo, String> {
    let repo = store.get(&id)?;
    require_password(&repo, password).await?;
    blocking(move || {
        agent::resume(&id)?;
        if web::load_link().is_some_and(|l| !l.revoked) {
            let _ = web::report_now();
        }
        Ok(())
    })
    .await?;
    agent_info().await
}

/// «Es normal, reanudar la subida»: quita el freno por un cambio inusual.
/// Pide la contraseña. Requiere administrador.
#[tauri::command]
async fn agent_offsite_resume(store: State<'_, Store>, id: String, password: String) -> Result<AgentInfo, String> {
    let repo = store.get(&id)?;
    require_password(&repo, password).await?;
    blocking(move || {
        tasks::clear_hold(&id)?;
        if web::load_link().is_some_and(|l| !l.revoked) {
            let _ = web::report_now();
        }
        Ok(())
    })
    .await?;
    agent_info().await
}

/// Pide al agente verificar o subir ahora (en ≤ 5 minutos). No cambia nada.
#[tauri::command]
async fn agent_task_now(id: String, kind: String) -> Result<(), String> {
    blocking(move || tasks::request_now(&id, &kind)).await
}

/// Historial de actividad (copias, verificaciones y subidas), lo más reciente primero.
#[tauri::command]
async fn activity_history(app: AppHandle, limit: Option<usize>) -> Result<Vec<history::Entry>, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    blocking(move || Ok(history::combined(&dir, limit.unwrap_or(500).min(5000)))).await
}

/// Últimas líneas del registro del agente (solo lectura).
#[tauri::command]
async fn agent_log() -> Result<Vec<String>, String> {
    blocking(|| Ok(agent::log_tail(200))).await
}

/// Registro detallado del agente, con rutas completas (solo administradores).
#[tauri::command]
async fn agent_log_detail() -> Result<Vec<String>, String> {
    blocking(|| agent::detail_tail(400)).await
}

/// Archivos que no se pudieron leer en la última copia automática de un plan
/// (solo administradores).
#[tauri::command]
async fn agent_failed_files(id: String, plan: String) -> Result<Vec<String>, String> {
    blocking(move || agent::failed_files(&id, &plan)).await
}

/// Vuelve a crear la tarea del agente (requiere administrador).
#[tauri::command]
async fn agent_repair() -> Result<AgentInfo, String> {
    agent::require_admin()?;
    blocking(agent::sync_task).await?;
    agent_info().await
}

#[derive(serde::Serialize)]
struct WebInfo {
    link: Option<web::WebLink>,
    state: web::WebState,
    elevated: bool,
    default_url: String,
    default_key: String,
    default_name: String,
}

#[tauri::command]
async fn web_info() -> Result<WebInfo, String> {
    blocking(|| {
        let (default_url, default_key) = web::defaults();
        Ok(WebInfo {
            link: web::load_link(),
            state: agent::load_state().web,
            elevated: platform::is_elevated(),
            default_url,
            default_key,
            default_name: web::default_device_name(),
        })
    })
    .await
}

/// Vincula este equipo con la web usando el código que muestra la web.
#[tauri::command]
async fn web_pair(url: String, key: String, code: String, name: String) -> Result<WebInfo, String> {
    blocking(move || web::pair(&url, &key, &code, &name).map(|_| ())).await?;
    web_info().await
}

#[tauri::command]
async fn web_unpair() -> Result<WebInfo, String> {
    blocking(web::unpair).await?;
    web_info().await
}

#[tauri::command]
async fn web_report_now() -> Result<WebInfo, String> {
    agent::require_admin()?;
    blocking(web::report_now).await?;
    web_info().await
}

// ---------- Cerrar la ventana con copias en curso ----------

/// Si es true, la ventana se cierra sin preguntar.
#[derive(Default)]
struct CloseState(AtomicBool);

#[derive(serde::Serialize)]
struct RunningJob {
    repo_id: String,
    name: String,
    /// "backup" o "restore".
    kind: &'static str,
    /// El agente puede terminar esta copia.
    can_hand_off: bool,
}

#[tauri::command]
fn running_jobs(store: State<'_, Store>, jobs: State<'_, Jobs>, manual: State<'_, ManualPlans>) -> Vec<RunningJob> {
    jobs.keys()
        .into_iter()
        // Una búsqueda de archivos no impide cerrar: se detiene sin más.
        .filter(|key| !key.ends_with(":search"))
        .map(|key| {
            let (repo_id, kind) = match key.strip_suffix(":restore") {
                Some(id) => (id.to_string(), "restore"),
                None => (key.clone(), "backup"),
            };
            let name = store.get(&repo_id).map(|r| r.name).unwrap_or_else(|_| repo_id.clone());
            let plan = manual.0.lock().unwrap().get(&repo_id).cloned().unwrap_or_default();
            let can_hand_off = kind == "backup" && agent::can_hand_off(&repo_id, &plan);
            RunningJob { repo_id, name, kind, can_hand_off }
        })
        .collect()
}

/// Cierra la app. `mode`:
/// - "handoff": las copias que el agente puede terminar se le pasan (se
///   detienen aquí y el agente las retoma en ≤ 5 min); el resto sigue en
///   segundo plano y la app se cierra al acabar.
/// - "background": se oculta la ventana y se cierra al acabar todo.
/// - "stop": se detiene todo y se cierra ya.
#[tauri::command]
fn close_app(app: AppHandle, jobs: State<'_, Jobs>, close: State<'_, CloseState>, manual: State<'_, ManualPlans>, mode: String) -> Result<(), String> {
    close.0.store(true, Ordering::Relaxed);
    if mode == "stop" {
        app.exit(0);
        return Ok(());
    }
    if mode == "handoff" {
        for key in jobs.keys() {
            let plan = manual.0.lock().unwrap().get(&key).cloned().unwrap_or_default();
            if !key.ends_with(":restore") && agent::can_hand_off(&key, &plan) {
                agent::request_backup(&key, &plan)?;
                jobs.cancel(&key)?;
            }
        }
    }
    for key in jobs.keys() {
        if key.ends_with(":search") {
            jobs.cancel(&key)?;
        }
    }
    close_when_idle(app);
    Ok(())
}

/// Salir de la app (la X sin bandeja, o «Salir» en la bandeja). Con copias o
/// restauraciones en curso, se pregunta qué hacer (`close-requested`).
/// Devuelve false si no se puede cerrar ya.
pub(crate) fn request_quit(app: &AppHandle) -> bool {
    let forced = app.state::<CloseState>().0.load(Ordering::Relaxed);
    let jobs = app.state::<Jobs>();
    let keys = jobs.keys();
    for key in keys.iter().filter(|k| k.ends_with(":search")) {
        let _ = jobs.cancel(key);
    }
    if forced || !keys.iter().any(|k| !k.ends_with(":search")) {
        app.exit(0);
        return true;
    }
    if app.state::<applock::AppLock>().is_locked() {
        // Bloqueada no se puede preguntar qué hacer (ni mostrar qué está en
        // curso): lo en curso termina en segundo plano.
        app.state::<CloseState>().0.store(true, Ordering::Relaxed);
        close_when_idle(app.clone());
    } else {
        tray::show_main(app);
        let _ = app.emit("close-requested", ());
    }
    false
}

/// Oculta la ventana y cierra la app cuando no quede nada en curso.
fn close_when_idle(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(500));
        if app.state::<Jobs>().keys().is_empty() {
            app.exit(0);
            break;
        }
    });
}

// ---------- Bandeja ----------

/// Argumento con el que arranca al iniciar Windows: sin abrir la ventana.
const START_HIDDEN_ARG: &str = "--bandeja";

/// Oculta la ventana (la app sigue en la bandeja). Si está activado el
/// bloqueo, se bloquea: al volver a abrirla se pide Windows Hello.
fn hide_to_tray(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    let lock = app.state::<applock::AppLock>();
    if lock.config().enabled && !lock.is_locked() {
        lock.lock();
        let _ = app.emit("app-locked", ());
    }
}

/// Estado para el icono de la bandeja (lo calcula la interfaz).
#[tauri::command]
fn tray_update(app: AppHandle, status: tray::TrayStatus) -> Result<(), String> {
    tray::update(&app, status)
}

#[derive(serde::Serialize)]
struct TraySettingsInfo {
    close_to_tray: bool,
    notifications: bool,
    /// Se inicia con Windows (para este usuario).
    autostart: bool,
}

fn tray_settings_info(app: &AppHandle) -> TraySettingsInfo {
    use tauri_plugin_autostart::ManagerExt;
    let s = app.state::<tray::Tray>().settings();
    TraySettingsInfo { close_to_tray: s.close_to_tray, notifications: s.notifications, autostart: app.autolaunch().is_enabled().unwrap_or(false) }
}

#[tauri::command]
fn tray_settings(app: AppHandle) -> TraySettingsInfo {
    tray_settings_info(&app)
}

/// Ajustes de la bandeja: preferencias de este usuario en este equipo (como
/// el aspecto), sin contraseña ni administrador.
#[tauri::command]
fn tray_set(app: AppHandle, close_to_tray: bool, notifications: bool, autostart: bool) -> Result<TraySettingsInfo, String> {
    use tauri_plugin_autostart::ManagerExt;
    let tray = app.state::<tray::Tray>();
    let settings = tray::Settings { close_to_tray, notifications };
    tray::save_settings(&tray.dir, &settings)?;
    *tray.settings.lock().unwrap() = settings;
    let launcher = app.autolaunch();
    if launcher.is_enabled().unwrap_or(false) != autostart {
        let result = if autostart { launcher.enable() } else { launcher.disable() };
        result.map_err(|e| format!("No se pudo cambiar el inicio con Windows: {e}"))?;
    }
    Ok(tray_settings_info(&app))
}

// ---------- «Ver versiones en Resguardo» (Explorador) ----------

/// Vista pedida desde un aviso o la bandeja (una sola vez).
#[tauri::command]
fn take_view(pending: State<'_, avisos::PendingView>) -> Option<avisos::Target> {
    pending.0.lock().unwrap().take()
}

/// Ruta pedida con `--versiones` (al arrancar o desde otra instancia; una sola vez).
#[tauri::command]
fn versions_pending(pending: State<'_, versiones::Pending>) -> Option<String> {
    pending.0.lock().unwrap().take()
}

/// Qué copias incluyen una ruta de este equipo y cómo está ahora. La ruta
/// llega de fuera (Explorador): se valida aquí.
#[tauri::command]
async fn locate_path(store: State<'_, Store>, path: String) -> Result<versiones::Located, String> {
    let repos = store.list()?;
    blocking(move || {
        let path = versiones::validate_path(&path)?;
        let found = versiones::locate(&repos, &path.to_string_lossy());
        Ok(versiones::describe(&path, found))
    })
    .await
}

/// «Restaurar esta versión»: con otro nombre junto al original, sin
/// reemplazar nada (por eso, como restaurar sin reemplazar, no pide contraseña).
#[tauri::command]
async fn restore_version(app: AppHandle, store: State<'_, Store>, id: String, snapshot: String, path: String, name: String) -> Result<String, String> {
    let repo = store.get(&id)?;
    let access = store::access(&repo)?;
    if !restic::valid_snapshot_id(&snapshot) {
        return Err("ID de versión no válido.".into());
    }
    blocking(move || {
        let original = versiones::validate_path(&path)?;
        let target = versiones::restore_as(&app, &app.state::<Jobs>(), &repo, &access, &snapshot, &original, &name)?;
        Ok(target.to_string_lossy().into_owned())
    })
    .await
}

/// ¿Está «Ver versiones en Resguardo» en el menú del Explorador?
#[tauri::command]
fn shell_menu_status() -> bool {
    versiones::menu_installed()
}

/// Pone o quita la entrada del menú del Explorador (solo para este usuario,
/// como un ajuste de la app: sin contraseña ni administrador).
#[tauri::command]
fn shell_menu_set(enabled: bool) -> Result<bool, String> {
    versiones::set_menu(enabled)?;
    Ok(versiones::menu_installed())
}

/// Vuelve a abrir la app como administrador y cierra esta ventana.
#[tauri::command]
fn relaunch_as_admin(app: AppHandle) -> Result<(), String> {
    platform::relaunch_elevated()?;
    app.exit(0);
    Ok(())
}

#[tauri::command]
fn cancel_restore(jobs: State<'_, Jobs>, id: String) -> Result<(), String> {
    jobs.cancel(&restore::job_key(&id))
}

// ---------- Bloqueo con Windows Hello ----------

#[derive(serde::Serialize)]
struct AppLockStatus {
    enabled: bool,
    idle_minutes: Option<u32>,
    locked: bool,
    availability: applock::Availability,
    /// Por qué no se puede usar Windows Hello (vacío si se puede).
    explain: &'static str,
}

#[tauri::command]
async fn app_lock_status(app: AppHandle) -> Result<AppLockStatus, String> {
    blocking(move || {
        let lock = app.state::<applock::AppLock>();
        let config = lock.config();
        let availability = applock::availability();
        Ok(AppLockStatus {
            enabled: config.enabled,
            idle_minutes: config.idle_minutes,
            locked: lock.is_locked(),
            availability,
            explain: availability.explain(),
        })
    })
    .await
}

/// Pide Windows Hello (o la contraseña de Windows) con el diálogo delante de la ventana.
fn hello_check(app: &AppHandle, message: &str) -> Result<applock::Verification, String> {
    let hwnd = app.get_webview_window("main").and_then(|w| w.hwnd().ok()).map(|h| h.0 as isize).unwrap_or(0);
    applock::verify(hwnd, message)
}

#[derive(serde::Serialize)]
struct UnlockResult {
    result: applock::Verification,
    /// Aviso si no se pudo pedir Windows Hello (la app se abre igualmente).
    message: Option<String>,
}

#[tauri::command]
async fn app_lock_unlock(app: AppHandle) -> Result<UnlockResult, String> {
    blocking(move || {
        let lock = app.state::<applock::AppLock>();
        if !lock.is_locked() {
            return Ok(UnlockResult { result: applock::Verification::Verified, message: None });
        }
        let availability = applock::availability();
        if availability != applock::Availability::Available {
            // Sin Windows Hello no se puede comprobar: no se deja a nadie fuera
            // de sus copias, pero se explica por qué no se pidió.
            lock.unlock();
            return Ok(UnlockResult {
                result: applock::Verification::Unavailable,
                message: Some(format!("{} Por eso Resguardo se ha abierto sin pedirla.", availability.explain())),
            });
        }
        let result = hello_check(&app, "Desbloquear Resguardo")?;
        if result == applock::Verification::Verified {
            lock.unlock();
        }
        Ok(UnlockResult { result, message: None })
    })
    .await
}

/// Activar, desactivar o cambiar el bloqueo. Cualquier cambio pide pasar la comprobación.
#[tauri::command]
async fn app_lock_set(app: AppHandle, enabled: bool, idle_minutes: Option<u32>) -> Result<AppLockStatus, String> {
    let config = applock::LockConfig { enabled, idle_minutes: if enabled { idle_minutes } else { None } };
    applock::validate(&config)?;
    let app2 = app.clone();
    blocking(move || {
        let lock = app2.state::<applock::AppLock>();
        let availability = applock::availability();
        if availability == applock::Availability::Available {
            let message = if enabled { "Confirma que eres tú para proteger Resguardo" } else { "Confirma que eres tú para quitar el bloqueo de Resguardo" };
            match hello_check(&app2, message)? {
                applock::Verification::Verified => {}
                applock::Verification::Canceled => return Err("Cancelado: no se cambió nada.".into()),
                _ => return Err("No se pudo comprobar que eres tú: no se cambió nada.".into()),
            }
        } else if enabled {
            // Activarlo sin poder comprobarlo dejaría un bloqueo que no protege.
            return Err(availability.explain().to_string());
        }
        lock.set(config)?;
        lock.unlock();
        Ok(())
    })
    .await?;
    app_lock_status(app).await
}

/// Bloquear ya (tras un rato sin usar la app). Sin bloqueo activado no hace nada.
#[tauri::command]
fn app_lock_lock(lock: State<'_, applock::AppLock>) {
    lock.lock();
}

/// «Copias a distancia»: permitir (o no) que se pida «Copiar ahora» de este
/// equipo desde la web o desde Resguardo en otro equipo. Cambia la
/// configuración del agente: administrador y la contraseña de uno de sus
/// destinos (`id`, cualquiera de los que tiene el agente).
#[tauri::command]
async fn agent_set_remote_backup(store: State<'_, Store>, id: String, enabled: bool, password: String) -> Result<AgentInfo, String> {
    agent::require_admin()?;
    if !agent::is_scheduled(&id) {
        return Err("Ese repositorio no tiene copias automáticas en este equipo.".into());
    }
    let repo = store.get(&id)?;
    require_password(&repo, password).await?;
    blocking(move || {
        agent::set_remote_backup(enabled)?;
        if web::load_link().is_some_and(|l| !l.revoked) {
            let _ = web::report_now();
        }
        Ok(())
    })
    .await?;
    agent_info().await
}

/// «Modo discreto»: activar, cambiar o quitar (None) el horario en que el
/// agente usa restic con prioridad baja. Cambia la configuración del agente:
/// administrador y la contraseña de uno de sus destinos (`id`).
#[tauri::command]
async fn agent_set_discreet(store: State<'_, Store>, id: String, discreet: Option<discreto::Discreet>, password: String) -> Result<AgentInfo, String> {
    agent::require_admin()?;
    if !agent::is_scheduled(&id) {
        return Err("Ese repositorio no tiene copias automáticas en este equipo.".into());
    }
    if let Some(d) = &discreet {
        d.validate()?;
    }
    let repo = store.get(&id)?;
    require_password(&repo, password).await?;
    blocking(move || agent::set_discreet(discreet)).await?;
    agent_info().await
}

// ---------- Todos mis equipos (cuenta de Resguardo Web) ----------

#[tauri::command]
fn account_status(account: State<'_, account::Account>) -> account::Status {
    account.status()
}

/// Paso 1: correo y contraseña (la contraseña no se guarda nunca).
#[tauri::command]
async fn account_sign_in(app: AppHandle, email: String, password: String) -> Result<account::Status, String> {
    blocking(move || {
        let acc = app.state::<account::Account>();
        acc.sign_in(&email, &password)?;
        Ok(acc.status())
    })
    .await
}

/// Paso 2: el código del autenticador (sesión aal2).
#[tauri::command]
async fn account_verify(app: AppHandle, code: String) -> Result<account::Status, String> {
    blocking(move || {
        let acc = app.state::<account::Account>();
        acc.verify_code(&code)?;
        Ok(acc.status())
    })
    .await
}

#[tauri::command]
async fn account_sign_out(app: AppHandle) -> Result<account::Status, String> {
    blocking(move || {
        let acc = app.state::<account::Account>();
        acc.sign_out();
        Ok(acc.status())
    })
    .await
}

/// Clientes, equipos y destinos de la cuenta (con la sesión del usuario).
#[tauri::command]
async fn account_overview(app: AppHandle) -> Result<serde_json::Value, String> {
    blocking(move || app.state::<account::Account>().overview()).await
}

/// «Copiar ahora» de un plan en otro equipo. Devuelve el id de la petición.
#[tauri::command]
async fn account_request_backup(app: AppHandle, device: String, repo: String, plan: String) -> Result<String, String> {
    blocking(move || app.state::<account::Account>().request_backup(&device, &repo, &plan)).await
}

#[tauri::command]
async fn account_command(app: AppHandle, id: String) -> Result<serde_json::Value, String> {
    blocking(move || app.state::<account::Account>().command(&id)).await
}

/// «Buscar un archivo» en las versiones del destino. Solo lee: no pide la
/// contraseña (como explorar una versión). Los resultados llegan como eventos.
#[tauri::command]
async fn search_files(app: AppHandle, store: State<'_, Store>, id: String, req: search::Request) -> Result<search::Outcome, String> {
    let access = store::access(&store.get(&id)?)?;
    blocking(move || search::run(&app, &app.state::<Jobs>(), &id, &access, &req)).await
}

#[tauri::command]
fn cancel_search(jobs: State<'_, Jobs>, id: String) -> Result<(), String> {
    jobs.cancel(&search::job_key(&id))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let handler: Box<dyn Fn(tauri::ipc::Invoke) -> bool + Send + Sync> = Box::new(tauri::generate_handler![
        restic_version,
        list_repos,
        add_repo,
        cancel_add_repo,
        remove_repo,
        list_snapshots,
        set_plans,
        move_plan,
        run_backup,
        cancel_backup,
        check_password,
        list_snapshot_dir,
        snapshot_largest,
        inspect_target,
        run_restore,
        cancel_restore,
        search_files,
        cancel_search,
        retention_preview,
        set_retention,
        repo_stats,
        rename_repo,
        snapshot_diff,
        set_expected_interval,
        agent_info,
        agent_log,
        agent_log_detail,
        agent_set_remote_backup,
        agent_set_discreet,
        account_status,
        account_sign_in,
        account_verify,
        account_sign_out,
        account_overview,
        account_request_backup,
        account_command,
        agent_failed_files,
        activity_history,
        agent_set_schedule,
        agent_set_verify,
        agent_set_offsite,
        agent_pause,
        agent_resume,
        agent_offsite_resume,
        agent_set_restore_test,
        recovery_info,
        kit_confirm,
        protection_status,
        probe_append_only,
        set_object_lock,
        list_places,
        rename_place,
        place_scan,
        server_status,
        server_setup,
        server_disable,
        server_add_user,
        server_remove_user,
        server_public_ip,
        managed_list,
        managed_pair_start,
        managed_pair_poll,
        managed_pair_confirm,
        managed_pair_cancel,
        unlock_repo,
        update_saved_password,
        managed_set_retention,
        managed_agent_installer,
        managed_save_agent_installer,
        managed_prune_now,
        managed_set_config,
        managed_backup_now,
        managed_unpair,
        managed_open_repo,
        place_share_status,
        place_share_set,
        shares_available,
        share_request,
        share_cancel,
        shares_received,
        received_share_add,
        clone_repo,
        cancel_clone,
        offsite_prepare,
        agent_task_now,
        relaunch_as_admin,
        web_info,
        web_pair,
        web_unpair,
        web_report_now,
        running_jobs,
        close_app,
        agent_repair,
        app_lock_status,
        app_lock_unlock,
        app_lock_set,
        app_lock_lock,
        tray_update,
        tray_settings,
        tray_set,
        versions_pending,
        take_view,
        locate_path,
        restore_version,
        shell_menu_status,
        shell_menu_set
    ]);
    tauri::Builder::default()
        // Una sola instancia: abrir Resguardo otra vez trae la ventana al frente.
        // Con `--versiones "<ruta>"` (menú del Explorador), además, se abren
        // las versiones de esa ruta.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            tray::show_main(app);
            if let Some(raw) = versiones::from_args(args) {
                *app.state::<versiones::Pending>().0.lock().unwrap() = Some(raw);
                let _ = app.emit("open-versions", ());
            }
        }))
        // «Iniciar con Windows»: entrada en HKCU\...\Run, solo para este usuario.
        .plugin(tauri_plugin_autostart::Builder::new().arg(START_HIDDEN_ARG).build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Jobs::default())
        .manage(account::Account::default())
        .manage(ManualPlans::default())
        .manage(Checks::default())
        .manage(CloseState::default())
        .manage(avisos::PendingView::default())
        .manage(versiones::Pending(std::sync::Mutex::new(versiones::from_args(std::env::args()))))
        // Al pulsar la X con copias o restauraciones en curso, se pregunta qué hacer.
        // Con «Al cerrar, seguir en la bandeja», la X solo oculta la ventana.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                if app.state::<CloseState>().0.load(Ordering::Relaxed) {
                    return;
                }
                api.prevent_close();
                if app.state::<tray::Tray>().settings().close_to_tray {
                    hide_to_tray(app);
                } else {
                    request_quit(app);
                }
            }
        })
        .setup(|app| {
            // Abierta como administrador: si falta la tarea del agente (p. ej.
            // tras una actualización), se vuelve a crear.
            if platform::is_elevated() {
                std::thread::spawn(|| {
                    if let Err(e) = agent::sync_task() {
                        agent::log(&format!("No se pudo reactivar la tarea del agente: {e}"));
                    }
                });
            }
            let dir = app.path().app_config_dir()?;
            app.manage(applock::AppLock::new(dir.clone()));
            // Bandeja: la primera vez (instalación nueva) se activa el inicio
            // con Windows; quien ya usaba Resguardo lo decide en Ajustes.
            let tray_settings = match tray::load_settings(&dir) {
                Some(s) => s,
                None => {
                    let fresh = !dir.join("repos.json").exists();
                    let s = tray::Settings::default();
                    let _ = tray::save_settings(&dir, &s);
                    if fresh {
                        use tauri_plugin_autostart::ManagerExt;
                        let _ = app.autolaunch().enable();
                    }
                    s
                }
            };
            app.manage(tray::Tray::new(dir.clone(), tray_settings));
            tray::create(app.handle())?;
            // Iniciada con Windows: se queda en la bandeja sin abrir la ventana.
            if !std::env::args().any(|a| a == START_HIDDEN_ARG) {
                tray::show_main(app.handle());
            }
            app.manage(stats_cache::StatsCache::new(dir.clone()));
            app.manage(Store::new(dir.clone()));
            avisos::start(app.handle().clone(), dir);
            // Como administrador: lo guardado sin administrador (retención,
            // kit, bloqueo de objetos) se refleja en el agente y en la web.
            if platform::is_elevated() {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    let Ok(repos) = handle.state::<Store>().list() else { return };
                    match agent::sync_all_meta(&repos) {
                        Ok(0) => {}
                        Ok(n) => {
                            agent::log(&format!("Datos de {n} repositorio(s) actualizados en el agente (retención, kit o bloqueo)."));
                            if web::load_link().is_some_and(|l| !l.revoked) {
                                let _ = web::report_now();
                            }
                        }
                        Err(e) => agent::log(&format!("No se pudieron actualizar los datos de los repositorios en el agente: {e}")),
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(move |invoke| {
            // Con la app bloqueada, el backend no responde a nada que muestre
            // o cambie datos: no basta con tapar la interfaz.
            let command = invoke.message.command();
            let locked = invoke.message.webview().state::<applock::AppLock>().is_locked();
            if locked && !applock::allowed_while_locked(command) {
                invoke.resolver.reject("Resguardo está bloqueado.");
                return true;
            }
            handler(invoke)
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // No dejar procesos de restic huérfanos al cerrar la app.
            if let RunEvent::Exit = event {
                app.state::<Jobs>().cancel_all();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::has_embedded_password;

    #[test]
    fn detecta_contrasenas_en_la_ubicacion() {
        assert!(has_embedded_password("rest:http://ana:secreto@nas:8000/repo"));
        assert!(has_embedded_password("rest:https://ana:s3cr3t@host/"));
        assert!(!has_embedded_password("rest:http://nas:8000/repo"));
        assert!(!has_embedded_password("rest:http://ana@nas:8000/repo"));
        assert!(!has_embedded_password("sftp:ana@nas:/srv/restic"));
        assert!(!has_embedded_password(r"D:\Copias\restic"));
        assert!(!has_embedded_password("rest:http://nas:8000/a:b@c"));
    }
}
