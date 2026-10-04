//! Mensajes BIEN firmados con contenido arbitrario (fechas, seq, tipo,
//! cuerpo): llega a la lógica de después de la firma (caducidad, seq), que
//! el fuzzing con bytes sueltos no alcanza.
#![no_main]
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use libfuzzer_sys::{arbitrary::Arbitrary, fuzz_target};
use resguardo_protocolo::mensajes;

#[derive(Arbitrary, Debug)]
struct Entrada {
    v: u32,
    endpoint: String,
    seq: u64,
    issued_at: String,
    kind: String,
    body: String,
    last_seq: u64,
}

fuzz_target!(|e: Entrada| {
    let b64 = base64::engine::general_purpose::STANDARD;
    let key = SigningKey::from_bytes(&[7u8; 32]);
    let public = b64.encode(key.verifying_key().to_bytes());
    let msg = serde_json::json!({
        "v": e.v, "endpoint": e.endpoint, "seq": e.seq, "issued_at": e.issued_at, "kind": e.kind,
        "body": serde_json::from_str::<serde_json::Value>(&e.body).unwrap_or(serde_json::Value::Null),
    });
    let payload = serde_json::to_vec(&msg).unwrap();
    let signed = serde_json::json!({ "payload": b64.encode(&payload), "sig": b64.encode(key.sign(&payload).to_bytes()) });
    let plain = serde_json::to_vec(&signed).unwrap();
    let _ = mensajes::verify_signed(&plain, &public, "equipo-1", e.last_seq, chrono::Local::now());
});
