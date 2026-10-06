//! Servidor, primer arranque, sesiones con TOTP, cuenta e invitaciones.

use crate::almacen::{ahora, Rol};
use crate::auth::{self, Pendiente, Usuario};
use crate::error::{ErrorApi, Res};
use crate::estado::{IpCliente, St};
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

const VENTANA: Duration = Duration::from_secs(15 * 60);
const MAX_INTENTOS: u32 = 10;

fn con_cookie(cookie: String, cuerpo: Value) -> Response {
    ([(header::SET_COOKIE, cookie)], Json(cuerpo)).into_response()
}

/// La IP para los límites y la auditoría (IPv6 por su /64, ver `clave_ip`).
fn ip_de(ip: Option<Extension<IpCliente>>) -> String {
    crate::estado::clave_ip(ip.and_then(|Extension(IpCliente(i))| i))
}

/// Lo más largo que puede ser un correo (RFC 5321) y una contraseña al entrar:
/// lo demás ni se busca ni se anota en los límites.
const MAX_CORREO: usize = 254;
const MAX_CONTRASENA: usize = 1024;

/// App key pública de la app «Resguardo» de Dropbox (permiso «App folder»,
/// OAuth con PKCE, sin secreto: no es un secreto).
pub const DROPBOX_APP_KEY: &str = "beobf3c13cvlrup";

/// La app key que da `GET /api/servidor`: la de la app «Resguardo» o la de
/// `RESGUARDO_DROPBOX_APP_KEY` al compilar (vacía = sin Dropbox: la consola
/// dice que falta configurarla).
pub fn dropbox_app_key() -> Option<&'static str> {
    Some(option_env!("RESGUARDO_DROPBOX_APP_KEY").unwrap_or(DROPBOX_APP_KEY).trim()).filter(|k| !k.is_empty())
}

pub async fn servidor(State(st): State<St>) -> Res<Json<Value>> {
    let cuentas = st.db(|db| db.contar_cuentas()).await?;
    let mut v = json!({
        "version": env!("CARGO_PKG_VERSION"),
        "nombre": "Resguardo Server",
        "identidad": st.identidad_pub,
        "huella_ca": st.huella_ca,
        "inicializado": cuentas > 0,
        // v1.17: ¿puede dar el instalador del agente «listo para vincular»?
        "instalador_agente": st.opciones.instalador_agente.as_ref().is_some_and(|p| p.is_file()),
        // v1.19: ¿hay un Resguardo Agente en esta misma máquina? («Vincular este servidor»).
        "agente_local": super::instaladores::agente_local_instalado(),
        // v1.34: la dirección para los agentes y las otras consolas, si no es la de
        // la consola (consola en internet: https://agentes.<dominio>); si no, null.
        "url_agentes": st.opciones.url_agentes,
        // v1.34: consola en internet (cuotas más estrictas por defecto).
        "publico": st.opciones.publico,
        // v1.39: canal en vivo de la consola (`GET /api/clientes/{c}/vivo`).
        "vivo": true,
        // v1.4x: acepta códigos de emparejamiento generados en el navegador (`codigo_hash`) y
        // da el instalador genérico (`GET /api/clientes/{c}/instalador-agente`).
        "codigo_navegador": true,
    });
    if let Some(k) = dropbox_app_key() {
        v["dropbox_app_key"] = json!(k);
    }
    Ok(Json(v))
}

/// La autoridad TLS propia (pública): el agente la fija al vincularse. Su
/// autenticidad la garantiza el SAS v2 (que incluye la identidad del servidor).
pub async fn servidor_ca(State(st): State<St>) -> Res<Response> {
    let pem = tokio::fs::read_to_string(st.datos.join("tls").join("ca.crt")).await.map_err(|_| ErrorApi::no_existe())?;
    Ok(([(header::CONTENT_TYPE, "application/x-pem-file")], pem).into_response())
}

/// Código de primer arranque (se muestra en el registro del servidor mientras no haya cuentas).
pub fn preparar_codigo_arranque(st: &St) -> Result<Option<String>, String> {
    if st.db.contar_cuentas()? > 0 {
        return Ok(None);
    }
    let codigo = resguardo_protocolo::mensajes::pairing_code();
    st.db.poner_valor("codigo_arranque", &auth::hash_ficha(&resguardo_protocolo::mensajes::code_hash(&codigo)))?;
    Ok(Some(codigo))
}

#[derive(Deserialize)]
pub struct Inicio {
    codigo_arranque: String,
    correo: String,
    nombre: String,
    contrasena: String,
}

pub async fn inicio(State(st): State<St>, ip: Option<Extension<IpCliente>>, Json(p): Json<Inicio>) -> Res<Response> {
    // Por IP y en total: el código no se puede probar desde muchas IP a la vez.
    if !st.limites.intento(&format!("inicio:{}", ip_de(ip)), MAX_INTENTOS, VENTANA) || !st.limites.intento("inicio:*", 50, std::time::Duration::from_secs(3600))
    {
        return Err(ErrorApi::demasiados());
    }
    let correo = auth::valida_correo(&p.correo)?;
    auth::valida_contrasena(&p.contrasena)?;
    let nombre = p.nombre.trim().to_string();
    if nombre.is_empty() || nombre.chars().count() > 80 {
        return Err(ErrorApi::datos("Escribe tu nombre (hasta 80 caracteres)."));
    }
    let esperado = st.db(|db| db.valor("codigo_arranque")).await?;
    let dado = auth::hash_ficha(&resguardo_protocolo::mensajes::code_hash(&p.codigo_arranque));
    let ok = esperado.as_deref().is_some_and(|e| subtle::ConstantTimeEq::ct_eq(e.as_bytes(), dado.as_bytes()).into());
    if st.db(|db| db.contar_cuentas()).await? > 0 {
        return Err(ErrorApi::nuevo(StatusCode::CONFLICT, "inicializado", "Este servidor ya tiene cuentas: entra con la tuya."));
    }
    if !ok {
        return Err(ErrorApi::datos("El código de primer arranque no es correcto (está en el registro del servidor).").acceso("primer_arranque"));
    }
    let hash = auth::hash_contrasena(&p.contrasena).map_err(ErrorApi::interno)?;
    let secreto = auth::nuevo_secreto_totp();
    let (c2, s2) = (correo.clone(), secreto.clone());
    let cuenta = st
        .db(move |db| {
            // Otra vez dentro (las llamadas a la base van de una en una): dos
            // primeros arranques a la vez no crean dos cuentas.
            if db.contar_cuentas()? > 0 || db.valor("codigo_arranque")?.is_none_or(|v| v.is_empty()) {
                return Err("Este servidor ya tiene cuentas.".to_string());
            }
            let c = db.crear_cuenta(&c2, &nombre, &hash, true)?;
            db.poner_totp(&c.id, Some(&s2), false)?;
            db.poner_valor("codigo_arranque", "")?;
            db.auditar_servidor(&format!("cuenta:{c2}"), "inicio", &c.id, "{}")?;
            Ok(c)
        })
        .await?;
    let ficha = auth::ficha();
    let th = auth::hash_ficha(&ficha);
    let id = cuenta.id.clone();
    st.db(move |db| db.crear_sesion(&th, &id, 1, ahora() + auth::SESION_PENDIENTE_MIN * 60)).await?;
    // El código de primer arranque ya no sirve: fuera su archivo (el del servicio de Windows).
    let _ = std::fs::remove_file(st.datos.join("codigo-arranque.txt"));
    Ok(con_cookie(
        auth::cookie_sesion(&st, &ficha, auth::SESION_HORAS * 3600),
        json!({ "totp": { "secreto": secreto, "uri": auth::uri_totp(&correo, &secreto) } }),
    ))
}

#[derive(Deserialize)]
pub struct Entrar {
    correo: String,
    contrasena: String,
}

pub async fn entrar(State(st): State<St>, ip: Option<Extension<IpCliente>>, Json(p): Json<Entrar>) -> Res<Response> {
    let correo = p.correo.trim().to_lowercase();
    let ip = ip_de(ip);
    if !st.limites.intento(&format!("entrar-ip:{ip}"), MAX_INTENTOS * 3, VENTANA) {
        return Err(ErrorApi::demasiados());
    }
    if correo.len() > MAX_CORREO || p.contrasena.len() > MAX_CONTRASENA {
        return Err(ErrorApi::nuevo(StatusCode::UNAUTHORIZED, "credenciales", "Correo o contraseña incorrectos."));
    }
    if !st.limites.intento(&format!("entrar:{correo}"), MAX_INTENTOS, VENTANA) {
        return Err(ErrorApi::demasiados());
    }
    let c2 = correo.clone();
    let cuenta = st.db(move |db| db.cuenta_por_correo(&c2)).await?;
    let contrasena = p.contrasena;
    let valida = match &cuenta {
        Some(c) => {
            let h = c.hash.clone();
            tokio::task::spawn_blocking(move || auth::comprueba_contrasena(&contrasena, &h)).await.unwrap_or(false)
        }
        None => {
            // Mismo trabajo que con una cuenta real: no revela qué correos existen.
            tokio::task::spawn_blocking(move || auth::comprueba_contrasena(&contrasena, auth::hash_senuelo())).await.unwrap_or(false);
            false
        }
    };
    let Some(cuenta) = cuenta.filter(|_| valida) else {
        let _ = st.db(move |db| db.auditar_servidor(&format!("ip:{ip}"), "entrar_fallido", &correo, "{}")).await;
        return Err(ErrorApi::nuevo(StatusCode::UNAUTHORIZED, "credenciales", "Correo o contraseña incorrectos.").acceso("entrar"));
    };
    let ficha = auth::ficha();
    let th = auth::hash_ficha(&ficha);
    let id = cuenta.id.clone();
    st.db(move |db| db.crear_sesion(&th, &id, 1, ahora() + auth::SESION_PENDIENTE_MIN * 60)).await?;
    let cookie = auth::cookie_sesion(&st, &ficha, auth::SESION_HORAS * 3600);
    // v1.27: el propietario le restableció la verificación en dos pasos: sin
    // el código que le dio, la contraseña sola no da de alta otro autenticador.
    if let Some(r) = restablecimiento(&st, &cuenta.id).await? {
        return Ok(con_cookie(cookie, json!({ "necesita": "restablecimiento", "restablecida": r.aviso(), "caducado": r.caduca <= ahora() })));
    }
    if cuenta.totp_activo {
        return Ok(con_cookie(cookie, json!({ "necesita": "totp" })));
    }
    // Sin TOTP todavía: se da de alta en este paso (obligatorio). Si ya hay
    // uno pendiente, el mismo: entrar otra vez con la contraseña no cambia
    // el autenticador que se está dando de alta por otro.
    let secreto = match cuenta.totp_secreto.clone().filter(|s| !s.is_empty()) {
        Some(s) => s,
        None => {
            let s = auth::nuevo_secreto_totp();
            let (id, s2) = (cuenta.id.clone(), s.clone());
            st.db(move |db| db.poner_totp(&id, Some(&s2), false)).await?;
            s
        }
    };
    Ok(con_cookie(cookie, json!({ "necesita": "alta_totp", "totp": { "secreto": secreto, "uri": auth::uri_totp(&cuenta.correo, &secreto) } })))
}

#[derive(Deserialize)]
pub struct Totp {
    codigo: Option<String>,
    recuperacion: Option<String>,
}

pub async fn totp(State(st): State<St>, Pendiente(s): Pendiente, Json(p): Json<Totp>) -> Res<Response> {
    if !st.limites.intento(&format!("totp:{}", s.cuenta.id), MAX_INTENTOS, VENTANA) {
        return Err(ErrorApi::demasiados());
    }
    let restablecida = if s.cuenta.totp_activo { None } else { restablecimiento(&st, &s.cuenta.id).await? };
    if restablecida.as_ref().is_some_and(|r| r.sesion.as_deref() != Some(s.token_hash.as_str())) {
        return Err(ErrorApi::nuevo(StatusCode::FORBIDDEN, "restablecimiento", "Falta el código que te dio el propietario: vuelve a entrar."));
    }
    let cuenta = s.cuenta;
    let secreto = cuenta.totp_secreto.clone().ok_or_else(|| ErrorApi::datos("Esta cuenta no tiene autenticador: vuelve a entrar."))?;
    let mut paso = None;
    let mut por_recuperacion = false;
    if let Some(codigo) = p.codigo.as_deref() {
        paso = auth::comprueba_totp(&secreto, codigo, cuenta.totp_ultimo, ahora());
    } else if let (Some(rec), true) = (p.recuperacion.as_deref(), cuenta.totp_activo) {
        let (id, h) = (cuenta.id.clone(), auth::hash_ficha(rec.trim().to_lowercase().as_str()));
        por_recuperacion = st.db(move |db| db.usar_codigo_recuperacion(&id, &h)).await?;
    }
    if paso.is_none() && !por_recuperacion {
        return Err(ErrorApi::nuevo(StatusCode::UNAUTHORIZED, "codigo", "El código no es correcto.").acceso("totp"));
    }
    // El paso se gasta antes de nada: el mismo código a la vez en dos peticiones solo vale una.
    if let Some(p) = paso {
        let id = cuenta.id.clone();
        if !st.db(move |db| db.gastar_totp(&id, p)).await? {
            return Err(ErrorApi::nuevo(StatusCode::UNAUTHORIZED, "codigo", "El código no es correcto."));
        }
    }
    st.limites.olvidar(&format!("totp:{}", cuenta.id));
    st.limites.olvidar(&format!("entrar:{}", cuenta.correo));
    let primera_vez = !cuenta.totp_activo;
    let (id, th, correo) = (cuenta.id.clone(), s.token_hash.clone(), cuenta.correo.clone());
    // Sesión completa = ficha nueva: la de la contraseña sola (a medias) deja de valer.
    let (ficha, th_nueva) = auth::ficha_nueva();
    let codigos = if primera_vez { Some(auth::codigos_recuperacion()) } else { None };
    let hashes = codigos.as_ref().map(|(_, h)| h.clone());
    st.db(move |db| {
        if let Some(h) = hashes {
            // Primera vez: el autenticador queda activo y se dan los códigos de recuperación.
            db.poner_totp(&id, Some(&secreto), true)?;
            db.poner_codigos_recuperacion(&id, &h)?;
            // El código del propietario ya se usó: no sirve otra vez.
            db.poner_valor(&clave_restablecimiento(&id), "")?;
        }
        db.borrar_sesion(&th)?;
        db.crear_sesion(&th_nueva, &id, 2, ahora() + auth::SESION_HORAS * 3600)?;
        db.auditar_servidor(&format!("cuenta:{correo}"), if por_recuperacion { "entrar_recuperacion" } else { "entrar" }, &id, "{}")
    })
    .await?;
    let mut cuerpo = json!({ "cuenta": cuenta_json(&st, &cuenta.id).await? });
    if let Some((codigos, _)) = codigos {
        cuerpo["codigos_recuperacion"] = json!(codigos);
    }
    if let Some(r) = restablecida {
        cuerpo["restablecida"] = r.aviso();
    }
    Ok(con_cookie(auth::cookie_sesion(&st, &ficha, auth::SESION_HORAS * 3600), cuerpo))
}

pub async fn salir(State(st): State<St>, Pendiente(s): Pendiente) -> Res<Response> {
    let th = s.token_hash;
    st.db(move |db| db.borrar_sesion(&th)).await?;
    Ok(([(header::SET_COOKIE, auth::cookie_borrar(&st))], StatusCode::NO_CONTENT).into_response())
}

pub async fn cuenta_json(st: &St, id: &str) -> Res<Value> {
    let id = id.to_string();
    let (cuenta, clientes) = st.db(move |db| Ok((db.cuenta(&id)?, db.clientes_de(&id)?))).await?;
    let cuenta = cuenta.ok_or_else(ErrorApi::sin_sesion)?;
    Ok(json!({
        "id": cuenta.id, "correo": cuenta.correo, "nombre": cuenta.nombre, "superusuario": cuenta.superusuario,
        "clientes": clientes.iter().map(|(c, r)| json!({ "id": c.id, "nombre": c.nombre, "rol": r.texto() })).collect::<Vec<_>>(),
    }))
}

pub async fn cuenta(State(st): State<St>, u: Usuario) -> Res<Json<Value>> {
    Ok(Json(cuenta_json(&st, u.id()).await?))
}

#[derive(Deserialize)]
pub struct CambioContrasena {
    actual: String,
    nueva: String,
}

pub async fn cambiar_contrasena(State(st): State<St>, u: Usuario, Json(p): Json<CambioContrasena>) -> Res<Response> {
    if !st.limites.intento(&format!("contrasena:{}", u.id()), MAX_INTENTOS, VENTANA) {
        return Err(ErrorApi::demasiados());
    }
    auth::valida_contrasena(&p.nueva)?;
    let h = u.0.cuenta.hash.clone();
    let actual = p.actual;
    if !tokio::task::spawn_blocking(move || auth::comprueba_contrasena(&actual, &h)).await.unwrap_or(false) {
        return Err(ErrorApi::nuevo(StatusCode::UNAUTHORIZED, "credenciales", "La contraseña actual no es correcta.").acceso("contrasena"));
    }
    let hash = auth::hash_contrasena(&p.nueva).map_err(ErrorApi::interno)?;
    let (id, correo) = (u.id().to_string(), u.0.cuenta.correo.clone());
    // Todas las sesiones fuera, también esta: sigue con una ficha nueva.
    let (ficha, th_nueva) = auth::ficha_nueva();
    st.db(move |db| {
        db.cambiar_contrasena(&id, &hash)?;
        db.borrar_sesiones_de(&id, None)?;
        db.crear_sesion(&th_nueva, &id, 2, ahora() + auth::SESION_HORAS * 3600)?;
        db.auditar_servidor(&format!("cuenta:{correo}"), "cambiar_contrasena", &id, "{}")
    })
    .await?;
    Ok(([(header::SET_COOKIE, auth::cookie_sesion(&st, &ficha, auth::SESION_HORAS * 3600))], StatusCode::NO_CONTENT).into_response())
}

#[derive(Deserialize)]
pub struct Aceptar {
    token: String,
    correo: Option<String>,
    nombre: Option<String>,
    contrasena: Option<String>,
}

pub async fn aceptar_invitacion(State(st): State<St>, headers: HeaderMap, ip: Option<Extension<IpCliente>>, Json(p): Json<Aceptar>) -> Res<Response> {
    if !st.limites.intento(&format!("invitacion:{}", ip_de(ip)), MAX_INTENTOS, VENTANA) {
        return Err(ErrorApi::demasiados());
    }
    let th = auth::hash_ficha(p.token.trim());
    // ¿Ya con sesión completa? Se añade a esa cuenta.
    if let Ok(s) = auth::sesion_de_cabeceras(&headers, &st).await {
        if s.aal >= 2 {
            let id = s.cuenta.id.clone();
            let correo = s.cuenta.correo.clone();
            let ok = st
                .db(move |db| {
                    let Some((cliente, rol)) = db.tomar_invitacion(&th)? else { return Ok(false) };
                    // No baja el rol de quien ya era miembro.
                    if db.rol(&id, &cliente)?.is_none_or(|r| r < rol) {
                        db.poner_rol(&id, &cliente, rol)?;
                    }
                    db.auditar(
                        &crate::almacen::ClienteCtx::autorizado(&cliente),
                        &format!("cuenta:{correo}"),
                        "aceptar_invitacion",
                        &id,
                        &json!({ "rol": rol.texto() }).to_string(),
                    )?;
                    Ok(true)
                })
                .await?;
            if !ok {
                return Err(ErrorApi::datos("La invitación no es válida o ha caducado.").acceso("invitacion"));
            }
            return Ok(Json(json!({ "cuenta": cuenta_json(&st, &s.cuenta.id).await? })).into_response());
        }
    }
    // Sin sesión: cuenta nueva.
    let correo = auth::valida_correo(p.correo.as_deref().unwrap_or(""))?;
    let contrasena = p.contrasena.unwrap_or_default();
    auth::valida_contrasena(&contrasena)?;
    let nombre = p.nombre.unwrap_or_default().trim().to_string();
    if nombre.is_empty() || nombre.chars().count() > 80 {
        return Err(ErrorApi::datos("Escribe tu nombre (hasta 80 caracteres)."));
    }
    let hash = auth::hash_contrasena(&contrasena).map_err(ErrorApi::interno)?;
    let secreto = auth::nuevo_secreto_totp();
    let (c2, s2) = (correo.clone(), secreto.clone());
    let r = st
        .db_crudo(move |db| {
            if db.cuenta_por_correo(&c2)?.is_some() {
                return Err("Ya hay una cuenta con ese correo: entra con ella y vuelve a abrir el enlace.".into());
            }
            let Some((cliente, rol)) = db.tomar_invitacion(&th)? else { return Err("La invitación no es válida o ha caducado.".into()) };
            let c = db.crear_cuenta(&c2, &nombre, &hash, false)?;
            db.poner_totp(&c.id, Some(&s2), false)?;
            db.poner_rol(&c.id, &cliente, rol)?;
            db.auditar(
                &crate::almacen::ClienteCtx::autorizado(&cliente),
                &format!("cuenta:{c2}"),
                "aceptar_invitacion",
                &c.id,
                &json!({ "rol": rol.texto() }).to_string(),
            )?;
            Ok(c)
        })
        .await?;
    let cuenta = r.map_err(|e| {
        let invalida = e.starts_with("La invitación no es válida");
        let err = ErrorApi::datos(e);
        if invalida {
            err.acceso("invitacion")
        } else {
            err
        }
    })?;
    let ficha = auth::ficha();
    let th = auth::hash_ficha(&ficha);
    let id = cuenta.id.clone();
    st.db(move |db| db.crear_sesion(&th, &id, 1, ahora() + auth::SESION_PENDIENTE_MIN * 60)).await?;
    Ok(con_cookie(
        auth::cookie_sesion(&st, &ficha, auth::SESION_HORAS * 3600),
        json!({ "necesita": "alta_totp", "totp": { "secreto": secreto, "uri": auth::uri_totp(&correo, &secreto) } }),
    ))
}

/// Rol mínimo para leer (cualquier miembro).
pub const MIEMBRO: Rol = Rol::Lectura;

#[derive(Deserialize)]
pub struct NombreCuenta {
    nombre: String,
}

/// `PATCH /api/cuenta`: cambia el nombre visible de la cuenta.
pub async fn renombrar(State(st): State<St>, u: Usuario, Json(p): Json<NombreCuenta>) -> Res<Json<Value>> {
    let nombre = p.nombre.trim().to_string();
    if nombre.is_empty() || nombre.chars().count() > 80 || nombre.chars().any(char::is_control) {
        return Err(ErrorApi::datos("Escribe un nombre (hasta 80 caracteres)."));
    }
    let (id, correo) = (u.id().to_string(), u.0.cuenta.correo.clone());
    st.db(move |db| {
        db.renombrar_cuenta(&id, &nombre)?;
        db.auditar_servidor(&format!("cuenta:{correo}"), "renombrar_cuenta", &id, &json!({ "nombre": nombre }).to_string())
    })
    .await?;
    Ok(Json(cuenta_json(&st, u.id()).await?))
}

#[derive(Deserialize)]
pub struct Reautenticar {
    contrasena: String,
    codigo: String,
}

/// Pide la contraseña y un código TOTP del autenticador actual (para cambiar
/// el autenticador o los códigos de recuperación). Devuelve el paso aceptado.
async fn reautenticar(st: &St, u: &Usuario, p: &Reautenticar) -> Res<i64> {
    if !st.limites.intento(&format!("reautenticar:{}", u.id()), MAX_INTENTOS, VENTANA) {
        return Err(ErrorApi::demasiados());
    }
    let (h, contrasena) = (u.0.cuenta.hash.clone(), p.contrasena.clone());
    let contrasena_ok = tokio::task::spawn_blocking(move || auth::comprueba_contrasena(&contrasena, &h)).await.unwrap_or(false);
    let id = u.id().to_string();
    // La cuenta de nuevo: el último paso de TOTP puede haber cambiado en esta sesión.
    let cuenta = st.db(move |db| db.cuenta(&id)).await?.ok_or_else(ErrorApi::sin_sesion)?;
    let paso = match (&cuenta.totp_secreto, cuenta.totp_activo) {
        (Some(sec), true) => auth::comprueba_totp(sec, &p.codigo, cuenta.totp_ultimo, ahora()),
        _ => None,
    };
    match (contrasena_ok, paso) {
        (true, Some(paso)) => {
            st.limites.olvidar(&format!("reautenticar:{}", u.id()));
            let id = u.id().to_string();
            if !st.db(move |db| db.gastar_totp(&id, paso)).await? {
                return Err(ErrorApi::nuevo(StatusCode::UNAUTHORIZED, "credenciales", "La contraseña o el código no son correctos."));
            }
            Ok(paso)
        }
        _ => Err(ErrorApi::nuevo(StatusCode::UNAUTHORIZED, "credenciales", "La contraseña o el código no son correctos.").acceso("reautenticar")),
    }
}

fn clave_totp_pendiente(cuenta: &str) -> String {
    format!("totp_pendiente:{cuenta}")
}

/// `POST /api/cuenta/totp`: prepara un autenticador nuevo (contraseña y
/// código del actual). El actual sigue valiendo hasta confirmar el nuevo.
pub async fn totp_nuevo(State(st): State<St>, u: Usuario, Json(p): Json<Reautenticar>) -> Res<Json<Value>> {
    reautenticar(&st, &u, &p).await?;
    let secreto = auth::nuevo_secreto_totp();
    let caduca = ahora() + 15 * 60;
    let (clave, valor) = (clave_totp_pendiente(u.id()), format!("{caduca}:{secreto}"));
    st.db(move |db| db.poner_valor(&clave, &valor)).await?;
    Ok(Json(json!({ "totp": { "secreto": secreto, "uri": auth::uri_totp(&u.0.cuenta.correo, &secreto) }, "caduca": crate::api::fecha(caduca) })))
}

#[derive(Deserialize)]
pub struct Codigo {
    codigo: String,
}

/// `POST /api/cuenta/totp/confirmar`: un código del autenticador nuevo lo
/// activa. Se cierran las demás sesiones de la cuenta.
pub async fn totp_confirmar(State(st): State<St>, u: Usuario, Json(p): Json<Codigo>) -> Res<Response> {
    if !st.limites.intento(&format!("totp-nuevo:{}", u.id()), MAX_INTENTOS, VENTANA) {
        return Err(ErrorApi::demasiados());
    }
    let clave = clave_totp_pendiente(u.id());
    let k2 = clave.clone();
    let pendiente = st.db(move |db| db.valor(&k2)).await?.unwrap_or_default();
    let secreto = match pendiente.split_once(':') {
        Some((caduca, sec)) if caduca.parse::<i64>().is_ok_and(|c| c > ahora()) && !sec.is_empty() => sec.to_string(),
        _ => return Err(ErrorApi::datos("No hay ningún autenticador nuevo pendiente (o pasaron 15 minutos): empieza otra vez.")),
    };
    let paso = auth::comprueba_totp(&secreto, &p.codigo, 0, ahora())
        .ok_or_else(|| ErrorApi::nuevo(StatusCode::UNAUTHORIZED, "codigo", "El código no es correcto."))?;
    st.limites.olvidar(&format!("totp-nuevo:{}", u.id()));
    let (id, correo) = (u.id().to_string(), u.0.cuenta.correo.clone());
    // Todas las sesiones fuera, también esta: sigue con una ficha nueva.
    let (ficha, th_nueva) = auth::ficha_nueva();
    st.db(move |db| {
        db.poner_totp(&id, Some(&secreto), true)?;
        db.poner_totp_ultimo(&id, paso)?;
        db.poner_valor(&clave, "")?;
        db.borrar_sesiones_de(&id, None)?;
        db.crear_sesion(&th_nueva, &id, 2, ahora() + auth::SESION_HORAS * 3600)?;
        db.auditar_servidor(&format!("cuenta:{correo}"), "cambiar_autenticador", &id, "{}")
    })
    .await?;
    Ok(con_cookie(auth::cookie_sesion(&st, &ficha, auth::SESION_HORAS * 3600), json!({ "cuenta": cuenta_json(&st, u.id()).await? })))
}

/// `POST /api/cuenta/recuperacion`: códigos de recuperación nuevos (los
/// anteriores dejan de valer). Pide la contraseña y un código TOTP.
pub async fn codigos_nuevos(State(st): State<St>, u: Usuario, Json(p): Json<Reautenticar>) -> Res<Json<Value>> {
    reautenticar(&st, &u, &p).await?;
    let (codigos, hashes) = auth::codigos_recuperacion();
    let (id, correo) = (u.id().to_string(), u.0.cuenta.correo.clone());
    st.db(move |db| {
        db.poner_codigos_recuperacion(&id, &hashes)?;
        db.auditar_servidor(&format!("cuenta:{correo}"), "codigos_recuperacion_nuevos", &id, "{}")
    })
    .await?;
    Ok(Json(json!({ "codigos_recuperacion": codigos })))
}

// ---------- Restablecer la verificación en dos pasos de otro (v1.27) ----------

/// Lo que queda guardado cuando un propietario restablece la verificación en
/// dos pasos de alguien: el hash del código que le dio, hasta cuándo vale,
/// quién y cuándo, y la sesión (a medias) que ya lo usó.
#[derive(Serialize, Deserialize, Clone, Debug)]
struct Restablecimiento {
    h: String,
    caduca: i64,
    cuando: i64,
    por: String,
    #[serde(default)]
    sesion: Option<String>,
}

impl Restablecimiento {
    /// Lo que ve la persona al entrar: «se restableció el … por …».
    fn aviso(&self) -> Value {
        json!({ "cuando": crate::api::fecha(self.cuando), "por": self.por })
    }
}

/// Lo que dura el código que da el propietario.
pub const RESTABLECIMIENTO_HORAS: i64 = 24;

fn clave_restablecimiento(cuenta: &str) -> String {
    format!("totp_restablecido:{cuenta}")
}

async fn restablecimiento(st: &St, cuenta: &str) -> Res<Option<Restablecimiento>> {
    let clave = clave_restablecimiento(cuenta);
    let v = st.db(move |db| db.valor(&clave)).await?.unwrap_or_default();
    Ok(if v.is_empty() { None } else { serde_json::from_str(&v).ok() })
}

/// 16 caracteres sin los que se confunden (80 bits), en grupos de 4.
fn codigo_restablecimiento() -> String {
    const A: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut b = [0u8; 16];
    rand::RngCore::fill_bytes(&mut rand::rng(), &mut b);
    let s: String = b.iter().map(|x| A[*x as usize % 32] as char).collect();
    format!("{}-{}-{}-{}", &s[..4], &s[4..8], &s[8..12], &s[12..])
}

fn hash_restablecimiento(codigo: &str) -> String {
    let limpio: String = codigo.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_uppercase()).collect();
    auth::hash_ficha(&format!("restablecer-totp:{limpio}"))
}

/// `POST /api/clientes/{c}/miembros/{cuenta}/restablecer-totp` (v1.27): quien
/// perdió el móvil y sus códigos de recuperación vuelve a entrar con un código
/// de un solo uso que le da el propietario. Pide un código del autenticador de
/// quien lo hace. Al otro le quita el autenticador, los códigos de recuperación
/// y las sesiones.
///
/// Quién: el propietario del cliente, a miembros que no son propietarios (de
/// ningún cliente) y que solo están en clientes suyos; el propietario del
/// servidor, a cualquiera de ese cliente. Nadie a sí mismo ni al propietario
/// del servidor.
pub async fn restablecer_totp(
    State(st): State<St>,
    u: Usuario,
    axum::extract::Path((c, cuenta)): axum::extract::Path<(String, String)>,
    Json(p): Json<Codigo>,
) -> Res<Json<Value>> {
    let (ctx, rol) = u.miembro(&st, &c, Rol::Lectura).await?;
    let su = u.0.cuenta.superusuario;
    if !su && rol != Rol::Propietario {
        return Err(ErrorApi::prohibido());
    }
    if cuenta == u.id() {
        return Err(ErrorApi::datos("La tuya se cambia en «Mi cuenta» (o, sin el móvil, con un código de recuperación)."));
    }
    // Primero si se puede (sin gastar el código de quien lo hace)…
    let (yo_id, c2, cuenta2) = (u.id().to_string(), c.clone(), cuenta.clone());
    st.db_crudo(move |db| puede_restablecer(db, &yo_id, su, &c2, &cuenta2).map(|_| ())).await?.map_err(error_restablecer)?;

    // …después, el paso de más: un código del autenticador de quien lo hace, recién sacado.
    if !st.limites.intento(&format!("restablecer:{}", u.id()), MAX_INTENTOS, VENTANA) {
        return Err(ErrorApi::demasiados());
    }
    let id = u.id().to_string();
    let yo = st.db(move |db| db.cuenta(&id)).await?.ok_or_else(ErrorApi::sin_sesion)?;
    let paso = match (&yo.totp_secreto, yo.totp_activo) {
        (Some(sec), true) => auth::comprueba_totp(sec, &p.codigo, yo.totp_ultimo, ahora()),
        _ => None,
    };
    let mal_codigo = || ErrorApi::nuevo(StatusCode::UNAUTHORIZED, "codigo", "El código de tu autenticador no es correcto.").acceso("totp");
    let paso = paso.ok_or_else(mal_codigo)?;
    let id = u.id().to_string();
    if !st.db(move |db| db.gastar_totp(&id, paso)).await? {
        return Err(mal_codigo());
    }
    st.limites.olvidar(&format!("restablecer:{}", u.id()));

    let codigo = codigo_restablecimiento();
    let ahora_s = ahora();
    let r = Restablecimiento {
        h: hash_restablecimiento(&codigo),
        caduca: ahora_s + RESTABLECIMIENTO_HORAS * 3600,
        cuando: ahora_s,
        por: u.nombre().to_string(),
        sesion: None,
    };
    let valor = serde_json::to_string(&r).map_err(ErrorApi::interno)?;
    let (yo_id, actor) = (u.id().to_string(), format!("cuenta:{}", u.0.cuenta.correo));
    st.db_crudo(move |db| {
        // Otra vez, ya dentro (las llamadas a la base van de una en una).
        let otro = puede_restablecer(db, &yo_id, su, &c, &cuenta)?;
        db.poner_totp(&cuenta, None, false)?;
        // El último paso era del autenticador perdido: el nuevo empieza de cero.
        db.poner_totp_ultimo(&cuenta, 0)?;
        db.poner_codigos_recuperacion(&cuenta, &[])?;
        db.poner_valor(&clave_totp_pendiente(&cuenta), "")?;
        db.borrar_sesiones_de(&cuenta, None)?;
        db.poner_valor(&clave_restablecimiento(&cuenta), &valor)?;
        let datos = json!({ "correo": otro.correo, "nombre": otro.nombre }).to_string();
        db.auditar(&ctx, &actor, "restablecer_totp", &cuenta, &datos)?;
        db.auditar_servidor(&actor, "restablecer_totp", &cuenta, &json!({ "cliente": ctx.id(), "correo": otro.correo }).to_string())
    })
    .await?
    .map_err(error_restablecer)?;
    Ok(Json(json!({ "codigo": codigo, "caduca": crate::api::fecha(r.caduca) })))
}

/// ¿Puede `yo` restablecer la verificación de `cuenta` desde el cliente `c`? Devuelve esa cuenta.
fn puede_restablecer(db: &dyn crate::almacen::Almacen, yo: &str, su: bool, c: &str, cuenta: &str) -> Result<crate::almacen::Cuenta, String> {
    if !db.miembros(c)?.iter().any(|m| m.cuenta == cuenta) {
        return Err("no_existe".into());
    }
    let otro = db.cuenta(cuenta)?.ok_or("no_existe")?;
    if otro.superusuario {
        return Err("La del propietario del servidor no se puede restablecer desde la consola.".into());
    }
    if !su {
        let suyos = db.clientes_de(cuenta)?;
        if suyos.iter().any(|(_, r)| *r == Rol::Propietario) {
            return Err("Es propietario: solo el propietario del servidor puede restablecer su verificación en dos pasos.".into());
        }
        for (cl, _) in &suyos {
            if db.rol(yo, &cl.id)? != Some(Rol::Propietario) {
                return Err("También está en clientes de los que no eres propietario: pídeselo al propietario del servidor.".into());
            }
        }
    }
    Ok(otro)
}

fn error_restablecer(e: String) -> ErrorApi {
    if e == "no_existe" {
        ErrorApi::no_existe()
    } else {
        ErrorApi::nuevo(StatusCode::FORBIDDEN, "prohibido", e)
    }
}

/// `POST /api/sesion/restablecimiento` (v1.27): con la contraseña ya dada,
/// el código del propietario abre el alta de un autenticador nuevo.
pub async fn usar_restablecimiento(State(st): State<St>, Pendiente(s): Pendiente, Json(p): Json<Codigo>) -> Res<Json<Value>> {
    if !st.limites.intento(&format!("restablecimiento:{}", s.cuenta.id), MAX_INTENTOS, VENTANA) {
        return Err(ErrorApi::demasiados());
    }
    let mut r = restablecimiento(&st, &s.cuenta.id)
        .await?
        .ok_or_else(|| ErrorApi::datos("Esta cuenta no tiene ningún restablecimiento pendiente: vuelve a entrar."))?;
    if r.caduca <= ahora() {
        return Err(ErrorApi::nuevo(StatusCode::GONE, "caducado", "El código caducó (dura 24 horas): pide al propietario que la restablezca otra vez."));
    }
    let dado = hash_restablecimiento(&p.codigo);
    if !bool::from(subtle::ConstantTimeEq::ct_eq(r.h.as_bytes(), dado.as_bytes())) {
        return Err(ErrorApi::nuevo(StatusCode::UNAUTHORIZED, "codigo", "El código no es correcto.").acceso("restablecimiento"));
    }
    st.limites.olvidar(&format!("restablecimiento:{}", s.cuenta.id));
    // Vale hasta que se dé de alta el autenticador (o caduque), atado a esta sesión.
    let secreto = auth::nuevo_secreto_totp();
    r.sesion = Some(s.token_hash.clone());
    let valor = serde_json::to_string(&r).map_err(ErrorApi::interno)?;
    let (id, s2, correo) = (s.cuenta.id.clone(), secreto.clone(), s.cuenta.correo.clone());
    st.db(move |db| {
        db.poner_totp(&id, Some(&s2), false)?;
        db.poner_valor(&clave_restablecimiento(&id), &valor)?;
        db.auditar_servidor(&format!("cuenta:{correo}"), "usar_restablecimiento_totp", &id, "{}")
    })
    .await?;
    Ok(Json(json!({ "necesita": "alta_totp", "totp": { "secreto": secreto, "uri": auth::uri_totp(&s.cuenta.correo, &secreto) }, "restablecida": r.aviso() })))
}
