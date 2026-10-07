//! Pruebas de los trabajos de espejo (plan 0.7.26, bloque 4).

use super::*;
use crate::espejo_motor::{retencion, Archivo, Estado, Resumen};
use chrono::TimeZone;
use serde_json::json;
use std::collections::BTreeMap;

/// Los vectores compartidos con la consola (crates/protocolo/vectors/espejo-trabajos.json).
#[test]
fn vectores_compartidos_con_la_consola() {
    let doc: serde_json::Value = serde_json::from_str(include_str!("../../../protocolo/vectors/espejo-trabajos.json")).unwrap();
    let con = |cambios: &serde_json::Value| -> serde_json::Value {
        let mut t = doc["base"].clone();
        for (k, v) in cambios.as_object().unwrap() {
            t[k] = v.clone();
        }
        t
    };
    let lista = |l: &serde_json::Value| -> Vec<serde_json::Value> { l.as_array().unwrap().iter().map(con).collect() };
    for p in doc["pedidos"].as_array().unwrap() {
        let r = leer_pedido(&json!({ "trabajos": lista(&p["cambios"]) }), p["quien"].as_str().unwrap());
        assert_eq!(r.is_ok(), p["ok"].as_bool().unwrap(), "{}: {:?}", p["nombre"], r.err());
    }
    for c in doc["reduce"].as_array().unwrap() {
        let de = |k: &str| -> Vec<Trabajo> { lista(&c[k]).into_iter().map(|t| serde_json::from_value(t).unwrap()).collect() };
        assert_eq!(reduce(&de("antes"), &de("ahora")), c["reduce"].as_bool().unwrap(), "{}", c["nombre"]);
    }
}

fn trabajo(id: &str, adonde: Adonde) -> Trabajo {
    Trabajo {
        id: id.into(),
        nombre: format!("Espejo {id}"),
        activo: true,
        quien: QUIEN_ALMACEN.into(),
        adonde,
        cuando: Cuando { horario: Some(horario_diario("02:00")), ..Default::default() },
        ..Default::default()
    }
}
fn carpeta(c: &str) -> Adonde {
    Adonde { tipo: "carpeta".into(), carpeta: c.into(), nube: None }
}
fn nube(n: &str, c: &str) -> Adonde {
    Adonde { tipo: "nube".into(), carpeta: c.into(), nube: Some(n.into()) }
}
fn hora(d: u32, h: u32, m: u32) -> chrono::DateTime<chrono::Local> {
    chrono::Local.with_ymd_and_hms(2026, 10, d, h, m, 0).unwrap()
}
fn base(nombre: &str) -> PathBuf {
    let b = std::env::temp_dir().join(format!("resguardo-trabajos-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&b);
    b
}
/// Un archivo con fecha antigua (el espejo deja lo reciente para la vuelta siguiente).
fn escribir(p: &Path, contenido: &[u8]) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, contenido).unwrap();
    std::fs::File::options().write(true).open(p).unwrap().set_modified(SystemTime::now() - Duration::from_secs(3600)).unwrap();
}
fn envejecer(d: &Path) {
    for e in std::fs::read_dir(d).unwrap().flatten() {
        if e.file_type().unwrap().is_dir() {
            envejecer(&e.path());
        } else {
            std::fs::File::options().write(true).open(e.path()).unwrap().set_modified(SystemTime::now() - Duration::from_secs(3600)).unwrap();
        }
    }
}

/// Los espejos de antes (por destinos) se leen como trabajos equivalentes, con el
/// mismo archivo de estado, y se ven igual desde una consola o un agente anteriores.
#[test]
fn conversion_de_los_espejos_de_antes() {
    let e = crate::espejo::pedido(&json!({ "hora": "03:30", "limite_kib": 512, "destinos": [
        { "tipo": "carpeta", "carpeta": "E:\\Espejo", "retencion_dias": 30, "verificar_pct": 10 },
        { "tipo": "nube", "nube": "Dropbox Sur", "carpeta": "Resguardo/Sur", "tras_copia": true, "repos": ["ana/conta"], "vistos": ["ana/conta", "srv"] },
        { "tipo": "zona", "carpeta": "principal", "zona": "z0e0e0e", "bloqueo": true,
          "horario": { "dias": [1, 2, 3, 4, 5], "horas": ["21:00"] } },
    ] }))
    .unwrap()
    .unwrap();
    let ts = e.trabajos_efectivos();
    assert_eq!(ts.len(), 3);
    let (disco, dropbox, zona) = (&ts[0], &ts[1], &ts[2]);
    // Lo mismo que hacía cada destino.
    assert_eq!((disco.que.clone(), disco.retencion.clone(), disco.verificar_pct), (Que::Todos, Retencion::Retraso { dias: 30 }, Some(10)));
    assert_eq!(disco.cuando.horario, Some(horario_diario("03:30")), "sin horario propio: cada día a `hora`");
    assert_eq!((disco.limite_kib, dropbox.limite_kib), (None, Some(512)), "el límite, solo hacia la nube");
    assert!(dropbox.cuando.tras_copia && dropbox.que == Que::Repos { repos: vec!["ana/conta".into()] } && dropbox.vistos == vec!["ana/conta", "srv"]);
    assert_eq!((zona.zona.as_deref(), zona.bloqueo, zona.retencion.clone()), (Some("z0e0e0e"), true, Retencion::Nunca));
    assert_eq!(zona.cuando.horario.as_ref().unwrap().horas, vec!["21:00"]);
    assert_eq!(ts.iter().map(|t| t.orden).collect::<Vec<_>>(), vec![0, 1, 2]);
    assert!(ts.iter().all(|t| t.activo && t.quien == QUIEN_ALMACEN && t.freno == Freno::default()));
    // El mismo archivo de estado que tenía cada destino (lo anotado sigue valiendo).
    for (t, d) in ts.iter().zip(e.destinos()) {
        assert_eq!(t.id, id_legado(&d));
        assert_eq!(archivo_estado(t, None).file_name().unwrap().to_string_lossy(), format!("espejo-{}.json", t.id));
        // Y la vista de antes es la misma.
        let v = a_destino(t);
        assert!(v.mismo(&d));
        assert_eq!((v.repos.clone(), v.retencion_dias, v.bloqueo, v.tras_copia), (d.repos.clone(), d.retencion_dias, d.bloqueo, d.tras_copia));
        assert!(exacto(t, &ts, e.limite_kib), "{}", t.nombre);
    }
    // Una carpeta larga: el nombre por defecto se queda en 80 letras (el final) y vale.
    let larga = format!("E:\\{}\\Espejo", "carpeta-muy-larga\\".repeat(8));
    let mut t = de_destino(&crate::espejo::Destino { tipo: "carpeta".into(), carpeta: larga.clone(), ..Default::default() }, "02:00", None, 0);
    assert!(t.nombre.chars().count() == 80 && t.nombre.starts_with('…') && t.nombre.ends_with("\\Espejo"));
    validar(&mut t, QUIEN_ALMACEN).unwrap();
    assert!(exacto(&t, &[t.clone()], None));
    // La forma de 0.7.0 (una carpeta) también.
    let viejo = crate::espejo::Espejo { carpeta: "E:\\x".into(), hora: "02:00".into(), ..Default::default() };
    assert_eq!(viejo.trabajos_efectivos()[0].adonde, carpeta("E:\\x"));
    // Guardados como trabajos: la vista compatible (destinos, hora y límite) para quien no los entiende.
    let g = crate::espejo::Espejo::de_trabajos(ts.clone());
    assert_eq!((g.destinos.len(), g.hora.as_str(), g.limite_kib), (3, "03:30", Some(512)));
    assert_eq!(g.trabajos_efectivos(), ts, "con trabajos, mandan ellos");
    let r = g.resumen();
    assert_eq!(r["trabajos"].as_array().unwrap().len(), 3);
    assert_eq!(r["destinos"][1]["trabajo"], ts[1].id.as_str());
    assert_eq!(r["trabajos"][0]["retencion"], json!({ "modo": "retraso", "dias": 30 }));
    assert_eq!(r["trabajos"][2]["zona"], "z0e0e0e");
    // Desde un agente anterior (vuelta atrás): lee `destinos` y no ve `trabajos`.
    let texto = serde_json::to_string(&g).unwrap();
    let antiguo: serde_json::Value = serde_json::from_str(&texto).unwrap();
    assert_eq!(antiguo["destinos"].as_array().unwrap().len(), 3);
    // Lo que no se puede decir igual: lo más parecido sin borrar antes.
    let mut igual = ts[0].clone();
    igual.retencion = Retencion::Igual;
    assert_eq!(a_destino(&igual).retencion_dias, Some(crate::espejo_motor::RETENCION_MIN));
    assert!(!exacto(&igual, &[igual.clone()], None));
    let mut cadena = ts[1].clone();
    cadena.cuando = Cuando { cadena: Some(ts[0].id.clone()), ..Default::default() };
    assert!(a_destino(&cadena).tras_copia && !exacto(&cadena, &ts, Some(512)));
    let mut pausado = ts[0].clone();
    pausado.activo = false;
    let g2 = crate::espejo::Espejo::de_trabajos(vec![pausado.clone(), ts[1].clone()]);
    assert_eq!(g2.destinos.len(), 1, "un agente anterior no hace los pausados");
    assert_eq!(g2.destinos().len(), 2, "pero cuentan para saber qué se usa");
    // Una orden de antes (por destinos) sobre trabajos que se pueden decir igual: cada uno sigue siendo el mismo.
    let mut nuevos = crate::espejo::pedido(&json!({ "destinos": [{ "tipo": "carpeta", "carpeta": "E:\\Espejo", "retencion_dias": 60 }] }))
        .unwrap()
        .unwrap()
        .trabajos_efectivos();
    let mut con_otro_id = ts.clone();
    con_otro_id[0].id = "t1a2b3".into();
    mapear_ids(&mut nuevos, &con_otro_id);
    assert_eq!(nuevos[0].id, "t1a2b3");
    assert!(reduce(&ts, &nuevos), "quita destinos");
}

/// Lo que manda la consola se comprueba: quién, qué, adónde, cuándo, retención, bloqueo y freno.
#[test]
fn pedidos_de_trabajos() {
    let uno = |extra: serde_json::Value| {
        let mut t = json!({ "id": "t1", "nombre": "Disco E", "quien": "almacen", "que": { "tipo": "todos" },
            "adonde": { "tipo": "carpeta", "carpeta": "E:\\Espejo" }, "cuando": { "tras_copia": true }, "retencion": { "modo": "nunca" } });
        for (k, v) in extra.as_object().unwrap() {
            t[k] = v.clone();
        }
        leer_pedido(&json!({ "trabajos": [t] }), QUIEN_ALMACEN)
    };
    let t = uno(json!({})).unwrap();
    assert_eq!((t[0].freno.pct, t[0].freno.min_archivos, t[0].freno.min_faltan, t[0].freno.accion), (10, 100, 20, AccionFreno::Confirmar));
    assert!(t[0].activo && t[0].estado == EstadoTrabajo::default());
    // Lo que recuerda el equipo no lo dice la consola.
    assert_eq!(uno(json!({ "estado": { "resultado": "inventado" } })).unwrap()[0].estado.resultado, None);
    for (mal, por) in [
        (json!({ "quien": "equipo" }), "quién"),
        (json!({ "id": "../x" }), "id"),
        (json!({ "que": { "tipo": "repos", "repos": [] } }), "selección vacía"),
        (json!({ "que": { "tipo": "repos", "repos": ["a/b/c"] } }), "repositorio"),
        (json!({ "que": { "tipo": "equipos", "equipos": ["ana/conta"] } }), "equipo con barra"),
        (json!({ "adonde": { "tipo": "ftp", "carpeta": "x" } }), "adónde"),
        (json!({ "adonde": { "tipo": "nube", "carpeta": "x" } }), "nube sin nombre"),
        (json!({ "adonde": { "tipo": "zona", "carpeta": "principal" } }), "la misma zona"),
        (json!({ "cuando": {} }), "sin cuándo"),
        (json!({ "cuando": { "cadena": "t1" } }), "después de sí mismo"),
        (json!({ "cuando": { "tras_copia": true, "retraso_min": 5000 } }), "retraso"),
        (json!({ "cuando": { "horario": { "dias": [9], "horas": ["02:00"] } } }), "horario"),
        (json!({ "retencion": { "modo": "retraso", "dias": 0 } }), "días"),
        (json!({ "retencion": { "modo": "borrar_todo" } }), "modo"),
        (json!({ "retencion": { "modo": "igual" }, "bloqueo": true }), "bloqueo sin plazo"),
        (json!({ "retencion": { "modo": "igual" }, "bloqueo_dias": 30 }), "igual con bloqueo"),
        (json!({ "retencion": { "modo": "retraso", "dias": 30 }, "bloqueo_dias": 30 }), "retraso = bloqueo"),
        (json!({ "freno": { "pct": 0 } }), "freno apagado"),
        (json!({ "freno": { "pct": 60 } }), "freno de más del 50 %"),
        (json!({ "freno": { "accion": "nada" } }), "acción"),
        (json!({ "verificar_pct": 101 }), "verificación"),
    ] {
        assert!(uno(mal.clone()).is_err(), "{por}: {mal}");
    }
    // Con bloqueo de objetos de 30 días, un retraso de 31 sí.
    let b = uno(json!({ "retencion": { "modo": "retraso", "dias": 31 }, "bloqueo_dias": 30 })).unwrap();
    assert_eq!(b[0].opciones().bloqueo_dias, Some(30));
    // Un espejo del propio equipo: repositorios por id, carpeta o nube (no zonas ni equipos).
    let eq = |t: serde_json::Value| leer_pedido(&json!({ "trabajos": [t] }), QUIEN_EQUIPO);
    let bien = json!({ "id": "e1", "quien": "equipo", "que": { "tipo": "repos", "repos": ["documentos"] }, "adonde": { "tipo": "carpeta", "carpeta": "F:\\Espejo" }, "cuando": { "tras_copia": true } });
    assert_eq!(eq(bien.clone()).unwrap()[0].nombre, "Espejo a F:\\Espejo", "sin nombre, uno por defecto");
    let mut zona = bien.clone();
    zona["adonde"] = json!({ "tipo": "zona", "carpeta": "z0e0e0e" });
    let mut equipos = bien.clone();
    equipos["que"] = json!({ "tipo": "equipos", "equipos": ["ana"] });
    let mut almacen = bien.clone();
    almacen["quien"] = json!("almacen");
    for mal in [zona, equipos, almacen] {
        assert!(eq(mal.clone()).is_err(), "{mal}");
    }
    // Ids repetidos, «después de» uno que no está y círculos.
    let dos = |a: serde_json::Value, b: serde_json::Value| leer_pedido(&json!({ "trabajos": [a, b] }), QUIEN_ALMACEN);
    let t = |id: &str, cuando: serde_json::Value| json!({ "id": id, "adonde": { "tipo": "carpeta", "carpeta": format!("E:\\{id}") }, "cuando": cuando });
    assert!(dos(t("a", json!({ "tras_copia": true })), t("a", json!({ "tras_copia": true }))).is_err());
    assert!(dos(t("a", json!({ "tras_copia": true })), t("b", json!({ "cadena": "zz" }))).is_err());
    assert!(dos(t("a", json!({ "despues": "b" })), t("b", json!({ "cadena": "a" }))).unwrap_err().contains("círculo"));
    assert!(dos(t("a", json!({ "tras_copia": true })), t("b", json!({ "cadena": "a" }))).is_ok());
}

/// Varios trabajos por repositorio: a destinos u horas distintas; al mismo destino, con la misma retención.
#[test]
fn varios_trabajos_por_repositorio() {
    let solo = Que::Repos { repos: vec!["ana/conta".into()] };
    let mut cada_hora = trabajo("h", carpeta("E:\\Espejo"));
    cada_hora.que = solo.clone();
    cada_hora.cuando.horario = Some(Horario {
        dias: vec![],
        horas: vec![],
        reglas: vec![crate::gestion_v2::Regla::Intervalo { dias: vec![1, 2, 3, 4, 5], cada_min: 60, desde: "08:00".into(), hasta: "18:00".into() }],
    });
    let mut noche = trabajo("n", nube("Dropbox Sur", "Sur"));
    noche.que = solo.clone();
    noche.retencion = Retencion::Retraso { dias: 30 };
    let mut todos_noche = trabajo("t", carpeta("E:\\Espejo"));
    todos_noche.cuando.horario = Some(horario_diario("23:00"));
    let ts = vec![cada_hora.clone(), noche.clone(), todos_noche.clone()];
    validar_conjunto(&ts).unwrap();
    // A la misma carpeta con otra retención, no: uno borraría lo que el otro guarda.
    let mut otro = todos_noche.clone();
    otro.retencion = Retencion::Igual;
    assert!(validar_conjunto(&[cada_hora.clone(), otro]).unwrap_err().contains("misma retención"));
    // Cada uno toca a su hora (viernes 2 de octubre de 2026).
    let hecho = |t: &Trabajo, d: u32, h: u32| Trabajo { estado: EstadoTrabajo { inicio: Some(hora(d, h, 0).to_rfc3339()), ..Default::default() }, ..t.clone() };
    assert_eq!(toca(&hecho(&cada_hora, 2, 9), &ts, hora(2, 10, 0), None), Some(Motivo::Horario));
    assert_eq!(toca(&hecho(&todos_noche, 1, 23), &ts, hora(2, 10, 0), None), None);
    assert_eq!(toca(&hecho(&todos_noche, 1, 23), &ts, hora(2, 23, 1), None), Some(Motivo::Horario));
    assert_eq!(toca(&hecho(&noche, 2, 2), &ts, hora(2, 23, 1), None), None);
    // Cada uno con su archivo de estado (lo que falta, la rotación), aunque vayan al mismo sitio.
    assert_ne!(archivo_estado(&cada_hora, None), archivo_estado(&todos_noche, None));
    // Uno pausado no toca nunca.
    assert_eq!(toca(&Trabajo { activo: false, ..cada_hora.clone() }, &ts, hora(2, 10, 0), None), None);
}

/// Espejos en cadena (solo si el anterior salió bien) y «después de» (siempre), con retraso.
#[test]
fn espejos_en_cadena_y_despues() {
    let mut a = trabajo("a", carpeta("E:\\A"));
    let mut b = trabajo("b", carpeta("E:\\B"));
    b.cuando = Cuando { cadena: Some("a".into()), retraso_min: 10, ..Default::default() };
    let mut c = trabajo("c", nube("Dropbox Sur", "C"));
    c.cuando = Cuando { despues: Some("a".into()), ..Default::default() };
    let fin = |t: &mut Trabajo, h: u32, m: u32, ok: bool| {
        t.estado.ultima = Some(hora(2, h, m).to_rfc3339());
        t.estado.resultado = Some(if ok { "Espejo hecho".into() } else { "ERROR: el disco no está".into() });
    };
    // A no ha hecho nada todavía: ni B ni C.
    let ts = |a: &Trabajo, b: &Trabajo, c: &Trabajo| vec![a.clone(), b.clone(), c.clone()];
    assert_eq!(toca(&b, &ts(&a, &b, &c), hora(2, 3, 0), None), None);
    // A sale bien a las 02:05: C enseguida; B, a los 10 minutos.
    fin(&mut a, 2, 5, true);
    assert_eq!(toca(&c, &ts(&a, &b, &c), hora(2, 2, 6), None), Some(Motivo::TrasOtro));
    assert_eq!(toca(&b, &ts(&a, &b, &c), hora(2, 2, 10), None), None, "con su retraso");
    assert_eq!(toca(&b, &ts(&a, &b, &c), hora(2, 2, 15), None), Some(Motivo::TrasOtro));
    // Hechos: no vuelven a tocar hasta que A termine otra vez.
    b.estado.inicio = Some(hora(2, 2, 15).to_rfc3339());
    c.estado.inicio = Some(hora(2, 2, 6).to_rfc3339());
    assert_eq!(toca(&b, &ts(&a, &b, &c), hora(2, 2, 20), None), None);
    assert_eq!(toca(&c, &ts(&a, &b, &c), hora(2, 2, 20), None), None);
    // A falla la noche siguiente: «después de» (C) va igual; «en cadena» (B), no.
    fin(&mut a, 2, 5, false);
    a.estado.ultima = Some(hora(3, 2, 5).to_rfc3339());
    assert_eq!(toca(&c, &ts(&a, &b, &c), hora(3, 2, 30), None), Some(Motivo::TrasOtro));
    assert_eq!(toca(&b, &ts(&a, &b, &c), hora(3, 3, 0), None), None);
    // B sin horario: solo en cadena.
    assert!(b.plan().is_none());
}

/// «Después de cada copia nueva» con su retraso (como mínimo los 12 min de siempre).
#[test]
fn tras_cada_copia_con_retraso() {
    let mut t = trabajo("t", carpeta("E:\\T"));
    t.cuando = Cuando { tras_copia: true, retraso_min: 30, ..Default::default() };
    t.estado.inicio = Some(hora(2, 2, 0).to_rfc3339());
    let s = |h: u32, m: u32| SystemTime::from(hora(2, h, m));
    let nueva = Some((s(11, 55), s(11, 55)));
    assert_eq!(toca(&t, &[], hora(2, 12, 10), nueva), None, "espera 30 min");
    assert_eq!(toca(&t, &[], hora(2, 12, 26), nueva), Some(Motivo::TrasCopia));
    t.cuando.retraso_min = 0;
    assert_eq!(toca(&t, &[], hora(2, 12, 0), nueva), None, "como mínimo, 12 min");
    assert_eq!(toca(&t, &[], hora(2, 12, 7), nueva), Some(Motivo::TrasCopia));
    // «Hacer ahora»: toca ya (salvo en pausa).
    t.estado.pedido_ahora = true;
    assert_eq!(toca(&t, &[], hora(2, 12, 0), None), Some(Motivo::Ahora));
    t.activo = false;
    assert_eq!(toca(&t, &[], hora(2, 12, 0), None), None);
}

/// Las tres retenciones con fechas simuladas: nunca borra, con retraso de N días e igual que el origen.
#[test]
fn las_tres_retenciones() {
    let b = base("retenciones");
    let d = b.join("destino");
    let mut origen: Vec<Archivo> = Vec::new();
    let mut destino = BTreeMap::new();
    for i in 0..200 {
        let rel = if i == 0 { "ana/r/config".to_string() } else { format!("ana/r/data/{i:03}") };
        escribir(&d.join(&rel), b"x");
        destino.insert(rel.clone(), 1u64);
        origen.push(Archivo { rel, len: 1, reciente: false });
    }
    let listar =
        || -> BTreeMap<String, u64> { crate::espejo_motor::listar_carpeta(&d, &Alcance::Todos).unwrap().into_iter().map(|a| (a.rel, a.len)).collect() };
    let dia = |n: i64| chrono::NaiveDate::from_ymd_opt(2026, 10, 1).unwrap() + chrono::Duration::days(n);
    let lado = Lado::Carpeta(&d);
    // La poda del almacén quita 3 archivos.
    let podados: Vec<String> = (1..=3).map(|i| format!("ana/r/data/{i:03}")).collect();
    origen.retain(|a| !podados.contains(&a.rel));
    let pasar = |t: &Trabajo, est: &mut Estado, n: i64| {
        let mut r = Resumen::default();
        retencion(&lado, &origen, &listar(), &t.opciones(), est, &mut r, dia(n)).unwrap();
        r
    };
    let mut t = trabajo("t", carpeta(&d.display().to_string()));
    // 1. Nunca borra: ni se anota.
    let mut est = Estado::default();
    for n in [0, 400] {
        let r = pasar(&t, &mut est, n);
        assert_eq!((r.borrados, r.por_borrar), (0, 0));
    }
    assert!(podados.iter().all(|p| d.join(p).is_file()));
    // 2. Con retraso de 30 días: se anota hoy y se borra el día 30, no antes.
    t.retencion = Retencion::Retraso { dias: 30 };
    let mut est = Estado::default();
    let r = pasar(&t, &mut est, 0);
    assert_eq!((r.borrados, r.por_borrar, r.primer_borrado.as_deref()), (0, 3, Some("2026-10-31")));
    assert_eq!(pasar(&t, &mut est, 29).borrados, 0);
    assert!(d.join(&podados[0]).is_file());
    assert_eq!(pasar(&t, &mut est, 30).borrados, 3);
    assert!(podados.iter().all(|p| !d.join(p).exists()));
    // 3. Igual que el origen: se ve faltar en una vuelta y se borra en la siguiente (aunque sea el mismo día).
    for p in &podados {
        escribir(&d.join(p), b"x");
    }
    t.retencion = Retencion::Igual;
    let mut est = Estado::default();
    let r = pasar(&t, &mut est, 0);
    assert_eq!((r.borrados, r.por_borrar), (0, 3), "la primera vez solo se anota");
    assert!(d.join(&podados[0]).is_file());
    let r = pasar(&t, &mut est, 0);
    assert_eq!((r.borrados, r.por_borrar), (3, 0), "en la vuelta siguiente, fuera");
    assert!(podados.iter().all(|p| !d.join(p).exists()));
    let _ = std::fs::remove_dir_all(&b);
}

/// El freno: el % solo cuenta por encima de los mínimos; un repositorio entero frena siempre;
/// «avisar» conserva lo que falta y sigue; «confirmar» para hasta que se confirma.
#[test]
fn freno_con_minimos_y_repositorio_entero() {
    let f = Freno::default();
    // 30 de 1000 que faltan de golpe, con 970 en el origen: un 3 %, no salta.
    assert!(!f.salta(30, 1000, 970));
    // 150 de 1000 (15 %): salta.
    assert!(f.salta(150, 1000, 850));
    // Repositorio pequeño (100 archivos o menos en el origen): el % no cuenta.
    assert!(!f.salta(50, 150, 100));
    // Pocos archivos (menos de 20) aunque sea mucho %: no.
    assert!(!f.salta(19, 120, 101));
    // «Más de» el %: justo el 10 % no salta.
    assert!(!f.salta(100, 1000, 900) && f.salta(101, 1000, 899));
    let a_medida = Freno { pct: 5, min_archivos: 10, min_faltan: 2, accion: AccionFreno::Avisar };
    assert!(a_medida.salta(2, 30, 28));
    assert!(Freno { pct: 0, ..f.clone() }.validar().is_err() && Freno { pct: 51, ..f.clone() }.validar().is_err() && f.validar().is_ok());

    let b = base("freno");
    let d = b.join("destino");
    let mut destino = BTreeMap::new();
    let mut origen: Vec<Archivo> = Vec::new();
    // Dos repositorios: uno grande (300 archivos) y uno pequeño (5).
    for (repo, n) in [("ana/grande", 300), ("srv/chico", 5)] {
        for i in 0..n {
            let rel = if i == 0 { format!("{repo}/config") } else { format!("{repo}/data/{i:03}") };
            escribir(&d.join(&rel), b"x");
            destino.insert(rel.clone(), 1u64);
            origen.push(Archivo { rel, len: 1, reciente: false });
        }
    }
    let listar =
        || -> BTreeMap<String, u64> { crate::espejo_motor::listar_carpeta(&d, &Alcance::Todos).unwrap().into_iter().map(|a| (a.rel, a.len)).collect() };
    let dia = |n: i64| chrono::NaiveDate::from_ymd_opt(2026, 10, 1).unwrap() + chrono::Duration::days(n);
    let lado = Lado::Carpeta(&d);
    let mut t = trabajo("t", carpeta(&d.display().to_string()));
    t.retencion = Retencion::Igual;
    let pasar = |t: &Trabajo, origen: &[Archivo], est: &mut Estado| {
        let mut r = Resumen::default();
        retencion(&lado, origen, &listar(), &t.opciones(), est, &mut r, dia(0)).unwrap();
        r
    };
    // El repositorio pequeño, casi entero (4 de 5): sin su config sigue ahí, y el % no cuenta en él… pero
    // cuenta lo que hay en todo el origen (más de 100): 4 de 305 no es mucho. Se anota.
    let mut o1 = origen.clone();
    o1.retain(|a| !a.rel.starts_with("srv/chico/data/"));
    let mut est = Estado::default();
    let r = pasar(&t, &o1, &mut est);
    assert_eq!((r.freno.clone(), r.por_borrar), (None, 4));
    // El repositorio pequeño entero (su config): frena aunque sean 5 archivos.
    let mut o2 = origen.clone();
    o2.retain(|a| !a.rel.starts_with("srv/chico/"));
    let mut est = Estado::default();
    let r = pasar(&t, &o2, &mut est);
    assert!(r.freno.as_deref().is_some_and(|f| f.contains("srv/chico entero")), "{:?}", r.freno);
    assert_eq!((r.por_borrar, r.borrados), (0, 0));
    // Confirmar: frena hasta que se confirma (y sigue sin borrar en las vueltas siguientes).
    assert!(pasar(&t, &o2, &mut est).freno.is_some());
    assert!(d.join("srv/chico/config").is_file());
    est.aceptar_freno = true;
    let r = pasar(&t, &o2, &mut est);
    assert_eq!((r.freno.clone(), r.por_borrar), (None, 5));
    assert_eq!(pasar(&t, &o2, &mut est).borrados, 5, "igual que el origen: en la vuelta siguiente");
    assert!(!d.join("srv/chico/config").exists());
    // Avisar: falta de golpe un 40 % del grande; se conserva, se avisa y lo demás sigue.
    t.freno.accion = AccionFreno::Avisar;
    let mut o3 = origen.clone();
    let quitados: Vec<String> = (100..220).map(|i| format!("ana/grande/data/{i:03}")).collect();
    o3.retain(|a| !quitados.contains(&a.rel) && !a.rel.starts_with("srv/chico/"));
    let mut est = Estado::default();
    let r = pasar(&t, &o3, &mut est);
    assert!(r.freno.as_deref().is_some_and(|f| f.contains("se conserva")), "{:?}", r.freno);
    assert_eq!((r.retenidos, r.por_borrar, r.borrados), (120, 0, 0));
    // La vuelta siguiente no vuelve a frenar por lo mismo, y no lo borra.
    let r = pasar(&t, &o3, &mut est);
    assert_eq!((r.freno.clone(), r.retenidos, r.borrados), (None, 120, 0));
    assert!(quitados.iter().all(|q| d.join(q).is_file()));
    // Lo que falta después, poco a poco, sí se sigue (y se borra).
    let mut o4 = o3.clone();
    o4.retain(|a| a.rel != "ana/grande/data/001");
    pasar(&t, &o4, &mut est);
    assert_eq!(pasar(&t, &o4, &mut est).borrados, 1);
    // Confirmado: lo conservado también se borra (en la vuelta siguiente).
    est.aceptar_freno = true;
    let r = pasar(&t, &o4, &mut est);
    assert_eq!((r.retenidos, r.por_borrar), (0, 120));
    assert_eq!(pasar(&t, &o4, &mut est).borrados, 120);
    let _ = std::fs::remove_dir_all(&b);
}

/// Bloqueo de objetos simulado: sin plazo, nunca se borra; con N días, solo con un retraso de más de N
/// (y el motor, aunque le llegue otra cosa, no borra antes).
#[test]
fn bloqueo_de_objetos() {
    let b = base("bloqueo");
    let d = b.join("destino");
    let mut origen: Vec<Archivo> = Vec::new();
    for i in 0..10 {
        let rel = format!("ana/r/data/{i:03}");
        escribir(&d.join(&rel), b"x");
        origen.push(Archivo { rel, len: 1, reciente: false });
    }
    escribir(&d.join("ana/r/config"), b"c");
    origen.push(Archivo { rel: "ana/r/config".into(), len: 1, reciente: false });
    origen.retain(|a| a.rel != "ana/r/data/000");
    let listar =
        || -> BTreeMap<String, u64> { crate::espejo_motor::listar_carpeta(&d, &Alcance::Todos).unwrap().into_iter().map(|a| (a.rel, a.len)).collect() };
    let dia = |n: i64| chrono::NaiveDate::from_ymd_opt(2026, 10, 1).unwrap() + chrono::Duration::days(n);
    let lado = Lado::Carpeta(&d);
    let pasar = |op: &Opciones, est: &mut Estado, n: i64| {
        let mut r = Resumen::default();
        retencion(&lado, &origen, &listar(), op, est, &mut r, dia(n)).unwrap();
        r
    };
    let mut t = trabajo("t", nube("B2 Oficina", "espejo"));
    // Sin plazo: nunca.
    t.bloqueo = true;
    let mut op = t.opciones();
    op.retencion_dias = Some(1);
    let mut est = Estado::default();
    assert_eq!((pasar(&op, &mut est, 0).por_borrar, pasar(&op, &mut est, 500).borrados), (0, 0));
    // 30 días de bloqueo: lo que llegue con 30 o menos, o «igual», no borra nunca.
    t.bloqueo = false;
    t.bloqueo_dias = Some(30);
    for (dias, igual) in [(Some(30), false), (Some(10), false), (None, true)] {
        let op = Opciones { retencion_dias: dias, igual, ..t.opciones() };
        let mut est = Estado::default();
        pasar(&op, &mut est, 0);
        assert_eq!(pasar(&op, &mut est, 400).borrados, 0, "{dias:?} {igual}");
    }
    // Con 31 días: lo que lleva 31 días faltando (ya fuera del bloqueo) se borra.
    t.retencion = Retencion::Retraso { dias: 31 };
    let mut est = Estado::default();
    pasar(&t.opciones(), &mut est, 0);
    assert_eq!(pasar(&t.opciones(), &mut est, 30).borrados, 0);
    assert_eq!(pasar(&t.opciones(), &mut est, 31).borrados, 1);
    // Y en la vista de antes, un destino con bloqueo que nunca borra.
    t.retencion = Retencion::Nunca;
    assert!(a_destino(&t).bloqueo);
    let _ = std::fs::remove_dir_all(&b);
}

/// Qué reduce la protección (y espera).
#[test]
fn que_reduce_la_proteccion() {
    let a = trabajo("a", carpeta("E:\\A"));
    let con = |f: &dyn Fn(&mut Trabajo)| {
        let mut x = a.clone();
        f(&mut x);
        vec![x]
    };
    let antes = vec![a.clone()];
    assert!(!reduce(&antes, &antes));
    assert!(reduce(&antes, &[]), "quitarlo");
    assert!(reduce(&antes, &con(&|x| x.activo = false)), "pausarlo");
    assert!(reduce(&antes, &con(&|x| x.adonde.carpeta = "E:\\B".into())), "otro destino");
    assert!(reduce(&antes, &con(&|x| x.que = Que::Repos { repos: vec!["a/b".into()] })), "de todos a algunos");
    assert!(reduce(&antes, &con(&|x| x.retencion = Retencion::Retraso { dias: 30 })), "empezar a borrar");
    assert!(reduce(&con(&|x| x.retencion = Retencion::Retraso { dias: 30 }), &con(&|x| x.retencion = Retencion::Igual)), "borrar antes");
    assert!(!reduce(&con(&|x| x.retencion = Retencion::Igual), &con(&|x| x.retencion = Retencion::Retraso { dias: 30 })), "borrar después no");
    assert!(reduce(&con(&|x| x.bloqueo_dias = Some(30)), &antes), "quitar el bloqueo");
    assert!(reduce(&antes, &con(&|x| x.freno.pct = 20)), "aflojar el freno");
    assert!(reduce(&antes, &con(&|x| x.freno.min_archivos = 1000)));
    assert!(reduce(&antes, &con(&|x| x.freno.accion = AccionFreno::Avisar)));
    assert!(!reduce(&antes, &con(&|x| x.freno.pct = 5)), "apretarlo no");
    assert!(!reduce(&antes, &con(&|x| x.cuando.horario = Some(horario_diario("05:00")))), "cambiar la hora no");
    assert!(!reduce(&antes, &[a.clone(), trabajo("b", carpeta("E:\\B"))]), "añadir uno no");
    let pausado = con(&|x| x.activo = false);
    assert!(!reduce(&pausado, &[]), "quitar uno pausado no");
    let repos = |l: &[&str]| con(&|x| x.que = Que::Repos { repos: l.iter().map(|s| s.to_string()).collect() });
    assert!(reduce(&repos(&["a/x", "a/y"]), &repos(&["a/x"])) && !reduce(&repos(&["a/x"]), &repos(&["a/x", "a/y"])));
}

/// El almacén hace un trabajo «igual que el origen» a una carpeta (de verdad, con el
/// servicio): copia, se anota el resultado en su trabajo y lo podado se borra a la vuelta siguiente.
#[test]
fn almacen_igual_que_el_origen() {
    let _l = crate::restic::tests::real_repo_lock();
    let b = base("almacen");
    std::fs::create_dir_all(b.join("agente/privado")).unwrap();
    std::env::set_var("RESGUARDO_AGENT_DIR", b.join("agente"));
    let (almacen, espejo) = (b.join("almacen"), b.join("espejo"));
    for f in ["ana/conta/config", "ana/conta/data/ab/abcd", "ana/conta/data/cd/cdef", "srv/sql/config"] {
        escribir(&almacen.join(f), f.as_bytes());
    }
    let mut t = trabajo("t1", carpeta(&espejo.display().to_string()));
    t.nombre = "Disco E".into();
    t.que = Que::Equipos { equipos: vec!["ana".into()] };
    t.retencion = Retencion::Igual;
    let c = crate::server::ServerConfig {
        enabled: true,
        path: almacen.display().to_string(),
        espejo: Some(crate::espejo::Espejo::de_trabajos(vec![t.clone()])),
        ..Default::default()
    };
    crate::server::save(&c).unwrap();
    let texto = crate::espejo::hacer(&t, Motivo::Horario, &almacen);
    assert!(texto.starts_with("Espejo «Disco E» hecho"), "{texto}");
    assert!(espejo.join("ana/conta/data/ab/abcd").is_file() && !espejo.join("srv").exists(), "solo los de ese equipo");
    let guardado = crate::server::load().espejo.unwrap();
    assert_eq!(guardado.trabajos[0].estado.resultado.as_deref(), Some(texto.as_str()));
    assert_eq!(guardado.destinos.len(), 1, "y la vista para un agente anterior");
    // «Hacer ahora»: queda pedido hasta que empieza la vuelta siguiente.
    assert!(pedir_ahora(&json!({ "trabajo": "t1" })).unwrap().contains("Disco E"));
    assert!(pedir_ahora(&json!({ "trabajo": "otro" })).is_err());
    assert!(crate::server::load().espejo.unwrap().trabajos[0].estado.pedido_ahora);
    // La poda del almacén quita un paquete: se ve faltar y se borra en la vuelta siguiente.
    std::fs::remove_file(almacen.join("ana/conta/data/ab/abcd")).unwrap();
    crate::espejo::hacer(&t, Motivo::Ahora, &almacen);
    assert!(!crate::server::load().espejo.unwrap().trabajos[0].estado.pedido_ahora, "al empezar, se olvida");
    assert!(espejo.join("ana/conta/data/ab/abcd").is_file());
    assert_eq!(crate::server::load().espejo.unwrap().trabajos[0].estado.por_borrar.as_ref().map(|p| p.archivos), Some(1));
    crate::espejo::hacer(&t, Motivo::Horario, &almacen);
    assert!(!espejo.join("ana/conta/data/ab/abcd").exists() && espejo.join("ana/conta/data/cd/cdef").is_file());
    // Una orden de antes (por destinos) no puede cambiar estos trabajos.
    assert!(crate::server::poner_espejo(
        crate::espejo::pedido(&json!({ "destinos": [{ "tipo": "carpeta", "carpeta": espejo.display().to_string() }] })).unwrap()
    )
    .unwrap_err()
    .contains("consola actualizada"));
    // Y con los de antes, sí, convertidos.
    crate::server::save(&crate::server::ServerConfig { espejo: None, ..c.clone() }).unwrap();
    crate::server::poner_espejo(
        crate::espejo::pedido(&json!({ "hora": "04:00", "destinos": [{ "tipo": "carpeta", "carpeta": espejo.display().to_string() }] })).unwrap(),
    )
    .unwrap();
    let e = crate::server::load().espejo.unwrap();
    assert_eq!((e.trabajos.len(), e.trabajos[0].cuando.horario.clone()), (1, Some(horario_diario("04:00"))));
    std::env::remove_var("RESGUARDO_AGENT_DIR");
    let _ = std::fs::remove_dir_all(&b);
}

/// El propio equipo hace el espejo de sus repositorios (sin consola ni almacén): con restic y
/// rclone de verdad, a otra carpeta y a una «nube» (remoto local), y se restaura desde ahí.
#[test]
fn espejo_del_equipo_con_restic_y_rclone() {
    if crate::restic::version().is_err() {
        eprintln!("Sin restic: se salta la prueba.");
        return;
    }
    let _l = crate::restic::tests::real_repo_lock();
    let b = base("equipo");
    std::fs::create_dir_all(b.join("agente/privado")).unwrap();
    std::env::set_var("RESGUARDO_AGENT_DIR", b.join("agente"));
    let (datos, discos, otro, nube_dir) = (b.join("datos"), b.join("D"), b.join("F"), b.join("nube"));
    std::fs::create_dir_all(&datos).unwrap();
    std::fs::create_dir_all(&nube_dir).unwrap();
    std::fs::write(datos.join("factura.txt"), "factura de prueba\n".repeat(500)).unwrap();
    let contrasena = "contraseña de prueba del equipo";
    let repo = discos.join("documentos");
    let acc = crate::restic::Access::new(repo.display().to_string(), contrasena);
    for args in [vec!["init"], vec!["backup", datos.to_str().unwrap()]] {
        let out = crate::restic::run_raw(&acc, &args, Duration::from_secs(120)).unwrap();
        assert_eq!(out.code, Some(0), "{}", out.stderr);
    }
    envejecer(&repo);
    let locales = vec![("documentos".to_string(), repo.clone()), ("ya-no".to_string(), b.join("no-esta"))];
    // A otra carpeta del equipo.
    let pedido = json!({ "trabajos": [{ "id": "e1", "nombre": "Disco F", "quien": "equipo", "que": { "tipo": "repos", "repos": ["documentos"] },
        "adonde": { "tipo": "carpeta", "carpeta": otro.display().to_string() }, "cuando": { "tras_copia": true }, "retencion": { "modo": "nunca" } }] });
    let mut ts = leer_pedido(&pedido, QUIEN_EQUIPO).unwrap();
    poner_equipo(Some(ts.clone())).unwrap();
    assert_eq!(cargar_equipo().trabajos.len(), 1);
    let texto = hacer_equipo(&ts[0], Motivo::TrasCopia, &locales);
    assert!(texto.contains("documentos:") && !texto.starts_with("ERROR"), "{texto}");
    assert!(otro.join("documentos/config").is_file(), "cada repositorio en su carpeta");
    let r = resumen_equipo();
    assert_eq!(r["trabajos"][0]["resultado"].as_str(), Some(texto.as_str()));
    // Se abre con la misma contraseña desde el espejo.
    let desde = crate::restic::Access::new(otro.join("documentos").display().to_string(), contrasena);
    assert_eq!(crate::restic::snapshots(&desde).unwrap().len(), 1);
    // Un repositorio que ya no está: error, sin tocar nada.
    ts[0].que = Que::Repos { repos: vec!["documentos".into(), "ya-no".into()] };
    let texto = hacer_equipo(&ts[0], Motivo::Horario, &locales);
    assert!(texto.starts_with("ERROR") && texto.contains("ya-no"), "{texto}");
    // A una «nube» (rclone con un remoto local, el mismo camino que Dropbox o B2).
    if crate::nube::comprobar_binario().is_ok() {
        crate::nube::conectar_rclone(&json!({ "tipo": "alias", "nombre": "Nube de pruebas", "parametros": { "carpeta": nube_dir.display().to_string() } }))
            .unwrap();
        let mut n = leer_pedido(&json!({ "trabajos": [{ "id": "e2", "quien": "equipo", "que": { "tipo": "todos" },
            "adonde": { "tipo": "nube", "nube": "Nube de pruebas", "carpeta": "Resguardo" }, "cuando": { "tras_copia": true }, "retencion": { "modo": "igual" } }] }), QUIEN_EQUIPO)
        .unwrap();
        n[0].nombre = "Nube".into();
        let texto = hacer_equipo(&n[0], Motivo::TrasOtro, &locales[..1]);
        assert!(!texto.starts_with("ERROR"), "{texto}");
        let desde = crate::restic::Access::new(nube_dir.join("Resguardo/documentos").display().to_string(), contrasena);
        assert_eq!(crate::restic::snapshots(&desde).unwrap().len(), 1, "se abre con la contraseña del kit");
        let fuera = b.join("restaurado");
        let out = crate::restic::run_raw(&desde, &["restore", "latest", "--target", fuera.to_str().unwrap()], Duration::from_secs(120)).unwrap();
        assert_eq!(out.code, Some(0), "{}", out.stderr);
        // Una nube que usa un espejo del equipo no se desconecta.
        let mut dos = ts.clone();
        dos[0].que = Que::Todos;
        dos.push(Trabajo { cuando: Cuando { despues: Some("e1".into()), ..Default::default() }, ..n[0].clone() });
        poner_equipo(Some(dos)).unwrap();
        assert!(crate::nube_anular::quien_la_usa("Nube de pruebas").is_some_and(|u| u.contains("de este equipo")));
    } else {
        eprintln!("Sin rclone: se salta la parte de la nube.");
    }
    // Quitar todos los espejos del equipo.
    poner_equipo(None).unwrap();
    assert!(resumen_equipo().is_null());
    std::env::remove_var("RESGUARDO_AGENT_DIR");
    let _ = std::fs::remove_dir_all(&b);
}
