//! v1.30: pistas a los agentes para que lo que enseña la consola no se quede
//! atrás (docs/api-servidor.md §8, `refrescar`).
//!
//! Cuando un almacén aplica la retención en el repositorio de otro equipo
//! (`guarda_copias.retenciones[]` de su resumen trae un resultado nuevo), el
//! equipo dueño no se entera: sus versiones en la consola seguían siendo las de
//! antes hasta su próxima copia o 6 h. El servidor le manda entonces
//! `{ "t": "refrescar", "repo": "<id>" }`. Es solo una pista: el agente vuelve a
//! leer las versiones de ese repositorio (si es suyo, como mucho una vez por
//! minuto) y nada más. No da autoridad sobre nada.

use crate::almacen::Equipo;
use serde_json::Value;

/// `(usuario, repo)` de las retenciones del almacén con un resultado nuevo
/// (otra `ultima` u otras `versiones` que en el resumen anterior).
pub fn retenciones_nuevas(antes: Option<&Value>, ahora: &Value) -> Vec<(String, String)> {
    let lista = |r: Option<&Value>| r.and_then(|r| r["guarda_copias"]["retenciones"].as_array().cloned()).unwrap_or_default();
    let previas = lista(antes);
    lista(Some(ahora))
        .iter()
        .filter(|r| !r["ultima"].is_null())
        .filter_map(|r| {
            let (u, k) = (r["usuario"].as_str()?, r["repo"].as_str()?);
            let antes = previas.iter().find(|p| p["usuario"] == u && p["repo"] == k);
            antes.is_none_or(|p| p["ultima"] != r["ultima"] || p["versiones"] != r["versiones"]).then(|| (u.to_string(), k.to_string()))
        })
        .collect()
}

/// El usuario de un equipo en un almacén: lo último de `https://ip:puerto/<usuario>/`
/// (como `usuarioEnAlmacen` de la consola).
fn usuario_en_almacen(donde: &str) -> Option<&str> {
    let d = donde.trim();
    let d = d.strip_prefix("rest:").unwrap_or(d);
    let resto = d.strip_prefix("https://").or_else(|| d.strip_prefix("http://"))?;
    let (_, camino) = resto.split_once('/')?;
    let u = camino.strip_suffix('/').unwrap_or(camino);
    (!u.is_empty() && !u.contains('/')).then_some(u)
}

/// ¿Es este destino (del resumen de un equipo) el de ese almacén? El mismo criterio que la consola (`esDeAlmacen`).
fn de_almacen(d: &Value, almacen: &Equipo) -> bool {
    match d["equipo_almacen"].as_str() {
        Some(a) => a == almacen.id,
        None => {
            let corto: String = almacen.id.chars().take(8).collect();
            d["id"].as_str() == Some(&format!("almacen-{corto}")) || (d["tipo"] == "rest" && d["nombre"].as_str() == Some(&almacen.nombre))
        }
    }
}

/// Los equipos dueños (y el id de su repositorio) de esas retenciones del almacén.
pub fn duenos(almacen: &Equipo, cambios: &[(String, String)], equipos: &[Equipo]) -> Vec<(String, String)> {
    let mut v = Vec::new();
    for e in equipos.iter().filter(|e| e.confirmado && e.modo == "gestionado") {
        let Some(resumen) = &e.resumen else { continue };
        let destinos = resumen["destinos"].as_array().cloned().unwrap_or_default();
        for r in resumen["repositorios"].as_array().into_iter().flatten() {
            let (Some(id), Some(destino)) = (r["id"].as_str(), r["destino"].as_str()) else { continue };
            // `destino` es el nombre (y quizá el id) del destino del repositorio.
            let Some(d) = destinos.iter().find(|d| d["id"] == destino).or_else(|| destinos.iter().find(|d| d["nombre"] == destino)) else { continue };
            if !de_almacen(d, almacen) {
                continue;
            }
            let Some(usuario) = d["donde"].as_str().and_then(usuario_en_almacen) else { continue };
            let carpeta = r["ruta"].as_str().filter(|s| !s.is_empty()).unwrap_or(id);
            if cambios.iter().any(|(u, k)| u == usuario && k == carpeta) {
                v.push((e.id.clone(), id.to_string()));
            }
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn equipo(id: &str, nombre: &str, resumen: Value) -> Equipo {
        Equipo {
            id: id.into(),
            nombre: nombre.into(),
            so: "windows".into(),
            version_agente: String::new(),
            box_pub: String::new(),
            sign_pub: String::new(),
            sal_equipo: String::new(),
            etiqueta: None,
            rol: "agente".into(),
            modo: "gestionado".into(),
            confirmado: true,
            ultimo_contacto: None,
            estado_servicio: None,
            siguiente_seq: 1,
            seq_espera: 1000,
            atencion_hasta: None,
            resumen: Some(resumen),
            espera_min_horas: None,
            etiquetas: Vec::new(),
        }
    }

    #[test]
    fn solo_las_retenciones_con_resultado_nuevo() {
        let r = |ultima: Value, versiones: Value| json!({ "guarda_copias": { "retenciones": [{ "usuario": "b", "repo": "docs", "ultima": ultima, "versiones": versiones }] } });
        assert!(retenciones_nuevas(None, &r(Value::Null, Value::Null)).is_empty(), "sin aplicar todavía");
        assert_eq!(retenciones_nuevas(None, &r(json!("2026-10-04T03:00:00Z"), json!(4))), vec![("b".to_string(), "docs".to_string())]);
        let antes = r(json!("2026-10-04T03:00:00Z"), json!(4));
        assert!(retenciones_nuevas(Some(&antes), &antes).is_empty(), "lo mismo otra vez");
        assert_eq!(retenciones_nuevas(Some(&antes), &r(json!("2026-10-05T03:00:00Z"), json!(4))).len(), 1, "otra vuelta");
        assert_eq!(retenciones_nuevas(Some(&antes), &r(json!("2026-10-04T03:00:00Z"), json!(3))).len(), 1);
        assert!(retenciones_nuevas(Some(&antes), &json!({})).is_empty());
    }

    #[test]
    fn el_dueno_por_su_destino_en_el_almacen() {
        let almacen = equipo("aaaaaaaa-1111-2222-3333-444444444444", "ALMACEN-A", json!({}));
        let b = equipo(
            "b",
            "EQUIPO-B",
            json!({
                "repositorios": [
                    { "id": "almacen-a-1234", "destino": "ALMACEN-A" },
                    { "id": "adoptado", "destino": "ALMACEN-A", "ruta": "siigo" },
                    { "id": "otro", "destino": "NAS" },
                ],
                "destinos": [
                    { "id": "almacen-aaaaaaaa", "nombre": "ALMACEN-A", "tipo": "rest", "donde": "https://localhost:8000/equipo-b/", "equipo_almacen": almacen.id },
                    { "id": "nas", "nombre": "NAS", "tipo": "rest", "donde": "https://nas:8000/equipo-b/" },
                ],
            }),
        );
        // Un agente anterior (sin `equipo_almacen`): por el id del destino.
        let c = equipo(
            "c",
            "EQUIPO-C",
            json!({
                "repositorios": [{ "id": "docs", "destino": "ALMACEN-A" }],
                "destinos": [{ "id": "almacen-aaaaaaaa", "nombre": "ALMACEN-A", "tipo": "rest", "donde": "rest:https://192.168.1.2:8000/equipo-c" }],
            }),
        );
        let equipos = [almacen.clone(), b, c];
        let cambios = [
            ("equipo-b".to_string(), "almacen-a-1234".to_string()),
            ("equipo-b".to_string(), "siigo".to_string()),
            ("equipo-c".to_string(), "docs".to_string()),
        ];
        assert_eq!(
            duenos(&almacen, &cambios, &equipos),
            vec![("b".to_string(), "almacen-a-1234".to_string()), ("b".to_string(), "adoptado".to_string()), ("c".to_string(), "docs".to_string())]
        );
        // El repositorio de otro usuario, o de otro almacén, no.
        assert!(duenos(&almacen, &[("equipo-c".to_string(), "almacen-a-1234".to_string())], &equipos).is_empty());
        let otro = equipo("zzzzzzzz-0000", "OTRO", json!({}));
        assert!(duenos(&otro, &cambios, &equipos).is_empty());
    }

    #[test]
    fn usuario_de_la_direccion() {
        assert_eq!(usuario_en_almacen("https://localhost:8000/equipo-b/"), Some("equipo-b"));
        assert_eq!(usuario_en_almacen("rest:http://10.0.0.1:8000/ana"), Some("ana"));
        assert_eq!(usuario_en_almacen("https://nas:8000/"), None);
        assert_eq!(usuario_en_almacen("https://nas:8000/a/b"), None);
        assert_eq!(usuario_en_almacen("D:\\Copias"), None);
    }
}
