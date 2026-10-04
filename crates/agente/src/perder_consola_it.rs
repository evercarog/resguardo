//! «Si pierdes la consola» (docs/plataforma.md, §3.5) de punta a punta, con
//! Resguardo Server reales (TLS) en el mismo proceso y un servidor que se
//! **apaga de verdad** (su puerto deja de responder):
//!
//! - **Servidores de respaldo:** el principal muere; pasados los días fijados
//!   (3 por defecto) el equipo pasa solo al de respaldo, que ve sus copias.
//! - **Volver a vincular con un código:** sin respaldo, un servidor nuevo con
//!   el cliente creado otra vez (otra sal) y la misma clave de administración
//!   se queda con el equipo sin reconfigurar nada; con otra clave, no (salvo
//!   «empezar de cero»), y un código solo no le da nada.
//! - **Sin servidor, sigue copiando:** el programador no depende del servidor.
//!
//! Los datos de la prueba van en `<temp>/prueba-traslado/`.

use crate::servidor_v2::{cargar, empezar_de_cero, guardar, ronda, vincular, Vinculado};
use crate::traslado_it::{arrancar_en, enviar, estado, orden, Consola, Equipo, Srv, CLAVE};
use base64::Engine;
use resguardo_protocolo::orden_v2;
use resguardo_protocolo::{derivaciones, simetrico};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
const OTRA: &str = "otra clave de otro cliente distinto";

fn base(nombre: &str) -> PathBuf {
    let b = std::env::temp_dir().join("prueba-traslado").join(format!("{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&b);
    std::fs::create_dir_all(&b).unwrap();
    b
}

fn cliente(consola: &Consola, nombre: &str) -> (String, String) {
    let (st, c) = consola.pedir("POST", "/api/clientes", Some(json!({ "nombre": nombre, "espera_min_horas": 1 })));
    assert_eq!(st, 200, "{c}");
    (c["id"].as_str().unwrap().to_string(), c["sal_cliente"].as_str().unwrap().to_string())
}

struct Alta {
    e: Equipo,
    kcfg: [u8; 32],
    vinc: Vinculado,
}

/// Lo que hace la consola al añadir un equipo: código, `vincular` en el
/// equipo, etiqueta con K_cfg y orden `alta` (seq 1) con la clave dada.
fn emparejar_y_alta(consola: &Consola, url: &str, c: &str, sal_cliente: &str, clave: &str) -> Alta {
    let (_, emp) = consola.pedir("POST", &format!("/api/clientes/{c}/emparejamientos"), None);
    let (codigo, emp_id) = (emp["codigo"].as_str().unwrap().to_string(), emp["id"].as_str().unwrap().to_string());
    let vinc = vincular(url, &codigo, "PRUEBA-TRASLADO").unwrap();
    let (_, est) = consola.pedir("GET", &format!("/api/clientes/{c}/emparejamientos/{emp_id}"), None);
    let eq = &est["equipo"];
    let (id, box_pub, sign_pub) =
        (eq["id"].as_str().unwrap().to_string(), eq["box_pub"].as_str().unwrap().to_string(), eq["sign_pub"].as_str().unwrap().to_string());
    let kcfg = derivaciones::k_cfg(clave, sal_cliente).unwrap();
    let etiqueta = derivaciones::etiqueta_equipo(&kcfg, &id, &box_pub, &sign_pub);
    let (st, r) = consola.pedir("POST", &format!("/api/clientes/{c}/emparejamientos/{emp_id}/confirmar"), Some(json!({ "etiqueta": etiqueta })));
    assert_eq!(st, 200, "{r}");
    let prueba = derivaciones::prueba_admin(clave, eq["sal_equipo"].as_str().unwrap()).unwrap();
    let verificador = B64.encode(derivaciones::verificador(&prueba));
    let e = Equipo { c: c.to_string(), id: id.clone(), box_pub, prueba };
    let mut alta = orden(&e, 1, "alta", json!({ "verificador": verificador, "k_cfg": B64.encode(kcfg), "espera_min_horas": 1 }));
    alta.autorizacion.prueba_codigo = Some(orden_v2::prueba_codigo(&codigo, &id, &verificador));
    enviar(consola, &e, &alta);
    let _ = ronda(); // el servidor de antes puede estar muerto
    Alta { e, kcfg, vinc }
}

/// Un equipo en S1 con un repositorio (disco local) y una copia programada,
/// más un «Servidor de copias» y una nube que el servidor no gestiona.
fn preparar(s1: &Srv, c1: &Consola, base: &Path) -> (Alta, String, PathBuf) {
    // Lo que el equipo tiene por su cuenta: tiene que seguir igual pase lo que pase.
    crate::agent::prepare_dir().unwrap();
    let sc = crate::server::ServerConfig {
        path: base.join("guardadas").display().to_string(),
        port: 8123,
        cert_names: vec!["equipo.local".into()],
        ..Default::default()
    };
    std::fs::write(crate::agent::agent_dir().join("servidor.json"), serde_json::to_vec(&sc).unwrap()).unwrap();
    let nubes = vec![crate::nube::Nube { nombre: "Dropbox prueba".into(), tipo: "dropbox".into(), token: "{}".into(), app_key: None }];
    std::fs::write(crate::agent::private_dir().join("nubes.bin"), crate::platform::protect(&serde_json::to_vec(&nubes).unwrap()).unwrap()).unwrap();

    let (c, sal) = cliente(c1, "Café del Sur");
    let a = emparejar_y_alta(c1, &s1.url, &c, &sal, CLAVE);
    assert!(!a.vinc.espera_alta);
    assert_eq!(estado(c1, &a.e, 1)["estado"], "hecha");
    let datos = base.join("datos");
    std::fs::create_dir_all(&datos).unwrap();
    std::fs::write(datos.join("hola.txt"), b"hola").unwrap();
    enviar(
        c1,
        &a.e,
        &orden(
            &a.e,
            2,
            "crear_repositorio",
            json!({ "id": "r1", "nombre": "Principal", "contrasena": "contraseña del repo",
                    "destino": { "id": "d1", "nombre": "Disco", "tipo": "local", "donde": base.join("destino").display().to_string() } }),
        ),
    );
    ronda().unwrap();
    assert_eq!(estado(c1, &a.e, 2)["estado"], "hecha", "{}", estado(c1, &a.e, 2));
    let cfg = json!({ "v": 1, "copias": [{ "id": "docs", "nombre": "Documentos", "repo": "r1", "carpetas": [datos.display().to_string()],
        "exclusiones": [], "horario": { "dias": [1, 2, 3, 4, 5, 6, 7], "horas": ["13:00"] }, "activa": true }] });
    enviar(c1, &a.e, &orden(&a.e, 3, "config", json!({ "config": cfg })));
    ronda().unwrap();
    assert_eq!(estado(c1, &a.e, 3)["estado"], "hecha", "{}", estado(c1, &a.e, 3));
    (a, sal, datos)
}

/// Todo lo que el equipo tiene configurado (sin el vínculo con el servidor).
fn huella_local() -> Value {
    let v = cargar().unwrap();
    let secretos: std::collections::BTreeMap<String, String> = crate::agent::load_secrets().unwrap().into_iter().map(|(k, s)| (k, s.password)).collect();
    json!({
        "agente": serde_json::to_value(crate::agent::load_config()).unwrap(),
        "secretos": secretos,
        "guarda_copias": serde_json::to_value(crate::server::load()).unwrap(),
        "nubes": serde_json::to_value(crate::nube::cargar()).unwrap(),
        "repos": serde_json::to_value(&v.repos_v2).unwrap(),
        "destinos": serde_json::to_value(&v.destinos).unwrap(),
        "config": v.config_v1,
        "verificador": v.verificador,
        "espera": v.espera_min_horas,
        "claves": [v.box_secret, v.sign_seed, v.sal_equipo],
    })
}

/// La configuración que ve la consola de un servidor, descifrada con su K_cfg.
fn config_en_consola(consola: &Consola, e: &Equipo, kcfg: &[u8; 32]) -> (Value, Value) {
    let (st, cf) = consola.pedir("GET", &format!("/api/clientes/{}/equipos/{}/config", e.c, e.id), None);
    assert_eq!(st, 200, "{cf}");
    let plano = simetrico::descifrar_config(kcfg, &e.id, cf["seq"].as_u64().unwrap(), &B64.decode(cf["cifrado"].as_str().unwrap()).unwrap())
        .expect("la consola descifra la configuración con su clave");
    (serde_json::from_slice(&plano).unwrap(), cf["resumen"].clone())
}

/// La consola nueva tiene el historial de antes del equipo (v1.23): la copia
/// que hizo mientras no había servidor, con sus cifras y sin rutas.
fn historial_en_consola(consola: &Consola, e: &Equipo) {
    let (st, h) = consola.pedir("GET", &format!("/api/clientes/{}/equipos/{}/historial", e.c, e.id), None);
    assert_eq!(st, 200, "{h}");
    let copia = h.as_array().unwrap().iter().find(|x| x["tipo"] == "copia" && x["copia"] == "docs").unwrap_or_else(|| panic!("sin la copia de antes: {h}"));
    assert_eq!((copia["repo"].as_str(), copia["resultado"].as_str()), (Some("r1"), Some("ok")), "{copia}");
    assert!(copia["duracion_s"].is_number(), "{copia}");
    assert!(!h.to_string().contains("hola.txt") && !h.to_string().contains("prueba-traslado"), "sin rutas: {h}");
    // Otra vuelta no repite nada.
    let n = h.as_array().unwrap().len();
    ronda().unwrap();
    let (_, h2) = consola.pedir("GET", &format!("/api/clientes/{}/equipos/{}/historial", e.c, e.id), None);
    assert_eq!(h2.as_array().unwrap().len(), n);
}

fn sin_servidor_sigue_copiando(datos: &Path) {
    // La copia de las 13:00 «tocó» (se activó hace 2 días): el programador la hace sin preguntar a nadie.
    let mut cfg = crate::agent::load_config();
    cfg.repos[0].plans[0].enabled_at = (chrono::Local::now() - chrono::Duration::days(2)).to_rfc3339();
    crate::agent::write_json("agent.json", &cfg).unwrap();
    assert_eq!(crate::agent_main(), 0);
    let run = crate::agent::load_state().runs.remove("r1#docs").expect("la copia programada se hizo sin servidor");
    assert_eq!(run.result, "ok", "{}", run.message);
    let acc = crate::gestion_v2::acceso(&cargar().unwrap(), "r1").unwrap();
    let versiones = resguardo_motor::restic::snapshots(&acc).unwrap();
    assert!(versiones.iter().any(|s| Some(&s.id) == run.snapshot_id.as_ref()));
    assert!(datos.join("hola.txt").is_file());
}

/// Apaga los servidores que quedan y borra los datos de la prueba (en Windows,
/// la base de datos se suelta un poco después de parar el servidor).
fn limpiar(base: &Path, vivos: Vec<Srv>) {
    std::env::remove_var("RESGUARDO_AGENT_DIR");
    for s in vivos {
        let _ = s.matar();
    }
    for _ in 0..50 {
        if std::fs::remove_dir_all(base).is_ok() || !base.exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// Camino 1: servidores de respaldo. S1 muere; a los 3 días el equipo pasa a
/// S2 («Recibir un cliente» con la misma sal) con todo, y sigue copiando.
#[test]
fn perder_la_consola_con_un_servidor_de_respaldo() {
    let _real = crate::restic::tests::real_repo_lock();
    if resguardo_motor::restic::version().is_err() {
        return; // sin restic
    }
    let base = base("respaldo");
    std::env::set_var("RESGUARDO_AGENT_DIR", base.join("agente"));
    let (s1, cod1) = arrancar_en(base.join("s1"));
    let (s2, cod2) = arrancar_en(base.join("s2"));
    let (c1, c2) = (Consola::entrar(&s1, &cod1), Consola::entrar(&s2, &cod2));
    let (a, sal, datos) = preparar(&s1, &c1, &base);
    let url_s1 = s1.url.clone();

    // S2 recibe el cliente (el bloque de «Mover este cliente»: nombre y sal) y da una ficha.
    let (st, rec) = c2.pedir("POST", "/api/clientes/recibir", Some(json!({ "nombre": "Café del Sur", "sal_cliente": sal, "usos": 5 })));
    assert_eq!(st, 200, "{rec}");
    let c_b = rec["cliente"]["id"].as_str().unwrap().to_string();
    let ficha = rec["ficha"].as_str().unwrap();
    // Sin «dias»: 3 por defecto.
    let resp = json!({ "servidores": [{ "url": s2.url, "identidad": s2.identidad, "ca_pem": s2.ca, "ficha": ficha }] });
    enviar(&c1, &a.e, &orden(&a.e, 4, "servidores_respaldo", resp));
    ronda().unwrap();
    assert_eq!(estado(&c1, &a.e, 4)["estado"], "hecha", "{}", estado(&c1, &a.e, 4));
    assert_eq!(cargar().unwrap().respaldo_dias, 3);
    let (_, resumen) = config_en_consola(&c1, &a.e, &a.kcfg);
    assert_eq!(resumen["servidores_respaldo"][0]["url"], s2.url.as_str());

    // S1 se apaga. El equipo sigue copiando solo.
    let _ = s1.matar();
    sin_servidor_sigue_copiando(&datos);
    let antes = huella_local();

    // 2 días sin respuesta: aún no se mueve.
    let mut v = cargar().unwrap();
    v.ultimo_ok = chrono::Utc::now().timestamp() - 2 * 86_400;
    guardar(&v).unwrap();
    assert!(ronda().is_err());
    assert_eq!(cargar().unwrap().url, url_s1);

    // 3 días: pasa al de respaldo, el mismo equipo.
    let mut v = cargar().unwrap();
    v.ultimo_ok = chrono::Utc::now().timestamp() - 3 * 86_400 - 60;
    guardar(&v).unwrap();
    assert!(ronda().is_err(), "esta vuelta aún era con S1");
    let v = cargar().unwrap();
    assert_eq!((v.url.as_str(), v.cliente_id.as_str(), v.equipo_id.as_str()), (s2.url.as_str(), c_b.as_str(), a.e.id.as_str()));
    assert_eq!(huella_local(), antes, "nada se reconfigura en el equipo");
    assert!(ronda().is_ok());

    // La consola de S2 lo ve todo: el equipo confirmado, sus copias, repositorios y la última copia.
    let (_, eq) = c2.pedir("GET", &format!("/api/clientes/{c_b}/equipos/{}", a.e.id), None);
    assert_eq!(eq["confirmado"], true, "{eq}");
    let eb = Equipo { c: c_b.clone(), id: a.e.id.clone(), box_pub: a.e.box_pub.clone(), prueba: a.e.prueba };
    let (doc, resumen) = config_en_consola(&c2, &eb, &a.kcfg);
    assert_eq!(doc["copias"][0]["id"], "docs");
    assert_eq!(doc["repositorios"][0]["id"], "r1");
    assert_eq!(resumen["copias"][0]["ultima"]["estado"], "ok", "{resumen}");
    // Y obedece a S2 con la clave de administración.
    enviar(&c2, &eb, &orden(&eb, 1, "cambiar_espera", json!({ "horas": 6 })));
    ronda().unwrap();
    assert_eq!(estado(&c2, &eb, 1)["estado"], "hecha");
    historial_en_consola(&c2, &eb);
    limpiar(&base, vec![s2]);
}

/// Camino 3: sin respaldo. S1 muere; en S3 se crea otra vez el cliente (otra
/// sal) y, con `vincular` y la misma clave de administración, el equipo pasa
/// con todo. Con otra clave no pasa nada (salvo «empezar de cero»).
#[test]
fn perder_la_consola_y_volver_a_vincular_con_un_codigo() {
    let _real = crate::restic::tests::real_repo_lock();
    if resguardo_motor::restic::version().is_err() {
        return; // sin restic
    }
    let base = base("vincular");
    std::env::set_var("RESGUARDO_AGENT_DIR", base.join("agente"));
    let (s1, cod1) = arrancar_en(base.join("s1"));
    let c1 = Consola::entrar(&s1, &cod1);
    let (a1, _, datos) = preparar(&s1, &c1, &base);
    let url_s1 = s1.url.clone();
    let _ = s1.matar();
    sin_servidor_sigue_copiando(&datos);
    let antes = huella_local();
    let k_cfg_antes = cargar().unwrap().k_cfg;

    let (s3, cod3) = arrancar_en(base.join("s3"));
    let c3 = Consola::entrar(&s3, &cod3);

    // 1. Un código de OTRO cliente (otra clave): el equipo no cambia de dueño.
    let (otro, sal_otro) = cliente(&c3, "Otro");
    let x = emparejar_y_alta(&c3, &s3.url, &otro, &sal_otro, OTRA);
    assert!(x.vinc.espera_alta, "ya tenía clave: queda pendiente del alta");
    let o1 = estado(&c3, &x.e, 1);
    assert_eq!(o1["estado"], "rechazada", "{o1}");
    let v = cargar().unwrap();
    assert_eq!(v.url, url_s1, "sigue con el servidor de antes");
    assert_eq!(v.adopcion.as_ref().unwrap().url, s3.url);
    assert_eq!(huella_local(), antes);
    // S3 no recibe ni la configuración ni el resumen, ni puede mandar otras órdenes.
    let (st, _) = c3.pedir("GET", &format!("/api/clientes/{otro}/equipos/{}/config", x.e.id), None);
    assert_ne!(st, 200);
    enviar(&c3, &x.e, &orden(&x.e, 2, "copiar_ahora", json!({ "copia": "docs" })));
    let _ = ronda();
    assert_eq!(estado(&c3, &x.e, 2)["estado"], "rechazada");
    assert_eq!(cargar().unwrap().url, url_s1);

    // 2. El cliente de verdad, creado otra vez en S3 (otra sal) con la MISMA clave.
    let (sur, sal_sur) = cliente(&c3, "Café del Sur");
    let a = emparejar_y_alta(&c3, &s3.url, &sur, &sal_sur, CLAVE);
    assert!(a.vinc.espera_alta);
    let o = estado(&c3, &a.e, 1);
    assert_eq!(o["estado"], "hecha", "{o}");
    let v = cargar().unwrap();
    assert_eq!((v.url.as_str(), v.cliente_id.as_str(), v.equipo_id.as_str()), (s3.url.as_str(), sur.as_str(), a.e.id.as_str()));
    assert!(v.adopcion.is_none() && v.codigo.is_none());
    assert_eq!(v.k_cfg.as_deref(), Some(B64.encode(a.kcfg).as_str()), "toma la K_cfg del cliente nuevo");
    assert_ne!(v.k_cfg, k_cfg_antes);
    assert_eq!(huella_local(), antes, "nada se pierde ni se reconfigura");
    assert_eq!(a1.e.prueba, a.e.prueba, "la misma clave de administración");
    // La consola nueva lo ve todo, descifrado con su K_cfg.
    let (doc, resumen) = config_en_consola(&c3, &a.e, &a.kcfg);
    assert_eq!(doc["copias"][0]["id"], "docs");
    assert_eq!(doc["repositorios"][0]["id"], "r1");
    assert_eq!(doc["destinos"][0]["id"], "d1");
    assert_eq!(resumen["copias"][0]["ultima"]["estado"], "ok", "{resumen}");
    let (_, eq) = c3.pedir("GET", &format!("/api/clientes/{sur}/equipos/{}", a.e.id), None);
    assert_eq!(eq["confirmado"], true);
    // Y le obedece con la clave de administración.
    enviar(&c3, &a.e, &orden(&a.e, 2, "cambiar_espera", json!({ "horas": 6 })));
    ronda().unwrap();
    assert_eq!(estado(&c3, &a.e, 2)["estado"], "hecha");
    // Y su historial de antes llega a la consola nueva (v1.23): la copia que hizo sin servidor.
    historial_en_consola(&c3, &a.e);

    // 3. «Empezar de cero» para otro cliente: olvida lo gestionado (lo copiado se queda).
    empezar_de_cero().unwrap();
    assert!(cargar().is_none());
    assert!(crate::agent::load_config().repos.is_empty(), "sin copias programadas");
    let y = emparejar_y_alta(&c3, &s3.url, &otro, &sal_otro, OTRA);
    assert!(!y.vinc.espera_alta);
    assert_eq!(estado(&c3, &y.e, 1)["estado"], "hecha");
    let v = cargar().unwrap();
    assert_eq!(v.cliente_id, otro);
    assert!(v.repos_v2.is_empty() && v.config_v1.is_none());
    assert!(base.join("destino").is_dir(), "lo copiado sigue en su destino");
    limpiar(&base, vec![s3]);
}
