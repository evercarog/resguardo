//! Prueba de integración de las órdenes en espera (v1.4x, docs/consolas-multiples.md §5)
//! con **dos Resguardo Server reales** (TLS) en el mismo proceso: la consola local
//! (A) y una «en línea» (B). El equipo recibe al momento una orden destructiva de A
//! (sin aplicarla), B la ve en el resumen, recibe el aviso, la cancela y nunca se
//! aplica; otra se aplica a su hora; A cancela una suya; las inofensivas con espera,
//! una con la clave mal y las que llegan a un agente que no lo admite, como siempre.

use crate::servidor_v2::{self as s, cargar, ronda};
use crate::traslado_it::{arrancar_en, enviar, estado, orden, Consola, Equipo, Srv, CLAVE};
use base64::Engine;
use resguardo_protocolo::{derivaciones, orden_v2, simetrico};
use serde_json::{json, Value};
use std::time::Duration;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

fn arrancar(nombre: &str) -> (Srv, String) {
    arrancar_en(std::env::temp_dir().join(format!("resguardo-espera-{nombre}-{}", std::process::id())))
}

/// Vueltas de sondeo hasta que la orden `seq` de esa consola llegue a `fin`.
fn esperar_estado(consola: &Consola, e: &Equipo, seq: u64, fin: &str) -> Value {
    for _ in 0..80 {
        let _ = ronda();
        let o = estado(consola, e, seq);
        if o["estado"] == fin {
            return o;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    panic!("la orden {seq} no llegó a «{fin}»: {}", estado(consola, e, seq));
}

/// Hasta que `f` diga que sí (con vueltas de sondeo entre medias).
fn hasta<T>(que: &str, mut f: impl FnMut() -> Option<T>) -> T {
    for _ in 0..80 {
        let _ = ronda();
        if let Some(x) = f() {
            return x;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    panic!("no llegó: {que}");
}

/// El resumen que tiene una consola del equipo (lo sube el equipo con su configuración).
fn resumen_de(consola: &Consola, e: &Equipo, kcfg: &[u8; 32]) -> Value {
    let (st, cf) = consola.pedir("GET", &format!("/api/clientes/{}/equipos/{}/config", e.c, e.id), None);
    assert_eq!(st, 200, "{cf}");
    let _ = simetrico::descifrar_config(kcfg, &e.id, cf["seq"].as_u64().unwrap(), &B64.decode(cf["cifrado"].as_str().unwrap()).unwrap()).unwrap();
    cf["resumen"].clone()
}

/// Una orden destructiva de verdad, con su espera hasta `en` segundos desde ahora.
fn con_espera(e: &Equipo, seq: u64, tipo: &str, cuerpo: Value, en: i64) -> orden_v2::OrdenV2 {
    let mut o = orden(e, seq, tipo, cuerpo);
    o.not_before = Some((chrono::Local::now() + chrono::Duration::seconds(en)).to_rfc3339());
    o.por = Some("Ana".into());
    o
}

/// Las órdenes de esa consola en el historial común del equipo.
fn historial_ordenes(consola: &Consola, e: &Equipo) -> Vec<Value> {
    let (st, h) = consola.pedir("GET", &format!("/api/clientes/{}/equipos/{}/historial?tipo=orden&limite=200", e.c, e.id), None);
    assert_eq!(st, 200, "{h}");
    h.as_array().cloned().unwrap_or_default()
}

/// Sin la espera de horas (solo en compilaciones de desarrollo), y la deja como estaba al terminar.
struct SinEspera;
impl SinEspera {
    fn poner() -> Self {
        std::env::set_var(resguardo_protocolo::ordenes::PRUEBA_SIN_ESPERA, "1");
        SinEspera
    }
}
impl Drop for SinEspera {
    fn drop(&mut self) {
        std::env::remove_var(resguardo_protocolo::ordenes::PRUEBA_SIN_ESPERA);
    }
}

#[test]
fn ordenes_en_espera_entre_dos_consolas() {
    let _real = crate::restic::tests::real_repo_lock();
    let _sin_espera = SinEspera::poner();
    let dir_agente = std::env::temp_dir().join(format!("resguardo-espera-agente-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir_agente);
    std::env::set_var("RESGUARDO_AGENT_DIR", &dir_agente);

    let (a, codigo_a) = arrancar("a");
    let (b, codigo_b) = arrancar("b");
    let ca = Consola::entrar(&a, &codigo_a);
    let cb = Consola::entrar(&b, &codigo_b);

    // En A: cliente, emparejamiento y alta (espera mínima 1 h, que aquí es 0 s).
    let (_, cliente) = ca.pedir("POST", "/api/clientes", Some(json!({ "nombre": "Panadería La Espiga", "espera_min_horas": 1 })));
    let (c_a, sal_a) = (cliente["id"].as_str().unwrap().to_string(), cliente["sal_cliente"].as_str().unwrap().to_string());
    let (_, emp) = ca.pedir("POST", &format!("/api/clientes/{c_a}/emparejamientos"), None);
    let (codigo, emp_id) = (emp["codigo"].as_str().unwrap().to_string(), emp["id"].as_str().unwrap().to_string());
    s::vincular(&a.url, &codigo, "PRUEBA-ESPERA").unwrap();
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

    // Conectar también a B (otra sal), como en consolas_it.
    let sal_b = B64.encode(uuid::Uuid::new_v4().as_bytes());
    let (st, rec) = cb.pedir("POST", "/api/clientes/recibir", Some(json!({ "nombre": "Panadería La Espiga", "sal_cliente": sal_b, "usos": 10 })));
    assert_eq!(st, 200, "{rec}");
    let c_b = rec["cliente"]["id"].as_str().unwrap().to_string();
    let kcfg_b = derivaciones::k_cfg(CLAVE, &sal_b).unwrap();
    let eb = Equipo { c: c_b.clone(), id: id.clone(), box_pub: box_pub.clone(), prueba };
    let anadir = json!({
        "url": b.url, "identidad": b.identidad, "huella_ca": s::huella_de_una_autoridad(&b.ca).unwrap(),
        "ficha": rec["ficha"], "sal_cliente": sal_b, "k_cfg": B64.encode(kcfg_b), "nombre": "Consola en línea", "sal_origen": sal_a,
    });
    enviar(&ca, &ea, &orden(&ea, 2, "anadir_consola", anadir));
    esperar_estado(&ca, &ea, 2, "hecha");
    // A tiene nombre en el equipo (como lo pondría su código de conexión).
    crate::consolas_v2::modificar(|v| v.nombre_consola = "Oficina".into());
    let ids = cargar().unwrap().ids_enlaces();
    let (id_a, id_b) = (ids[0].clone(), ids[1].clone());
    // El resumen (con `admite: ordenes_en_espera`) en las dos consolas.
    for i in [&id_a, &id_b] {
        s::enviar_informe(i).unwrap();
    }
    let (_, e_a) = ca.pedir("GET", &format!("/api/clientes/{c_a}/equipos/{id}"), None);
    assert!(e_a["resumen"]["admite"].to_string().contains("ordenes_en_espera"), "{e_a}");
    // Subir la espera (no reduce la protección: al momento) para luego acortarla con espera.
    enviar(&ca, &ea, &orden(&ea, 3, "cambiar_espera", json!({ "horas": 3 })));
    esperar_estado(&ca, &ea, 3, "hecha");

    // 1. A manda algo destructivo con espera: el equipo la recibe ya y la guarda, sin aplicarla.
    enviar(&ca, &ea, &con_espera(&ea, 4, "cambiar_espera", json!({ "horas": 2 }), 3600));
    ronda().unwrap();
    assert_eq!(estado(&ca, &ea, 4)["estado"], "entregada", "entregada antes de su hora");
    let v = cargar().unwrap();
    assert_eq!(v.en_espera.len(), 1);
    assert_eq!(v.espera_min_horas, 3, "no se aplicó");
    let guardada = serde_json::to_string(&v.en_espera[0]).unwrap();
    assert!(!guardada.contains(&B64.encode(prueba)), "la prueba de la clave no se guarda en claro");
    let orden_4 = v.en_espera[0].id.clone();
    // Sigue «esperando su turno» en A, aunque ya esté entregada.
    let (_, pend) = ca.pedir("GET", &format!("/api/clientes/{c_a}/ordenes?pendientes=1"), None);
    assert!(pend.as_array().unwrap().iter().any(|o| o["id"] == orden_4.as_str()), "{pend}");

    // 2. B la ve en el resumen (desde qué consola, sin su dirección), recibe el aviso y la ve en el historial.
    s::enviar_informe(&id_b).unwrap();
    let r_b = resumen_de(&cb, &eb, &kcfg_b);
    let l = r_b["en_espera"].as_array().unwrap();
    assert_eq!(l.len(), 1, "{r_b}");
    assert_eq!(l[0]["id"], orden_4.as_str());
    assert_eq!(l[0]["consola"]["nombre"], "Oficina");
    assert_eq!(l[0]["consola"]["identidad"], a.identidad.as_str());
    assert_eq!(l[0]["consola"]["esta"], false);
    assert_eq!(l[0]["por"], "Ana");
    assert_eq!(l[0]["descripcion"], "Acortar la espera a 2 h");
    assert!(!r_b["en_espera"].to_string().contains(&a.url), "sin la dirección de la otra consola");
    hasta("el aviso «orden_en_espera» en B", || {
        let (_, av) = cb.pedir("GET", &format!("/api/clientes/{c_b}/avisos?abiertos=1"), None);
        av.as_array().unwrap().iter().any(|x| x["tipo"] == "orden_en_espera" && x["mensaje"].as_str().unwrap_or("").contains("Oficina")).then_some(())
    });
    hasta("la orden en espera en el historial de B", || {
        historial_ordenes(&cb, &eb).into_iter().find(|x| x["orden_id"] == orden_4.as_str() && x["resultado"] == "en_espera")
    });
    // A no recibe ese aviso (ya tiene su «Orden destructiva pendiente»).
    let (_, av_a) = ca.pedir("GET", &format!("/api/clientes/{c_a}/avisos?abiertos=1"), None);
    assert!(!av_a.to_string().contains("orden_en_espera"), "{av_a}");

    // 3. B la cancela (inofensiva, sin clave) y A se entera: nunca se aplica.
    let mut cancelar = orden(&eb, 1, "cancelar_espera", json!({ "id": orden_4 }));
    cancelar.autorizacion = Default::default();
    cancelar.por = Some("Bruno".into());
    enviar(&cb, &eb, &cancelar);
    let hecha = esperar_estado(&cb, &eb, 1, "hecha");
    assert!(hecha["mensaje"].as_str().unwrap().contains("Oficina"), "{hecha}");
    assert!(cargar().unwrap().en_espera.is_empty());
    s::largas::reintentar();
    let o4 = estado(&ca, &ea, 4);
    // `rechazada` (lo que firmó el equipo) con `detalle.cancelada`.
    assert_eq!(o4["estado"], "rechazada", "{o4}");
    assert!(o4["detalle"].as_str().unwrap_or("").contains("\"cancelada\":true"), "{o4}");
    assert!(o4["mensaje"].as_str().unwrap().contains("Consola en línea") || o4["mensaje"].as_str().unwrap().contains("otra consola"), "{o4}");
    // Cancelar otra vez: ya no está.
    let mut otra_vez = orden(&eb, 2, "cancelar_espera", json!({ "id": orden_4 }));
    otra_vez.autorizacion = Default::default();
    enviar(&cb, &eb, &otra_vez);
    esperar_estado(&cb, &eb, 2, "fallida");
    let h = hasta("la cancelación en el historial de A", || {
        historial_ordenes(&ca, &ea).into_iter().find(|x| x["orden_id"] == orden_4.as_str() && x["resultado"] == "cancelada")
    });
    assert_eq!(h["identidad"], a.identidad.as_str(), "{h}");
    assert!(h["mensaje"].as_str().unwrap().contains("Bruno"), "quién la canceló: {h}");

    // 4. Otra se aplica a su hora (la cuentan el reloj del equipo y el de A) y A recibe el resultado firmado.
    enviar(&ca, &ea, &con_espera(&ea, 5, "cambiar_espera", json!({ "horas": 2 }), 4));
    ronda().unwrap();
    assert_eq!(cargar().unwrap().en_espera.len(), 1);
    assert_eq!(cargar().unwrap().espera_min_horas, 3, "todavía no");
    std::thread::sleep(Duration::from_secs(5));
    let o5 = esperar_estado(&ca, &ea, 5, "hecha");
    assert!(o5["firma_agente"].is_string(), "{o5}");
    assert_eq!(cargar().unwrap().espera_min_horas, 2);
    assert!(cargar().unwrap().en_espera.is_empty());
    hasta("la aplicada en el historial de B", || {
        historial_ordenes(&cb, &eb).into_iter().find(|x| x["orden"] == "cambiar_espera" && x["resultado"] == "hecha" && x["identidad"] == a.identidad.as_str())
    });

    // 5. A cancela una suya como siempre: su servidor se lo dice al equipo, que la quita.
    enviar(&ca, &ea, &con_espera(&ea, 6, "pausar", json!({}), 3600));
    ronda().unwrap();
    assert_eq!(cargar().unwrap().en_espera.len(), 1);
    let id6 = cargar().unwrap().en_espera[0].id.clone();
    let (st, c6) = ca.pedir("POST", &format!("/api/clientes/{c_a}/ordenes/{id6}/cancelar"), None);
    assert_eq!(st, 200, "{c6}");
    hasta("que el equipo la quite", || cargar().unwrap().en_espera.is_empty().then_some(()));

    // 6. Una inofensiva con espera: a su hora, como siempre (el servidor no la adelanta).
    let mut inofensiva = orden(&ea, 7, "copiar_ahora", json!({}));
    inofensiva.not_before = Some((chrono::Local::now() + chrono::Duration::hours(1)).to_rfc3339());
    inofensiva.autorizacion = Default::default();
    enviar(&ca, &ea, &inofensiva);
    ronda().unwrap();
    assert_eq!(estado(&ca, &ea, 7)["estado"], "pendiente");

    // 7. Con la clave mal: rechazada al recibirla (no se guarda ni avisa a nadie).
    let falsa = derivaciones::prueba_admin("otra clave cualquiera", eq["sal_equipo"].as_str().unwrap()).unwrap();
    let ea_mala = Equipo { c: c_a.clone(), id: id.clone(), box_pub: box_pub.clone(), prueba: falsa };
    enviar(&ca, &ea_mala, &con_espera(&ea_mala, 8, "pausar", json!({}), 3600));
    esperar_estado(&ca, &ea, 8, "rechazada");
    assert!(cargar().unwrap().en_espera.is_empty());

    // 8. Quitar la consola que la mandó: sus órdenes en espera no se aplicarán.
    enviar(&ca, &ea, &con_espera(&ea, 9, "pausar", json!({}), 3600));
    ronda().unwrap();
    assert_eq!(cargar().unwrap().en_espera.len(), 1);
    let mut quitar = orden(&eb, 3, "quitar_consola", json!({ "identidad": a.identidad }));
    quitar.autorizacion = orden_v2::Autorizacion { prueba_admin: Some(B64.encode(prueba)), ..Default::default() };
    enviar(&cb, &eb, &quitar);
    esperar_estado(&cb, &eb, 3, "hecha");
    s::enviar_informe(&id_b).unwrap();
    let r_b = resumen_de(&cb, &eb, &kcfg_b);
    assert_eq!(r_b["en_espera"].as_array().map(Vec::len), Some(0), "{r_b}");

    std::env::remove_var("RESGUARDO_AGENT_DIR");
    let _ = std::fs::remove_dir_all(&dir_agente);
}
