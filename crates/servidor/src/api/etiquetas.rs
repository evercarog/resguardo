//! Ajustes de las etiquetas de los equipos (v1.52, tarea 6 del plan): el color
//! elegido (de la paleta de la consola), la plantilla de copia que se propone a
//! un equipo nuevo con esa etiqueta y cómo se avisa de sus equipos. Se guardan
//! por cliente, en claro, como las etiquetas (son metadatos para organizar; la
//! plantilla es solo su id: su nombre y sus carpetas siguen cifrados).
//!
//! Leerlos: cualquier persona del cliente (el color sale en todas las pantallas).
//! Cambiarlos: administradores y propietarios; los avisos, solo el propietario
//! (como el resto de las notificaciones del cliente).

use super::cuentas::MIEMBRO;
use super::equipos::normalizar_etiquetas;
use super::fecha;
use crate::almacen::{AjusteEtiqueta, Almacen, AvisosEtiqueta, CanalRef, ClienteCtx, Rol};
use crate::auth::Usuario;
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use crate::notificaciones::{self as notif, ajustes as nt, Severidad};
use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

/// Colores de la paleta (Okabe-Ito, `--et-0` … `--et-6` en la consola).
pub const N_COLORES: u8 = 7;
/// Como mucho, etiquetas con ajustes por cliente.
pub const MAX_AJUSTES: usize = 200;
/// Como mucho, canales que avisan siempre de una etiqueta.
pub const MAX_CANALES: usize = 10;

/// Una etiqueta limpia (como las de los equipos): sin espacios de más, de 1 a 32 caracteres, sin comas.
pub fn normalizar_etiqueta(x: &str) -> Result<String, String> {
    match normalizar_etiquetas(&[x.to_string()])?.pop() {
        Some(t) => Ok(t),
        None => Err("Escribe el nombre de la etiqueta.".into()),
    }
}

pub fn vista(a: &AjusteEtiqueta) -> Value {
    json!({
        "nombre": a.nombre,
        "color": a.color,
        "plantilla": a.plantilla,
        "avisos": a.avisos,
        "actualizada": if a.actualizada > 0 { Value::String(fecha(a.actualizada)) } else { Value::Null },
        "por": a.por,
    })
}

fn lista(db: &dyn Almacen, ctx: &ClienteCtx) -> crate::almacen::R<Value> {
    Ok(json!(db.ajustes_etiquetas(ctx)?.iter().map(vista).collect::<Vec<_>>()))
}

/// `GET /api/clientes/{c}/etiquetas` (cualquier persona del cliente).
pub async fn listar(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, MIEMBRO).await?;
    Ok(Json(st.db(move |db| lista(db, &ctx)).await?))
}

#[derive(Deserialize)]
pub struct Cambio {
    nombre: String,
    #[serde(default)]
    color: Option<u8>,
    #[serde(default)]
    plantilla: Option<String>,
    #[serde(default)]
    avisos: Option<AvisosEtiqueta>,
}

fn id_plantilla_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Los avisos que se pueden guardar: importancia «importante» o «crítico» y canales
/// compartidos que ya pueden recibir lo de este cliente (los del cliente y los del
/// servidor que no lo excluyen). Vacío → `None`.
pub fn valida_avisos(db: &dyn Almacen, cliente: &str, a: Option<AvisosEtiqueta>) -> Result<Option<AvisosEtiqueta>, String> {
    let Some(mut a) = a else { return Ok(None) };
    if a.importancia == Some(Severidad::Informativo) {
        a.importancia = None;
    }
    if a.canales.len() > MAX_CANALES {
        return Err(format!("Como mucho {MAX_CANALES} canales por etiqueta."));
    }
    let ajustes = nt::ajustes(db).map_err(|e| e.to_string())?;
    // Todos los que pueden recibir algo de este cliente, sin mirar la gravedad.
    let posibles = notif::destinos(db, &ajustes, cliente, None, &notif::DeEtiquetas::default()).map_err(|e| e.to_string())?;
    let mut canales: Vec<CanalRef> = Vec::new();
    for r in &a.canales {
        let ambito = match r.ambito.as_str() {
            "servidor" => "servidor".to_string(),
            "cliente" => notif::ambito_cliente(cliente),
            _ => return Err("Canal no válido.".into()),
        };
        let existe = posibles.iter().any(|d| d.ambito == ambito && d.canal.id == r.id && d.canal.tipo != nt::TipoCanal::Correo);
        if !existe {
            return Err("Ese canal ya no existe o no recibe avisos de este cliente.".into());
        }
        if !canales.contains(r) {
            canales.push(r.clone());
        }
    }
    a.canales = canales;
    Ok((!a.vacio()).then_some(a))
}

/// `PUT /api/clientes/{c}/etiquetas` (administrador o propietario; los avisos, solo el
/// propietario): `{ nombre, color?, plantilla?, avisos? }`. Sustituye lo que hubiera;
/// sin nada, la etiqueta vuelve a lo de siempre. Devuelve la lista entera.
pub async fn poner(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(p): Json<Cambio>) -> Res<Json<Value>> {
    let (ctx, rol) = u.miembro(&st, &c, Rol::Administrador).await?;
    let nombre = normalizar_etiqueta(&p.nombre).map_err(ErrorApi::datos)?;
    if p.color.is_some_and(|x| x >= N_COLORES) {
        return Err(ErrorApi::datos("Color no válido."));
    }
    if p.plantilla.as_deref().is_some_and(|x| !id_plantilla_valido(x)) {
        return Err(ErrorApi::datos("Id de plantilla no válido."));
    }
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let por = u.0.cuenta.nombre.clone();
    let r = st
        .db_crudo(move |db| {
            let previo = db.ajustes_etiquetas(&ctx)?.into_iter().find(|a| a.nombre.to_lowercase() == nombre.to_lowercase());
            let avisos = valida_avisos(db, ctx.id(), p.avisos)?;
            // Los avisos son cosa del propietario (como los canales del cliente).
            if rol < Rol::Propietario && avisos != previo.as_ref().and_then(|a| a.avisos.clone()) {
                return Err("prohibido".into());
            }
            if let Some(id) = &p.plantilla {
                if !db.plantillas(&ctx)?.iter().any(|x| &x.id == id) {
                    return Err("Esa plantilla ya no existe.".into());
                }
            }
            let a = AjusteEtiqueta { nombre: nombre.clone(), color: p.color, plantilla: p.plantilla, avisos, actualizada: 0, por };
            if a.color.is_none() && a.plantilla.is_none() && a.avisos.is_none() {
                if db.borrar_ajuste_etiqueta(&ctx, &nombre)? {
                    db.auditar(&ctx, &actor, "ajustes_etiqueta", &nombre, "{}")?;
                }
            } else {
                if !db.poner_ajuste_etiqueta(&ctx, &a, MAX_AJUSTES)? {
                    return Err(format!("Como mucho {MAX_AJUSTES} etiquetas con ajustes por cliente."));
                }
                let datos = json!({ "color": a.color, "plantilla": a.plantilla, "avisos": a.avisos }).to_string();
                db.auditar(&ctx, &actor, "ajustes_etiqueta", &nombre, &datos)?;
            }
            lista(db, &ctx)
        })
        .await?
        .map_err(|e| if e == "prohibido" { ErrorApi::prohibido() } else { ErrorApi::datos(e) })?;
    Ok(Json(r))
}

#[cfg(test)]
mod tests {
    use super::normalizar_etiqueta;

    #[test]
    fn nombre_de_etiqueta() {
        assert_eq!(normalizar_etiqueta("  Sede   norte ").unwrap(), "Sede norte");
        assert!(normalizar_etiqueta("   ").is_err());
        assert!(normalizar_etiqueta("a,b").is_err());
        assert!(normalizar_etiqueta(&"x".repeat(33)).is_err());
    }
}
