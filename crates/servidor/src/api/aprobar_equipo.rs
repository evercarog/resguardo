//! 14a (v1.6x, pendiente de numerar al unir): la opción del cliente «Aprobar en el equipo»
//! (docs/consolas-multiples.md §5.11).
//!
//! Activada por defecto: quien está delante de un equipo, como administrador y con la clave
//! de administración, puede adelantar una orden en espera que mandó **esta** consola (en la
//! ventana del agente o con `resguardo-agente ordenes aprobar`). La propietaria del cliente
//! puede desactivarla: el equipo lo recibe de esta consola (`aprobar_en_equipo` en el `hola`
//! del canal, en `tomar` y, al cambiarla, `{"t": "ajustes"}`) y deja de ofrecerlo para las
//! órdenes de esta consola. Nada de esto deja aprobar desde el navegador: aprobar solo se hace
//! en el equipo.
//!
//! Se guarda en los valores del servidor (`aprobar_en_equipo:<cliente>`), como la marca: sin
//! cambios en el esquema. Sin valor: activada.

use crate::almacen::{ahora, Almacen, Rol, Ts, R};
use crate::auth::Usuario;
use crate::error::Res;
use crate::estado::St;
use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Serialize, Deserialize, Clone)]
pub struct Opcion {
    pub activa: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actualizada: Option<Ts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub por: Option<String>,
}

impl Default for Opcion {
    fn default() -> Self {
        Opcion { activa: true, actualizada: None, por: None }
    }
}

fn clave(cliente: &str) -> String {
    format!("aprobar_en_equipo:{cliente}")
}

pub fn leer(db: &dyn Almacen, cliente: &str) -> R<Opcion> {
    Ok(db.valor(&clave(cliente))?.filter(|v| !v.is_empty()).and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default())
}

/// ¿Activa para este cliente? (Si no se puede leer, la de por defecto.)
pub fn activa(db: &dyn Almacen, cliente: &str) -> bool {
    leer(db, cliente).map(|o| o.activa).unwrap_or(true)
}

/// Lo que ve la consola: `{ activa, actualizada, por }`.
pub fn json(o: &Opcion) -> Value {
    json!({ "activa": o.activa, "actualizada": super::fecha_opt(o.actualizada), "por": o.por })
}

#[derive(Deserialize)]
pub struct Cambiar {
    activa: bool,
}

/// `PUT /api/clientes/{c}/aprobar-en-equipo` (propietario). Auditado; se lo dice al momento a
/// los equipos conectados del cliente (los demás, al conectar).
pub async fn cambiar(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(p): Json<Cambiar>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Propietario).await?;
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let por = u.0.cuenta.nombre.clone();
    let c2 = c.clone();
    let ctx2 = ctx.clone();
    let (o, equipos) = st
        .db(move |db| {
            let o = Opcion { activa: p.activa, actualizada: Some(ahora()), por: Some(por) };
            db.poner_valor(&clave(&c2), &serde_json::to_string(&o).map_err(|e| e.to_string())?)?;
            db.auditar(&ctx2, &actor, "aprobar_en_equipo", &c2, &json!({ "activa": o.activa }).to_string())?;
            Ok((o, db.equipos(&ctx2)?.into_iter().map(|e| e.id).collect::<Vec<_>>()))
        })
        .await?;
    for e in equipos {
        st.al_agente(&e, &json!({ "t": "ajustes", "aprobar_en_equipo": o.activa }));
    }
    Ok(Json(json(&o)))
}
