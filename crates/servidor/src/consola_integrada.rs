//! La consola web dentro del binario (feature `consola-integrada`): los
//! archivos de `consola/build` se incluyen al compilar. `--consola DIR` tiene
//! prioridad (para probar otra versión sin recompilar).

use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};

#[derive(rust_embed::Embed)]
#[folder = "../../consola/build"]
struct Archivos;

/// ¿Lleva este binario la consola? (Sin `index.html`, la compilación no la encontró.)
pub fn disponible() -> bool {
    Archivos::get("index.html").is_some()
}

/// Sirve un archivo de la consola; las rutas que no son archivos van a `index.html` (SPA).
pub async fn servir(uri: Uri) -> Response {
    let ruta = uri.path().trim_start_matches('/');
    let (archivo, nombre) = match Archivos::get(ruta).filter(|_| !ruta.is_empty()) {
        Some(a) => (a, ruta),
        None if ruta.starts_with("_app/") => return StatusCode::NOT_FOUND.into_response(),
        None => match Archivos::get("index.html") {
            Some(a) => (a, "index.html"),
            None => return StatusCode::NOT_FOUND.into_response(),
        },
    };
    let tipo = archivo.metadata.mimetype().to_string();
    // Lo de `_app/immutable` lleva su hash en el nombre: se puede guardar para siempre.
    let cache = if nombre.starts_with("_app/immutable/") { "public, max-age=31536000, immutable" } else { "no-cache" };
    ([(header::CONTENT_TYPE, tipo), (header::CACHE_CONTROL, cache.to_string())], archivo.data.into_owned()).into_response()
}
