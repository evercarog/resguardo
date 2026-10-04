//! Marca de un cliente (v1.32): su logo y un
//! acento de los siete de la consola (docs/diseno.md §2, ya comprobados de
//! contraste). Lo ven todos sus miembros (selector de clientes, cabecera,
//! portada de los informes); lo cambian propietarios y administradores, y
//! queda en la auditoría.
//!
//! El logo es **solo PNG** (≤ 200 KB, ≤ 2048 px de lado): la consola convierte
//! antes cualquier SVG, JPG o WebP a PNG en el navegador, así que aquí nunca
//! llega un SVG con scripts, manejadores o referencias externas. Se comprueba
//! que es un PNG de verdad (firma, cabecera IHDR y final IEND) y se sirve como
//! `image/png` con `nosniff`, aparte del JSON, desde el propio servidor
//! (`img-src 'self'` de la CSP).
//!
//! Se guarda en los valores del servidor (`marca:<cliente>`), como los ajustes
//! de notificaciones de cada cliente: sin cambios en el esquema.

use crate::almacen::{ahora, Almacen, Rol, Ts, R};
use crate::auth::Usuario;
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Digest;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// Tamaño máximo del logo (el PNG ya convertido).
pub const MAX_LOGO: usize = 200 * 1024;
/// Lado máximo del logo, en píxeles.
pub const MAX_LADO: u32 = 2048;
/// Los acentos de la consola (docs/diseno.md §2): ningún otro color.
pub const ACENTOS: [&str; 7] = ["teal", "blue", "indigo", "violet", "rose", "amber", "graphite"];

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Marca {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acento: Option<String>,
    /// El PNG en base64.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    /// SHA-256 (16 cifras hex) del PNG: versión del logo en su URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub huella: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actualizada: Option<Ts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub por: Option<String>,
}

fn clave(cliente: &str) -> String {
    format!("marca:{cliente}")
}

pub fn leer(db: &dyn Almacen, cliente: &str) -> R<Marca> {
    Ok(db.valor(&clave(cliente))?.filter(|v| !v.is_empty()).and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default())
}

fn guardar(db: &dyn Almacen, cliente: &str, m: &Marca) -> R<()> {
    db.poner_valor(&clave(cliente), &serde_json::to_string(m).map_err(|e| e.to_string())?)
}

/// Lo que ve la consola: `{ acento, logo, actualizada, por }` (`logo` es la URL, con su versión).
pub fn json(cliente: &str, m: &Marca) -> Value {
    json!({
        "acento": m.acento,
        "logo": m.huella.as_ref().filter(|_| m.logo.is_some()).map(|h| format!("/api/clientes/{cliente}/marca/logo?v={h}")),
        "actualizada": super::fecha_opt(m.actualizada),
        "por": m.por,
    })
}

/// ¿Es un PNG entero y razonable? Firma, IHDR con el tamaño y el final IEND.
pub fn png_valido(b: &[u8]) -> Result<(), &'static str> {
    const FIRMA: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    const IEND: [u8; 12] = [0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82];
    if b.len() < 8 + 25 + 12 || b[..8] != FIRMA {
        return Err("El logo tiene que ser una imagen PNG.");
    }
    if b[8..12] != [0, 0, 0, 13] || &b[12..16] != b"IHDR" {
        return Err("El logo tiene que ser una imagen PNG.");
    }
    let ancho = u32::from_be_bytes([b[16], b[17], b[18], b[19]]);
    let alto = u32::from_be_bytes([b[20], b[21], b[22], b[23]]);
    if ancho == 0 || alto == 0 || ancho > MAX_LADO || alto > MAX_LADO {
        return Err("El logo es demasiado grande: como mucho 2048 px de lado.");
    }
    if !b.ends_with(&IEND) {
        return Err("El logo está incompleto o dañado.");
    }
    Ok(())
}

/// `GET /api/clientes/{c}/marca` (cualquier miembro).
pub async fn ver(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    u.miembro(&st, &c, super::cuentas::MIEMBRO).await?;
    let c2 = c.clone();
    let m = st.db(move |db| leer(db, &c2)).await?;
    Ok(Json(json(&c, &m)))
}

#[derive(Deserialize)]
pub struct Cambiar {
    /// Uno de [`ACENTOS`], o `null` para el de siempre.
    acento: Option<String>,
    /// PNG en base64: lo sustituye. Sin el campo, se queda el que había.
    logo: Option<String>,
    /// `true`: quita el logo.
    #[serde(default)]
    quitar_logo: bool,
}

/// `PUT /api/clientes/{c}/marca` (propietario o administrador). Auditado.
pub async fn cambiar(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(p): Json<Cambiar>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    if let Some(a) = &p.acento {
        if !ACENTOS.contains(&a.as_str()) {
            return Err(ErrorApi::datos("Ese color no está entre los de la consola."));
        }
    }
    let png = match &p.logo {
        Some(b) => {
            let bytes = B64.decode(b).map_err(|_| ErrorApi::datos("Logo no válido (base64)."))?;
            if bytes.len() > MAX_LOGO {
                return Err(ErrorApi::datos("El logo pesa demasiado: como mucho 200 KB."));
            }
            png_valido(&bytes).map_err(ErrorApi::datos)?;
            Some((b.clone(), hex16(&sha2::Sha256::digest(&bytes))))
        }
        None => None,
    };
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let por = u.0.cuenta.nombre.clone();
    let c2 = c.clone();
    let m = st
        .db(move |db| {
            let mut m = leer(db, &c2)?;
            let logo = if p.quitar_logo {
                m.logo = None;
                m.huella = None;
                "quitado"
            } else if let Some((b, h)) = png {
                m.logo = Some(b);
                m.huella = Some(h);
                "puesto"
            } else {
                "igual"
            };
            m.acento = p.acento;
            m.actualizada = Some(ahora());
            m.por = Some(por);
            guardar(db, &c2, &m)?;
            db.auditar(&ctx, &actor, "cambiar_marca", &c2, &json!({ "acento": m.acento, "logo": logo }).to_string())?;
            Ok(m)
        })
        .await?;
    Ok(Json(json(&c, &m)))
}

/// `GET /api/clientes/{c}/marca/logo` (cualquier miembro): el PNG tal cual.
pub async fn logo(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Response> {
    u.miembro(&st, &c, super::cuentas::MIEMBRO).await?;
    let m = st.db(move |db| leer(db, &c)).await?;
    let bytes = m.logo.and_then(|b| B64.decode(b).ok()).ok_or_else(ErrorApi::no_existe)?;
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "image/png"),
            // Privado (pide sesión); la URL lleva la versión (`?v=`), así que puede guardarse un día.
            (header::CACHE_CONTROL, "private, max-age=86400"),
            (header::CONTENT_DISPOSITION, "inline; filename=\"logo.png\""),
        ],
        bytes,
    )
        .into_response())
}

fn hex16(d: &[u8]) -> String {
    d.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn png(ancho: u32, alto: u32) -> Vec<u8> {
        let mut b = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
        b.extend_from_slice(b"IHDR");
        b.extend_from_slice(&ancho.to_be_bytes());
        b.extend_from_slice(&alto.to_be_bytes());
        b.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        b.extend_from_slice(&[0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82]);
        b
    }

    #[test]
    fn solo_png_entero_y_de_tamano_razonable() {
        assert!(png_valido(&png(64, 32)).is_ok());
        assert!(png_valido(&png(0, 32)).is_err());
        assert!(png_valido(&png(4096, 32)).is_err());
        let mut cortado = png(64, 64);
        cortado.truncate(cortado.len() - 4);
        assert!(png_valido(&cortado).is_err());
        assert!(png_valido(b"<svg xmlns=\"http://www.w3.org/2000/svg\"><script>alert(1)</script></svg>").is_err());
        let mut jpg = png(64, 64);
        jpg[..3].copy_from_slice(&[0xFF, 0xD8, 0xFF]);
        assert!(png_valido(&jpg).is_err());
    }
}
