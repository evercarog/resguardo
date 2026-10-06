//! Ancla externa de la auditoría (plan-mejoras 9b; docs/plataforma.md §7.3.1).
//!
//! La cadena de la auditoría de cada cliente (`almacen/sqlite.rs`, `hash_entrada`)
//! detecta un cambio en medio, pero no que el servidor la rehaga entera desde el
//! principio. Para eso, la cabeza de la cadena (número, hora y huella de la última
//! entrada) sale del servidor de vez en cuando y se queda fuera:
//!
//! - en el resumen diario y semanal por correo (`notificaciones/resumen.rs`), con
//!   una línea `resguardo-ancla:1:…` que la consola sabe comprobar («Comprobar con
//!   un ancla», en Actividad);
//! - en los equipos del cliente, firmada con la identidad del servidor: en el `hola`
//!   del canal, en cada `tomar` y cada hora por el canal. El agente guarda las
//!   últimas y avisa a todas sus consolas si una retrocede (`agente/src/ancla.rs`).

use crate::almacen::{Almacen, ClienteCtx, Ts, R};
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// Cada cuánto se manda por el canal abierto (además de al abrirlo y en cada sondeo).
pub const CADA: std::time::Duration = std::time::Duration::from_secs(3600);

/// La cabeza de la cadena de un cliente.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Ancla {
    pub cliente: String,
    pub n: i64,
    /// La hora de esa entrada (segundos Unix): la misma que entra en su huella.
    pub creado: Ts,
    pub hash: String,
}

impl Ancla {
    /// La línea que va en el correo y que se pega en la consola.
    pub fn linea(&self) -> String {
        resguardo_protocolo::derivaciones::linea_ancla(&self.cliente, self.n.max(0) as u64, self.creado, &self.hash)
    }

    /// Lo que se manda al agente: el ancla y la firma de la identidad del servidor.
    pub fn firmada(&self, identidad: &SigningKey) -> Value {
        let texto = resguardo_protocolo::derivaciones::texto_ancla_auditoria(&self.cliente, self.n.max(0) as u64, self.creado, &self.hash);
        json!({
            "cliente": self.cliente,
            "n": self.n,
            "creado": self.creado,
            "hash": self.hash,
            "firma": B64.encode(identidad.sign(texto.as_bytes()).to_bytes()),
        })
    }
}

/// La última entrada de la auditoría del cliente, si tiene alguna.
pub fn de_cliente(db: &dyn Almacen, ctx: &ClienteCtx) -> R<Option<Ancla>> {
    Ok(db.auditoria_desc(ctx, None, 1)?.into_iter().next().map(|e| Ancla { cliente: ctx.id().to_string(), n: e.n, creado: e.creado, hash: e.hash }))
}

/// La de `de_cliente`, ya firmada (o `null`): un fallo al leerla no corta nada.
pub fn firmada_de(db: &dyn Almacen, ctx: &ClienteCtx, identidad: &SigningKey) -> Value {
    match de_cliente(db, ctx) {
        Ok(Some(a)) => a.firmada(identidad),
        _ => Value::Null,
    }
}
