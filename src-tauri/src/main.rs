// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Antes de nada: las DLL solo desde System32 y la carpeta del programa.
    resguardo_lib::harden_dll_search();
    // La versión que informa el agente (crate compartido) es la de la app.
    resguardo_lib::poner_version_app(env!("CARGO_PKG_VERSION"));
    // Modo agente: copias programadas, sin interfaz.
    if std::env::args().any(|a| a == "--agent-run") {
        std::process::exit(resguardo_lib::agent_main());
    }
    // Tareas de mantenimiento (verificar, copia externa): las lanza el agente.
    if std::env::args().any(|a| a == "--agent-tasks") {
        std::process::exit(resguardo_lib::agent_tasks_main());
    }
    // Lo usa el instalador: vuelve a crear la tarea del agente tras actualizar.
    // Lo usa el desinstalador si se pide borrar los datos del agente.
    if std::env::args().any(|a| a == "--agent-purge") {
        std::process::exit(resguardo_lib::agent_purge());
    }
    // Servidor de copias: lo arranca su tarea de SYSTEM al iniciar el equipo.
    if std::env::args().any(|a| a == "--server-run") {
        std::process::exit(resguardo_lib::server_main());
    }
    // Lo usa el desinstalador: para el servidor antes de quitar el programa.
    if std::env::args().any(|a| a == "--server-stop") {
        resguardo_lib::server_stop();
        std::process::exit(0);
    }
    // Retención de los equipos gestionados en el servidor (la lanza el agente).
    if std::env::args().any(|a| a == "--managed-maintenance") {
        std::process::exit(resguardo_lib::managed_maintenance());
    }
    if std::env::args().any(|a| a == "--agent-sync-task") {
        std::process::exit(resguardo_lib::agent_sync_task());
    }
    resguardo_lib::run()
}
