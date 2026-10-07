//! 0.7.26 (bloque 8): los datos comunes del cliente en la consola web
//! (`crate::datos_comunes`, docs/consolas-multiples.md §6.5). Solo
//! administradores y propietarios (como cambiar los colores, el catálogo o las
//! plantillas). Las órdenes a los equipos las sella el navegador; aquí se
//! guarda qué hay, qué falta mandar y las diferencias entre consolas.

use crate::almacen::Rol;
use crate::auth::Usuario;
use crate::datos_comunes::{self as dcs, Estado};
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use crate::vivo::Cambio;
use axum::extract::{Path, State};
use axum::Json;
use resguardo_protocolo::datos_cliente::{self as dc, Clase};
use serde::Deserialize;
use serde_json::{json, Value};

/// `GET /api/clientes/{c}/datos-comunes` (administrador): `{ filas, sin_compartir }`.
pub async fn ver(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let propia = st.identidad_pub.clone();
    let r = st
        .db(move |db| {
            let filas = dcs::filas(db, &ctx)?;
            let sin = dcs::sin_compartir(db, &ctx, &filas)?;
            Ok(json!({ "filas": filas.iter().map(|(k, f)| dcs::vista(k, f, &propia)).collect::<Vec<_>>(), "sin_compartir": sin }))
        })
        .await?;
    Ok(Json(r))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Nueva {
    clave: String,
    #[serde(default)]
    valor: Value,
    /// Lo que ya había aquí antes de compartir (solo si aún no hay nada).
    #[serde(default)]
    semilla: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Poner {
    entradas: Vec<Nueva>,
}

/// `POST /api/clientes/{c}/datos-comunes` (administrador): pone aquí estos valores (o
/// los de aquí, con `semilla`) y los deja por enviar a los equipos. Para elegir en una
/// diferencia y para compartir lo de antes. Una plantilla no lleva valor: va la de
/// aquí, cifrada como está (`null` la borra). Devuelve `{ entradas }` para la orden.
pub async fn poner(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(p): Json<Poner>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    if p.entradas.is_empty() || p.entradas.len() > dc::MAX_ENTRADAS_ORDEN {
        return Err(ErrorApi::datos(format!("De 1 a {} datos cada vez.", dc::MAX_ENTRADAS_ORDEN)));
    }
    for n in &p.entradas {
        let Some((clase, _)) = dc::clase(&n.clave) else { return Err(ErrorApi::datos("Dato común desconocido.")) };
        if clase != Clase::Plantilla {
            dc::valor_valido(&n.clave, &n.valor).map_err(ErrorApi::datos)?;
        }
    }
    let (propia, por, actor) = (st.identidad_pub.clone(), u.0.cuenta.nombre.clone(), format!("cuenta:{}", u.0.cuenta.correo));
    let r = st
        .db_crudo(move |db| {
            let mut out = Vec::new();
            for n in p.entradas {
                let clase = dc::clase(&n.clave).map(|x| x.0);
                // El valor que se reparte: el que se elige, o el de aquí para una plantilla o una semilla.
                let valor = if clase == Some(Clase::Plantilla) && !n.valor.is_null() || n.semilla {
                    match dcs::valor_para_compartir(db, &ctx, &n.clave)? {
                        Some(v) if !v.is_null() => v,
                        _ if n.semilla => continue,
                        _ => return Err("Esa plantilla no está en esta consola o es demasiado grande para compartirla (hasta 32 KiB).".into()),
                    }
                } else {
                    dc::valor_valido(&n.clave, &n.valor)?
                };
                if !n.semilla {
                    let local = dcs::valor_local(db, &ctx, &n.clave)?;
                    if !dcs::iguales(&n.clave, &local, &valor) || clase == Some(Clase::Plantilla) && valor.is_null() && !local.is_null() {
                        dcs::aplicar_local(db, &ctx, &n.clave, &valor, &por)?;
                    }
                }
                if let Some(e) = dcs::registrar(db, &ctx, &n.clave, &valor, n.semilla, &por, &propia)? {
                    let datos = json!({ "semilla": n.semilla });
                    db.auditar(&ctx, &actor, "datos_comunes", &n.clave, &datos.to_string())?;
                    out.push(json!({ "clave": n.clave, "valor": e.valor, "cambiado": e.cambiado, "semilla": e.semilla, "por": e.por }));
                }
            }
            Ok(out)
        })
        .await?
        .map_err(ErrorApi::datos)?;
    st.vivo.avisar(&c, Cambio::DatosComunes);
    Ok(Json(json!({ "entradas": r })))
}

#[derive(Deserialize)]
pub struct Enviada {
    clave: String,
    cambiado: String,
}

#[derive(Deserialize)]
pub struct Enviadas {
    entradas: Vec<Enviada>,
}

/// `POST /api/clientes/{c}/datos-comunes/enviadas` (administrador): estos datos ya van en
/// órdenes a los equipos (dejan de estar «por enviar»). `{ n }`.
pub async fn enviadas(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(p): Json<Enviadas>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let l: Vec<(String, String)> = p.entradas.into_iter().take(1000).map(|e| (e.clave, e.cambiado)).collect();
    let n = st.db(move |db| dcs::marcar_enviadas(db, &ctx, &l)).await?;
    Ok(Json(json!({ "n": n })))
}

#[derive(Deserialize)]
pub struct Traer {
    clave: String,
    cambiado: String,
    cifrado: String,
}

/// `POST /api/clientes/{c}/datos-comunes/traer` (administrador): una plantilla de otra
/// consola ya abierta en el navegador y cifrada otra vez con la clave de aquí.
pub async fn traer(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(p): Json<Traer>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &p.cifrado).map_err(|_| ErrorApi::datos("Plantilla no válida (base64)."))?;
    if bytes.len() < 40 || bytes.len() > crate::api::plantillas::MAX_BYTES {
        return Err(ErrorApi::datos("Plantilla demasiado grande o vacía (hasta 64 KiB)."));
    }
    let (por, actor) = (u.0.cuenta.nombre.clone(), format!("cuenta:{}", u.0.cuenta.correo));
    let ok = st
        .db_crudo(move |db| {
            let ok = dcs::traer(db, &ctx, &p.clave, &p.cambiado, &p.cifrado, &por)?;
            if ok {
                db.auditar(&ctx, &actor, "guardar_plantilla", p.clave.trim_start_matches("plantilla:"), &json!({ "desde_otra_consola": true }).to_string())?;
            }
            Ok(ok)
        })
        .await?
        .map_err(ErrorApi::datos)?;
    if !ok {
        return Err(ErrorApi::datos("Esa plantilla ya no está por traer (cambió o ya se trajo)."));
    }
    st.vivo.avisar(&c, Cambio::DatosComunes);
    Ok(Json(json!({ "estado": Estado::Aplicado })))
}
