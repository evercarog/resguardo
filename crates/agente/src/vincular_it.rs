//! 14ee: `vincular --result` (lo que lanza el instalador) cuando la carpeta del agente
//! aún no existe, como justo después de instalar con el servicio recién arrancado. Antes,
//! si fallaba ahí, el motivo no quedaba en ningún registro; ahora `vincular` prepara la
//! carpeta él mismo y deja siempre el resultado y el motivo. Con un Resguardo Server real
//! (TLS propio) en el mismo proceso.

use crate::traslado_it::{arrancar_en, Consola};
use serde_json::json;
use std::path::Path;

fn vincular(args: &[&str]) -> i32 {
    let mut v = vec!["vincular".to_string()];
    v.extend(args.iter().map(|a| a.to_string()));
    crate::cli::ejecutar(&v)
}

fn leer(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_default()
}

#[test]
fn vincular_sin_carpeta_preparada() {
    // RESGUARDO_AGENT_DIR es de todo el proceso: como las demás pruebas que lo cambian, con el candado.
    let _real = crate::restic::tests::real_repo_lock();
    let base = std::env::temp_dir().join(format!("resguardo-vincular-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let dir = base.join("agente");
    std::env::set_var("RESGUARDO_AGENT_DIR", &dir);
    let resultado = base.join("pair.txt");
    let res = resultado.display().to_string();

    // 1. Un servidor que no responde, sin carpeta del agente: el resultado dice el motivo y el
    //    registro también (la carpeta se preparó antes), sin el código.
    assert!(!dir.exists());
    assert_eq!(vincular(&["ABCD-EFGH-JK", "--servidor", "https://127.0.0.1:1", "--result", &res]), 1);
    let r = leer(&resultado);
    assert!(r.starts_with("error\r\n") && r.lines().nth(1).is_some_and(|l| !l.trim().is_empty()), "{r}");
    assert!(dir.join("privado").is_dir(), "vincular prepara la carpeta del agente");
    let log = leer(&dir.join("agent.log"));
    assert!(log.contains("Vinculando este equipo con https://127.0.0.1:1"), "{log}");
    assert!(log.contains("No se pudo vincular"), "{log}");
    assert!(!log.contains("ABCD-EFGH-JK"), "el código no va al registro: {log}");

    // 2. Con --esperar sigue intentando un rato antes de rendirse (solo antes de unirse).
    let t = std::time::Instant::now();
    assert_eq!(vincular(&["ABCD-EFGH-JK", "--servidor", "https://127.0.0.1:1", "--esperar", "8", "--result", &res]), 1);
    assert!(t.elapsed() >= std::time::Duration::from_secs(4), "{:?}", t.elapsed());
    assert!(leer(&dir.join("agent.log")).contains("se sigue intentando"));
    // Unos datos que no valen también dejan el resultado (antes salía sin escribir nada).
    assert_eq!(vincular(&["ABCD-EFGH-JK", "--result", &res]), 1);
    assert!(leer(&resultado).starts_with("error\r\nFalta --servidor"), "{}", leer(&resultado));

    // 3. Un servidor real, otra vez sin carpeta: se vincula y el vínculo queda guardado.
    std::fs::remove_dir_all(&dir).unwrap();
    let (srv, cod) = arrancar_en(base.join("srv"));
    let consola = Consola::entrar(&srv, &cod);
    let (st, c) = consola.pedir("POST", "/api/clientes", Some(json!({ "nombre": "Ventas", "espera_min_horas": 1 })));
    assert_eq!(st, 200, "{c}");
    let cliente = c["id"].as_str().unwrap().to_string();
    let (_, emp) = consola.pedir("POST", &format!("/api/clientes/{cliente}/emparejamientos"), None);
    let codigo = emp["codigo"].as_str().unwrap().to_string();
    assert_eq!(vincular(&[&codigo, "--servidor", &srv.url, "--nombre", "pc-01", "--esperar", "10", "--result", &res]), 0);
    let r = leer(&resultado);
    assert!(r.starts_with("ok\r\n"), "{r}");
    assert_eq!(r.lines().nth(2), Some(srv.url.as_str()));
    let v = crate::servidor_v2::cargar().expect("vínculo guardado");
    assert_eq!(v.url, srv.url);
    let log = leer(&dir.join("agent.log"));
    assert!(log.contains("Vinculado con Resguardo Server"), "{log}");
    assert!(!log.contains(&codigo), "el código no va al registro");

    std::env::remove_var("RESGUARDO_AGENT_DIR");
    let _ = srv.matar();
    let _ = std::fs::remove_dir_all(&base);
}
