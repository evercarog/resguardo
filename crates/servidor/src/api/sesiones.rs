//! Sesiones interactivas (lado de la consola) y relé de descargas.

use crate::almacen::{ahora, Rol};
use crate::auth::Usuario;
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

/// Tamaño máximo de un mensaje de sesión (base64).
pub const MAX_MENSAJE: usize = 350 * 1024;
/// Espera larga máxima.
pub const ESPERA_LARGA: Duration = Duration::from_secs(25);

#[derive(Deserialize)]
pub struct Escribir {
    cifrado: String,
}

pub async fn escribir(State(st): State<St>, u: Usuario, Path((c, s)): Path<(String, String)>, Json(p): Json<Escribir>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Tecnico).await?;
    if p.cifrado.is_empty() || p.cifrado.len() > MAX_MENSAJE {
        return Err(ErrorApi::datos("Mensaje vacío o demasiado grande."));
    }
    let (ctx2, s2, cifrado) = (ctx.clone(), s.clone(), p.cifrado.clone());
    let (sesion, n) = st
        .db(move |db| {
            let Some(sesion) = db.sesion_interactiva(&ctx2, &s2)? else { return Ok((None, 0)) };
            let n = db.mensaje_sesion(&ctx2, &s2, "consola", &cifrado, ahora() + 600)?;
            Ok((Some(sesion), n))
        })
        .await?;
    let sesion = sesion.ok_or_else(ErrorApi::no_existe)?;
    st.al_agente(&sesion.equipo_id, &json!({ "t": "sesion", "sesion": s, "n": n, "cifrado": p.cifrado }));
    st.cambios.notify_waiters();
    Ok(Json(json!({ "n": n })))
}

#[derive(Deserialize)]
pub struct Desde {
    desde: Option<i64>,
}

/// Espera larga hasta que haya mensajes del otro lado (o 25 s).
pub async fn esperar_mensajes(
    st: &St,
    ctx: crate::almacen::ClienteCtx,
    sesion: String,
    para: &'static str,
    desde: i64,
) -> Res<Option<Vec<crate::almacen::MensajeSesion>>> {
    let fin = tokio::time::Instant::now() + ESPERA_LARGA;
    loop {
        // Se apunta antes de mirar, para no perder un aviso entre medias.
        let aviso = st.cambios.notified();
        let (ctx2, s2) = (ctx.clone(), sesion.clone());
        let (existe, msgs) = st
            .db(move |db| {
                if db.sesion_interactiva(&ctx2, &s2)?.is_none() {
                    return Ok((false, vec![]));
                }
                Ok((true, db.mensajes_sesion(&ctx2, &s2, para, desde)?))
            })
            .await?;
        if !existe {
            return Ok(None);
        }
        if !msgs.is_empty() || tokio::time::Instant::now() >= fin {
            return Ok(Some(msgs));
        }
        let _ = tokio::time::timeout_at(fin, aviso).await;
    }
}

pub async fn leer(State(st): State<St>, u: Usuario, Path((c, s)): Path<(String, String)>, Query(q): Query<Desde>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Tecnico).await?;
    let msgs = esperar_mensajes(&st, ctx, s, "consola", q.desde.unwrap_or(0)).await?.ok_or_else(ErrorApi::no_existe)?;
    Ok(Json(json!(msgs)))
}

pub async fn cerrar(State(st): State<St>, u: Usuario, Path((c, s)): Path<(String, String)>) -> Res<StatusCode> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Tecnico).await?;
    let (ctx2, s2) = (ctx.clone(), s.clone());
    let sesion = st.db(move |db| db.sesion_interactiva(&ctx2, &s2)).await?;
    st.db(move |db| db.cerrar_sesion_interactiva(&ctx, &s)).await?;
    if let Some(ses) = sesion {
        st.al_agente(&ses.equipo_id, &json!({ "t": "sesion_cerrada", "sesion": ses.id }));
    }
    st.cambios.notify_waiters();
    Ok(StatusCode::NO_CONTENT)
}

// ---------- Relé ----------

pub fn dir_relevo(st: &St, cliente: &str, relevo: &str) -> Option<std::path::PathBuf> {
    let ok = |x: &str| uuid::Uuid::parse_str(x).is_ok();
    (ok(cliente) && ok(relevo)).then(|| st.datos.join("relevos").join(cliente).join(relevo))
}

pub async fn relevo(State(st): State<St>, u: Usuario, Path((c, r)): Path<(String, String)>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Tecnico).await?;
    let rel = st.db(move |db| db.relevo(&ctx, &r)).await?.ok_or_else(ErrorApi::no_existe)?;
    let estado = if rel.caduca <= ahora() { "caducado" } else { rel.estado.as_str() };
    Ok(Json(json!({ "estado": estado, "trozos": rel.trozos, "bytes": rel.bytes, "caduca": super::fecha(rel.caduca) })))
}

pub async fn trozo(State(st): State<St>, u: Usuario, Path((c, r, n)): Path<(String, String, u64)>) -> Res<Response> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Tecnico).await?;
    let r2 = r.clone();
    let rel = st.db(move |db| db.relevo(&ctx, &r2)).await?.ok_or_else(ErrorApi::no_existe)?;
    if n >= rel.trozos || rel.caduca <= ahora() {
        return Err(ErrorApi::no_existe());
    }
    let path = dir_relevo(&st, &c, &r).ok_or_else(ErrorApi::no_existe)?.join(n.to_string());
    let datos = tokio::fs::read(&path).await.map_err(|_| ErrorApi::no_existe())?;
    Ok(([(header::CONTENT_TYPE, "application/octet-stream")], Body::from(datos)).into_response())
}

pub async fn borrar_relevo(State(st): State<St>, u: Usuario, Path((c, r)): Path<(String, String)>) -> Res<StatusCode> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Tecnico).await?;
    let r2 = r.clone();
    st.db(move |db| db.borrar_relevo(&ctx, &r2)).await?;
    if let Some(dir) = dir_relevo(&st, &c, &r) {
        // Lo que ocupaba, fuera de la cuenta de los relés (medido y borrado fuera de la tarea async).
        let st2 = st.clone();
        let _ = tokio::task::spawn_blocking(move || {
            let bytes = crate::estado::medir_relevos(&dir);
            if std::fs::remove_dir_all(&dir).is_ok() {
                st2.uso_relevos.soltar(bytes);
            }
        })
        .await;
    }
    Ok(StatusCode::NO_CONTENT)
}
