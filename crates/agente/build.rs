// La página de la ventana del agente (docs/agente-ventana.md): lo que deja
// `npm run build:ventana` (en consola/) en crates/agente/ventana va dentro del
// ejecutable. Aquí se hace la lista de esos archivos para `include_bytes!`.

use std::path::Path;

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("ventana");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut archivos: Vec<(String, String)> = Vec::new();
    if let Ok(entradas) = std::fs::read_dir(&dir) {
        for e in entradas.flatten() {
            let p = e.path();
            if !p.is_file() {
                continue;
            }
            let nombre = e.file_name().to_string_lossy().into_owned();
            // Solo lo que sirve la ventana (nada de mapas ni archivos ocultos).
            let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("");
            if nombre.starts_with('.') || !matches!(ext, "html" | "js" | "css" | "svg" | "png" | "woff2") {
                continue;
            }
            println!("cargo:rerun-if-changed={}", p.display());
            archivos.push((nombre, p.display().to_string().replace('\\', "/")));
        }
    }
    archivos.sort();
    let mut out = String::from("/// Los archivos de la ventana: (nombre, contenido).\npub static ARCHIVOS: &[(&str, &[u8])] = &[\n");
    for (nombre, ruta) in &archivos {
        out.push_str(&format!("    ({nombre:?}, include_bytes!({ruta:?})),\n"));
    }
    out.push_str("];\n");
    let destino = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("ventana_archivos.rs");
    std::fs::write(destino, out).expect("escribir la lista de la ventana");
}
