//! Vectores de prueba compartidos (Rust ↔ JavaScript): `vectors/v1.json`.
//!
//! La prueba comprueba que esta implementación reproduce el archivo. Para
//! regenerarlo (solo si cambia el formato, a propósito):
//! `RESGUARDO_GENERAR_VECTORES=1 cargo test -p resguardo-protocolo vectores`.

use crate::claves::{self, B64};
use crate::derivaciones as d;
use crate::mensajes::{self, Message};
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{json, Value};

const FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/v1.json");

/// Semillas fijas (solo para pruebas).
const SEED_CONSOLA: [u8; 32] = [7u8; 32];
const SECRET_EQUIPO: [u8; 32] = [9u8; 32];

fn deterministic() -> Value {
    let consola = SigningKey::from_bytes(&SEED_CONSOLA);
    let consola_pub = B64.encode(consola.verifying_key().to_bytes());
    let equipo_secret = B64.encode(SECRET_EQUIPO);
    let equipo_pub = claves::public_of(&equipo_secret).unwrap();
    let msg = Message {
        v: 1,
        endpoint: "equipo-1".into(),
        seq: 42,
        issued_at: "2026-10-02T10:00:00-05:00".into(),
        kind: "backup_now".into(),
        body: json!({ "plan": "documentos" }),
    };
    let payload = serde_json::to_vec(&msg).unwrap();
    let sig = consola.sign(&payload);
    json!({
        "version": 1,
        "nota": "Vectores de prueba del protocolo de Resguardo. Base64 estándar con relleno. Ver crates/protocolo.",
        "code_hash": [
            { "code": "ABCD-EFGH-JK", "hash": mensajes::code_hash("ABCD-EFGH-JK") },
            { "code": "abcd efgh jk", "hash": mensajes::code_hash("abcd efgh jk") },
        ],
        "sas": {
            "console_sign_pub": consola_pub,
            "endpoint_box_pub": equipo_pub,
            "sas": mensajes::sas(&consola_pub, &equipo_pub),
            "regla": "SHA-256(\"resguardo-sas-v1|\" + console_sign_pub + \"|\" + endpoint_box_pub); primeros 4 bytes big-endian mod 1000000; \"NNN NNN\"",
        },
        "x25519": { "secret": equipo_secret, "public": equipo_pub },
        "ed25519": {
            "seed": B64.encode(SEED_CONSOLA),
            "public": consola_pub,
            "message_json": String::from_utf8(payload.clone()).unwrap(),
            "signature": B64.encode(sig.to_bytes()),
        },
        "derivaciones": derivaciones(&equipo_pub, &consola_pub),
        "simetrico": simetrico(),
        "paquete": paquete(),
        "plantilla": plantilla(),
        "respaldo_consola": respaldo_consola(),
        "signed_envelope": {
            "regla": "plano del sobre = JSON {\"payload\": base64(message_json), \"sig\": base64(firma)}; después, sobre sellado X25519 (libsodium crypto_box_seal) para el equipo",
            "plain": serde_json::to_string(&json!({ "payload": B64.encode(&payload), "sig": B64.encode(sig.to_bytes()) })).unwrap(),
        },
    })
}

/// Huellas de ejemplo para el SAS v3 (con `:` y en mayúsculas, como `huella_ca`).
const HUELLA_CA: &str = "3F:A1:09:7C:52:E4:8B:D0:16:2A:9E:C3:75:0B:F8:44:D9:61:2C:A7:30:5E:B2:8F:E1:47:0A:96:CD:13:7B:58";
const HUELLA_OTRA: &str = "3F:A1:09:7C:52:E4:8B:D0:16:2A:9E:C3:75:0B:F8:44:D9:61:2C:A7:30:5E:B2:8F:E1:47:0A:96:CD:13:7B:59";

/// Clave de administración, sales y lo que se deriva (docs/api-servidor.md, §1).
fn derivaciones(box_pub: &str, sign_pub: &str) -> Value {
    let clave = "caballo bateria grapa correcta";
    let sal_equipo = B64.encode([3u8; 16]);
    let sal_cliente = B64.encode([4u8; 16]);
    let prueba = d::prueba_admin(clave, &sal_equipo).unwrap();
    let kcfg = d::k_cfg(clave, &sal_cliente).unwrap();
    let servidor = SigningKey::from_bytes(&[5u8; 32]);
    let identidad = B64.encode(servidor.verifying_key().to_bytes());
    let reto = B64.encode([6u8; 32]);
    let texto = d::texto_identidad_servidor(&reto, "equipo-1");
    json!({
        "clave_admin": clave,
        "argon2id": { "m_kib": d::ARGON2_M_KIB, "t": d::ARGON2_T, "p": d::ARGON2_P, "longitud": 32 },
        "sal_equipo": sal_equipo,
        "prueba_admin": B64.encode(prueba),
        "verificador": B64.encode(d::verificador(&prueba)),
        "sal_cliente": sal_cliente,
        "argon2id_cliente": B64.encode(d::argon2id(clave, &[4u8; 16]).unwrap()),
        "k_cfg": B64.encode(kcfg),
        "k_exp": B64.encode(d::k_exp(clave, &sal_cliente).unwrap()),
        "etiqueta": { "equipo": "equipo-1", "box_pub": box_pub, "sign_pub": sign_pub, "valor": d::etiqueta_equipo(&kcfg, "equipo-1", box_pub, sign_pub) },
        "sas_v2": { "identidad_servidor": identidad, "box_pub": box_pub, "sign_pub": sign_pub, "sas": d::sas_v2(&identidad, box_pub, sign_pub) },
        "sas_v3": {
            "identidad_servidor": identidad, "box_pub": box_pub, "sign_pub": sign_pub,
            "huella_ca": HUELLA_CA, "huella_normalizada": d::huella_para_sas(HUELLA_CA),
            "sas": d::sas_v3(&identidad, box_pub, sign_pub, HUELLA_CA),
            // Otra autoridad (alguien en medio): otro número.
            "huella_otra": HUELLA_OTRA, "sas_otra": d::sas_v3(&identidad, box_pub, sign_pub, HUELLA_OTRA),
            // Sin TLS (http:// en el mismo equipo y el servidor sin autoridad): huella vacía.
            "sas_sin_huella": d::sas_v3(&identidad, box_pub, sign_pub, ""),
        },
        "identidad_servidor": { "seed": B64.encode([5u8; 32]), "publica": identidad, "reto": reto, "equipo": "equipo-1", "texto": texto, "firma": B64.encode(servidor.sign(texto.as_bytes()).to_bytes()) },
        "resultado": { "texto": d::texto_resultado("orden-1", 18, "hecha", Some("Copia hecha."), None) },
        // La clave se normaliza a NFC antes de Argon2id: la escrita descompuesta da lo mismo.
        "nfc": {
            "clave_nfd": "Contrasen\u{303}a de ma\u{301}quina",
            "clave_nfc": d::normalizar_clave("Contrasen\u{303}a de ma\u{301}quina"),
            "sal_equipo": sal_equipo,
            "prueba_admin": B64.encode(d::prueba_admin("Contrasen\u{303}a de ma\u{301}quina", &sal_equipo).unwrap()),
        },
    })
}

/// Paquete de exportación (crates/protocolo/src/paquete.rs), con K_exp de la clave de los vectores.
fn paquete() -> Value {
    let sal_cliente = B64.encode([4u8; 16]);
    let k_exp = d::k_exp("caballo bateria grapa correcta", &sal_cliente).unwrap();
    let json = r#"{"v":1,"cliente":{"nombre":"Sur"}}"#;
    let p = crate::paquete::cifrar_con(&k_exp, &sal_cliente, json.as_bytes(), |_| [15u8; 24]);
    json!({
        "k_exp": B64.encode(k_exp),
        "sal_cliente": sal_cliente,
        "json": json,
        "nonce": B64.encode([15u8; 24]),
        "paquete": B64.encode(p),
        "regla": "\"RESGUARDO-CLIENTE-1\\n\" + sal_cliente_b64 + \"\\n\" + por cada trozo de 4 MiB: u32 big-endian(longitud) + nonce(24) + XChaCha20-Poly1305(K_exp, nonce, datos, aad = \"resguardo-cliente-v1|\" + n + \"|\" + (último ? 1 : 0))",
    })
}

/// Clave pública de la clave de respaldo de la consola (v1.23, «Copia de la consola»):
/// la calcula la consola en el navegador y es lo único que guarda el servidor.
fn respaldo_consola() -> Value {
    let clave = "Ñandú gris 2026 · respaldo de la consola";
    let sal = B64.encode([12u8; 16]);
    let secreto = crate::respaldo_consola::secreto(clave, &sal).unwrap();
    json!({
        "clave": clave,
        "sal": sal,
        "secreto": B64.encode(secreto),
        "publica": crate::respaldo_consola::publica_de(&secreto),
        "regla": "secreto = HKDF-SHA256(Argon2id(NFC(clave), sal, m=65536, t=3, p=1, 32), salt = \"\", info = \"resguardo-respaldo-consola-v1\"); publica = X25519(secreto) (clave pública de crypto_box)",
    })
}

/// Sesiones, relé y configuración (crates/protocolo/src/simetrico.rs), con nonces fijos.
fn simetrico() -> Value {
    use crate::simetrico as s;
    let clave_sesion = [8u8; 32];
    let sesion = "0f0e0d0c-0b0a-4908-8706-050403020100";
    let kc = s::clave_direccion(&clave_sesion, sesion, s::Lado::Consola);
    let ke = s::clave_direccion(&clave_sesion, sesion, s::Lado::Equipo);
    let msg = r#"{"i":1,"op":"carpetas","ruta":"C:\\Users"}"#;
    let clave_relevo = [10u8; 32];
    let k_cfg = [11u8; 32];
    let cfg = r#"{"v":1,"copias":[],"repositorios":[],"destinos":[]}"#;
    json!({
        "sesion": {
            "clave_sesion": B64.encode(clave_sesion),
            "sesion_id": sesion,
            "k_consola": B64.encode(kc),
            "k_equipo": B64.encode(ke),
            "json": msg,
            "nonce": B64.encode([12u8; 24]),
            "cifrado_consola": B64.encode(s::cifrar_mensaje(&kc, sesion, msg.as_bytes(), &[12u8; 24])),
        },
        "relevo": {
            "clave": B64.encode(clave_relevo),
            "relevo_id": "relevo-1",
            "n": 2,
            "ultimo": true,
            "datos": B64.encode(b"trozo final"),
            "nonce": B64.encode([13u8; 24]),
            "cifrado": B64.encode(s::cifrar_trozo(&clave_relevo, "relevo-1", 2, true, b"trozo final", &[13u8; 24])),
        },
        "config": {
            "k_cfg": B64.encode(k_cfg),
            "equipo": "equipo-1",
            "seq": 7,
            "json": cfg,
            "nonce": B64.encode([14u8; 24]),
            "cifrado": B64.encode(s::cifrar_config(&k_cfg, "equipo-1", 7, cfg.as_bytes(), &[14u8; 24])),
        },
    })
}

/// Plantillas de copia (v1.20): clave derivada de K_cfg y cifrado atado al cliente y al id.
fn plantilla() -> Value {
    use crate::simetrico as s;
    let k_cfg = [11u8; 32];
    let k_pla = s::clave_plantillas(&k_cfg);
    let json = r#"{"v":1,"id":"pla-1","nombre":"Copia de Siigo","copia":{"carpetas":["C:\\SIIWI01"],"exclusiones":["*.tmp"],"horario":{"dias":[1,2,3,4,5],"horas":["13:00"]},"solo_si_cambios":true,"gancho":null}}"#;
    json!({
        "k_cfg": B64.encode(k_cfg),
        "k_plantillas": B64.encode(k_pla),
        "cliente": "cliente-1",
        "id": "pla-1",
        "json": json,
        "nonce": B64.encode([16u8; 24]),
        "cifrado": B64.encode(s::cifrar_plantilla(&k_pla, "cliente-1", "pla-1", json.as_bytes(), &[16u8; 24])),
    })
}

#[test]
fn vectores_v1() {
    let det = deterministic();
    if std::env::var("RESGUARDO_GENERAR_VECTORES").is_ok() {
        // El sobre sellado es aleatorio (clave efímera): se genera una vez y se guarda.
        let mut out = det.clone();
        let plain = det["signed_envelope"]["plain"].as_str().unwrap();
        out["signed_envelope"]["sealed"] = json!(claves::seal_bytes(det["x25519"]["public"].as_str().unwrap(), plain.as_bytes()).unwrap());
        std::fs::write(FILE, serde_json::to_string_pretty(&out).unwrap() + "\n").unwrap();
    }
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(FILE).expect("falta vectors/v1.json")).unwrap();
    // Todo lo determinista coincide.
    for key in ["code_hash", "sas", "x25519", "ed25519", "derivaciones", "simetrico", "paquete", "plantilla", "respaldo_consola"] {
        assert_eq!(saved[key], det[key], "el vector «{key}» ya no coincide: ¿cambió el formato?");
    }
    assert_eq!(saved["signed_envelope"]["plain"], det["signed_envelope"]["plain"]);
    // El sobre guardado se abre y se acepta con la clave fijada.
    let sealed = saved["signed_envelope"]["sealed"].as_str().unwrap();
    let secret = saved["x25519"]["secret"].as_str().unwrap();
    let now = chrono::DateTime::parse_from_rfc3339("2026-10-02T11:00:00-05:00").unwrap().with_timezone(&chrono::Local);
    let msg = mensajes::open_message(sealed, secret, saved["ed25519"]["public"].as_str().unwrap(), "equipo-1", 41, now).unwrap();
    assert_eq!((msg.seq, msg.kind.as_str()), (42, "backup_now"));
}
