//! 14bbb (plan 0.8.0, B4): el recordatorio de comprobar los kits.
//!
//! Por cliente (activado por defecto, cada [`MESES`] meses; se cambia o se apaga en sus
//! ajustes): si la contraseña de un repositorio lleva ese tiempo sin comprobarse con
//! «Comprobar la contraseña», un aviso **informativo** («Comprueba que tu kit de «X» sigue
//! sirviendo»). Lo que cuenta es lo que dice el equipo en su resumen
//! (`repositorios[].contrasena_comprobada.bien`); de un repositorio que nunca se comprobó,
//! desde que este servidor lo vio por primera vez (así, al actualizar, no salen todos a la
//! vez). Como mucho un aviso por repositorio cada ese tiempo. Solo equipos que saben
//! comprobarla (`admite: comprobar_contrasena`).

use crate::almacen::{Almacen, ClienteCtx, Rol, Ts, R};
use crate::auth::Usuario;
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use axum::extract::{Path as Ruta, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const MESES: u32 = 6;
pub const MAX_MESES: u32 = 36;
const MES_S: Ts = 30 * 24 * 3600;
pub const TIPO: &str = "kit_por_comprobar";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ajustes {
    #[serde(default = "si")]
    pub activo: bool,
    #[serde(default = "meses")]
    pub meses: u32,
}

impl Default for Ajustes {
    fn default() -> Self {
        Ajustes { activo: true, meses: MESES }
    }
}

fn si() -> bool {
    true
}

fn meses() -> u32 {
    MESES
}

fn clave(c: &str) -> String {
    format!("kit:recordatorio:{c}")
}

pub fn ajustes(db: &dyn Almacen, c: &str) -> R<Ajustes> {
    let mut a: Ajustes = db.valor(&clave(c))?.and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    a.meses = a.meses.clamp(1, MAX_MESES);
    Ok(a)
}

/// Un repositorio de un equipo, tal como lo dice su resumen.
#[derive(Debug, PartialEq, Eq)]
pub struct Repo {
    pub id: String,
    pub nombre: String,
    /// Última comprobación buena (segundos).
    pub bien: Option<Ts>,
}

fn corto(v: &Value, max: usize) -> String {
    v.as_str().unwrap_or_default().chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

/// Los repositorios del resumen (si el equipo sabe comprobar la contraseña).
pub fn repos_del_resumen(resumen: &Value) -> Vec<Repo> {
    let admite = resumen["admite"].as_array().is_some_and(|a| a.iter().any(|x| x == "comprobar_contrasena"));
    if !admite {
        return Vec::new();
    }
    resumen["repositorios"]
        .as_array()
        .into_iter()
        .flatten()
        .take(200)
        .filter_map(|r| {
            let id = corto(&r["id"], 64);
            if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                return None;
            }
            let nombre = Some(corto(&r["nombre"], 80)).filter(|n| !n.is_empty()).unwrap_or_else(|| id.clone());
            let bien = r["contrasena_comprobada"]["bien"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()).map(|d| d.timestamp());
            Some(Repo { id, nombre, bien })
        })
        .collect()
}

/// ¿Toca avisar? `desde`: la última comprobación buena o cuando se vio; `avisado`: el último aviso.
pub fn toca(a: &Ajustes, desde: Ts, avisado: Option<Ts>, ahora: Ts) -> bool {
    let plazo = i64::from(a.meses.clamp(1, MAX_MESES)) * MES_S;
    a.activo && ahora - desde >= plazo && avisado.is_none_or(|x| x < desde || ahora - x >= plazo)
}

/// El texto del aviso.
pub fn texto(repo: &str, equipo: &str, meses: u32, nunca: bool) -> String {
    let cuando = if nunca {
        format!("no se ha comprobado nunca (y lleva más de {meses} meses aquí)")
    } else {
        format!("no se comprueba desde hace más de {meses} meses")
    };
    format!(
        "Comprueba que tu kit de «{repo}» sigue sirviendo: su contraseña {cuando}. En la página del repositorio, en «{equipo}», usa «Comprobar la contraseña»."
    )
}

/// En la vuelta de limpieza (cada 10 min): los avisos que tocan en un cliente.
pub fn recordar(st: &St, ctx: &ClienteCtx, ahora: Ts) {
    let db = st.db.as_ref();
    let Ok(a) = ajustes(db, ctx.id()) else { return };
    if !a.activo {
        return;
    }
    let Ok(equipos) = db.equipos(ctx) else { return };
    for e in equipos.iter().filter(|e| e.confirmado && e.modo == "gestionado") {
        let Some(resumen) = &e.resumen else { continue };
        for r in repos_del_resumen(resumen) {
            let base = format!("kit:{}:{}:{}", ctx.id(), e.id, r.id);
            let visto = match db.valor(&format!("{base}:visto")).ok().flatten().and_then(|t| t.parse::<Ts>().ok()) {
                Some(t) => t,
                None => {
                    let _ = db.poner_valor(&format!("{base}:visto"), &ahora.to_string());
                    ahora
                }
            };
            let desde = r.bien.unwrap_or(visto);
            let avisado = db.valor(&format!("{base}:avisado")).ok().flatten().and_then(|t| t.parse::<Ts>().ok());
            if toca(&a, desde, avisado, ahora)
                && crate::notificaciones::aviso(db, ctx, Some(&e.id), TIPO, &texto(&r.nombre, &e.nombre, a.meses, r.bien.is_none())).is_ok()
            {
                let _ = db.poner_valor(&format!("{base}:avisado"), &ahora.to_string());
                st.vivo.avisar(ctx.id(), crate::vivo::Cambio::Avisos(Some(&e.id)));
            }
        }
    }
}

/// `GET /api/clientes/{c}/recordatorio-kit` (cualquiera del cliente): `{ activo, meses }`.
pub async fn ver(State(st): State<St>, u: Usuario, Ruta(c): Ruta<String>) -> Res<Json<Value>> {
    u.miembro(&st, &c, Rol::Lectura).await?;
    let a = st.db(move |db| ajustes(db, &c)).await?;
    Ok(Json(json!(a)))
}

/// `PUT /api/clientes/{c}/recordatorio-kit` (administrador): `{ activo, meses }`.
pub async fn cambiar(State(st): State<St>, u: Usuario, Ruta(c): Ruta<String>, Json(n): Json<Ajustes>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    if !(1..=MAX_MESES).contains(&n.meses) {
        return Err(ErrorApi::datos(format!("Cada 1 a {MAX_MESES} meses.")));
    }
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let n2 = n.clone();
    st.db(move |db| {
        db.poner_valor(&clave(&c), &serde_json::to_string(&n2).map_err(|e| e.to_string())?)?;
        db.auditar(&ctx, &actor, "recordatorio_kit", &c, &json!(n2).to_string())
    })
    .await?;
    Ok(Json(json!(n)))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cuando_avisa() {
        let a = Ajustes::default();
        let seis = 6 * MES_S;
        assert!(!toca(&a, 0, None, seis - 1));
        assert!(toca(&a, 0, None, seis));
        // Ya avisado: no otra vez hasta pasado otro plazo…
        assert!(!toca(&a, 0, Some(seis), seis + 10));
        assert!(toca(&a, 0, Some(seis), 2 * seis));
        // …salvo que se comprobara después del aviso y vuelva a vencer.
        assert!(toca(&a, seis + 5, Some(seis), 2 * seis + 5));
        assert!(!toca(&Ajustes { activo: false, meses: 6 }, 0, None, 10 * seis));
        assert!(toca(&Ajustes { activo: true, meses: 1 }, 0, None, MES_S));
    }

    #[test]
    fn lee_el_resumen_como_dato_de_fuera() {
        let r = json!({
            "admite": ["comprobar_contrasena"],
            "repositorios": [
                { "id": "docs", "nombre": "Ventas", "contrasena_comprobada": { "bien": "2026-04-01T10:00:00+00:00", "ultima": "x", "resultado": "abre" } },
                { "id": "otro", "nombre": "" },
                { "id": "../malo", "nombre": "X" },
            ],
        });
        let v = repos_del_resumen(&r);
        assert_eq!(v.len(), 2);
        assert_eq!((v[0].nombre.as_str(), v[0].bien), ("Ventas", Some(1775037600)));
        assert_eq!((v[1].nombre.as_str(), v[1].bien), ("otro", None));
        // Un agente que no sabe comprobarla: nada que recordar.
        assert!(repos_del_resumen(&json!({ "repositorios": [{ "id": "docs" }] })).is_empty());
        let t = texto("Ventas", "PC-01", 6, false);
        assert!(t.starts_with("Comprueba que tu kit de «Ventas» sigue sirviendo") && !t.to_lowercase().contains("restic"));
    }
}
