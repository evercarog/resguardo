//! Informe ejecutivo (plan 0.8.0, G1; docs/api-servidor.md, «Informe ejecutivo»):
//!
//! - `GET  /api/clientes/{c}/informe-ejecutivo` (miembro): el programa, el prestador, la
//!   última regla que calculó la consola y el último envío.
//! - `PUT  /api/clientes/{c}/informe-ejecutivo` (administrador o más): el programa
//!   (`activo`, `periodo`, `dia_semana`, `destinatarios`, `logo`). Auditado.
//! - `PUT  /api/clientes/{c}/informe-ejecutivo/regla` (técnico o más): la regla 3-2-1-1-0
//!   que acaba de calcular la consola (solo ids y estados).
//! - `GET  /api/clientes/{c}/informe-ejecutivo/pdf?periodo=&cual=` (miembro): «Descargar el informe».
//! - `POST /api/clientes/{c}/informe-ejecutivo/enviar` (administrador o más): «Enviar ahora».
//! - `GET/PUT /api/servidor/informes` (propietario del servidor): el nombre y el logo del
//!   prestador; `GET /api/servidor/informes/logo` (cualquier sesión): el PNG.
//!
//! El logo es solo PNG (como el de la marca de un cliente: la consola convierte antes un
//! SVG, JPG o WebP a PNG en el navegador; aquí nunca llega un SVG con scripts) y se comprueba
//! con `marca::png_valido`. Nada de rutas ni nombres de archivos; nunca contraseñas.

use crate::almacen::{ahora, Rol};
use crate::auth::Usuario;
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use crate::informe::{self, CopiaFoto, FotoRegla, Logo, Periodo, Prestador, Programa};
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::Digest;
use std::time::Duration;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

fn actor(u: &Usuario) -> String {
    format!("cuenta:{}", u.0.cuenta.correo)
}

fn vista_programa(p: &Programa) -> Value {
    json!({
        "activo": p.activo, "periodo": p.periodo, "dia_semana": p.dia_semana, "destinatarios": p.destinatarios, "logo": p.logo,
        "actualizado": (p.actualizado > 0).then(|| super::fecha(p.actualizado)), "por": (!p.por.is_empty()).then_some(&p.por),
    })
}

fn vista_prestador(p: &Prestador) -> Value {
    json!({
        "nombre": p.nombre,
        "logo": p.huella.as_ref().filter(|_| p.logo.is_some()).map(|h| format!("/api/servidor/informes/logo?v={h}")),
        "actualizado": super::fecha_opt(p.actualizado), "por": p.por,
    })
}

/// `GET /api/clientes/{c}/informe-ejecutivo`.
pub async fn ver(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    u.miembro(&st, &c, super::cuentas::MIEMBRO).await?;
    let v = st
        .db(move |db| {
            let p = informe::programa(db, &c)?;
            let prestador = informe::prestador(db)?;
            let foto = informe::foto_regla(db, &c)?;
            let ultimo = informe::ultimo(db, &c)?;
            let ajustes = crate::notificaciones::ajustes::ajustes(db)?;
            let propios = crate::notificaciones::ajustes::ajustes_cliente(db, &c)?;
            let logo_cliente = super::marca::leer(db, &c)?.logo.is_some();
            Ok(json!({
                "programa": vista_programa(&p),
                "prestador": vista_prestador(&prestador),
                "logo_cliente": logo_cliente,
                "regla": foto.map(|f| json!({ "calculada": super::fecha(f.calculada), "por": f.por, "copias": f.copias.len() })),
                "ultimo": ultimo.map(|u| json!({
                    "cuando": super::fecha(u.cuando), "periodo": u.periodo, "desde": super::fecha(u.desde), "hasta": super::fecha(u.hasta),
                    "destinatarios": u.destinatarios, "a_mano": u.a_mano, "por": u.por,
                })),
                "correo": crate::notificaciones::correo_de(&ajustes, &propios, &c).is_some(),
                "hora": ajustes.hora_resumen,
            }))
        })
        .await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct CambioPrograma {
    activo: Option<bool>,
    periodo: Option<String>,
    dia_semana: Option<u8>,
    destinatarios: Option<Vec<String>>,
    logo: Option<Logo>,
}

/// `PUT /api/clientes/{c}/informe-ejecutivo` (administrador o más).
pub async fn cambiar(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(b): Json<CambioPrograma>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let periodo = match b.periodo.as_deref() {
        Some(p) => Some(Periodo::de(p).ok_or_else(|| ErrorApi::datos("El periodo es «semanal» o «mensual»."))?),
        None => None,
    };
    if b.dia_semana.is_some_and(|d| !(1..=7).contains(&d)) {
        return Err(ErrorApi::datos("Día no válido (1 = lunes … 7 = domingo)."));
    }
    let destinatarios = match &b.destinatarios {
        Some(d) => Some(informe::valida_destinatarios(d).map_err(ErrorApi::datos)?),
        None => None,
    };
    let (actor, por) = (actor(&u), u.0.cuenta.nombre.clone());
    let p = st
        .db_crudo(move |db| {
            let previo = informe::programa(db, &c)?;
            let mut p = previo.clone();
            if let Some(x) = b.activo {
                p.activo = x;
            }
            if let Some(x) = periodo {
                p.periodo = x;
            }
            if let Some(x) = b.dia_semana {
                p.dia_semana = x;
            }
            if let Some(x) = destinatarios {
                p.destinatarios = x;
            }
            if let Some(x) = b.logo {
                p.logo = x;
            }
            if p.activo && p.destinatarios.is_empty() {
                return Ok(Err("Añade al menos un destinatario para programar el informe."));
            }
            let t = ahora();
            p.actualizado = t;
            p.por = por;
            informe::guardar_programa(db, &c, &previo, &p, t)?;
            // Sin las direcciones en la auditoría: cuántas.
            let datos = json!({ "activo": p.activo, "periodo": p.periodo, "dia_semana": p.dia_semana, "destinatarios": p.destinatarios.len(), "logo": p.logo });
            db.auditar(&ctx, &actor, "cambiar_informe_ejecutivo", &c, &datos.to_string())?;
            Ok(Ok(p))
        })
        .await?
        .map_err(|e: String| ErrorApi::datos(e))?
        .map_err(ErrorApi::datos)?;
    Ok(Json(vista_programa(&p)))
}

#[derive(Deserialize)]
pub struct Regla {
    copias: Vec<CopiaFoto>,
}

/// `PUT /api/clientes/{c}/informe-ejecutivo/regla` (técnico o más): la regla que calculó la consola.
pub async fn poner_regla(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(b): Json<Regla>) -> Res<StatusCode> {
    u.miembro(&st, &c, Rol::Tecnico).await?;
    if b.copias.len() > informe::MAX_COPIAS_REGLA || b.copias.iter().any(|x| !informe::id_valido(&x.equipo) || !informe::id_valido(&x.copia)) {
        return Err(ErrorApi::datos("Regla no válida."));
    }
    if !st.limites.intento(&format!("informe-regla:{c}"), 120, Duration::from_secs(3600)) {
        return Err(ErrorApi::demasiados());
    }
    let por = u.0.cuenta.nombre.clone();
    st.db(move |db| {
        let ctx = crate::almacen::ClienteCtx::autorizado(&c);
        let equipos: Vec<String> = db.equipos(&ctx)?.into_iter().map(|e| e.id).collect();
        let copias = b.copias.into_iter().filter(|x| equipos.contains(&x.equipo)).collect();
        informe::guardar_foto_regla(db, &c, &FotoRegla { calculada: ahora(), por, copias })
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, Default)]
pub struct QuePeriodo {
    /// `semanal` o `mensual` (sin él, el del programa).
    #[serde(default)]
    periodo: Option<String>,
    /// `anterior` (el último entero; por defecto) o `actual` (el que va en curso, hasta hoy).
    #[serde(default)]
    cual: Option<String>,
}

fn periodo_de(q: &QuePeriodo, programa: &Programa, ahora: i64) -> Res<(Periodo, i64, i64)> {
    let p = match q.periodo.as_deref() {
        None | Some("") => programa.periodo,
        Some(x) => Periodo::de(x).ok_or_else(|| ErrorApi::datos("El periodo es «semanal» o «mensual»."))?,
    };
    let (desde, hasta) = match q.cual.as_deref() {
        None | Some("") | Some("anterior") => informe::periodo_anterior(p, ahora),
        Some("actual") => informe::periodo_actual(p, ahora),
        Some(_) => return Err(ErrorApi::datos("«cual» es «anterior» o «actual».")),
    };
    Ok((p, desde, hasta))
}

/// `GET /api/clientes/{c}/informe-ejecutivo/pdf` (miembro): el PDF, para descargar.
pub async fn pdf(State(st): State<St>, u: Usuario, Path(c): Path<String>, Query(q): Query<QuePeriodo>) -> Res<Response> {
    u.miembro(&st, &c, super::cuentas::MIEMBRO).await?;
    if !st.limites.intento(&format!("informe-pdf:{}", u.id()), 60, Duration::from_secs(3600)) {
        return Err(ErrorApi::demasiados());
    }
    let (nombre, bytes) = st
        .db_crudo(move |db| {
            let t = ahora();
            let prog = informe::programa(db, &c)?;
            let (p, desde, hasta) = match periodo_de(&q, &prog, t) {
                Ok(x) => x,
                Err(e) => return Ok(Err(e)),
            };
            let Some(i) = informe::datos::preparar(db, &c, p, desde, hasta, t)? else { return Ok(Err(ErrorApi::no_existe())) };
            let logo = informe::logo(db, &c, prog.logo);
            Ok(Ok((informe::nombre_archivo(&i), informe::pdf::pdf(&i, logo.as_deref()))))
        })
        .await?
        .map_err(|e: String| ErrorApi::datos(e))??;
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/pdf".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{nombre}\"")),
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
        bytes,
    )
        .into_response())
}

/// `POST /api/clientes/{c}/informe-ejecutivo/enviar` (administrador o más): «Enviar ahora» a los
/// destinatarios del programa. Como mucho 10 por hora y cliente.
pub async fn enviar(State(st): State<St>, u: Usuario, Path(c): Path<String>, b: Option<Json<QuePeriodo>>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    if !st.limites.intento(&format!("informe-enviar:{c}"), 10, Duration::from_secs(3600)) {
        return Err(ErrorApi::demasiados());
    }
    let q = b.map(|Json(x)| x).unwrap_or_default();
    let (actor, por) = (actor(&u), u.0.cuenta.nombre.clone());
    let n = st
        .db_crudo(move |db| {
            let t = ahora();
            let prog = informe::programa(db, &c)?;
            let (p, desde, hasta) = match periodo_de(&q, &prog, t) {
                Ok(x) => x,
                Err(e) => return Ok(Err(e)),
            };
            let r = informe::encolar(db, &c, p, desde, hasta, &prog.destinatarios, t, Some(&por))?;
            if let Ok(n) = &r {
                db.auditar(&ctx, &actor, "enviar_informe_ejecutivo", &c, &json!({ "periodo": p, "destinatarios": n }).to_string())?;
            }
            Ok(r.map_err(ErrorApi::datos))
        })
        .await?
        .map_err(|e: String| ErrorApi::datos(e))??;
    st.notif.despertar.notify_one();
    Ok(Json(json!({ "enviados": n })))
}

// ---------- El prestador (propietario del servidor) ----------

fn solo_propietario(u: &Usuario) -> Res<()> {
    if u.0.cuenta.superusuario {
        Ok(())
    } else {
        Err(ErrorApi::prohibido())
    }
}

/// `GET /api/servidor/informes` (propietario del servidor).
pub async fn ver_prestador(State(st): State<St>, u: Usuario) -> Res<Json<Value>> {
    solo_propietario(&u)?;
    let p = st.db(|db| informe::prestador(db)).await?;
    Ok(Json(vista_prestador(&p)))
}

#[derive(Deserialize)]
pub struct CambioPrestador {
    /// `""` lo quita.
    nombre: Option<String>,
    /// PNG en base64: lo sustituye.
    logo: Option<String>,
    #[serde(default)]
    quitar_logo: bool,
}

/// `PUT /api/servidor/informes` (propietario del servidor): el nombre y el logo del prestador.
pub async fn cambiar_prestador(State(st): State<St>, u: Usuario, Json(b): Json<CambioPrestador>) -> Res<Json<Value>> {
    solo_propietario(&u)?;
    let nombre = match &b.nombre {
        Some(n) => Some(informe::valida_nombre_prestador(n).map_err(ErrorApi::datos)?),
        None => None,
    };
    let png = match &b.logo {
        Some(x) => {
            let bytes = B64.decode(x).map_err(|_| ErrorApi::datos("Logo no válido (base64)."))?;
            if bytes.len() > super::marca::MAX_LOGO {
                return Err(ErrorApi::datos("El logo pesa demasiado: como mucho 200 KB."));
            }
            super::marca::png_valido(&bytes).map_err(ErrorApi::datos)?;
            let huella: String = sha2::Sha256::digest(&bytes).iter().take(8).map(|b| format!("{b:02x}")).collect();
            Some((x.clone(), huella))
        }
        None => None,
    };
    let (actor, por) = (actor(&u), u.0.cuenta.nombre.clone());
    let p = st
        .db(move |db| {
            let mut p = informe::prestador(db)?;
            if let Some(n) = nombre {
                p.nombre = n;
            }
            let logo = if b.quitar_logo {
                p.logo = None;
                p.huella = None;
                "quitado"
            } else if let Some((x, h)) = png {
                p.logo = Some(x);
                p.huella = Some(h);
                "puesto"
            } else {
                "igual"
            };
            p.actualizado = Some(ahora());
            p.por = Some(por);
            informe::guardar_prestador(db, &p)?;
            db.auditar_servidor(&actor, "cambiar_prestador_informes", "servidor", &json!({ "nombre": p.nombre.is_some(), "logo": logo }).to_string())?;
            Ok(p)
        })
        .await?;
    Ok(Json(vista_prestador(&p)))
}

/// `GET /api/servidor/informes/logo` (cualquier sesión): el PNG tal cual.
pub async fn logo_prestador(State(st): State<St>, _u: Usuario) -> Res<Response> {
    let p = st.db(|db| informe::prestador(db)).await?;
    let bytes = p.logo.and_then(|b| B64.decode(b).ok()).ok_or_else(ErrorApi::no_existe)?;
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "private, max-age=86400"),
            (header::CONTENT_DISPOSITION, "inline; filename=\"logo.png\""),
        ],
        bytes,
    )
        .into_response())
}
