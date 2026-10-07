//! Prueba de integración de «los datos comunes del cliente, iguales en todas sus
//! consolas» (0.7.26, bloque 8; docs/consolas-multiples.md §6.5), con **dos
//! Resguardo Server reales** (TLS) en el mismo proceso, como `datos_equipo_it.rs`.
//!
//! 1. B ya tenía un color para «Servidor» antes de compartir el equipo con A.
//! 2. A cambia colores (los guarda su pantalla de siempre: quedan «por enviar») y los
//!    manda al equipo con `datos_cliente`: B pone el que no tenía y enseña la
//!    diferencia del que sí tenía, sin pisarlo.
//! 3. En B se elige el suyo para todas: llega a A con «desde la consola …».
//! 4. El tipo y las marcas de un destino solo con la clave de administración.
//! 5. Una plantilla de A llega a B cifrada tal cual, «por traer», y se trae.

use crate::servidor_v2::{self as s, cargar, ronda};
use crate::traslado_it::{arrancar_en, enviar, estado, orden, Consola, Equipo, Srv, CLAVE};
use base64::Engine;
use resguardo_protocolo::{derivaciones, orden_v2};
use serde_json::{json, Value};
use std::time::Duration;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

fn arrancar(nombre: &str) -> (Srv, String) {
    arrancar_en(std::env::temp_dir().join(format!("resguardo-comunes-{nombre}-{}", std::process::id())))
}

fn esperar_estado(consola: &Consola, e: &Equipo, seq: u64, fin: &str) -> Value {
    for _ in 0..60 {
        let _ = ronda();
        let o = estado(consola, e, seq);
        if o["estado"] == fin {
            return o;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    panic!("la orden {seq} no llegó a «{fin}»: {}", estado(consola, e, seq));
}

/// Vueltas hasta que `ruta` en esa consola cumpla `cond`.
fn hasta(consola: &Consola, ruta: &str, que: &str, cond: impl Fn(&Value) -> bool) -> Value {
    for _ in 0..40 {
        let _ = ronda();
        let (_, x) = consola.pedir("GET", ruta, None);
        if cond(&x) {
            return x;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    panic!("{que}: {}", consola.pedir("GET", ruta, None).1);
}

fn color(etiquetas: &Value, nombre: &str) -> Option<u64> {
    etiquetas.as_array()?.iter().find(|a| a["nombre"] == nombre)?["color"].as_u64()
}

fn fila<'a>(datos: &'a Value, clave: &str) -> Option<&'a Value> {
    datos["filas"].as_array()?.iter().find(|f| f["clave"] == clave)
}

/// Las filas «por enviar» de esa consola, como entradas de la orden (lo que hace el navegador).
fn por_enviar(datos: &Value) -> Vec<Value> {
    datos["filas"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["por_enviar"] == true)
        .map(|f| json!({ "clave": f["clave"], "valor": f["valor"], "cambiado": f["cambiado"], "semilla": f["semilla"], "por": f["por"] }))
        .collect()
}

#[test]
fn datos_comunes_del_cliente_en_todas_sus_consolas() {
    let _real = crate::restic::tests::real_repo_lock();
    let dir_agente = std::env::temp_dir().join(format!("resguardo-comunes-agente-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir_agente);
    std::env::set_var("RESGUARDO_AGENT_DIR", &dir_agente);

    let (a, codigo_a) = arrancar("a");
    let (b, codigo_b) = arrancar("b");
    let ca = Consola::entrar(&a, &codigo_a);
    let cb = Consola::entrar(&b, &codigo_b);

    // En A: cliente, emparejamiento y alta.
    let (_, cliente) = ca.pedir("POST", "/api/clientes", Some(json!({ "nombre": "Panadería Ejemplo", "espera_min_horas": 1 })));
    let (c_a, sal_a) = (cliente["id"].as_str().unwrap().to_string(), cliente["sal_cliente"].as_str().unwrap().to_string());
    let (_, emp) = ca.pedir("POST", &format!("/api/clientes/{c_a}/emparejamientos"), None);
    let (codigo, emp_id) = (emp["codigo"].as_str().unwrap().to_string(), emp["id"].as_str().unwrap().to_string());
    s::vincular(&a.url, &codigo, "PRUEBA-COMUNES").unwrap();
    let (_, est) = ca.pedir("GET", &format!("/api/clientes/{c_a}/emparejamientos/{emp_id}"), None);
    let eq = &est["equipo"];
    let (id, box_pub, sign_pub) =
        (eq["id"].as_str().unwrap().to_string(), eq["box_pub"].as_str().unwrap().to_string(), eq["sign_pub"].as_str().unwrap().to_string());
    let kcfg_a = derivaciones::k_cfg(CLAVE, &sal_a).unwrap();
    let etiqueta_a = derivaciones::etiqueta_equipo(&kcfg_a, &id, &box_pub, &sign_pub);
    assert_eq!(ca.pedir("POST", &format!("/api/clientes/{c_a}/emparejamientos/{emp_id}/confirmar"), Some(json!({ "etiqueta": etiqueta_a }))).0, 200);
    let prueba = derivaciones::prueba_admin(CLAVE, eq["sal_equipo"].as_str().unwrap()).unwrap();
    let verificador = B64.encode(derivaciones::verificador(&prueba));
    let ea = Equipo { c: c_a.clone(), id: id.clone(), box_pub: box_pub.clone(), prueba };
    let mut alta = orden(&ea, 1, "alta", json!({ "verificador": verificador, "k_cfg": B64.encode(kcfg_a), "espera_min_horas": 1 }));
    alta.autorizacion.prueba_codigo = Some(orden_v2::prueba_codigo(&codigo, &id, &verificador));
    enviar(&ca, &ea, &alta);
    esperar_estado(&ca, &ea, 1, "hecha");

    // B: el mismo cliente (otra sal), que ya tenía «Servidor» en verde (3) antes de tener equipos.
    let sal_b = B64.encode(uuid::Uuid::new_v4().as_bytes());
    let (st, rec) = cb.pedir("POST", "/api/clientes/recibir", Some(json!({ "nombre": "Panadería Ejemplo", "sal_cliente": sal_b, "usos": 10 })));
    assert_eq!(st, 200, "{rec}");
    let c_b = rec["cliente"]["id"].as_str().unwrap().to_string();
    assert_eq!(cb.pedir("PUT", &format!("/api/clientes/{c_b}/etiquetas"), Some(json!({ "nombre": "Servidor", "color": 3 }))).0, 200);
    let (_, d_b) = cb.pedir("GET", &format!("/api/clientes/{c_b}/datos-comunes"), None);
    assert!(d_b["filas"].as_array().unwrap().is_empty(), "sin equipos que lo guarden, es solo de B: {d_b}");
    assert_eq!(d_b["sin_compartir"][0]["clave"], "etiqueta.color:servidor", "{d_b}");
    // Conectar también a B.
    let anadir = json!({
        "url": b.url, "identidad": b.identidad, "huella_ca": s::huella_de_una_autoridad(&b.ca).unwrap(),
        "ficha": rec["ficha"], "sal_cliente": sal_b, "k_cfg": B64.encode(derivaciones::k_cfg(CLAVE, &sal_b).unwrap()),
        "nombre": "Consola en línea", "sal_origen": sal_a,
    });
    enviar(&ca, &ea, &orden(&ea, 2, "anadir_consola", anadir));
    esperar_estado(&ca, &ea, 2, "hecha");
    let eb = Equipo { c: c_b.clone(), id: id.clone(), box_pub: box_pub.clone(), prueba };
    ronda().unwrap();

    // 1. En A, con su pantalla de siempre: quedan por enviar (el equipo admite los datos comunes).
    let ruta_a = format!("/api/clientes/{c_a}/datos-comunes");
    let ruta_b = format!("/api/clientes/{c_b}/datos-comunes");
    for (n, c) in [("Oficina", 2), ("Servidor", 1)] {
        assert_eq!(ca.pedir("PUT", &format!("/api/clientes/{c_a}/etiquetas"), Some(json!({ "nombre": n, "color": c }))).0, 200);
    }
    let (_, d_a) = ca.pedir("GET", &ruta_a, None);
    let entradas = por_enviar(&d_a);
    assert_eq!(entradas.len(), 2, "{d_a}");
    assert!(fila(&d_a, "etiqueta.color:oficina").is_some_and(|f| f["esta"] == true && f["estado"] == "aplicado"), "{d_a}");

    // 2. A los manda al equipo (sin clave: inofensiva) y los marca enviados.
    let mut o = orden(&ea, 3, "datos_cliente", json!({ "entradas": entradas }));
    o.autorizacion.prueba_admin = None;
    o.por = Some("Ana".into());
    enviar(&ca, &ea, &o);
    let r = esperar_estado(&ca, &ea, 3, "hecha");
    assert!(r["mensaje"].as_str().unwrap().contains("2 datos comunes"), "{r}");
    let marcas: Vec<Value> = entradas.iter().map(|e| json!({ "clave": e["clave"], "cambiado": e["cambiado"] })).collect();
    let (_, n) = ca.pedir("POST", &format!("{ruta_a}/enviadas"), Some(json!({ "entradas": marcas })));
    assert_eq!(n["n"], 2);
    let v = cargar().unwrap();
    assert_eq!(v.datos_cliente.len(), 2);
    assert_eq!(v.datos_cliente["etiqueta.color:oficina"].identidad, a.identidad);
    // B pone el que no tenía y enseña la diferencia del que sí tenía (no lo pisa).
    let et_b = hasta(&cb, &format!("/api/clientes/{c_b}/etiquetas"), "el color de A llega a B", |x| color(x, "Oficina") == Some(2));
    assert_eq!(color(&et_b, "Servidor"), Some(3), "lo de B no se pisa sin preguntar");
    let oficina = et_b.as_array().unwrap().iter().find(|x| x["nombre"] == "Oficina").unwrap();
    assert_eq!(oficina["por"], "Ana (desde otra consola)", "la principal no tiene nombre en el equipo y nunca va su dirección: {oficina}");
    let (_, d_b) = cb.pedir("GET", &ruta_b, None);
    let dif = fila(&d_b, "etiqueta.color:servidor").unwrap_or_else(|| panic!("{d_b}"));
    assert_eq!((dif["estado"].as_str(), dif["valor"]["color"].as_u64(), dif["local"]["color"].as_u64()), (Some("conflicto"), Some(1), Some(3)), "{dif}");
    assert_eq!(dif["esta"], false);
    assert!(!d_b.to_string().contains(&a.url), "nunca la dirección de otra consola");
    let (_, aud) = cb.pedir("GET", &format!("/api/clientes/{c_b}/auditoria?orden=desc&limite=50"), None);
    assert!(aud.to_string().contains("datos_comunes_diferencia"), "{aud}");

    // 3. En B se elige el suyo (verde) para todas: llega a A.
    let (st, puesto) =
        cb.pedir("POST", &ruta_b, Some(json!({ "entradas": [{ "clave": "etiqueta.color:servidor", "valor": { "nombre": "Servidor", "color": 3 } }] })));
    assert_eq!(st, 200, "{puesto}");
    let mut o = orden(&eb, 1, "datos_cliente", json!({ "entradas": puesto["entradas"] }));
    o.autorizacion.prueba_admin = None;
    enviar(&cb, &eb, &o);
    esperar_estado(&cb, &eb, 1, "hecha");
    let (_, d_b) = cb.pedir("GET", &ruta_b, None);
    assert_eq!(fila(&d_b, "etiqueta.color:servidor").unwrap()["estado"], "aplicado", "{d_b}");
    let et_a = hasta(&ca, &format!("/api/clientes/{c_a}/etiquetas"), "la elección de B llega a A", |x| color(x, "Servidor") == Some(3));
    let servidor = et_a.as_array().unwrap().iter().find(|x| x["nombre"] == "Servidor").unwrap();
    assert!(servidor["por"].as_str().unwrap().contains("«Consola en línea»"), "{servidor}");

    // 4. El tipo y las marcas de un destino piden la clave de administración.
    let regla = json!({ "entradas": [{
        "clave": "destino.regla:zona:e9:principal",
        "valor": { "clase": "zona", "atributos": { "tipo": "fuera", "inmutable": "solo_anadir" } },
        "cambiado": chrono::Utc::now().to_rfc3339(),
    }]});
    let mut sin_clave = orden(&ea, 4, "datos_cliente", regla.clone());
    sin_clave.autorizacion.prueba_admin = None;
    enviar(&ca, &ea, &sin_clave);
    let r = esperar_estado(&ca, &ea, 4, "fallida");
    assert!(r["mensaje"].as_str().unwrap().contains("clave de administración"), "{r}");
    let mut mala = orden(&ea, 5, "datos_cliente_admin", regla.clone());
    mala.autorizacion.prueba_admin = Some(B64.encode([7u8; 32]));
    enviar(&ca, &ea, &mala);
    let r = esperar_estado(&ca, &ea, 5, "rechazada");
    assert!(r["mensaje"].as_str().unwrap().contains("no es correcta"), "{r}");
    assert!(!cargar().unwrap().datos_cliente.contains_key("destino.regla:zona:e9:principal"));
    enviar(&ca, &ea, &orden(&ea, 6, "datos_cliente_admin", regla));
    esperar_estado(&ca, &ea, 6, "hecha");
    let cat_b = hasta(&cb, &format!("/api/clientes/{c_b}/destinos"), "las marcas llegan a B", |x| x.as_array().is_some_and(|l| !l.is_empty()));
    assert_eq!(cat_b[0]["atributos"], json!({ "tipo": "fuera", "inmutable": "solo_anadir" }), "{cat_b}");

    // 5. Una plantilla de A, cifrada como la guardó A (con su sal y su cliente): en B, por traer.
    let cifrado = B64.encode([5u8; 120]);
    assert_eq!(ca.pedir("PUT", &format!("/api/clientes/{c_a}/plantillas/pla-1"), Some(json!({ "cifrado": cifrado }))).0, 204);
    let (_, d_a) = ca.pedir("GET", &ruta_a, None);
    let entradas = por_enviar(&d_a);
    assert_eq!(entradas.len(), 1, "{d_a}");
    assert_eq!(entradas[0]["valor"], json!({ "cifrado": cifrado, "sal": sal_a, "cliente": c_a }));
    let mut o = orden(&ea, 7, "datos_cliente", json!({ "entradas": entradas }));
    o.autorizacion.prueba_admin = None;
    enviar(&ca, &ea, &o);
    esperar_estado(&ca, &ea, 7, "hecha");
    let d_b = hasta(&cb, &ruta_b, "la plantilla llega a B", |x| fila(x, "plantilla:pla-1").is_some());
    let p = fila(&d_b, "plantilla:pla-1").unwrap();
    assert_eq!(p["estado"], "por_traer");
    let (_, pl_b) = cb.pedir("GET", &format!("/api/clientes/{c_b}/plantillas"), None);
    assert!(pl_b.as_array().unwrap().is_empty(), "B no la puede abrir: aún no está entre las suyas");
    let otra = B64.encode([6u8; 120]);
    let (st, r) = cb.pedir("POST", &format!("{ruta_b}/traer"), Some(json!({ "clave": "plantilla:pla-1", "cambiado": p["cambiado"], "cifrado": otra })));
    assert_eq!(st, 200, "{r}");
    let (_, pl_b) = cb.pedir("GET", &format!("/api/clientes/{c_b}/plantillas"), None);
    assert_eq!(pl_b[0]["cifrado"], otra.as_str());
    let (_, d_b) = cb.pedir("GET", &ruta_b, None);
    let p = fila(&d_b, "plantilla:pla-1").unwrap();
    assert_eq!((p["estado"].as_str(), p["por_enviar"].as_bool()), (Some("aplicado"), Some(false)), "traerla no la vuelve a repartir: {p}");

    // El resumen dice cuántos datos tiene el equipo (el documento va con la configuración).
    let (_, x_b) = cb.pedir("GET", &format!("/api/clientes/{c_b}/equipos/{id}"), None);
    assert!(x_b["resumen"]["datos_cliente"]["n"].as_u64().unwrap() >= 4, "{}", x_b["resumen"]["datos_cliente"]);
    assert!(x_b["resumen"]["admite"].to_string().contains("datos_cliente"));

    std::env::remove_var("RESGUARDO_AGENT_DIR");
    let _ = std::fs::remove_dir_all(dir_agente);
}
