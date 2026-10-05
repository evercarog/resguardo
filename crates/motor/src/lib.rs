//! Motor de copias de Resguardo: lo que comparten la app, el agente y (más
//! adelante) el servidor. Ver docs/plataforma.md, «Repositorios».
//!
//! - [`restic`]: ejecutar restic con seguridad (sin shell, secretos por
//!   variables de entorno, límites de tiempo, errores en español).
//! - [`plans`]: planes de copia y sus horarios.
//! - [`retention`]: políticas de retención (`forget`).
//! - [`sizes`]: lo que más ocupa de una versión.
//! - [`diferencias`]: qué cambió entre dos versiones, archivo por archivo.
//! - [`buscar`]: buscar archivos por su nombre en todas las versiones.
//! - [`proceso`]: prioridad baja de los procesos y herramientas del sistema.
//! - [`tls`]: autoridad propia y certificados de servidor.

pub mod buscar;
pub mod diferencias;
pub mod ganchos;
pub mod plans;
pub mod proceso;
pub mod restic;
pub mod retention;
pub mod sizes;
pub mod tls;

#[cfg(test)]
mod propiedades;
