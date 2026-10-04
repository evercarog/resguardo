//! «Copia de la consola» (v1.23): el propietario del servidor la enciende con la
//! clave pública de su clave de respaldo (calculada en el navegador), mira cómo
//! fue la última y la hace al momento. Ver `crate::respaldo`.
//!
//! No hay ruta para descargar las copias: se quedan en la carpeta de datos del
//! servidor (las recoge el agente de esa máquina como una copia más). Así, quien
//! entre en la consola con la cuenta del propietario no puede llevárselas, y
//! cambiar la clave pública solo afecta a las copias siguientes (queda en la
//! auditoría del servidor).

use crate::auth::Usuario;
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use crate::respaldo::{self, Ajustes};
use axum::extract::State;
use axum::Json;
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};

fn solo_propietario(u: &Usuario) -> Res<()> {
    if u.0.cuenta.superusuario {
        Ok(())
    } else {
        Err(ErrorApi::prohibido())
    }
}

/// La próxima copia diaria.
fn proxima(a: &Ajustes, ahora: chrono::DateTime<chrono::Local>) -> Option<String> {
    if !a.activo || a.publica.is_none() {
        return None;
    }
    let h = chrono::NaiveTime::parse_from_str(&a.hora, "%H:%M").ok()?;
    let hoy = ahora.date_naive();
    let hecha_hoy = a.diaria.as_deref().is_some_and(|d| d.starts_with(&hoy.format("%Y-%m-%d").to_string())) && a.diaria_ok;
    let dia = if hecha_hoy || (ahora.time() >= h && !respaldo::toca(a, ahora)) { hoy.succ_opt()? } else { hoy };
    if dia == hoy && ahora.time() >= h {
        // Pasada la hora y sin hacer: en los próximos minutos.
        return Some(ahora.to_rfc3339());
    }
    dia.and_time(h).and_local_timezone(chrono::Local).single().map(|t| t.to_rfc3339())
}

fn vista(st: &St, a: &Ajustes) -> Value {
    json!({
        "activo": a.activo,
        "clave_puesta": a.clave_puesta,
        "sal": a.sal,
        "conservar": a.conservar,
        "hora": a.hora,
        "carpeta": respaldo::carpeta(&st.datos).display().to_string(),
        "ultima": a.ultima,
        "proxima": proxima(a, chrono::Local::now()),
        "copias": respaldo::copias(&st.datos).into_iter().map(|(archivo, bytes)| json!({ "archivo": archivo, "bytes": bytes })).collect::<Vec<_>>(),
        "identidad": st.identidad_pub,
    })
}

/// `GET /api/servidor/respaldo` (propietario del servidor).
pub async fn ver(State(st): State<St>, u: Usuario) -> Res<Json<Value>> {
    solo_propietario(&u)?;
    let datos = st.datos.clone();
    let a = tokio::task::spawn_blocking(move || respaldo::leer(&datos)).await.map_err(ErrorApi::interno)?;
    Ok(Json(vista(&st, &a)))
}

#[derive(Deserialize)]
pub struct Cambio {
    activo: Option<bool>,
    /// Clave pública X25519 de la clave de respaldo (base64), con su sal.
    publica: Option<String>,
    sal: Option<String>,
    conservar: Option<u32>,
    hora: Option<String>,
}

/// `PUT /api/servidor/respaldo` (propietario del servidor): encender o apagar,
/// poner (o cambiar) la clave, cuántas conservar y a qué hora.
pub async fn cambiar(State(st): State<St>, u: Usuario, Json(c): Json<Cambio>) -> Res<Json<Value>> {
    solo_propietario(&u)?;
    let datos = st.datos.clone();
    let mut a = tokio::task::spawn_blocking(move || respaldo::leer(&datos)).await.map_err(ErrorApi::interno)?;
    let clave_nueva = match (c.publica, c.sal) {
        (Some(p), Some(s)) => {
            let sal_ok = base64::engine::general_purpose::STANDARD.decode(&s).is_ok_and(|v| v.len() == 16);
            if !resguardo_protocolo::respaldo_consola::publica_valida(&p) || !sal_ok {
                return Err(ErrorApi::datos("Clave pública o sal no válidas."));
            }
            a.publica = Some(p);
            a.sal = Some(s);
            a.clave_puesta = Some(chrono::Local::now().to_rfc3339());
            true
        }
        (None, None) => false,
        _ => return Err(ErrorApi::datos("La clave pública y su sal van juntas.")),
    };
    if let Some(n) = c.conservar {
        if !(1..=60).contains(&n) {
            return Err(ErrorApi::datos("Se pueden conservar de 1 a 60 copias."));
        }
        a.conservar = n;
    }
    if let Some(h) = c.hora {
        if chrono::NaiveTime::parse_from_str(&h, "%H:%M").is_err() || h.len() != 5 {
            return Err(ErrorApi::datos("Hora no válida (HH:MM)."));
        }
        a.hora = h;
    }
    if let Some(x) = c.activo {
        a.activo = x;
    }
    if a.activo && a.publica.is_none() {
        return Err(ErrorApi::datos("Primero pon la clave de respaldo de la consola."));
    }
    let (datos, a2, correo) = (st.datos.clone(), a.clone(), u.0.cuenta.correo.clone());
    let detalle = json!({ "activo": a.activo, "conservar": a.conservar, "hora": a.hora, "clave_nueva": clave_nueva }).to_string();
    st.db(move |db| {
        respaldo::guardar(&datos, &a2)?;
        db.auditar_servidor(&format!("cuenta:{correo}"), "copia_consola", "", &detalle)
    })
    .await?;
    Ok(Json(vista(&st, &a)))
}

/// `POST /api/servidor/respaldo/ahora` (propietario del servidor): una copia al momento.
pub async fn ahora(State(st): State<St>, u: Usuario) -> Res<Json<Value>> {
    solo_propietario(&u)?;
    if !st.limites.intento("respaldo-consola", 12, std::time::Duration::from_secs(3600)) {
        return Err(ErrorApi::demasiados());
    }
    let (datos, identidad) = (st.datos.clone(), st.identidad.clone());
    let r = tokio::task::spawn_blocking(move || respaldo::hacer(&datos, &identidad, "manual")).await.map_err(ErrorApi::interno)?;
    let correo = u.0.cuenta.correo.clone();
    let ok = r.is_ok();
    st.db(move |db| db.auditar_servidor(&format!("cuenta:{correo}"), "copia_consola_ahora", "", &json!({ "ok": ok }).to_string())).await?;
    r.map_err(ErrorApi::datos)?;
    let datos = st.datos.clone();
    let a = tokio::task::spawn_blocking(move || respaldo::leer(&datos)).await.map_err(ErrorApi::interno)?;
    Ok(Json(vista(&st, &a)))
}
