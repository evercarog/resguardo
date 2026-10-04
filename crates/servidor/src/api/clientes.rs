//! Clientes, miembros e invitaciones.

use super::cuentas::MIEMBRO;
use crate::almacen::{ahora, Rol};
use crate::auth::{self, Usuario};
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use base64::Engine;
use rand::RngCore;
use serde::Deserialize;
use serde_json::{json, Value};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

fn nombre_valido(n: &str) -> Res<String> {
    let n = n.trim().to_string();
    if n.is_empty() || n.chars().count() > 80 {
        return Err(ErrorApi::datos("Escribe un nombre (hasta 80 caracteres)."));
    }
    Ok(n)
}

pub async fn listar(State(st): State<St>, u: Usuario) -> Res<Json<Value>> {
    let id = u.id().to_string();
    let clientes = st.db(move |db| db.clientes_de(&id)).await?;
    let mut out = Vec::new();
    for (c, rol) in clientes {
        let ctx = crate::almacen::ClienteCtx::autorizado(&c.id);
        let id = c.id.clone();
        let (equipos, avisos, marca) = st.db(move |db| Ok((db.equipos(&ctx)?.len(), db.avisos(&ctx, true)?.len(), super::marca::leer(db, &id)?))).await?;
        // v1.32: `marca` ({ acento, logo, … }), para el selector de clientes.
        out.push(
            json!({ "id": c.id, "nombre": c.nombre, "rol": rol.texto(), "equipos": equipos, "avisos": avisos, "marca": super::marca::json(&c.id, &marca) }),
        );
    }
    Ok(Json(json!(out)))
}

#[derive(Deserialize)]
pub struct Nuevo {
    nombre: String,
    espera_min_horas: Option<i64>,
}

pub async fn crear(State(st): State<St>, u: Usuario, Json(p): Json<Nuevo>) -> Res<Json<Value>> {
    if !u.0.cuenta.superusuario {
        return Err(ErrorApi::prohibido());
    }
    let nombre = nombre_valido(&p.nombre)?;
    let espera = p.espera_min_horas.unwrap_or(24);
    if !(1..=168).contains(&espera) {
        return Err(ErrorApi::datos("La espera mínima debe estar entre 1 y 168 horas."));
    }
    let mut sal = [0u8; 16];
    rand::rng().fill_bytes(&mut sal);
    let sal = B64.encode(sal);
    let (cuenta, correo) = (u.id().to_string(), u.0.cuenta.correo.clone());
    let cliente = st
        .db(move |db| {
            let c = db.crear_cliente(&nombre, &sal, espera)?;
            db.poner_rol(&cuenta, &c.id, Rol::Propietario)?;
            db.auditar_servidor(&format!("cuenta:{correo}"), "crear_cliente", &c.id, &json!({ "nombre": c.nombre }).to_string())?;
            db.auditar(&crate::almacen::ClienteCtx::autorizado(&c.id), &format!("cuenta:{correo}"), "crear_cliente", &c.id, "{}")?;
            Ok(c)
        })
        .await?;
    Ok(Json(json!({ "id": cliente.id, "nombre": cliente.nombre, "sal_cliente": cliente.sal_cliente, "espera_min_horas": cliente.espera_min_horas })))
}

pub async fn ver(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    let (_, rol) = u.miembro(&st, &c, MIEMBRO).await?;
    let (cliente, marca) = st.db(move |db| Ok((db.cliente(&c)?, super::marca::leer(db, &c)?))).await?;
    let cliente = cliente.ok_or_else(ErrorApi::no_existe)?;
    let marca = super::marca::json(&cliente.id, &marca);
    Ok(Json(json!({
        "id": cliente.id, "nombre": cliente.nombre, "sal_cliente": cliente.sal_cliente, "espera_min_horas": cliente.espera_min_horas, "rol": rol.texto(),
        // v1.32: la marca del cliente.
        "marca": marca,
    })))
}

#[derive(Deserialize)]
pub struct Renombrar {
    nombre: String,
}

pub async fn renombrar(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(p): Json<Renombrar>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Propietario).await?;
    let nombre = nombre_valido(&p.nombre)?;
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let c2 = c.clone();
    st.db(move |db| {
        db.renombrar_cliente(&c2, &nombre)?;
        db.auditar(&ctx, &actor, "renombrar_cliente", &c2, &json!({ "nombre": nombre }).to_string())
    })
    .await?;
    ver(State(st), u, Path(c)).await
}

pub async fn miembros(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    u.miembro(&st, &c, Rol::Propietario).await?;
    let m = st.db(move |db| db.miembros(&c)).await?;
    Ok(Json(json!(m.iter().map(|m| json!({ "cuenta": m.cuenta, "correo": m.correo, "nombre": m.nombre, "rol": m.rol.texto() })).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct PonerRol {
    rol: String,
}

/// Nunca se queda un cliente sin propietario.
fn quedaria_sin_propietario(miembros: &[crate::almacen::Miembro], cuenta: &str, nuevo: Option<Rol>) -> bool {
    let propietarios = miembros.iter().filter(|m| m.rol == Rol::Propietario).count();
    let es_propietario = miembros.iter().any(|m| m.cuenta == cuenta && m.rol == Rol::Propietario);
    es_propietario && propietarios == 1 && nuevo != Some(Rol::Propietario)
}

pub async fn poner_rol(State(st): State<St>, u: Usuario, Path((c, cuenta)): Path<(String, String)>, Json(p): Json<PonerRol>) -> Res<StatusCode> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Propietario).await?;
    let rol = Rol::de(&p.rol).ok_or_else(|| ErrorApi::datos("Rol no válido."))?;
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let r = st
        .db_crudo(move |db| {
            let m = db.miembros(&c)?;
            if !m.iter().any(|x| x.cuenta == cuenta) {
                return Err("no_existe".into());
            }
            if quedaria_sin_propietario(&m, &cuenta, Some(rol)) {
                return Err("El cliente necesita al menos un propietario.".into());
            }
            db.poner_rol(&cuenta, &c, rol)?;
            db.auditar(&ctx, &actor, "poner_rol", &cuenta, &json!({ "rol": rol.texto() }).to_string())
        })
        .await?;
    match r {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) if e == "no_existe" => Err(ErrorApi::no_existe()),
        Err(e) => Err(ErrorApi::datos(e)),
    }
}

pub async fn quitar(State(st): State<St>, u: Usuario, Path((c, cuenta)): Path<(String, String)>) -> Res<StatusCode> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Propietario).await?;
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let r = st
        .db_crudo(move |db| {
            let m = db.miembros(&c)?;
            if quedaria_sin_propietario(&m, &cuenta, None) {
                return Err("El cliente necesita al menos un propietario.".into());
            }
            db.quitar_miembro(&cuenta, &c)?;
            db.auditar(&ctx, &actor, "quitar_miembro", &cuenta, "{}")
        })
        .await?;
    r.map_err(ErrorApi::datos)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct Invitar {
    rol: String,
}

pub async fn invitar(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(p): Json<Invitar>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Propietario).await?;
    let rol = Rol::de(&p.rol).ok_or_else(|| ErrorApi::datos("Rol no válido."))?;
    let token = auth::ficha();
    let th = auth::hash_ficha(&token);
    let caduca = ahora() + 7 * 86_400;
    let (por, actor) = (u.id().to_string(), format!("cuenta:{}", u.0.cuenta.correo));
    st.db(move |db| {
        db.crear_invitacion(&th, &c, rol, caduca, &por)?;
        db.auditar(&ctx, &actor, "invitar", rol.texto(), "{}")
    })
    .await?;
    Ok(Json(json!({ "enlace": format!("/invitacion#{token}"), "caduca": super::fecha(caduca) })))
}

// ---------- F6: recibir un cliente, fichas, paquete e importación ----------

#[derive(Deserialize)]
pub struct Recibir {
    nombre: String,
    /// La sal del cliente en el servidor anterior: `K_cfg` y `K_exp` dependen de ella.
    sal_cliente: String,
    espera_min_horas: Option<i64>,
    /// Cuántos equipos pueden darse de alta con la ficha (1–1000).
    usos: Option<i64>,
    /// Validez de la ficha en días (1–365; 7 por defecto).
    dias: Option<i64>,
}

fn nueva_ficha() -> String {
    let mut b = [0u8; 24];
    rand::rng().fill_bytes(&mut b);
    crate::auth::base32(&b)
}

fn servidor_json(st: &St) -> Value {
    json!({
        "identidad": st.identidad_pub,
        "ca_pem": std::fs::read_to_string(st.datos.join("tls").join("ca.crt")).ok(),
    })
}

/// `POST /api/clientes/recibir` (superusuario): «Recibir un cliente» de otro
/// servidor, con su misma sal, y la ficha con la que entrarán sus equipos.
pub async fn recibir(State(st): State<St>, u: Usuario, Json(p): Json<Recibir>) -> Res<Json<Value>> {
    if !u.0.cuenta.superusuario {
        return Err(ErrorApi::prohibido());
    }
    let nombre = nombre_valido(&p.nombre)?;
    if !B64.decode(&p.sal_cliente).is_ok_and(|s| (16..=64).contains(&s.len())) {
        return Err(ErrorApi::datos("Sal del cliente no válida."));
    }
    let espera = p.espera_min_horas.unwrap_or(24);
    let (usos, dias) = (p.usos.unwrap_or(100), p.dias.unwrap_or(7));
    if !(1..=168).contains(&espera) || !(1..=1000).contains(&usos) || !(1..=365).contains(&dias) {
        return Err(ErrorApi::datos("Espera (1–168 h), usos (1–1000) o días (1–365) no válidos."));
    }
    let ficha = nueva_ficha();
    let caduca = crate::almacen::ahora() + dias * 86_400;
    let (cuenta, correo, h, sal) = (u.id().to_string(), u.0.cuenta.correo.clone(), crate::auth::hash_ficha(&ficha), p.sal_cliente.clone());
    let cliente = st
        .db(move |db| {
            let c = db.crear_cliente(&nombre, &sal, espera)?;
            db.poner_rol(&cuenta, &c.id, Rol::Propietario)?;
            db.crear_ficha(&h, &c.id, usos, caduca)?;
            db.auditar_servidor(&format!("cuenta:{correo}"), "recibir_cliente", &c.id, &json!({ "nombre": c.nombre }).to_string())?;
            db.auditar(
                &crate::almacen::ClienteCtx::autorizado(&c.id),
                &format!("cuenta:{correo}"),
                "recibir_cliente",
                &c.id,
                &json!({ "usos": usos }).to_string(),
            )?;
            Ok(c)
        })
        .await?;
    Ok(Json(json!({
        "cliente": { "id": cliente.id, "nombre": cliente.nombre, "sal_cliente": cliente.sal_cliente, "espera_min_horas": cliente.espera_min_horas },
        "ficha": ficha, "caduca": crate::api::fecha(caduca), "usos": usos,
        "servidor": servidor_json(&st),
    })))
}

#[derive(Deserialize)]
pub struct NuevaFicha {
    usos: Option<i64>,
    dias: Option<i64>,
}

/// `POST /api/clientes/{c}/fichas` (propietario): otra ficha para este cliente
/// (más equipos, o para usar este servidor como respaldo; hasta 365 días).
pub async fn ficha(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(p): Json<NuevaFicha>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Propietario).await?;
    let (usos, dias) = (p.usos.unwrap_or(100), p.dias.unwrap_or(7));
    if !(1..=1000).contains(&usos) || !(1..=365).contains(&dias) {
        return Err(ErrorApi::datos("Usos (1–1000) o días (1–365) no válidos."));
    }
    let ficha = nueva_ficha();
    let caduca = crate::almacen::ahora() + dias * 86_400;
    let (h, actor) = (crate::auth::hash_ficha(&ficha), format!("cuenta:{}", u.0.cuenta.correo));
    st.db(move |db| {
        db.crear_ficha(&h, &c, usos, caduca)?;
        db.auditar(&ctx, &actor, "ficha_recepcion", &c, &json!({ "usos": usos, "dias": dias }).to_string())
    })
    .await?;
    Ok(Json(json!({ "ficha": ficha, "caduca": crate::api::fecha(caduca), "usos": usos, "servidor": servidor_json(&st) })))
}

/// Tamaño máximo del paquete de exportación.
pub const MAX_PAQUETE: usize = 64 * 1024 * 1024;

fn ruta_paquete(st: &St, c: &str) -> Option<std::path::PathBuf> {
    (!c.is_empty() && c.chars().all(|x| x.is_ascii_alphanumeric() || x == '-')).then(|| st.datos.join("paquetes").join(format!("{c}.resguardo-cliente")))
}

/// `PUT /api/clientes/{c}/paquete` (administrador): guarda el paquete de
/// exportación **ya cifrado** en el navegador con `K_exp`. El servidor solo
/// comprueba la cabecera (que es de este cliente) y el tamaño.
pub async fn poner_paquete(State(st): State<St>, u: Usuario, Path(c): Path<String>, cuerpo: axum::body::Bytes) -> Res<StatusCode> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    if cuerpo.len() > MAX_PAQUETE {
        return Err(ErrorApi::datos("El paquete ocupa demasiado (64 MB como mucho)."));
    }
    let (sal, _) = resguardo_protocolo::paquete::cabecera(&cuerpo).map_err(ErrorApi::datos)?;
    let c2 = c.clone();
    let cliente = st.db(move |db| db.cliente(&c2)).await?.ok_or_else(ErrorApi::no_existe)?;
    if sal != cliente.sal_cliente {
        return Err(ErrorApi::datos("Ese paquete es de otro cliente (su sal no coincide)."));
    }
    let ruta = ruta_paquete(&st, &c).ok_or_else(ErrorApi::no_existe)?;
    tokio::fs::create_dir_all(ruta.parent().unwrap_or(&st.datos)).await.map_err(ErrorApi::interno)?;
    tokio::fs::write(&ruta, &cuerpo).await.map_err(ErrorApi::interno)?;
    let (actor, n) = (format!("cuenta:{}", u.0.cuenta.correo), cuerpo.len());
    st.db(move |db| db.auditar(&ctx, &actor, "guardar_paquete", &c, &json!({ "bytes": n }).to_string())).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/clientes/{c}/paquete` (administrador): el paquete cifrado, tal cual.
pub async fn ver_paquete(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<axum::response::Response> {
    u.miembro(&st, &c, Rol::Administrador).await?;
    let ruta = ruta_paquete(&st, &c).ok_or_else(ErrorApi::no_existe)?;
    let datos = tokio::fs::read(&ruta).await.map_err(|_| ErrorApi::no_existe())?;
    use axum::response::IntoResponse;
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (axum::http::header::CONTENT_DISPOSITION, "attachment; filename=\"cliente.resguardo-cliente\"".to_string()),
        ],
        datos,
    )
        .into_response())
}

pub async fn borrar_paquete(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<StatusCode> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let ruta = ruta_paquete(&st, &c).ok_or_else(ErrorApi::no_existe)?;
    let _ = tokio::fs::remove_file(ruta).await;
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    st.db(move |db| db.auditar(&ctx, &actor, "borrar_paquete", "paquete", "{}")).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct EntradaImportada {
    n: i64,
    /// Unix (segundos) o RFC 3339.
    creado: Value,
    actor: String,
    accion: String,
    objetivo: String,
    datos: String,
    hash: String,
    prev_hash: String,
}

#[derive(Deserialize)]
pub struct InformeImportado {
    equipo: String,
    recibido: Value,
    datos: Value,
}

#[derive(Deserialize)]
pub struct AvisoImportado {
    equipo: Option<String>,
    tipo: String,
    mensaje: String,
    creado: Value,
}

#[derive(Deserialize)]
pub struct Importar {
    /// De dónde viene (p. ej. la identidad del servidor anterior).
    origen: String,
    #[serde(default)]
    auditoria: Vec<EntradaImportada>,
    #[serde(default)]
    informes: Vec<InformeImportado>,
    #[serde(default)]
    avisos: Vec<AvisoImportado>,
    /// v1.3x: observaciones y comentarios del cliente (opcional).
    #[serde(default)]
    notas: Option<super::notas::NotasImportadas>,
}

fn instante(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| v.as_str().and_then(crate::api::de_fecha))
}

/// `POST /api/clientes/{c}/importar` (propietario): lo que el navegador sacó
/// del paquete descifrado (metadatos que este servidor ya iba a tener): la
/// auditoría del servidor anterior (con su cadena comprobada; se guarda
/// aparte y la propia la enlaza), informes y avisos. Una sola vez por cliente.
pub async fn importar(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(p): Json<Importar>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Propietario).await?;
    let origen = p.origen.chars().filter(|x| !x.is_control()).take(200).collect::<String>();
    let mut entradas = Vec::with_capacity(p.auditoria.len());
    for e in p.auditoria {
        let creado = instante(&e.creado).ok_or_else(|| ErrorApi::datos("Fecha no válida en la auditoría."))?;
        entradas.push(crate::almacen::EntradaAuditoria {
            n: e.n,
            creado,
            actor: e.actor,
            accion: e.accion,
            objetivo: e.objetivo,
            datos: e.datos,
            hash: e.hash,
            prev_hash: e.prev_hash,
        });
    }
    let ultimo = entradas.last().map(|e| e.hash.clone());
    let (n_aud, n_inf, n_av) = (entradas.len(), p.informes.len(), p.avisos.len());
    let (obs, coms) = super::notas::preparar_importadas(p.notas.unwrap_or_default(), &origen);
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let r = st
        .db_crudo(move |db| {
            db.importar_auditoria(&ctx, &origen, &entradas)?;
            for i in &p.informes {
                if let Some(t) = instante(&i.recibido) {
                    db.importar_informe(&ctx, &i.equipo, t, &i.datos)?;
                }
            }
            for a in &p.avisos {
                if let Some(t) = instante(&a.creado) {
                    db.importar_aviso(&ctx, a.equipo.as_deref(), &a.tipo, &a.mensaje, t)?;
                }
            }
            let (n_obs, n_com) = db.importar_notas(&ctx, &obs, &coms)?;
            let mut datos = json!({ "origen": origen, "auditoria": n_aud, "ultimo_hash": ultimo, "informes": n_inf, "avisos": n_av });
            if n_obs + n_com > 0 {
                datos["observaciones"] = json!(n_obs);
                datos["comentarios"] = json!(n_com);
            }
            db.auditar(&ctx, &actor, "importar_cliente", ctx.id(), &datos.to_string())?;
            Ok((n_obs, n_com))
        })
        .await?;
    let (n_obs, n_com) =
        r.map_err(|e| if e == "ya_importada" { ErrorApi::conflicto("Este cliente ya tiene datos importados.") } else { ErrorApi::datos(e) })?;
    Ok(Json(json!({ "auditoria": n_aud, "informes": n_inf, "avisos": n_av, "observaciones": n_obs, "comentarios": n_com })))
}

#[derive(Deserialize)]
pub struct DesdeImportada {
    desde: Option<i64>,
    limite: Option<i64>,
}

/// `GET /api/clientes/{c}/auditoria/importada` (técnico o más).
pub async fn auditoria_importada(State(st): State<St>, u: Usuario, Path(c): Path<String>, Query(q): Query<DesdeImportada>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Tecnico).await?;
    let (desde, limite) = (q.desde.unwrap_or(0), q.limite.unwrap_or(100).clamp(1, 1000));
    let e = st.db(move |db| db.auditoria_importada(&ctx, desde, limite)).await?;
    Ok(Json(json!(e
        .iter()
        .map(|e| json!({ "n": e.n, "creado": crate::api::fecha(e.creado), "actor": e.actor, "accion": e.accion, "objetivo": e.objetivo, "datos": e.datos, "hash": e.hash, "prev_hash": e.prev_hash }))
        .collect::<Vec<_>>())))
}
