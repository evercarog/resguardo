//! Prueba de integración de «los datos del equipo, iguales en todas sus consolas»
//! (docs/consolas-multiples.md §6) y de «Quitar este destino», con **dos Resguardo
//! Server reales** (TLS) en el mismo proceso, como `consolas_it.rs`.
//!
//! 1. Convivencia: cada consola tiene su nombre del equipo y nada lo pisa mientras
//!    nadie lo cambie con la orden.
//! 2. `nombre_equipo` desde A: el nombre cambia en A y en B, y queda en el historial
//!    común con la consola que lo cambió. `etiquetas_equipo` y `observacion_equipo`
//!    desde B llegan a A.
//! 3. Un cambio local en una consola (como hasta ahora) con el equipo ya con nombre
//!    propio: manda el del equipo.
//! 4. `quitar_destino` (con la clave de administración): no quita uno en uso; quita uno vacío sin borrar nada de su
//!    carpeta (y lo dice si aún tiene copias). `quitar_repositorio { quitar_destino }`
//!    olvida también el destino que se queda vacío.

use crate::servidor_v2::{self as s, cargar, guardar, ronda};
use crate::traslado_it::{arrancar_en, enviar, estado, orden, Consola, Equipo, Srv, CLAVE};
use base64::Engine;
use resguardo_protocolo::{derivaciones, orden_v2};
use serde_json::{json, Value};
use std::time::Duration;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

fn arrancar(nombre: &str) -> (Srv, String) {
    arrancar_en(std::env::temp_dir().join(format!("resguardo-datos-{nombre}-{}", std::process::id())))
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

fn equipo_en(consola: &Consola, e: &Equipo) -> Value {
    consola.pedir("GET", &format!("/api/clientes/{}/equipos/{}", e.c, e.id), None).1
}

/// Vueltas hasta que en esa consola el equipo cumpla `cond` (las demás consolas reciben
/// la configuración nueva por su canal o en la siguiente vuelta).
fn hasta(consola: &Consola, e: &Equipo, que: &str, cond: impl Fn(&Value) -> bool) -> Value {
    for _ in 0..40 {
        let _ = ronda();
        let x = equipo_en(consola, e);
        if cond(&x) {
            return x;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    panic!("{que}: {}", equipo_en(consola, e));
}

#[test]
fn datos_del_equipo_en_todas_sus_consolas_y_quitar_destinos() {
    let _real = crate::restic::tests::real_repo_lock();
    let dir_agente = std::env::temp_dir().join(format!("resguardo-datos-agente-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir_agente);
    std::env::set_var("RESGUARDO_AGENT_DIR", &dir_agente);

    let (a, codigo_a) = arrancar("a");
    let (b, codigo_b) = arrancar("b");
    let ca = Consola::entrar(&a, &codigo_a);
    let cb = Consola::entrar(&b, &codigo_b);

    // En A: cliente, emparejamiento y alta (como en consolas_it.rs).
    let (_, cliente) = ca.pedir("POST", "/api/clientes", Some(json!({ "nombre": "Panadería Ejemplo", "espera_min_horas": 1 })));
    let (c_a, sal_a) = (cliente["id"].as_str().unwrap().to_string(), cliente["sal_cliente"].as_str().unwrap().to_string());
    let (_, emp) = ca.pedir("POST", &format!("/api/clientes/{c_a}/emparejamientos"), None);
    let (codigo, emp_id) = (emp["codigo"].as_str().unwrap().to_string(), emp["id"].as_str().unwrap().to_string());
    s::vincular(&a.url, &codigo, "PRUEBA-DATOS").unwrap();
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

    // Conectar también a B (otra sal).
    let sal_b = B64.encode(uuid::Uuid::new_v4().as_bytes());
    let (st, rec) = cb.pedir("POST", "/api/clientes/recibir", Some(json!({ "nombre": "Panadería Ejemplo", "sal_cliente": sal_b, "usos": 10 })));
    assert_eq!(st, 200, "{rec}");
    let c_b = rec["cliente"]["id"].as_str().unwrap().to_string();
    let anadir = json!({
        "url": b.url, "identidad": b.identidad, "huella_ca": s::huella_de_una_autoridad(&b.ca).unwrap(),
        "ficha": rec["ficha"], "sal_cliente": sal_b, "k_cfg": B64.encode(derivaciones::k_cfg(CLAVE, &sal_b).unwrap()),
        "nombre": "Consola en línea", "sal_origen": sal_a,
    });
    enviar(&ca, &ea, &orden(&ea, 2, "anadir_consola", anadir));
    esperar_estado(&ca, &ea, 2, "hecha");
    let eb = Equipo { c: c_b.clone(), id: id.clone(), box_pub: box_pub.clone(), prueba };
    ronda().unwrap();

    // 1. Convivencia: un nombre distinto en cada consola (como antes de actualizar). El equipo
    // admite los datos compartidos, pero nadie los ha puesto: ninguna consola pisa a la otra.
    assert_eq!(cb.pedir("PATCH", &format!("/api/clientes/{c_b}/equipos/{id}"), Some(json!({ "nombre": "Recepción (en línea)" }))).0, 200);
    let mut v = cargar().unwrap();
    v.otras[0].config_pendiente = true;
    guardar(&v).unwrap();
    ronda().unwrap();
    ronda().unwrap();
    let (x_a, x_b) = (equipo_en(&ca, &ea), equipo_en(&cb, &eb));
    assert!(x_a["resumen"]["admite"].to_string().contains("datos_equipo"), "{x_a}");
    assert!(x_b["resumen"]["datos_equipo"].is_null(), "nada puesto todavía: {x_b}");
    assert_eq!((x_a["nombre"].as_str(), x_b["nombre"].as_str()), (Some("PRUEBA-DATOS"), Some("Recepción (en línea)")));

    // 2. El nombre desde A: cambia en las dos, y el historial común dice desde qué consola.
    let mut o = orden(&ea, 3, "nombre_equipo", json!({ "nombre": "  Recepción  " }));
    o.autorizacion.prueba_admin = None; // inofensiva: no pide clave
    o.por = Some("Ana".into());
    enviar(&ca, &ea, &o);
    let r = esperar_estado(&ca, &ea, 3, "hecha");
    assert!(r["mensaje"].as_str().unwrap().contains("Recepción"), "{r}");
    assert_eq!(equipo_en(&ca, &ea)["nombre"], "Recepción");
    let x_b = hasta(&cb, &eb, "el nombre nuevo llega a B", |x| x["nombre"] == "Recepción");
    assert_eq!(x_b["resumen"]["datos_equipo"]["nombre"]["valor"], "Recepción");
    assert_eq!(x_b["resumen"]["datos_equipo"]["nombre"]["esta"], false, "lo cambió la otra consola");
    assert_eq!(x_b["resumen"]["datos_equipo"]["nombre"]["por"], "Ana");
    assert!(!x_b["resumen"]["datos_equipo"].to_string().contains(&a.url), "nunca la dirección de otra consola");
    let (_, hist) = cb.pedir("GET", &format!("/api/clientes/{c_b}/equipos/{id}/historial?tipo=orden&limite=50"), None);
    let entrada = hist.as_array().unwrap().iter().find(|h| h["orden"] == "nombre_equipo").cloned().unwrap_or_else(|| panic!("{hist}"));
    assert_eq!(entrada["identidad"], a.identidad.as_str());
    assert_eq!(entrada["resultado"], "hecha");
    assert!(entrada["descripcion"].as_str().unwrap().contains("Recepción"), "{entrada}");
    // En la auditoría de B queda que lo cambió el equipo.
    let (_, aud) = cb.pedir("GET", &format!("/api/clientes/{c_b}/auditoria?orden=desc&limite=20"), None);
    assert!(aud.to_string().contains("desde_equipo"), "{aud}");

    // Un técnico no puede cambiar el nombre (como `PATCH`); sí las etiquetas. Lo comprueba el
    // servidor con la tabla de órdenes: aquí basta con que la orden del nombre sea de administradores.
    assert!(resguardo_protocolo::ordenes::tipo("nombre_equipo").unwrap().solo_administradores);

    // 3. Etiquetas y observación desde B: llegan a A.
    let mut o = orden(&eb, 1, "etiquetas_equipo", json!({ "etiquetas": ["Contabilidad", "contabilidad", "Sede norte"] }));
    o.autorizacion.prueba_admin = None;
    enviar(&cb, &eb, &o);
    esperar_estado(&cb, &eb, 1, "hecha");
    let mut o = orden(&eb, 2, "observacion_equipo", json!({ "texto": "Disco nuevo el 3/10" }));
    o.autorizacion.prueba_admin = None;
    o.por = Some("Luis".into());
    enviar(&cb, &eb, &o);
    esperar_estado(&cb, &eb, 2, "hecha");
    let x_a = hasta(&ca, &ea, "las etiquetas de B llegan a A", |x| x["etiquetas"] == json!(["Contabilidad", "Sede norte"]));
    assert_eq!(x_a["resumen"]["datos_equipo"]["etiquetas"]["esta"], false);
    let (_, nota) = ca.pedir("GET", &format!("/api/clientes/{c_a}/notas/objeto?tipo=equipo&objeto={id}"), None);
    assert_eq!(nota["observacion"]["texto"], "Disco nuevo el 3/10", "{nota}");
    assert!(nota["observacion"]["por"].as_str().unwrap().starts_with("Luis (desde la consola"), "{nota}");
    // Y en B también (la consola que la mandó recibe el resumen al momento).
    let (_, nota_b) = cb.pedir("GET", &format!("/api/clientes/{c_b}/notas/objeto?tipo=equipo&objeto={id}"), None);
    assert_eq!(nota_b["observacion"]["texto"], "Disco nuevo el 3/10", "{nota_b}");

    // 4. Un cambio local en A (como hacía una consola anterior) con el equipo ya con nombre
    // propio: en el siguiente resumen manda el del equipo.
    assert_eq!(ca.pedir("PATCH", &format!("/api/clientes/{c_a}/equipos/{id}"), Some(json!({ "nombre": "Solo en A" }))).0, 200);
    assert_eq!(equipo_en(&ca, &ea)["nombre"], "Solo en A");
    let mut v = cargar().unwrap();
    crate::gestion_v2::subir_config(&mut v).unwrap();
    guardar(&v).unwrap();
    assert_eq!(equipo_en(&ca, &ea)["nombre"], "Recepción", "el del equipo manda");

    // 5. Quitar destinos. Uno local con un repositorio de restic dentro (de mentira: solo su
    // forma) y otro que usa un repositorio.
    let carpeta = dir_agente.join("destino-local");
    for d in ["repo-viejo/data", "repo-viejo/keys"] {
        std::fs::create_dir_all(carpeta.join(d)).unwrap();
    }
    std::fs::write(carpeta.join("repo-viejo/config"), b"x").unwrap();
    let mut v = cargar().unwrap();
    v.destinos.push(crate::gestion_v2::Destino {
        id: "d-local".into(),
        nombre: "Disco del equipo".into(),
        tipo: "local".into(),
        donde: carpeta.to_string_lossy().into(),
        ..Default::default()
    });
    v.destinos.push(crate::gestion_v2::Destino {
        id: "d-usado".into(),
        nombre: "Servidor de copias".into(),
        tipo: "rest".into(),
        donde: "https://almacen.ejemplo:8000/".into(),
        ..Default::default()
    });
    v.destinos.push(crate::gestion_v2::Destino {
        id: "d-vacio".into(),
        nombre: "Disco 2".into(),
        tipo: "local".into(),
        donde: dir_agente.join("vacio").to_string_lossy().into(),
        ..Default::default()
    });
    v.repos_v2.push(crate::gestion_v2::RepoV2 {
        id: "r1".into(),
        nombre: "Documentos".into(),
        destino: "d-usado".into(),
        contrasena: "x".into(),
        ..Default::default()
    });
    guardar(&v).unwrap();
    // Pide la clave de administración (v1.59): sin ella (una consola anterior, que la
    // mandaba como inofensiva) se rechaza con un mensaje claro y no cuenta como intento fallido.
    let mut sin_clave = orden(&ea, 4, "quitar_destino", json!({ "destino": "d-vacio" }));
    sin_clave.autorizacion.prueba_admin = None;
    enviar(&ca, &ea, &sin_clave);
    let r = esperar_estado(&ca, &ea, 4, "rechazada");
    assert!(r["mensaje"].as_str().unwrap().contains("clave de administración"), "{r}");
    assert!(cargar().unwrap().destinos.iter().any(|d| d.id == "d-vacio"), "sin la clave no se quita nada");
    // Con una clave equivocada, tampoco (y sí cuenta como intento fallido).
    let mut mala = orden(&ea, 5, "quitar_destino", json!({ "destino": "d-vacio" }));
    mala.autorizacion.prueba_admin = Some(B64.encode([7u8; 32]));
    enviar(&ca, &ea, &mala);
    let r = esperar_estado(&ca, &ea, 5, "rechazada");
    assert!(r["mensaje"].as_str().unwrap().contains("no es correcta"), "{r}");
    let quitar = |seq: u64, destino: &str| orden(&ea, seq, "quitar_destino", json!({ "destino": destino }));
    enviar(&ca, &ea, &quitar(6, "d-usado"));
    let r = esperar_estado(&ca, &ea, 6, "fallida");
    assert!(r["mensaje"].as_str().unwrap().contains("Documentos"), "{r}");
    enviar(&ca, &ea, &quitar(7, "d-local"));
    let r = esperar_estado(&ca, &ea, 7, "hecha");
    assert!(r["mensaje"].as_str().unwrap().contains("aún tiene copias guardadas (1 repositorio)"), "{r}");
    assert!(carpeta.join("repo-viejo/config").is_file(), "nunca se borra nada de la carpeta");
    enviar(&ca, &ea, &quitar(8, "d-vacio"));
    let r = esperar_estado(&ca, &ea, 8, "hecha");
    assert!(r["mensaje"].as_str().unwrap().contains("No se ha borrado nada"), "{r}");
    enviar(&ca, &ea, &quitar(9, "d-vacio"));
    esperar_estado(&ca, &ea, 9, "fallida");
    let ids: Vec<String> = cargar().unwrap().destinos.iter().map(|d| d.id.clone()).collect();
    assert_eq!(ids, vec!["d-usado".to_string()]);
    // B lo ve también (sin ese destino en su resumen).
    let x_b = hasta(&cb, &eb, "B deja de ver los destinos quitados", |x| x["resumen"]["destinos"].as_array().is_some_and(|l| l.len() == 1));
    assert_eq!(x_b["resumen"]["destinos"][0]["id"], "d-usado");

    // `quitar_repositorio { quitar_destino: true }`: el destino que se queda sin uso también se va
    // (la orden pasa por su espera y su contraseña; aquí, la parte del equipo).
    let mut v = cargar().unwrap();
    let m = crate::gestion_v2::dejar_de_copiar(&mut v, "r1", true, true).unwrap();
    assert!(m.contains("Servidor de copias") && m.contains("Lo guardado allí se queda"), "{m}");
    assert!(v.destinos.is_empty() && v.repos_v2.is_empty());
    // Sin la marca (una consola anterior), el destino se queda como siempre.
    let mut v = cargar().unwrap();
    crate::gestion_v2::dejar_de_copiar(&mut v, "r1", true, false).unwrap();
    assert_eq!(v.destinos.len(), 1);

    std::env::remove_var("RESGUARDO_AGENT_DIR");
    let _ = std::fs::remove_dir_all(dir_agente);
}
