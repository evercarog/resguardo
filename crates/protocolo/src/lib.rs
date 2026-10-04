//! Protocolo de Resguardo: lo que comparten la app, el agente y el servidor
//! (docs/plataforma.md, docs/agente-gestionado.md, docs/compartir.md).
//!
//! - [`claves`]: claves X25519 de los equipos y sobres sellados.
//! - [`mensajes`]: mensajes de la consola firmados (Ed25519) y sellados para
//!   un equipo, con `seq` creciente y caducidad; códigos de emparejamiento y
//!   de comprobación (SAS).
//!
//! Los vectores de prueba de `vectors/` fijan el formato para otras
//! implementaciones (la consola web en JavaScript).

pub mod cifrado;
pub mod claves;
pub mod derivaciones;
pub mod instalador;
pub mod mensajes;
pub mod orden_v2;
pub mod ordenes;
pub mod paquete;
pub mod respaldo_consola;
pub mod simetrico;

#[cfg(test)]
mod humo;
#[cfg(test)]
mod vectores;
