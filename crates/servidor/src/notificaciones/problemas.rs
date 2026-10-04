//! Qué falla en un equipo, según lo que cuenta: el informe (copias,
//! verificaciones, copia externa y pruebas de restauración) y el resumen (el
//! espejo del almacén). Lo que está bien también cuenta: es lo que cierra un
//! incidente con «Volvió a funcionar».

use crate::almacen::Ts;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::hash::{Hash, Hasher};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Fuente {
    Informe,
    Resumen,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Problema {
    pub tipo: String,
    /// La copia o el repositorio (vacío si es del equipo entero).
    pub sujeto: String,
    pub nombre: String,
    pub mensaje: String,
    /// Lo que distingue una vuelta de otra (su hora, tal como la cuenta el equipo).
    pub marca: String,
    pub cuando: Option<Ts>,
}

/// Los tipos que salen de cada fuente (los que esa fuente puede dar por arreglados).
pub fn tipos(f: Fuente) -> &'static [&'static str] {
    match f {
        Fuente::Informe => &["copia_fallida", "verificacion_fallida", "externa_fallida", "prueba_fallida"],
        Fuente::Resumen => &["espejo_fallido"],
    }
}

const MAX_PROBLEMAS: usize = 200;

fn corto(v: &Value, max: usize) -> String {
    v.as_str().unwrap_or_default().chars().filter(|c| !c.is_control()).take(max).collect()
}

fn id_ok(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// (Problemas, sanos): los sanos van como `tipo|sujeto`.
pub fn de_informe(datos: &Value) -> (Vec<Problema>, Vec<String>) {
    let (mut p, mut sanos) = (Vec::new(), Vec::new());
    let mut mirar = |tipo: &str, sujeto: &str, nombre: String, v: &Value, hora: &str, mensaje: &str| {
        if !id_ok(sujeto) || p.len() >= MAX_PROBLEMAS {
            return;
        }
        match v.get("resultado").or_else(|| v.get("estado")).and_then(Value::as_str) {
            Some("fallo") => {
                let marca = corto(&v[hora], 40);
                p.push(Problema {
                    tipo: tipo.into(),
                    sujeto: sujeto.into(),
                    nombre,
                    mensaje: corto(&v[mensaje], 300),
                    cuando: crate::api::de_fecha(&marca),
                    marca,
                });
            }
            Some("ok" | "aviso" | "sin_cambios") => sanos.push(format!("{tipo}|{sujeto}")),
            _ => {}
        }
    };
    for c in datos["copias"].as_array().into_iter().flatten() {
        let id = corto(&c["id"], 64);
        let nombre = Some(corto(&c["nombre"], 80)).filter(|n| !n.is_empty()).unwrap_or_else(|| id.clone());
        mirar("copia_fallida", &id, nombre, c, "cuando", "mensaje");
    }
    for r in datos["repos"].as_array().into_iter().flatten() {
        let id = corto(&r["id"], 64);
        let nombre = Some(corto(&r["nombre"], 80)).filter(|n| !n.is_empty()).unwrap_or_else(|| id.clone());
        for (campo, tipo) in [("verificacion", "verificacion_fallida"), ("externa", "externa_fallida"), ("prueba_restauracion", "prueba_fallida")] {
            if r[campo].is_object() {
                mirar(tipo, &id, nombre.clone(), &r[campo], "ultima", "mensaje_corto");
            }
        }
    }
    (p, sanos)
}

/// El espejo del almacén (`guarda_copias.espejo`): `resultado` empieza por `ERROR` si falló.
pub fn de_resumen(resumen: &Value) -> (Vec<Problema>, Vec<String>) {
    let e = &resumen["guarda_copias"]["espejo"];
    let resultado = corto(&e["resultado"], 300);
    if !e.is_object() || resultado.is_empty() {
        return (Vec::new(), Vec::new());
    }
    match resultado.strip_prefix("ERROR") {
        Some(resto) => {
            let marca = corto(&e["ultima"], 40);
            let p = Problema {
                tipo: "espejo_fallido".into(),
                sujeto: String::new(),
                nombre: String::new(),
                mensaje: resto.trim_start_matches([':', ' ']).to_string(),
                cuando: crate::api::de_fecha(&marca),
                marca,
            };
            (vec![p], Vec::new())
        }
        None => (Vec::new(), vec!["espejo_fallido|".into()]),
    }
}

/// Para no apuntar lo mismo con cada informe.
pub fn huella(p: &[Problema], sanos: &[String]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for x in p {
        (&x.tipo, &x.sujeto, &x.marca).hash(&mut h);
    }
    0u8.hash(&mut h);
    sanos.hash(&mut h);
    h.finish()
}

/// De qué se habla: «la copia «Documentos» en «PC-Ana»».
fn sujeto(tipo: &str, nombre: &str, equipo: &str) -> String {
    match tipo {
        "copia_fallida" => format!("la copia «{nombre}» en «{equipo}»"),
        "verificacion_fallida" => format!("la verificación de «{nombre}» en «{equipo}»"),
        "externa_fallida" => format!("la copia externa de «{nombre}» en «{equipo}»"),
        "prueba_fallida" => format!("la prueba de restauración de «{nombre}» en «{equipo}»"),
        "espejo_fallido" => format!("el espejo de «{equipo}»"),
        _ => format!("algo en «{equipo}»"),
    }
}

/// (Cuando falla, cuando se arregla.)
pub fn titulos(p: &Problema, equipo: &str) -> (String, String) {
    let s = sujeto(&p.tipo, &p.nombre, equipo);
    (format!("Falló {s}"), format!("Volvió a funcionar {s}"))
}

/// El título de «arreglado» a partir del de «falló».
pub fn titulo_ok(titulo: &str) -> String {
    match titulo.strip_prefix("Falló ") {
        Some(s) => format!("Volvió a funcionar {s}"),
        None => format!("Resuelto: {titulo}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn del_informe() {
        let informe = json!({
            "copias": [
                { "id": "c1", "nombre": "Documentos", "estado": "fallo", "cuando": "2026-10-04T10:00:00+02:00", "mensaje": "repositorio bloqueado" },
                { "id": "c2", "nombre": "Correo", "estado": "ok", "cuando": "2026-10-04T10:00:00+02:00" },
                { "id": "../x", "estado": "fallo" }
            ],
            "repos": [{ "id": "r1", "nombre": "Oficina",
                        "verificacion": { "ultima": "2026-10-03T03:00:00+02:00", "resultado": "fallo", "mensaje_corto": "datos dañados" },
                        "externa": { "ultima": "2026-10-03T03:00:00+02:00", "resultado": "ok" },
                        "prueba_restauracion": null }]
        });
        let (p, sanos) = de_informe(&informe);
        assert_eq!(p.len(), 2);
        assert_eq!((p[0].tipo.as_str(), p[0].sujeto.as_str(), p[0].nombre.as_str()), ("copia_fallida", "c1", "Documentos"));
        assert_eq!(p[0].cuando, crate::api::de_fecha("2026-10-04T10:00:00+02:00"));
        assert!(p[0].cuando.is_some());
        assert_eq!(p[1].tipo, "verificacion_fallida");
        assert_eq!(sanos, vec!["copia_fallida|c2".to_string(), "externa_fallida|r1".to_string()]);
        let (t, ok) = titulos(&p[0], "PC-Ana");
        assert_eq!(t, "Falló la copia «Documentos» en «PC-Ana»");
        assert_eq!(ok, "Volvió a funcionar la copia «Documentos» en «PC-Ana»");
        assert_eq!(titulo_ok(&t), ok);
        // La huella cambia con otra vuelta, no con el mismo informe otra vez.
        assert_eq!(huella(&p, &sanos), huella(&de_informe(&informe).0, &sanos));
        let mut otra = p.clone();
        otra[0].marca = "2026-10-04T11:00:00+02:00".into();
        assert_ne!(huella(&p, &sanos), huella(&otra, &sanos));
    }

    #[test]
    fn del_resumen() {
        let (p, s) = de_resumen(&json!({ "guarda_copias": { "espejo": { "ultima": "2026-10-04T02:00:00+02:00", "resultado": "ERROR: el disco no está" } } }));
        assert_eq!(p[0].tipo, "espejo_fallido");
        assert_eq!(p[0].mensaje, "el disco no está");
        assert!(s.is_empty());
        let (p, s) = de_resumen(&json!({ "guarda_copias": { "espejo": { "ultima": "x", "resultado": "Bien: 12 archivos" } } }));
        assert!(p.is_empty());
        assert_eq!(s, vec!["espejo_fallido|".to_string()]);
        assert_eq!(de_resumen(&json!({})), (vec![], vec![]));
    }
}
