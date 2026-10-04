//! «Copias a distancia» (lado del equipo): la web —o Resguardo en otro equipo
//! de la misma cuenta— puede pedir «Copiar ahora» de un plan que ya existe.
//!
//! Nada más: ni restaurar, ni borrar, ni pausar, ni cambiar la configuración.
//! Cada equipo lo activa en local (desactivado por defecto; `agent.json`, que
//! solo cambia un administrador). El agente recoge las peticiones en cada
//! vuelta, valida cada una aquí y la hace como una copia más, por el mismo
//! camino que las programadas.

use crate::agent::AgentConfig;
use chrono::{DateTime, Duration, Local};
use serde::Deserialize;
use serde_json::json;

/// Una petición pendiente (`device_take_commands`).
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Command {
    pub id: String,
    pub repo_id: String,
    pub plan_id: String,
    #[serde(default)]
    pub kind: String,
    pub requested_at: String,
    /// Nombre del equipo (o «la web») desde el que se pidió.
    #[serde(default)]
    pub requested_from: Option<String>,
}

impl Command {
    /// «la web», «ESTUDIO»…
    pub fn origin_label(&self) -> String {
        match self.requested_from.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(from) => from.chars().take(60).collect(),
            None => "la web".into(),
        }
    }
}

/// Una petición vale como mucho media hora: más tarde ya no es «ahora».
pub const MAX_AGE_MIN: i64 = 30;

/// Comprueba una petición antes de hacerla. Devuelve la clave del plan
/// (`repo#plan`) o el motivo del rechazo (que se le devuelve a quien la pidió).
pub fn validate(cmd: &Command, config: &AgentConfig, now: DateTime<Local>) -> Result<String, String> {
    if cmd.kind != "backup" {
        return Err("Solo se pueden pedir copias a distancia.".into());
    }
    let Some(repo) = config.repos.iter().find(|r| r.id == cmd.repo_id) else {
        return Err("Ese repositorio no tiene copias automáticas en este equipo.".into());
    };
    if !repo.plans.iter().any(|p| p.id == cmd.plan_id) {
        return Err("Esa copia no está programada en este equipo.".into());
    }
    if repo.active_pause(now).is_some() {
        return Err("El repositorio está en pausa.".into());
    }
    let requested = DateTime::parse_from_rfc3339(&cmd.requested_at).map_err(|_| "Petición sin fecha válida.".to_string())?;
    let age = now.signed_duration_since(requested.with_timezone(&Local));
    if age > Duration::minutes(MAX_AGE_MIN) {
        return Err(format!("La petición caducó: llegó después de {MAX_AGE_MIN} minutos."));
    }
    Ok(crate::plans::plan_key(&repo.id, &cmd.plan_id))
}

/// Cuerpo de `device_take_commands`.
pub fn take_body(device: &str, secret: &str) -> serde_json::Value {
    json!({ "p_device": device, "p_secret": secret })
}

/// Cuerpo de `device_finish_command`. `status`: "done", "failed" o "rejected".
pub fn finish_body(device: &str, secret: &str, id: &str, status: &str, message: &str) -> serde_json::Value {
    json!({
        "p_device": device,
        "p_secret": secret,
        "p_id": id,
        "p_status": status,
        "p_message": crate::web::public_message(message),
    })
}

/// Recoge las peticiones pendientes de este equipo.
pub fn take(url: &str, key: &str, device: &str, secret: &str) -> Result<Vec<Command>, String> {
    let answer = crate::web::rpc(url, key, "device_take_commands", &take_body(device, secret))?;
    parse_take(&answer)
}

/// `{"commands":[…]}` (también con `"revoked":true` o `"too_soon":true`, que
/// no son errores: simplemente no hay nada que hacer ahora).
pub fn parse_take(answer: &serde_json::Value) -> Result<Vec<Command>, String> {
    match answer.get("commands") {
        None | Some(serde_json::Value::Null) => Ok(Vec::new()),
        Some(list) => serde_json::from_value(list.clone()).map_err(|e| format!("Respuesta inesperada de la web: {e}")),
    }
}

/// Avisa a la web de cómo terminó una petición.
pub fn finish(url: &str, key: &str, device: &str, secret: &str, id: &str, status: &str, message: &str) {
    if let Err(e) = crate::web::rpc(url, key, "device_finish_command", &finish_body(device, secret, id, status, message)) {
        crate::agent::log(&format!("No se pudo avisar a la web del final de una copia a distancia: {e}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{AgentConfig, AgentPlan, AgentRepo};

    fn config() -> AgentConfig {
        let plan = AgentPlan {
            id: "p1".into(),
            name: "Laboral".into(),
            paths: vec![],
            excludes: vec![],
            tags: vec![],
            schedule: crate::agent::plan_schedule_from_legacy(&crate::agent::Schedule::Daily { time: "10:00".into() }).unwrap(),
            enabled_at: "2026-10-01T09:00:00-05:00".into(),
            skip_unchanged: false,
            ganchos: vec![],
        };
        let repo: AgentRepo = serde_json::from_value(serde_json::json!({
            "id": "r1",
            "name": "Disco",
            "location": "D:\\copias",
            "paths": [],
            "schedule": { "kind": "plans" },
            "enabled_at": "2026-10-01T09:00:00-05:00",
        }))
        .unwrap();
        AgentConfig { repos: vec![AgentRepo { plans: vec![plan], ..repo }], ..Default::default() }
    }

    fn cmd(kind: &str, repo: &str, plan: &str, at: &str) -> Command {
        Command {
            id: "c1".into(),
            repo_id: repo.into(),
            plan_id: plan.into(),
            kind: kind.into(),
            requested_at: at.into(),
            requested_from: Some("ESTUDIO".into()),
        }
    }

    fn now() -> DateTime<Local> {
        DateTime::parse_from_rfc3339("2026-10-01T12:00:00-05:00").unwrap().with_timezone(&Local)
    }

    #[test]
    fn valida_las_peticiones() {
        let c = config();
        assert_eq!(validate(&cmd("backup", "r1", "p1", "2026-10-01T11:50:00-05:00"), &c, now()), Ok("r1#p1".into()));
        assert!(validate(&cmd("restore", "r1", "p1", "2026-10-01T11:50:00-05:00"), &c, now()).is_err());
        assert!(validate(&cmd("backup", "otro", "p1", "2026-10-01T11:50:00-05:00"), &c, now()).is_err());
        assert!(validate(&cmd("backup", "r1", "otro", "2026-10-01T11:50:00-05:00"), &c, now()).is_err());
        // Caducada (más de 30 min) o sin fecha.
        assert!(validate(&cmd("backup", "r1", "p1", "2026-10-01T11:20:00-05:00"), &c, now()).unwrap_err().contains("caducó"));
        assert!(validate(&cmd("backup", "r1", "p1", "ayer"), &c, now()).is_err());
        // En pausa.
        let mut paused = config();
        paused.repos[0].pause = Some(crate::agent::Pause { since: "2026-10-01T08:00:00-05:00".into(), until: None });
        assert_eq!(validate(&cmd("backup", "r1", "p1", "2026-10-01T11:50:00-05:00"), &paused, now()).unwrap_err(), "El repositorio está en pausa.");
    }

    #[test]
    fn cuerpos_de_las_llamadas_y_origen() {
        assert_eq!(take_body("d", "s"), serde_json::json!({ "p_device": "d", "p_secret": "s" }));
        let b = finish_body("d", "s", "c1", "failed", "open C:\\Users\\ana\\x.pst: en uso");
        assert_eq!(b["p_status"], "failed");
        assert!(!b["p_message"].as_str().unwrap().contains("ana"), "sin rutas: {b}");
        assert_eq!(cmd("backup", "r", "p", "x").origin_label(), "ESTUDIO");
        let web = Command { requested_from: None, ..cmd("backup", "r", "p", "x") };
        assert_eq!(web.origin_label(), "la web");
        let parsed = parse_take(&serde_json::json!({ "commands": [
            { "id": "1", "repo_id": "r", "plan_id": "p", "kind": "backup", "requested_at": "2026-10-01T11:50:00Z", "requested_from": "SERVIDOR-01" }
        ]}))
        .unwrap();
        assert_eq!(parsed[0].requested_from.as_deref(), Some("SERVIDOR-01"));
        assert!(parse_take(&serde_json::json!({ "revoked": true, "commands": [] })).unwrap().is_empty());
        assert!(parse_take(&serde_json::json!({ "commands": [], "too_soon": true })).unwrap().is_empty());
    }
}
