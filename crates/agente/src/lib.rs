//! El agente de Resguardo y lo que comparte con la app de escritorio:
//! copias programadas (agent, tasks), configuración y secretos (store),
//! Servidor de copias (server), equipos gestionados (fase 5: endpoint,
//! console, managed, agente) y el agente v2 de Resguardo Server
//! (servidor_v2, cli). Sin Tauri: se compila también para Linux.
//! Ver docs/plataforma.md, «Repositorios».

// Fuera de Windows quedan sin usar las piezas de funciones que solo existen
// allí (bandeja, Programador de tareas, DPAPI…): no es un error.
#![cfg_attr(not(windows), allow(dead_code, unused_imports, unused_variables))]

pub use resguardo_motor::{plans, restic, retention, sizes};

static VERSION_APP: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();

/// La app de escritorio (0.6.x) dice aquí su versión al arrancar: lo que
/// informa a la web es la versión del programa que corre, no la de este crate
/// (que es la del agente de la plataforma, 0.7.x).
pub fn poner_version_app(v: &'static str) {
    let _ = VERSION_APP.set(v);
}

/// Versión del programa que se está ejecutando (la app o `resguardo-agente`).
pub fn version_programa() -> &'static str {
    VERSION_APP.get().copied().unwrap_or(env!("CARGO_PKG_VERSION"))
}

pub mod actualizacion;
#[cfg(test)]
mod actualizacion_it;
pub mod adoptar_v2;
pub mod agent;
pub mod agente;
pub mod ancla;
pub mod bandeja;
pub mod bitacora;
pub mod cli;
#[cfg(test)]
mod consolas_it;
pub mod consolas_v2;
pub mod console;
pub mod datos_equipo;
#[cfg(test)]
mod datos_equipo_it;
pub mod discover;
pub mod discreto;
pub mod endpoint;
pub mod escritorio;
pub mod espacio;
pub mod espejo;
pub mod espejo_motor;
#[cfg(test)]
mod espera_it;
pub mod espera_v2;
pub mod ganchos;
pub mod gestion_v2;
pub mod history;
pub mod informe_v2;
pub mod ipc_local;
pub mod jobs;
pub mod kit;
pub mod managed;
pub mod nube;
pub mod nube_anular;
#[cfg(test)]
mod perder_consola_it;
pub mod places;
pub mod platform;
pub mod progreso_v2;
#[cfg(test)]
mod propiedades;
pub mod protection;
pub mod remote;
pub mod restore_test;
pub mod retencion_almacen;
pub mod retencion_registro;
pub mod s3list;
pub mod server;
#[cfg(test)]
mod server_it;
pub mod servidor_v2;
pub mod sesiones_v2;
pub mod share;
pub mod store;
pub mod tasks;
#[cfg(test)]
mod traslado_it;
pub mod traslado_v2;
pub mod ventana;
pub mod volumenes;
pub mod web;

/// Orden de búsqueda de DLL seguro (ver platform::harden_dll_search).
pub fn harden_dll_search() {
    platform::harden_dll_search();
}

/// `resguardo.exe --server-run`: el Servidor de copias (lo lanza su tarea de SYSTEM).
pub fn server_main() -> i32 {
    server::run_forever()
}

/// `resguardo-agente.exe`: el agente de un equipo gestionado (fase 5).
pub fn agente_main() -> i32 {
    agente::main()
}

/// `resguardo.exe --server-stop` (desinstalador).
pub fn server_stop() {
    server::stop();
}

/// `resguardo.exe --agent-run`: lo ejecuta el Programador de tareas, sin ventana.
pub fn agent_main() -> i32 {
    let antes = huella_copias();
    let r = agent::run();
    // Agente gestionado: si cambió alguna copia, el resumen de la consola al día ya.
    if agent::is_managed_agent() && huella_copias() != antes {
        let gestionado = || servidor_v2::cargar().filter(|v| v.modo == "gestionado" && !v.secreto.is_empty());
        // v1.30: con las versiones de lo que acaba de copiar (si no, la consola veía la
        // copia terminada con «0 versiones» hasta el informe siguiente, 15–35 s después).
        if let Some(v) = gestionado() {
            informe_v2::refrescar_tras_copias(&v, servidor_v2::RELEER_TRAS_COPIA);
        }
        // El vínculo se vuelve a leer después: mientras restic leía, otro proceso pudo
        // cambiarlo (p. ej. `vincular` con otro servidor) y no se debe pisar.
        servidor_v2::subir_config_desde_fuera();
    }
    r
}

/// Cuándo terminó cada copia (para saber si algo cambió).
fn huella_copias() -> String {
    agent::load_state().runs.iter().map(|(k, r)| format!("{k}={};", r.finished)).collect()
}

/// `resguardo.exe --agent-tasks`: verificaciones y copias externas (lo lanza el agente).
pub fn agent_tasks_main() -> i32 {
    tasks::run()
}

/// `resguardo.exe --managed-maintenance`: retención de los equipos gestionados.
pub fn managed_maintenance() -> i32 {
    console::maintenance_main()
}

/// `resguardo.exe --agent-purge`: el desinstalador lo llama si el usuario pide
/// borrar también los datos del agente (requiere administrador).
pub fn agent_purge() -> i32 {
    match agent::purge() {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// `resguardo.exe --agent-sync-task`: el instalador lo llama al terminar. Al
/// actualizar, la desinstalación de la versión anterior quita la tarea del
/// agente; aquí se vuelve a crear si hay copias programadas o vínculo web.
pub fn agent_sync_task() -> i32 {
    // El Servidor de copias, si estaba activado (la actualización quitó su tarea).
    if server::load().enabled {
        if let Err(e) = platform::install_server_task(&agent::private_dir()) {
            agent::log(&format!("No se pudo recrear la tarea del Servidor de copias al instalar: {e}"));
        }
    }
    match agent::sync_task() {
        Ok(()) => 0,
        Err(e) => {
            agent::log(&format!("No se pudo recrear la tarea del agente al instalar: {e}"));
            1
        }
    }
}
