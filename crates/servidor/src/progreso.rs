//! Progreso en vivo de lo que está en marcha en cada equipo (una copia, una
//! verificación…), de los agentes a la consola (docs/api-servidor.md, §6 y §8).
//!
//! Solo en memoria: el último mensaje de cada equipo, que caduca enseguida.
//! No se guarda en la base de datos ni pasa a los informes. El servidor lo
//! limpia (campos conocidos, textos cortos y sin controles, números finitos):
//! la consola nunca recibe nada que el agente no debería mandar.

use crate::almacen::Rol;
use crate::api::fecha;
use crate::auth::Usuario;
use crate::error::{ErrorApi, Res};
use crate::estado::St;
pub use crate::vivo::Paso;
use axum::extract::{Path, State};
use axum::Json;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Tareas por equipo, como mucho.
pub const MAX_TAREAS: usize = 8;
/// Tamaño máximo del mensaje (antes de limpiarlo).
pub const MAX_BYTES: usize = 16 * 1024;
/// Sin noticias en este tiempo, lo que estaba en marcha deja de enseñarse (el
/// agente lo manda cada 5 s por el canal, o cada 10 s por sondeo).
pub const CADUCA: Duration = Duration::from_secs(90);

const TIPOS: &[&str] = &["copia", "verificar", "verificar_externa", "copia_externa", "prueba_restauracion"];
const FASES: &[&str] = &["antes_de_copiar", "preparando", "escaneando", "subiendo", "terminando", "en_marcha"];
const TEXTOS: &[(&str, usize)] = &[("repo", 64), ("copia", 64), ("nombre", 120), ("etapa", 120), ("empezo", 40), ("actualizado", 40)];
// v1.36: `lectura` y `subida` (bytes/s medidos en el equipo) y `archivos_s`, para las gráficas en vivo.
const NUMEROS: &[&str] =
    &["archivos", "archivos_total", "bytes", "bytes_total", "velocidad", "lectura", "subida", "archivos_s", "quedan_s", "versiones", "versiones_total"];

struct Entrada {
    cliente: String,
    /// Hora (Unix) en que llegó, para la consola.
    recibido: i64,
    visto: Instant,
    tareas: Vec<Value>,
}

/// El último progreso de cada equipo.
#[derive(Default)]
pub struct Progresos(Mutex<HashMap<String, Entrada>>);

impl Progresos {
    /// Anota lo que está en marcha en un equipo; vacío, lo quita. Devuelve qué
    /// pasó, para el canal en vivo de las consolas (`None`: nada que contar).
    pub fn poner(&self, cliente: &str, equipo: &str, tareas: Vec<Value>) -> Option<Paso> {
        let mut m = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let ahora = Instant::now();
        let habia = m.get(equipo).filter(|e| e.cliente == cliente && ahora.duration_since(e.visto) < CADUCA).map(|e| e.tareas.clone());
        m.retain(|_, e| ahora.duration_since(e.visto) < CADUCA);
        if tareas.is_empty() {
            m.remove(equipo);
            habia.map(|_| Paso::Termina)
        } else {
            // Otra tarea (u otra lista) que la de antes: empieza algo; si no, solo cambian las cifras.
            let paso = match &habia {
                None => Paso::Empieza,
                Some(t) if claves(t) != claves(&tareas) => Paso::Empieza,
                Some(_) => Paso::Cambia,
            };
            m.insert(equipo.to_string(), Entrada { cliente: cliente.to_string(), recibido: crate::almacen::ahora(), visto: ahora, tareas });
            Some(paso)
        }
    }

    /// Quita lo que caducó (el agente dejó de contar) y dice de quién era: `[(cliente, equipo)]`.
    pub fn purgar(&self) -> Vec<(String, String)> {
        let mut m = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let ahora = Instant::now();
        let viejos: Vec<(String, String)> =
            m.iter().filter(|(_, e)| ahora.duration_since(e.visto) >= CADUCA).map(|(id, e)| (e.cliente.clone(), id.clone())).collect();
        for (_, id) in &viejos {
            m.remove(id);
        }
        viejos
    }

    /// Lo que está en marcha en los equipos de un cliente: `[(equipo, recibido, tareas)]`.
    pub fn de_cliente(&self, cliente: &str) -> Vec<(String, i64, Vec<Value>)> {
        let m = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let ahora = Instant::now();
        let mut v: Vec<_> = m
            .iter()
            .filter(|(_, e)| e.cliente == cliente && ahora.duration_since(e.visto) < CADUCA)
            .map(|(id, e)| (id.clone(), e.recibido, e.tareas.clone()))
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }
}

/// Qué tareas son (tipo, repositorio y copia), sin sus cifras.
fn claves(t: &[Value]) -> Vec<String> {
    t.iter().map(|x| format!("{}|{}|{}", x["tipo"].as_str().unwrap_or(""), x["repo"].as_str().unwrap_or(""), x["copia"].as_str().unwrap_or(""))).collect()
}

fn texto(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect()
}

/// Una tarea limpia: solo los campos conocidos, con su tipo. `None` si no vale.
fn limpiar_tarea(t: &Value) -> Option<Value> {
    let o = t.as_object()?;
    let tipo = o.get("tipo")?.as_str().filter(|x| TIPOS.contains(x))?;
    let fase = o.get("fase")?.as_str().filter(|x| FASES.contains(x))?;
    let mut out = Map::new();
    out.insert("tipo".into(), json!(tipo));
    out.insert("fase".into(), json!(fase));
    for (k, max) in TEXTOS {
        if let Some(s) = o.get(*k).and_then(Value::as_str) {
            out.insert((*k).into(), json!(texto(s, *max)));
        }
    }
    out.get("repo")?;
    for k in NUMEROS {
        if let Some(n) = o.get(*k).and_then(Value::as_u64) {
            out.insert((*k).into(), json!(n));
        }
    }
    if let Some(p) = o.get("porcentaje").and_then(Value::as_f64).filter(|p| p.is_finite()) {
        out.insert("porcentaje".into(), json!(p.clamp(0.0, 1.0)));
    }
    Some(Value::Object(out))
}

/// Las tareas de un mensaje del agente, limpias. Error si no es una lista o es demasiado grande.
pub fn limpiar(tareas: &Value) -> Res<Vec<Value>> {
    let lista = tareas.as_array().ok_or_else(|| ErrorApi::datos("Progreso no válido."))?;
    if lista.len() > MAX_TAREAS || tareas.to_string().len() > MAX_BYTES {
        return Err(ErrorApi::datos("Progreso demasiado grande."));
    }
    Ok(lista.iter().filter_map(limpiar_tarea).collect())
}

/// `GET /api/clientes/{c}/progreso`: lo que está en marcha ahora en sus equipos
/// (cualquier papel, como los informes). `[{ equipo, recibido, tareas }]`.
pub async fn ver(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Lectura).await?;
    let filas = st.progreso.de_cliente(ctx.id());
    if filas.is_empty() {
        return Ok(Json(json!([])));
    }
    // Solo los equipos que siguen en el cliente (uno que se movió a otro no se enseña aquí).
    let suyos: std::collections::HashSet<String> = st.db(move |db| Ok(db.equipos(&ctx)?.into_iter().map(|e| e.id).collect())).await?;
    Ok(Json(json!(filas
        .into_iter()
        .filter(|(e, _, _)| suyos.contains(e))
        .map(|(e, t, tareas)| json!({ "equipo": e, "recibido": fecha(t), "tareas": tareas }))
        .collect::<Vec<_>>())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limpia_y_acota() {
        let t = json!([{
            "tipo": "copia", "fase": "subiendo", "repo": "r1", "copia": "k1", "nombre": "Docu\u{0007}mentos",
            "porcentaje": 1.7, "archivos": 3, "bytes": -1, "ruta": "C:\\Users\\Ana", "velocidad": 2.5
        }, { "tipo": "otra", "fase": "subiendo", "repo": "r1" }, { "tipo": "copia", "fase": "subiendo" }]);
        let v = limpiar(&t).unwrap();
        assert_eq!(v.len(), 1, "sin tipo conocido o sin repositorio, fuera");
        let x = &v[0];
        assert_eq!(x["nombre"], "Documentos");
        assert_eq!(x["porcentaje"], 1.0);
        assert_eq!(x["archivos"], 3);
        assert!(x.get("ruta").is_none() && x.get("bytes").is_none() && x.get("velocidad").is_none());
        assert!(limpiar(&json!({})).is_err());
        // v1.36: las cifras de las gráficas pasan (enteros no negativos).
        let v = limpiar(&json!([{ "tipo": "copia", "fase": "subiendo", "repo": "r", "lectura": 1000, "subida": 400, "archivos_s": 12 }])).unwrap();
        assert_eq!((v[0]["lectura"].as_u64(), v[0]["subida"].as_u64(), v[0]["archivos_s"].as_u64()), (Some(1000), Some(400), Some(12)));
        assert!(limpiar(&json!(vec![json!({}); MAX_TAREAS + 1])).is_err());
        assert!(limpiar(&json!([{ "tipo": "copia", "fase": "subiendo", "repo": "r", "nombre": "x".repeat(MAX_BYTES) }])).is_err());
    }

    #[test]
    fn por_cliente_y_vacio_lo_quita() {
        let p = Progresos::default();
        let t = vec![json!({ "tipo": "copia", "fase": "subiendo", "repo": "r" })];
        assert_eq!(p.poner("c1", "e1", t.clone()), Some(Paso::Empieza));
        assert_eq!(p.poner("c1", "e1", t.clone()), Some(Paso::Cambia));
        p.poner("c2", "e2", t.clone());
        assert_eq!(p.de_cliente("c1").len(), 1);
        assert_eq!(p.de_cliente("c1")[0].0, "e1");
        assert_eq!(p.poner("c1", "e1", vec![]), Some(Paso::Termina));
        assert_eq!(p.poner("c1", "e1", vec![]), None, "lo que ya no estaba no termina otra vez");
        assert!(p.de_cliente("c1").is_empty());
        // Otra tarea en el mismo equipo: empieza algo nuevo.
        let otra = vec![json!({ "tipo": "verificar", "fase": "en_marcha", "repo": "r" })];
        assert_eq!(p.poner("c2", "e2", otra), Some(Paso::Empieza));
        assert!(p.purgar().is_empty(), "nada caducado aún");
        assert_eq!(p.de_cliente("c2").len(), 1);
    }
}
