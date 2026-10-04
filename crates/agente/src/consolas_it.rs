//! Prueba de integración de «varias consolas a la vez» (docs/consolas-multiples.md)
//! con **dos Resguardo Server reales** (TLS) en el mismo proceso: la consola
//! local (A) y una «en línea» (B, con otra sal del cliente). El equipo se
//! conecta también a B con `anadir_consola`, obedece a las dos (cada una con su
//! `seq` y sus bloqueos), la configuración converge, y quitar cualquiera de las
//! dos deja la otra funcionando. Empieza con un vínculo del formato anterior.

use crate::servidor_v2::{self as s, cargar, guardar, ronda};
use crate::traslado_it::{arrancar_en, arrancar_sobre, enviar, estado, orden, Consola, Equipo, Srv, CLAVE};
use base64::Engine;
use resguardo_protocolo::{derivaciones, orden_v2, simetrico};
use serde_json::{json, Value};
use std::time::Duration;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

fn arrancar(nombre: &str) -> (Srv, String) {
    arrancar_en(std::env::temp_dir().join(format!("resguardo-consolas-{nombre}-{}", std::process::id())))
}

/// Vueltas de sondeo hasta que la orden `seq` de esa consola llegue a `fin` (las largas siguen aparte).
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

fn config_de(consola: &Consola, e: &Equipo, kcfg: &[u8; 32]) -> (Value, Value) {
    let (st, cf) = consola.pedir("GET", &format!("/api/clientes/{}/equipos/{}/config", e.c, e.id), None);
    assert_eq!(st, 200, "{cf}");
    let plano = simetrico::descifrar_config(kcfg, &e.id, cf["seq"].as_u64().unwrap(), &B64.decode(cf["cifrado"].as_str().unwrap()).unwrap()).unwrap();
    (serde_json::from_slice(&plano).unwrap(), cf["resumen"].clone())
}

/// El cuerpo de `anadir_consola` como lo arma la consola con un código de conexión.
fn anadir(destino: &Srv, ficha: &str, sal_destino: &str, sal_origen: &str) -> Value {
    json!({
        "url": destino.url, "identidad": destino.identidad, "huella_ca": s::huella_de_una_autoridad(&destino.ca).unwrap(),
        "ficha": ficha, "sal_cliente": sal_destino, "k_cfg": B64.encode(derivaciones::k_cfg(CLAVE, sal_destino).unwrap()),
        "nombre": "Consola en línea", "sal_origen": sal_origen,
    })
}

#[test]
fn dos_consolas_a_la_vez() {
    let _real = crate::restic::tests::real_repo_lock();
    let dir_agente = std::env::temp_dir().join(format!("resguardo-consolas-agente-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir_agente);
    std::env::set_var("RESGUARDO_AGENT_DIR", &dir_agente);

    let (a, codigo_a) = arrancar("a");
    let (b, codigo_b) = arrancar("b");
    let ca = Consola::entrar(&a, &codigo_a);
    let cb = Consola::entrar(&b, &codigo_b);

    // En A (la local): cliente, emparejamiento y alta.
    let (_, cliente) = ca.pedir("POST", "/api/clientes", Some(json!({ "nombre": "Café del Sur", "espera_min_horas": 1 })));
    let (c_a, sal_a) = (cliente["id"].as_str().unwrap().to_string(), cliente["sal_cliente"].as_str().unwrap().to_string());
    let (_, emp) = ca.pedir("POST", &format!("/api/clientes/{c_a}/emparejamientos"), None);
    let (codigo, emp_id) = (emp["codigo"].as_str().unwrap().to_string(), emp["id"].as_str().unwrap().to_string());
    s::vincular(&a.url, &codigo, "PRUEBA-CONSOLAS").unwrap();
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

    // 1. Un vínculo del formato anterior (sin `otras` ni `enlace_id`): sigue funcionando y es una lista de uno.
    let mut viejo = serde_json::to_value(cargar().unwrap()).unwrap();
    for k in ["enlace_id", "nombre_consola", "sal_cliente", "desde", "config_pendiente", "otras", "ultimo_cambio"] {
        viejo.as_object_mut().unwrap().remove(k);
    }
    let v: s::Vinculo = serde_json::from_value(viejo).unwrap();
    guardar(&v).unwrap();
    assert_eq!(cargar().unwrap().ids_enlaces(), vec![crate::consolas_v2::PRINCIPAL.to_string()]);
    enviar(&ca, &ea, &orden(&ea, 2, "cambiar_espera", json!({ "horas": 2 })));
    esperar_estado(&ca, &ea, 2, "hecha");

    // 2. En B (la en línea): «Recibir un cliente» con OTRA sal (la del código de conexión).
    let sal_b = B64.encode(uuid::Uuid::new_v4().as_bytes());
    let (st, rec) = cb.pedir("POST", "/api/clientes/recibir", Some(json!({ "nombre": "Café del Sur", "sal_cliente": sal_b, "usos": 10 })));
    assert_eq!(st, 200, "{rec}");
    let c_b = rec["cliente"]["id"].as_str().unwrap().to_string();
    let ficha_b = rec["ficha"].as_str().unwrap().to_string();
    let kcfg_b = derivaciones::k_cfg(CLAVE, &sal_b).unwrap();
    assert_ne!(kcfg_a, kcfg_b, "otra sal, otra K_cfg");
    let eb = Equipo { c: c_b.clone(), id: id.clone(), box_pub: box_pub.clone(), prueba };

    // Con la huella de otra autoridad: no se conecta (y el equipo sigue con A).
    let mut mala = anadir(&b, &ficha_b, &sal_b, &sal_a);
    mala["huella_ca"] = json!(s::huella_de_una_autoridad(&a.ca).unwrap());
    enviar(&ca, &ea, &orden(&ea, 3, "anadir_consola", mala));
    let o3 = esperar_estado(&ca, &ea, 3, "fallida");
    assert!(o3["mensaje"].as_str().unwrap().contains("autoridad TLS"), "{o3}");
    // Una identidad que no es la de B: tampoco.
    let mut mala = anadir(&b, &ficha_b, &sal_b, &sal_a);
    mala["identidad"] = json!(a.identidad);
    enviar(&ca, &ea, &orden(&ea, 4, "anadir_consola", mala));
    let o4 = esperar_estado(&ca, &ea, 4, "fallida");
    assert!(o4["mensaje"].as_str().unwrap().contains("ya gestiona"), "A ya es una de sus consolas: {o4}");
    let mut mala = anadir(&b, &ficha_b, &sal_b, &sal_a);
    mala["identidad"] = json!(B64.encode([77u8; 32]));
    enviar(&ca, &ea, &orden(&ea, 5, "anadir_consola", mala));
    let o5 = esperar_estado(&ca, &ea, 5, "fallida");
    assert!(o5["mensaje"].as_str().unwrap().contains("identidad"), "{o5}");
    assert!(cargar().unwrap().otras.is_empty());

    // 3. Conectar también a B, de verdad.
    enviar(&ca, &ea, &orden(&ea, 6, "anadir_consola", anadir(&b, &ficha_b, &sal_b, &sal_a)));
    let o6 = esperar_estado(&ca, &ea, 6, "hecha");
    assert!(o6["mensaje"].as_str().unwrap().contains("Consola en línea"), "{o6}");
    let v = cargar().unwrap();
    assert_eq!(v.url, a.url, "A sigue siendo la principal");
    assert_eq!(v.otras.len(), 1);
    assert_eq!((v.otras[0].url.as_str(), v.otras[0].cliente_id.as_str()), (b.url.as_str(), c_b.as_str()));
    assert_eq!(v.sal_cliente.as_deref(), Some(sal_a.as_str()), "aprendió la sal de la consola que la mandó");
    // En B: el mismo equipo, confirmado, con la etiqueta de SU K_cfg, y avisado.
    let (_, e_b) = cb.pedir("GET", &format!("/api/clientes/{c_b}/equipos/{id}"), None);
    assert_eq!(e_b["confirmado"], true);
    assert_eq!(e_b["etiqueta"], derivaciones::etiqueta_equipo(&kcfg_b, &id, &box_pub, &sign_pub).as_str());
    let (_, avisos_b) = cb.pedir("GET", &format!("/api/clientes/{c_b}/avisos?abiertos=1"), None);
    assert!(avisos_b.to_string().contains("también"), "{avisos_b}");
    // Y en A, el aviso de que se conectó a otra consola (con su huella).
    ronda().unwrap();
    let (_, avisos_a) = ca.pedir("GET", &format!("/api/clientes/{c_a}/avisos?abiertos=1"), None);
    assert!(avisos_a.to_string().contains("se conectó también a otra consola"), "{avisos_a}");
    // B recibe la configuración (cifrada con su K_cfg) con la lista de consolas, y el historial.
    ronda().unwrap();
    let (_, resumen_b) = config_de(&cb, &eb, &kcfg_b);
    let consolas = resumen_b["consolas"].as_array().unwrap();
    assert_eq!(consolas.len(), 2, "{resumen_b}");
    assert!(consolas.iter().any(|x| x["esta"] == true && x["url"] == b.url.as_str()));
    assert!(consolas.iter().any(|x| x["esta"] == false && x["url"] == a.url.as_str() && x["sal_cliente"] == sal_a.as_str()));
    assert!(resumen_b["admite"].to_string().contains("consolas_multiples"));
    let (_, hist_b) = cb.pedir("GET", &format!("/api/clientes/{c_b}/equipos/{id}/historial?limite=50"), None);
    assert!(!hist_b.as_array().unwrap().is_empty(), "el historial del equipo llega a la consola nueva");
    // La espera que puso A (2 h) también la sabe B (por la configuración).
    assert_eq!(e_b["espera_min_horas"], 2, "{e_b}");

    // 4. Órdenes de las dos, cada una con su seq.
    enviar(&cb, &eb, &orden(&eb, 1, "cambiar_espera", json!({ "horas": 6 })));
    esperar_estado(&cb, &eb, 1, "hecha");
    enviar(&ca, &ea, &orden(&ea, 7, "cambiar_espera", json!({ "horas": 8 })));
    esperar_estado(&ca, &ea, 7, "hecha");
    assert_eq!(cargar().unwrap().espera_min_horas, 8);
    let v = cargar().unwrap();
    assert_eq!((v.ultimo_seq, v.otras[0].ultimo_seq), (7, 1), "seq de cada consola");
    // Un sobre de A presentado por B (otro cliente): rechazado aunque el seq cupiera.
    let ajena = orden(&ea, 2, "cambiar_espera", json!({ "horas": 10 }));
    let sobre = orden_v2::sellar(&ajena, &box_pub).unwrap();
    let (st, _) = cb.pedir(
        "POST",
        &format!("/api/clientes/{c_b}/equipos/{id}/ordenes"),
        Some(json!({ "tipo": "cambiar_espera", "seq": 2, "sellado": sobre, "caduca": ajena.caduca })),
    );
    assert_eq!(st, 200);
    esperar_estado(&cb, &eb, 2, "rechazada");
    assert_eq!(cargar().unwrap().espera_min_horas, 8);

    // 5. La configuración converge: lo que cambia A llega a B, con «desde otra consola».
    let cfg = json!({ "config": { "v": 1, "copias": [], "verificacion": { "marca": "desde A" } } });
    enviar(&ca, &ea, &orden(&ea, 8, "config", cfg));
    esperar_estado(&ca, &ea, 8, "hecha");
    ronda().unwrap();
    let (doc_b, resumen_b) = config_de(&cb, &eb, &kcfg_b);
    assert_eq!(doc_b["verificacion"]["marca"], "desde A", "{doc_b}");
    assert_eq!(resumen_b["cambio_config"]["tipo"], "config");
    assert_eq!(resumen_b["cambio_config"]["consola"]["identidad"], a.identidad.as_str(), "{resumen_b}");
    // Y al revés.
    let cfg = json!({ "config": { "v": 1, "copias": [], "verificacion": { "marca": "desde B" } } });
    enviar(&cb, &eb, &orden(&eb, 3, "config", cfg));
    esperar_estado(&cb, &eb, 3, "hecha");
    ronda().unwrap();
    let (doc_a, resumen_a) = config_de(&ca, &ea, &kcfg_a);
    assert_eq!(doc_a["verificacion"]["marca"], "desde B");
    assert_eq!(resumen_a["cambio_config"]["consola"]["identidad"], b.identidad.as_str());

    // 6. Bloqueos por consola: pruebas falsas por B bloquean B, no A.
    let falsa = derivaciones::prueba_admin("otra clave cualquiera", eq["sal_equipo"].as_str().unwrap()).unwrap();
    let eb_mala = Equipo { c: c_b.clone(), id: id.clone(), box_pub: box_pub.clone(), prueba: falsa };
    for seq in 4..=8 {
        enviar(&cb, &eb_mala, &orden(&eb_mala, seq, "cambiar_espera", json!({ "horas": 12 })));
        esperar_estado(&cb, &eb, seq, "rechazada");
    }
    enviar(&cb, &eb, &orden(&eb, 9, "cambiar_espera", json!({ "horas": 12 })));
    let o = esperar_estado(&cb, &eb, 9, "rechazada");
    assert!(o["mensaje"].as_str().unwrap().contains("Bloqueado"), "{o}");
    enviar(&ca, &ea, &orden(&ea, 9, "cambiar_espera", json!({ "horas": 12 })));
    esperar_estado(&ca, &ea, 9, "hecha");
    let mut v = cargar().unwrap();
    v.otras[0].fallos.clear();
    guardar(&v).unwrap();

    // 7. Por los dos canales a la vez (un WebSocket por consola): las órdenes llegan al momento.
    let ids = cargar().unwrap().ids_enlaces();
    let mut hilos: Vec<_> = ids.iter().cloned().map(|i| std::thread::spawn(move || s::canal_de(&i))).collect();
    for (consola, e) in [(&ca, &ea), (&cb, &eb)] {
        let mut conectado = false;
        for _ in 0..50 {
            if consola.pedir("GET", &format!("/api/clientes/{}/equipos/{}", e.c, e.id), None).1["conectado"] == true {
                conectado = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(conectado, "el canal con {} no se abrió", e.c);
    }
    let llega = |consola: &Consola, e: &Equipo, seq: u64, fin: &str| {
        for _ in 0..80 {
            if estado(consola, e, seq)["estado"] == fin {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        panic!("la orden {seq} no llegó por el canal: {}", estado(consola, e, seq));
    };
    enviar(&cb, &eb, &orden(&eb, 10, "cambiar_espera", json!({ "horas": 16 })));
    llega(&cb, &eb, 10, "hecha");
    enviar(&ca, &ea, &orden(&ea, 10, "cambiar_espera", json!({ "horas": 20 })));
    llega(&ca, &ea, 10, "hecha");

    // 8. Quitar B desde A: el canal de B se cierra; A sigue.
    let ident_b = b.identidad.clone();
    enviar(&ca, &ea, &orden(&ea, 11, "quitar_consola", json!({ "identidad": ident_b })));
    llega(&ca, &ea, 11, "hecha");
    // El canal de B se cierra al quitarla (el de A sigue abierto).
    let canal_a = hilos.remove(0);
    assert!(hilos.pop().unwrap().join().unwrap().is_ok());
    assert!(cargar().unwrap().otras.is_empty());
    let mut avisado = false;
    for _ in 0..50 {
        let (_, avisos_b) = cb.pedir("GET", &format!("/api/clientes/{c_b}/avisos?abiertos=1"), None);
        if avisos_b.to_string().contains("dejó de conectarse") {
            avisado = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(avisado, "B recibe un último aviso");
    enviar(&ca, &ea, &orden(&ea, 12, "cambiar_espera", json!({ "horas": 24 })));
    llega(&ca, &ea, 12, "hecha");

    // 9. Otra vez B (misma ficha: B lo reconoce por sus llaves), y ahora se va B por su lado:
    // «Dejar de gestionar este equipo» (desvincular, seguir en local) solo quita B.
    enviar(&ca, &ea, &orden(&ea, 13, "anadir_consola", anadir(&b, &ficha_b, &sal_b, &sal_a)));
    llega(&ca, &ea, 13, "hecha");
    let siguiente_b = cb.pedir("GET", &format!("/api/clientes/{c_b}/equipos/{id}"), None).1["siguiente_seq"].as_u64().unwrap();
    enviar(&cb, &eb, &orden(&eb, siguiente_b, "desvincular", json!({ "modo": "seguir_local" })));
    let o = esperar_estado(&cb, &eb, siguiente_b, "hecha");
    assert!(o["mensaje"].as_str().unwrap().contains("siguen gestionando"), "{o}");
    let (_, e_b) = cb.pedir("GET", &format!("/api/clientes/{c_b}/equipos/{id}"), None);
    assert_eq!(e_b["modo"], "local");
    let v = cargar().unwrap();
    assert_eq!((v.modo.as_str(), v.url.as_str(), v.otras.len()), ("gestionado", a.url.as_str(), 0));
    enviar(&ca, &ea, &orden(&ea, 14, "cambiar_espera", json!({ "horas": 30 })));
    llega(&ca, &ea, 14, "hecha");

    // 10. Y al revés: B otra vez, y desde B se quita A (la principal). B pasa a ser la principal.
    enviar(&ca, &ea, &orden(&ea, 15, "anadir_consola", anadir(&b, &ficha_b, &sal_b, &sal_a)));
    llega(&ca, &ea, 15, "hecha");
    ronda().unwrap();
    let siguiente_b = cb.pedir("GET", &format!("/api/clientes/{c_b}/equipos/{id}"), None).1["siguiente_seq"].as_u64().unwrap();
    let ident_a = a.identidad.clone();
    enviar(&cb, &eb, &orden(&eb, siguiente_b, "quitar_consola", json!({ "identidad": ident_a })));
    esperar_estado(&cb, &eb, siguiente_b, "hecha");
    // El canal de A se cierra solo (su consola ya no está).
    assert!(canal_a.join().unwrap().is_ok());
    let v = cargar().unwrap();
    assert_eq!((v.url.as_str(), v.cliente_id.as_str(), v.otras.len()), (b.url.as_str(), c_b.as_str(), 0), "B es ahora la principal");
    enviar(&cb, &eb, &orden(&eb, siguiente_b + 1, "cambiar_espera", json!({ "horas": 36 })));
    esperar_estado(&cb, &eb, siguiente_b + 1, "hecha");
    assert_eq!(cargar().unwrap().espera_min_horas, 36);
    // A ya no recibe nada de él: sus órdenes se quedan sin entregar.
    enviar(&ca, &ea, &orden(&ea, 16, "cambiar_espera", json!({ "horas": 40 })));
    ronda().unwrap();
    assert_eq!(estado(&ca, &ea, 16)["estado"], "pendiente");

    // 11. En el equipo: `consolas quitar` de una que ya no existe (vuelve A y se quita a mano).
    let (_, fa) = ca.pedir("POST", &format!("/api/clientes/{c_a}/fichas"), Some(json!({ "usos": 1, "dias": 1 })));
    enviar(&cb, &eb, &orden(&eb, siguiente_b + 2, "anadir_consola", anadir(&a, fa["ficha"].as_str().unwrap(), &sal_a, &sal_b)));
    esperar_estado(&cb, &eb, siguiente_b + 2, "hecha");
    let lista = crate::consolas_v2::listar(&cargar().unwrap());
    assert_eq!(lista.len(), 2, "{lista:?}");
    assert!(crate::consolas_v2::quitar_local("9").is_err());
    let quitada = crate::consolas_v2::quitar_local(&a.url).unwrap();
    assert!(quitada.contains(&a.url), "{quitada}");
    assert!(crate::consolas_v2::quitar_local("1").unwrap_err().contains("única"));
    let v = cargar().unwrap();
    assert_eq!((v.url.as_str(), v.otras.len()), (b.url.as_str(), 0));

    // 12. Mover todo de B a A: conectar también a A y, ya allí, «Dejar esta consola» desde B
    // (quitar_consola de sí misma): B lo marca como que ya no está aquí y A queda sola.
    let (_, fa) = ca.pedir("POST", &format!("/api/clientes/{c_a}/fichas"), Some(json!({ "usos": 1, "dias": 1 })));
    enviar(&cb, &eb, &orden(&eb, siguiente_b + 3, "anadir_consola", anadir(&a, fa["ficha"].as_str().unwrap(), &sal_a, &sal_b)));
    esperar_estado(&cb, &eb, siguiente_b + 3, "hecha");
    ronda().unwrap();
    let ident_b = b.identidad.clone();
    enviar(&cb, &eb, &orden(&eb, siguiente_b + 4, "quitar_consola", json!({ "identidad": ident_b })));
    let o = esperar_estado(&cb, &eb, siguiente_b + 4, "hecha");
    assert!(o["detalle"].as_str().unwrap_or("").contains("deja_esta_consola"), "{o}");
    let (_, e_b) = cb.pedir("GET", &format!("/api/clientes/{c_b}/equipos/{id}"), None);
    assert_eq!(e_b["modo"], "local", "B ya no lo tiene: {e_b}");
    let v = cargar().unwrap();
    assert_eq!((v.url.as_str(), v.cliente_id.as_str(), v.otras.len(), v.modo.as_str()), (a.url.as_str(), c_a.as_str(), 0, "gestionado"));
    // A manda (sus órdenes pendientes llegan ahora, con su seq) y sigue funcionando.
    let siguiente_a = ca.pedir("GET", &format!("/api/clientes/{c_a}/equipos/{id}"), None).1["siguiente_seq"].as_u64().unwrap();
    enviar(&ca, &ea, &orden(&ea, siguiente_a, "cambiar_espera", json!({ "horas": 50 })));
    esperar_estado(&ca, &ea, siguiente_a, "hecha");

    std::env::remove_var("RESGUARDO_AGENT_DIR");
    let _ = std::fs::remove_dir_all(dir_agente);
    let _ = std::fs::remove_dir_all(a.dir.clone());
    let _ = std::fs::remove_dir_all(b.dir.clone());
}

fn copiar_carpeta(de: &std::path::Path, a: &std::path::Path) {
    std::fs::create_dir_all(a).unwrap();
    for e in std::fs::read_dir(de).unwrap().flatten() {
        if e.file_type().unwrap().is_dir() {
            copiar_carpeta(&e.path(), &a.join(e.file_name()));
        } else {
            std::fs::copy(e.path(), a.join(e.file_name())).unwrap();
        }
    }
}

fn siguiente_seq(consola: &Consola, e: &Equipo) -> u64 {
    consola.pedir("GET", &format!("/api/clientes/{}/equipos/{}", e.c, e.id), None).1["siguiente_seq"].as_u64().unwrap()
}

/// Con dos consolas, una vuelve de una copia suya de antes (restaurar la copia de la
/// consola): recuerda un número de orden anterior al último que el equipo le aceptó.
/// El equipo le dice el suyo (el de ESA consola) al abrir el canal, antes de contar
/// como conectado: la primera orden que se firma al verlo conectado se acepta. La
/// otra consola sigue con su número. (Lo encontró la prueba de extremo a extremo.)
#[test]
fn una_consola_restaurada_no_rechaza_sus_ordenes() {
    let _real = crate::restic::tests::real_repo_lock();
    let dir_agente = std::env::temp_dir().join(format!("resguardo-consolas-rest-agente-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir_agente);
    std::env::set_var("RESGUARDO_AGENT_DIR", &dir_agente);

    let (a, codigo_a) = arrancar("rest-a");
    let (b, codigo_b) = arrancar("rest-b");
    let ca = Consola::entrar(&a, &codigo_a);
    let cb = Consola::entrar(&b, &codigo_b);
    // A: cliente, emparejamiento y alta.
    let (_, cliente) = ca.pedir("POST", "/api/clientes", Some(json!({ "nombre": "Café del Sur", "espera_min_horas": 1 })));
    let (c_a, sal_a) = (cliente["id"].as_str().unwrap().to_string(), cliente["sal_cliente"].as_str().unwrap().to_string());
    let (_, emp) = ca.pedir("POST", &format!("/api/clientes/{c_a}/emparejamientos"), None);
    let (codigo, emp_id) = (emp["codigo"].as_str().unwrap().to_string(), emp["id"].as_str().unwrap().to_string());
    s::vincular(&a.url, &codigo, "PRUEBA-RESTAURADA").unwrap();
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
    // Y también B.
    let sal_b = B64.encode(uuid::Uuid::new_v4().as_bytes());
    let (_, rec) = cb.pedir("POST", "/api/clientes/recibir", Some(json!({ "nombre": "Café del Sur", "sal_cliente": sal_b, "usos": 10 })));
    let c_b = rec["cliente"]["id"].as_str().unwrap().to_string();
    let eb = Equipo { c: c_b, id: id.clone(), box_pub: box_pub.clone(), prueba };
    enviar(&ca, &ea, &orden(&ea, 2, "anadir_consola", anadir(&b, rec["ficha"].as_str().unwrap(), &sal_b, &sal_a)));
    esperar_estado(&ca, &ea, 2, "hecha");
    enviar(&cb, &eb, &orden(&eb, 1, "cambiar_espera", json!({ "horas": 3 })));
    esperar_estado(&cb, &eb, 1, "hecha");
    enviar(&ca, &ea, &orden(&ea, 3, "cambiar_espera", json!({ "horas": 4 })));
    esperar_estado(&ca, &ea, 3, "hecha");

    // La copia de A (con el servidor parado, como el archivo de la copia de la consola).
    let url_a = a.url.clone();
    let dir_a = a.matar();
    let copia = dir_a.with_file_name(format!("{}-copia", dir_a.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&copia);
    copiar_carpeta(&dir_a, &copia);
    let a = arrancar_sobre(dir_a, &url_a);
    // Después de la copia, una orden más de A: el equipo va por delante de lo que recuerda la copia.
    enviar(&ca, &ea, &orden(&ea, 4, "cambiar_espera", json!({ "horas": 5 })));
    esperar_estado(&ca, &ea, 4, "hecha");
    let v = cargar().unwrap();
    assert_eq!((v.ultimo_seq, v.otras[0].ultimo_seq), (4, 1), "seq de cada consola");
    let (id_a, id_b) = (v.id_enlace(), v.otras[0].id.clone());

    // A vuelve desde la copia (en otra carpeta), en la misma dirección: recuerda el 4 como siguiente.
    let dir_a = a.matar();
    let restaurada = |n: u32| {
        let d = copia.with_file_name(format!("{}-{n}", copia.file_name().unwrap().to_string_lossy()));
        let _ = std::fs::remove_dir_all(&d);
        copiar_carpeta(&copia, &d);
        let a = arrancar_sobre(d, &url_a);
        assert_eq!(siguiente_seq(&ca, &ea), 4, "la copia es de antes de la orden 4");
        a
    };
    // Por el sondeo: con el primero ya está al día (el último de A; B no se toca).
    let a = restaurada(1);
    s::ronda_enlace(&id_a).unwrap();
    assert_eq!(siguiente_seq(&ca, &ea), 5);
    assert_eq!(siguiente_seq(&cb, &eb), 2);
    enviar(&ca, &ea, &orden(&ea, 5, "cambiar_espera", json!({ "horas": 6 })));
    esperar_estado(&ca, &ea, 5, "hecha");
    // Por el canal (como en la prueba de extremo a extremo): otra vez desde la copia.
    let dir_r1 = a.matar();
    let a = restaurada(2);
    let canal = std::thread::spawn(move || s::canal_de(&id_a));
    let mut conectado = false;
    for _ in 0..100 {
        let (_, e) = ca.pedir("GET", &format!("/api/clientes/{c_a}/equipos/{id}"), None);
        if e["conectado"] == true {
            // En cuanto cuenta como conectado, el número ya está al día (sin esperar al informe).
            assert_eq!(e["siguiente_seq"], 6, "{e}");
            conectado = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(conectado, "el canal con A no se abrió");
    enviar(&ca, &ea, &orden(&ea, 6, "cambiar_espera", json!({ "horas": 6 })));
    let mut hecha = false;
    for _ in 0..80 {
        if estado(&ca, &ea, 6)["estado"] == "hecha" {
            hecha = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(hecha, "la orden de A restaurada, por el canal: {}", estado(&ca, &ea, 6));
    // B, con su número de siempre.
    assert_eq!(siguiente_seq(&cb, &eb), 2);
    enviar(&cb, &eb, &orden(&eb, 2, "cambiar_espera", json!({ "horas": 7 })));
    for _ in 0..40 {
        let _ = s::ronda_enlace(&id_b);
        if estado(&cb, &eb, 2)["estado"] == "hecha" {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    assert_eq!(estado(&cb, &eb, 2)["estado"], "hecha", "{}", estado(&cb, &eb, 2));
    let v = cargar().unwrap();
    assert_eq!((v.ultimo_seq, v.otras[0].ultimo_seq, v.espera_min_horas), (6, 2, 7));

    let dir_r2 = a.matar();
    assert!(canal.join().unwrap().is_err(), "el canal se corta al parar A");
    std::env::remove_var("RESGUARDO_AGENT_DIR");
    for d in [dir_agente, dir_a, copia, dir_r1, dir_r2, b.dir.clone()] {
        let _ = std::fs::remove_dir_all(d);
    }
}
