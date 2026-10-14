use super::*;
use crate::espejo_motor::{Alcance, Estado, Lado, Opciones};
use crate::espejo_trabajos::{Adonde, Que, Retencion, Trabajo};
use crate::server::{htpasswd_line, ServerUser};
use std::time::Duration;

const UUID: &str = "0b5c1f8e-1d2a-4c3b-9e8f-7a6b5c4d3e2f";

/// Una carpeta temporal que se borra al terminar.
struct Tmp(PathBuf);
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn tmp(que: &str) -> Tmp {
    let d = std::env::temp_dir().join(format!("resguardo-nombres-{que}-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&d).unwrap();
    Tmp(d)
}

/// Un archivo con fecha de hace una hora (el espejo no copia lo de hace menos de 10 min).
fn escribir(p: &Path, contenido: &[u8]) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, contenido).unwrap();
    std::fs::File::options().write(true).open(p).unwrap().set_modified(SystemTime::now() - Duration::from_secs(3600)).unwrap();
}

/// Un repositorio de mentira (lo que mira el almacén: `config`, `snapshots/`, datos).
fn repo(dir: &Path, n: usize) {
    escribir(&dir.join("config"), b"config");
    std::fs::create_dir_all(dir.join("snapshots")).unwrap();
    for i in 0..n {
        escribir(&dir.join(format!("data/{i:03}")), format!("dato {i}").as_bytes());
    }
}

#[test]
fn vectores_compartidos_con_la_consola() {
    let doc: Value = serde_json::from_str(include_str!("../../../protocolo/vectors/nombres-legibles.json")).unwrap();
    for v in doc["normalizar"].as_array().unwrap() {
        let (e, max, defecto) = (v["entrada"].as_str().unwrap(), v["max"].as_u64().unwrap() as usize, v["defecto"].as_str().unwrap());
        assert_eq!(normalizar(e, max, defecto), v["salida"].as_str().unwrap(), "{e:?}");
        assert!(carpeta_valida(&normalizar(e, max, defecto)), "lo normalizado siempre vale: {e:?}");
    }
    for v in doc["validas"].as_array().unwrap() {
        let c = v["carpeta"].as_str().unwrap();
        assert_eq!(carpeta_valida(c), v["valida"].as_bool().unwrap(), "{c:?}");
    }
}

fn almacen(base: &Path) -> (Files, ServerConfig) {
    let f = Files { public: base.join("publico"), private: base.join("privado") };
    std::fs::create_dir_all(&f.public).unwrap();
    std::fs::create_dir_all(&f.private).unwrap();
    let datos = base.join("copias");
    std::fs::create_dir_all(&datos).unwrap();
    let c = ServerConfig {
        enabled: true,
        path: datos.display().to_string(),
        users: vec![ServerUser { name: UUID.into(), ..Default::default() }],
        ..Default::default()
    };
    (f, c)
}

#[test]
fn usuario_legible_y_libre() {
    let t = tmp("usuario");
    let (_, mut c) = almacen(&t.0);
    let u = usuario_legible(&c, "Recepción · Planta 2").unwrap();
    let (base, suf) = u.rsplit_once('-').unwrap();
    assert_eq!(base, "recepcion-planta-2");
    assert!(suf.len() == 4 && suf.chars().all(|x| x.is_ascii_hexdigit()), "{u}");
    assert!(carpeta_valida(&u) && u.len() <= 40);
    // Ocupado: un usuario, una carpeta en el disco o un nombre antiguo de una carpeta movida.
    assert!(usuario_ocupado(&c, UUID));
    std::fs::create_dir_all(Path::new(&c.path).join("pc-01-aaaa")).unwrap();
    assert!(usuario_ocupado(&c, "pc-01-aaaa"), "una carpeta que quedó de un equipo quitado");
    c.renombres.push(Renombre { de: "pc-02-bbbb".into(), a: "x-cccc".into(), cuando: String::new(), zona: None });
    assert!(usuario_ocupado(&c, "pc-02-bbbb") && usuario_ocupado(&c, "x-cccc"));
    assert!(!usuario_ocupado(&c, "pc-03-dddd"));
    // Sin nombre aprovechable: «equipo-xxxx».
    assert!(usuario_legible(&c, "日本").unwrap().starts_with("equipo-"));
}

#[test]
fn anadir_con_nombre_legible() {
    let t = tmp("anadir");
    let (f, mut c) = almacen(&t.0);
    let (u, pass) = crate::server::create_user_legible(&f, &mut c, None, "7f6e5d4c-0000-4000-8000-000000000001", Some("PC 01")).unwrap();
    assert!(u.starts_with("pc-01-") && u.len() == 10, "{u}");
    let nuevo = c.users.iter().find(|x| x.name == u).unwrap();
    // El id del equipo (como lo deja `user_name`: 32 caracteres), para saber si ya tiene usuario.
    assert_eq!(nuevo.para.as_deref(), Some(crate::server::user_name("7f6e5d4c-0000-4000-8000-000000000001").as_str()));
    assert!(f.read_htpasswd_de(None).iter().any(|l| l.starts_with(&format!("{u}:"))));
    assert_eq!(pass.len(), 32);
    // Sin nombre, como siempre: el id.
    let (u2, _) = crate::server::create_user_en(&f, &mut c, None, "7f6e5d4c-0000-4000-8000-000000000002").unwrap();
    assert_eq!(u2, crate::server::user_name("7f6e5d4c-0000-4000-8000-000000000002"));
}

#[test]
fn leer_el_pedido() {
    let p = leer_pedido(&json!({ "usuario": UUID, "legible": "PC-01", "repos": [{ "de": "almacen-01-3f2a", "nombre": "Contabilidad" }] })).unwrap();
    assert_eq!(p.repos, vec![("almacen-01-3f2a".to_string(), "Contabilidad".to_string())]);
    for mal in [
        json!({ "usuario": "../x", "legible": "a" }),
        json!({ "usuario": UUID }),
        json!({ "usuario": UUID, "repos": [{ "de": "..", "nombre": "x" }] }),
        json!({ "usuario": UUID, "repos": [{ "de": "a/b", "nombre": "x" }] }),
        json!({ "usuario": UUID, "repos": [{ "de": "a", "nombre": "x\u{0}" }] }),
        json!({ "usuario": UUID, "repos": [{ "de": "a", "nombre": "x" }, { "de": "a", "nombre": "y" }] }),
        json!({ "usuario": UUID, "legible": "x".repeat(81) }),
        json!({ "usuario": UUID, "repos": "a" }),
    ] {
        assert!(leer_pedido(&mal).is_err(), "{mal}");
    }
}

#[test]
fn mover_y_deshacer() {
    let t = tmp("mover");
    let (f, mut c) = almacen(&t.0);
    let base = PathBuf::from(&c.path);
    let du = base.join(UUID);
    repo(&du.join("almacen-01-3f2a"), 3);
    repo(&du.join("almacen-01-9c1d"), 2);
    repo(&du.join("ventas"), 1);
    // Otro equipo que no se toca.
    repo(&base.join("caja").join("caja"), 1);
    c.users.push(ServerUser { name: "caja".into(), ..Default::default() });
    let linea = htpasswd_line(UUID, "clave-de-prueba-123").unwrap();
    let otra = htpasswd_line("caja", "otra-clave").unwrap();
    f.write_htpasswd_de(None, &[otra.clone(), linea.clone()]).unwrap();
    // Un espejo con selección y otro de ciertos equipos.
    let mut sel = Trabajo {
        id: "sel".into(),
        adonde: Adonde { tipo: "carpeta".into(), carpeta: t.0.join("espejo").display().to_string(), nube: None },
        ..Default::default()
    };
    sel.activo = true;
    sel.que = Que::Repos { repos: vec![format!("{UUID}/almacen-01-3f2a"), "caja/caja".into()] };
    sel.vistos = vec![format!("{UUID}/almacen-01-3f2a"), format!("{UUID}/ventas"), "caja/caja".into()];
    let mut eqs = sel.clone();
    eqs.id = "eqs".into();
    eqs.que = Que::Equipos { equipos: vec![UUID.into()] };
    eqs.retencion = Retencion::Retraso { dias: 30 };
    c.espejo = Some(crate::espejo::Espejo::de_trabajos(vec![sel, eqs]));
    let antes_cfg = c.clone();

    let p = Pedido {
        usuario: UUID.into(),
        legible: Some("PC 01".into()),
        repos: vec![("almacen-01-3f2a".into(), "Contabilidad".into()), ("almacen-01-9c1d".into(), "Contabilidad".into()), ("ventas".into(), "Ventas".into())],
        deshacer: false,
    };
    let m = mover_en(&f, &mut c, &p, "2026-10-13T10:00:00-05:00").unwrap();
    let nuevo = m.usuario_final().to_string();
    assert!(nuevo.starts_with("pc-01-"), "{nuevo}");
    assert_eq!(
        m.repos,
        vec![("almacen-01-3f2a".to_string(), "contabilidad".to_string()), ("almacen-01-9c1d".to_string(), "contabilidad-2".to_string())],
        "«ventas» ya se llamaba así"
    );
    // En el disco: todo en su sitio nuevo, nada en el viejo, el otro equipo igual.
    let dn = base.join(&nuevo);
    assert!(!du.exists());
    for r in ["contabilidad", "contabilidad-2", "ventas"] {
        assert!(dn.join(r).join("config").is_file(), "{r}");
    }
    assert!(dn.join("contabilidad/data/002").is_file());
    assert!(base.join("caja/caja/config").is_file());
    // La misma contraseña, otro nombre; el otro usuario, igual.
    let l = f.read_htpasswd_de(None);
    assert_eq!(l[0], otra);
    assert_eq!(l[1], format!("{nuevo}:{}", linea.split_once(':').unwrap().1));
    assert!(bcrypt::verify("clave-de-prueba-123", l[1].split_once(':').unwrap().1).unwrap());
    // La configuración: el usuario (con su equipo en `para`), los espejos y lo movido.
    let u = c.users.iter().find(|u| u.name == nuevo).unwrap();
    assert_eq!(u.para.as_deref(), Some(UUID));
    let ts = c.espejo.as_ref().unwrap().trabajos.clone();
    assert_eq!(ts[0].que, Que::Repos { repos: vec![format!("{nuevo}/contabilidad"), "caja/caja".into()] });
    assert_eq!(ts[0].vistos, vec![format!("{nuevo}/contabilidad"), format!("{nuevo}/ventas"), "caja/caja".into()]);
    assert_eq!(ts[1].que, Que::Equipos { equipos: vec![nuevo.clone()] });
    assert_eq!(
        c.espejo.as_ref().unwrap().destinos[0].repos.as_ref().unwrap()[0],
        format!("{nuevo}/contabilidad"),
        "también la vista para una consola anterior"
    );
    assert_eq!(c.renombres.len(), 3);
    assert!(c.renombres.iter().any(|r| r.de == UUID && r.a == nuevo));
    assert!(c.renombres.iter().any(|r| r.de == format!("{UUID}/almacen-01-3f2a") && r.a == format!("{nuevo}/contabilidad")));
    assert_eq!(antiguas(&c, None).len(), 3);
    assert!(antiguas(&c, Some("z0a0b0c")).is_empty(), "son de la principal");
    // Lo que contesta, para el equipo dueño.
    let d = m.detalle();
    assert_eq!(d["mover"]["usuario"]["a"], nuevo.as_str());
    assert_eq!(d["mover"]["repos"][1]["a"], "contabilidad-2");
    assert_eq!(m.ruta_nueva(&format!("{UUID}/ventas")), Some(format!("{nuevo}/ventas")));
    assert_eq!(m.ruta_nueva("caja/caja"), None);
    // La retención del almacén.
    let mut ret: Vec<crate::retencion_almacen::Entrada> = serde_json::from_value(json!([
        { "usuario": UUID, "repo": "almacen-01-3f2a", "clave": "k", "retencion": {}, "horario": { "dias": [7], "hora": "03:00" }, "desde": "" },
        { "usuario": UUID, "repo": "ventas", "clave": "k", "retencion": {}, "horario": { "dias": [7], "hora": "03:00" }, "desde": "" },
        { "usuario": "caja", "repo": "caja", "clave": "k", "retencion": {}, "horario": { "dias": [7], "hora": "03:00" }, "desde": "" }
    ]))
    .unwrap();
    crate::retencion_almacen::renombrar_en(&mut ret, &m.antes, m.usuario_final(), &m.repos);
    let par = |e: &crate::retencion_almacen::Entrada| (e.usuario.clone(), e.repo.clone());
    assert_eq!(par(&ret[0]), (nuevo.clone(), "contabilidad".into()));
    assert_eq!(par(&ret[1]), (nuevo.clone(), "ventas".into()));
    assert_eq!(par(&ret[2]), ("caja".into(), "caja".into()));

    // Deshacer lo deja como estaba en el disco.
    assert!(m.deshacer.aplicar(&f).is_empty());
    assert!(du.join("almacen-01-3f2a/config").is_file() && du.join("almacen-01-9c1d/config").is_file() && !dn.exists());
    assert_eq!(f.read_htpasswd_de(None), vec![otra, linea]);
    let _ = antes_cfg;
}

#[test]
fn no_se_mueve_si_algo_no_cuadra() {
    let t = tmp("rechazos");
    let (f, mut c) = almacen(&t.0);
    let base = PathBuf::from(&c.path);
    let du = base.join(UUID);
    repo(&du.join("r1"), 1);
    f.write_htpasswd_de(None, &[htpasswd_line(UUID, "x").unwrap()]).unwrap();
    let pedir = |legible: Option<&str>, repos: Vec<(&str, &str)>| Pedido {
        usuario: UUID.into(),
        legible: legible.map(str::to_string),
        repos: repos.into_iter().map(|(a, b)| (a.into(), b.into())).collect(),
        deshacer: false,
    };
    // Un equipo que no es de este almacén.
    assert!(mover_en(&f, &mut c, &Pedido { usuario: "otro".into(), ..pedir(Some("x"), vec![]) }, "").unwrap_err().contains("no es un equipo"));
    // Un repositorio que no existe.
    assert!(mover_en(&f, &mut c, &pedir(None, vec![("r9", "Nueve")]), "").unwrap_err().contains("No hay ningún repositorio"));
    // Ya se llama así: nada que mover.
    assert!(mover_en(&f, &mut c, &pedir(None, vec![("r1", "R1")]), "").unwrap_err().contains("nada que mover"));
    // Una copia en marcha (un bloqueo reciente): no se toca nada.
    escribir(&du.join("r1/locks/abc"), b"lock");
    std::fs::File::options().write(true).open(du.join("r1/locks/abc")).unwrap().set_modified(SystemTime::now()).unwrap();
    let e = mover_en(&f, &mut c, &pedir(Some("PC"), vec![]), "").unwrap_err();
    assert!(e.contains("en marcha"), "{e}");
    assert!(du.join("r1/config").is_file() && c.users[0].name == UUID && c.renombres.is_empty());
    // Un bloqueo viejo (de una copia que se cortó) no frena.
    std::fs::File::options().write(true).open(du.join("r1/locks/abc")).unwrap().set_modified(SystemTime::now() - Duration::from_secs(3600)).unwrap();
    // Sin la línea del usuario en `.htpasswd`: falla y deshace lo que ya movió.
    f.write_htpasswd_de(None, &[htpasswd_line("otro", "x").unwrap()]).unwrap();
    let e = mover_en(&f, &mut c, &pedir(Some("PC"), vec![("r1", "Uno")]), "").unwrap_err();
    assert!(e.contains("No se ha cambiado nada"), "{e}");
    assert!(du.join("r1/config").is_file() && !du.join("uno").exists(), "deshecho");
    assert!(c.users[0].name == UUID && c.renombres.is_empty());
    // Sin la carpeta del almacén (un disco que no está): nada.
    c.path = t.0.join("no-esta").display().to_string();
    assert!(mover_en(&f, &mut c, &pedir(Some("PC"), vec![]), "").unwrap_err().contains("carpeta del almacén"));
}

/// Una carpeta de enlace no se mueve (Windows: una unión; Linux: un enlace simbólico).
#[test]
fn sin_enlaces_en_el_camino() {
    let t = tmp("enlaces");
    let (f, mut c) = almacen(&t.0);
    let base = PathBuf::from(&c.path);
    let fuera = t.0.join("fuera");
    repo(&fuera.join("r1"), 1);
    let enlace = base.join(UUID);
    #[cfg(unix)]
    let hecho = std::os::unix::fs::symlink(&fuera, &enlace).is_ok();
    #[cfg(windows)]
    let hecho = std::process::Command::new("cmd").args(["/C", "mklink", "/J"]).arg(&enlace).arg(&fuera).output().is_ok_and(|o| o.status.success());
    if !hecho {
        eprintln!("no se pudo crear el enlace: prueba saltada");
        return;
    }
    let p = Pedido { usuario: UUID.into(), legible: Some("PC".into()), repos: vec![], deshacer: false };
    assert!(mover_en(&f, &mut c, &p, "").unwrap_err().contains("enlace"));
    assert!(fuera.join("r1/config").is_file());
}

/// El espejo: lo movido no hace saltar el freno; la carpeta vieja se queda en el destino
/// para su retención, y la nueva se copia entera.
#[test]
fn el_espejo_no_frena_por_lo_movido() {
    let t = tmp("espejo");
    let (origen, destino) = (t.0.join("almacen"), t.0.join("espejo"));
    repo(&origen.join("u1/r1"), 30);
    repo(&origen.join("u2/r2"), 5);
    std::fs::create_dir_all(&destino).unwrap();
    let lado = Lado::Carpeta(&destino);
    let op = Opciones { retencion_dias: Some(30), freno: crate::espejo_motor::Freno { min_archivos: 0, ..Default::default() }, ..Default::default() };
    let mut est = Estado::default();
    crate::espejo_motor::vuelta(&origen, &lado, &Alcance::Todos, &op, &mut est, &mut |_, _| {}).unwrap();
    assert!(destino.join("u1/r1/data/029").is_file());
    // Se mueve u1/r1 → pc-01-3f2a/contabilidad en el almacén.
    std::fs::create_dir_all(origen.join("pc-01-3f2a")).unwrap();
    std::fs::rename(origen.join("u1/r1"), origen.join("pc-01-3f2a/contabilidad")).unwrap();
    // Sin saberlo, el freno salta (falta un repositorio entero).
    let mut sin = est.clone();
    let r = crate::espejo_motor::vuelta(&origen, &lado, &Alcance::Todos, &op, &mut sin, &mut |_, _| {}).unwrap();
    assert!(r.freno.as_deref().is_some_and(|f| f.contains("u1/r1 entero")), "{:?}", r.freno);
    // Sabiéndolo, no: la nueva se copia y la vieja espera su retención.
    let op2 = Opciones { antiguas: vec![("u1/r1".into(), "pc-01-3f2a/contabilidad".into())], ..op.clone() };
    let r = crate::espejo_motor::vuelta(&origen, &lado, &Alcance::Todos, &op2, &mut est, &mut |_, _| {}).unwrap();
    assert_eq!(r.freno, None);
    assert_eq!(r.por_borrar, 31, "config y 30 datos de la vieja");
    assert!(destino.join("pc-01-3f2a/contabilidad/data/029").is_file() && destino.join("u1/r1/config").is_file(), "la vieja no se borra al moverla");
    // Con selección: la ruta vieja de lo elegido se sigue mirando en el destino.
    let sel = Alcance::Repos(vec!["pc-01-3f2a/contabilidad".into()]);
    assert_eq!(sel.con_antiguas(&op2.antiguas), Alcance::Repos(vec!["pc-01-3f2a/contabilidad".into(), "u1/r1".into()]));
    let mut est_sel = Estado::default();
    let r = crate::espejo_motor::vuelta(&origen, &lado, &sel, &op2, &mut est_sel, &mut |_, _| {}).unwrap();
    assert_eq!((r.freno, r.por_borrar), (None, 31));
    // Lo movido dos veces (de un usuario entero y luego su repositorio) también se encuentra.
    let dos = vec![("u9".to_string(), "pc-01-3f2a".to_string()), ("pc-01-3f2a/x".to_string(), "pc-01-3f2a/y".to_string())];
    let a = Alcance::Repos(vec!["pc-01-3f2a/y".into()]).con_antiguas(&dos);
    assert_eq!(a, Alcance::Repos(vec!["pc-01-3f2a/y".into(), "u9/y".into(), "pc-01-3f2a/x".into(), "u9/x".into()]));
}

/// `nombre_repositorio`: el nombre que se ve, sin tocar el id ni la carpeta; y cómo
/// se describen las órdenes nuevas en el historial común.
#[test]
fn renombrar_un_repositorio_y_descripciones() {
    use crate::gestion_v2::{nombre_repositorio, RepoV2};
    let mut v = crate::servidor_v2::Vinculo {
        repos_v2: vec![RepoV2 {
            id: "almacen-01-3f2a".into(),
            nombre: "Copias en ALMACEN-01".into(),
            destino: "d".into(),
            contrasena: "x".repeat(12),
            ..Default::default()
        }],
        ..Default::default()
    };
    let m = nombre_repositorio(&mut v, &json!({ "repo": "almacen-01-3f2a", "nombre": "  Contabilidad  " })).unwrap();
    assert!(m.contains("«Copias en ALMACEN-01» ahora se llama «Contabilidad»"), "{m}");
    assert_eq!(
        (v.repos_v2[0].id.as_str(), v.repos_v2[0].nombre.as_str(), v.repos_v2[0].ubicacion_origen.as_deref()),
        ("almacen-01-3f2a", "Contabilidad", None)
    );
    assert!(nombre_repositorio(&mut v, &json!({ "repo": "almacen-01-3f2a", "nombre": "Contabilidad" })).unwrap().contains("ya se llama"));
    for mal in [
        json!({ "repo": "almacen-01-3f2a", "nombre": "" }),
        json!({ "repo": "almacen-01-3f2a", "nombre": "x".repeat(81) }),
        json!({ "repo": "almacen-01-3f2a", "nombre": "a\nb" }),
    ] {
        assert!(nombre_repositorio(&mut v, &mal).is_err(), "{mal}");
    }
    assert!(nombre_repositorio(&mut v, &json!({ "repo": "otro", "nombre": "X" })).unwrap_err().contains("no lo gestiona"));
    // Descripciones (sin rutas: solo nombres de carpeta del almacén, como antes).
    let orden = |tipo: &str, cuerpo: Value| -> resguardo_protocolo::orden_v2::OrdenV2 {
        serde_json::from_value(
            json!({ "v": 2, "cliente": "c", "equipo": "e", "seq": 1, "nonce": "n", "emitida": "", "caduca": "", "tipo": tipo, "cuerpo": cuerpo }),
        )
        .unwrap()
    };
    let d = |tipo: &str, cuerpo: Value| crate::espera_v2::descripcion(&v, &orden(tipo, cuerpo)).unwrap();
    assert_eq!(d("nombre_repositorio", json!({ "repo": "r", "nombre": "Ventas" })), "Cambiar el nombre de un repositorio a «Ventas»");
    assert_eq!(
        d("guarda_copias", json!({ "mover": { "usuario": "pc-01-3f2a", "deshacer": true } })),
        "Ordenar los nombres de carpeta de «pc-01-3f2a» en el almacén"
    );
    assert_eq!(
        d("retencion_almacen", json!({ "usuario": UUID, "repo": "r1", "nombre": "Contabilidad, de PC-01" })),
        "Cambiar la retención del almacén en «Contabilidad, de PC-01»"
    );
    assert_eq!(d("aplicar_retencion_almacen", json!({ "usuario": "u", "repo": "r1" })), "Aplicar la retención del almacén en u/r1", "sin nombre, como antes");
}

/// «Deshacer en el almacén»: la misma orden con `deshacer`, vuelve a los nombres
/// exactos de antes, y en `renombres` queda solo la vuelta (para el espejo).
#[test]
fn deshacer_con_la_orden() {
    let t = tmp("deshacer");
    let (f, mut c) = almacen(&t.0);
    let base = PathBuf::from(&c.path);
    repo(&base.join(UUID).join("almacen-01-3f2a"), 1);
    f.write_htpasswd_de(None, &[htpasswd_line(UUID, "x").unwrap()]).unwrap();
    let p = Pedido { usuario: UUID.into(), legible: Some("PC 01".into()), repos: vec![("almacen-01-3f2a".into(), "Contabilidad".into())], deshacer: false };
    let m = mover_en(&f, &mut c, &p, "2026-10-13T10:00:00-05:00").unwrap();
    let nuevo = m.usuario_final().to_string();
    assert!(leer_pedido(&json!({ "usuario": nuevo, "deshacer": true })).unwrap().deshacer);
    // Sin nada apuntado, no hay nada que deshacer.
    let otro = Pedido { usuario: nuevo.clone(), legible: None, repos: vec![], deshacer: true };
    let mut c2 = c.clone();
    c2.renombres.clear();
    assert!(mover_en(&f, &mut c2, &otro, "x").unwrap_err().contains("nada que deshacer"));
    let m2 = mover_en(&f, &mut c, &otro, "2026-10-13T10:05:00-05:00").unwrap();
    assert_eq!(m2.usuario, Some((nuevo.clone(), UUID.to_string())));
    assert_eq!(m2.repos, vec![("contabilidad".to_string(), "almacen-01-3f2a".to_string())]);
    assert!(base.join(UUID).join("almacen-01-3f2a/config").is_file() && !base.join(&nuevo).exists());
    assert!(f.read_htpasswd_de(None)[0].starts_with(&format!("{UUID}:")));
    assert!(c.users.iter().any(|u| u.name == UUID));
    // Solo la vuelta: de la carpeta legible a la de antes.
    assert_eq!(c.renombres.len(), 2, "{:?}", c.renombres);
    assert!(c.renombres.iter().all(|r| r.de.starts_with(&nuevo) && r.a.starts_with(UUID)));
}
