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
        // 14uu: los datos básicos del equipo (disco casi lleno, sistema sin soporte).
        Fuente::Resumen => &["espejo_fallido", "retencion_fallida", "disco_lleno", "sin_soporte_pronto", "sin_soporte"],
    }
}

const MAX_PROBLEMAS: usize = 200;

fn corto(v: &Value, max: usize) -> String {
    v.as_str().unwrap_or_default().chars().filter(|c| !c.is_control()).take(max).collect()
}

/// 15oo: el mensaje de un fallo, en palabras: los agentes anteriores mandaban lo que
/// decía el motor de copias tal cual (en inglés, en JSON y cortado). La frase con qué
/// hacer y, detrás de «Detalle técnico:», el texto original (el correo lo pone aparte).
fn en_palabras(s: &str) -> String {
    resguardo_motor::errores::en_una_linea(&resguardo_motor::errores::en_palabras(s))
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
                    mensaje: en_palabras(&corto(&v[mensaje], 300)),
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
        // Plan 0.8.0, E4 (14j): las pruebas de restauración desde un espejo, con su sujeto
        // `<repo>--e-<prueba>` y el aviso de siempre (`prueba_fallida`).
        for d in r["pruebas_espejo"].as_array().into_iter().flatten().take(16) {
            let pid = corto(&d["id"], 40);
            let sujeto = format!("{id}--e-{pid}");
            if id_ok(&pid) && sujeto.len() <= 64 {
                let desde = Some(corto(&d["nombre"], 80)).filter(|n| !n.is_empty()).unwrap_or_else(|| pid.clone());
                mirar("prueba_fallida", &sujeto, format!("{nombre}» desde «{desde}"), d, "ultima", "mensaje_corto");
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
/// B4 (0.8.2): el informe sin la copia derivada «de cada día» (`repos[].externa`) de los
/// repositorios que, según el resumen del equipo, ya no la tienen. El equipo guarda la última
/// vuelta de cada tarea y los agentes hasta la 0.8.1 la seguían mandando tras quitarla: su fallo
/// dejaba un aviso abierto para siempre. Todos los agentes mandan `repositorios[].externa` en el
/// resumen mientras exista. Sin resumen o sin el repositorio en él, no se toca. `None`: nada que quitar.
pub fn sin_externas_quitadas(datos: &Value, resumen: Option<&Value>) -> Option<Value> {
    let repos = resumen?.get("repositorios")?.as_array()?;
    let quitada = |id: &str| repos.iter().any(|r| r["id"].as_str() == Some(id) && !r["externa"].is_object());
    let hay = datos["repos"].as_array()?.iter().any(|r| r["externa"].is_object() && r["id"].as_str().is_some_and(quitada));
    if !hay {
        return None;
    }
    let mut d = datos.clone();
    for r in d["repos"].as_array_mut().into_iter().flatten() {
        if r["externa"].is_object() && r["id"].as_str().is_some_and(quitada) {
            if let Some(o) = r.as_object_mut() {
                o.remove("externa");
            }
        }
    }
    Some(d)
}

pub fn de_resumen(resumen: &Value) -> (Vec<Problema>, Vec<String>) {
    let (mut p, mut sanos) = (Vec::new(), Vec::new());
    let e = &resumen["guarda_copias"]["espejo"];
    let resultado = corto(&e["resultado"], 300);
    // G4: con trabajos (0.7.26), cada espejo con su aviso, su nombre y su destino; el
    // global (sin sujeto) de antes queda arreglado.
    let trabajos = e["trabajos"].as_array().filter(|t| !t.is_empty());
    for t in trabajos.into_iter().flatten().take(MAX_PROBLEMAS) {
        let id = corto(&t["id"], 64);
        let r = corto(&t["resultado"], 300);
        if !id_ok(&id) || r.is_empty() || t["activo"] == false {
            continue;
        }
        match r.strip_prefix("ERROR") {
            Some(resto) => {
                let marca = corto(&t["ultima"], 40);
                let nombre = Some(corto(&t["nombre"], 80)).filter(|n| !n.is_empty()).unwrap_or_else(|| id.clone());
                p.push(Problema {
                    tipo: "espejo_fallido".into(),
                    sujeto: id,
                    nombre: format!("{nombre}» a «{}", destino_espejo(&t["adonde"])),
                    mensaje: en_palabras(resto.trim_start_matches([':', ' '])),
                    cuando: crate::api::de_fecha(&marca),
                    marca,
                });
            }
            None => sanos.push(format!("espejo_fallido|{id}")),
        }
    }
    if trabajos.is_some() {
        sanos.push("espejo_fallido|".into());
    } else if e.is_object() && !resultado.is_empty() {
        match resultado.strip_prefix("ERROR") {
            Some(resto) => {
                let marca = corto(&e["ultima"], 40);
                p.push(Problema {
                    tipo: "espejo_fallido".into(),
                    sujeto: String::new(),
                    nombre: String::new(),
                    mensaje: en_palabras(resto.trim_start_matches([':', ' '])),
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
                    // G4: el nombre del repositorio lo pone quien procesa el aviso
                    // (`nombre_en_almacen`); el almacén solo sabe su carpeta.
                    nombre: format!("{usuario}/{repo}"),
                    mensaje: en_palabras(&corto(&r["mensaje"], 300)),
                    cuando: crate::api::de_fecha(&marca),
                    marca,
                });
            }
            Some("ok") => sanos.push(format!("retencion_fallida|{sujeto}")),
            _ => {}
        }
    }
    // 14uu (plan 0.8.0, D3): disco del equipo casi lleno y sistema que se queda sin soporte.
    let (mas, mas_sanos) = super::datos_basicos::problemas(resumen, chrono::Local::now().date_naive());
    p.extend(mas);
    sanos.extend(mas_sanos);
    (p, sanos)
}

/// G4: a dónde sube un espejo, dicho sin rutas: la nube por su nombre, la zona o «un disco».
fn destino_espejo(a: &Value) -> String {
    match a["tipo"].as_str() {
        Some("nube") => corto(&a["nube"], 60),
        Some("zona") if a["carpeta"] == "principal" => "la zona principal del almacén".into(),
        Some("zona") => format!("la zona {}", corto(&a["carpeta"], 40)),
        _ => "un disco del almacén".into(),
    }
}

/// G4: «Contabilidad» de «PC-01» para la retención de la carpeta `pc-01-3f2a/contabilidad`
/// de un almacén, buscando el repositorio entre los equipos del cliente (`repositorios` y
/// `destinos` de su resumen). Sin encontrarlo, la carpeta.
pub fn nombre_en_almacen(carpeta: &str, equipos: &[(String, Value)]) -> String {
    let Some((usuario, repo)) = carpeta.split_once('/') else { return carpeta.to_string() };
    for (nombre_equipo, r) in equipos {
        let destinos = r["destinos"].as_array().map(Vec::as_slice).unwrap_or_default();
        for x in r["repositorios"].as_array().into_iter().flatten() {
            let Some(d) = destinos.iter().find(|d| d["id"] == x["destino"] && d["tipo"] == "rest") else { continue };
            let donde = d["donde"].as_str().unwrap_or_default();
            let ruta = donde.split_once("://").map_or(donde, |(_, r)| r);
            let primero = ruta.split('/').nth(1).unwrap_or_default();
            let propia = x["ruta"].as_str().filter(|r| !r.is_empty()).or(x["id"].as_str()).unwrap_or_default();
            if primero == usuario && propia == repo {
                let n = x["nombre"].as_str().unwrap_or(repo);
                return format!("{n}» de «{nombre_equipo}");
            }
        }
    }
    repo.to_string()
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
        "externa_fallida" => format!("la copia derivada de «{nombre}» en «{equipo}»"),
        "prueba_fallida" => format!("la prueba de restauración de «{nombre}» en «{equipo}»"),
        "espejo_fallido" if nombre.is_empty() => format!("el espejo de «{equipo}»"),
        "espejo_fallido" => format!("el espejo «{nombre}» en «{equipo}»"),
        "retencion_fallida" => format!("la retención de «{nombre}» en el almacén «{equipo}»"),
        "cadena_parada" => format!("la cadena de la copia «{nombre}» en «{equipo}»"),
        _ => format!("algo en «{equipo}»"),
    }
}

/// (Cuando falla, cuando se arregla.)
pub fn titulos(p: &Problema, equipo: &str) -> (String, String) {
    // 14uu: no son fallos de algo que se hizo, sino el estado del equipo.
    match p.tipo.as_str() {
        "disco_lleno" => {
            let t = format!("{ESPACIO_POCO}{} de «{equipo}»", p.nombre);
            return (t.clone(), titulo_ok(&t));
        }
        "sin_soporte_pronto" | "sin_soporte" => {
            let t = if p.tipo == "sin_soporte" {
                format!("El sistema de «{equipo}» ya no tiene soporte")
            } else {
                format!("El sistema de «{equipo}» se queda sin soporte pronto")
            };
            return (t.clone(), titulo_ok(&t));
        }
        _ => {}
    }
    // 14tt: una copia derivada (sujeto `<repo>--<derivada>`) no es «la copia externa».
    let s = if p.tipo == "externa_fallida" && p.sujeto.contains("--") {
        format!("la copia derivada «{}» en «{equipo}»", p.nombre)
    } else {
        sujeto(&p.tipo, &p.nombre, equipo)
    };
    (format!("Falló {s}"), format!("Volvió a funcionar {s}"))
}

/// 14uu: el principio del título de `disco_lleno` («Queda poco espacio en el disco C: de «pc-01»»).
const ESPACIO_POCO: &str = "Queda poco espacio en el disco ";

/// El título de «arreglado» a partir del de «falló».
pub fn titulo_ok(titulo: &str) -> String {
    if let Some(s) = titulo.strip_prefix(ESPACIO_POCO) {
        return format!("Vuelve a haber espacio en el disco {s}");
    }
    if let Some(r) = titulo.strip_prefix("El sistema de ") {
        if let Some(e) = r.strip_suffix(" se queda sin soporte pronto").or_else(|| r.strip_suffix(" ya no tiene soporte")) {
            return format!("Resuelto: el sistema de {e} tiene soporte");
        }
    }
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
        assert_eq!((p[0].tipo.as_str(), p[0].sujeto.as_str(), p[0].nombre.as_str()), ("retencion_fallida", "d8c60f71_r-oficina", "d8c60f71/r-oficina"));
        assert_eq!(s, vec!["retencion_fallida|a1b2_r-caja".to_string()]);
        // G4: con el nombre del repositorio y su equipo, si se encuentra; si no, su carpeta.
        let equipos = vec![(
            "PC-01".to_string(),
            json!({ "destinos": [{ "id": "d1", "tipo": "rest", "donde": "https://192.0.2.5:8000/d8c60f71/" }],
                    "repositorios": [{ "id": "r-oficina", "nombre": "Oficina", "destino": "d1" }] }),
        )];
        let mut con = p[0].clone();
        con.nombre = nombre_en_almacen(&p[0].nombre, &equipos);
        let (t, ok) = titulos(&con, "ALMACEN");
        assert_eq!(t, "Falló la retención de «Oficina» de «PC-01» en el almacén «ALMACEN»");
        assert_eq!(titulo_ok(&t), ok);
        assert_eq!(nombre_en_almacen(&p[0].nombre, &[]), "r-oficina");
        assert!(tipos(Fuente::Resumen).contains(&"retencion_fallida"));
    }

    /// G4: cada trabajo de espejo con su aviso, su nombre y su destino.
    #[test]
    fn espejos_por_trabajo() {
        let (p, s) = de_resumen(&json!({ "guarda_copias": { "espejo": { "resultado": "ERROR: espejo del Servidor de copias: 1 de 2", "trabajos": [
            { "id": "t1", "nombre": "Bases a la nube", "activo": true, "adonde": { "tipo": "nube", "nube": "Nube Oficina", "carpeta": "x" },
              "ultima": "2026-10-04T02:00:00+02:00", "resultado": "ERROR: espejo «Bases a la nube» en Nube Oficina:x: no se pudo subir" },
            { "id": "t2", "nombre": "Disco E", "activo": true, "adonde": { "tipo": "carpeta", "carpeta": "E:\\Espejo" }, "resultado": "Espejo «Disco E» hecho" }
        ] } } }));
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].sujeto.as_str(), p[0].nombre.as_str()), ("t1", "Bases a la nube» a «Nube Oficina"));
        assert_eq!(s, vec!["espejo_fallido|t2".to_string(), "espejo_fallido|".to_string()]);
        assert_eq!(titulos(&p[0], "ALMACEN").0, "Falló el espejo «Bases a la nube» a «Nube Oficina» en «ALMACEN»");
    }

    /// 15oo: el error crudo de un agente anterior llega en palabras, con el original aparte.
    #[test]
    fn errores_en_palabras() {
        let crudo = r#"Falló: {"message_type":"exit_error","code":1,"message":"Fatal: unable to save snapshot: Post [ruta]: dial tcp 192.0.2.10:8002: connectex: A socket operation"#;
        let (p, _) =
            de_informe(&json!({ "copias": [{ "id": "c1", "nombre": "Docs", "estado": "fallo", "cuando": "2026-10-04T10:00:00+02:00", "mensaje": crudo }] }));
        assert!(p[0].mensaje.starts_with("Se cortó la conexión con el destino (192.0.2.10)"), "{}", p[0].mensaje);
        assert!(p[0].mensaje.contains("Detalle técnico: Fatal: unable to save snapshot") && !p[0].mensaje.contains("message_type"), "{}", p[0].mensaje);
    }

    /// 14uu: los títulos de los datos básicos del equipo y su vuelta a la normalidad.
    #[test]
    fn titulos_de_los_datos_basicos() {
        let p = |tipo: &str, nombre: &str| Problema {
            tipo: tipo.into(),
            sujeto: String::new(),
            nombre: nombre.into(),
            mensaje: String::new(),
            marca: String::new(),
            cuando: None,
        };
        let (t, ok) = titulos(&p("disco_lleno", "C:"), "pc-01");
        assert_eq!(t, "Queda poco espacio en el disco C: de «pc-01»");
        assert_eq!(ok, "Vuelve a haber espacio en el disco C: de «pc-01»");
        assert_eq!(titulo_ok(&t), ok);
        let (t, ok) = titulos(&p("sin_soporte_pronto", "Windows 10 Pro 22H2"), "pc-01");
        assert_eq!(t, "El sistema de «pc-01» se queda sin soporte pronto");
        assert_eq!(ok, "Resuelto: el sistema de «pc-01» tiene soporte");
        let (t, ok) = titulos(&p("sin_soporte", "x"), "srv-ventas");
        assert_eq!(t, "El sistema de «srv-ventas» ya no tiene soporte");
        assert_eq!(titulo_ok(&t), ok);
        for t in ["disco_lleno", "sin_soporte_pronto", "sin_soporte"] {
            assert!(tipos(Fuente::Resumen).contains(&t));
        }
        // Del resumen entero (con la fecha de hoy: un sistema sin fecha en la tabla no avisa).
        let (p, s) = de_resumen(&json!({ "datos_basicos": {
            "sistema": { "familia": "linux", "nombre": "Inventado 1", "id": "inventado", "version": "1" },
            "discos": [{ "punto": "/", "total": 100u64 << 30, "libre": 1u64 << 30, "en_uso": true }]
        } }));
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].tipo, "disco_lleno");
        assert_eq!(s, vec!["sin_soporte|".to_string(), "sin_soporte_pronto|".to_string()]);
    }

    /// E4 (14j): una prueba desde un espejo que falla sale como `prueba_fallida`, con su propio sujeto.
    #[test]
    fn pruebas_desde_un_espejo() {
        let informe = json!({ "repos": [{ "id": "r1", "nombre": "Oficina", "pruebas_espejo": [
            { "id": "eq-t1", "nombre": "Disco E", "ultima": "2026-10-04T05:00:00+02:00", "resultado": "fallo", "mensaje_corto": "Un archivo no coincide." },
            { "id": "n-sur", "nombre": "Nube Sur", "ultima": "2026-10-04T05:10:00+02:00", "resultado": "ok" },
            { "id": "programada-sin-hacer", "nombre": "Otra", "cada_dias": 30 }
        ] }] });
        let (p, sanos) = de_informe(&informe);
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].tipo.as_str(), p[0].sujeto.as_str()), ("prueba_fallida", "r1--e-eq-t1"));
        assert!(p[0].nombre.contains("Disco E"));
        assert!(sanos.contains(&"prueba_fallida|r1--e-n-sur".to_string()), "{sanos:?}");
    }

    /// B4 (0.8.2): la última vuelta de una copia derivada «de cada día» ya quitada no deja aviso.
    #[test]
    fn externa_quitada_no_avisa() {
        let informe = json!({ "repos": [
            { "id": "r1", "nombre": "Oficina", "externa": { "ultima": "2026-10-04T03:00:00+02:00", "resultado": "fallo", "mensaje_corto": "sin red" } },
            { "id": "r2", "nombre": "Fotos", "externa": { "ultima": "2026-10-04T03:00:00+02:00", "resultado": "fallo", "mensaje_corto": "sin red" } },
            { "id": "r3", "nombre": "Otro", "externa": { "ultima": "2026-10-04T03:00:00+02:00", "resultado": "fallo" } }
        ] });
        let resumen = json!({ "repositorios": [
            { "id": "r1", "externa": null },
            { "id": "r2", "externa": { "destino": "Nube", "hora": "21:00" } }
        ] });
        let limpio = sin_externas_quitadas(&informe, Some(&resumen)).expect("quita la de r1");
        let (p, _) = de_informe(&limpio);
        let sujetos: Vec<&str> = p.iter().map(|x| x.sujeto.as_str()).collect();
        // r1 ya no la tiene; r2 sí; r3 no está en el resumen (no se sabe: se deja).
        assert_eq!(sujetos, ["r2", "r3"]);
        assert!(sin_externas_quitadas(&informe, None).is_none(), "sin resumen no se toca");
        assert!(sin_externas_quitadas(&limpio, Some(&resumen)).is_none(), "nada más que quitar");
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
        // 14tt: una derivada se dice derivada (no «copia externa»).
        assert_eq!(titulos(&p[0], "pc-01").0, "Falló la copia derivada «Oficina → Dropbox Sur» en «pc-01»");
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
