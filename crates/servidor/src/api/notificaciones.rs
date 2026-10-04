//! Notificaciones (docs/api-servidor.md, «Notificaciones»): canales del
//! servidor (su propietario) y de cada cliente (sus propietarios), «Enviar
//! prueba», el registro de envíos y las preferencias de cada persona.
//!
//! Los secretos de los canales nunca salen (solo «configurado»). Cambiar a
//! dónde va un canal o sus secretos pide además un código del autenticador
//! recién sacado. Todo cambio queda en la auditoría (sin secretos).

use crate::almacen::{ahora, Almacen, Rol};
use crate::auth::{self, Usuario};
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use crate::notificaciones::ajustes::{self, Ajustes, AjustesCliente, CambioCanal, Canal, PrefsCliente, PrefsPersona, MAX_CANALES};
use crate::notificaciones::{self as notif, Envio, Severidad, Silencio};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::Duration;

fn solo_propietario(u: &Usuario) -> Res<()> {
    if u.0.cuenta.superusuario {
        Ok(())
    } else {
        Err(ErrorApi::prohibido())
    }
}

fn actor(u: &Usuario) -> String {
    format!("cuenta:{}", u.0.cuenta.correo)
}

/// Un código del autenticador de quien lo hace, recién sacado (y gastado: no vale otra vez).
async fn codigo_fresco(st: &St, u: &Usuario, codigo: Option<&str>) -> Res<()> {
    let mal = || ErrorApi::nuevo(StatusCode::UNAUTHORIZED, "codigo", "Hace falta un código de tu aplicación de autenticación (recién sacado).");
    let codigo = codigo.filter(|c| !c.trim().is_empty()).ok_or_else(mal)?.to_string();
    if !st.limites.intento(&format!("notif-codigo:{}", u.id()), 10, Duration::from_secs(15 * 60)) {
        return Err(ErrorApi::demasiados());
    }
    let id = u.id().to_string();
    let yo = st.db(move |db| db.cuenta(&id)).await?.ok_or_else(ErrorApi::sin_sesion)?;
    let paso = match (&yo.totp_secreto, yo.totp_activo) {
        (Some(sec), true) => auth::comprueba_totp(sec, &codigo, yo.totp_ultimo, ahora()),
        _ => None,
    }
    .ok_or_else(mal)?;
    let id = u.id().to_string();
    if !st.db(move |db| db.gastar_totp(&id, paso)).await? {
        return Err(mal());
    }
    st.limites.olvidar(&format!("notif-codigo:{}", u.id()));
    Ok(())
}

// ---------- Vistas ----------

fn vista_ajustes(a: &Ajustes) -> Value {
    json!({
        "url_consola": a.url_consola, "max_por_hora": a.max_por_hora, "hora_resumen": a.hora_resumen, "dia_semanal": a.dia_semanal,
        "canales": a.canales.iter().map(ajustes::vista_canal).collect::<Vec<_>>(),
    })
}

/// Dónde acaba un envío, para enseñarlo: el correo de la persona o el servidor del canal.
fn vista_envio(e: &Envio, canales: &BTreeMap<(String, String), Canal>) -> Value {
    let canal = canales.get(&(e.ambito.clone(), e.canal.clone()));
    let destino = if e.destino.is_empty() {
        canal.map(|c| match c.tipo {
            ajustes::TipoCanal::Telegram => format!("chat {}", c.config.chat_id.clone().unwrap_or_default()),
            _ => c.config.servidor.clone().unwrap_or_default(),
        })
    } else {
        Some(e.destino.clone())
    };
    json!({
        "id": e.id, "creado": crate::api::fecha(e.creado), "enviado": crate::api::fecha_opt(e.enviado),
        "canal": canal.map(|c| json!({ "id": c.id, "nombre": c.nombre, "tipo": c.tipo })),
        "ambito": if e.ambito == "servidor" { "servidor" } else { "cliente" },
        "cliente": e.cliente, "destino": destino, "tipo": e.tipo, "severidad": e.severidad.clave(), "titulo": e.titulo,
        "estado": e.estado, "intentos": e.intentos,
        "siguiente": if e.estado == "pendiente" { json!(crate::api::fecha(e.siguiente)) } else { Value::Null },
        "error": e.error, "nota": e.nota,
    })
}

fn registro(db: &dyn Almacen, cliente: Option<&str>) -> crate::almacen::R<Vec<Value>> {
    let a = ajustes::ajustes(db)?;
    let mut canales: BTreeMap<(String, String), Canal> = a.canales.iter().map(|c| (("servidor".to_string(), c.id.clone()), c.clone())).collect();
    let envios = db.notif_registro(cliente, 100)?;
    let mut clientes: Vec<String> = envios.iter().filter_map(|e| e.ambito.strip_prefix("cliente:").map(str::to_string)).collect();
    clientes.sort();
    clientes.dedup();
    for c in clientes {
        for k in ajustes::ajustes_cliente(db, &c)?.canales {
            canales.insert((notif::ambito_cliente(&c), k.id.clone()), k);
        }
    }
    Ok(envios.iter().map(|e| vista_envio(e, &canales)).collect())
}

// ---------- Servidor ----------

/// `GET /api/servidor/notificaciones` (propietario del servidor).
pub async fn ver(State(st): State<St>, u: Usuario) -> Res<Json<Value>> {
    solo_propietario(&u)?;
    let a = st.db(|db| ajustes::ajustes(db)).await?;
    Ok(Json(vista_ajustes(&a)))
}

#[derive(Deserialize)]
pub struct CambioAjustes {
    /// `""` la quita.
    url_consola: Option<String>,
    max_por_hora: Option<u32>,
    hora_resumen: Option<String>,
    dia_semanal: Option<u8>,
}

/// `PUT /api/servidor/notificaciones`: dirección pública de la consola, tope por hora y hora de los resúmenes.
pub async fn cambiar(State(st): State<St>, u: Usuario, Json(c): Json<CambioAjustes>) -> Res<Json<Value>> {
    solo_propietario(&u)?;
    if let Some(url) = c.url_consola.as_deref().filter(|u| !u.trim().is_empty()) {
        ajustes::valida_url_consola(url).map_err(ErrorApi::datos)?;
    }
    if c.max_por_hora.is_some_and(|n| !(1..=120).contains(&n)) {
        return Err(ErrorApi::datos("El tope por hora va de 1 a 120."));
    }
    if c.hora_resumen.as_deref().is_some_and(|h| !ajustes::hora_valida(h)) {
        return Err(ErrorApi::datos("Hora no válida (HH:MM)."));
    }
    if c.dia_semanal.is_some_and(|d| !(1..=7).contains(&d)) {
        return Err(ErrorApi::datos("Día no válido (1 = lunes … 7 = domingo)."));
    }
    let actor = actor(&u);
    let a = st
        .db(move |db| {
            let mut a = ajustes::ajustes(db)?;
            if let Some(url) = &c.url_consola {
                a.url_consola = if url.trim().is_empty() { None } else { ajustes::valida_url_consola(url).ok() };
            }
            if let Some(n) = c.max_por_hora {
                a.max_por_hora = n;
            }
            if let Some(h) = c.hora_resumen {
                a.hora_resumen = h;
            }
            if let Some(d) = c.dia_semanal {
                a.dia_semanal = d;
            }
            ajustes::guardar_ajustes(db, &a)?;
            let datos = json!({ "url_consola": a.url_consola, "max_por_hora": a.max_por_hora, "hora_resumen": a.hora_resumen, "dia_semanal": a.dia_semanal });
            db.auditar_servidor(&actor, "notificaciones_ajustes", "", &datos.to_string())?;
            Ok(a)
        })
        .await?;
    Ok(Json(vista_ajustes(&a)))
}

/// Dónde vive un canal: el servidor o un cliente.
#[derive(Clone)]
enum Ambito {
    Servidor,
    Cliente(crate::almacen::ClienteCtx),
}

impl Ambito {
    fn texto(&self) -> String {
        match self {
            Ambito::Servidor => "servidor".into(),
            Ambito::Cliente(c) => notif::ambito_cliente(c.id()),
        }
    }
    fn leer(&self, db: &dyn Almacen) -> crate::almacen::R<Vec<Canal>> {
        Ok(match self {
            Ambito::Servidor => ajustes::ajustes(db)?.canales,
            Ambito::Cliente(c) => ajustes::ajustes_cliente(db, c.id())?.canales,
        })
    }
    fn guardar(&self, db: &dyn Almacen, canales: Vec<Canal>) -> crate::almacen::R<()> {
        match self {
            Ambito::Servidor => {
                let mut a = ajustes::ajustes(db)?;
                a.canales = canales;
                ajustes::guardar_ajustes(db, &a)
            }
            Ambito::Cliente(c) => ajustes::guardar_ajustes_cliente(db, c.id(), &AjustesCliente { canales }),
        }
    }
    fn auditar(&self, db: &dyn Almacen, actor: &str, accion: &str, objetivo: &str, datos: &str) -> crate::almacen::R<()> {
        match self {
            Ambito::Servidor => db.auditar_servidor(actor, accion, objetivo, datos),
            Ambito::Cliente(c) => db.auditar(c, actor, accion, objetivo, datos),
        }
    }
}

async fn crear_en(st: &St, u: &Usuario, ambito: Ambito, c: CambioCanal) -> Res<Json<Value>> {
    codigo_fresco(st, u, c.codigo.as_deref()).await?;
    let (clave, actor, por) = (st.notif.clave.clone(), actor(u), u.nombre().to_string());
    let r = st
        .db_crudo(move |db| {
            let mut canales = ambito.leer(db)?;
            if canales.len() >= MAX_CANALES {
                return Err(format!("Como mucho {MAX_CANALES} canales."));
            }
            let (canal, cambiado) = ajustes::aplicar(None, &c, &clave, &ambito.texto(), &por, ahora())?;
            if canal.tipo == ajustes::TipoCanal::Correo && canales.iter().any(|k| k.tipo == ajustes::TipoCanal::Correo) {
                return Err("Ya hay un canal de correo: cámbialo en vez de crear otro.".into());
            }
            canales.push(canal.clone());
            ambito.guardar(db, canales)?;
            let datos = json!({ "accion": "crear", "tipo": canal.tipo, "nombre": canal.nombre, "activo": canal.activo, "secretos": cambiado.secretos });
            ambito.auditar(db, &actor, "notificaciones_canal", &canal.id, &datos.to_string())?;
            Ok(canal)
        })
        .await?
        .map_err(ErrorApi::datos)?;
    Ok(Json(ajustes::vista_canal(&r)))
}

async fn cambiar_en(st: &St, u: &Usuario, ambito: Ambito, id: String, c: CambioCanal) -> Res<Json<Value>> {
    // ¿Cambia a dónde va o algún secreto? Entonces, el código (antes de tocar nada).
    let (clave, amb, id2) = (st.notif.clave.clone(), ambito.clone(), id.clone());
    let previo = st.db(move |db| Ok(amb.leer(db)?.into_iter().find(|k| k.id == id2))).await?.ok_or_else(ErrorApi::no_existe)?;
    let (_, cambiado) = ajustes::aplicar(Some(&previo), &c, &clave, &ambito.texto(), u.nombre(), ahora()).map_err(ErrorApi::datos)?;
    if cambiado.sensible {
        codigo_fresco(st, u, c.codigo.as_deref()).await?;
    }
    let (actor, por) = (actor(u), u.nombre().to_string());
    let r = st
        .db_crudo(move |db| {
            let mut canales = ambito.leer(db)?;
            let Some(i) = canales.iter().position(|k| k.id == id) else { return Err("no_existe".into()) };
            let (canal, cambiado) = ajustes::aplicar(Some(&canales[i]), &c, &clave, &ambito.texto(), &por, ahora())?;
            canales[i] = canal.clone();
            ambito.guardar(db, canales)?;
            let datos = json!({ "accion": "cambiar", "tipo": canal.tipo, "nombre": canal.nombre, "activo": canal.activo, "secretos": cambiado.secretos, "destino_cambiado": cambiado.sensible });
            ambito.auditar(db, &actor, "notificaciones_canal", &canal.id, &datos.to_string())?;
            Ok(canal)
        })
        .await?
        .map_err(|e| if e == "no_existe" { ErrorApi::no_existe() } else { ErrorApi::datos(e) })?;
    Ok(Json(ajustes::vista_canal(&r)))
}

async fn borrar_en(st: &St, u: &Usuario, ambito: Ambito, id: String) -> Res<StatusCode> {
    let actor = actor(u);
    let r = st
        .db(move |db| {
            let mut canales = ambito.leer(db)?;
            let Some(i) = canales.iter().position(|k| k.id == id) else { return Ok(false) };
            let canal = canales.remove(i);
            ambito.guardar(db, canales)?;
            let datos = json!({ "accion": "borrar", "tipo": canal.tipo, "nombre": canal.nombre });
            ambito.auditar(db, &actor, "notificaciones_canal", &canal.id, &datos.to_string())?;
            Ok(true)
        })
        .await?;
    if r {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ErrorApi::no_existe())
    }
}

async fn probar_en(st: &St, u: &Usuario, ambito: Ambito, id: String) -> Res<Json<Value>> {
    if !st.limites.intento(&format!("notif-prueba:{}", u.id()), 20, Duration::from_secs(3600)) {
        return Err(ErrorApi::demasiados());
    }
    let (amb, id2) = (ambito.clone(), id.clone());
    let canal = st.db(move |db| Ok(amb.leer(db)?.into_iter().find(|k| k.id == id2))).await?.ok_or_else(ErrorApi::no_existe)?;
    // Un correo de prueba va a quien lo pide.
    let destino = if canal.tipo == ajustes::TipoCanal::Correo { u.0.cuenta.correo.clone() } else { String::new() };
    let (st2, quien) = (st.clone(), u.nombre().to_string());
    let cliente = match &ambito {
        Ambito::Cliente(c) => Some(c.id().to_string()),
        Ambito::Servidor => None,
    };
    let r =
        tokio::task::spawn_blocking(move || notif::probar(st2.db.as_ref(), &st2.notif, &ambito.texto(), &canal, &destino, &quien, cliente.as_deref(), ahora()))
            .await
            .map_err(ErrorApi::interno)?
            .map_err(ErrorApi::interno)?;
    Ok(Json(match r {
        Ok(()) => json!({ "ok": true, "mensaje": "Enviado. Mira si ha llegado." }),
        Err(e) => json!({ "ok": false, "mensaje": e }),
    }))
}

/// `POST /api/servidor/notificaciones/canales`.
pub async fn crear_canal(State(st): State<St>, u: Usuario, Json(c): Json<CambioCanal>) -> Res<Json<Value>> {
    solo_propietario(&u)?;
    crear_en(&st, &u, Ambito::Servidor, c).await
}

/// `PATCH /api/servidor/notificaciones/canales/{k}`.
pub async fn cambiar_canal(State(st): State<St>, u: Usuario, Path(k): Path<String>, Json(c): Json<CambioCanal>) -> Res<Json<Value>> {
    solo_propietario(&u)?;
    cambiar_en(&st, &u, Ambito::Servidor, k, c).await
}

/// `DELETE /api/servidor/notificaciones/canales/{k}`.
pub async fn borrar_canal(State(st): State<St>, u: Usuario, Path(k): Path<String>) -> Res<StatusCode> {
    solo_propietario(&u)?;
    borrar_en(&st, &u, Ambito::Servidor, k).await
}

/// `POST /api/servidor/notificaciones/canales/{k}/prueba`.
pub async fn probar_canal(State(st): State<St>, u: Usuario, Path(k): Path<String>) -> Res<Json<Value>> {
    solo_propietario(&u)?;
    probar_en(&st, &u, Ambito::Servidor, k).await
}

/// `GET /api/servidor/notificaciones/registro`: los últimos 100 envíos (de todos).
pub async fn registro_servidor(State(st): State<St>, u: Usuario) -> Res<Json<Value>> {
    solo_propietario(&u)?;
    Ok(Json(json!(st.db(|db| registro(db, None)).await?)))
}

// ---------- Cliente ----------

/// `GET /api/clientes/{c}/notificaciones` (propietario): sus canales y lo que pone el servidor.
pub async fn ver_cliente(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    u.miembro(&st, &c, Rol::Propietario).await?;
    let v = st
        .db(move |db| {
            let a = ajustes::ajustes(db)?;
            let propios = ajustes::ajustes_cliente(db, &c)?;
            let correo =
                notif::correo_de(&a, &propios, &c).map(|(amb, k)| json!({ "de": if amb == "servidor" { "servidor" } else { "cliente" }, "nombre": k.nombre }));
            let del_servidor: Vec<Value> = a
                .canales
                .iter()
                .filter(|k| {
                    k.tipo != ajustes::TipoCanal::Correo && k.activo && ajustes::completo(k) && k.reglas.clientes.as_ref().is_none_or(|l| l.contains(&c))
                })
                .map(|k| json!({ "nombre": k.nombre, "tipo": k.tipo, "severidades": k.reglas.severidades }))
                .collect();
            Ok(json!({
                "canales": propios.canales.iter().map(ajustes::vista_canal).collect::<Vec<_>>(),
                "correo": correo,
                "servidor": { "canales": del_servidor, "url_consola": a.url_consola },
            }))
        })
        .await?;
    Ok(Json(v))
}

async fn ctx_propietario(st: &St, u: &Usuario, c: &str) -> Res<Ambito> {
    let (ctx, _) = u.miembro(st, c, Rol::Propietario).await?;
    Ok(Ambito::Cliente(ctx))
}

pub async fn crear_canal_cliente(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(cambio): Json<CambioCanal>) -> Res<Json<Value>> {
    let a = ctx_propietario(&st, &u, &c).await?;
    crear_en(&st, &u, a, cambio).await
}

pub async fn cambiar_canal_cliente(
    State(st): State<St>,
    u: Usuario,
    Path((c, k)): Path<(String, String)>,
    Json(cambio): Json<CambioCanal>,
) -> Res<Json<Value>> {
    let a = ctx_propietario(&st, &u, &c).await?;
    cambiar_en(&st, &u, a, k, cambio).await
}

pub async fn borrar_canal_cliente(State(st): State<St>, u: Usuario, Path((c, k)): Path<(String, String)>) -> Res<StatusCode> {
    let a = ctx_propietario(&st, &u, &c).await?;
    borrar_en(&st, &u, a, k).await
}

pub async fn probar_canal_cliente(State(st): State<St>, u: Usuario, Path((c, k)): Path<(String, String)>) -> Res<Json<Value>> {
    let a = ctx_propietario(&st, &u, &c).await?;
    probar_en(&st, &u, a, k).await
}

/// `GET /api/clientes/{c}/notificaciones/registro` (propietario): lo de este cliente.
pub async fn registro_cliente(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    u.miembro(&st, &c, Rol::Propietario).await?;
    Ok(Json(json!(st.db(move |db| registro(db, Some(&c))).await?)))
}

fn vista_prefs(p: &PrefsCliente, propias: bool) -> Value {
    // De más a menos grave.
    let mut inmediatos = p.inmediatos.clone();
    inmediatos.sort_by(|a, b| b.cmp(a));
    json!({ "inmediatos": inmediatos, "resumen": p.resumen, "propias": propias })
}

/// `GET /api/clientes/{c}/notificaciones/personas` (propietario): qué recibe cada persona por correo.
pub async fn personas(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    u.miembro(&st, &c, Rol::Propietario).await?;
    let v = st
        .db(move |db| {
            let mut out = Vec::new();
            for m in db.miembros(&c)? {
                let (p, propias) = ajustes::prefs_cliente(db, &c, &m.cuenta, m.rol)?;
                let persona = ajustes::prefs_persona(db, &m.cuenta)?;
                out.push(json!({
                    "cuenta": m.cuenta, "nombre": m.nombre, "correo": m.correo, "rol": m.rol.texto(),
                    "preferencias": vista_prefs(&p, propias),
                    "silencio": persona.silencio, "resumen_diario": persona.resumen_diario, "resumen_semanal": persona.resumen_semanal,
                }));
            }
            Ok(json!(out))
        })
        .await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct CambioPrefs {
    inmediatos: Vec<Severidad>,
    resumen: bool,
}

/// `PUT /api/clientes/{c}/notificaciones/personas/{cuenta}`: el propietario del cliente, o la propia persona.
pub async fn poner_prefs(State(st): State<St>, u: Usuario, Path((c, cuenta)): Path<(String, String)>, Json(p): Json<CambioPrefs>) -> Res<Json<Value>> {
    let minimo = if cuenta == u.id() { Rol::Lectura } else { Rol::Propietario };
    let (ctx, _) = u.miembro(&st, &c, minimo).await?;
    let mut inmediatos = p.inmediatos;
    inmediatos.sort();
    inmediatos.dedup();
    let prefs = PrefsCliente { inmediatos, resumen: p.resumen };
    let actor = actor(&u);
    let r = st
        .db_crudo(move |db| {
            if !db.miembros(&c)?.iter().any(|m| m.cuenta == cuenta) {
                return Err("no_existe".into());
            }
            ajustes::guardar_prefs_cliente(db, &c, &cuenta, &prefs)?;
            db.auditar(&ctx, &actor, "notificaciones_preferencias", &cuenta, &json!({ "inmediatos": prefs.inmediatos, "resumen": prefs.resumen }).to_string())?;
            Ok(prefs)
        })
        .await?
        .map_err(|e| if e == "no_existe" { ErrorApi::no_existe() } else { ErrorApi::datos(e) })?;
    Ok(Json(vista_prefs(&r, true)))
}

// ---------- La propia cuenta ----------

/// `GET /api/cuenta/notificaciones`: lo que me llega (horas de silencio, resúmenes y, por cliente, qué avisos).
pub async fn mias(State(st): State<St>, u: Usuario) -> Res<Json<Value>> {
    let id = u.id().to_string();
    let v = st
        .db(move |db| {
            let persona = ajustes::prefs_persona(db, &id)?;
            let a = ajustes::ajustes(db)?;
            let mut clientes = Vec::new();
            for (cl, rol) in db.clientes_de(&id)? {
                let (p, propias) = ajustes::prefs_cliente(db, &cl.id, &id, rol)?;
                let propios = ajustes::ajustes_cliente(db, &cl.id)?;
                let correo = notif::correo_de(&a, &propios, &cl.id).is_some();
                clientes.push(json!({ "id": cl.id, "nombre": cl.nombre, "rol": rol.texto(), "correo": correo, "preferencias": vista_prefs(&p, propias) }));
            }
            Ok(json!({
                "silencio": persona.silencio, "resumen_diario": persona.resumen_diario, "resumen_semanal": persona.resumen_semanal,
                "hora_resumen": a.hora_resumen, "dia_semanal": a.dia_semanal, "clientes": clientes,
            }))
        })
        .await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct CambioMias {
    /// `null` quita las horas de silencio.
    #[serde(default, with = "doble_opcion")]
    silencio: Option<Option<Silencio>>,
    resumen_diario: Option<bool>,
    resumen_semanal: Option<bool>,
}

/// Distingue «no viene» de `null`.
mod doble_opcion {
    use serde::{Deserialize, Deserializer};
    pub fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(d: D) -> Result<Option<Option<T>>, D::Error> {
        Ok(Some(Option::deserialize(d)?))
    }
}

/// `PUT /api/cuenta/notificaciones`.
pub async fn cambiar_mias(State(st): State<St>, u: Usuario, Json(c): Json<CambioMias>) -> Res<Json<Value>> {
    if let Some(Some(s)) = &c.silencio {
        if !ajustes::hora_valida(&s.desde) || !ajustes::hora_valida(&s.hasta) {
            return Err(ErrorApi::datos("Las horas de silencio van como HH:MM (p. ej. 22:00 a 07:00)."));
        }
    }
    let (id, actor) = (u.id().to_string(), actor(&u));
    st.db(move |db| {
        let mut p: PrefsPersona = ajustes::prefs_persona(db, &id)?;
        if let Some(s) = c.silencio {
            p.silencio = s;
        }
        if let Some(x) = c.resumen_diario {
            p.resumen_diario = x;
        }
        if let Some(x) = c.resumen_semanal {
            p.resumen_semanal = x;
        }
        ajustes::guardar_prefs_persona(db, &id, &p)?;
        db.auditar_servidor(&actor, "notificaciones_mias", &id, &serde_json::to_string(&p).map_err(|e| e.to_string())?)
    })
    .await?;
    mias(State(st), u).await
}
