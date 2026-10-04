//! Plantillas de copia de un cliente (v1.20). La consola las cifra con una
//! clave derivada de `K_cfg` (crates/protocolo/src/simetrico.rs,
//! `clave_plantillas`), atadas al cliente y a su id: aquí solo se guardan
//! bytes opacos. Ni el nombre ni las carpetas se ven en el servidor. Usarlas
//! en un equipo es una orden `config` normal, firmada con la clave.

use super::fecha;
use crate::almacen::Rol;
use crate::auth::Usuario;
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};

/// Como mucho, plantillas por cliente y bytes de cada una (cifrada).
pub const MAX_PLANTILLAS: usize = 100;
pub const MAX_BYTES: usize = 64 * 1024;

fn id_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// `GET /api/clientes/{c}/plantillas` (administrador): `[{ id, cifrado, actualizada, por }]`.
pub async fn listar(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let l = st.db(move |db| db.plantillas(&ctx)).await?;
    Ok(Json(json!(l.iter().map(|p| json!({ "id": p.id, "cifrado": p.cifrado, "actualizada": fecha(p.actualizada), "por": p.por })).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct Guardar {
    cifrado: String,
}

/// `PUT /api/clientes/{c}/plantillas/{id}` (administrador): crea o sustituye.
pub async fn guardar(State(st): State<St>, u: Usuario, Path((c, id)): Path<(String, String)>, Json(p): Json<Guardar>) -> Res<StatusCode> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    if !id_valido(&id) {
        return Err(ErrorApi::datos("Id de plantilla no válido."));
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(&p.cifrado).map_err(|_| ErrorApi::datos("Plantilla no válida (base64)."))?;
    // nonce (24) + etiqueta (16) como mínimo.
    if bytes.len() < 40 || bytes.len() > MAX_BYTES {
        return Err(ErrorApi::datos("Plantilla demasiado grande o vacía (hasta 64 KiB)."));
    }
    let por = u.0.cuenta.nombre.clone();
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let id2 = id.clone();
    let cabe = st
        .db(move |db| {
            let ok = db.guardar_plantilla(&ctx, &id2, &p.cifrado, &por, MAX_PLANTILLAS)?;
            if ok {
                db.auditar(&ctx, &actor, "guardar_plantilla", &id2, "{}")?;
            }
            Ok(ok)
        })
        .await?;
    if !cabe {
        return Err(ErrorApi::datos(format!("Como mucho {MAX_PLANTILLAS} plantillas por cliente: borra alguna.")));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /api/clientes/{c}/plantillas/{id}` (administrador). No toca ningún equipo.
pub async fn borrar(State(st): State<St>, u: Usuario, Path((c, id)): Path<(String, String)>) -> Res<StatusCode> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let existia = st
        .db(move |db| {
            let ok = db.borrar_plantilla(&ctx, &id)?;
            if ok {
                db.auditar(&ctx, &actor, "borrar_plantilla", &id, "{}")?;
            }
            Ok(ok)
        })
        .await?;
    if !existia {
        return Err(ErrorApi::no_existe());
    }
    Ok(StatusCode::NO_CONTENT)
}
