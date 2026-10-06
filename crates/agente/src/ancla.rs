//! Anclas de la auditoría de cada consola (v1.50, plan-mejoras 9b; docs/plataforma.md
//! §7.3.1 y docs/api-servidor.md, «Cambios»).
//!
//! Cada consola manda, firmada con su identidad (la que el equipo fijó al vincular),
//! la cabeza de la cadena de la auditoría de su cliente: número de la última entrada,
//! su hora y su huella (en el `hola` del canal, en cada `tomar` y cada hora por
//! el canal). El equipo guarda las últimas de cada consola
//! (`privado/anclas-auditoria.json`) y, si una retrocede, lo anota en su bitácora
//! (`auditoria_rehecha`), que llega a **todas** sus consolas:
//!
//! - la misma entrada con otra huella: la consola rehízo su cadena;
//! - un número menor que el más alto que ya dio: la cadena perdió entradas (rehecha
//!   o restaurada de una copia anterior de la consola).
//!
//! Solo mira la cabeza: una consola que rehaga la cadena entera y la deje más larga
//! que la última vez no se nota aquí (eso lo comprueba «Comprobar con un ancla» en la
//! consola, con el ancla de un correo anterior). Tras avisar, el equipo sigue con la
//! cadena nueva (no se queda avisando con cada ancla). Una consola anterior no manda
//! ancla y no pasa nada.

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
const ARCHIVO: &str = "anclas-auditoria.json";
/// Anclas que se guardan de cada consola (una al día como mucho, más la última).
pub const POR_CONSOLA: usize = 30;
/// Entre dos anclas guardadas (salvo la última, que se va poniendo al día).
const ENTRE_GUARDADAS_S: i64 = 86_400;
/// Consolas recordadas como mucho (las de menos uso se olvidan).
const MAX_CONSOLAS: usize = 64;

/// Una cabeza de la cadena tal como la dio una consola.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Ancla {
    pub n: u64,
    /// La hora de esa entrada en la consola (segundos Unix).
    pub creado: i64,
    pub hash: String,
    /// Cuándo la recibió el equipo (segundos Unix).
    pub recibida: i64,
}

#[derive(Serialize, Deserialize, Default)]
struct Archivo {
    /// `identidad|cliente` → anclas, de la más antigua a la más reciente.
    #[serde(default)]
    consolas: BTreeMap<String, Vec<Ancla>>,
}

/// Lo que dice una ancla nueva frente a las guardadas.
#[derive(Debug, PartialEq, Eq)]
pub enum Veredicto {
    /// La primera, o una que sigue a las anteriores.
    Sigue,
    /// La cadena retrocedió: (la que lo demuestra, la nueva).
    Rehecha(Ancla, Ancla),
}

/// Compara sin tocar nada: la misma entrada con otra huella, o un número menor que el
/// más alto ya visto, es una cadena rehecha.
pub fn comprobar(guardadas: &[Ancla], nueva: &Ancla) -> Veredicto {
    if let Some(a) = guardadas.iter().find(|a| a.n == nueva.n && a.hash != nueva.hash) {
        return Veredicto::Rehecha(a.clone(), nueva.clone());
    }
    if let Some(max) = guardadas.iter().max_by_key(|a| a.n).filter(|m| m.n > nueva.n) {
        return Veredicto::Rehecha(max.clone(), nueva.clone());
    }
    Veredicto::Sigue
}

/// Añade la nueva (si sigue): la última siempre al día y, por detrás, una al día como
/// mucho, hasta [`POR_CONSOLA`].
pub fn anadir(guardadas: &mut Vec<Ancla>, nueva: Ancla) {
    if let Some(u) = guardadas.last_mut() {
        if u.n == nueva.n {
            u.recibida = nueva.recibida;
            return;
        }
    }
    let n = guardadas.len();
    if n >= 2 && nueva.recibida - guardadas[n - 2].recibida < ENTRE_GUARDADAS_S {
        guardadas[n - 1] = nueva;
    } else {
        guardadas.push(nueva);
    }
    if guardadas.len() > POR_CONSOLA {
        let sobra = guardadas.len() - POR_CONSOLA;
        guardadas.drain(..sobra);
    }
}

/// El ancla de un mensaje de la consola (`{ cliente, n, creado, hash, firma }`), si es de
/// este cliente, tiene su forma y la firma la identidad fijada. Si no, `None` (no se usa).
pub fn leer(valor: &Value, identidad_b64: &str, cliente: &str, ahora: i64) -> Option<Ancla> {
    let n = valor.get("n")?.as_u64().filter(|n| *n >= 1 && *n < i64::MAX as u64)?;
    let creado = valor.get("creado")?.as_i64()?;
    let hash = valor.get("hash")?.as_str()?;
    if valor.get("cliente")?.as_str()? != cliente || hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return None;
    }
    let firma: [u8; 64] = B64.decode(valor.get("firma")?.as_str()?).ok()?.try_into().ok()?;
    let clave: [u8; 32] = B64.decode(identidad_b64).ok()?.try_into().ok()?;
    let texto = resguardo_protocolo::derivaciones::texto_ancla_auditoria(cliente, n, creado, hash);
    ed25519_dalek::VerifyingKey::from_bytes(&clave).ok()?.verify_strict(texto.as_bytes(), &ed25519_dalek::Signature::from_bytes(&firma)).ok()?;
    Some(Ancla { n, creado, hash: hash.to_string(), recibida: ahora })
}

fn leer_archivo(path: &Path) -> Archivo {
    std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn guardar_archivo(path: &Path, a: &Archivo) -> std::io::Result<()> {
    let b = serde_json::to_vec_pretty(a).map_err(std::io::Error::other)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("tmp-{}", uuid::Uuid::new_v4().simple()));
    std::fs::write(&tmp, b)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Un solo cambio del archivo a la vez en este proceso (varias consolas, cada una en su hilo).
static CERROJO: Mutex<()> = Mutex::new(());

/// Lo que se anota en la bitácora cuando una consola rehízo su cadena (sin su dirección:
/// solo el nombre que le dio el equipo y su identidad).
pub fn entrada_bitacora(nombre: &str, identidad: &str, antes: &Ancla, ahora: &Ancla) -> Value {
    let cabeza = |a: &Ancla| json!({ "n": a.n, "creado": a.creado, "hash": a.hash });
    json!({
        "consola": Some(nombre.trim()).filter(|n| !n.is_empty()),
        "identidad": identidad,
        "antes": cabeza(antes),
        "ahora": cabeza(ahora),
        "motivo": if ahora.n < antes.n { "retrocede" } else { "otra_huella" },
    })
}

/// Recibe el ancla de una consola (`valor`, `null` si no la manda): la comprueba con las
/// guardadas en `path` y la guarda. Devuelve la entrada de la bitácora si la cadena se rehízo.
pub fn recibir_en(path: &Path, identidad: &str, cliente: &str, nombre: &str, valor: &Value, ahora: i64) -> Option<Value> {
    if valor.is_null() {
        return None;
    }
    let nueva = leer(valor, identidad, cliente, ahora)?;
    let _g = CERROJO.lock().unwrap_or_else(|e| e.into_inner());
    let mut archivo = leer_archivo(path);
    let clave = format!("{identidad}|{cliente}");
    let lista = archivo.consolas.entry(clave.clone()).or_default();
    let aviso = match comprobar(lista, &nueva) {
        Veredicto::Sigue => {
            let antes = lista.clone();
            anadir(lista, nueva);
            if *lista == antes {
                return None; // nada que escribir
            }
            None
        }
        Veredicto::Rehecha(antes, ahora_) => {
            // Se avisa una vez y se sigue con la cadena nueva.
            *lista = vec![ahora_.clone()];
            Some(entrada_bitacora(nombre, identidad, &antes, &ahora_))
        }
    };
    if archivo.consolas.len() > MAX_CONSOLAS {
        // Las que hace más que no mandan nada (nunca la de ahora).
        let mut por_uso: Vec<(i64, String)> =
            archivo.consolas.iter().filter(|(k, _)| **k != clave).map(|(k, v)| (v.last().map_or(0, |a| a.recibida), k.clone())).collect();
        por_uso.sort();
        for (_, k) in por_uso.into_iter().take(archivo.consolas.len() - MAX_CONSOLAS) {
            archivo.consolas.remove(&k);
        }
    }
    if let Err(e) = guardar_archivo(path, &archivo) {
        crate::agent::log(&format!("No se pudo guardar el ancla de la auditoría: {e}"));
    }
    aviso
}

pub fn archivo() -> PathBuf {
    crate::agent::private_dir().join(ARCHIVO)
}

/// Lo de `recibir_en` con el vínculo de una consola, y si se rehízo, a la bitácora (que
/// llega a todas las consolas) y al registro del equipo.
pub fn recibir(v: &crate::servidor_v2::Vinculo, valor: &Value) {
    let Some(entrada) = recibir_en(&archivo(), &v.identidad, &v.cliente_id, &v.nombre_consola, valor, chrono::Utc::now().timestamp()) else { return };
    let quien = crate::consolas_v2::nombre_de(&v.nombre_consola, &v.url);
    crate::agent::log(&format!(
        "La consola {quien} rehízo su registro de actividad (antes, entrada n.º {}; ahora, n.º {}): se avisa a todas las consolas.",
        entrada["antes"]["n"], entrada["ahora"]["n"]
    ));
    crate::bitacora::anotar("auditoria_rehecha", entrada);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    const CLIENTE: &str = "cliente-norte";

    /// Una carpeta temporal que se borra al terminar.
    struct Temporal(PathBuf);
    impl Temporal {
        fn nueva() -> Self {
            let p = std::env::temp_dir().join(format!("resguardo-ancla-{}", uuid::Uuid::new_v4().simple()));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Temporal {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn h(c: char) -> String {
        c.to_string().repeat(64)
    }

    fn firmada(key: &SigningKey, cliente: &str, n: u64, creado: i64, hash: &str) -> Value {
        let texto = resguardo_protocolo::derivaciones::texto_ancla_auditoria(cliente, n, creado, hash);
        json!({ "cliente": cliente, "n": n, "creado": creado, "hash": hash, "firma": B64.encode(key.sign(texto.as_bytes()).to_bytes()) })
    }

    fn a(n: u64, hash: &str, recibida: i64) -> Ancla {
        Ancla { n, creado: 1_700_000_000 + n as i64, hash: hash.into(), recibida }
    }

    #[test]
    fn veredictos() {
        let g = vec![a(10, &h('a'), 0), a(20, &h('b'), 100_000)];
        assert_eq!(comprobar(&g, &a(25, &h('c'), 1)), Veredicto::Sigue, "sigue creciendo");
        assert_eq!(comprobar(&g, &a(20, &h('b'), 1)), Veredicto::Sigue, "la misma");
        assert_eq!(comprobar(&[], &a(1, &h('c'), 1)), Veredicto::Sigue, "la primera");
        assert!(matches!(comprobar(&g, &a(20, &h('c'), 1)), Veredicto::Rehecha(x, _) if x.n == 20), "misma entrada, otra huella");
        assert!(matches!(comprobar(&g, &a(10, &h('d'), 1)), Veredicto::Rehecha(x, _) if x.n == 10), "una antigua, cambiada");
        assert!(matches!(comprobar(&g, &a(15, &h('e'), 1)), Veredicto::Rehecha(x, _) if x.n == 20), "retrocede");
    }

    #[test]
    fn una_al_dia_y_la_ultima_al_dia() {
        let mut g = Vec::new();
        for i in 0..100u64 {
            anadir(&mut g, a(i + 1, &h('a'), i as i64 * 3600)); // una por hora durante 100 h
        }
        assert_eq!(g.last().unwrap().n, 100, "la última siempre");
        assert!(g.len() <= 6, "unas pocas: {}", g.len());
        assert!(g.windows(2).all(|w| w[0].n < w[1].n));
        let mut g = Vec::new();
        for i in 0..200u64 {
            anadir(&mut g, a(i + 1, &h('a'), i as i64 * 86_400));
        }
        assert_eq!(g.len(), POR_CONSOLA, "con tope");
        assert_eq!(g.last().unwrap().n, 200);
        // La misma otra vez: solo se pone al día cuándo se vio.
        let antes = g.len();
        anadir(&mut g, a(200, &h('a'), 999_999_999));
        assert_eq!((g.len(), g.last().unwrap().recibida), (antes, 999_999_999));
    }

    #[test]
    fn solo_con_la_firma_de_la_identidad_y_del_cliente() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let otra = SigningKey::from_bytes(&[8u8; 32]);
        let id = B64.encode(key.verifying_key().to_bytes());
        let v = firmada(&key, CLIENTE, 5, 1_700_000_000, &h('a'));
        assert_eq!(leer(&v, &id, CLIENTE, 9).map(|x| (x.n, x.recibida)), Some((5, 9)));
        assert!(leer(&firmada(&otra, CLIENTE, 5, 1_700_000_000, &h('a')), &id, CLIENTE, 9).is_none(), "otra identidad");
        assert!(leer(&v, &id, "otro-cliente", 9).is_none(), "otro cliente");
        let mut cambiada = v.clone();
        cambiada["n"] = json!(6);
        assert!(leer(&cambiada, &id, CLIENTE, 9).is_none(), "firma de otra entrada");
        assert!(leer(&firmada(&key, CLIENTE, 5, 0, "ABC"), &id, CLIENTE, 9).is_none(), "huella sin su forma");
        assert!(leer(&firmada(&key, CLIENTE, 0, 0, &h('a')), &id, CLIENTE, 9).is_none(), "n.º 0");
        assert!(leer(&json!({ "n": "x" }), &id, CLIENTE, 9).is_none());
        assert!(leer(&Value::Null, &id, CLIENTE, 9).is_none());
    }

    #[test]
    fn detecta_una_cadena_rehecha_y_lo_recuerda() {
        let dir = Temporal::nueva();
        let path = dir.0.join(ARCHIVO);
        let key = SigningKey::from_bytes(&[3u8; 32]);
        let id = B64.encode(key.verifying_key().to_bytes());
        let recibir =
            |n: u64, hash: &str, ahora: i64| recibir_en(&path, &id, CLIENTE, "Consola en línea", &firmada(&key, CLIENTE, n, 1_700_000_000, hash), ahora);
        assert!(recibir(40, &h('a'), 1000).is_none());
        assert!(recibir(41, &h('b'), 2000).is_none());
        assert!(recibir(41, &h('b'), 3000).is_none(), "la misma otra vez");
        // Otro proceso (el archivo se lee de nuevo): sigue sabiendo lo de antes.
        let guardado: Archivo = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(guardado.consolas[&format!("{id}|{CLIENTE}")].last().unwrap().n, 41);
        // La consola restaura una copia anterior o rehace la cadena: n.º 39.
        let e = recibir(39, &h('c'), 4000).expect("retrocede");
        assert_eq!((e["antes"]["n"].as_u64(), e["ahora"]["n"].as_u64(), e["motivo"].as_str()), (Some(41), Some(39), Some("retrocede")));
        assert_eq!(e["consola"], "Consola en línea");
        assert!(!e.to_string().contains("http"), "sin la dirección de la consola");
        // Se avisa una vez: la cadena nueva sigue sin más avisos.
        assert!(recibir(40, &h('d'), 5000).is_none());
        // La misma entrada con otra huella.
        let e = recibir(40, &h('e'), 6000).expect("otra huella");
        assert_eq!(e["motivo"], "otra_huella");
        // Un ancla falsa (otra firma) no cuenta para nada.
        let falsa = firmada(&SigningKey::from_bytes(&[4u8; 32]), CLIENTE, 1, 0, &h('f'));
        assert!(recibir_en(&path, &id, CLIENTE, "", &falsa, 7000).is_none());
        assert!(recibir(41, &h('g'), 8000).is_none(), "la falsa no se guardó");
        // Sin ancla (servidor anterior): nada.
        assert!(recibir_en(&path, &id, CLIENTE, "", &Value::Null, 9000).is_none());
        // Cada consola por su lado.
        let otra = SigningKey::from_bytes(&[5u8; 32]);
        let id2 = B64.encode(otra.verifying_key().to_bytes());
        assert!(recibir_en(&path, &id2, CLIENTE, "", &firmada(&otra, CLIENTE, 2, 0, &h('a')), 9000).is_none());
        assert!(recibir(41, &h('g'), 9500).is_none());
    }

    #[test]
    fn archivo_danado_empieza_de_cero() {
        let dir = Temporal::nueva();
        let path = dir.0.join(ARCHIVO);
        std::fs::write(&path, b"{no es json").unwrap();
        let key = SigningKey::from_bytes(&[9u8; 32]);
        let id = B64.encode(key.verifying_key().to_bytes());
        assert!(recibir_en(&path, &id, CLIENTE, "", &firmada(&key, CLIENTE, 3, 0, &h('a')), 1).is_none());
        assert!(recibir_en(&path, &id, CLIENTE, "", &firmada(&key, CLIENTE, 2, 0, &h('a')), 2).is_some());
    }
}
