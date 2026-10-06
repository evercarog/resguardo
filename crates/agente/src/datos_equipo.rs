//! v1.4x: el nombre, las etiquetas y la observación del equipo, iguales en todas
//! sus consolas (docs/consolas-multiples.md §6).
//!
//! Cada consola (cada Resguardo Server) guarda lo suyo: si se cambiaba el nombre
//! en una, en la otra seguía el de antes. Ahora el equipo guarda el valor
//! **canónico** de cada uno de estos datos, que llega con una orden inofensiva de
//! cualquiera de sus consolas (`nombre_equipo`, `etiquetas_equipo`,
//! `observacion_equipo`), lo anota en su historial (el que reciben todas) con la
//! consola que lo cambió y lo dice en su resumen (`datos_equipo`). Cada consola
//! enseña el valor del equipo.
//!
//! Convivencia: mientras un dato no se haya puesto con su orden, el equipo no
//! dice nada de él y cada consola sigue con el suyo (no se pisa el nombre que ya
//! tenía cada una al actualizar el agente). Solo son metadatos (como los ve hoy
//! cualquier servidor): no hay secretos ni rutas, y no reducen la protección.

use crate::servidor_v2::Vinculo;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Como en el servidor (`PATCH …/equipos/{e}`): hasta 80 caracteres.
pub const MAX_NOMBRE: usize = 80;
/// Como en el servidor (`PUT …/etiquetas`): hasta 10 etiquetas de 1 a 32 caracteres.
pub const MAX_ETIQUETAS: usize = 10;
pub const MAX_LARGO_ETIQUETA: usize = 32;
/// Como las observaciones del servidor (v1.40): hasta 2000 caracteres.
pub const MAX_OBSERVACION: usize = 2000;

/// Los datos que se comparten (la clave en `Vinculo::datos_equipo` y en el resumen).
pub const NOMBRE: &str = "nombre";
pub const ETIQUETAS: &str = "etiquetas";
pub const OBSERVACION: &str = "observacion";

/// Un dato del equipo y quién lo cambió por última vez.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Campo {
    /// El valor: un texto (nombre, observación; `""` es «sin observación») o una lista (etiquetas).
    pub valor: Value,
    /// Cuándo (RFC 3339).
    pub cuando: String,
    /// El nombre que esa consola tiene en el equipo (nunca su dirección).
    #[serde(default)]
    pub consola: String,
    /// Su identidad (para saber si lo cambió la que recibe el resumen).
    #[serde(default)]
    pub identidad: String,
    /// Quién lo pidió, si la consola lo dice (informativo, como en las órdenes en espera).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub por: Option<String>,
}

/// El nombre, limpio: sin espacios en los extremos, de 1 a 80 caracteres y sin
/// caracteres de control.
pub fn limpiar_nombre(n: &str) -> Result<String, String> {
    let n = n.trim();
    if n.is_empty() || n.chars().count() > MAX_NOMBRE || n.chars().any(char::is_control) {
        return Err(format!("Escribe un nombre (hasta {MAX_NOMBRE} caracteres, sin caracteres de control)."));
    }
    Ok(n.to_string())
}

/// Las etiquetas, limpias como en el servidor: sin espacios de más, sin repetir
/// (sin distinguir mayúsculas), de 1 a 32 caracteres, sin comas ni caracteres de
/// control, hasta 10.
pub fn limpiar_etiquetas(xs: &[Value]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for x in xs {
        let Some(x) = x.as_str() else { return Err("Las etiquetas son textos.".into()) };
        let t = x.split_whitespace().collect::<Vec<_>>().join(" ");
        if t.is_empty() {
            continue;
        }
        if t.chars().count() > MAX_LARGO_ETIQUETA || t.chars().any(|c| c.is_control() || c == ',') {
            return Err(format!("Etiqueta no válida (hasta {MAX_LARGO_ETIQUETA} caracteres, sin comas)."));
        }
        if !out.iter().any(|o| o.to_lowercase() == t.to_lowercase()) {
            out.push(t);
        }
    }
    if out.len() > MAX_ETIQUETAS {
        return Err(format!("Como mucho {MAX_ETIQUETAS} etiquetas por equipo."));
    }
    Ok(out)
}

/// La observación, limpia como en el servidor: saltos de línea `\n`, sin espacios
/// en los extremos, sin caracteres de control (salvo salto de línea y tabulador),
/// hasta 2000 caracteres. Vacía: sin observación.
pub fn limpiar_observacion(t: &str) -> Result<String, String> {
    let t = t.replace("\r\n", "\n").replace('\r', "\n");
    let t = t.trim().to_string();
    if t.chars().count() > MAX_OBSERVACION {
        return Err(format!("Como mucho {MAX_OBSERVACION} caracteres."));
    }
    if t.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
        return Err("El texto lleva caracteres no válidos.".into());
    }
    Ok(t)
}

fn corto(t: &str, max: usize) -> String {
    t.chars().take(max).collect()
}

/// Guarda el dato `clave` con lo que pide la orden (`cuerpo`), de la consola activa de `v`.
fn poner(v: &mut Vinculo, clave: &str, valor: Value, por: Option<&str>) {
    v.datos_equipo.insert(
        clave.to_string(),
        Campo {
            valor,
            cuando: chrono::Local::now().to_rfc3339(),
            consola: corto(&crate::consolas_v2::nombre_de(&v.nombre_consola, ""), 60),
            identidad: v.identidad.clone(),
            por: por.map(|p| corto(p.trim(), 60)).filter(|p| !p.is_empty()),
        },
    );
}

/// `nombre_equipo { nombre }`: el nombre del equipo en todas sus consolas.
pub fn nombre(v: &mut Vinculo, c: &Value, por: Option<&str>) -> Result<String, String> {
    let n = limpiar_nombre(c["nombre"].as_str().ok_or("Falta el nombre.")?)?;
    poner(v, NOMBRE, json!(n), por);
    Ok(format!("Nombre del equipo: «{n}». Lo verán así todas sus consolas."))
}

/// `etiquetas_equipo { etiquetas }`: las etiquetas del equipo en todas sus consolas.
pub fn etiquetas(v: &mut Vinculo, c: &Value, por: Option<&str>) -> Result<String, String> {
    let xs = limpiar_etiquetas(c["etiquetas"].as_array().ok_or("Faltan las etiquetas (una lista, vacía para quitarlas).")?)?;
    let m = if xs.is_empty() {
        "Etiquetas quitadas en todas sus consolas.".to_string()
    } else {
        format!("Etiquetas: {}. Las verán así todas sus consolas.", xs.join(", "))
    };
    poner(v, ETIQUETAS, json!(xs), por);
    Ok(m)
}

/// `observacion_equipo { texto }`: la observación del equipo en todas sus consolas (vacía, se quita).
pub fn observacion(v: &mut Vinculo, c: &Value, por: Option<&str>) -> Result<String, String> {
    let t = limpiar_observacion(c["texto"].as_str().ok_or("Falta el texto (vacío para quitarla).")?)?;
    let m = if t.is_empty() { "Observación quitada en todas sus consolas." } else { "Observación guardada: la verán todas sus consolas." };
    poner(v, OBSERVACION, json!(t), por);
    Ok(m.into())
}

/// La descripción para el historial común (sin la observación: puede ser larga).
pub fn descripcion(tipo: &str, c: &Value) -> Option<String> {
    Some(match tipo {
        "nombre_equipo" => format!("Cambiar el nombre del equipo a «{}»", corto(c["nombre"].as_str().unwrap_or("").trim(), 80)),
        "etiquetas_equipo" => {
            let xs: Vec<&str> = c["etiquetas"].as_array().map(|l| l.iter().filter_map(Value::as_str).take(10).collect()).unwrap_or_default();
            if xs.is_empty() {
                "Quitar las etiquetas del equipo".to_string()
            } else {
                format!("Poner las etiquetas del equipo: {}", corto(&xs.join(", "), 120))
            }
        }
        "observacion_equipo" => "Cambiar la observación del equipo".to_string(),
        _ => return None,
    })
}

/// `resumen.datos_equipo`: lo que el equipo tiene puesto con las órdenes (solo eso;
/// `null` si nada). `consola` es el nombre que esa consola tiene en el equipo, nunca
/// su dirección; `esta: true` si lo cambió la consola que recibe el resumen.
pub fn resumen(v: &Vinculo) -> Value {
    if v.datos_equipo.is_empty() {
        return Value::Null;
    }
    let m: serde_json::Map<String, Value> = v
        .datos_equipo
        .iter()
        .filter(|(k, _)| [NOMBRE, ETIQUETAS, OBSERVACION].contains(&k.as_str()))
        .map(|(k, c)| {
            (
                k.clone(),
                json!({
                    "valor": c.valor, "cuando": c.cuando,
                    "consola": (!c.consola.is_empty()).then_some(&c.consola),
                    "esta": !c.identidad.is_empty() && c.identidad == v.identidad,
                    "por": c.por,
                }),
            )
        })
        .collect();
    Value::Object(m)
}

/// Para las pruebas y para quien quiera leerlo: el valor de un dato (si se puso).
pub fn valor<'a>(v: &'a Vinculo, clave: &str) -> Option<&'a Value> {
    v.datos_equipo.get(clave).map(|c| &c.valor)
}

/// Mapa vacío (para `#[serde(default)]`).
pub type Datos = BTreeMap<String, Campo>;

#[cfg(test)]
mod tests {
    use super::*;

    fn vinculo() -> Vinculo {
        Vinculo { identidad: "id-oficina".into(), nombre_consola: "Oficina".into(), ..Default::default() }
    }

    #[test]
    fn nombre_limpio_y_con_su_consola() {
        let mut v = vinculo();
        assert!(nombre(&mut v, &json!({ "nombre": "  " }), None).is_err());
        assert!(nombre(&mut v, &json!({ "nombre": "a".repeat(81) }), None).is_err());
        assert!(nombre(&mut v, &json!({ "nombre": "PC\u{7}" }), None).is_err());
        assert!(nombre(&mut v, &json!({}), None).is_err());
        assert!(v.datos_equipo.is_empty(), "lo que no vale no se guarda");
        let m = nombre(&mut v, &json!({ "nombre": "  Recepción 2 " }), Some("Ana")).unwrap();
        assert!(m.contains("Recepción 2"));
        assert_eq!(valor(&v, NOMBRE), Some(&json!("Recepción 2")));
        let c = &v.datos_equipo[NOMBRE];
        assert_eq!((c.consola.as_str(), c.identidad.as_str(), c.por.as_deref()), ("Oficina", "id-oficina", Some("Ana")));
        assert!(chrono::DateTime::parse_from_rfc3339(&c.cuando).is_ok());
    }

    #[test]
    fn etiquetas_como_en_el_servidor() {
        let mut v = vinculo();
        etiquetas(&mut v, &json!({ "etiquetas": ["  Contabilidad ", "contabilidad", "Sede  norte", ""] }), None).unwrap();
        assert_eq!(valor(&v, ETIQUETAS), Some(&json!(["Contabilidad", "Sede norte"])));
        assert!(etiquetas(&mut v, &json!({ "etiquetas": ["a,b"] }), None).is_err());
        assert!(etiquetas(&mut v, &json!({ "etiquetas": [3] }), None).is_err());
        assert!(etiquetas(&mut v, &json!({ "etiquetas": (0..11).map(|i| format!("e{i}")).collect::<Vec<_>>() }), None).is_err());
        assert!(etiquetas(&mut v, &json!({ "etiquetas": "x" }), None).is_err());
        // Vacía: se quitan en todas (y eso también es un valor que se comparte).
        etiquetas(&mut v, &json!({ "etiquetas": [] }), None).unwrap();
        assert_eq!(valor(&v, ETIQUETAS), Some(&json!([])));
    }

    #[test]
    fn observacion_limpia_y_vacia_la_quita() {
        let mut v = vinculo();
        observacion(&mut v, &json!({ "texto": " Cambiado el disco\r\nel 3/10 " }), None).unwrap();
        assert_eq!(valor(&v, OBSERVACION), Some(&json!("Cambiado el disco\nel 3/10")));
        assert!(observacion(&mut v, &json!({ "texto": "ñ".repeat(MAX_OBSERVACION + 1) }), None).is_err());
        assert!(observacion(&mut v, &json!({ "texto": "a\u{7}" }), None).is_err());
        observacion(&mut v, &json!({ "texto": "" }), None).unwrap();
        assert_eq!(valor(&v, OBSERVACION), Some(&json!("")));
    }

    #[test]
    fn resumen_solo_lo_puesto_y_sin_direcciones() {
        let mut v = vinculo();
        assert_eq!(resumen(&v), Value::Null, "sin nada puesto, cada consola sigue con lo suyo");
        v.url = "https://oficina.ejemplo.com".into();
        nombre(&mut v, &json!({ "nombre": "Recepción" }), Some("Ana")).unwrap();
        let r = resumen(&v);
        assert_eq!(r["nombre"]["valor"], "Recepción");
        assert_eq!(r["nombre"]["consola"], "Oficina");
        assert_eq!(r["nombre"]["esta"], true);
        assert_eq!(r["nombre"]["por"], "Ana");
        assert!(r.get("etiquetas").is_none());
        assert!(!r.to_string().contains("ejemplo.com"), "nunca la dirección de una consola");
        // Visto desde otra consola: `esta` es falso.
        v.identidad = "id-en-linea".into();
        assert_eq!(resumen(&v)["nombre"]["esta"], false);
    }

    #[test]
    fn persiste_en_el_vinculo_y_un_archivo_anterior_no_tiene_nada() {
        let mut v = vinculo();
        nombre(&mut v, &json!({ "nombre": "Almacén norte" }), None).unwrap();
        let guardado = serde_json::to_string(&v).unwrap();
        let leido: Vinculo = serde_json::from_str(&guardado).unwrap();
        assert_eq!(leido.datos_equipo, v.datos_equipo);
        // Un vínculo de un agente anterior (sin el campo) se lee vacío.
        let mut viejo = serde_json::to_value(vinculo()).unwrap();
        viejo.as_object_mut().unwrap().remove("datos_equipo");
        let leido: Vinculo = serde_json::from_value(viejo).unwrap();
        assert!(leido.datos_equipo.is_empty());
    }

    #[test]
    fn descripcion_para_el_historial() {
        assert_eq!(descripcion("nombre_equipo", &json!({ "nombre": "Recepción" })).unwrap(), "Cambiar el nombre del equipo a «Recepción»");
        assert_eq!(descripcion("etiquetas_equipo", &json!({ "etiquetas": [] })).unwrap(), "Quitar las etiquetas del equipo");
        assert!(descripcion("observacion_equipo", &json!({ "texto": "secreto" })).unwrap().find("secreto").is_none(), "sin el texto");
        assert!(descripcion("copiar_ahora", &json!({})).is_none());
    }
}
