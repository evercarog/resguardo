//! «Todos los clientes» (v1.38): lo de cada cliente del que la cuenta es
//! **miembro**, junto y en una sola petición, para el panel de la consola que
//! los enseña a la vez (en vez de N peticiones por cliente y equipo).
//!
//! - Solo los clientes de los que la cuenta es miembro (cualquier papel), con
//!   su papel: el propietario del servidor no ve los demás (para eso están las
//!   cifras de «Clientes del servidor», que no entran en los clientes).
//! - Tamaño acotado: como mucho [`MAX_CLIENTES`] clientes y, de cada equipo,
//!   su último informe **resumido** (las versiones y vueltas recientes, sin
//!   mensajes largos ni etiquetas), hasta [`MAX_BYTES_INFORMES`] en total.
//! - Lo de cada cliente se guarda [`CACHE`] en memoria (lo comparten sus
//!   miembros); la pertenencia se comprueba siempre en el momento.

use super::{equipo_json, fecha};
use crate::almacen::{ahora, ClienteCtx, Rol};
use crate::auth::Usuario;
use crate::error::Res;
use crate::estado::St;
use axum::extract::State;
use axum::Json;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

/// Clientes por respuesta, como mucho (por nombre; los demás se cuentan en `omitidos`).
pub const MAX_CLIENTES: usize = 100;
/// Versiones de cada repositorio en el informe resumido (las más recientes).
pub const MAX_VERSIONES: usize = 60;
/// Vueltas de cada repositorio en el informe resumido (de los últimos [`DIAS_VUELTAS`] días).
pub const MAX_VUELTAS: usize = 60;
pub const DIAS_VUELTAS: i64 = 15;
/// Bytes de todos los informes resumidos juntos; lo que no cabe se queda fuera (`informes_completos: false`).
pub const MAX_BYTES_INFORMES: usize = 4 * 1024 * 1024;
/// Lo que se guarda lo de cada cliente.
pub const CACHE: Duration = Duration::from_secs(5);

/// Lo de un cliente, ya en JSON (sin su papel: ese es de cada cuenta).
struct Foto {
    hecha: Instant,
    marca: Value,
    equipos: Vec<Value>,
    ids: HashSet<String>,
    avisos: usize,
    pendientes: usize,
    /// `(equipo, { equipo, recibido, datos })`, con su tamaño.
    informes: Vec<(Value, usize)>,
}

static FOTOS: LazyLock<Mutex<HashMap<String, Arc<Foto>>>> = LazyLock::new(Default::default);

fn foto_guardada(cliente: &str) -> Option<Arc<Foto>> {
    let m = FOTOS.lock().unwrap_or_else(|e| e.into_inner());
    m.get(cliente).filter(|f| f.hecha.elapsed() < CACHE).cloned()
}

fn guardar_foto(cliente: &str, f: Arc<Foto>) {
    let mut m = FOTOS.lock().unwrap_or_else(|e| e.into_inner());
    // Las caducadas fuera siempre (no solo pasadas 1000): cada una puede llevar hasta 4 MB
    // de informes, y la de cada cliente que alguien miró una vez se quedaba en memoria.
    m.retain(|_, f| f.hecha.elapsed() < CACHE);
    m.insert(cliente.to_string(), f);
}

/// Un texto corto (los mensajes de las vueltas pueden ser largos).
fn corto(v: Option<&Value>, max: usize) -> Value {
    match v.and_then(Value::as_str) {
        Some(s) if s.chars().count() > max => json!(format!("{}…", s.chars().take(max).collect::<String>())),
        Some(s) => json!(s),
        None => Value::Null,
    }
}

fn campos(o: &Map<String, Value>, ks: &[&str]) -> Map<String, Value> {
    ks.iter().filter_map(|k| o.get(*k).map(|v| ((*k).to_string(), v.clone()))).collect()
}

/// El informe resumido de un equipo: lo que necesita el panel (estado, cuadros
/// de 14 días, cifras de 24 h, lo que ocupa cada destino y a qué ritmo crece).
pub fn resumir_informe(datos: &Value, ahora: i64) -> Value {
    let Some(o) = datos.as_object() else { return json!({}) };
    let mut out = campos(o, &["version", "proximas", "disco"]);
    if let Some(copias) = o.get("copias").and_then(Value::as_array) {
        let cs: Vec<Value> = copias
            .iter()
            .take(50)
            .filter_map(Value::as_object)
            .map(|k| {
                let mut x = campos(k, &["id", "repo", "nombre", "estado", "cuando", "bytes", "archivos", "duracion_s"]);
                x.insert("mensaje".into(), corto(k.get("mensaje"), 200));
                Value::Object(x)
            })
            .collect();
        out.insert("copias".into(), json!(cs));
    }
    let desde = ahora - DIAS_VUELTAS * 86_400;
    if let Some(repos) = o.get("repos").and_then(Value::as_array) {
        let rs: Vec<Value> = repos
            .iter()
            .take(50)
            .filter_map(Value::as_object)
            .map(|r| {
                let mut x =
                    campos(r, &["id", "nombre", "solo_lectura", "versiones_leidas", "espacio", "verificacion", "prueba_restauracion", "externa", "proteccion"]);
                let versiones = r.get("versiones").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
                let vs: Vec<Value> = versiones
                    .iter()
                    .take(MAX_VERSIONES)
                    .filter_map(Value::as_object)
                    .map(|v| {
                        let mut y = campos(v, &["id", "hora", "copia", "total_bytes", "anadido", "anadido_empaquetado"]);
                        y.insert("etiquetas".into(), json!([]));
                        Value::Object(y)
                    })
                    .collect();
                let vueltas = r.get("ejecuciones").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
                let es: Vec<Value> = vueltas
                    .iter()
                    .filter_map(Value::as_object)
                    .filter(|e| e.get("hora").and_then(Value::as_str).and_then(super::de_fecha).is_some_and(|t| t >= desde))
                    .take(MAX_VUELTAS)
                    .map(|e| {
                        let mut y = campos(e, &["hora", "copia", "resultado", "duracion_s", "anadido"]);
                        y.insert("mensaje_corto".into(), corto(e.get("mensaje_corto"), 160));
                        Value::Object(y)
                    })
                    .collect();
                let recortado = vs.len() < versiones.len() || es.len() < vueltas.len() || r.get("recortado").and_then(Value::as_bool) == Some(true);
                x.insert("versiones".into(), json!(vs));
                x.insert("ejecuciones".into(), json!(es));
                if recortado {
                    x.insert("recortado".into(), json!(true));
                }
                for k in ["versiones_leidas", "espacio", "verificacion", "prueba_restauracion", "externa", "proteccion"] {
                    x.entry(k).or_insert(Value::Null);
                }
                Value::Object(x)
            })
            .collect();
        out.insert("repos".into(), json!(rs));
    }
    Value::Object(out)
}

/// Lo de cada cliente (de la memoria si es reciente; si no, de su base de datos).
async fn fotos(st: &St, clientes: Vec<String>) -> Res<HashMap<String, Arc<Foto>>> {
    let mut out = HashMap::new();
    let mut faltan = Vec::new();
    for c in clientes {
        match foto_guardada(&c) {
            Some(f) => {
                out.insert(c, f);
            }
            None => faltan.push(c),
        }
    }
    if faltan.is_empty() {
        return Ok(out);
    }
    let t = ahora();
    let leidos = st
        .db(move |db| {
            let mut v = Vec::new();
            for c in faltan {
                let ctx = ClienteCtx::autorizado(&c);
                let equipos = db.equipos(&ctx)?;
                let mut informes = Vec::new();
                for e in &equipos {
                    if let Some((r, d)) = db.informes(&ctx, &e.id, 1)?.into_iter().next() {
                        let x = json!({ "equipo": e.id, "recibido": fecha(r), "datos": resumir_informe(&d, t) });
                        let n = x.to_string().len();
                        informes.push((x, n));
                    }
                }
                let avisos = db.avisos(&ctx, true)?.len();
                // v1.4x: también las que mandó otra consola y el equipo tiene en espera.
                let pendientes = db.ordenes_con_espera(&ctx, t)?.len() + super::en_espera_de_otras(&equipos, t);
                let marca = super::marca::json(&c, &super::marca::leer(db, &c)?);
                v.push((c, equipos, avisos, pendientes, informes, marca));
            }
            Ok(v)
        })
        .await?;
    for (c, equipos, avisos, pendientes, informes, marca) in leidos {
        let f = Arc::new(Foto {
            hecha: Instant::now(),
            marca,
            ids: equipos.iter().map(|e| e.id.clone()).collect(),
            equipos: equipos.iter().map(|e| equipo_json(e, st.conectado(&e.id))).collect(),
            avisos,
            pendientes,
            informes,
        });
        guardar_foto(&c, f.clone());
        out.insert(c, f);
    }
    Ok(out)
}

/// Los clientes de la cuenta, por nombre: `(id, nombre, rol)` y cuántos quedan fuera.
async fn mis_clientes(st: &St, u: &Usuario) -> Res<(Vec<(String, String, Rol)>, usize)> {
    let id = u.id().to_string();
    let mut cs: Vec<_> = st.db(move |db| db.clientes_de(&id)).await?.into_iter().map(|(c, r)| (c.id, c.nombre, r)).collect();
    cs.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()).then_with(|| a.0.cmp(&b.0)));
    let omitidos = cs.len().saturating_sub(MAX_CLIENTES);
    cs.truncate(MAX_CLIENTES);
    Ok((cs, omitidos))
}

/// Lo que está en marcha en los equipos (que siguen en su cliente) de esos clientes.
fn progreso_de(st: &St, cliente: &str, ids: &HashSet<String>) -> Vec<Value> {
    st.progreso
        .de_cliente(cliente)
        .into_iter()
        .filter(|(e, _, _)| ids.contains(e))
        .map(|(e, t, tareas)| json!({ "cliente": cliente, "equipo": e, "recibido": fecha(t), "tareas": tareas }))
        .collect()
}

/// `GET /api/panel`: lo de todos los clientes de la cuenta (ver arriba).
pub async fn ver(State(st): State<St>, u: Usuario) -> Res<Json<Value>> {
    let (cs, omitidos) = mis_clientes(&st, &u).await?;
    let fotos = fotos(&st, cs.iter().map(|c| c.0.clone()).collect()).await?;
    let mut quedan = MAX_BYTES_INFORMES;
    let mut clientes = Vec::new();
    let mut progreso = Vec::new();
    for (id, nombre, rol) in &cs {
        let Some(f) = fotos.get(id) else { continue };
        let mut informes = Vec::new();
        let mut completos = true;
        for (x, n) in &f.informes {
            if *n <= quedan {
                quedan -= n;
                informes.push(x.clone());
            } else {
                completos = false;
            }
        }
        progreso.extend(progreso_de(&st, id, &f.ids));
        clientes.push(json!({
            "id": id, "nombre": nombre, "rol": rol.texto(), "marca": f.marca,
            "equipos": f.equipos, "avisos_abiertos": f.avisos, "pendientes": f.pendientes,
            "informes": informes, "informes_completos": completos,
        }));
    }
    Ok(Json(json!({ "generado": fecha(ahora()), "clientes": clientes, "omitidos": omitidos, "progreso": progreso })))
}

/// `GET /api/panel/progreso`: solo lo que está en marcha ahora, de todos los clientes de la cuenta.
pub async fn progreso(State(st): State<St>, u: Usuario) -> Res<Json<Value>> {
    let (cs, _) = mis_clientes(&st, &u).await?;
    // Solo se mira la base de datos de los clientes con algo en marcha.
    let con_algo: Vec<String> = cs.iter().filter(|c| !st.progreso.de_cliente(&c.0).is_empty()).map(|c| c.0.clone()).collect();
    let fotos = fotos(&st, con_algo.clone()).await?;
    let out: Vec<Value> = con_algo.iter().filter_map(|c| fotos.get(c).map(|f| progreso_de(&st, c, &f.ids))).flatten().collect();
    Ok(Json(json!(out)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lo guardado de un cliente que ya nadie mira no se queda en memoria.
    #[test]
    fn las_fotos_caducadas_no_se_quedan() {
        let foto = |hace: u64| {
            Arc::new(Foto {
                hecha: Instant::now().checked_sub(Duration::from_secs(hace)).unwrap_or_else(Instant::now),
                marca: Value::Null,
                equipos: Vec::new(),
                ids: HashSet::new(),
                avisos: 0,
                pendientes: 0,
                informes: vec![(json!({ "datos": "x".repeat(1000) }), 1000)],
            })
        };
        guardar_foto("prueba-fotos-vieja", foto(60));
        guardar_foto("prueba-fotos-nueva", foto(0));
        let m = FOTOS.lock().unwrap();
        assert!(!m.contains_key("prueba-fotos-vieja") && m.contains_key("prueba-fotos-nueva"));
    }

    #[test]
    fn el_informe_resumido_es_corto() {
        let ahora = 1_800_000_000;
        let hora = |dias: i64| fecha(ahora - dias * 86_400);
        let versiones: Vec<Value> = (0..200)
            .map(|i| json!({ "id": format!("{i:08x}"), "hora": hora(i / 4), "copia": "k", "total_bytes": 10, "anadido": 1, "anadido_empaquetado": 1, "archivos_nuevos": 3, "etiquetas": ["x"] }))
            .collect();
        let vueltas: Vec<Value> = (0..40).map(|i| json!({ "hora": hora(i), "copia": "k", "resultado": "ok", "mensaje_corto": "m".repeat(400) })).collect();
        let d = json!({
            "version": "0.7.14", "progreso": [{ "tipo": "copia" }], "otra": "no",
            "copias": [{ "id": "k", "estado": "fallo", "mensaje": "x".repeat(1000), "ganchos": [{}] }],
            "repos": [{ "id": "r", "nombre": "Docs", "versiones": versiones, "ejecuciones": vueltas, "espacio": { "en_disco_bytes": 5 } }],
        });
        let r = resumir_informe(&d, ahora);
        assert!(r.get("progreso").is_none() && r.get("otra").is_none());
        assert!(r["copias"][0].get("ganchos").is_none());
        assert_eq!(r["copias"][0]["mensaje"].as_str().unwrap().chars().count(), 201);
        let repo = &r["repos"][0];
        assert_eq!(repo["versiones"].as_array().unwrap().len(), MAX_VERSIONES);
        assert!(repo["versiones"][0].get("archivos_nuevos").is_none());
        assert_eq!(repo["versiones"][0]["etiquetas"], json!([]));
        assert_eq!(repo["ejecuciones"].as_array().unwrap().len(), DIAS_VUELTAS as usize + 1, "solo las de los últimos días");
        assert_eq!(repo["recortado"], true);
        assert_eq!(repo["espacio"]["en_disco_bytes"], 5);
        assert!(repo["verificacion"].is_null());
        assert!(r.to_string().len() < 20_000);
        assert_eq!(resumir_informe(&json!("no"), ahora), json!({}));
    }
}
