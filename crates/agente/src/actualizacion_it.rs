//! Prueba de integración de la actualización automática (docs/actualizaciones.md)
//! con un **Resguardo Server real** (TLS) en el mismo proceso: la consola sirve
//! una versión firmada con la llave de pruebas, el equipo la ve, la comprueba,
//! decide con la política de la consola, la baja y «se instala» con la
//! plataforma simulada (el paso que cambia el programa se prueba en máquinas
//! virtuales, §11).
//!
//! 1. Anillo general: espera sus días.
//! 2. En pausa: no se instala.
//! 3. Anillo de prueba con una versión que no está sana: vuelta atrás, aviso
//!    `actualizacion_fallida` en la consola y la versión retenida para el cliente.
//! 4. «Actualizar ahora»: se instala aunque esté retenida y queda actualizada.

use crate::servidor_v2::{self as s};
use crate::traslado_it::{arrancar_con, Consola};
use resguardo_protocolo::publicacion::{self as p, pruebas as pp};
use serde_json::{json, Value};
use std::time::Duration;

fn siguiente(v: &str) -> String {
    let mut n = p::Version::leer(v).unwrap();
    n.pre = None;
    n.numeros[2] += 1;
    n.to_string()
}

/// Espera a que el actualizador simulado deje su resultado y lo procesa.
fn esperar_resultado() {
    let r = super::actualizacion::carpeta().join("resultado.json");
    for _ in 0..100 {
        if r.exists() {
            super::actualizacion::tick();
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("el actualizador simulado no terminó: {:?}", super::actualizacion::leer_estado());
}

fn politica(ca: &Consola, c: &str) -> Value {
    ca.pedir("GET", &format!("/api/clientes/{c}/actualizaciones"), None).1
}

#[test]
fn actualizacion_desde_su_consola_con_vuelta_atras_y_aprobacion() {
    let _real = crate::restic::tests::real_repo_lock();
    let dir_agente = std::env::temp_dir().join(format!("resguardo-act-agente-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir_agente);
    std::fs::create_dir_all(&dir_agente).unwrap();
    std::env::set_var("RESGUARDO_AGENT_DIR", &dir_agente);
    std::env::set_var("RESGUARDO_LLAVES_PRUEBAS", concat!(env!("CARGO_MANIFEST_DIR"), "/../protocolo/tests/fixtures/llave-pruebas-a.pub"));
    std::env::set_var("RESGUARDO_ACTUALIZACION_PLAZO_S", "1");
    let Some(plataforma) = p::plataforma_actual() else { return };

    let opciones = resguardo_servidor::estado::Opciones { llaves_pruebas: Some(pp::LLAVE_A_PUB.to_string()), ..Default::default() };
    let (a, codigo_a) = arrancar_con(std::env::temp_dir().join(format!("resguardo-act-srv-{}", std::process::id())), opciones);
    let ca = Consola::entrar(&a, &codigo_a);
    let (_, cliente) = ca.pedir("POST", "/api/clientes", Some(json!({ "nombre": "Ferretería Altamar", "espera_min_horas": 1 })));
    let c = cliente["id"].as_str().unwrap().to_string();
    let (_, emp) = ca.pedir("POST", &format!("/api/clientes/{c}/emparejamientos"), None);
    s::vincular(&a.url, emp["codigo"].as_str().unwrap(), "PRUEBA-ACT").unwrap();
    let equipo = s::cargar().unwrap().equipo_id;
    assert!(super::actualizacion::activada(), "con la llave de pruebas (compilación de desarrollo)");

    // La consola sirve una versión firmada más nueva.
    let nueva = siguiente(crate::version_programa());
    let paquete = format!("paquete de la versión {nueva}").repeat(500).into_bytes();
    let nombre = if plataforma.starts_with("windows") { "Resguardo-Agente-setup.exe" } else { "resguardo-agente.tar.gz" };
    let m = pp::manifiesto(&nueva, &[(plataforma, nombre, &paquete)]);
    let texto = serde_json::to_string_pretty(&m).unwrap();
    let origen = a.dir.join("origen-publicacion");
    std::fs::create_dir_all(&origen).unwrap();
    std::fs::write(origen.join(p::NOMBRE_MANIFIESTO), &texto).unwrap();
    std::fs::write(origen.join(p::NOMBRE_FIRMA), pp::firmar_a(texto.as_bytes())).unwrap();
    std::fs::write(origen.join(nombre), &paquete).unwrap();
    let llaves_srv =
        resguardo_servidor::publicaciones::llaves(&resguardo_servidor::estado::Opciones { llaves_pruebas: Some(pp::LLAVE_A_PUB.into()), ..Default::default() });
    assert!(resguardo_servidor::publicaciones::poner_carpeta(&a.dir, &llaves_srv, &origen).unwrap().completa());

    // 1. Anillo general (por defecto): espera sus 2 días.
    super::actualizacion::buscar_e_instalar().unwrap();
    let e = super::actualizacion::leer_estado();
    assert_eq!((e.estado.as_str(), e.motivo.as_deref(), e.version_disponible.as_deref()), ("pendiente", Some("espera_anillo"), Some(nueva.as_str())), "{e:?}");
    assert!(e.hasta.is_some());
    assert_eq!(super::actualizacion::informe()["motivo"], "espera_anillo");

    // 2. Anillo de prueba, pero el cliente en pausa: nada.
    assert_eq!(ca.pedir("PUT", &format!("/api/clientes/{c}/equipos/{equipo}/anillo"), Some(json!({ "anillo": "prueba" }))).0, 200);
    assert_eq!(ca.pedir("PUT", &format!("/api/clientes/{c}/actualizaciones"), Some(json!({ "modo": "pausada" }))).0, 200);
    super::actualizacion::buscar_e_instalar().unwrap();
    assert_eq!(super::actualizacion::leer_estado().estado, "pausada");

    // 3. Automática otra vez, con una versión que no llega a estar sana: vuelta atrás.
    assert_eq!(ca.pedir("PUT", &format!("/api/clientes/{c}/actualizaciones"), Some(json!({ "modo": "auto" }))).0, 200);
    std::fs::write(dir_agente.join("simular-actualizacion.txt"), "falla").unwrap();
    super::actualizacion::buscar_e_instalar().unwrap();
    assert_eq!(super::actualizacion::leer_estado().estado, "actualizando");
    esperar_resultado();
    // Lo bajó de la consola y es lo que firmó la llave.
    let simulada: Value = serde_json::from_slice(&std::fs::read(super::actualizacion::carpeta().join("simulada.json")).unwrap()).unwrap();
    assert_eq!(simulada["sha256"], m.archivos[0].sha256.as_str());
    let e = super::actualizacion::leer_estado();
    assert_eq!((e.estado.as_str(), e.version_fallida.as_deref()), ("vuelta_atras", Some(nueva.as_str())), "{e:?}");
    assert!(e.fallidas.contains(&nueva) && e.aviso_para.is_empty(), "aviso mandado a su consola: {e:?}");
    let (_, avisos) = ca.pedir("GET", &format!("/api/clientes/{c}/avisos"), None);
    assert!(avisos.as_array().unwrap().iter().any(|x| x["tipo"] == "actualizacion_fallida"), "{avisos}");
    // La consola la retiene para el resto del cliente (lo dijo el informe).
    assert_eq!(politica(&ca, &c)["politica"]["retenidas"], json!([nueva]));
    // Y no lo vuelve a intentar sola (la consola sigue viendo que volvió a la anterior).
    super::actualizacion::buscar_e_instalar().unwrap();
    assert_eq!(super::actualizacion::leer_estado().estado, "vuelta_atras");
    assert!(!super::actualizacion::carpeta().join("plan.json").exists());

    // 4. «Actualizar ahora» en la consola: se instala (simulada, sana) y queda actualizada.
    std::fs::write(dir_agente.join("simular-actualizacion.txt"), "ok").unwrap();
    let (st, r) = ca.pedir("POST", &format!("/api/clientes/{c}/actualizaciones/ahora"), Some(json!({})));
    assert_eq!((st, r["version"].as_str()), (200, Some(nueva.as_str())), "{r}");
    super::actualizacion::buscar_e_instalar().unwrap();
    esperar_resultado();
    let e = super::actualizacion::leer_estado();
    assert_eq!((e.estado.as_str(), e.version_objetivo.as_deref()), ("actualizada", Some(nueva.as_str())), "{e:?}");
    // Una firma que no es de la llave fijada nunca llega a instalarse (otra publicación, firmada con la B).
    let otra = siguiente(&nueva);
    let m2 = pp::manifiesto(&otra, &[(plataforma, nombre, &paquete)]);
    let t2 = serde_json::to_string(&m2).unwrap();
    assert!(resguardo_servidor::publicaciones::poner_manifiesto(&a.dir, &llaves_srv, &t2, &pp::firmar_b(t2.as_bytes())).is_err());

    std::env::remove_var("RESGUARDO_ACTUALIZACION_PLAZO_S");
    std::env::remove_var("RESGUARDO_LLAVES_PRUEBAS");
    let _ = std::fs::remove_dir_all(&a.dir);
}
