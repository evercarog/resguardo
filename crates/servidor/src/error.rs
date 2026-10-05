//! Errores de la API: código HTTP y cuerpo `{ "error", "mensaje" }`
//! (docs/api-servidor.md, §0).

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug)]
pub struct ErrorApi {
    pub estado: StatusCode,
    pub codigo: &'static str,
    pub mensaje: String,
    /// Datos extra para el cliente (p. ej. el `siguiente_seq` en un conflicto).
    pub extra: Option<serde_json::Value>,
    /// Un intento de acceso fallido (contraseña, código, secreto de un equipo…):
    /// va al registro con la IP, para fail2ban (`crate::registro`).
    pub acceso_fallido: Option<&'static str>,
}

/// En la respuesta: qué acceso falló (lo anota el registro con la IP).
#[derive(Clone, Copy, Debug)]
pub struct AccesoFallido(pub &'static str);

pub type Res<T> = Result<T, ErrorApi>;

impl ErrorApi {
    pub fn nuevo(estado: StatusCode, codigo: &'static str, mensaje: impl Into<String>) -> Self {
        Self { estado, codigo, mensaje: mensaje.into(), extra: None, acceso_fallido: None }
    }
    pub fn sin_sesion() -> Self {
        Self::nuevo(StatusCode::UNAUTHORIZED, "sin_sesion", "Inicia sesión para continuar.")
    }
    pub fn necesita_totp() -> Self {
        Self::nuevo(StatusCode::UNAUTHORIZED, "necesita_totp", "Falta el código de tu aplicación de autenticación.")
    }
    pub fn prohibido() -> Self {
        Self::nuevo(StatusCode::FORBIDDEN, "prohibido", "Tu rol no permite hacer esto.")
    }
    pub fn no_existe() -> Self {
        Self::nuevo(StatusCode::NOT_FOUND, "no_existe", "No existe.")
    }
    pub fn datos(mensaje: impl Into<String>) -> Self {
        Self::nuevo(StatusCode::UNPROCESSABLE_ENTITY, "datos", mensaje)
    }
    pub fn conflicto(mensaje: impl Into<String>) -> Self {
        Self::nuevo(StatusCode::CONFLICT, "conflicto", mensaje)
    }
    pub fn demasiados() -> Self {
        Self::nuevo(StatusCode::TOO_MANY_REQUESTS, "demasiados_intentos", "Demasiados intentos. Espera unos minutos.")
    }
    /// 429 con el mensaje que se da y cuánto esperar: `retry_after` (segundos)
    /// en el cuerpo y la cabecera `Retry-After`.
    pub fn demasiados_esperar(mensaje: impl Into<String>, espera: std::time::Duration) -> Self {
        let s = espera.as_secs() + u64::from(espera.subsec_nanos() > 0);
        Self::nuevo(StatusCode::TOO_MANY_REQUESTS, "demasiados_intentos", mensaje).con(json!({ "retry_after": s.max(1) }))
    }
    pub fn interno(detalle: impl std::fmt::Display) -> Self {
        // El detalle va al registro del servidor, no al cliente.
        eprintln!("ERROR interno: {detalle}");
        Self::nuevo(StatusCode::INTERNAL_SERVER_ERROR, "interno", "Error del servidor. Inténtalo de nuevo.")
    }
    /// Marca el error como un acceso fallido (`que`: «entrar», «totp», «equipo»…).
    pub fn acceso(mut self, que: &'static str) -> Self {
        self.acceso_fallido = Some(que);
        self
    }
    pub fn con(mut self, extra: serde_json::Value) -> Self {
        self.extra = Some(extra);
        self
    }
}

impl IntoResponse for ErrorApi {
    fn into_response(self) -> Response {
        let mut cuerpo = json!({ "error": self.codigo, "mensaje": self.mensaje });
        let reintentar = self.extra.as_ref().and_then(|e| e.get("retry_after")).and_then(|v| v.as_u64());
        if let (Some(extra), Some(obj)) = (self.extra, cuerpo.as_object_mut()) {
            if let Some(e) = extra.as_object() {
                for (k, v) in e {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }
        let mut r = (self.estado, Json(cuerpo)).into_response();
        if let Some(s) = reintentar {
            r.headers_mut().insert(axum::http::header::RETRY_AFTER, axum::http::HeaderValue::from(s));
        }
        if let Some(que) = self.acceso_fallido {
            r.extensions_mut().insert(AccesoFallido(que));
        }
        r
    }
}

impl From<rusqlite::Error> for ErrorApi {
    fn from(e: rusqlite::Error) -> Self {
        ErrorApi::interno(e)
    }
}
