//! Catálogo de destinos de un cliente (tarea 7a, docs/copias-en-cadena.md).
//!
//! Un destino («Almacén · Disco E», «Backblaze B2 · copias-sur») se puede
//! crear sin crear un repositorio y renombrar cuando se quiera. Aquí solo se
//! guarda lo que lo describe, **en claro**: nombre, tipo y, en un destino de
//! red, el servidor o el bucket (lo mismo que ya dicen los resúmenes de los
//! equipos en `destinos[].donde`). **Nunca** credenciales ni rutas de carpetas
//! locales: las credenciales van selladas para cada equipo en la orden que las
//! usa, como siempre. Por eso este catálogo rechaza cualquier campo de más.
//! Cambiarlo no manda órdenes ni toca ningún equipo.

use super::fecha;
use crate::almacen::{DestinoCatalogo, Rol};
use crate::auth::Usuario;
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

/// Como mucho, destinos por cliente en el catálogo.
pub const MAX_DESTINOS: usize = 200;

/// Los tipos que admite el catálogo.
pub const TIPOS: [&str; 7] = ["zona", "rest", "s3", "b2", "sftp", "nube", "local"];

/// Id: letras minúsculas, cifras y `:_.-`, hasta 120 (`zona:<equipo>:<zona>`, `destino-1a2b3c4d`…).
pub fn id_valido(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 120
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, ':' | '_' | '.' | '-'))
        && !id.split(':').any(|p| p.is_empty() || p == "." || p == "..")
}

/// Nombre: 1 a 80 caracteres, sin caracteres de control.
fn nombre_valido(n: &str) -> bool {
    let n = n.trim();
    !n.is_empty() && n.chars().count() <= 80 && !n.chars().any(char::is_control)
}

/// ¿Parece una ruta de una carpeta local o de red? (`C:\…`, `\\nas\…`, `/srv/…`, `~/…`).
fn parece_ruta_local(d: &str) -> bool {
    let b = d.as_bytes();
    (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && !d.contains("://")) || d.starts_with('\\') || d.starts_with('/') || d.starts_with('~')
}

/// `donde` de un destino de red: servidor o bucket, sin credenciales ni rutas locales.
pub fn donde_valido(tipo: &str, donde: Option<&str>) -> Result<Option<String>, &'static str> {
    let d = donde.map(str::trim).filter(|d| !d.is_empty());
    match tipo {
        "rest" | "s3" | "b2" | "sftp" => {}
        _ if d.is_none() => return Ok(None),
        _ => return Err("Solo los destinos de red (rest-server, S3, B2, SFTP) llevan dirección."),
    }
    let Some(d) = d else { return Err("Falta la dirección del destino (servidor o bucket).") };
    if d.chars().count() > 300 || d.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("Dirección no válida (hasta 300 caracteres, sin espacios).");
    }
    // `https://usuario:clave@host` o `usuario@host:clave`: nada de credenciales.
    let sin_esquema = d.split_once("://").map_or(d, |(_, r)| r);
    let autoridad = sin_esquema.split('/').next().unwrap_or_default();
    if autoridad.contains('@') && (tipo != "sftp" || autoridad.split_once('@').is_some_and(|(u, _)| u.contains(':'))) {
        return Err("La dirección no puede llevar usuario ni contraseña: las credenciales van selladas para cada equipo.");
    }
    if parece_ruta_local(d) {
        return Err("Una carpeta local no va en el catálogo: se elige al crear el repositorio en su equipo.");
    }
    Ok(Some(d.to_string()))
}

/// `GET /api/clientes/{c}/destinos` (cualquier miembro): `[{ id, nombre, tipo, donde, actualizado, por }]`.
pub async fn listar(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Lectura).await?;
    let l = st.db(move |db| db.destinos_catalogo(&ctx)).await?;
    Ok(Json(json!(l
        .iter()
        .map(|d| json!({ "id": d.id, "nombre": d.nombre, "tipo": d.tipo, "donde": d.donde, "actualizado": fecha(d.actualizado), "por": d.por }))
        .collect::<Vec<_>>())))
}

/// El cuerpo de `PUT`: solo estos campos (cualquier otro, p. ej. un secreto, se rechaza).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Guardar {
    nombre: String,
    tipo: String,
    #[serde(default)]
    donde: Option<String>,
}

/// `PUT /api/clientes/{c}/destinos/{id}` (administrador): crea o sustituye.
pub async fn guardar(State(st): State<St>, u: Usuario, Path((c, id)): Path<(String, String)>, Json(g): Json<Value>) -> Res<StatusCode> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    if !id_valido(&id) {
        return Err(ErrorApi::datos("Id de destino no válido."));
    }
    let g: Guardar =
        serde_json::from_value(g).map_err(|_| ErrorApi::datos("Destino no válido: solo nombre, tipo y dirección (las credenciales nunca van al servidor)."))?;
    if !nombre_valido(&g.nombre) {
        return Err(ErrorApi::datos("Escribe un nombre para el destino (hasta 80 caracteres)."));
    }
    if !TIPOS.contains(&g.tipo.as_str()) {
        return Err(ErrorApi::datos("Tipo de destino no válido."));
    }
    let donde = donde_valido(&g.tipo, g.donde.as_deref()).map_err(ErrorApi::datos)?;
    let d = DestinoCatalogo {
        id: id.clone(),
        nombre: g.nombre.trim().to_string(),
        tipo: g.tipo,
        donde,
        actualizado: crate::almacen::ahora(),
        por: u.0.cuenta.nombre.clone(),
    };
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let datos = json!({ "nombre": d.nombre, "tipo": d.tipo }).to_string();
    let cabe = st
        .db(move |db| {
            let ok = db.guardar_destino(&ctx, &d, MAX_DESTINOS)?;
            if ok {
                db.auditar(&ctx, &actor, "guardar_destino", &id, &datos)?;
            }
            Ok(ok)
        })
        .await?;
    if !cabe {
        return Err(ErrorApi::datos(format!("Como mucho {MAX_DESTINOS} destinos en el catálogo: quita alguno.")));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /api/clientes/{c}/destinos/{id}` (administrador). Solo lo quita del
/// catálogo: no toca ningún equipo ni lo que hay en el destino.
pub async fn borrar(State(st): State<St>, u: Usuario, Path((c, id)): Path<(String, String)>) -> Res<StatusCode> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let existia = st
        .db(move |db| {
            let ok = db.borrar_destino(&ctx, &id)?;
            if ok {
                db.auditar(&ctx, &actor, "borrar_destino", &id, "{}")?;
            }
            Ok(ok)
        })
        .await?;
    if !existia {
        return Err(ErrorApi::no_existe());
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_y_direcciones() {
        for bueno in [
            "zona:0b5c1f8e-1d2a-4c3b-9e8f-7a6b5c4d3e2f:principal",
            "zona:0b5c1f8e:z1a2b3c",
            "destino-1a2b3c4d",
            "nube:0b5c1f8e:dropbox-oficina",
            "almacen-0b5c1f8e",
        ] {
            assert!(id_valido(bueno), "{bueno}");
        }
        for malo in ["", "Mayus", "a/b", "zona::x", "a b", "../x", "zona:..:x", &"x".repeat(121)] {
            assert!(!id_valido(malo), "{malo}");
        }
        assert_eq!(donde_valido("b2", Some(" copias-sur ")).unwrap().as_deref(), Some("copias-sur"));
        assert_eq!(donde_valido("rest", Some("https://almacen.ejemplo.com:8000")).unwrap().as_deref(), Some("https://almacen.ejemplo.com:8000"));
        assert_eq!(
            donde_valido("sftp", Some("copias@nas.ejemplo.com:/copias")).unwrap().as_deref(),
            Some("copias@nas.ejemplo.com:/copias"),
            "el usuario de SFTP no es secreto"
        );
        assert!(donde_valido("rest", Some("https://ana:secreta@almacen.ejemplo.com:8000")).is_err(), "sin credenciales");
        assert!(donde_valido("sftp", Some("ana:secreta@nas.ejemplo.com:/x")).is_err());
        assert!(donde_valido("s3", None).is_err(), "falta");
        for local in [r"D:\Copias", r"\\nas\copias", "/srv/copias", "~/copias"] {
            assert!(donde_valido("rest", Some(local)).is_err(), "{local}");
        }
        assert_eq!(donde_valido("zona", None).unwrap(), None);
        assert!(donde_valido("local", Some(r"D:\Copias")).is_err(), "una carpeta local nunca va en el catálogo");
        assert!(donde_valido("b2", Some("dos palabras")).is_err());
        assert!(nombre_valido("Almacén · Disco E") && !nombre_valido(" ") && !nombre_valido("a\u{7}") && !nombre_valido(&"x".repeat(81)));
    }
}
