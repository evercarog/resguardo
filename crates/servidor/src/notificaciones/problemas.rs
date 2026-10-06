//! Qué falla en un equipo, según lo que cuenta: el informe (copias,
//! verificaciones, copia externa y pruebas de restauración) y el resumen (el
//! espejo y la retención del almacén). Lo que está bien también cuenta: es lo que cierra un
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
        Fuente::Informe => &["copia_fallida", "verificacion_fallida", "externa_fallida", "prueba_fallida", "cadena_parada"],
        Fuente::Resumen => &["espejo_fallido", "retencion_fallida"],
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
        // Tarea 4b: las otras copias derivadas (la primera es `externa`), cada una con su sujeto
        // `<repo>--<derivada>` y el aviso de siempre (`externa_fallida`).
        for d in r["derivadas"].as_array().into_iter().flatten().take(16) {
            let did = corto(&d["id"], 64);
            let sujeto = format!("{id}--{did}");
            if id_ok(&did) && sujeto.len() <= 64 {
                let destino = Some(corto(&d["destino"], 80)).filter(|n| !n.is_empty()).unwrap_or_else(|| did.clone());
                mirar("externa_fallida", &sujeto, format!("{nombre} → {destino}"), d, "ultima", "mensaje_corto");
            }
        }
    }
    // Tarea 7c: una copia «después de la anterior» que no se hizo porque la anterior falló.
    for c in datos["cadenas"].as_array().into_iter().flatten().take(MAX_PROBLEMAS) {
        let id = corto(&c["id"], 64);
        let nombre = Some(corto(&c["nombre"], 80)).filter(|n| !n.is_empty()).unwrap_or_else(|| id.clone());
        let anterior = corto(&c["anterior_nombre"], 80);
        let estado = match c["estado"].as_str() {
            Some("parada") => "fallo",
            Some("ok") => "ok",
            _ => continue,
        };
        let mensaje = if anterior.is_empty() {
            "No se hizo porque la copia anterior de la cadena falló.".to_string()
        } else {
            format!("No se hizo porque la copia anterior de la cadena («{anterior}») falló.")
        };
        mirar("cadena_parada", &id, nombre, &serde_json::json!({ "estado": estado, "cuando": c["cuando"], "mensaje": mensaje }), "cuando", "mensaje");
    }
    (p, sanos)
}

/// El espejo del almacén (`guarda_copias.espejo`): `resultado` empieza por `ERROR` si falló.
/// Y la retención del almacén (`guarda_copias.retenciones[]`, `resultado = "fallo"`), por
/// repositorio: sin esto, la que se aplica sola a su hora fallaba sin que nadie se enterara
/// (en la prueba de resistencia, con un archivo dañado al podar).
pub fn de_resumen(resumen: &Value) -> (Vec<Problema>, Vec<String>) {
    let (mut p, mut sanos) = (Vec::new(), Vec::new());
    let e = &resumen["guarda_copias"]["espejo"];
    let resultado = corto(&e["resultado"], 300);
    if e.is_object() && !resultado.is_empty() {
        match resultado.strip_prefix("ERROR") {
            Some(resto) => {
                let marca = corto(&e["ultima"], 40);
                p.push(Problema {
                    tipo: "espejo_fallido".into(),
                    sujeto: String::new(),
                    nombre: String::new(),
                    mensaje: resto.trim_start_matches([':', ' ']).to_string(),
                    cuando: crate::api::de_fecha(&marca),
                    marca,
                });
            }
            None => sanos.push("espejo_fallido|".into()),
        }
    }
    for r in resumen["guarda_copias"]["retenciones"].as_array().into_iter().flatten().take(MAX_PROBLEMAS) {
        let (usuario, repo) = (corto(&r["usuario"], 64), corto(&r["repo"], 64));
        let sujeto = format!("{usuario}_{repo}");
        if !id_ok(&usuario) || !id_ok(&repo) || sujeto.len() > 64 {
            continue;
        }
        match r["resultado"].as_str() {
            Some("fallo") => {
                let marca = corto(&r["ultima"], 40);
                p.push(Problema {
                    tipo: "retencion_fallida".into(),
                    sujeto,
                    nombre: repo,
                    mensaje: corto(&r["mensaje"], 300),
                    cuando: crate::api::de_fecha(&marca),
                    marca,
                });
            }
            Some("ok") => sanos.push(format!("retencion_fallida|{sujeto}")),
            _ => {}
        }
    }
    (p, sanos)
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
        "retencion_fallida" => format!("la retención de «{nombre}» en el almacén «{equipo}»"),
        "cadena_parada" => format!("la cadena de la copia «{nombre}» en «{equipo}»"),
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
        // La retención del almacén, por repositorio (la que se aplica sola a su hora).
        let (p, s) = de_resumen(&json!({ "guarda_copias": { "retenciones": [
            { "usuario": "d8c60f71", "repo": "r-oficina", "resultado": "fallo", "ultima": "2026-10-05T03:00:09-05:00", "mensaje": "Hay datos dañados en el destino" },
            { "usuario": "a1b2", "repo": "r-caja", "resultado": "ok", "ultima": "2026-10-05T03:00:09-05:00" },
            { "usuario": "../x", "repo": "r", "resultado": "fallo" },
            { "usuario": "c3", "repo": "nunca" }
        ] } }));
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].tipo.as_str(), p[0].sujeto.as_str(), p[0].nombre.as_str()), ("retencion_fallida", "d8c60f71_r-oficina", "r-oficina"));
        assert_eq!(s, vec!["retencion_fallida|a1b2_r-caja".to_string()]);
        let (t, ok) = titulos(&p[0], "ALMACEN");
        assert_eq!(t, "Falló la retención de «r-oficina» en el almacén «ALMACEN»");
        assert_eq!(titulo_ok(&t), ok);
        assert!(tipos(Fuente::Resumen).contains(&"retencion_fallida"));
    }

    /// Tarea 7c: `cadena_parada` sale de `informe.cadenas[]`; y 4b: las derivadas, como `externa_fallida`.
    #[test]
    fn cadenas_y_derivadas() {
        let informe = json!({
            "cadenas": [
                { "id": "disco-e", "repo": "r1", "nombre": "Disco E", "estado": "parada", "cuando": "2026-10-04T10:05:00+02:00", "anterior": "docs", "anterior_nombre": "Documentos" },
                { "id": "nube", "repo": "r1", "nombre": "Nube", "estado": "ok", "cuando": "2026-10-04T10:05:00+02:00" },
                { "id": "../x", "estado": "parada" }
            ],
            "repos": [{ "id": "r1", "nombre": "Oficina", "derivadas": [
                { "id": "d2", "destino": "Dropbox Sur", "ultima": "2026-10-04T03:00:00+02:00", "resultado": "fallo", "mensaje_corto": "sin red" },
                { "id": "d3", "ultima": "2026-10-04T03:00:00+02:00", "resultado": "ok" }
            ] }]
        });
        let (p, sanos) = de_informe(&informe);
        assert_eq!(p.len(), 2);
        assert_eq!((p[0].tipo.as_str(), p[0].sujeto.as_str(), p[0].nombre.as_str()), ("externa_fallida", "r1--d2", "Oficina → Dropbox Sur"));
        assert_eq!((p[1].tipo.as_str(), p[1].sujeto.as_str()), ("cadena_parada", "disco-e"));
        assert!(p[1].mensaje.contains("«Documentos»"));
        assert_eq!(p[1].marca, "2026-10-04T10:05:00+02:00");
        assert_eq!(sanos, vec!["externa_fallida|r1--d3".to_string(), "cadena_parada|nube".to_string()]);
        let (t, ok) = titulos(&p[1], "PC-Ana");
        assert_eq!(t, "Falló la cadena de la copia «Disco E» en «PC-Ana»");
        assert_eq!(titulo_ok(&t), ok);
        assert!(tipos(Fuente::Informe).contains(&"cadena_parada"));
    }
}
