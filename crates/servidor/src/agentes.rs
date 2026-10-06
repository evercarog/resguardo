//! El lado de los agentes (docs/api-servidor.md, §4, §8 y §9): unirse,
//! WebSocket y sondeo, resultados firmados, informes, configuración,
//! avisos, mensajes de sesión y subida al relé.

use crate::almacen::{ahora, ClienteCtx, EquipoNuevo};
use crate::api::orden_agente;
use crate::auth;
use crate::error::{ErrorApi, Res};
use crate::estado::{IpCliente, St};
use crate::vivo::Cambio;
use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{DefaultBodyLimit, FromRequestParts, Path, Query, State};
use axum::http::request::Parts;
use axum::http::{header, StatusCode};
use axum::response::Response;
use axum::routing::{get, post, put};
use axum::{Extension, Json, Router};
use base64::Engine;
use ed25519_dalek::{Signer, Verifier};
use futures_util::{SinkExt, StreamExt};
use resguardo_protocolo::derivaciones;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::mpsc;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
/// Tamaño máximo de un trozo del relé (4 MiB más el cifrado).
pub const MAX_TROZO: usize = 4 * 1024 * 1024 + 64;

pub fn router() -> Router<St> {
    Router::new()
        .route("/api/agente/unirse", post(unirse))
        .route("/api/agente/recibir", post(recibir))
        .route("/api/agente/canal", get(canal))
        .route("/api/agente/tomar", post(tomar))
        .route("/api/agente/resultado", post(resultado_http))
        .route("/api/agente/informe", post(informe_http))
        .route("/api/agente/config", post(config_http))
        .route("/api/agente/aviso", post(aviso_http))
        .route("/api/agente/historial", post(historial_http))
        .route("/api/agente/progreso", post(progreso_http))
        .route("/api/agente/sesiones/{s}/mensajes", get(leer_sesion).post(escribir_sesion))
        .route("/api/agente/relevos/{r}/trozos/{n}", put(subir_trozo).layer(DefaultBodyLimit::max(MAX_TROZO)))
        .route("/api/agente/relevos/{r}/fin", post(fin_relevo))
        .layer(DefaultBodyLimit::max(1024 * 1024))
}

fn ip_de(ip: &Option<Extension<IpCliente>>) -> String {
    crate::estado::clave_ip(ip.as_ref().and_then(|Extension(IpCliente(i))| *i))
}

fn b64_32(s: &str) -> bool {
    B64.decode(s).is_ok_and(|v| v.len() == 32)
}

fn texto_corto(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect()
}

// ---------- Unirse (anónima) ----------

#[derive(Deserialize)]
pub struct Unirse {
    codigo_hash: String,
    nombre: String,
    so: String,
    version: String,
    box_pub: String,
    sign_pub: String,
    sal_equipo: String,
    /// v1.26: 3 si el equipo calcula el SAS v3 (con la huella de la autoridad TLS que fijó).
    /// Los agentes anteriores no lo mandan: SAS v2.
    #[serde(default)]
    sas_version: Option<i64>,
}

/// El código de comprobación de un equipo según la versión que anunció (v1.26).
pub fn sas_de(st: &St, version: Option<i64>, box_pub: &str, sign_pub: &str) -> (i64, String) {
    if version.unwrap_or(2) >= 3 {
        (3, derivaciones::sas_v3(&st.identidad_pub, box_pub, sign_pub, &st.huella_ca))
    } else {
        (2, derivaciones::sas_v2(&st.identidad_pub, box_pub, sign_pub))
    }
}

async fn unirse(State(st): State<St>, ip: Option<Extension<IpCliente>>, Json(p): Json<Unirse>) -> Res<Json<Value>> {
    if !st.limites.intento(&format!("unirse:{}", ip_de(&ip)), 20, Duration::from_secs(3600)) {
        return Err(ErrorApi::demasiados());
    }
    if p.codigo_hash.len() != 64 || !p.codigo_hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ErrorApi::datos("Código no válido."));
    }
    if !b64_32(&p.box_pub) || !b64_32(&p.sign_pub) || !B64.decode(&p.sal_equipo).is_ok_and(|s| (16..=64).contains(&s.len())) {
        return Err(ErrorApi::datos("Claves del equipo no válidas."));
    }
    let nombre = texto_corto(p.nombre.trim(), 80);
    if nombre.is_empty() {
        return Err(ErrorApi::datos("Falta el nombre del equipo."));
    }
    let mut secreto = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rng(), &mut secreto);
    let secreto = B64.encode(secreto);
    let equipo = EquipoNuevo {
        id: uuid::Uuid::new_v4().to_string(),
        nombre,
        so: texto_corto(&p.so, 40),
        version: texto_corto(&p.version, 40),
        box_pub: p.box_pub,
        sign_pub: p.sign_pub,
        sal_equipo: p.sal_equipo,
        secreto_hash: auth::hash_ficha(&secreto),
    };
    let e2 = equipo.clone();
    let (sas_version, sas) = sas_de(&st, p.sas_version, &equipo.box_pub, &equipo.sign_pub);
    let publico = st.opciones.publico;
    let cliente = st
        .db(move |db| {
            let Some((cliente, emp)) = db.tomar_codigo(&p.codigo_hash)? else { return Ok(None) };
            let ctx = ClienteCtx::autorizado(&cliente);
            let Some(e) = db.emparejamiento(&ctx, &emp)? else { return Ok(None) };
            if e.estado != "abierto" || e.caduca <= ahora() {
                return Ok(None);
            }
            // v1.34: la cuota de equipos del cliente (la consola ya no da códigos si está llena).
            if let Err(m) = crate::cuotas::cabe_otro_equipo(db, publico, &cliente, db.equipos(&ctx)?.len())? {
                return Ok(Some(Err(m)));
            }
            // Preparado (v1.17): el equipo entra con el nombre que se le dio en la consola.
            let mut e2 = e2;
            if let Some(n) = e.nombre.filter(|n| !n.trim().is_empty()) {
                e2.nombre = n;
            }
            db.crear_equipo(&ctx, &e2)?;
            db.indexar_equipo(&e2.id, &cliente)?;
            db.poner_estado_emparejamiento(&ctx, &emp, "unido", Some(&e2.id))?;
            db.poner_sas_emparejamiento(&ctx, &emp, sas_version)?;
            db.auditar(&ctx, &format!("equipo:{}", e2.id), "unirse", &e2.id, &json!({ "nombre": e2.nombre, "so": e2.so }).to_string())?;
            Ok(Some(Ok(cliente)))
        })
        .await?;
    let cliente = cliente.ok_or_else(|| ErrorApi::nuevo(StatusCode::NOT_FOUND, "codigo", "Código no válido o caducado.").acceso("codigo_equipo"))?;
    let cliente = cliente.map_err(error_cuota)?;
    st.vivo.avisar(&cliente, Cambio::Equipo(&equipo.id));
    Ok(Json(json!({
        "equipo_id": equipo.id,
        "secreto": secreto,
        "cliente_id": cliente,
        "servidor": { "identidad": st.identidad_pub, "nombre": "Resguardo Server" },
        "sas": sas,
        "sas_version": sas_version,
    })))
}

// ---------- Recibir un equipo de otro servidor (F6) ----------

#[derive(Deserialize)]
pub struct Recibir {
    ficha: String,
    /// Su id (el del servidor anterior): la etiqueta lo incluye y la consola lo tiene fijado.
    equipo_id: String,
    nombre: String,
    so: String,
    version: String,
    box_pub: String,
    sign_pub: String,
    sal_equipo: String,
    /// La etiqueta la calcula el propio equipo con su `K_cfg`; la consola la comprueba como siempre.
    etiqueta: String,
    espera_min_horas: Option<i64>,
    /// v1.36: `anadir_consola` si el equipo se conecta también a este servidor
    /// (sigue gestionado desde otro): solo cambia el texto del aviso.
    #[serde(default)]
    motivo: Option<String>,
}

/// `POST /api/agente/recibir` (anónima, con límite por IP): un equipo que se
/// muda a este servidor (`cambiar_servidor`, o un servidor de respaldo) entra
/// con la ficha de «Recibir un cliente», ya confirmado.
async fn recibir(State(st): State<St>, ip: Option<Extension<IpCliente>>, Json(p): Json<Recibir>) -> Res<Json<Value>> {
    if !st.limites.intento(&format!("recibir:{}", ip_de(&ip)), 60, Duration::from_secs(3600)) {
        return Err(ErrorApi::demasiados());
    }
    if !b64_32(&p.box_pub) || !b64_32(&p.sign_pub) || !b64_32(&p.etiqueta) || !B64.decode(&p.sal_equipo).is_ok_and(|s| (16..=64).contains(&s.len())) {
        return Err(ErrorApi::datos("Claves del equipo no válidas."));
    }
    let nombre = texto_corto(p.nombre.trim(), 80);
    if nombre.is_empty() {
        return Err(ErrorApi::datos("Falta el nombre del equipo."));
    }
    let mut secreto = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rng(), &mut secreto);
    let secreto = B64.encode(secreto);
    if uuid::Uuid::parse_str(&p.equipo_id).is_err() {
        return Err(ErrorApi::datos("Id del equipo no válido."));
    }
    let id = p.equipo_id.clone();
    let ficha_hash = auth::hash_ficha(p.ficha.trim());
    let (box_pub, sign_pub, etiqueta) = (p.box_pub.clone(), p.sign_pub.clone(), p.etiqueta.clone());
    let espera = p.espera_min_horas.filter(|h| (1..=168).contains(h));
    let (so, version, sal) = (texto_corto(&p.so, 40), texto_corto(&p.version, 40), p.sal_equipo.clone());
    let sec_hash = auth::hash_ficha(&secreto);
    let otra_consola = p.motivo.as_deref() == Some("anadir_consola");
    let publico = st.opciones.publico;
    let r = st
        .db_crudo(move |db| {
            let Some(cliente) = db.usar_ficha(&ficha_hash, ahora())? else { return Ok(None) };
            let ctx = ClienteCtx::autorizado(&cliente);
            // v1.34: la cuota de equipos (salvo un equipo que vuelve: ya contaba).
            if db.cliente_de_equipo(&id)?.is_none() {
                if let Err(m) = crate::cuotas::cabe_otro_equipo(db, publico, &cliente, db.equipos(&ctx)?.len())? {
                    return Err(format!("cuota:{m}"));
                }
            }
            // El mismo id si está libre en este servidor.
            let e = EquipoNuevo { id: id.clone(), nombre, so, version, box_pub, sign_pub, sal_equipo: sal, secreto_hash: sec_hash };
            match db.cliente_de_equipo(&id)? {
                None => {
                    db.crear_equipo(&ctx, &e)?;
                    db.indexar_equipo(&id, &cliente)?;
                }
                // Vuelve un equipo que ya estuvo aquí (p. ej. este servidor era su respaldo): con las mismas claves, se reactiva.
                Some(otro) if otro.id() == cliente => {
                    let previo = db.equipo(&ctx, &id)?.ok_or("equipo")?;
                    if previo.box_pub != e.box_pub || previo.sign_pub != e.sign_pub {
                        return Err("ocupado".into());
                    }
                    db.reactivar_equipo(&ctx, &id, &e.secreto_hash)?;
                }
                Some(_) => return Err("ocupado".into()),
            }
            db.confirmar_equipo(&ctx, &id, &etiqueta)?;
            if let Some(h) = espera {
                db.poner_espera_equipo(&ctx, &id, h)?;
            }
            db.auditar(
                &ctx,
                &format!("equipo:{id}"),
                "equipo_recibido",
                &id,
                &json!({ "nombre": e.nombre, "so": e.so, "otra_consola": otra_consola }).to_string(),
            )?;
            crate::notificaciones::aviso(
                db,
                &ctx,
                Some(&id),
                "cambio_inusual",
                &if otra_consola {
                    format!("«{}» se conectó también a este servidor: lo sigue gestionando además otra consola.", e.nombre)
                } else {
                    format!("«{}» llegó a este servidor (cambio de servidor o respaldo).", e.nombre)
                },
            )?;
            Ok(Some((cliente, id)))
        })
        .await?
        .map_err(|e| match e.strip_prefix("cuota:") {
            Some(m) => error_cuota(m.to_string()),
            None if e == "ocupado" => ErrorApi::conflicto("Ya hay otro equipo con ese id en este servidor."),
            None => ErrorApi::interno(e),
        })?;
    let (cliente, id) = r.ok_or_else(|| ErrorApi::nuevo(StatusCode::NOT_FOUND, "ficha", "Ficha no válida, caducada o ya gastada.").acceso("ficha"))?;
    st.vivo.avisar(&cliente, Cambio::Equipo(&id));
    st.vivo.avisar(&cliente, Cambio::Avisos(Some(&id)));
    Ok(Json(json!({
        "equipo_id": id,
        "secreto": secreto,
        "cliente_id": cliente,
        "servidor": { "identidad": st.identidad_pub, "nombre": "Resguardo Server" },
    })))
}

/// Lo que no cabe en las cuotas del cliente (v1.34): 403 con el código «cuota».
pub fn error_cuota(mensaje: String) -> ErrorApi {
    ErrorApi::nuevo(StatusCode::FORBIDDEN, "cuota", mensaje)
}

// ---------- Autenticación del agente ----------

/// Un equipo autenticado con `Authorization: Equipo <id>:<secreto>`.
pub struct Agente {
    pub ctx: ClienteCtx,
    pub equipo: String,
    pub sign_pub: String,
}

impl FromRequestParts<St> for Agente {
    type Rejection = ErrorApi;
    async fn from_request_parts(parts: &mut Parts, st: &St) -> Res<Self> {
        let ip = crate::estado::clave_ip(parts.extensions.get::<IpCliente>().and_then(|i| i.0));
        let valor = parts.headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).unwrap_or("");
        let Some((id, secreto)) = valor.strip_prefix("Equipo ").and_then(|v| v.split_once(':')) else {
            return Err(ErrorApi::sin_sesion());
        };
        if !st.limites.intento(&format!("agente-ip:{ip}"), 600, Duration::from_secs(60))
            || st.limites.superado(&format!("agente-fallo:{ip}"), 30, Duration::from_secs(900))
        {
            return Err(ErrorApi::demasiados());
        }
        let (id, hash) = (id.to_string(), auth::hash_ficha(secreto));
        let id2 = id.clone();
        let encontrado = st
            .db(move |db| {
                let Some(ctx) = db.cliente_de_equipo(&id2)? else { return Ok(None) };
                let Some(guardado) = db.secreto_equipo(&ctx, &id2)? else { return Ok(None) };
                let ok: bool = subtle::ConstantTimeEq::ct_eq(guardado.as_bytes(), hash.as_bytes()).into();
                if !ok {
                    return Ok(None);
                }
                db.contacto_equipo(&ctx, &id2, ahora())?;
                let e = db.equipo(&ctx, &id2)?;
                Ok(e.map(|e| (ctx, e.sign_pub)))
            })
            .await?;
        match encontrado {
            Some((ctx, sign_pub)) => Ok(Agente { ctx, equipo: id, sign_pub }),
            None => {
                st.limites.intento(&format!("agente-fallo:{ip}"), 30, Duration::from_secs(900));
                Err(ErrorApi::sin_sesion().acceso("equipo"))
            }
        }
    }
}

fn firma_identidad(st: &St, reto: &str, equipo: &str) -> Res<String> {
    if !b64_32(reto) {
        return Err(ErrorApi::datos("Reto no válido (32 bytes en base64)."));
    }
    Ok(B64.encode(st.identidad.sign(derivaciones::texto_identidad_servidor(reto, equipo).as_bytes()).to_bytes()))
}

/// Manda al agente conectado las órdenes que ya tocan y las cancelaciones.
pub async fn empujar(st: &St, ctx: &ClienteCtx, equipo: &str) {
    if !st.conectado(equipo) {
        return;
    }
    let (ctx2, e2) = (ctx.clone(), equipo.to_string());
    let Ok((ordenes, canceladas)) = st.db(move |db| Ok((db.entregar_ordenes(&ctx2, &e2, ahora())?, db.canceladas_sin_avisar(&ctx2, &e2)?))).await else {
        return;
    };
    for o in &ordenes {
        st.al_agente(equipo, &json!({ "t": "orden", "orden": orden_agente(o) }));
        st.vivo.avisar(ctx.id(), Cambio::Orden { equipo, orden: &o.id, estado: "entregada" });
    }
    for id in canceladas {
        st.al_agente(equipo, &json!({ "t": "cancelada", "orden": id }));
    }
}

// ---------- Mensajes del agente (comunes al WebSocket y al sondeo) ----------

#[derive(Deserialize)]
pub struct Resultado {
    orden: String,
    estado: String,
    mensaje: Option<String>,
    detalle: Option<String>,
    firma: String,
}

async fn registrar_resultado(st: &St, a: &Agente, r: Resultado) -> Res<()> {
    if !matches!(r.estado.as_str(), "en_marcha" | "hecha" | "fallida" | "rechazada") {
        return Err(ErrorApi::datos("Estado no válido."));
    }
    let mensaje = r.mensaje.map(|m| texto_corto(&m, 500));
    if r.detalle.as_ref().is_some_and(|d| d.len() > 512 * 1024) {
        return Err(ErrorApi::datos("Detalle demasiado grande."));
    }
    let (ctx, o) = (a.ctx.clone(), r.orden.clone());
    let orden = st.db(move |db| db.orden(&ctx, &o)).await?.filter(|o| o.equipo_id == a.equipo).ok_or_else(ErrorApi::no_existe)?;
    // La firma del equipo sobre el resultado: la consola puede comprobarla con su clave.
    let texto = derivaciones::texto_resultado(&orden.id, orden.seq, &r.estado, mensaje.as_deref(), r.detalle.as_deref());
    let clave: [u8; 32] = B64.decode(&a.sign_pub).ok().and_then(|v| v.try_into().ok()).ok_or_else(|| ErrorApi::interno("clave del equipo"))?;
    let firma: [u8; 64] = B64.decode(&r.firma).ok().and_then(|v| v.try_into().ok()).ok_or_else(|| ErrorApi::datos("Firma no válida."))?;
    let vk = ed25519_dalek::VerifyingKey::from_bytes(&clave).map_err(ErrorApi::interno)?;
    if vk.verify(texto.as_bytes(), &ed25519_dalek::Signature::from_bytes(&firma)).is_err() {
        return Err(ErrorApi::datos("La firma del resultado no es de este equipo."));
    }
    let (ctx, equipo) = (a.ctx.clone(), a.equipo.clone());
    // v1.36: `quitar_consola` de esta misma consola («Dejar esta consola»): el equipo lo dice en el
    // detalle firmado (sigue gestionado desde otras, pero ya no desde aquí).
    let deja_esta_consola = r.estado == "hecha"
        && orden.tipo == "quitar_consola"
        && r.detalle.as_deref().and_then(|d| serde_json::from_str::<Value>(d).ok()).is_some_and(|d| d["deja_esta_consola"] == true);
    let deja_el_servidor = (r.estado == "hecha" && matches!(orden.tipo.as_str(), "desvincular" | "baja_equipo")) || deja_esta_consola;
    let trasladado = r.estado == "hecha" && orden.tipo == "cambiar_servidor";
    // El alta hecha: el emparejamiento ya no está a medias (se olvida su código).
    let alta_hecha = r.estado == "hecha" && orden.tipo == "alta";
    // La clave de administración cambió (lo confirma el equipo): se avisa a los propietarios.
    let clave_cambiada = r.estado == "hecha" && orden.tipo == "cambiar_clave_admin";
    // v1.30: una destructiva que termina cierra su «Orden destructiva pendiente» (en la próxima pasada: ya).
    let destructiva_terminada = r.estado != "en_marcha" && resguardo_protocolo::ordenes::tipo(&orden.tipo).is_some_and(|t| t.destructiva);
    // `cambiar_espera` hecha: el equipo firma en `detalle` la espera que aplica
    // desde ahora ({"espera_min_horas": h}). Solo así cambia en el servidor.
    let nueva_espera = (r.estado == "hecha" && orden.tipo == "cambiar_espera")
        .then(|| r.detalle.as_deref().and_then(|d| serde_json::from_str::<Value>(d).ok()))
        .flatten()
        .and_then(|d| d["espera_min_horas"].as_i64())
        .filter(|h| (1..=168).contains(h));
    let (orden_id, estado) = (r.orden.clone(), r.estado.clone());
    let res = crate::almacen::ResultadoOrden { orden: r.orden, estado: r.estado, mensaje, detalle: r.detalle, firma: r.firma };
    st.db(move |db| {
        db.resultado_orden(&ctx, &equipo, &res)?;
        if alta_hecha {
            db.alta_hecha(&ctx, &equipo)?;
        }
        if let Some(h) = nueva_espera {
            db.poner_espera_equipo(&ctx, &equipo, h)?;
            db.auditar(&ctx, &format!("equipo:{equipo}"), "espera_confirmada", &equipo, &json!({ "espera_min_horas": h }).to_string())?;
            // La del cliente (la que se usa para los equipos nuevos), cuando todos los gestionados coinciden.
            let gestionados: Vec<i64> =
                db.equipos(&ctx)?.iter().filter(|e| e.confirmado && e.modo == "gestionado").map(|e| e.espera_min_horas.unwrap_or(-1)).collect();
            if !gestionados.is_empty() && gestionados.iter().all(|x| *x == h) {
                db.poner_espera_cliente(ctx.id(), h)?;
            }
        }
        if trasladado {
            db.poner_modo(&ctx, &equipo, "trasladado")?;
            db.auditar(&ctx, &format!("equipo:{equipo}"), "trasladado", &equipo, "{}")?;
        }
        if clave_cambiada {
            db.auditar(&ctx, &format!("equipo:{equipo}"), "clave_admin_cambiada", &equipo, "{}")?;
            crate::notificaciones::aviso(db, &ctx, Some(&equipo), "cambio_clave", "El equipo confirmó el cambio de su clave de administración.")?;
        }
        if deja_el_servidor {
            db.poner_modo(&ctx, &equipo, "local")?;
            db.auditar(&ctx, &format!("equipo:{equipo}"), "deja_el_servidor", &equipo, "{}")?;
        }
        Ok(())
    })
    .await?;
    if destructiva_terminada {
        st.notif.despertar.notify_one();
    }
    st.vivo.avisar(a.ctx.id(), Cambio::Orden { equipo: &a.equipo, orden: &orden_id, estado: &estado });
    if trasladado || deja_el_servidor || nueva_espera.is_some() {
        st.vivo.avisar(a.ctx.id(), Cambio::Equipo(&a.equipo));
    }
    if clave_cambiada {
        st.vivo.avisar(a.ctx.id(), Cambio::Avisos(Some(&a.equipo)));
    }
    Ok(())
}

async fn registrar_informe(st: &St, a: &Agente, datos: Value) -> Res<()> {
    if datos.to_string().len() > 256 * 1024 {
        return Err(ErrorApi::datos("Informe demasiado grande."));
    }
    // Un agente manda un informe cada pocos minutos: más de 120 por hora no es normal.
    if !st.limites.intento(&format!("informe:{}", a.equipo), 120, Duration::from_secs(3600)) {
        return Err(ErrorApi::demasiados());
    }
    // Lo que está en marcha viaja también en el informe (v1.25): si el canal no pasa, llega así.
    let paso = if let Some(t) = datos.get("progreso").filter(|t| t.is_array()) {
        st.progreso.poner(a.ctx.id(), &a.equipo, crate::progreso::limpiar(t).unwrap_or_default())
    } else if datos.get("version").is_some() {
        st.progreso.poner(a.ctx.id(), &a.equipo, Vec::new())
    } else {
        None
    };
    let servicio = datos.get("servicio").and_then(Value::as_str).map(|s| texto_corto(s, 40));
    // El último número de orden que aceptó el equipo: tras restaurar una copia de la
    // consola (o de la base de datos), el servidor recuerda uno anterior y el equipo
    // rechazaría por «repetidas» las órdenes nuevas hasta alcanzarlo. Solo se sube.
    let ultimo_seq = datos.get("ultimo_seq").and_then(Value::as_u64).filter(|n| seq_valido(*n));
    let version = datos.get("version").and_then(Value::as_str).map(|s| texto_corto(s, 40));
    // Copias, verificaciones… que fallan o vuelven a ir bien: para las notificaciones (solo si cambió algo).
    let evento = crate::notificaciones::evento_estado(&st.notif, &a.ctx, &a.equipo, crate::notificaciones::problemas::Fuente::Informe, &datos);
    let hay_evento = evento.is_some();
    let (ctx, equipo) = (a.ctx.clone(), a.equipo.clone());
    st.db(move |db| {
        db.guardar_informe(&ctx, &equipo, &datos)?;
        db.poner_estado_servicio(&ctx, &equipo, servicio.as_deref(), version.as_deref())?;
        if let Some(n) = ultimo_seq {
            db.adelantar_seq(&ctx, &equipo, n + 1)?;
        }
        match &evento {
            Some(e) => crate::notificaciones::apuntar(db, e),
            None => Ok(()),
        }
    })
    .await?;
    if hay_evento {
        st.notif.despertar.notify_one();
    }
    // A las consolas en vivo: primero el informe (las cifras ya están guardadas) y después el progreso.
    st.vivo.avisar(a.ctx.id(), Cambio::Informe(&a.equipo));
    if let Some(p) = paso {
        st.vivo.avisar(a.ctx.id(), Cambio::Progreso(&a.equipo, p));
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct Config {
    seq: u64,
    cifrado: String,
    resumen: Value,
    /// v1.36: la etiqueta que calcula el equipo con la `K_cfg` de este servidor. Tras
    /// cambiar la clave de administración (quizá desde otra consola), la de aquí
    /// queda al día. Solo en un equipo ya confirmado (si no, la pone la consola).
    #[serde(default)]
    etiqueta: Option<String>,
    /// v1.36: la espera mínima que aplica el equipo (quizá la cambió otra consola).
    #[serde(default)]
    espera_min_horas: Option<i64>,
}

async fn registrar_config(st: &St, a: &Agente, c: Config) -> Res<()> {
    if c.cifrado.len() > 2 * 1024 * 1024 || c.resumen.to_string().len() > 64 * 1024 || B64.decode(&c.cifrado).is_err() {
        return Err(ErrorApi::datos("Configuración no válida o demasiado grande."));
    }
    if !st.limites.intento(&format!("config:{}", a.equipo), 120, Duration::from_secs(3600)) {
        return Err(ErrorApi::demasiados());
    }
    let (ctx, equipo) = (a.ctx.clone(), a.equipo.clone());
    let papel = if c.resumen["guarda_copias"]["activo"] == true { "almacenamiento" } else { "agente" };
    // El espejo del almacén (va en el resumen): para las notificaciones, si cambió.
    let evento = crate::notificaciones::evento_estado(&st.notif, &a.ctx, &a.equipo, crate::notificaciones::problemas::Fuente::Resumen, &c.resumen);
    // La clave de administración cambiada desde **otra** consola (docs/consolas-multiples.md §4.2):
    // aquí no llega el resultado de esa orden, pero sí la etiqueta nueva y de dónde vino el cambio.
    let clave_desde_otra = (c.resumen["cambio_config"]["tipo"] == "cambiar_clave_admin"
        && c.resumen["cambio_config"]["consola"]["identidad"].as_str().is_some_and(|i| i != st.identidad_pub))
    .then(|| texto_corto(c.resumen["cambio_config"]["consola"]["nombre"].as_str().unwrap_or("otra consola"), 80));
    let aviso_otra = clave_desde_otra.is_some();
    let pistas = st
        .db(move |db| {
            // v1.30: el almacén aplicó la retención en el repositorio de otro equipo: pista a su dueño.
            let actual = db.equipo(&ctx, &equipo)?;
            let previo = if papel == "almacenamiento" { actual.clone() } else { None };
            db.guardar_config(&ctx, &equipo, c.seq, &c.cifrado, &c.resumen)?;
            db.poner_papel(&ctx, &equipo, papel)?;
            // v1.36: lo que pudo cambiar otra consola (la clave o la espera): el equipo lo dice al subir su configuración.
            if let Some(e) = actual.as_ref().filter(|e| e.confirmado) {
                if let Some(et) = c.etiqueta.as_deref().filter(|et| b64_32(et) && e.etiqueta.as_deref() != Some(*et)) {
                    db.confirmar_equipo(&ctx, &equipo, et)?;
                    db.auditar(&ctx, &format!("equipo:{equipo}"), "etiqueta_equipo", &equipo, "{}")?;
                    if let Some(desde) = &clave_desde_otra {
                        crate::notificaciones::aviso(
                            db,
                            &ctx,
                            Some(&equipo),
                            "cambio_clave",
                            &format!("La clave de administración se cambió desde otra consola («{desde}»): aquí vale ya la nueva."),
                        )?;
                    }
                }
                if let Some(h) = c.espera_min_horas.filter(|h| (1..=168).contains(h) && e.espera_min_horas != Some(*h)) {
                    db.poner_espera_equipo(&ctx, &equipo, h)?;
                    db.auditar(&ctx, &format!("equipo:{equipo}"), "espera_confirmada", &equipo, &json!({ "espera_min_horas": h }).to_string())?;
                }
            }
            if let Some(e) = &evento {
                crate::notificaciones::apuntar(db, e)?;
            }
            let Some(almacen) = previo else { return Ok(Vec::new()) };
            let cambios = crate::pistas::retenciones_nuevas(almacen.resumen.as_ref(), &c.resumen);
            if cambios.is_empty() {
                return Ok(Vec::new());
            }
            Ok(crate::pistas::duenos(&almacen, &cambios, &db.equipos(&ctx)?))
        })
        .await?;
    for (e, repo) in pistas {
        st.al_agente(&e, &json!({ "t": "refrescar", "repo": repo }));
    }
    st.vivo.avisar(a.ctx.id(), Cambio::Config(&a.equipo));
    if aviso_otra {
        st.vivo.avisar(a.ctx.id(), Cambio::Avisos(Some(&a.equipo)));
    }
    Ok(())
}

const TIPOS_AVISO: &[&str] = &["intentos_fallidos", "bloqueo", "copia_fallida", "copia_atrasada", "servicio_detenido", "cambio_inusual"];

#[derive(Deserialize)]
pub struct Aviso {
    tipo: String,
    mensaje: String,
}

async fn registrar_aviso(st: &St, a: &Agente, av: Aviso) -> Res<()> {
    if !TIPOS_AVISO.contains(&av.tipo.as_str()) {
        return Err(ErrorApi::datos("Tipo de aviso desconocido."));
    }
    if !st.limites.intento(&format!("aviso:{}", a.equipo), 60, Duration::from_secs(3600)) {
        return Err(ErrorApi::demasiados());
    }
    let (ctx, equipo, mensaje) = (a.ctx.clone(), a.equipo.clone(), texto_corto(&av.mensaje, 500));
    st.db(move |db| crate::notificaciones::aviso(db, &ctx, Some(&equipo), &av.tipo, &mensaje)).await?;
    st.notif.despertar.notify_one();
    st.vivo.avisar(a.ctx.id(), Cambio::Avisos(Some(&a.equipo)));
    Ok(())
}

// ---------- Historial del equipo (v1.23) ----------

/// Entradas por petición y tamaño de cada una (en JSON).
pub const MAX_ENTRADAS_HISTORIAL: usize = 500;
const MAX_ENTRADA_HISTORIAL: usize = 4 * 1024;
/// v1.4x: una vuelta de la retención lleva las versiones que quitó (como mucho 2000; el agente la recorta a 96 KiB).
pub const MAX_ENTRADA_RETENCION: usize = 96 * 1024;
/// v1.4x: `historial` (se trajo el historial de otro repositorio; con `mover`, un paso de «Mover a otro sitio…»).
pub const TIPOS_HISTORIAL: &[&str] = &["copia", "resumen_dia", "verificacion", "prueba_restauracion", "externa", "espejo", "aviso", "retencion", "historial"];
/// Los que solo se dan si se piden con `tipo` (v1.4x): una consola anterior no los conoce
/// y son grandes. Sin `tipo`, el historial es el de siempre.
pub const TIPOS_SOLO_PEDIDOS: &[&str] = &["retencion"];
/// Vueltas de la retención con la lista de versiones por equipo; las anteriores, solo con sus cifras.
pub const RETENCIONES_CON_DETALLE: i64 = 50;
/// 2000-01-01: nada de antes (ni de más de un día en el futuro, por los relojes).
const HISTORIAL_DESDE: crate::almacen::Ts = 946_684_800;
/// Avisos que un equipo puede pasar a la lista de avisos con su historial, por día
/// (los de antes de vincularse, al subirlo entero; después, unos pocos).
pub const MAX_AVISOS_HISTORIAL_DIA: u32 = 1000;

#[derive(Deserialize)]
pub struct Historial {
    entradas: Vec<Value>,
}

/// Una entrada del historial que manda el equipo, comprobada: id corto, hora
/// razonable (desde el año 2000 y hasta un día en el futuro, por los relojes),
/// un tipo conocido y como mucho 4 KiB. Los textos los escribe el equipo sin rutas.
pub(crate) fn entrada_historial(v: &Value, ahora: crate::almacen::Ts) -> Option<crate::almacen::EntradaHistorial> {
    let id = v.get("id")?.as_str()?;
    if id.is_empty() || id.len() > 64 || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    let hora = crate::api::de_fecha(v.get("hora")?.as_str()?)?;
    if hora < HISTORIAL_DESDE || hora > ahora + 86_400 {
        return None;
    }
    let tipo = v.get("tipo")?.as_str()?;
    if !TIPOS_HISTORIAL.contains(&tipo) {
        return None;
    }
    let datos = v.to_string();
    if datos.len() > if tipo == "retencion" { MAX_ENTRADA_RETENCION } else { MAX_ENTRADA_HISTORIAL } {
        return None;
    }
    let aviso = if tipo == "aviso" {
        let t = v.get("aviso")?.as_str()?;
        if !TIPOS_AVISO.contains(&t) {
            return None;
        }
        Some((t.to_string(), texto_corto(v.get("mensaje").and_then(Value::as_str).unwrap_or(""), 500)))
    } else {
        None
    };
    Some(crate::almacen::EntradaHistorial { id: id.to_string(), hora, tipo: tipo.to_string(), datos, aviso })
}

/// Guarda lo que el equipo cuenta de su historial (idempotente: cada entrada, una vez).
/// Las entradas que no valen se ignoran (las demás entran).
async fn registrar_historial(st: &St, a: &Agente, h: Historial) -> Res<Value> {
    if h.entradas.len() > MAX_ENTRADAS_HISTORIAL {
        return Err(ErrorApi::datos(format!("Como mucho {MAX_ENTRADAS_HISTORIAL} entradas por vez.")));
    }
    // Lo normal: una subida al vincularse (de unos cientos) y unas pocas al día.
    if !st.limites.intento(&format!("historial:{}", a.equipo), 120, Duration::from_secs(3600)) {
        return Err(ErrorApi::demasiados());
    }
    let ahora = ahora();
    let mut entradas: Vec<_> = h.entradas.iter().filter_map(|v| entrada_historial(v, ahora)).collect();
    // Los avisos del historial pasan a la lista de avisos del cliente (ya vistos):
    // con un tope propio, para que un equipo no la llene saltándose el de `aviso`.
    for e in entradas.iter_mut().filter(|e| e.aviso.is_some()) {
        if !st.limites.intento(&format!("aviso-historial:{}", a.equipo), MAX_AVISOS_HISTORIAL_DIA, Duration::from_secs(86_400)) {
            e.aviso = None;
        }
    }
    let con_avisos = entradas.iter().any(|e| e.aviso.is_some());
    let (ctx, equipo, publico) = (a.ctx.clone(), a.equipo.clone(), st.opciones.publico);
    let (nuevas, ultima) = st
        .db(move |db| {
            let tope = crate::cuotas::tope_historial(db, publico, ctx.id())?;
            let nuevas = if entradas.is_empty() { 0 } else { db.guardar_historial_tope(&ctx, &equipo, &entradas, tope)? };
            Ok((nuevas, db.ultima_historial(&ctx, &equipo)?))
        })
        .await?;
    if nuevas > 0 {
        st.vivo.avisar(a.ctx.id(), Cambio::Historial(&a.equipo));
        if con_avisos {
            st.vivo.avisar(a.ctx.id(), Cambio::Avisos(Some(&a.equipo)));
        }
    }
    Ok(json!({ "nuevas": nuevas, "ultima": crate::api::fecha_opt(ultima) }))
}

async fn historial_http(State(st): State<St>, a: Agente, Json(h): Json<Historial>) -> Res<Json<Value>> {
    Ok(Json(registrar_historial(&st, &a, h).await?))
}

/// Progreso en vivo (v1.25): solo en memoria, para las consolas. Llega cada
/// pocos segundos mientras algo está en marcha.
fn registrar_progreso(st: &St, a: &Agente, tareas: &Value) -> Res<()> {
    if !st.limites.intento(&format!("progreso:{}", a.equipo), 60, Duration::from_secs(60)) {
        return Err(ErrorApi::demasiados());
    }
    if let Some(p) = st.progreso.poner(a.ctx.id(), &a.equipo, crate::progreso::limpiar(tareas)?) {
        st.vivo.avisar(a.ctx.id(), Cambio::Progreso(&a.equipo, p));
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct Progreso {
    tareas: Value,
}

async fn progreso_http(State(st): State<St>, a: Agente, Json(p): Json<Progreso>) -> Res<StatusCode> {
    registrar_progreso(&st, &a, &p.tareas)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn mensaje_de_agente(st: &St, a: &Agente, sesion: &str, cifrado: &str) -> Res<i64> {
    if cifrado.is_empty() || cifrado.len() > crate::api::MAX_MENSAJE_SESION {
        return Err(ErrorApi::datos("Mensaje vacío o demasiado grande."));
    }
    let (ctx, s, equipo, c) = (a.ctx.clone(), sesion.to_string(), a.equipo.clone(), cifrado.to_string());
    let n = st
        .db(move |db| {
            let Some(ses) = db.sesion_interactiva(&ctx, &s)? else { return Ok(None) };
            if ses.equipo_id != equipo {
                return Ok(None);
            }
            Ok(Some(db.mensaje_sesion(&ctx, &s, "equipo", &c, ahora() + 600)?))
        })
        .await?
        .ok_or_else(ErrorApi::no_existe)?;
    st.cambios.notify_waiters();
    Ok(n)
}

// ---------- Sondeo ----------

#[derive(Deserialize)]
pub struct Tomar {
    reto: String,
    /// v1.35 (agente > 0.7.15): el último número de orden que aceptó el equipo de
    /// este servidor, como en el informe (ver `adelantar_seq_de`).
    #[serde(default)]
    ultimo_seq: Option<u64>,
}

async fn tomar(State(st): State<St>, a: Agente, Json(p): Json<Tomar>) -> Res<Json<Value>> {
    let firma = firma_identidad(&st, &p.reto, &a.equipo)?;
    adelantar_seq_de(&st, &a, p.ultimo_seq).await?;
    let (ctx, equipo) = (a.ctx.clone(), a.equipo.clone());
    let (ordenes, canceladas, atencion, sesiones, ultima) = st
        .db(move |db| {
            let ahora = ahora();
            let ordenes = db.entregar_ordenes(&ctx, &equipo, ahora)?;
            let canceladas = db.canceladas_sin_avisar(&ctx, &equipo)?;
            let sesiones = db.sesiones_abiertas(&ctx, &equipo, ahora)?;
            let atencion = db.equipo(&ctx, &equipo)?.and_then(|e| e.atencion_hasta).is_some_and(|t| t > ahora) || !sesiones.is_empty();
            let ultima = db.ultima_historial(&ctx, &equipo)?;
            Ok((ordenes, canceladas, atencion, sesiones, ultima))
        })
        .await?;
    for o in &ordenes {
        st.vivo.avisar(a.ctx.id(), Cambio::Orden { equipo: &a.equipo, orden: &o.id, estado: "entregada" });
    }
    Ok(Json(json!({
        "firma": firma,
        "ordenes": ordenes.iter().map(orden_agente).collect::<Vec<_>>(),
        "canceladas": canceladas,
        "atencion": atencion,
        "sesiones": sesiones,
        "historial": { "ultima": crate::api::fecha_opt(ultima) },
    })))
}

async fn resultado_http(State(st): State<St>, a: Agente, Json(r): Json<Resultado>) -> Res<StatusCode> {
    registrar_resultado(&st, &a, r).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct Informe {
    datos: Value,
}

async fn informe_http(State(st): State<St>, a: Agente, Json(i): Json<Informe>) -> Res<StatusCode> {
    registrar_informe(&st, &a, i.datos).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn config_http(State(st): State<St>, a: Agente, Json(c): Json<Config>) -> Res<StatusCode> {
    registrar_config(&st, &a, c).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn aviso_http(State(st): State<St>, a: Agente, Json(av): Json<Aviso>) -> Res<StatusCode> {
    registrar_aviso(&st, &a, av).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct Cifrado {
    cifrado: String,
}

async fn escribir_sesion(State(st): State<St>, a: Agente, Path(s): Path<String>, Json(m): Json<Cifrado>) -> Res<Json<Value>> {
    let n = mensaje_de_agente(&st, &a, &s, &m.cifrado).await?;
    Ok(Json(json!({ "n": n })))
}

#[derive(Deserialize)]
pub struct Desde {
    desde: Option<i64>,
}

async fn leer_sesion(State(st): State<St>, a: Agente, Path(s): Path<String>, Query(q): Query<Desde>) -> Res<Json<Value>> {
    let (ctx, s2, equipo) = (a.ctx.clone(), s.clone(), a.equipo.clone());
    let suya = st.db(move |db| Ok(db.sesion_interactiva(&ctx, &s2)?.is_some_and(|x| x.equipo_id == equipo))).await?;
    if !suya {
        return Err(ErrorApi::no_existe());
    }
    let msgs = crate::api::esperar_mensajes(&st, a.ctx.clone(), s, "equipo", q.desde.unwrap_or(0)).await?.ok_or_else(ErrorApi::no_existe)?;
    Ok(Json(json!(msgs)))
}

// ---------- Relé ----------

fn uso_relevos(dir: &std::path::Path) -> u64 {
    fn suma(d: &std::path::Path) -> u64 {
        std::fs::read_dir(d)
            .map(|it| {
                it.flatten()
                    .map(|e| match e.file_type() {
                        Ok(t) if t.is_dir() => suma(&e.path()),
                        Ok(_) => e.metadata().map(|m| m.len()).unwrap_or(0),
                        Err(_) => 0,
                    })
                    .sum()
            })
            .unwrap_or(0)
    }
    suma(dir)
}

async fn subir_trozo(State(st): State<St>, a: Agente, Path((r, n)): Path<(String, u64)>, cuerpo: Bytes) -> Res<StatusCode> {
    if cuerpo.is_empty() || cuerpo.len() > MAX_TROZO {
        return Err(ErrorApi::datos("Trozo vacío o demasiado grande."));
    }
    let (ctx, r2) = (a.ctx.clone(), r.clone());
    let rel = st.db(move |db| db.relevo(&ctx, &r2)).await?.filter(|x| x.equipo_id == a.equipo).ok_or_else(ErrorApi::no_existe)?;
    if rel.estado != "subiendo" || rel.caduca <= ahora() {
        return Err(ErrorApi::conflicto("Ese relé ya no admite trozos."));
    }
    if n != rel.trozos {
        return Err(ErrorApi::conflicto("Los trozos van en orden.").con(json!({ "siguiente": rel.trozos })));
    }
    let bytes = rel.bytes + cuerpo.len() as u64;
    if bytes > rel.max_bytes || uso_relevos(&st.datos.join("relevos")) + cuerpo.len() as u64 > st.opciones.total_relevo {
        return Err(ErrorApi::datos("La descarga supera el tamaño permitido."));
    }
    // v1.34: lo que el cliente puede pasar por el relé este mes.
    let (cliente, publico, largo, mes) = (a.ctx.id().to_string(), st.opciones.publico, cuerpo.len() as u64, crate::cuotas::mes(ahora()));
    let (c2, m2) = (cliente.clone(), mes.clone());
    if !st.db(move |db| crate::cuotas::cabe_en_relevo(db, publico, &c2, &m2, largo)).await? {
        return Err(error_cuota("Este cliente ya usó este mes todo el relé de descargas que le permite el servidor. Restaura en el propio equipo o pide más a quien administra el servidor.".into()));
    }
    let dir = crate::api::dir_relevo(&st, a.ctx.id(), &r).ok_or_else(ErrorApi::no_existe)?;
    tokio::fs::create_dir_all(&dir).await.map_err(ErrorApi::interno)?;
    tokio::fs::write(dir.join(n.to_string()), &cuerpo).await.map_err(ErrorApi::interno)?;
    let ctx = a.ctx.clone();
    st.db(move |db| {
        db.actualizar_relevo(&ctx, &r, n + 1, bytes, "subiendo", rel.caduca)?;
        crate::cuotas::sumar_relevo(db, &cliente, &mes, largo).map(|_| ())
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct Fin {
    trozos: u64,
    bytes: u64,
}

async fn fin_relevo(State(st): State<St>, a: Agente, Path(r): Path<String>, Json(f): Json<Fin>) -> Res<StatusCode> {
    let (ctx, r2) = (a.ctx.clone(), r.clone());
    let rel = st.db(move |db| db.relevo(&ctx, &r2)).await?.filter(|x| x.equipo_id == a.equipo).ok_or_else(ErrorApi::no_existe)?;
    if rel.trozos != f.trozos || rel.bytes != f.bytes {
        return Err(ErrorApi::conflicto("Faltan trozos por subir."));
    }
    let ctx = a.ctx.clone();
    st.db(move |db| db.actualizar_relevo(&ctx, &r, rel.trozos, rel.bytes, "listo", ahora() + 3600)).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------- WebSocket ----------

/// Cada cuánto se le manda `ping` al equipo (contesta `pong`).
pub const LATIDO_AGENTE: Duration = Duration::from_secs(30);
/// Sin nada del equipo en este tiempo, el canal se da por muerto (y el equipo, sin
/// conexión). Holgado: el agente puede tardar en leer mientras prepara un informe.
pub const SIN_RESPUESTA_AGENTE: Duration = Duration::from_secs(150);
/// Plazo para mandarle un mensaje (un equipo que no lee no retiene la tarea).
const PLAZO_ENVIO_AGENTE: Duration = Duration::from_secs(30);

#[derive(Deserialize)]
pub struct Reto {
    reto: String,
    /// v1.35 (agente > 0.7.15): el último número de orden que aceptó el equipo de este servidor.
    #[serde(default)]
    ultimo_seq: Option<u64>,
}

fn seq_valido(n: u64) -> bool {
    n < i64::MAX as u64
}

/// Sube `siguiente_seq` hasta después del último número de orden que aceptó el
/// equipo (nunca lo baja). Al abrir el canal y en cada sondeo, **antes** de que
/// el equipo cuente como conectado: tras restaurar la copia de la consola, el
/// servidor recuerda un número anterior y la primera orden que firmara la
/// consola nada más verlo conectado saldría «repetida» (el informe, que también
/// lo lleva, llega un momento después).
async fn adelantar_seq_de(st: &St, a: &Agente, ultimo_seq: Option<u64>) -> Res<()> {
    let Some(n) = ultimo_seq.filter(|n| *n > 0 && seq_valido(*n)) else { return Ok(()) };
    let (ctx, equipo) = (a.ctx.clone(), a.equipo.clone());
    // Y lo que se le entregó por un canal que ya estaba muerto (red caída sin aviso) y
    // nunca le llegó: se le vuelve a entregar ahora. Antes se quedaba «entregada» para
    // siempre (prueba de resistencia, docs/estabilidad.md).
    st.db(move |db| {
        db.adelantar_seq(&ctx, &equipo, n + 1)?;
        db.reponer_no_recibidas(&ctx, &equipo, n, ahora())?;
        Ok(())
    })
    .await
}

async fn canal(State(st): State<St>, a: Agente, Query(q): Query<Reto>, ws: WebSocketUpgrade) -> Res<Response> {
    let firma = firma_identidad(&st, &q.reto, &a.equipo)?;
    adelantar_seq_de(&st, &a, q.ultimo_seq).await?;
    Ok(ws.max_message_size(1024 * 1024).max_frame_size(1024 * 1024).on_upgrade(move |socket| atender(st, a, firma, socket)))
}

async fn atender(st: St, a: Agente, firma: String, socket: WebSocket) {
    let (mut envio, mut recepcion) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    // Uno por equipo: una conexión nueva sustituye a la anterior.
    st.conectados.lock().unwrap_or_else(|e| e.into_inner()).insert(a.equipo.clone(), tx.clone());
    st.vivo.avisar(a.ctx.id(), Cambio::Equipo(&a.equipo));
    let (ctx, e2) = (a.ctx.clone(), a.equipo.clone());
    let (atencion, ultima) = st
        .db(move |db| {
            let ahora = ahora();
            let atencion =
                db.equipo(&ctx, &e2)?.and_then(|e| e.atencion_hasta).is_some_and(|t| t > ahora) || !db.sesiones_abiertas(&ctx, &e2, ahora)?.is_empty();
            Ok((atencion, db.ultima_historial(&ctx, &e2)?))
        })
        .await
        .unwrap_or((false, None));
    // v1.23: hasta dónde tiene el historial del equipo (el agente sube lo que falte).
    let _ = tx.send(json!({ "t": "hola", "firma": firma, "atencion": atencion, "historial": { "ultima": crate::api::fecha_opt(ultima) } }).to_string());
    // Lo que estaba esperando (y lo que se entregó y quizá no llegó).
    empujar(&st, &a.ctx, &a.equipo).await;

    let mut ping = tokio::time::interval(LATIDO_AGENTE);
    ping.tick().await;
    // Lo último que llegó del equipo (su «pong», un informe…). Sin esto, con la red caída
    // sin aviso (sin RST: un cable quitado, un portátil que se duerme, un router que
    // descarta) la conexión seguía «abierta» para siempre: la consola veía el equipo
    // conectado, con su «último contacto» al día, y la tarea se quedaba (prueba de
    // resistencia, docs/estabilidad.md).
    let mut visto = tokio::time::Instant::now();
    loop {
        tokio::select! {
            salida = rx.recv() => {
                let Some(texto) = salida else { break };
                if !matches!(tokio::time::timeout(PLAZO_ENVIO_AGENTE, envio.send(Message::Text(texto.into()))).await, Ok(Ok(()))) {
                    break;
                }
            }
            entrada = recepcion.next() => {
                match entrada {
                    Some(Ok(Message::Text(t))) => {
                        visto = tokio::time::Instant::now();
                        if let Err(e) = procesar(&st, &a, &t).await {
                            let _ = tx.send(json!({ "t": "error", "error": e.codigo, "mensaje": e.mensaje }).to_string());
                        }
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(_)) => visto = tokio::time::Instant::now(),
                }
            }
            _ = ping.tick() => {
                if visto.elapsed() > SIN_RESPUESTA_AGENTE {
                    break;
                }
                if tx.send(json!({ "t": "ping" }).to_string()).is_err() {
                    break;
                }
                // El último contacto, solo si de verdad contesta.
                if visto.elapsed() <= LATIDO_AGENTE * 2 {
                    let (ctx, e) = (a.ctx.clone(), a.equipo.clone());
                    let _ = st.db(move |db| db.contacto_equipo(&ctx, &e, ahora())).await;
                }
            }
        }
    }
    let quitado = {
        let mut mapa = st.conectados.lock().unwrap_or_else(|e| e.into_inner());
        mapa.get(&a.equipo).is_some_and(|t| t.same_channel(&tx)) && mapa.remove(&a.equipo).is_some()
    };
    if quitado {
        st.vivo.avisar(a.ctx.id(), Cambio::Equipo(&a.equipo));
    }
}

async fn procesar(st: &St, a: &Agente, texto: &str) -> Res<()> {
    let v: Value = serde_json::from_str(texto).map_err(|_| ErrorApi::datos("Mensaje no válido."))?;
    let campo = |k: &str| v.get(k).cloned().unwrap_or(Value::Null);
    match v.get("t").and_then(Value::as_str).unwrap_or("") {
        "pong" => Ok(()),
        "resultado" => registrar_resultado(st, a, serde_json::from_value(v.clone()).map_err(|_| ErrorApi::datos("Resultado no válido."))?).await,
        "informe" => registrar_informe(st, a, campo("datos")).await,
        "progreso" => registrar_progreso(st, a, &campo("tareas")),
        "config" => registrar_config(st, a, serde_json::from_value(v.clone()).map_err(|_| ErrorApi::datos("Configuración no válida."))?).await,
        "aviso" => registrar_aviso(st, a, serde_json::from_value(v.clone()).map_err(|_| ErrorApi::datos("Aviso no válido."))?).await,
        "sesion" => {
            let s = v.get("sesion").and_then(Value::as_str).unwrap_or("");
            let c = v.get("cifrado").and_then(Value::as_str).unwrap_or("");
            mensaje_de_agente(st, a, s, c).await.map(|_| ())
        }
        _ => Err(ErrorApi::datos("Tipo de mensaje desconocido.")),
    }
}
