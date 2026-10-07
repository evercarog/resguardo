//! 0.7.26 (bloque 8): los **datos comunes del cliente** que el equipo guarda para
//! todas sus consolas (docs/consolas-multiples.md §6.5): colores de las etiquetas,
//! catálogo de destinos y plantillas de copia (cifradas, tal cual).
//!
//! Cada consola manda sus cambios a todos los equipos del cliente con la orden
//! `datos_cliente` (o `datos_cliente_admin`, con la clave de administración, para
//! el tipo y las marcas de un destino); el equipo los junta en su documento
//! (gana el cambio más reciente de cada dato, `resguardo_protocolo::datos_cliente`)
//! y lo sube con su configuración a todas sus consolas, que lo juntan con lo de
//! sus demás equipos. El equipo no entiende nada de esto: solo valida, guarda y
//! reparte. Nunca descifra una plantilla.

use crate::servidor_v2::Vinculo;
use resguardo_protocolo::datos_cliente::{self as dc, Documento, Entrada};
use serde_json::{json, Value};

/// Tolerancia del reloj de una consola: un cambio «del futuro» se apunta con la hora del equipo.
const ADELANTO_MAX_MS: i64 = 5 * 60 * 1000;

/// `datos_cliente { entradas: [{ clave, valor, cambiado, semilla?, por? }] }`. `admin`: la orden
/// llegó con la clave de administración (`datos_cliente_admin`). Todo o nada.
pub fn aplicar(v: &mut Vinculo, c: &Value, admin: bool, por: Option<&str>) -> Result<String, String> {
    let entradas = c["entradas"].as_array().ok_or("Faltan los datos (una lista).")?;
    if entradas.is_empty() || entradas.len() > dc::MAX_ENTRADAS_ORDEN {
        return Err(format!("De 1 a {} datos por orden.", dc::MAX_ENTRADAS_ORDEN));
    }
    let ahora = chrono::Utc::now();
    let mut doc: Documento = v.datos_cliente.clone();
    let mut cambiados = 0;
    for x in entradas {
        let clave = x["clave"].as_str().ok_or("Falta la clave de un dato.")?;
        if dc::clase(clave).is_none() {
            return Err("Dato común desconocido.".into());
        }
        if dc::pide_admin(clave) && !admin {
            return Err("El tipo y las marcas de un destino cuentan en la regla 3-2-1: cambiarlos pide la clave de administración.".into());
        }
        let valor = dc::valor_valido(clave, &x["valor"])?;
        let ms = x["cambiado"].as_str().and_then(dc::milis).ok_or("Falta la hora del cambio.")?;
        // Un reloj de consola muy adelantado no deja un dato «para siempre» por delante.
        let cambiado = if ms > ahora.timestamp_millis() + ADELANTO_MAX_MS {
            ahora.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        } else {
            x["cambiado"].as_str().unwrap_or_default().to_string()
        };
        let quien = x["por"].as_str().or(por).map(|p| dc::texto_corto(p, 60)).filter(|p| !p.is_empty());
        let e = Entrada {
            valor,
            cambiado,
            consola: dc::texto_corto(&crate::consolas_v2::nombre_de(&v.nombre_consola, ""), 60),
            identidad: v.identidad.clone(),
            por: quien,
            semilla: x["semilla"] == true,
        };
        if dc::fusionar(&mut doc, clave, e) {
            cambiados += 1;
        }
    }
    if doc.len() > dc::MAX_ENTRADAS || dc::bytes(&doc) > dc::MAX_BYTES {
        return Err(format!(
            "No cabe: los datos comunes del cliente ocupan como mucho {} KiB en cada equipo (quita plantillas que no uses).",
            dc::MAX_BYTES / 1024
        ));
    }
    v.datos_cliente = doc;
    Ok(match cambiados {
        0 => "Ya los tenía (o tenía otros más recientes).".to_string(),
        1 => "1 dato común del cliente guardado: lo verán todas sus consolas.".to_string(),
        n => format!("{n} datos comunes del cliente guardados: los verán todas sus consolas."),
    })
}

/// Qué cambia la orden, en palabras, para el historial común (sin valores largos).
pub fn descripcion(c: &Value) -> Option<String> {
    let entradas = c["entradas"].as_array()?;
    let una = |x: &Value| -> String {
        let clave = x["clave"].as_str().unwrap_or("");
        let sujeto = |p: &str| clave.strip_prefix(p).unwrap_or("").chars().take(40).collect::<String>();
        match dc::clase(clave).map(|x| x.0) {
            Some(dc::Clase::EtiquetaColor) => {
                format!("el color de la etiqueta «{}»", x["valor"]["nombre"].as_str().map(str::to_string).unwrap_or_else(|| sujeto("etiqueta.color:")))
            }
            Some(dc::Clase::EtiquetaPlantilla) => {
                format!("la plantilla de la etiqueta «{}»", x["valor"]["nombre"].as_str().map(str::to_string).unwrap_or_else(|| sujeto("etiqueta.plantilla:")))
            }
            Some(dc::Clase::Destino) => "el nombre de un destino".to_string(),
            Some(dc::Clase::DestinoRegla) => "el tipo y las marcas de un destino".to_string(),
            Some(dc::Clase::Plantilla) => "una plantilla de copia".to_string(),
            None => "un dato".to_string(),
        }
    };
    Some(match entradas.as_slice() {
        [] => return None,
        [x] => format!("Cambiar {} en todas las consolas", una(x)),
        xs => format!("Cambiar {} datos comunes del cliente en todas las consolas", xs.len()),
    })
}

/// `resumen.datos_cliente`: cuántos datos tiene el equipo y su huella (`null` si ninguno).
/// El documento entero va con la configuración (`/api/agente/config`, `datos_cliente`).
pub fn resumen(v: &Vinculo) -> Value {
    if v.datos_cliente.is_empty() {
        return Value::Null;
    }
    json!({ "n": v.datos_cliente.len(), "huella": dc::huella(&v.datos_cliente), "bytes": dc::bytes(&v.datos_cliente) })
}

/// El documento para subirlo con la configuración (`None` si está vacío).
pub fn documento(v: &Vinculo) -> Option<Value> {
    (!v.datos_cliente.is_empty()).then(|| serde_json::to_value(&v.datos_cliente).unwrap_or(Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    fn vinculo(identidad: &str, nombre: &str) -> Vinculo {
        Vinculo { identidad: identidad.into(), nombre_consola: nombre.into(), ..Default::default() }
    }

    fn color(nombre: &str, color: u8, cambiado: &str) -> Value {
        json!({ "clave": format!("etiqueta.color:{}", nombre.to_lowercase()), "valor": { "nombre": nombre, "color": color }, "cambiado": cambiado })
    }

    #[test]
    fn guarda_y_gana_el_mas_reciente() {
        let mut v = vinculo("id-oficina", "Oficina");
        let m = aplicar(&mut v, &json!({ "entradas": [color("Servidor", 1, "2026-10-07T10:00:00Z")] }), false, Some("Ana")).unwrap();
        assert!(m.contains("1 dato"), "{m}");
        let e = &v.datos_cliente["etiqueta.color:servidor"];
        assert_eq!((e.valor["color"].as_u64(), e.consola.as_str(), e.identidad.as_str(), e.por.as_deref()), (Some(1), "Oficina", "id-oficina", Some("Ana")));
        // Uno más antiguo (de otra consola, que llegó tarde) no lo pisa.
        let mut otra = v.clone();
        otra.identidad = "id-en-linea".into();
        otra.nombre_consola = "En línea".into();
        let m = aplicar(&mut otra, &json!({ "entradas": [color("Servidor", 4, "2026-10-07T09:00:00Z")] }), false, None).unwrap();
        assert!(m.starts_with("Ya los tenía"), "{m}");
        assert_eq!(otra.datos_cliente["etiqueta.color:servidor"].valor["color"], 1);
        // Uno más reciente sí, con la consola que lo mandó (la del vínculo, no la que diga la orden).
        aplicar(&mut otra, &json!({ "entradas": [color("Servidor", 4, "2026-10-07T11:00:00Z")] }), false, None).unwrap();
        let e = &otra.datos_cliente["etiqueta.color:servidor"];
        assert_eq!((e.valor["color"].as_u64(), e.consola.as_str(), e.identidad.as_str()), (Some(4), "En línea", "id-en-linea"));
        // Una semilla (lo que ya tenía otra consola) nunca pisa un cambio.
        let mut s = color("Servidor", 6, "2026-10-08T11:00:00Z");
        s["semilla"] = json!(true);
        aplicar(&mut otra, &json!({ "entradas": [s] }), false, None).unwrap();
        assert_eq!(otra.datos_cliente["etiqueta.color:servidor"].valor["color"], 4);
    }

    #[test]
    fn reloj_adelantado_y_lo_que_no_vale() {
        let mut v = vinculo("id", "Oficina");
        aplicar(&mut v, &json!({ "entradas": [color("Servidor", 1, "2099-01-01T00:00:00Z")] }), false, None).unwrap();
        let ms = dc::milis(&v.datos_cliente["etiqueta.color:servidor"].cambiado).unwrap();
        assert!(ms <= chrono::Utc::now().timestamp_millis(), "con la hora del equipo");
        for malo in [
            json!({}),
            json!({ "entradas": [] }),
            json!({ "entradas": [{ "clave": "inventada:x", "valor": null, "cambiado": "2026-10-07T10:00:00Z" }] }),
            json!({ "entradas": [{ "clave": "etiqueta.color:servidor", "valor": { "nombre": "Servidor", "color": 9 }, "cambiado": "2026-10-07T10:00:00Z" }] }),
            json!({ "entradas": [{ "clave": "etiqueta.color:servidor", "valor": null }] }),
            json!({ "entradas": (0..101).map(|i| color(&format!("e{i}"), 1, "2026-10-07T10:00:00Z")).collect::<Vec<_>>() }),
        ] {
            let antes = v.datos_cliente.clone();
            assert!(aplicar(&mut v, &malo, true, None).is_err(), "{malo}");
            assert_eq!(v.datos_cliente, antes, "nada a medias");
        }
    }

    #[test]
    fn el_tipo_y_las_marcas_piden_la_clave() {
        let mut v = vinculo("id", "Oficina");
        let regla = json!({ "entradas": [
            color("Servidor", 2, "2026-10-07T10:00:00Z"),
            { "clave": "destino.regla:zona:e1:principal", "valor": { "clase": "zona", "atributos": { "tipo": "fuera", "inmutable": "solo_anadir" } }, "cambiado": "2026-10-07T10:00:00Z" },
        ]});
        let e = aplicar(&mut v, &regla, false, None).unwrap_err();
        assert!(e.contains("clave de administración"), "{e}");
        assert!(v.datos_cliente.is_empty(), "ni siquiera el color: todo o nada");
        aplicar(&mut v, &regla, true, None).unwrap();
        assert_eq!(v.datos_cliente.len(), 2);
        // El nombre de un destino no la pide.
        let nombre = json!({ "entradas": [{ "clave": "destino:zona:e1:principal", "valor": { "nombre": "Almacén · Disco D", "clase": "zona" }, "cambiado": "2026-10-07T10:00:00Z" }] });
        aplicar(&mut v, &nombre, false, None).unwrap();
    }

    #[test]
    fn tope_de_tamano() {
        let mut v = vinculo("id", "Oficina");
        let sal = base64::engine::general_purpose::STANDARD.encode([1u8; 16]);
        let grande = base64::engine::general_purpose::STANDARD.encode(vec![7u8; 30 * 1024]);
        let mut n = 0;
        let error = loop {
            let o = json!({ "entradas": [{ "clave": format!("plantilla:pla-{n}"), "valor": { "cifrado": grande, "sal": sal, "cliente": "c1" }, "cambiado": "2026-10-07T10:00:00Z" }] });
            match aplicar(&mut v, &o, false, None) {
                Ok(_) => n += 1,
                Err(e) => break e,
            }
            assert!(n < 20, "tendría que haberse llenado");
        };
        assert!(error.contains("No cabe"), "{error}");
        assert!(dc::bytes(&v.datos_cliente) <= dc::MAX_BYTES);
        assert_eq!(v.datos_cliente.len(), n);
        // Quitar una (null) libera sitio.
        let quitar = json!({ "entradas": [{ "clave": "plantilla:pla-0", "valor": null, "cambiado": "2026-10-07T11:00:00Z" }] });
        aplicar(&mut v, &quitar, false, None).unwrap();
        assert!(v.datos_cliente["plantilla:pla-0"].valor.is_null());
    }

    #[test]
    fn resumen_y_descripcion_sin_valores() {
        let mut v = vinculo("id", "Oficina");
        assert!(resumen(&v).is_null() && documento(&v).is_none());
        v.url = "https://oficina.ejemplo.com".into();
        aplicar(&mut v, &json!({ "entradas": [color("Servidor", 1, "2026-10-07T10:00:00Z")] }), false, None).unwrap();
        let r = resumen(&v);
        assert_eq!(r["n"], 1);
        assert_eq!(r["huella"].as_str().unwrap().len(), 16);
        let d = documento(&v).unwrap();
        assert_eq!(d["etiqueta.color:servidor"]["consola"], "Oficina");
        assert!(!d.to_string().contains("ejemplo.com"), "nunca la dirección de una consola");
        assert_eq!(
            descripcion(&json!({ "entradas": [color("Servidor", 1, "x")] })).unwrap(),
            "Cambiar el color de la etiqueta «Servidor» en todas las consolas"
        );
        assert!(descripcion(&json!({ "entradas": [color("a", 1, "x"), color("b", 1, "x")] })).unwrap().contains("2 datos"));
        // Persiste en el vínculo; uno anterior (sin el campo) se lee vacío.
        let leido: Vinculo = serde_json::from_str(&serde_json::to_string(&v).unwrap()).unwrap();
        assert_eq!(leido.datos_cliente, v.datos_cliente);
        let mut viejo = serde_json::to_value(vinculo("id", "x")).unwrap();
        viejo.as_object_mut().unwrap().remove("datos_cliente");
        assert!(serde_json::from_value::<Vinculo>(viejo).unwrap().datos_cliente.is_empty());
    }
}
