//! Prueba de integración REAL del Servidor de copias (fase 4): el rest-server
//! oficial con los mismos argumentos, certificados y `.htpasswd` que en
//! producción, y restic contra él. Solo se ejecuta con
//! `RESGUARDO_TEST_REST_SERVER=<ruta a rest-server.exe>` (y restic en el PATH
//! o junto al ejecutable de la prueba).

use crate::restic::{self, Access};
use crate::server::{binary_check_at, htpasswd_line, Files};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Para el rest-server (por su PID) y borra la carpeta temporal, pase lo que pase.
struct Cleanup {
    child: Option<Child>,
    dir: PathBuf,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn access(port: u16, owner: &str, user: &str, http_password: &str, ca: Option<&Path>) -> Access {
    Access {
        location: format!("rest:https://127.0.0.1:{port}/{owner}/equipo/"),
        password: "clave-del-repositorio".into(),
        rest_auth: Some((user.into(), http_password.into())),
        cacert: ca.map(|p| p.display().to_string()),
        env: Vec::new(),
    }
}

fn run(a: &Access, args: &[&str]) -> restic::RawOutput {
    restic::run_raw(a, args, Duration::from_secs(120)).expect("no se pudo ejecutar restic")
}

#[test]
fn servidor_de_copias_real() {
    let Ok(bin) = std::env::var("RESGUARDO_TEST_REST_SERVER") else {
        return;
    };
    let bin = PathBuf::from(bin);

    // 6. El binario incluido coincide con la huella fijada.
    binary_check_at(&bin).expect("la huella del rest-server incluido no coincide");

    // 1. Certificados y usuarios como en producción, en una carpeta temporal.
    let dir = std::env::temp_dir().join(format!("resguardo-servidor-{}", uuid::Uuid::new_v4()));
    let mut guard = Cleanup { child: None, dir: dir.clone() };
    let files = Files { public: dir.join("publico"), private: dir.join("privado") };
    let data = dir.join("copias");
    let source = dir.join("origen");
    for d in [&files.public, &files.private, &data, &source] {
        std::fs::create_dir_all(d).unwrap();
    }
    std::fs::write(source.join("factura.txt"), "Factura de prueba\n".repeat(200)).unwrap();
    std::fs::write(source.join("notas.md"), "# Notas\n").unwrap();
    files.make_cert(&["localhost".into(), "127.0.0.1".into()]).unwrap();
    let (pass_a, pass_b) = (crate::server::new_password(), crate::server::new_password());
    files.write_htpasswd(&[htpasswd_line("ana", &pass_a).unwrap(), htpasswd_line("beto", &pass_b).unwrap()]).unwrap();

    // 2. El rest-server real, con los argumentos de producción, en un puerto libre.
    let port = free_port();
    let log = std::fs::File::create(dir.join("rest-server.log")).unwrap();
    let child = Command::new(&bin)
        .args(files.args(&data.display().to_string(), &format!("127.0.0.1:{port}")))
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .expect("no arranca el rest-server");
    guard.child = Some(child);
    let start = Instant::now();
    while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(start.elapsed() < Duration::from_secs(15), "el rest-server no escucha");
        std::thread::sleep(Duration::from_millis(200));
    }
    let ca = files.ca();
    let a = access(port, "ana", "ana", &pass_a, Some(&ca));

    // Sin la autoridad fijada, TLS no se acepta.
    let no_ca = run(&access(port, "ana", "ana", &pass_a, None), &["init"]);
    eprintln!("sin --cacert: código {:?}: {}", no_ca.code, no_ca.stderr.trim());
    assert_ne!(no_ca.code, Some(0), "sin --cacert no debería conectar");

    // 3. Ana: crear, copiar y ver versiones.
    let init = run(&a, &["init"]);
    assert_eq!(init.code, Some(0), "init: {}", init.stderr);
    let backup = run(&a, &["backup", &source.display().to_string()]);
    assert_eq!(backup.code, Some(0), "backup: {}", backup.stderr);
    let snaps = restic::snapshots(&a).expect("snapshots");
    assert_eq!(snaps.len(), 1);

    // 4. Beto no puede leer ni listar el repositorio de Ana (--private-repos).
    for args in [&["snapshots"][..], &["cat", "config"][..]] {
        let out = run(&access(port, "ana", "beto", &pass_b, Some(&ca)), args);
        eprintln!("beto {args:?}: código {:?}: {}", out.code, out.stderr.trim());
        assert_ne!(out.code, Some(0), "beto pudo leer el de ana");
        assert!(out.stderr.contains("401") || out.stderr.contains("403"), "se esperaba 401/403: {}", out.stderr);
    }
    // Ni con una contraseña equivocada.
    let wrong = run(&access(port, "ana", "ana", "no-es-esta", Some(&ca)), &["snapshots"]);
    assert_ne!(wrong.code, Some(0));
    assert!(wrong.stderr.contains("401"), "se esperaba 401: {}", wrong.stderr);

    // 4b. Un equipo nuevo (como «Copiar en …» de la consola): el rest-server relee
    // `.htpasswd` como mucho cada 30 s, así que hasta entonces le daría 401.
    // `esperar_usuario_en` espera a que lo acepte; justo después ya puede crear su repositorio.
    let pass_c = crate::server::new_password();
    files.write_htpasswd(&[htpasswd_line("ana", &pass_a).unwrap(), htpasswd_line("beto", &pass_b).unwrap(), htpasswd_line("carla", &pass_c).unwrap()]).unwrap();
    let espera = Instant::now();
    crate::server::esperar_usuario_en(port, &ca, "carla", &pass_c).expect("el rest-server acepta a carla");
    eprintln!("carla aceptada en {:?}", espera.elapsed());
    let init_c = run(&access(port, "carla", "carla", &pass_c, Some(&ca)), &["init"]);
    assert_eq!(init_c.code, Some(0), "carla, recién añadida: {}", init_c.stderr);

    // 5. Solo añadir: borrar versiones falla y la versión sigue ahí.
    let forget = run(&a, &["forget", "--prune", &snaps[0].id]);
    eprintln!("forget --prune: código {:?}: {}", forget.code, forget.stderr.trim());
    assert_ne!(forget.code, Some(0), "append-only: forget --prune no debería poder borrar");
    let after = restic::snapshots(&a).expect("snapshots tras forget");
    assert_eq!(after.len(), 1, "la versión desapareció pese a --append-only");
    // La comprobación diaria lo ve con la autoridad propia; sin ella (las del sistema), no se sabe.
    let ca_txt = ca.display().to_string();
    assert_eq!(crate::protection::probe_append_only(&a.location, Some(("ana", &pass_a)), Some(&ca_txt)), Some(true));
    assert_eq!(crate::protection::probe_append_only(&a.location, Some(("ana", &pass_a)), None), None);

    // 5b. Venir de un repositorio antiguo (docs/agente-gestionado.md, «Venir de la
    // app de escritorio»), con el destino tal como lo crea «Copiar en …»
    // (`https://ip:puerto/<usuario>/`, usuario y autoridad del almacén).
    // RESGUARDO_AGENT_DIR es de todo el proceso: como las demás pruebas que lo cambian, con el candado.
    let _real = crate::restic::tests::real_repo_lock();
    std::env::set_var("RESGUARDO_AGENT_DIR", dir.join("agente"));
    let antiguo = dir.join("antiguo");
    let viejo = Access::new(antiguo.display().to_string(), "clave antigua");
    assert_eq!(run(&viejo, &["init"]).code, Some(0));
    for _ in 0..2 {
        std::fs::write(source.join("factura.txt"), format!("Factura {}\n", uuid::Uuid::new_v4()).repeat(300)).unwrap();
        let b = run(&viejo, &["backup", "--host", "SERVIDOR-01", &source.display().to_string()]);
        assert_eq!(b.code, Some(0), "{}", b.stderr);
    }
    let pem = std::fs::read_to_string(&ca).unwrap();
    let destino = serde_json::json!({ "id": "almacen-1a2b3c4d", "nombre": "ALMACEN-01", "tipo": "rest", "donde": format!("https://127.0.0.1:{port}/ana/"),
                                      "usuario": "ana", "secreto": pass_a, "ca_pem": pem });
    let mut v = crate::servidor_v2::Vinculo::default();
    //  - Copiar: un repositorio nuevo en el almacén con los parámetros de troceado
    //    del antiguo, y su historial traído después; el antiguo no se toca.
    let origen = serde_json::json!({ "destino": { "tipo": "local", "donde": dir.display().to_string() }, "ruta": "antiguo", "contrasena": "clave antigua" });
    let c =
        serde_json::json!({ "id": "siigo-copia", "nombre": "Siigo", "contrasena": "clave del repositorio nuevo", "destino": destino, "parametros_de": origen });
    crate::gestion_v2::crear_repositorio(&mut v, &c).expect("crear con parametros_de en el almacén");
    let src = crate::adoptar_v2::origen(&v, &origen).unwrap();
    let m = crate::adoptar_v2::traer(&v, "siigo-copia", &src, &crate::adoptar_v2::Filtro::default(), &mut |_, _| {}).expect("traer el historial al almacén");
    eprintln!("traer: {m}");
    let nuevo = crate::gestion_v2::acceso(&v, "siigo-copia").unwrap();
    assert!(nuevo.location.ends_with("/ana/siigo-copia"), "{}", nuevo.location);
    assert_eq!(restic::snapshots(&nuevo).unwrap().len(), 2, "todo el historial");
    let params = |a: &Access| {
        let o = run(a, &["cat", "config", "--no-lock"]);
        serde_json::from_slice::<serde_json::Value>(&o.stdout).unwrap()["chunker_polynomial"].clone()
    };
    assert_eq!(params(&nuevo), params(&viejo), "mismo troceado: lo traído se deduplica igual");
    assert_eq!(restic::snapshots(&viejo).unwrap().len(), 2, "el antiguo no se toca");
    //  - Mover y adoptar: la carpeta antigua, dentro de la del usuario en el
    //    almacén; se adopta con el destino que el equipo ya tiene y solo su nombre.
    std::fs::rename(&antiguo, data.join("ana").join("siigo")).unwrap();
    let c =
        serde_json::json!({ "id": "siigo", "nombre": "Siigo movido", "contrasena": "clave antigua", "destino": { "id": "almacen-1a2b3c4d" }, "ruta": "siigo" });
    let (m, d) = crate::adoptar_v2::adoptar_repositorio(&mut v, &c, false).expect("adoptar en el almacén");
    eprintln!("adoptar: {m}");
    assert_eq!(d["versiones"], 2);
    // `solo_anadir`: la sonda HTTP confía en la autoridad propia del almacén (la del destino).
    assert_eq!(d["solo_anadir"], true, "el almacén es de solo añadir: {d}");
    let movido = crate::gestion_v2::acceso(&v, "siigo").unwrap();
    let b = run(&movido, &["backup", "--host", "SERVIDOR-01", &source.display().to_string()]);
    assert_eq!(b.code, Some(0), "sigue copiando en él: {}", b.stderr);
    assert_eq!(restic::snapshots(&movido).unwrap().len(), 3);

    // 5c. Retención en el almacén (v1.22): el equipo no puede podar (403), así
    // que añade una clave propia del almacén por el rest-server de solo añadir
    // y el almacén poda en local con ella.
    let clave = "clave-propia-del-almacen-0123456789";
    let m = crate::retencion_almacen::anadir_clave(&v, "siigo-copia", &serde_json::json!({ "clave": clave })).expect("añadir la clave por el rest-server");
    eprintln!("clave_almacen: {m}");
    crate::server::save(&crate::server::ServerConfig {
        enabled: true,
        path: data.display().to_string(),
        users: vec![crate::server::ServerUser { name: "ana".into(), created_at: String::new() }],
        ..Default::default()
    })
    .unwrap();
    let regla = serde_json::json!({ "usuario": "ana", "repo": "siigo-copia", "clave": clave,
        "retencion": { "diarias": 1, "semanales": 0, "mensuales": 0, "anuales": 0 }, "horario": { "dias": [7], "hora": "03:00" } });
    let m = crate::retencion_almacen::configurar(&regla).expect("poner la retención en el almacén");
    assert!(!m.contains(crate::retencion_almacen::SIN_CLAVE), "{m}");
    let m = crate::retencion_almacen::aplicar("ana", "siigo-copia").expect("aplicar en el almacén");
    eprintln!("aplicar en el almacén: {m}");
    assert_eq!(restic::snapshots(&nuevo).unwrap().len(), 1, "el almacén podó: {m}");
    let r = crate::retencion_almacen::resumen();
    assert_eq!((r[0]["clave"].as_str(), r[0]["resultado"].as_str()), (Some("ok"), Some("ok")), "{r:?}");
    assert!(!serde_json::to_string(&r).unwrap().contains(clave), "el resumen no lleva la clave");
    // Desde el equipo sigue sin poderse borrar nada.
    let snap = restic::snapshots(&nuevo).unwrap()[0].id.clone();
    assert_ne!(run(&nuevo, &["forget", &snap]).code, Some(0), "el equipo no puede borrar");
    // Quitar la regla borra la clave del almacén: ya no abre; la del equipo, sí.
    crate::retencion_almacen::configurar(&serde_json::json!({ "usuario": "ana", "repo": "siigo-copia", "quitar": true })).unwrap();
    let del_almacen = Access::new(data.join("ana").join("siigo-copia").display().to_string(), clave);
    assert_ne!(run(&del_almacen, &["cat", "config", "--no-lock"]).code, Some(0), "clave revocada");
    assert_eq!(restic::snapshots(&nuevo).unwrap().len(), 1);

    // 5d. Un repositorio en su propio almacén (v1.28, `guarda_copias { anadir, local: true }`):
    // el almacén copia en sí mismo por `localhost`, con su usuario (beto), y
    // sigue siendo de solo añadir; su retención (con plazos) la aplica él en local.
    let propio = serde_json::json!({ "id": "almacen-propio", "nombre": "Este almacén", "tipo": "rest",
        "donde": crate::server::ubicacion_local(port, "beto").trim_start_matches("rest:"), "usuario": "beto", "secreto": pass_b, "ca_pem": pem });
    let c = serde_json::json!({ "id": "consola", "nombre": "Copia de la consola", "contrasena": "clave del repositorio propio", "destino": propio });
    crate::gestion_v2::crear_repositorio(&mut v, &c).expect("crear en el propio almacén por localhost");
    let mio = crate::gestion_v2::acceso(&v, "consola").unwrap();
    assert!(mio.location.starts_with("rest:https://localhost:"), "{}", mio.location);
    // Hace 4 h, 2 h y 10 min (horas de verdad: el almacén se fía de ellas, cuadran con la subida).
    for minutos in [240, 120, 10] {
        std::fs::write(source.join("factura.txt"), format!("Consola {}\n", uuid::Uuid::new_v4()).repeat(50)).unwrap();
        let cuando = (chrono::Local::now() - chrono::Duration::minutes(minutos)).format("%Y-%m-%d %H:%M:%S").to_string();
        assert_eq!(run(&mio, &["backup", "--host", "ALMACEN", "--time", &cuando, &source.display().to_string()]).code, Some(0));
    }
    let snap = restic::snapshots(&mio).unwrap()[0].id.clone();
    assert_ne!(run(&mio, &["forget", &snap]).code, Some(0), "su usuario tampoco puede borrar en su almacén");
    let clave = "clave-propia-del-almacen-para-si-mismo";
    crate::retencion_almacen::anadir_clave(&v, "consola", &serde_json::json!({ "clave": clave })).expect("su clave, por localhost");
    let mut cfg = crate::server::load();
    cfg.users.push(crate::server::ServerUser { name: "beto".into(), created_at: String::new() });
    crate::server::save(&cfg).unwrap();
    let regla = serde_json::json!({ "usuario": "beto", "repo": "consola", "clave": clave, "verificar": false,
        "retencion": { "plazos": { "horarias": "1h" } }, "horario": { "dias": [7], "hora": "03:00" } });
    crate::retencion_almacen::configurar(&regla).expect("retención con plazos en el almacén");
    let m = crate::retencion_almacen::aplicar("beto", "consola").expect("aplicar en el almacén");
    eprintln!("aplicar en el propio almacén: {m}");
    assert_eq!(restic::snapshots(&mio).unwrap().len(), 1, "en la última hora solo está la más reciente: queda una ({m})");
    std::env::remove_var("RESGUARDO_AGENT_DIR");

    // 7. Se para por su PID y se borra la carpeta (Cleanup); se comprueba que ya no escucha.
    drop(guard);
    std::thread::sleep(Duration::from_millis(300));
    assert!(std::net::TcpStream::connect_timeout(&std::net::SocketAddr::from(([127, 0, 0, 1], port)), Duration::from_millis(500)).is_err());
    assert!(!dir.exists(), "quedó la carpeta temporal");
}
