//! Equipos gestionados (fase 5, docs/agente-gestionado.md). La criptografía
//! (mensajes firmados y sellados, códigos de emparejamiento y SAS) vive en el
//! crate `resguardo-protocolo`; aquí queda lo propio de la app.

pub use resguardo_protocolo::mensajes::*;
use serde::{Deserialize, Serialize};

/// Configuración que la consola asigna a un equipo gestionado.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct EndpointConfig {
    /// `rest:https://servidor:puerto/<usuario>/<repo>/`.
    pub location: String,
    pub rest_user: String,
    pub rest_password: String,
    /// Contraseña del repositorio (la genera la consola, que puede restaurar).
    pub repo_password: String,
    /// Certificado del servidor (PEM), fijado.
    pub tls_cert_pem: Option<String>,
    pub plans: Vec<crate::plans::Plan>,
    /// Bandeja: mostrarla y si avisa de «Copia en curso / terminada».
    pub tray: bool,
    pub tray_toasts: bool,
}
