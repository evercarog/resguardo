//! 0.8.3 (16h): «Este equipo no llega a esa consola». Un equipo sin salida a internet (a
//! propósito: red aislada) nunca estará en la consola en línea, y el aviso «N equipos no
//! están en todas tus consolas» no debería contarlo. Por equipo y por consola (su
//! identidad), con un motivo opcional; se puede deshacer. Es de esta consola (lo ve quien
//! gestiona el equipo aquí), en los valores del servidor (`no_llega:<cliente>`), sin
//! secretos. Auditado.

use crate::almacen::{ahora, Almacen, Rol, R};
use crate::auth::Usuario;
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Como mucho tantas marcas por cliente.
pub const MAX_MARCAS: usize = 2000;
/// El motivo, como mucho.
pub const MAX_MOTIVO: usize = 200;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Marca {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motivo: Option<String>,
    pub por: String,
    /// Unix (s).
    pub cuando: i64,
}

/// Equipo → identidad de la consola → la marca.
pub type Marcas = BTreeMap<String, BTreeMap<String, Marca>>;

fn clave(cliente: &str) -> String {
    format!("no_llega:{cliente}")
}

pub fn leer(db: &dyn Almacen, cliente: &str) -> R<Marcas> {
    Ok(db.valor(&clave(cliente))?.filter(|v| !v.is_empty()).and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default())
}

/// El id de un equipo (como los de la base: uuid u otro id corto sin rarezas).
pub fn equipo_valido(e: &str) -> bool {
    !e.is_empty() && e.len() <= 128 && e.chars().all(|c| c.is_ascii_alphanumeric() || "-_".contains(c))
}

/// La identidad de una consola (Ed25519 pública en base64, como la de `resumen.consolas`).
pub fn identidad_valida(i: &str) -> bool {
    !i.is_empty() && i.len() <= 128 && i.chars().all(|c| c.is_ascii_alphanumeric() || "+/=_-".contains(c))
}

/// El motivo limpio (sin controles, corto) o `None` si va vacío.
pub fn motivo_valido(m: Option<&str>) -> Result<Option<String>, &'static str> {
    let Some(m) = m.map(str::trim).filter(|m| !m.is_empty()) else { return Ok(None) };
    if m.chars().count() > MAX_MOTIVO || m.chars().any(char::is_control) {
        return Err("El motivo es demasiado largo o tiene caracteres que no valen (hasta 200).");
    }
    Ok(Some(m.to_string()))
}

/// Pone o quita una marca. Devuelve si cambió algo.
pub fn aplicar(m: &mut Marcas, equipo: &str, consola: &str, marca: Option<Marca>) -> Result<bool, &'static str> {
    match marca {
        Some(x) => {
            let total: usize = m.values().map(BTreeMap::len).sum();
            let ya = m.get(equipo).is_some_and(|c| c.contains_key(consola));
            if !ya && total >= MAX_MARCAS {
                return Err("Demasiados equipos marcados en este cliente.");
            }
            let antes = m.entry(equipo.to_string()).or_default().insert(consola.to_string(), x.clone());
            Ok(antes.as_ref() != Some(&x))
        }
        None => {
            let Some(c) = m.get_mut(equipo) else { return Ok(false) };
            let quitada = c.remove(consola).is_some();
            if c.is_empty() {
                m.remove(equipo);
            }
            Ok(quitada)
        }
    }
}

fn vista(m: &Marcas) -> Value {
    json!(m
        .iter()
        .map(|(e, cs)| (
            e.clone(),
            Value::Object(cs.iter().map(|(i, x)| (i.clone(), json!({ "motivo": x.motivo, "por": x.por, "cuando": super::fecha(x.cuando) }))).collect())
        ))
        .collect::<serde_json::Map<_, _>>())
}

/// `GET /api/clientes/{c}/no-llega`: `{ "<equipo>": { "<identidad>": { motivo, por, cuando } } }`.
pub async fn ver(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    u.miembro(&st, &c, Rol::Lectura).await?;
    let m = st.db(move |db| leer(db, &c)).await?;
    Ok(Json(vista(&m)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Poner {
    equipo: String,
    consola: String,
    #[serde(default)]
    motivo: Option<String>,
    /// `true`: deshacer (vuelve a contar en el aviso).
    #[serde(default)]
    quitar: bool,
}

/// `PUT /api/clientes/{c}/no-llega` (administradores): pone o quita «no llega a esa consola».
pub async fn poner(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(p): Json<Poner>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    if !equipo_valido(&p.equipo) || !identidad_valida(&p.consola) {
        return Err(ErrorApi::datos("Equipo o consola no válidos."));
    }
    let motivo = motivo_valido(p.motivo.as_deref()).map_err(ErrorApi::datos)?;
    let (ctx2, e2) = (ctx.clone(), p.equipo.clone());
    if st.db(move |db| db.equipo(&ctx2, &e2)).await?.is_none() {
        return Err(ErrorApi::no_existe());
    }
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let marca = (!p.quitar).then(|| Marca { motivo: motivo.clone(), por: u.0.cuenta.nombre.clone(), cuando: ahora() });
    let m = st
        .db_crudo(move |db| {
            let mut m = leer(db, &c)?;
            let cambio = aplicar(&mut m, &p.equipo, &p.consola, marca).map_err(String::from)?;
            if cambio {
                db.poner_valor(&clave(&c), &serde_json::to_string(&m).map_err(|e| e.to_string())?)?;
                let accion = if p.quitar { "quitar_no_llega" } else { "marcar_no_llega" };
                db.auditar(&ctx, &actor, accion, &p.equipo, &json!({ "consola": p.consola, "motivo": motivo }).to_string())?;
            }
            Ok(m)
        })
        .await?
        .map_err(ErrorApi::datos)?;
    Ok(Json(vista(&m)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marca(motivo: Option<&str>) -> Marca {
        Marca { motivo: motivo.map(String::from), por: "Ana".into(), cuando: 1 }
    }

    #[test]
    fn poner_quitar_y_topes() {
        let mut m = Marcas::new();
        assert_eq!(aplicar(&mut m, "e1", "AAAA", Some(marca(Some("Sin internet")))), Ok(true));
        assert_eq!(aplicar(&mut m, "e1", "AAAA", Some(marca(Some("Sin internet")))), Ok(false), "igual: sin cambios");
        assert_eq!(aplicar(&mut m, "e1", "BBBB", Some(marca(None))), Ok(true));
        assert_eq!(m["e1"].len(), 2);
        assert_eq!(aplicar(&mut m, "e1", "AAAA", None), Ok(true));
        assert_eq!(aplicar(&mut m, "e1", "AAAA", None), Ok(false));
        assert_eq!(aplicar(&mut m, "e1", "BBBB", None), Ok(true));
        assert!(m.is_empty(), "sin marcas, sin equipo");
        for i in 0..MAX_MARCAS {
            aplicar(&mut m, &format!("e{i}"), "AAAA", Some(marca(None))).unwrap();
        }
        assert!(aplicar(&mut m, "otro", "AAAA", Some(marca(None))).is_err(), "tope");
        assert_eq!(aplicar(&mut m, "e1", "AAAA", Some(marca(Some("x")))), Ok(true), "cambiar una que ya está sí");
    }

    #[test]
    fn valida_lo_que_llega() {
        assert!(equipo_valido("3f2a9c1e-0b5c-4d1e-9a7f-2b6c8d0e1f3a"));
        assert!(!equipo_valido("../x") && !equipo_valido("") && !equipo_valido(&"e".repeat(129)));
        assert!(identidad_valida("q83vEjRWeJqrze8BI0VniQ+/=="));
        assert!(!identidad_valida("a b") && !identidad_valida(""));
        assert_eq!(motivo_valido(Some("  Red aislada  ")), Ok(Some("Red aislada".into())));
        assert_eq!(motivo_valido(Some("   ")), Ok(None));
        assert_eq!(motivo_valido(None), Ok(None));
        assert!(motivo_valido(Some("a\u{7}b")).is_err());
        assert!(motivo_valido(Some(&"m".repeat(201))).is_err());
    }
}
