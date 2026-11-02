// Recursos de Windows del ejecutable (16z): datos de versión, icono y
// manifiesto, los mismos que el agente (crates/agente/recursos_windows.rs).

include!("../agente/recursos_windows.rs");

fn main() {
    recursos_windows("resguardo-server", "Resguardo Server", "Resguardo Server: consola de copias de seguridad");
}
