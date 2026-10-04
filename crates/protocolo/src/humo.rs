//! Prueba de humo de los analizadores (la versión estable y rápida de fuzz/):
//! miles de entradas pseudoaleatorias y casi válidas no deben hacer `panic`.

use crate::claves::{self, B64};
use crate::mensajes;
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};

/// xorshift64*: determinista, sin dependencias.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn bytes(&mut self, max: usize) -> Vec<u8> {
        let n = (self.next() as usize) % (max + 1);
        (0..n).map(|_| self.next() as u8).collect()
    }
}

#[test]
fn entradas_arbitrarias_no_hacen_panic() {
    let mut rng = Rng(0x5EED_1234_ABCD_0001);
    let secret = claves::new_key();
    let console = SigningKey::from_bytes(&[7u8; 32]);
    let console_pub = B64.encode(console.verifying_key().to_bytes());
    let now = chrono::Local::now();
    for _ in 0..1_500 {
        let raw = rng.bytes(300);
        let as_b64 = B64.encode(&raw);
        let _ = claves::open_bytes(&secret, &as_b64);
        let _ = claves::open_bytes(&secret, &String::from_utf8_lossy(&raw));
        let _ = mensajes::open_message(&as_b64, &secret, &console_pub, "e", 0, now);
        let _ = mensajes::verify_signed(&raw, &console_pub, "e", 0, now);
        let _ = mensajes::code_hash(&String::from_utf8_lossy(&raw));
    }
}

#[test]
fn mensajes_bien_firmados_con_contenido_raro_no_hacen_panic() {
    let mut rng = Rng(0xDEAD_BEEF_0000_0042);
    let console = SigningKey::from_bytes(&[7u8; 32]);
    let console_pub = B64.encode(console.verifying_key().to_bytes());
    let fechas = ["", "x", "2026-10-02T10:00:00Z", "9999-12-31T23:59:59Z", "0000-01-01T00:00:00Z", "2026-10-02T10:00:00+23:59", "2026-13-40T99:99:99Z"];
    for i in 0..1_000u64 {
        let msg = serde_json::json!({
            "v": (rng.next() % 3) as u32,
            "endpoint": if rng.next().is_multiple_of(2) { "e" } else { "otro" },
            "seq": if i.is_multiple_of(7) { u64::MAX } else { rng.next() % 100 },
            "issued_at": fechas[(rng.next() as usize) % fechas.len()],
            "kind": String::from_utf8_lossy(&rng.bytes(20)),
            "body": serde_json::Value::Null,
        });
        let payload = serde_json::to_vec(&msg).unwrap();
        let signed = serde_json::json!({ "payload": B64.encode(&payload), "sig": B64.encode(console.sign(&payload).to_bytes()) });
        let plain = serde_json::to_vec(&signed).unwrap();
        let _ = mensajes::verify_signed(&plain, &console_pub, "e", rng.next() % 100, chrono::Local::now());
    }
}
