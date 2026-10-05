//! Órdenes selladas para los equipos (docs/api-servidor.md, §5). El servidor
//! no puede leerlas: comprueba el rol, el `seq`, la caducidad, la espera de
//! las destructivas y el tamaño, y las entrega.

use super::{de_fecha, orden_json};
use crate::almacen::{ahora, OrdenNueva, Rol};
use crate::auth::Usuario;
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use axum::extract::{Path, Query, State};
use axum::Json;
use base64::Engine;
use resguardo_protocolo::ordenes;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
/// Margen para relojes desajustados al comprobar la espera.
const HOLGURA_S: i64 = 120;

async fn nombres(st: &St, ids: Vec<String>) -> HashMap<String, String> {
    st.db(move |db| {
        let mut m = HashMap::new();
        for id in ids {
            if let std::collections::hash_map::Entry::Vacant(v) = m.entry(id.clone()) {
                v.insert(db.cuenta(&id)?.map(|c| c.nombre).unwrap_or_else(|| "?".into()));
            }
        }
        Ok(m)
    })
    .await
    .unwrap_or_default()
}

#[derive(Deserialize)]
pub struct Relevo {
    id: String,
    max_bytes: u64,
}

#[derive(Deserialize)]
pub struct Nueva {
    tipo: String,
    seq: u64,
    sellado: String,
    caduca: String,
    not_before: Option<String>,
    sesion: Option<String>,
    relevo: Option<Relevo>,
}

fn es_uuid(s: &str) -> bool {
    uuid::Uuid::parse_str(s).is_ok()
}

pub async fn enviar(State(st): State<St>, u: Usuario, Path((c, e)): Path<(String, String)>, Json(p): Json<Nueva>) -> Res<Json<Value>> {
    let (ctx, rol) = u.miembro(&st, &c, Rol::Tecnico).await?;
    // v1.34: órdenes por minuto del cliente (de todas sus personas).
    let (c2, publico) = (c.clone(), st.opciones.publico);
    let efectivas = st.db(move |db| crate::cuotas::efectivas(db, publico, &c2)).await?;
    if let Some(max) = crate::cuotas::limite(efectivas.ordenes_min) {
        if !st.limites.intento(&format!("ordenes:{c}"), max as u32, std::time::Duration::from_secs(60)) {
            return Err(ErrorApi::nuevo(
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                "cuota",
                format!("Este cliente ya envió {max} órdenes en el último minuto, el máximo que permite este servidor. Espera un momento."),
            ));
        }
    }
    let tipo = ordenes::tipo(&p.tipo).ok_or_else(|| ErrorApi::datos("Tipo de orden desconocido."))?;
    if tipo.solo_administradores && rol < Rol::Administrador {
        return Err(ErrorApi::prohibido());
    }
    if p.sellado.len() > ordenes::MAX_SOBRE_BYTES || B64.decode(&p.sellado).is_err() {
        return Err(ErrorApi::datos("Sobre no válido o demasiado grande."));
    }
    let ahora_s = ahora();
    let caduca = de_fecha(&p.caduca).ok_or_else(|| ErrorApi::datos("Fecha de caducidad no válida."))?;
    if caduca <= ahora_s || caduca > ahora_s + ordenes::MAX_CADUCIDAD_DIAS * 86_400 + HOLGURA_S {
        return Err(ErrorApi::datos("La caducidad debe estar en el futuro y a 7 días como mucho."));
    }
    let not_before = match p.not_before.as_deref() {
        Some(f) => Some(de_fecha(f).ok_or_else(|| ErrorApi::datos("Fecha not_before no válida."))?),
        None => None,
    };
    if not_before.is_some_and(|nb| nb >= caduca) {
        return Err(ErrorApi::datos("La orden caducaría antes de poder aplicarse."));
    }
    let c2 = c.clone();
    let cliente = st.db(move |db| db.cliente(&c2)).await?.ok_or_else(ErrorApi::no_existe)?;
    let e2 = e.clone();
    let ctx2 = ctx.clone();
    let equipo = st.db(move |db| db.equipo(&ctx2, &e2)).await?.ok_or_else(ErrorApi::no_existe)?;
    if !equipo.confirmado {
        return Err(ErrorApi::datos("El equipo aún no está confirmado."));
    }
    // Destructiva según el tipo. Las que solo lo son con cierto cuerpo (que el
    // servidor no ve) declaran su not_before: si lo llevan, se exige aquí la
    // espera desde ahora. El agente solo puede comprobarla contra `emitida`,
    // que pone quien sella la orden; sin esta comprobación, una orden con
    // `emitida` atrasada se aplicaría al momento. Sin not_before, el agente la
    // rechaza si su cuerpo es destructivo.
    let segun_cuerpo = ordenes::DESTRUCTIVAS_SEGUN_CUERPO.contains(&tipo.nombre) && not_before.is_some();
    if tipo.destructiva || segun_cuerpo {
        // La espera que confirmó el equipo (firmada) o, si aún no hay, la del cliente.
        let espera = equipo.espera_min_horas.unwrap_or(cliente.espera_min_horas);
        let minimo = ahora_s + ordenes::segundos_de_espera(espera) - HOLGURA_S;
        if not_before.is_none_or(|nb| nb < minimo) {
            return Err(ErrorApi::datos(format!("Esta orden reduce la protección: tiene que esperar al menos {espera} h antes de aplicarse (not_before)."))
                .con(json!({ "espera_min_horas": espera })));
        }
    }
    let sesion = if tipo.abre_sesion {
        let s = p.sesion.filter(|s| es_uuid(s)).ok_or_else(|| ErrorApi::datos("Esta orden abre una sesión: falta su id (UUID)."))?;
        Some(s)
    } else {
        None
    };
    let relevo = if tipo.nombre == "descargar" {
        let r = p.relevo.filter(|r| es_uuid(&r.id)).ok_or_else(|| ErrorApi::datos("Falta el relé de la descarga."))?;
        if r.max_bytes == 0 || r.max_bytes > st.opciones.max_relevo {
            return Err(ErrorApi::datos(format!("La descarga puede ocupar como mucho {} MB.", st.opciones.max_relevo / (1024 * 1024))));
        }
        // v1.34: si ya no queda nada del relé de este mes, ni se empieza.
        let (c2, mes) = (c.clone(), crate::cuotas::mes(ahora_s));
        if !st.db(move |db| crate::cuotas::cabe_en_relevo(db, publico, &c2, &mes, 1)).await? {
            return Err(crate::agentes::error_cuota(
                "Este cliente ya usó este mes todo el relé de descargas que le permite el servidor. Restaura en el propio equipo o pide más a quien administra el servidor.".into(),
            ));
        }
        Some(r)
    } else {
        None
    };

    let nueva = OrdenNueva {
        id: uuid::Uuid::new_v4().to_string(),
        equipo_id: e.clone(),
        tipo: tipo.nombre.into(),
        seq: p.seq,
        sellado: p.sellado.clone(),
        emitida_por: u.id().to_string(),
        not_before,
        caduca,
        sesion: sesion.clone(),
        relevo: relevo.as_ref().map(|r| r.id.clone()),
    };
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let sobre_hash: String = <sha2::Sha256 as sha2::Digest>::digest(p.sellado.as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
    let (ctx2, destructiva, nombre_equipo) = (ctx.clone(), tipo.destructiva, equipo.nombre.clone());
    let r = st
        .db_crudo(move |db| {
            let orden = db.insertar_orden(&ctx2, &nueva)?;
            if let Some(s) = &nueva.sesion {
                db.crear_sesion_interactiva(&ctx2, s, &nueva.equipo_id, &nueva.emitida_por, ahora() + 600)?;
            }
            if let Some(r) = &relevo {
                db.crear_relevo(&ctx2, &r.id, &nueva.equipo_id, r.max_bytes, ahora() + 2 * 3600)?;
            }
            db.auditar(
                &ctx2,
                &actor,
                "orden",
                &nueva.equipo_id,
                &json!({ "tipo": nueva.tipo, "seq": nueva.seq, "sobre": sobre_hash, "not_before": nueva.not_before }).to_string(),
            )?;
            if destructiva {
                crate::notificaciones::aviso(
                    db,
                    &ctx2,
                    Some(&nueva.equipo_id),
                    "orden_destructiva",
                    &format!("Pendiente en «{nombre_equipo}»: {} (puedes cancelarla antes de que se aplique).", nueva.tipo),
                )?;
            }
            Ok(orden)
        })
        .await?;
    let orden = r.map_err(|e| match e.strip_prefix("seq:").and_then(|n| n.parse::<u64>().ok()) {
        Some(siguiente) => ErrorApi::conflicto("Otra orden se envió a la vez: vuelve a intentarlo.").con(json!({ "siguiente_seq": siguiente })),
        None => ErrorApi::datos(e),
    })?;
    st.vivo.avisar(ctx.id(), crate::vivo::Cambio::Orden { equipo: &e, orden: &orden.id, estado: &orden.estado });
    if destructiva {
        st.vivo.avisar(ctx.id(), crate::vivo::Cambio::Avisos(Some(&e)));
    }
    crate::agentes::empujar(&st, &ctx, &e).await;
    Ok(Json(orden_json(&orden, u.nombre())))
}

#[derive(Deserialize)]
pub struct Limite {
    limite: Option<i64>,
}

pub async fn listar(State(st): State<St>, u: Usuario, Path((c, e)): Path<(String, String)>, Query(q): Query<Limite>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Lectura).await?;
    let limite = q.limite.unwrap_or(50).clamp(1, 500);
    let ordenes = st.db(move |db| db.ordenes_equipo(&ctx, &e, limite)).await?;
    let n = nombres(&st, ordenes.iter().map(|o| o.emitida_por.clone()).collect()).await;
    Ok(Json(json!(ordenes.iter().map(|o| orden_json(o, n.get(&o.emitida_por).map(String::as_str).unwrap_or("?"))).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct FiltroCliente {
    pendientes: Option<String>,
    limite: Option<i64>,
    antes: Option<String>,
    equipo: Option<String>,
    estado: Option<String>,
}

const ESTADOS: &[&str] = &["pendiente", "entregada", "en_marcha", "hecha", "fallida", "rechazada", "cancelada", "caducada"];

/// `GET /clientes/{c}/ordenes`:
/// - `?pendientes=1`: las que esperan su `not_before` (una lista, como antes);
/// - si no: las últimas de todo el cliente, de la más reciente a la más
///   antigua, con `limite` (50; hasta 200), `equipo`, `estado` y el cursor
///   `antes` (el `siguiente` de la página anterior). Devuelve `{ ordenes, siguiente }`.
pub async fn listar_cliente(State(st): State<St>, u: Usuario, Path(c): Path<String>, Query(q): Query<FiltroCliente>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Lectura).await?;
    if q.pendientes.as_deref().is_some_and(|p| p == "1" || p == "true") {
        let ordenes = st.db(move |db| db.ordenes_con_espera(&ctx, ahora())).await?;
        let n = nombres(&st, ordenes.iter().map(|o| o.emitida_por.clone()).collect()).await;
        return Ok(Json(json!(ordenes.iter().map(|o| orden_json(o, n.get(&o.emitida_por).map(String::as_str).unwrap_or("?"))).collect::<Vec<_>>())));
    }
    if q.estado.as_deref().is_some_and(|e| !ESTADOS.contains(&e)) {
        return Err(ErrorApi::datos("Estado no válido."));
    }
    let antes = match q.antes.as_deref() {
        None | Some("") => None,
        Some(cur) => Some(cursor_de(cur).ok_or_else(|| ErrorApi::datos("Cursor no válido."))?),
    };
    let limite = q.limite.unwrap_or(50).clamp(1, 200);
    let ordenes = st.db(move |db| db.ordenes_cliente(&ctx, q.equipo.as_deref(), q.estado.as_deref(), antes, limite)).await?;
    let siguiente = (ordenes.len() as i64 == limite).then(|| ordenes.last().map(|o| format!("{}_{}", o.emitida, o.id))).flatten();
    let n = nombres(&st, ordenes.iter().map(|o| o.emitida_por.clone()).collect()).await;
    Ok(Json(json!({
        "ordenes": ordenes.iter().map(|o| orden_json(o, n.get(&o.emitida_por).map(String::as_str).unwrap_or("?"))).collect::<Vec<_>>(),
        "siguiente": siguiente,
    })))
}

/// Cursor «<emitida>_<id>».
fn cursor_de(s: &str) -> Option<(i64, String)> {
    let (ts, id) = s.split_once('_')?;
    es_uuid(id).then_some((ts.parse().ok()?, id.to_string()))
}

pub async fn cancelar(State(st): State<St>, u: Usuario, Path((c, o)): Path<(String, String)>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Tecnico).await?;
    let (ctx2, o2, por, actor) = (ctx.clone(), o.clone(), u.id().to_string(), format!("cuenta:{}", u.0.cuenta.correo));
    let (ok, orden) = st
        .db(move |db| {
            let ok = db.cancelar_orden(&ctx2, &o2, &por, ahora())?;
            let orden = db.orden(&ctx2, &o2)?;
            if ok {
                db.auditar(&ctx2, &actor, "cancelar_orden", &o2, &json!({ "tipo": orden.as_ref().map(|o| o.tipo.clone()) }).to_string())?;
            }
            Ok((ok, orden))
        })
        .await?;
    let orden = orden.ok_or_else(ErrorApi::no_existe)?;
    if !ok {
        return Err(ErrorApi::conflicto("Esa orden ya no se puede cancelar (se entregó o ya terminó)."));
    }
    let equipo = orden.equipo_id.clone();
    st.vivo.avisar(ctx.id(), crate::vivo::Cambio::Orden { equipo: &equipo, orden: &orden.id, estado: &orden.estado });
    // v1.30: si era destructiva, su «Orden destructiva pendiente» se cierra ya.
    if ordenes::tipo(&orden.tipo).is_some_and(|t| t.destructiva) {
        st.notif.despertar.notify_one();
    }
    crate::agentes::empujar(&st, &ctx, &equipo).await;
    let n = nombres(&st, vec![orden.emitida_por.clone()]).await;
    Ok(Json(orden_json(&orden, n.get(&orden.emitida_por).map(String::as_str).unwrap_or("?"))))
}
