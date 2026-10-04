//! «Clientes del servidor» (v1.34): lo que ve y hace quien administra un
//! servidor compartido (el propietario del servidor), **sin entrar en los
//! clientes**: cuántas personas y equipos tiene cada uno, su última actividad
//! y lo que usa de sus cuotas. Ni nombres de equipos, ni correos, ni avisos,
//! ni órdenes: para eso hay que ser miembro del cliente, como cualquiera.
//!
//! Desde aquí también se da de alta un cliente **para otra persona**: el
//! cliente se crea sin que el propietario del servidor sea miembro, con una
//! invitación de propietario (de un solo uso, 7 días) para quien lo va a llevar.

use super::fecha_opt;
use crate::almacen::{ahora, ClienteCtx, Rol};
use crate::auth::{self, Usuario};
use crate::cuotas::{self, Cuotas};
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use axum::extract::{Path, State};
use axum::Json;
use base64::Engine;
use rand::RngCore;
use serde::Deserialize;
use serde_json::{json, Value};

/// Solo el propietario del servidor.
fn solo_propietario_servidor(u: &Usuario) -> Res<()> {
    if u.0.cuenta.superusuario {
        Ok(())
    } else {
        Err(ErrorApi::prohibido())
    }
}

/// `GET /api/servidor/clientes`: todos los clientes, solo con cifras.
pub async fn listar(State(st): State<St>, u: Usuario) -> Res<Json<Value>> {
    solo_propietario_servidor(&u)?;
    let (yo, publico) = (u.id().to_string(), st.opciones.publico);
    let conectados: std::collections::HashSet<String> = st.conectados.lock().unwrap_or_else(|e| e.into_inner()).keys().cloned().collect();
    let mes = cuotas::mes(ahora());
    let m2 = mes.clone();
    let (predeterminadas, filas) = st
        .db(move |db| {
            let mios: std::collections::HashSet<String> = db.clientes_de(&yo)?.into_iter().map(|(c, _)| c.id).collect();
            let mut filas = Vec::new();
            for c in db.clientes_del_servidor()? {
                let ctx = ClienteCtx::autorizado(&c.id);
                let uso = db.uso_cliente(&ctx)?;
                // Los equipos conectados ahora: solo cuántos (de los ids, ni el nombre).
                let ids: Vec<String> = db.equipos(&ctx)?.into_iter().map(|e| e.id).collect();
                let en_linea = ids.iter().filter(|id| conectados.contains(*id)).count();
                let propias = cuotas::del_cliente(db, &c.id)?;
                let efectivas = cuotas::efectivas(db, publico, &c.id)?;
                let relevo = cuotas::relevo_usado(db, &c.id, &m2)?;
                filas.push(json!({
                    "id": c.id, "nombre": c.nombre, "creado": super::fecha(c.creado),
                    "personas": c.personas, "propietarios": c.propietarios, "soy_miembro": mios.contains(&c.id),
                    "equipos": uso.equipos, "equipos_confirmados": uso.equipos_confirmados, "equipos_conectados": en_linea,
                    "historial": uso.historial, "ordenes_30d": uso.ordenes_30d,
                    "ultima_actividad": fecha_opt(uso.ultima_actividad), "bytes": uso.bytes,
                    "relevo_mes_bytes": relevo,
                    "cuotas": propias, "efectivas": efectivas,
                }));
            }
            Ok((cuotas::predeterminadas(db, publico)?, filas))
        })
        .await?;
    Ok(Json(json!({
        "publico": publico,
        "mes": mes,
        "predeterminadas": predeterminadas,
        "de_fabrica": Cuotas::de_fabrica(publico),
        "clientes": filas,
    })))
}

/// `PUT /api/servidor/cuotas`: las predeterminadas.
pub async fn poner_predeterminadas(State(st): State<St>, u: Usuario, Json(c): Json<Cuotas>) -> Res<Json<Value>> {
    solo_propietario_servidor(&u)?;
    c.validar().map_err(ErrorApi::datos)?;
    let (actor, publico) = (format!("cuenta:{}", u.0.cuenta.correo), st.opciones.publico);
    let r = st
        .db(move |db| {
            cuotas::poner_predeterminadas(db, &c)?;
            db.auditar_servidor(&actor, "cuotas_predeterminadas", "servidor", &serde_json::to_string(&c).unwrap_or_default())?;
            cuotas::predeterminadas(db, publico)
        })
        .await?;
    Ok(Json(json!({ "predeterminadas": r })))
}

/// `PUT /api/servidor/clientes/{c}/cuotas`: las de un cliente (`null` = la predeterminada).
pub async fn poner_cuotas(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(cu): Json<Cuotas>) -> Res<Json<Value>> {
    solo_propietario_servidor(&u)?;
    cu.validar().map_err(ErrorApi::datos)?;
    let (actor, publico) = (format!("cuenta:{}", u.0.cuenta.correo), st.opciones.publico);
    let r = st
        .db(move |db| {
            if db.cliente(&c)?.is_none() {
                return Ok(None);
            }
            cuotas::poner_del_cliente(db, &c, &cu)?;
            let datos = serde_json::to_string(&cu).unwrap_or_default();
            db.auditar_servidor(&actor, "cuotas_cliente", &c, &datos)?;
            // Que lo vean también las personas del cliente, en su auditoría.
            db.auditar(&ClienteCtx::autorizado(&c), &actor, "cuotas_cliente", &c, &datos)?;
            Ok(Some((cuotas::del_cliente(db, &c)?, cuotas::efectivas(db, publico, &c)?)))
        })
        .await?
        .ok_or_else(ErrorApi::no_existe)?;
    Ok(Json(json!({ "cuotas": r.0, "efectivas": r.1 })))
}

#[derive(Deserialize)]
pub struct NuevoAjeno {
    nombre: String,
    espera_min_horas: Option<i64>,
    #[serde(default)]
    cuotas: Option<Cuotas>,
}

fn invitacion_propietario(db: &dyn crate::almacen::Almacen, cliente: &str, por: &str) -> Result<Value, String> {
    let token = auth::ficha();
    let caduca = ahora() + 7 * 86_400;
    db.crear_invitacion(&auth::hash_ficha(&token), cliente, Rol::Propietario, caduca, por)?;
    Ok(json!({ "enlace": format!("/invitacion#{token}"), "caduca": super::fecha(caduca) }))
}

/// `POST /api/servidor/clientes`: un cliente para otra persona (sin ser miembro).
pub async fn crear_para_otro(State(st): State<St>, u: Usuario, Json(p): Json<NuevoAjeno>) -> Res<Json<Value>> {
    solo_propietario_servidor(&u)?;
    let nombre = p.nombre.trim().to_string();
    if nombre.is_empty() || nombre.chars().count() > 80 {
        return Err(ErrorApi::datos("Escribe un nombre (hasta 80 caracteres)."));
    }
    let espera = p.espera_min_horas.unwrap_or(24);
    if !(1..=168).contains(&espera) {
        return Err(ErrorApi::datos("La espera mínima debe estar entre 1 y 168 horas."));
    }
    if let Some(c) = &p.cuotas {
        c.validar().map_err(ErrorApi::datos)?;
    }
    let mut sal = [0u8; 16];
    rand::rng().fill_bytes(&mut sal);
    let sal = base64::engine::general_purpose::STANDARD.encode(sal);
    let (por, actor) = (u.id().to_string(), format!("cuenta:{}", u.0.cuenta.correo));
    let (cliente, invitacion) = st
        .db(move |db| {
            let c = db.crear_cliente(&nombre, &sal, espera)?;
            if let Some(cu) = &p.cuotas {
                cuotas::poner_del_cliente(db, &c.id, cu)?;
            }
            let inv = invitacion_propietario(db, &c.id, &por)?;
            db.auditar_servidor(&actor, "crear_cliente_para_otro", &c.id, &json!({ "nombre": c.nombre }).to_string())?;
            db.auditar(&ClienteCtx::autorizado(&c.id), &actor, "crear_cliente", &c.id, &json!({ "para_otro": true }).to_string())?;
            Ok((c, inv))
        })
        .await?;
    Ok(Json(json!({ "id": cliente.id, "nombre": cliente.nombre, "invitacion": invitacion })))
}

/// `POST /api/servidor/clientes/{c}/invitacion`: otra invitación de propietario, solo
/// mientras el cliente no tenga ninguno (p. ej. la primera caducó sin usarse). Con un
/// propietario dentro, quien invita es él: el del servidor no puede meterse así.
pub async fn invitar_propietario(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    solo_propietario_servidor(&u)?;
    let (por, actor) = (u.id().to_string(), format!("cuenta:{}", u.0.cuenta.correo));
    let r = st
        .db_crudo(move |db| {
            if db.cliente(&c)?.is_none() {
                return Err("no_existe".into());
            }
            if db.miembros(&c)?.iter().any(|m| m.rol == Rol::Propietario) {
                return Err("Este cliente ya tiene propietario: las invitaciones las hace él, desde su cliente.".into());
            }
            let inv = invitacion_propietario(db, &c, &por)?;
            db.auditar_servidor(&actor, "invitar_propietario", &c, "{}")?;
            db.auditar(&ClienteCtx::autorizado(&c), &actor, "invitar", "propietario", "{}")?;
            Ok(inv)
        })
        .await?;
    match r {
        Ok(inv) => Ok(Json(json!({ "invitacion": inv }))),
        Err(e) if e == "no_existe" => Err(ErrorApi::no_existe()),
        Err(e) => Err(ErrorApi::nuevo(axum::http::StatusCode::CONFLICT, "conflicto", e)),
    }
}
