//! Un sobre sellado cualquiera (lo que llega de la red) no debe hacer `panic`
//! al abrirlo, ni como sobre suelto ni como mensaje de la consola.
#![no_main]
use libfuzzer_sys::fuzz_target;
use resguardo_protocolo::{claves, mensajes};

const SECRET: &str = "CQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQk="; // la de vectors/v1.json
const CONSOLA_PUB: &str = "6kpsY+KcUgq+9VB7Ey7F+ZVHdq6+vnuSQh7qaRRG0iw="; // la de vectors/v1.json

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = claves::open_bytes(SECRET, s);
        let _ = mensajes::open_message(s, SECRET, CONSOLA_PUB, "equipo-1", 0, chrono::Local::now());
        let _ = claves::public_of(s);
    }
});
