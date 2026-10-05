//! Lo que quitó cada vuelta de la retención («Retención en detalle» de la
//! consola; docs/api-servidor.md, «Historial del equipo», tipo `retencion`).
//!
//! Cada vez que se aplica una retención —la del equipo (`aplicar_retencion`,
//! desde la consola o la ventana), la del almacén (`aplicar_retencion_almacen`
//! o a su hora) o la de la copia externa (en su destino)— se anota en la
//! bitácora una entrada `retencion` con:
//!
//! - quién la aplicó (`origen`: equipo, almacén o copia externa; `por`: una
//!   orden, la ventana del equipo o su horario) y la regla;
//! - cuántas versiones había, cuántas se quitaron y cuántas quedan, el espacio
//!   que liberó `prune` (si lo dice) y, en el almacén, las sospechosas que no
//!   se tocaron;
//! - **las versiones quitadas**: id corto (8), hora, grupo (la copia, o una
//!   versiones que quedan en ese grupo para que la consola sepa cuál es), lo que
//!   ocupaban sus archivos y qué regla las habría guardado y por qué no
//!   (`cupo:diarias`, `plazo:horarias`, `repe:horarias`).
//!
//! Solo metadatos que ya van en los informes (ids, horas, nombres de copias):
//! sin rutas ni nombres de archivos. Acotado: como mucho [`MAX_IDS`] versiones
//! por vuelta y [`MAX_BYTES`] por entrada; la bitácora guarda con detalle las
//! [`DETALLADAS`] más recientes y de las anteriores solo las cifras.

use crate::gestion_v2::{Plazo, Retencion, SIEMPRE};
use crate::retencion_almacen::{restar_plazo, Hora, HUECOS};
use chrono::{DateTime, Local};
use resguardo_motor::restic::Snapshot;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Versiones quitadas que se guardan por vuelta (las demás solo se cuentan: `mas`).
pub const MAX_IDS: usize = 2000;
/// Vueltas con la lista de versiones; de las anteriores, solo las cifras.
pub const DETALLADAS: usize = 50;
/// Tamaño máximo de una entrada (en JSON). El servidor admite hasta esto en las de tipo `retencion`.
pub const MAX_BYTES: usize = 96 * 1024;
/// Versiones que quedan en cada grupo que se anotan (para saber su copia en la consola).
const REFS: usize = 5;
/// Lo que se quita al compactar una entrada antigua.
const CAMPOS_DETALLE: [&str; 3] = ["versiones", "grupos", "motivos"];

const PERIODOS: [&str; 5] = ["horarias", "diarias", "semanales", "mensuales", "anuales"];

/// Una versión para decidir: id, grupo (como `restic forget`: equipo y carpetas) y hora.
pub struct Candidata {
    pub id: String,
    pub grupo: String,
    pub hora: Hora,
}

/// El grupo de restic de una versión (equipo y carpetas ordenadas), como el almacén.
pub fn grupo_de(s: &Snapshot) -> String {
    let mut carpetas = s.paths.clone();
    carpetas.sort();
    serde_json::to_string(&(&s.hostname, carpetas)).unwrap_or_default()
}

pub fn candidatas(snaps: &[Snapshot]) -> Vec<Candidata> {
    snaps.iter().filter_map(|s| Some(Candidata { id: s.id.clone(), grupo: grupo_de(s), hora: DateTime::parse_from_rfc3339(&s.time).ok()? })).collect()
}

/// Por qué se quitaría cada versión con la regla `r` (las reglas de `restic
/// forget`, como `retencion_almacen::planear`), o `None` si se queda:
/// - `cupo:<periodo>`: era la más reciente de su hora/día/semana/mes/año, pero
///   ya había tantas de ese tipo más recientes como dice la regla;
/// - `plazo:<periodo>`: lo era, pero queda fuera del plazo de ese tipo;
/// - `repe:<periodo>`: ya había otra más reciente en la misma hora (o día…:
///   el periodo más corto que usa la regla).
///
/// Si vale para varios periodos, el más largo (es lo que se echa de menos).
pub fn motivos(versiones: &[Candidata], r: &Retencion) -> HashMap<String, Option<String>> {
    let mut out = HashMap::new();
    let mut grupos: BTreeMap<&str, Vec<&Candidata>> = BTreeMap::new();
    for v in versiones {
        grupos.entry(v.grupo.as_str()).or_default().push(v);
    }
    let plazos: Vec<Option<Plazo>> = r.plazos.lista().iter().map(|p| p.and_then(Plazo::leer)).collect();
    let cantidades = r.cantidades();
    let activo = |i: usize| cantidades[i] != 0 || plazos[i].is_some();
    let mas_corto = (0..PERIODOS.len()).find(|i| activo(*i)).unwrap_or(1);
    for (_, mut lista) in grupos {
        lista.sort_by(|a, b| b.hora.cmp(&a.hora).then_with(|| b.id.cmp(&a.id)));
        let Some(mas_reciente) = lista.first().map(|x| x.hora) else { continue };
        let limites: Vec<Option<Hora>> = plazos.iter().map(|p| p.as_ref().and_then(|p| restar_plazo(&mas_reciente, p))).collect();
        let mut quedan: Vec<(i64, Option<i64>)> = cantidades.iter().map(|n| (*n, None)).collect();
        let mut en_plazo: Vec<Option<i64>> = vec![None; HUECOS.len()];
        // El hueco de la versión anterior (guardada o no): ¿es la más reciente de su hora, día…?
        let mut visto: Vec<Option<i64>> = vec![None; HUECOS.len()];
        let ultima = lista.len().saturating_sub(1);
        for (n, v) in lista.iter().enumerate() {
            let t = &v.hora;
            let mut guardar = false;
            let mut motivo: Option<String> = None;
            for (i, hueco) in HUECOS.iter().enumerate() {
                let h = hueco(t);
                let nuevo = visto[i] != Some(h);
                visto[i] = Some(h);
                let (cuantas, anterior) = &mut quedan[i];
                if *cuantas > 0 || *cuantas == SIEMPRE {
                    if *anterior != Some(h) || n == ultima {
                        guardar = true;
                        *anterior = Some(h);
                        if *cuantas > 0 {
                            *cuantas -= 1;
                        }
                    }
                } else if cantidades[i] != 0 && nuevo {
                    motivo = Some(format!("cupo:{}", PERIODOS[i]));
                }
                if let Some(limite) = limites[i] {
                    if *t > limite {
                        if en_plazo[i] != Some(h) || n == ultima {
                            guardar = true;
                            en_plazo[i] = Some(h);
                        }
                    } else if nuevo {
                        motivo = Some(format!("plazo:{}", PERIODOS[i]));
                    }
                }
            }
            out.insert(v.id.clone(), (!guardar).then(|| motivo.unwrap_or_else(|| format!("repe:{}", PERIODOS[mas_corto]))));
        }
    }
    out
}

/// La regla de una política de restic (la de la copia externa), como una
/// `Retencion`, si solo usa lo que la consola sabe enseñar.
pub fn retencion_de_politica(p: &resguardo_motor::retention::Policy) -> Option<Retencion> {
    let simple = p.keep_last == 0
        && p.keep_within.is_none()
        && p.keep_tags.is_empty()
        && p.filter_tags.is_empty()
        && p.filter_paths.is_empty()
        && p.filter_host.is_none()
        && p.group_by.is_none();
    simple.then(|| Retencion {
        horarias: p.keep_hourly,
        diarias: p.keep_daily,
        semanales: p.keep_weekly,
        mensuales: p.keep_monthly,
        anuales: p.keep_yearly,
        plazos: crate::gestion_v2::Plazos {
            horarias: p.keep_within_hourly.clone(),
            diarias: p.keep_within_daily.clone(),
            semanales: p.keep_within_weekly.clone(),
            mensuales: p.keep_within_monthly.clone(),
            anuales: p.keep_within_yearly.clone(),
        },
    })
}

/// Lo que libera `prune` según su salida («total prune: 13 blobs / 2.103 MiB»).
pub fn liberado(salida: &str) -> Option<u64> {
    let linea = salida.lines().find(|l| l.trim_start().starts_with("total prune:"))?;
    let tamano = linea.rsplit('/').next()?.trim();
    let (n, unidad) = tamano.split_once(' ')?;
    let n: f64 = n.trim().parse().ok()?;
    let factor = match unidad.trim() {
        "B" => 1.0,
        "KiB" => 1024.0,
        "MiB" => 1024.0 * 1024.0,
        "GiB" => 1024.0 * 1024.0 * 1024.0,
        "TiB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    (n.is_finite() && n >= 0.0).then(|| (n * factor).round() as u64)
}

/// Una vuelta de la retención, para anotarla.
pub struct Vuelta<'a> {
    /// "equipo", "almacen" o "externa".
    pub origen: &'static str,
    /// "orden", "ventana" o "automatica".
    pub por: &'static str,
    /// El repositorio (en el almacén, su carpeta dentro de la del usuario).
    pub repo: &'a str,
    /// En el almacén: el usuario (equipo dueño) del repositorio.
    pub usuario: Option<&'a str>,
    pub regla: Option<&'a Retencion>,
    pub inicio: DateTime<Local>,
    /// Las versiones antes y después (`None` si no se pudieron leer).
    pub antes: &'a [Snapshot],
    pub despues: Option<&'a [Snapshot]>,
    /// De [`motivos`] (por id completo).
    pub motivos: &'a HashMap<String, Option<String>>,
    /// Copia (plan) de cada versión, por su id (también el de origen de una copia externa).
    pub copias: &'a HashMap<String, String>,
    pub liberado: Option<u64>,
    /// En el almacén: las versiones con hora que no cuadra, que no se tocan.
    pub sospechosas: Option<usize>,
    /// El mensaje del resultado (`Err`: falló).
    pub resultado: Result<&'a str, &'a str>,
}

fn corto(id: &str) -> String {
    id.chars().take(8).collect()
}

/// La entrada de la bitácora de una vuelta (sin `id`: la pone la bitácora).
pub fn entrada(v: &Vuelta) -> Value {
    let despues_ids: Option<HashSet<&str>> = v.despues.map(|d| d.iter().map(|s| s.id.as_str()).collect());
    let mut quitadas: Vec<&Snapshot> = match &despues_ids {
        Some(d) => v.antes.iter().filter(|s| !d.contains(s.id.as_str())).collect(),
        None => Vec::new(),
    };
    quitadas.sort_by(|a, b| b.time.cmp(&a.time).then_with(|| b.id.cmp(&a.id)));
    let copia_de = |s: &Snapshot| v.copias.get(&s.id).or_else(|| s.original.as_ref().and_then(|o| v.copias.get(o))).cloned();

    // Los grupos de las quitadas: su copia (la más repetida) y la versión más reciente que queda.
    let mut grupos: Vec<String> = Vec::new();
    let mut indice: HashMap<String, usize> = HashMap::new();
    for s in &quitadas {
        let g = grupo_de(s);
        if !indice.contains_key(&g) {
            indice.insert(g.clone(), grupos.len());
            grupos.push(g);
        }
    }
    let grupos_json: Vec<Value> = grupos
        .iter()
        .map(|g| {
            let mut cuenta: BTreeMap<String, usize> = BTreeMap::new();
            for s in v.antes.iter().filter(|s| grupo_de(s) == *g) {
                if let Some(k) = copia_de(s) {
                    *cuenta.entry(k).or_default() += 1;
                }
            }
            let copia = cuenta.into_iter().max_by_key(|(_, n)| *n).map(|(k, _)| k);
            let mut quedan: Vec<&Snapshot> = v.despues.unwrap_or_default().iter().filter(|s| grupo_de(s) == *g).collect();
            quedan.sort_by(|a, b| b.time.cmp(&a.time));
            // Unas versiones que quedan en el grupo: la consola sabe de qué copia es alguna (su informe).
            let refs: Vec<String> = quedan.iter().take(REFS).map(|s| corto(&s.id)).collect();
            let mut g = json!({ "copia": copia, "quedan": quedan.len() });
            if !refs.is_empty() {
                g["refs"] = json!(refs);
            }
            g
        })
        .collect();
    let mut motivos: Vec<String> = Vec::new();
    let mut lista: Vec<Value> = Vec::new();
    for s in quitadas.iter().take(MAX_IDS) {
        let t = DateTime::parse_from_rfc3339(&s.time).map(|t| t.timestamp()).ok();
        let m = v.motivos.get(&s.id).map(|m| m.clone().unwrap_or_else(|| "restic".into())).map(|m| match motivos.iter().position(|x| *x == m) {
            Some(i) => i,
            None => {
                motivos.push(m);
                motivos.len() - 1
            }
        });
        let bytes = s.summary.as_ref().and_then(|x| x.total_bytes_processed);
        lista.push(json!([corto(&s.id), t, indice.get(&grupo_de(s)), bytes, m]));
    }
    let (ok, mensaje) = match v.resultado {
        Ok(m) => (true, m),
        Err(m) => (false, m),
    };
    let mut e = json!({
        "hora": Local::now().to_rfc3339(),
        "inicio": v.inicio.to_rfc3339(),
        "origen": v.origen,
        "por": v.por,
        "repo": v.repo,
        "usuario": v.usuario,
        "regla": v.regla,
        "resultado": if ok { "ok" } else { "fallo" },
        "mensaje": Some(crate::web::public_message(mensaje).chars().take(240).collect::<String>()).filter(|m| !m.is_empty()),
        "antes": v.antes.len(),
        "quedan": v.despues.map(|d| d.len()),
        "quitadas": v.despues.map(|_| quitadas.len()),
        "liberado": v.liberado,
        "sospechosas": v.sospechosas.filter(|n| *n > 0),
        "grupos": Some(grupos_json).filter(|g| !g.is_empty()),
        "motivos": Some(motivos).filter(|m| !m.is_empty()),
        "versiones": Some(lista).filter(|l| !l.is_empty()),
        "mas": Some(quitadas.len().saturating_sub(MAX_IDS)).filter(|n| *n > 0),
    });
    if let Some(o) = e.as_object_mut() {
        o.retain(|_, x| !x.is_null());
    }
    recortar(&mut e);
    e
}

/// Que quepa en [`MAX_BYTES`] (con margen para el `id` y el `tipo` de la bitácora):
/// fuera versiones de las más antiguas (se cuentan en `mas`).
pub fn recortar(e: &mut Value) {
    const MARGEN: usize = 200;
    loop {
        let largo = e.to_string().len();
        let n = e["versiones"].as_array().map_or(0, Vec::len);
        if largo + MARGEN <= MAX_BYTES || n == 0 {
            return;
        }
        // Lo que sobra, en versiones (cada una ocupa más o menos lo mismo), y alguna más.
        let por_version = largo / n.max(1);
        let quitar = ((largo + MARGEN - MAX_BYTES) / por_version.max(1) + 1).min(n);
        if let Some(l) = e["versiones"].as_array_mut() {
            l.truncate(n - quitar);
        }
        e["mas"] = json!(e["mas"].as_u64().unwrap_or(0) + quitar as u64);
        if e["versiones"].as_array().is_some_and(Vec::is_empty) {
            if let Some(o) = e.as_object_mut() {
                for k in CAMPOS_DETALLE {
                    o.remove(k);
                }
            }
        }
    }
}

/// Anota la vuelta en la bitácora del equipo (y lo dice en el registro si no hay versiones).
pub fn anotar(v: &Vuelta) {
    crate::bitacora::anotar("retencion", entrada(v));
}

/// ¿Tiene la lista de versiones (aún no compactada)?
pub fn con_detalle(e: &Value) -> bool {
    e["tipo"] == "retencion" && CAMPOS_DETALLE.iter().any(|k| e.get(*k).is_some())
}

/// Deja una entrada solo con sus cifras.
pub fn compactar(e: &mut Value) {
    if let Some(o) = e.as_object_mut() {
        for k in CAMPOS_DETALLE {
            o.remove(k);
        }
        o.insert("compactada".into(), json!(true));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retencion_almacen::{planear, Version};
    use chrono::{Duration, TimeZone};

    fn snap(id: &str, hora: DateTime<Local>, host: &str) -> Snapshot {
        serde_json::from_value(json!({
            "id": format!("{id:0<64}"), "short_id": id, "time": hora.to_rfc3339(), "hostname": host, "paths": ["/datos"],
            "summary": { "total_bytes_processed": 1000 }
        }))
        .unwrap()
    }

    fn version(c: &Candidata) -> Version {
        // Subida a los 5 minutos: de confianza (como las del almacén).
        Version { id: c.id.clone(), grupo: c.grupo.clone(), hora: Some(c.hora), subida: Some(c.hora.with_timezone(&chrono::Utc) + Duration::minutes(5)) }
    }

    /// Las que quitaría `motivos` son exactamente las de `planear` (las reglas de restic), con reglas variadas.
    #[test]
    fn mismas_que_planear() {
        let t0 = Local.with_ymd_and_hms(2026, 1, 1, 8, 0, 0).unwrap();
        let snaps: Vec<Snapshot> =
            (0..120i64).map(|i| snap(&format!("{i:08x}"), t0 + Duration::hours(i * 7 + (i % 5)), if i % 3 == 0 { "OTRO" } else { "CAJA" })).collect();
        let cands = candidatas(&snaps);
        let reglas = [
            Retencion { diarias: 7, semanales: 4, mensuales: 12, anuales: 2, ..Default::default() },
            Retencion { horarias: 5, diarias: 3, ..Default::default() },
            Retencion {
                mensuales: SIEMPRE,
                plazos: crate::gestion_v2::Plazos { horarias: Some("2d".into()), diarias: Some("15d".into()), ..Default::default() },
                ..Default::default()
            },
            Retencion { semanales: 2, plazos: crate::gestion_v2::Plazos { diarias: Some("10d".into()), ..Default::default() }, ..Default::default() },
        ];
        for r in &reglas {
            let m = motivos(&cands, r);
            let mut nuestras: Vec<&String> = m.iter().filter(|(_, x)| x.is_some()).map(|(id, _)| id).collect();
            let vs: Vec<Version> = cands.iter().map(version).collect();
            let mut plan: Vec<String> = planear(&vs, r).quitar;
            nuestras.sort();
            plan.sort();
            assert_eq!(nuestras, plan.iter().collect::<Vec<_>>(), "{r:?}");
            assert!(!plan.is_empty(), "{r:?}");
        }
    }

    #[test]
    fn por_que_se_va() {
        let t0 = Local.with_ymd_and_hms(2026, 10, 1, 9, 0, 0).unwrap();
        // Tres el 1 de octubre (9:00, 9:30 y 15:00) y una al día del 2 al 5.
        let horas = [
            t0,
            t0 + Duration::minutes(30),
            t0 + Duration::hours(6),
            t0 + Duration::days(1),
            t0 + Duration::days(2),
            t0 + Duration::days(3),
            t0 + Duration::days(4),
        ];
        let snaps: Vec<Snapshot> = horas.iter().enumerate().map(|(i, h)| snap(&format!("a{i}"), *h, "CAJA")).collect();
        let cands = candidatas(&snaps);
        let id = |i: usize| snaps[i].id.clone();
        // 3 diarias: se quedan los días 5, 4 y 3; las del 1 y 2, por cupo; las 9:00 y 9:30 del 1, repetidas en su día.
        let m = motivos(&cands, &Retencion { diarias: 3, ..Default::default() });
        assert_eq!(m[&id(6)], None);
        assert_eq!(m[&id(4)], None);
        assert_eq!(m[&id(3)].as_deref(), Some("cupo:diarias"));
        assert_eq!(m[&id(2)].as_deref(), Some("cupo:diarias"));
        assert_eq!(m[&id(1)].as_deref(), Some("repe:diarias"));
        // La más antigua de todas no se guarda: ya se gastó el cupo.
        assert_eq!(m[&id(0)].as_deref(), Some("repe:diarias"));
        // Plazo de 2 días de diarias (desde la más reciente, el 5 a las 9:00): fuera, las del 1, 2 y 3 (el límite es el 3 a las 9:00, que no entra).
        let m = motivos(&cands, &Retencion { plazos: crate::gestion_v2::Plazos { diarias: Some("2d".into()), ..Default::default() }, ..Default::default() });
        assert_eq!(m[&id(5)], None);
        assert_eq!(m[&id(4)].as_deref(), Some("plazo:diarias"));
        // Con varias reglas, el periodo más largo que la habría guardado.
        let m = motivos(&cands, &Retencion { horarias: 1, diarias: 1, ..Default::default() });
        assert_eq!(m[&id(5)].as_deref(), Some("cupo:diarias"));
        assert_eq!(m[&id(1)].as_deref(), Some("cupo:horarias"), "la de las 9:30 era la horaria de las 9, pero no la diaria (la de las 15:00)");
    }

    #[test]
    fn liberado_de_prune() {
        let salida = "to repack:             0 blobs / 0 B\nthis removes:          0 blobs / 0 B\nto delete:             4 blobs / 294.434 KiB\ntotal prune:           4 blobs / 294.434 KiB\nremaining:             4 blobs / 294.433 KiB\n";
        assert_eq!(liberado(salida), Some(301_500));
        assert_eq!(liberado("total prune:  10 blobs / 2.5 GiB"), Some(2_684_354_560));
        assert_eq!(liberado("total prune:  0 blobs / 0 B"), Some(0));
        assert_eq!(liberado("nada que podar"), None);
        assert_eq!(liberado("total prune: 1 blobs / 3 XB"), None);
    }

    fn vuelta<'a>(
        antes: &'a [Snapshot],
        despues: Option<&'a [Snapshot]>,
        m: &'a HashMap<String, Option<String>>,
        copias: &'a HashMap<String, String>,
    ) -> Vuelta<'a> {
        Vuelta {
            origen: "equipo",
            por: "orden",
            repo: "r1",
            usuario: None,
            regla: None,
            inicio: Local::now(),
            antes,
            despues,
            motivos: m,
            copias,
            liberado: Some(1234),
            sospechosas: None,
            resultado: Ok(r"Retención aplicada en C:\Users\Ana\repo."),
        }
    }

    #[test]
    fn entrada_con_lo_quitado_y_sin_rutas() {
        let t0 = Local.with_ymd_and_hms(2026, 10, 1, 9, 0, 0).unwrap();
        let antes: Vec<Snapshot> = (0..6).map(|i| snap(&format!("b{i}"), t0 + Duration::days(i), if i == 0 { "OTRO" } else { "CAJA" })).collect();
        let despues: Vec<Snapshot> = antes[3..].to_vec();
        let m = motivos(&candidatas(&antes), &Retencion { diarias: 3, ..Default::default() });
        let copias: HashMap<String, String> = antes.iter().skip(1).map(|s| (s.id.clone(), "docs".to_string())).collect();
        let e = entrada(&vuelta(&antes, Some(&despues), &m, &copias));
        assert_eq!((e["antes"].as_u64(), e["quedan"].as_u64(), e["quitadas"].as_u64(), e["liberado"].as_u64()), (Some(6), Some(3), Some(3), Some(1234)));
        let vs = e["versiones"].as_array().unwrap();
        assert_eq!(
            vs.iter().map(|v| v[0].as_str().unwrap()).collect::<Vec<_>>(),
            ["b2000000", "b1000000", "b0000000"],
            "las quitadas, la más reciente primero"
        );
        assert_eq!(vs[0][1].as_i64(), Some((t0 + Duration::days(2)).timestamp()));
        assert_eq!(vs[0][3].as_u64(), Some(1000));
        // Dos grupos: «docs» (en CAJA, con la versión que queda como referencia) y el de OTRO (sin copia ni referencia).
        let g = &e["grupos"];
        assert_eq!(g[vs[0][2].as_u64().unwrap() as usize]["copia"], "docs");
        assert_eq!(g[vs[0][2].as_u64().unwrap() as usize]["refs"], json!(["b5000000", "b4000000", "b3000000"]));
        assert_eq!(g[vs[2][2].as_u64().unwrap() as usize].get("refs"), None);
        assert_eq!(e["motivos"][vs[0][4].as_u64().unwrap() as usize], "cupo:diarias");
        assert!(!e.to_string().contains("Users"), "sin rutas: {e}");
        // Si no se pudo leer cómo quedó: las cifras de antes, sin lista.
        let e = entrada(&Vuelta { resultado: Err("restic falló"), ..vuelta(&antes, None, &m, &copias) });
        assert_eq!((e["resultado"].as_str(), e.get("versiones"), e.get("quitadas")), (Some("fallo"), None, None));
    }

    /// Con restic de verdad: lo anotado es lo que quitó `forget --prune` (con su
    /// motivo, sin «restic»: las reglas son las mismas) y lo que liberó.
    #[test]
    fn lo_anotado_es_lo_que_quito_restic() {
        let _real = crate::restic::tests::real_repo_lock();
        if crate::restic::version().is_err() {
            return;
        }
        let base = std::env::temp_dir().join(format!("resguardo-retreg-{}", uuid::Uuid::new_v4().simple()));
        let datos = base.join("datos");
        std::fs::create_dir_all(&datos).unwrap();
        let acc = crate::restic::Access::new(base.join("repo").display().to_string(), "contraseña");
        let limite = std::time::Duration::from_secs(300);
        assert_eq!(crate::restic::run_raw(&acc, &["init"], limite).unwrap().code, Some(0));
        let t0 = Local::now() - Duration::days(20);
        for i in 0..14i64 {
            // Datos distintos en cada una (que `prune` libere algo).
            std::fs::write(datos.join("f.bin"), (0..20_000).map(|j| ((i * 7919 + j * 31) % 251) as u8).collect::<Vec<u8>>()).unwrap();
            let cuando = (t0 + Duration::hours(i * 30)).format("%Y-%m-%d %H:%M:%S").to_string();
            let b = crate::restic::run_raw(&acc, &["backup", "--host", "CAJA", "--time", &cuando, &datos.display().to_string()], limite).unwrap();
            assert_eq!(b.code, Some(0), "{}", b.stderr);
        }
        let regla = Retencion { diarias: 3, semanales: 1, ..Default::default() };
        let antes = crate::restic::snapshots(&acc).unwrap();
        let m = motivos(&candidatas(&antes), &regla);
        let mut args: Vec<String> = vec!["forget".into(), "--prune".into()];
        args.extend(regla.politica().args());
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = crate::restic::run_raw(&acc, &refs, limite).unwrap();
        assert_eq!(out.code, Some(0), "{}", out.stderr);
        let despues = crate::restic::snapshots(&acc).unwrap();
        let copias = HashMap::new();
        let salida = String::from_utf8_lossy(&out.stdout).into_owned();
        let e = entrada(&Vuelta { liberado: liberado(&salida), regla: Some(&regla), ..vuelta(&antes, Some(&despues), &m, &copias) });
        let mut anotadas: Vec<String> = e["versiones"].as_array().unwrap().iter().map(|v| v[0].as_str().unwrap().to_string()).collect();
        let quedan: HashSet<&str> = despues.iter().map(|s| s.id.as_str()).collect();
        let mut quitadas: Vec<String> = antes.iter().filter(|s| !quedan.contains(s.id.as_str())).map(|s| corto(&s.id)).collect();
        anotadas.sort();
        quitadas.sort();
        assert!(!quitadas.is_empty());
        assert_eq!(anotadas, quitadas);
        assert_eq!(e["quitadas"].as_u64(), Some(quitadas.len() as u64));
        assert!(
            e["motivos"].as_array().unwrap().iter().all(|x| x.as_str().is_some_and(|x| x.starts_with("cupo:") || x.starts_with("repe:"))),
            "{}",
            e["motivos"]
        );
        assert!(e["liberado"].as_u64().is_some_and(|b| b > 0), "{salida}");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Como mucho 2000 versiones y 96 KiB por entrada; lo demás se cuenta en `mas`.
    #[test]
    fn acotada() {
        let t0 = Local.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
        let antes: Vec<Snapshot> = (0..2600i64).map(|i| snap(&format!("{i:08x}"), t0 + Duration::hours(i), "CAJA")).collect();
        let despues: Vec<Snapshot> = antes[2590..].to_vec();
        let m = motivos(&candidatas(&antes), &Retencion { horarias: 10, ..Default::default() });
        let copias: HashMap<String, String> = antes.iter().map(|s| (s.id.clone(), "una-copia-con-un-nombre-largo".to_string())).collect();
        let mut e = entrada(&vuelta(&antes, Some(&despues), &m, &copias));
        let n = e["versiones"].as_array().unwrap().len();
        assert!(n <= MAX_IDS && n > 1000, "{n}");
        assert_eq!(n as u64 + e["mas"].as_u64().unwrap(), 2590);
        assert_eq!(e["quitadas"].as_u64(), Some(2590));
        // Con el id y el tipo que pone la bitácora, cabe.
        e["id"] = json!(uuid::Uuid::new_v4().to_string());
        e["tipo"] = json!("retencion");
        assert!(e.to_string().len() <= MAX_BYTES, "{}", e.to_string().len());
        assert!(con_detalle(&e));
        compactar(&mut e);
        assert!(!con_detalle(&e) && e["compactada"] == true && e["quitadas"] == 2590);
    }
}
