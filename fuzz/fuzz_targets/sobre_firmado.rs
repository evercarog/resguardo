//! El contenido de un sobre ya abierto (JSON con `payload` y `sig`): cualquier
//! byte debe dar un error, nunca un `panic`.
#![no_main]
use libfuzzer_sys::fuzz_target;
use resguardo_protocolo::mensajes;

const CONSOLA_PUB: &str = "6kpsY+KcUgq+9VB7Ey7F+ZVHdq6+vnuSQh7qaRRG0iw=";

fuzz_target!(|data: &[u8]| {
    let _ = mensajes::verify_signed(data, CONSOLA_PUB, "equipo-1", 0, chrono::Local::now());
    let _ = mensajes::code_hash(&String::from_utf8_lossy(data));
});
