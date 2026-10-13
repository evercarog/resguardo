//! 14uu (plan 0.8.0, D3): los avisos que salen de los datos básicos del equipo
//! (`resumen.datos_basicos`, que manda el agente):
//!
//! - `disco_lleno`: un disco del equipo con menos del 10 % **y** menos de 10 GiB
//!   libres. Solo los discos que guardan algo de las copias (`en_uso`: carpetas que
//!   se copian, destinos locales, el almacén); si el agente no marca ninguno, todos.
//! - `sin_soporte_pronto` (180 días antes) y `sin_soporte` (desde el día): el sistema
//!   deja de recibir actualizaciones de seguridad, según la tabla
//!   `datos/fin-soporte.txt` (fácil de actualizar, con sus fuentes).
//!
//! Lo que manda el agente se trata como dato de fuera: textos recortados, números
//! comprobados y como mucho [`MAX_DISCOS`] discos.

use super::problemas::Problema;
use chrono::{Datelike, NaiveDate};
use serde_json::Value;

/// Menos de esta fracción libre…
const FRACCION_LLENO: f64 = 0.10;
/// … y menos de estos bytes libres: casi lleno.
const BYTES_LLENO: u64 = 10 * 1024 * 1024 * 1024;
/// Días de antelación del aviso de fin de soporte.
pub const DIAS_ANTES: i64 = 180;
/// Los discos que se miran (el agente no manda más de 32).
pub const MAX_DISCOS: usize = 32;

/// La tabla de fin de soporte (el archivo de datos, dentro del servidor).
const TABLA: &str = include_str!("../../datos/fin-soporte.txt");

const MESES: [&str; 12] = ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"];

fn corto(v: &Value, max: usize) -> String {
    v.as_str().unwrap_or_default().chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

/// Una línea de la tabla: (familia, versión, edición, fecha).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinSoporte {
    pub familia: String,
    pub version: String,
    pub variante: String,
    pub fecha: NaiveDate,
}

/// Lee la tabla. Las líneas que no valen se cuentan aparte (las pruebas exigen cero).
pub fn leer_tabla(texto: &str) -> (Vec<FinSoporte>, Vec<String>) {
    let (mut ok, mut malas) = (Vec::new(), Vec::new());
    for l in texto.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let c: Vec<&str> = l.split('|').map(str::trim).collect();
        match (c.as_slice(), c.get(3).and_then(|f| NaiveDate::parse_from_str(f, "%Y-%m-%d").ok())) {
            ([f, v, e, _], Some(fecha)) if !f.is_empty() && !v.is_empty() && !e.is_empty() => {
                ok.push(FinSoporte { familia: f.to_lowercase(), version: v.to_lowercase(), variante: e.to_lowercase(), fecha })
            }
            _ => malas.push(l.to_string()),
        }
    }
    (ok, malas)
}

fn tabla() -> &'static [FinSoporte] {
    static T: std::sync::OnceLock<Vec<FinSoporte>> = std::sync::OnceLock::new();
    T.get_or_init(|| leer_tabla(TABLA).0)
}

/// El grupo de edición de Windows 10/11 por `EditionID`.
pub fn grupo_variante(variante: &str) -> &'static str {
    let e = variante.to_ascii_lowercase();
    if e.starts_with("enterprises") || e.starts_with("iotenterprises") {
        "ltsc"
    } else if e.contains("enterprise") || e.contains("education") {
        "empresa"
    } else {
        "hogar"
    }
}

/// (familia, versión, edición) del sistema que dice el agente, para buscarlo en la tabla.
pub fn clave_sistema(s: &Value) -> Option<(String, String, String)> {
    let nombre = corto(&s["nombre"], 120);
    let version = corto(&s["version"], 40).to_lowercase();
    match s["familia"].as_str()? {
        "windows" => {
            if let Some(resto) = nombre.split("Server").nth(1) {
                let anio: String = resto.trim_start().chars().take_while(char::is_ascii_digit).collect();
                return (anio.len() == 4).then(|| ("windows-server".into(), anio, "*".into()));
            }
            let familia = if nombre.starts_with("Windows 11") {
                "windows-11"
            } else if nombre.starts_with("Windows 10") {
                "windows-10"
            } else {
                return None;
            };
            Some((familia.into(), version, grupo_variante(&corto(&s["variante"], 60)).into()))
        }
        "linux" => {
            let id = corto(&s["id"], 40).to_lowercase();
            (!id.is_empty() && !version.is_empty()).then_some((id, version, "*".into()))
        }
        _ => None,
    }
}

/// El fin de soporte del sistema, si está en la tabla.
pub fn fin_soporte_en(t: &[FinSoporte], s: &Value) -> Option<NaiveDate> {
    let (f, v, e) = clave_sistema(s)?;
    let buscar = |v: &str, e: &str| t.iter().find(|x| x.familia == f && x.version == v && x.variante == e).map(|x| x.fecha);
    buscar(&v, &e).or_else(|| buscar(&v, "*")).or_else(|| buscar("*", &e)).or_else(|| buscar("*", "*"))
}

pub fn fin_soporte(s: &Value) -> Option<NaiveDate> {
    fin_soporte_en(tabla(), s)
}

/// «14 de octubre de 2025».
pub fn fecha_texto(d: NaiveDate) -> String {
    format!("{} de {} de {}", d.day(), MESES[d.month0() as usize], d.year())
}

/// «4,2 GB» (unidades decimales, como la consola).
fn gb(b: u64) -> String {
    let g = b as f64 / 1e9;
    let t = if g >= 100.0 { format!("{g:.0}") } else { format!("{g:.1}") };
    format!("{} GB", t.replace('.', ","))
}

/// El sistema en palabras («Windows 11 Pro 24H2», «Ubuntu 22.04.4 LTS»).
pub fn nombre_sistema(s: &Value) -> String {
    let nombre = corto(&s["nombre"], 120);
    let version = corto(&s["version"], 40);
    if s["familia"] == "windows" && !version.is_empty() && !nombre.contains(&version) {
        format!("{nombre} {version}")
    } else if nombre.is_empty() {
        "El sistema".into()
    } else {
        nombre
    }
}

/// Para el sujeto de un incidente: solo letras, cifras, `-` y `_` (hasta 64).
fn sujeto_de(punto: &str) -> String {
    punto.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).take(64).collect()
}

/// (Problemas, sanos) de `resumen.datos_basicos` en el día `hoy` (hora del servidor).
pub fn problemas(resumen: &Value, hoy: NaiveDate) -> (Vec<Problema>, Vec<String>) {
    let (mut p, mut sanos) = (Vec::new(), Vec::new());
    let d = &resumen["datos_basicos"];
    if !d.is_object() {
        return (p, sanos);
    }
    // Discos casi llenos.
    let discos: Vec<&Value> = d["discos"].as_array().map(|a| a.iter().take(MAX_DISCOS).collect()).unwrap_or_default();
    let alguno_en_uso = discos.iter().any(|x| x["en_uso"] == true);
    for x in discos {
        let punto = corto(&x["punto"], 120);
        let (Some(total), Some(libre)) = (x["total"].as_u64(), x["libre"].as_u64()) else { continue };
        if punto.is_empty() || total == 0 || libre > total {
            continue;
        }
        let sujeto = sujeto_de(&punto);
        if alguno_en_uso && x["en_uso"] != true {
            continue;
        }
        let lleno = (libre as f64) < (total as f64) * FRACCION_LLENO && libre < BYTES_LLENO;
        if lleno {
            let pct = (libre as f64 * 100.0 / total as f64).floor() as u64;
            p.push(Problema {
                tipo: "disco_lleno".into(),
                sujeto,
                nombre: punto,
                mensaje: format!(
                    "Quedan {} libres de {} ({pct} %). Libera espacio o amplía el disco: si se llena, las copias pueden fallar.",
                    gb(libre),
                    gb(total)
                ),
                marca: "lleno".into(),
                cuando: None,
            });
        } else {
            sanos.push(format!("disco_lleno|{sujeto}"));
        }
    }
    // Fin del soporte del sistema.
    let s = &d["sistema"];
    if s.is_object() {
        let fin = fin_soporte(s);
        let nombre = nombre_sistema(s);
        match fin {
            Some(f) if hoy >= f => p.push(Problema {
                tipo: "sin_soporte".into(),
                sujeto: String::new(),
                nombre: nombre.clone(),
                mensaje: format!(
                    "{nombre} no recibe actualizaciones de seguridad desde el {}. Actualízalo a una versión con soporte o cambia el equipo.",
                    fecha_texto(f)
                ),
                marca: f.to_string(),
                cuando: None,
            }),
            Some(f) if (f - hoy).num_days() <= DIAS_ANTES => p.push(Problema {
                tipo: "sin_soporte_pronto".into(),
                sujeto: String::new(),
                nombre: nombre.clone(),
                mensaje: format!(
                    "{nombre} deja de recibir actualizaciones de seguridad el {} (en {} días). Planifica la actualización.",
                    fecha_texto(f),
                    (f - hoy).num_days()
                ),
                marca: f.to_string(),
                cuando: None,
            }),
            _ => {}
        }
        for t in ["sin_soporte", "sin_soporte_pronto"] {
            if !p.iter().any(|x| x.tipo == t) {
                sanos.push(format!("{t}|"));
            }
        }
    }
    (p, sanos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn dia(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn la_tabla_vale_entera() {
        let (t, malas) = leer_tabla(TABLA);
        assert!(malas.is_empty(), "líneas que no valen: {malas:?}");
        assert!(t.len() >= 25);
        // Sin repetidas (la búsqueda se quedaría con la primera).
        for (i, a) in t.iter().enumerate() {
            assert!(!t[i + 1..].iter().any(|b| b.familia == a.familia && b.version == a.version && b.variante == a.variante), "{a:?} repetida");
        }
        assert_eq!(leer_tabla("x | y | z | 2026-13-01\n# c\n\nw|1|*|2026-01-01").1, vec!["x | y | z | 2026-13-01".to_string()]);
    }

    #[test]
    fn fechas_conocidas() {
        let w = |nombre: &str, variante: &str, version: &str| json!({ "familia": "windows", "nombre": nombre, "variante": variante, "version": version });
        assert_eq!(fin_soporte(&w("Windows 10 Pro", "Professional", "22H2")), Some(dia("2025-10-14")));
        assert_eq!(fin_soporte(&w("Windows 10 Enterprise", "Enterprise", "21H2")), Some(dia("2025-10-14")));
        assert_eq!(fin_soporte(&w("Windows 10 Enterprise LTSC 2021", "EnterpriseS", "21H2")), Some(dia("2027-01-12")));
        assert_eq!(fin_soporte(&w("Windows 10 Enterprise LTSC", "EnterpriseS", "")), None, "LTSC sin versión conocida: sin aviso");
        assert_eq!(fin_soporte(&w("Windows Server 2012 R2 Standard", "ServerStandard", "")), Some(dia("2023-10-10")));
        assert_eq!(fin_soporte(&w("Windows Server 2016 Standard", "ServerStandard", "1607")), Some(dia("2027-01-12")));
        assert_eq!(fin_soporte(&w("Windows Server 2019 Datacenter", "ServerDatacenter", "1809")), Some(dia("2029-01-09")));
        assert_eq!(fin_soporte(&w("Windows Server 2022 Standard", "ServerStandard", "21H2")), Some(dia("2031-10-14")));
        assert_eq!(fin_soporte(&w("Windows 11 Pro", "Professional", "23H2")), Some(dia("2025-11-11")));
        assert_eq!(fin_soporte(&w("Windows 11 Home", "Core", "24H2")), Some(dia("2026-10-13")));
        assert_eq!(fin_soporte(&w("Windows 11 Enterprise", "Enterprise", "24H2")), Some(dia("2027-10-12")));
        assert_eq!(fin_soporte(&w("Windows 11 Education", "Education", "23H2")), Some(dia("2026-11-10")));
        assert_eq!(fin_soporte(&w("Windows 11 Pro", "Professional", "26H2")), None, "versión nueva: sin aviso");
        let l = |id: &str, v: &str| json!({ "familia": "linux", "nombre": "x", "id": id, "version": v });
        assert_eq!(fin_soporte(&l("ubuntu", "20.04")), Some(dia("2025-05-31")));
        assert_eq!(fin_soporte(&l("ubuntu", "22.04")), Some(dia("2027-04-30")));
        assert_eq!(fin_soporte(&l("ubuntu", "24.04")), Some(dia("2029-04-30")));
        assert_eq!(fin_soporte(&l("debian", "11")), Some(dia("2026-08-31")));
        assert_eq!(fin_soporte(&l("debian", "12")), Some(dia("2028-06-30")));
        assert_eq!(fin_soporte(&l("ubuntu", "24.10")), None);
        assert_eq!(fin_soporte(&l("fedora", "40")), None);
        assert_eq!(fin_soporte(&json!({ "familia": "mac" })), None);
        assert_eq!(fin_soporte(&json!(null)), None);
    }

    #[test]
    fn avisa_180_dias_antes_y_al_llegar() {
        let r = json!({ "datos_basicos": { "sistema": { "familia": "windows", "nombre": "Windows Server 2016 Standard", "version": "1607" } } });
        // Lejos: nada, y los dos están sanos.
        let (p, s) = problemas(&r, dia("2026-01-01"));
        assert!(p.is_empty());
        assert_eq!(s, vec!["sin_soporte|".to_string(), "sin_soporte_pronto|".to_string()]);
        // 180 días antes: pronto.
        let (p, s) = problemas(&r, dia("2026-07-16"));
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].tipo, "sin_soporte_pronto");
        assert!(p[0].mensaje.contains("12 de enero de 2027"), "{}", p[0].mensaje);
        assert!(p[0].mensaje.contains("(en 180 días)"), "{}", p[0].mensaje);
        assert!(p[0].mensaje.starts_with("Windows Server 2016 Standard 1607"), "{}", p[0].mensaje);
        assert_eq!(s, vec!["sin_soporte|".to_string()]);
        let (p, _) = problemas(&r, dia("2026-07-15"));
        assert!(p.is_empty(), "181 días antes, aún no");
        // El día: sin soporte.
        let (p, _) = problemas(&r, dia("2027-01-12"));
        assert_eq!(p[0].tipo, "sin_soporte");
        assert!(p[0].mensaje.contains("desde el 12 de enero de 2027"));
        // Un agente anterior (sin el campo): nada, ni sanos.
        assert_eq!(problemas(&json!({ "so": "Windows x86_64" }), dia("2027-01-12")), (vec![], vec![]));
    }

    #[test]
    fn discos_casi_llenos() {
        const G: u64 = 1_000_000_000;
        let r = |discos: Value| json!({ "datos_basicos": { "discos": discos } });
        // Sin marcar ninguno en uso: se miran todos.
        let (p, s) = problemas(
            &r(json!([
                { "punto": "C:", "total": 120 * G, "libre": 5 * G },
                { "punto": "D:", "total": 2000 * G, "libre": 150 * G },
                { "punto": "E:", "total": 50 * G, "libre": 6 * G },
            ])),
            dia("2026-10-13"),
        );
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].tipo.as_str(), p[0].sujeto.as_str(), p[0].nombre.as_str()), ("disco_lleno", "C_", "C:"));
        assert!(p[0].mensaje.starts_with("Quedan 5,0 GB libres de 120 GB (4 %)"), "{}", p[0].mensaje);
        // D: tiene menos del 10 % pero más de 10 GB; E: menos de 10 GB pero más del 10 %.
        assert_eq!(s, vec!["disco_lleno|D_".to_string(), "disco_lleno|E_".to_string()]);
        // Con alguno en uso: solo esos.
        let (p, s) = problemas(
            &r(json!([
                { "punto": "/", "total": 30 * G, "libre": G, "en_uso": false },
                { "punto": "/srv/copias", "total": 4000 * G, "libre": 9 * G, "en_uso": true },
            ])),
            dia("2026-10-13"),
        );
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].sujeto.as_str(), p[0].nombre.as_str()), ("_srv_copias", "/srv/copias"));
        assert!(s.is_empty());
        // Datos raros: se ignoran sin fallar.
        let (p, s) = problemas(
            &r(
                json!([{ "punto": "C:", "total": 0, "libre": 0 }, { "punto": "D:", "total": 10, "libre": 20 }, { "punto": 3 }, "x", { "punto": "", "total": 1, "libre": 0 }]),
            ),
            dia("2026-10-13"),
        );
        assert!(p.is_empty() && s.is_empty());
        // Como mucho MAX_DISCOS.
        let muchos: Vec<Value> = (0..100).map(|i| json!({ "punto": format!("/m{i}"), "total": 100 * G, "libre": G })).collect();
        assert_eq!(problemas(&r(Value::Array(muchos)), dia("2026-10-13")).0.len(), MAX_DISCOS);
    }

    #[test]
    fn grupos_de_variante() {
        assert_eq!(grupo_variante("Professional"), "hogar");
        assert_eq!(grupo_variante("Core"), "hogar");
        assert_eq!(grupo_variante("EnterpriseN"), "empresa");
        assert_eq!(grupo_variante("Education"), "empresa");
        assert_eq!(grupo_variante("EnterpriseS"), "ltsc");
        assert_eq!(grupo_variante("IoTEnterpriseS"), "ltsc");
        assert_eq!(nombre_sistema(&json!({ "familia": "windows", "nombre": "Windows 11 Pro", "version": "24H2" })), "Windows 11 Pro 24H2");
        assert_eq!(nombre_sistema(&json!({ "familia": "linux", "nombre": "Ubuntu 22.04.4 LTS", "version": "22.04" })), "Ubuntu 22.04.4 LTS");
    }
}
