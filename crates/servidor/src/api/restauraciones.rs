//! Registro de restauraciones (plan 0.8.0, E2: 13d, 14l). Cada restauración la
//! anota el equipo en su bitácora (entrada `restauracion`: quién, cuándo, qué
//! versión, desde dónde, cuánto y cuánto tardó, sin rutas), que llega a todas
//! sus consolas. Aquí:
//!
//! - `GET /api/clientes/{c}/restauraciones[?equipo=]` (lectura): las entradas de
//!   todos los equipos del cliente (o de uno), el motivo que les añadieron las
//!   personas de esta consola y el ajuste del cliente.
//! - `PUT /api/clientes/{c}/equipos/{e}/restauraciones/{id}` (técnico o más):
//!   `{ motivo, notas? }`, el motivo (`borrado`, `dano`, `prueba`, `otro`) y las
//!   notas de una restauración (también antes de que llegue su entrada: la
//!   consola lo guarda al mandar la orden, con el id `restauracion-<orden>`).
//! - `PUT /api/clientes/{c}/restauraciones/ajustes` (administrador o más):
//!   `{ motivo_obligatorio }`. Por defecto, opcional.
//!
//! En claro, como las observaciones (no son secretos); cada cambio se audita sin el texto.

use super::fecha;
use crate::almacen::{FiltroHistorial, NotaRestauracion, Rol};
use crate::auth::Usuario;
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

/// Motivos de una restauración.
pub const MOTIVOS: [&str; 4] = ["borrado", "dano", "prueba", "otro"];
/// Caracteres de las notas.
pub const MAX_NOTAS: usize = 500;
/// Restauraciones con motivo por cliente.
pub const MAX_NOTAS_CLIENTE: i64 = 20_000;
/// Entradas que se devuelven por equipo (las más recientes).
pub const POR_EQUIPO: i64 = 500;
const AJUSTE: &str = "restauraciones";

fn id_valido(id: &str) -> bool {
    id.strip_prefix("restauracion-").is_some_and(|r| !r.is_empty() && r.len() <= 50 && r.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
}

#[derive(Deserialize)]
pub struct Filtro {
    #[serde(default)]
    equipo: Option<String>,
}

fn ajustes_de(datos: Option<(String, i64, String)>) -> Value {
    let (d, t, por) = datos.unwrap_or_default();
    let v: Value = serde_json::from_str(&d).unwrap_or(Value::Null);
    json!({
        "motivo_obligatorio": v["motivo_obligatorio"].as_bool().unwrap_or(false),
        "actualizada": (t > 0).then(|| fecha(t)),
        "por": (!por.is_empty()).then_some(por),
    })
}

/// Lo de `listar` (también para «Todos los clientes», `GET /api/panel/restauraciones`).
pub(super) fn de_cliente(db: &dyn crate::almacen::Almacen, ctx: &crate::almacen::ClienteCtx, solo: Option<String>) -> crate::almacen::R<Value> {
    let equipos: Vec<String> = db.equipos(ctx)?.into_iter().map(|e| e.id).filter(|id| solo.as_ref().is_none_or(|s| s == id)).collect();
    let filtro = FiltroHistorial { tipos: vec!["restauracion".into()], ..Default::default() };
    let mut entradas = Vec::new();
    for e in &equipos {
        for f in db.historial(ctx, e, &filtro, POR_EQUIPO)? {
            if let Ok(mut v) = serde_json::from_str::<Value>(&f.datos) {
                v["equipo"] = json!(e);
                entradas.push(v);
            }
        }
    }
    let notas: Vec<Value> = db
        .notas_restauracion(ctx)?
        .into_iter()
        .filter(|n| equipos.contains(&n.equipo))
        .map(|n| json!({ "equipo": n.equipo, "id": n.id, "motivo": n.motivo, "notas": n.notas, "por": n.por, "actualizada": fecha(n.actualizada) }))
        .collect();
    Ok(json!({ "ajustes": ajustes_de(db.ajuste_cliente(ctx, AJUSTE)?), "entradas": entradas, "notas": notas }))
}

/// `GET /api/clientes/{c}/restauraciones[?equipo=]`: `{ ajustes, entradas: [{ equipo, … }], notas: [...] }`.
pub async fn listar(State(st): State<St>, u: Usuario, Path(c): Path<String>, Query(q): Query<Filtro>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Lectura).await?;
    let solo = q.equipo.filter(|e| !e.is_empty());
    let r = st.db(move |db| de_cliente(db, &ctx, solo)).await?;
    Ok(Json(r))
}

#[derive(Deserialize)]
pub struct Motivo {
    motivo: String,
    #[serde(default)]
    notas: Option<String>,
}

/// `PUT /api/clientes/{c}/equipos/{e}/restauraciones/{id}`: `{ motivo, notas? }`.
pub async fn poner_motivo(State(st): State<St>, u: Usuario, Path((c, e, id)): Path<(String, String, String)>, Json(p): Json<Motivo>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Tecnico).await?;
    if !id_valido(&id) {
        return Err(ErrorApi::datos("Restauración no válida."));
    }
    if !MOTIVOS.contains(&p.motivo.as_str()) {
        return Err(ErrorApi::datos("Motivo no válido."));
    }
    let notas = p.notas.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    if notas.as_ref().is_some_and(|n| n.chars().count() > MAX_NOTAS || n.chars().any(|c| c.is_control() && c != '\n')) {
        return Err(ErrorApi::datos(format!("Las notas, como mucho {MAX_NOTAS} caracteres.")));
    }
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let n = NotaRestauracion { equipo: e.clone(), id: id.clone(), motivo: p.motivo, notas, por: u.0.cuenta.nombre.clone(), actualizada: 0 };
    st.db_crudo(move |db| {
        if db.equipo(&ctx, &e)?.is_none() {
            return Err("no_existe".into());
        }
        if !db.poner_nota_restauracion(&ctx, &n, MAX_NOTAS_CLIENTE)? {
            return Err(format!("Como mucho {MAX_NOTAS_CLIENTE} restauraciones con motivo por cliente."));
        }
        db.auditar(&ctx, &actor, "motivo_restauracion", &format!("{e}/{id}"), &json!({ "motivo": n.motivo }).to_string())?;
        Ok(())
    })
    .await?
    .map_err(|e| if e == "no_existe" { ErrorApi::no_existe() } else { ErrorApi::datos(e) })?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct Ajustes {
    motivo_obligatorio: bool,
}

/// `PUT /api/clientes/{c}/restauraciones/ajustes`: `{ motivo_obligatorio }` (administrador o más).
pub async fn poner_ajustes(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(p): Json<Ajustes>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let por = u.0.cuenta.nombre.clone();
    let r = st
        .db(move |db| {
            let datos = json!({ "motivo_obligatorio": p.motivo_obligatorio }).to_string();
            db.poner_ajuste_cliente(&ctx, AJUSTE, &datos, &por)?;
            db.auditar(&ctx, &actor, "ajustes_restauraciones", "", &datos)?;
            Ok(ajustes_de(db.ajuste_cliente(&ctx, AJUSTE)?))
        })
        .await?;
    Ok(Json(r))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_de_restauracion() {
        assert!(id_valido("restauracion-0a1b2c3d-1111-2222-3333-444455556666"));
        assert!(!id_valido("restauracion-"));
        assert!(!id_valido("copia-1"));
        assert!(!id_valido("restauracion-a/b"));
        assert!(!id_valido(&format!("restauracion-{}", "x".repeat(51))));
    }

    #[test]
    fn ajustes_por_defecto() {
        assert_eq!(ajustes_de(None)["motivo_obligatorio"], false);
        assert_eq!(ajustes_de(Some((r#"{"motivo_obligatorio":true}"#.into(), 1, "Ana".into())))["motivo_obligatorio"], true);
    }
}
