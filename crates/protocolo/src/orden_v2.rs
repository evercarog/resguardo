//! Sobre de orden v2 (docs/api-servidor.md, §1): lo que la consola sella
//! para un equipo y el equipo comprueba antes de obedecer. El secreto que
//! autoriza la orden (prueba de la clave de administración o contraseña del
//! repositorio) viaja dentro del sobre: el servidor nunca lo ve.

use crate::claves;
use serde::{Deserialize, Serialize};

/// Margen para relojes desajustados.
pub const HOLGURA_S: i64 = 300;
/// Salto máximo del número de orden respecto al último aceptado.
pub const MAX_SALTO_SEQ: u64 = 100_000;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ClaveRepo {
    pub repo: String,
    pub contrasena: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Autorizacion {
    /// `prueba_e` en base64 (ver [`crate::derivaciones::prueba_admin`]).
    #[serde(default)]
    pub prueba_admin: Option<String>,
    #[serde(default)]
    pub clave_repo: Option<ClaveRepo>,
    /// Solo en la orden `alta`: `HMAC-SHA256(código de emparejamiento normalizado,
    /// "resguardo-alta-v1|" + equipo + "|" + verificador)`, en base64. Demuestra
    /// que la orden viene de quien vio el código (no del servidor, que solo tiene su hash).
    #[serde(default)]
    pub prueba_codigo: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct OrdenV2 {
    pub v: u32,
    pub cliente: String,
    pub equipo: String,
    pub seq: u64,
    /// 16 bytes aleatorios en base64: no se acepta dos veces.
    pub nonce: String,
    pub emitida: String,
    pub caduca: String,
    #[serde(default)]
    pub not_before: Option<String>,
    pub tipo: String,
    #[serde(default)]
    pub cuerpo: serde_json::Value,
    #[serde(default)]
    pub autorizacion: Autorizacion,
    /// X25519 pública efímera de la consola, para cifrarle el detalle del resultado.
    #[serde(default)]
    pub responder_a: Option<String>,
}

/// Sella una orden para la clave pública X25519 del equipo (lo hace la consola).
pub fn sellar(orden: &OrdenV2, box_pub_equipo: &str) -> Result<String, String> {
    claves::seal_bytes(box_pub_equipo, &serde_json::to_vec(orden).map_err(|e| e.to_string())?)
}

fn ts(s: &str) -> Result<i64, String> {
    chrono::DateTime::parse_from_rfc3339(s).map(|d| d.timestamp()).map_err(|_| "Fecha de la orden no válida.".to_string())
}

/// Lo que el equipo comprueba antes de mirar la autorización.
pub struct Contexto<'a> {
    pub cliente: &'a str,
    pub equipo: &'a str,
    pub ultimo_seq: u64,
    pub ahora: i64,
    /// Metadatos en claro que mandó el servidor: tienen que coincidir con los de dentro.
    pub tipo_meta: &'a str,
    pub seq_meta: u64,
}

/// Abre y valida una orden: destinatario, versión, `seq` creciente,
/// metadatos coherentes, fechas y tipo conocido. El `nonce` lo comprueba
/// quien llama (lleva la lista de los ya vistos).
pub fn abrir(sellado: &str, box_secret: &str, cx: &Contexto) -> Result<OrdenV2, String> {
    let plano = claves::open_bytes(box_secret, sellado)?;
    validar(&plano, cx)
}

/// La parte de [`abrir`] después de descifrar (también para el fuzzing).
pub fn validar(plano: &[u8], cx: &Contexto) -> Result<OrdenV2, String> {
    let o: OrdenV2 = serde_json::from_slice(plano).map_err(|_| "Orden no válida.".to_string())?;
    if o.v != 2 {
        return Err("Versión de orden no admitida.".into());
    }
    if o.cliente != cx.cliente || o.equipo != cx.equipo {
        return Err("La orden no es para este equipo.".into());
    }
    if o.tipo != cx.tipo_meta || o.seq != cx.seq_meta {
        return Err("Los datos de la orden no coinciden con los del servidor.".into());
    }
    if o.seq <= cx.ultimo_seq {
        return Err(format!("Orden repetida o antigua (n.º {} ≤ {}).", o.seq, cx.ultimo_seq));
    }
    // Un salto enorme dejaría el equipo sin poder aceptar órdenes nunca más
    // (un servidor comprometido puede sellar órdenes inofensivas).
    if o.seq - cx.ultimo_seq > MAX_SALTO_SEQ {
        return Err(format!("Número de orden fuera de rango (n.º {} tras {}).", o.seq, cx.ultimo_seq));
    }
    let tipo = crate::ordenes::tipo(&o.tipo).ok_or("Tipo de orden desconocido.")?;
    let (emitida, caduca) = (ts(&o.emitida)?, ts(&o.caduca)?);
    if emitida > cx.ahora + HOLGURA_S {
        return Err("La orden viene del futuro (¿hora mal puesta?).".into());
    }
    if caduca <= cx.ahora || caduca - emitida > crate::ordenes::MAX_CADUCIDAD_DIAS * 86_400 + HOLGURA_S {
        return Err("Orden caducada.".into());
    }
    let nb = o.not_before.as_deref().map(ts).transpose()?;
    if nb.is_some_and(|nb| nb > cx.ahora + HOLGURA_S) {
        return Err("Todavía no es la hora de esta orden.".into());
    }
    if tipo.destructiva && nb.is_none() {
        return Err("Una orden que reduce la protección tiene que llevar su espera (not_before).".into());
    }
    if o.nonce.len() > 64 {
        return Err("Orden no válida.".into());
    }
    Ok(o)
}

/// `prueba_codigo` de la orden `alta`.
pub fn prueba_codigo(codigo: &str, equipo: &str, verificador_b64: &str) -> String {
    use base64::Engine;
    use hmac::{Hmac, Mac};
    let norm: String = codigo.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_uppercase();
    let mut m = Hmac::<sha2::Sha256>::new_from_slice(norm.as_bytes()).expect("HMAC acepta cualquier longitud");
    m.update(format!("resguardo-alta-v1|{equipo}|{verificador_b64}").as_bytes());
    claves::B64.encode(m.finalize().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn orden(seq: u64, tipo: &str) -> OrdenV2 {
        let ahora = chrono::Local::now();
        OrdenV2 {
            v: 2,
            cliente: "c".into(),
            equipo: "e".into(),
            seq,
            nonce: "n".into(),
            emitida: ahora.to_rfc3339(),
            caduca: (ahora + chrono::Duration::hours(1)).to_rfc3339(),
            not_before: None,
            tipo: tipo.into(),
            cuerpo: serde_json::Value::Null,
            autorizacion: Autorizacion::default(),
            responder_a: None,
        }
    }

    fn cx(ultimo: u64, tipo: &'static str, seq: u64) -> Contexto<'static> {
        Contexto { cliente: "c", equipo: "e", ultimo_seq: ultimo, ahora: chrono::Utc::now().timestamp(), tipo_meta: tipo, seq_meta: seq }
    }

    #[test]
    fn sella_y_valida() {
        let k = claves::new_key();
        let s = sellar(&orden(5, "copiar_ahora"), &claves::public_of(&k).unwrap()).unwrap();
        assert_eq!(abrir(&s, &k, &cx(4, "copiar_ahora", 5)).unwrap().seq, 5);
        // Repetida, metadatos cambiados por el servidor, otro equipo, destructiva sin espera.
        assert!(abrir(&s, &k, &cx(5, "copiar_ahora", 5)).is_err());
        assert!(abrir(&s, &k, &cx(4, "baja_equipo", 5)).is_err());
        // Un salto enorme de seq (dejaría el equipo sin poder aceptar más órdenes).
        let lejos = orden(5 + MAX_SALTO_SEQ, "copiar_ahora");
        assert!(validar(&serde_json::to_vec(&lejos).unwrap(), &cx(4, "copiar_ahora", 5 + MAX_SALTO_SEQ)).unwrap_err().contains("fuera de rango"));
        let mut otra = orden(6, "copiar_ahora");
        otra.equipo = "x".into();
        assert!(validar(&serde_json::to_vec(&otra).unwrap(), &cx(4, "copiar_ahora", 6)).is_err());
        let pausa = orden(7, "pausar");
        assert!(validar(&serde_json::to_vec(&pausa).unwrap(), &cx(4, "pausar", 7)).unwrap_err().contains("espera"));
        let mut caducada = orden(8, "copiar_ahora");
        caducada.caduca = (chrono::Local::now() - chrono::Duration::minutes(1)).to_rfc3339();
        assert!(validar(&serde_json::to_vec(&caducada).unwrap(), &cx(4, "copiar_ahora", 8)).unwrap_err().contains("caducada"));
    }

    #[test]
    fn prueba_de_codigo() {
        let a = prueba_codigo("abcd-efgh-jk", "e", "V");
        assert_eq!(a, prueba_codigo("ABCD EFGH JK", "e", "V"));
        assert_ne!(a, prueba_codigo("ABCD-EFGH-JK", "e", "W"));
    }
}
