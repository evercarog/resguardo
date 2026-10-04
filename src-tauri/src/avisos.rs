//! Avisos de Windows (notificaciones) desde la app en la bandeja.
//!
//! Solo por cambios en el estado del agente, una vez por cambio:
//! - falló una copia automática (al pasar de bien a mal; no en cada reintento);
//! - una subida a la nube se frenó por un cambio inusual;
//! - la subida terminó después de reanudarla;
//! - falta el kit de recuperación de algún destino (como mucho una vez por semana).
//!
//! Lo ya avisado se guarda en `avisos.json` (por usuario). La primera vez solo
//! se toma nota del estado actual, sin avisar de lo que ya pasó. Si Windows
//! no acepta avisos ahora (pantalla completa, presentación, horas de
//! concentración, sesión bloqueada) se espera: no se marca nada como avisado.
//! Los textos no llevan rutas ni mensajes de error: solo nombres.

use chrono::{DateTime, Duration, Local};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};

/// Cada cuánto se mira el estado del agente.
const POLL: std::time::Duration = std::time::Duration::from_secs(60);
const KIT_EVERY_DAYS: i64 = 7;

/// Lo ya avisado.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct Seen {
    /// Ya se tomó nota del estado inicial.
    #[serde(default)]
    pub ready: bool,
    /// Copias ("repo#plan") que están fallando y de las que ya se avisó.
    #[serde(default)]
    pub failing: BTreeSet<String>,
    /// Subidas frenadas ya avisadas: destino → desde cuándo.
    #[serde(default)]
    pub holds: BTreeMap<String, String>,
    /// Destinos cuya subida se reanudó: se avisa cuando termine.
    #[serde(default)]
    pub released: BTreeSet<String>,
    /// Final de la última subida a la nube de cada destino.
    #[serde(default)]
    pub offsite: BTreeMap<String, String>,
    /// Último aviso del kit de recuperación (RFC 3339).
    #[serde(default)]
    pub kit_at: Option<String>,
}

/// Lo que se sabe ahora (leído del agente y de los destinos).
#[derive(Default, Debug)]
pub struct Facts {
    /// "repo#plan" → resultado de la última copia automática ("ok", "warning", "error").
    pub runs: HashMap<String, String>,
    /// Destino → desde cuándo está frenada su subida.
    pub holds: HashMap<String, String>,
    /// Destino → (final, resultado) de la última subida a la nube.
    pub offsite: HashMap<String, (String, String)>,
    /// Nombres de destinos ("repo") y copias ("repo#plan").
    pub names: HashMap<String, String>,
    /// Destinos sin kit de recuperación (o con uno que ya no vale).
    pub kit_missing: Vec<String>,
}

/// Adónde lleva el aviso al pulsarlo.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Target {
    Copy {
        #[serde(rename = "repoId")]
        repo_id: String,
        #[serde(rename = "planId")]
        plan_id: String,
    },
    Destination {
        #[serde(rename = "repoId")]
        repo_id: String,
    },
    Kit {
        ids: Vec<String>,
    },
    /// «Pausar copias automáticas 1 hora» (desde la bandeja).
    Pause,
}

/// Vista pedida desde fuera de la ventana (un aviso, la bandeja). Se guarda
/// aquí y la interfaz la recoge con `take_view`: si la app está bloqueada, la
/// interfaz no está montada y la recoge al desbloquearla.
#[derive(Default)]
pub struct PendingView(pub std::sync::Mutex<Option<Target>>);

/// Abre la ventana en una vista.
pub fn request_view(app: &AppHandle, target: Target) {
    *app.state::<PendingView>().0.lock().unwrap() = Some(target);
    crate::tray::show_main(app);
    let _ = app.emit("open-view", ());
}

#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    pub title: String,
    pub body: String,
    pub target: Target,
}

/// Compara lo que se sabe ahora con lo ya avisado: qué avisar y qué anotar.
pub fn diff(seen: &Seen, facts: &Facts, now: DateTime<Local>) -> (Vec<Notice>, Seen) {
    let mut next = seen.clone();
    let mut out = Vec::new();
    let name = |key: &str| facts.names.get(key).cloned().unwrap_or_else(|| "un repositorio".into());

    // Copias que fallan (solo al empezar a fallar).
    let mut keys: Vec<_> = facts.runs.keys().collect();
    keys.sort();
    for key in keys {
        if facts.runs[key] == "error" {
            if next.failing.insert(key.clone()) {
                let (repo, plan) = key.split_once('#').unwrap_or((key, ""));
                out.push(Notice {
                    title: format!("Falló la copia «{}»", name(key)),
                    body: format!("Se guarda en «{}». Ábrela para ver qué pasó; Resguardo lo volverá a intentar.", name(repo)),
                    target: if plan.is_empty() {
                        Target::Destination { repo_id: repo.into() }
                    } else {
                        Target::Copy { repo_id: repo.into(), plan_id: plan.into() }
                    },
                });
            }
        } else {
            next.failing.remove(key);
        }
    }
    next.failing.retain(|k| facts.runs.contains_key(k));

    // Subidas frenadas nuevas.
    let mut holds: Vec<_> = facts.holds.iter().collect();
    holds.sort();
    for (repo, since) in holds {
        if seen.holds.get(repo) != Some(since) {
            out.push(Notice {
                title: format!("Subida a la nube frenada en «{}»", name(repo)),
                body: "Una copia cambió mucho más de lo normal. Revisa qué cambió antes de reanudarla.".into(),
                target: Target::Destination { repo_id: repo.clone() },
            });
        }
        next.released.remove(repo);
    }
    // Subidas reanudadas: se espera a que termine la siguiente.
    for repo in seen.holds.keys().filter(|r| !facts.holds.contains_key(*r)) {
        next.released.insert(repo.clone());
    }
    next.holds = facts.holds.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    for repo in next.released.clone() {
        let Some((finished, result)) = facts.offsite.get(&repo) else { continue };
        if seen.offsite.get(&repo) != Some(finished) && result != "error" {
            out.push(Notice {
                title: "Subida a la nube completada".into(),
                body: format!("La copia externa de «{}» ya está al día.", name(&repo)),
                target: Target::Destination { repo_id: repo.clone() },
            });
            next.released.remove(&repo);
        }
    }
    next.offsite = facts.offsite.iter().map(|(k, (f, _))| (k.clone(), f.clone())).collect();

    // Kit de recuperación, como mucho una vez por semana.
    if !facts.kit_missing.is_empty() {
        let last = seen.kit_at.as_deref().and_then(|t| DateTime::parse_from_rfc3339(t).ok());
        let due = last.is_none_or(|t| now.signed_duration_since(t) >= Duration::days(KIT_EVERY_DAYS));
        if due {
            let body = match facts.kit_missing.as_slice() {
                [one] => format!("Sin él, si pierdes este equipo no podrás abrir las copias de «{}».", name(one)),
                many => format!("Falta en {} repositorios. Sin él, si pierdes este equipo no podrás abrir las copias.", many.len()),
            };
            out.push(Notice { title: "Guarda el kit de recuperación".into(), body, target: Target::Kit { ids: facts.kit_missing.clone() } });
            next.kit_at = Some(now.to_rfc3339());
        }
    }

    // La primera vez solo se toma nota (y el kit espera una semana).
    if !seen.ready {
        next.ready = true;
        next.released.clear();
        next.kit_at = Some(now.to_rfc3339());
        out.clear();
    }
    (out, next)
}

fn seen_path(dir: &Path) -> PathBuf {
    dir.join("avisos.json")
}

fn load_seen(dir: &Path) -> Seen {
    fs::read(seen_path(dir)).ok().and_then(|raw| serde_json::from_slice(&raw).ok()).unwrap_or_default()
}

fn save_seen(dir: &Path, seen: &Seen) {
    if let Ok(data) = serde_json::to_vec_pretty(seen) {
        let _ = fs::write(seen_path(dir), data);
    }
}

/// Lee el estado del agente y de los destinos de este usuario.
fn facts(app: &AppHandle) -> Facts {
    let mut f = Facts::default();
    let config = crate::agent::load_config();
    for repo in &config.repos {
        f.names.insert(repo.id.clone(), repo.name.clone());
        for plan in &repo.plans {
            f.names.insert(crate::plans::plan_key(&repo.id, &plan.id), plan.name.clone());
        }
    }
    let state = crate::agent::load_state();
    for (key, run) in state.runs {
        // Solo copias de planes que siguen programados.
        if f.names.contains_key(&key) && key.contains('#') {
            f.runs.insert(key, run.result);
        }
    }
    f.holds = crate::tasks::load_guard().holds.into_iter().map(|(k, h)| (k, h.since)).collect();
    for (key, run) in crate::tasks::load_state().runs {
        if let Some(repo) = key.strip_prefix("offsite:") {
            f.offsite.insert(repo.to_string(), (run.finished, run.result));
        }
    }
    if let Ok(repos) = app.state::<crate::store::Store>().list() {
        for r in repos {
            f.names.entry(r.id.clone()).or_insert_with(|| r.name.clone());
            if r.kit.as_ref().is_none_or(|k| k.location != r.location) {
                f.kit_missing.push(r.id);
            }
        }
    }
    f
}

/// ¿Acepta Windows avisos ahora? (No con pantalla completa, presentación,
/// horas de concentración o la sesión bloqueada.)
#[cfg(windows)]
fn windows_accepts() -> bool {
    use windows_sys::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_ACCEPTS_NOTIFICATIONS};
    let mut state = 0;
    // SAFETY: solo escribe en `state`.
    let hr = unsafe { SHQueryUserNotificationState(&mut state) };
    hr != 0 || state == QUNS_ACCEPTS_NOTIFICATIONS
}

#[cfg(not(windows))]
fn windows_accepts() -> bool {
    true
}

/// Muestra un aviso; al pulsarlo se abre la app en lo que corresponde.
#[cfg(windows)]
fn show(app: &AppHandle, notice: &Notice) {
    use tauri_winrt_notification::Toast;
    // Instalada, con su identificador (el acceso directo del menú Inicio lo
    // registra); en desarrollo, con el de PowerShell, que siempre existe.
    let installed = tauri::utils::platform::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.to_string_lossy().to_lowercase()))
        .is_some_and(|d| !d.ends_with("\\target\\debug") && !d.ends_with("\\target\\release"));
    let id = if installed { app.config().identifier.clone() } else { Toast::POWERSHELL_APP_ID.to_string() };
    let handle = app.clone();
    let target = notice.target.clone();
    let result = Toast::new(&id)
        .title(&notice.title)
        .text1(&notice.body)
        .on_activated(move |_| {
            request_view(&handle, target.clone());
            Ok(())
        })
        .show();
    if let Err(e) = result {
        eprintln!("No se pudo mostrar el aviso: {e}");
    }
}

#[cfg(not(windows))]
fn show(_app: &AppHandle, _notice: &Notice) {}

/// Hilo que vigila el estado y avisa.
pub fn start(app: AppHandle, dir: PathBuf) {
    std::thread::spawn(move || loop {
        tick(&app, &dir);
        std::thread::sleep(POLL);
    });
}

fn tick(app: &AppHandle, dir: &Path) {
    let seen = load_seen(dir);
    let (notices, next) = diff(&seen, &facts(app), Local::now());
    if next == seen {
        return;
    }
    let enabled = app.state::<crate::tray::Tray>().settings().notifications;
    if enabled && !notices.is_empty() {
        if !windows_accepts() {
            return; // se avisará cuando Windows los acepte
        }
        // Con la ventana delante, ya se ve en la app: no hace falta avisar.
        let looking = app.get_webview_window("main").is_some_and(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false));
        if !looking {
            for n in &notices {
                show(app, n);
            }
        }
    }
    save_seen(dir, &next);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Local> {
        DateTime::parse_from_rfc3339("2026-10-01T12:00:00-05:00").unwrap().with_timezone(&Local)
    }

    fn facts() -> Facts {
        let mut f = Facts::default();
        f.names.insert("r1".into(), "Disco".into());
        f.names.insert("r1#p1".into(), "Laboral".into());
        f.runs.insert("r1#p1".into(), "ok".into());
        f
    }

    fn ready() -> Seen {
        let (_, s) = diff(&Seen::default(), &facts(), now());
        s
    }

    #[test]
    fn la_primera_vez_solo_toma_nota() {
        let mut f = facts();
        f.runs.insert("r1#p1".into(), "error".into());
        f.holds.insert("r1".into(), "2026-10-01T10:00:00-05:00".into());
        f.kit_missing.push("r1".into());
        let (out, seen) = diff(&Seen::default(), &f, now());
        assert!(out.is_empty());
        assert!(seen.ready && seen.failing.contains("r1#p1") && seen.holds.contains_key("r1"));
        // El kit espera una semana.
        let (out, _) = diff(&seen, &f, now() + Duration::days(1));
        assert!(out.is_empty());
        let (out, _) = diff(&seen, &f, now() + Duration::days(8));
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0].target, Target::Kit { .. }));
    }

    #[test]
    fn avisa_una_vez_por_fallo() {
        let mut f = facts();
        f.runs.insert("r1#p1".into(), "error".into());
        let (out, seen) = diff(&ready(), &f, now());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "Falló la copia «Laboral»");
        assert_eq!(out[0].target, Target::Copy { repo_id: "r1".into(), plan_id: "p1".into() });
        // El reintento que vuelve a fallar no avisa otra vez.
        let (out, seen) = diff(&seen, &f, now());
        assert!(out.is_empty());
        // Tras una copia buena, un fallo nuevo sí.
        let (_, seen) = diff(&seen, &facts(), now());
        let (out, _) = diff(&seen, &f, now());
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn subida_frenada_y_luego_completada() {
        let mut f = facts();
        f.offsite.insert("r1".into(), ("2026-10-01T08:00:00-05:00".into(), "ok".into()));
        let (_, seen) = diff(&Seen::default(), &f, now());
        f.holds.insert("r1".into(), "2026-10-01T11:00:00-05:00".into());
        let (out, seen) = diff(&seen, &f, now());
        assert_eq!(out.len(), 1);
        assert!(out[0].title.contains("frenada"));
        // Se reanuda: aún no ha subido nada.
        f.holds.clear();
        let (out, seen) = diff(&seen, &f, now());
        assert!(out.is_empty());
        assert!(seen.released.contains("r1"));
        // Una subida que falla no cuenta; la siguiente buena, sí (una vez).
        f.offsite.insert("r1".into(), ("2026-10-01T12:30:00-05:00".into(), "error".into()));
        let (out, seen) = diff(&seen, &f, now());
        assert!(out.is_empty());
        f.offsite.insert("r1".into(), ("2026-10-01T13:00:00-05:00".into(), "ok".into()));
        let (out, seen) = diff(&seen, &f, now());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "Subida a la nube completada");
        assert!(seen.released.is_empty());
        let (out, _) = diff(&seen, &f, now());
        assert!(out.is_empty());
    }

    #[test]
    fn el_aviso_lleva_a_la_vista() {
        let t = serde_json::to_value(Target::Copy { repo_id: "r".into(), plan_id: "p".into() }).unwrap();
        assert_eq!(t, serde_json::json!({ "kind": "copy", "repoId": "r", "planId": "p" }));
        let t = serde_json::to_value(Target::Kit { ids: vec!["r".into()] }).unwrap();
        assert_eq!(t, serde_json::json!({ "kind": "kit", "ids": ["r"] }));
    }
}
