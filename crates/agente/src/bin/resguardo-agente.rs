// Resguardo Agente: el binario de los equipos gestionados (fase 5,
// docs/agente-gestionado.md). Sin ventana ni WebView2.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    resguardo_agente::harden_dll_search();
    std::process::exit(resguardo_agente::agente_main());
}
