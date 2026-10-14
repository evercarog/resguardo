//! Plan 0.8.0, D1 y D2: `GET /api/clientes/{c}/organizacion` (cualquier miembro):
//! las sedes del cliente, la de cada equipo y destino, las categorías de etiqueta y
//! el orden a mano de los equipos (`crate::sedes`). Se cambian como datos comunes
//! (`POST /api/clientes/{c}/datos-comunes`, administradores).

use super::cuentas::MIEMBRO;
use crate::auth::Usuario;
use crate::error::Res;
use crate::estado::St;
use axum::extract::{Path, State};
use axum::Json;
use serde_json::Value;

pub async fn ver(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, MIEMBRO).await?;
    Ok(Json(st.db(move |db| crate::sedes::organizacion(db, &ctx)).await?))
}
