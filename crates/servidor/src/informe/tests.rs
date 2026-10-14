//! Pruebas del informe ejecutivo (plan 0.8.0, G1): los datos de un periodo, el PDF
//! (sin rutas, contraseñas ni «restic»), los periodos y el envío programado por correo
//! con el PDF adjunto (con la base de datos de verdad y un transporte falso).

use super::*;
use crate::almacen::{EntradaHistorial, EquipoNuevo, Rol};
use crate::estado::St;
use crate::notificaciones::ajustes::{self as na, CambioCanal, ConfigCanal, TipoCanal};
use crate::notificaciones::transporte::{Enviado, Falso};
use serde_json::json;
use std::sync::Arc;

/// 2026-10-14 (miércoles) 12:00 UTC.
const AHORA: Ts = 1_791_979_200;

struct Prueba {
    _dir: tempfile::TempDir,
    st: St,
    falso: Arc<Falso>,
    cliente: String,
}

fn equipo(st: &St, ctx: &ClienteCtx, id: &str, nombre: &str) {
    st.db
        .crear_equipo(
            ctx,
            &EquipoNuevo {
                id: id.into(),
                nombre: nombre.into(),
                so: "windows".into(),
                version: "0.8.0".into(),
                box_pub: "b".into(),
                sign_pub: "s".into(),
                sal_equipo: "x".into(),
                secreto_hash: "h".into(),
            },
        )
        .unwrap();
    st.db.confirmar_equipo(ctx, id, "etiqueta").unwrap();
    st.db.contacto_equipo(ctx, id, AHORA - 600).unwrap();
    let resumen = json!({
        "copias": [{ "id": "k1", "nombre": "Documentos", "repo": "r1" }, { "id": "k2", "nombre": "Contabilidad", "repo": "r1" }],
        "repositorios": [{ "id": "r1", "nombre": "Principal" }],
    });
    st.db.guardar_config(ctx, id, 1, "cifrado", &resumen).unwrap();
}

fn entrada(n: usize, hora: Ts, tipo: &str, datos: serde_json::Value) -> EntradaHistorial {
    let mut d = datos;
    d["tipo"] = json!(tipo);
    EntradaHistorial { id: format!("h{n}-{hora}"), hora, tipo: tipo.into(), datos: d.to_string(), aviso: None }
}

fn servidor() -> Prueba {
    let dir = tempfile::tempdir().unwrap();
    let st = crate::preparar(dir.path(), Default::default()).unwrap();
    let falso = Arc::new(Falso::default());
    st.notif.poner_transporte(falso.clone());
    let ana = st.db.crear_cuenta("ana@ejemplo.com", "Ana", "h", true).unwrap();
    let c = st.db.crear_cliente("Oficina Norte", "c2Fs", 24).unwrap();
    st.db.poner_rol(&ana.id, &c.id, Rol::Propietario).unwrap();
    let ctx = ClienteCtx::autorizado(&c.id);
    equipo(&st, &ctx, "e1", "pc-01");
    equipo(&st, &ctx, "e2", "servidor-01");
    // La semana pasada (del lunes 5 al domingo 11 de octubre): copias, una que falla dos
    // veces (con una ruta y una dirección en el mensaje, que no deben salir), verificaciones,
    // una prueba que falla y una restauración. Y una copia de la semana anterior (tendencia).
    let (desde, _) = periodo_anterior(Periodo::Semanal, AHORA);
    let mut h1 = Vec::new();
    for d in 0..7 {
        let t = desde + d * 86_400 + 9 * 3600;
        h1.push(entrada(1, t, "copia", json!({ "copia": "k1", "repo": "r1", "resultado": "ok", "anadido": 1_000_000_000u64 })));
        h1.push(entrada(2, t + 3600, "copia", json!({ "copia": "k2", "repo": "r1", "resultado": if d == 6 { "aviso" } else { "sin_cambios" } })));
    }
    for d in [2, 4] {
        h1.push(entrada(
            3,
            desde + d * 86_400 + 20 * 3600,
            "copia",
            json!({ "copia": "k2", "repo": "r1", "resultado": "fallo", "mensaje": "No se pudo leer C:\\Users\\Ana\\Contabilidad\\libro.xlsx (rest:https://u:ClaveSecreta@10.0.0.2:8000/r)" }),
        ));
    }
    h1.push(entrada(4, desde + 86_400 * 3, "verificacion", json!({ "repo": "r1", "resultado": "ok" })));
    h1.push(entrada(5, desde + 86_400 * 5, "prueba_restauracion", json!({ "repo": "r1", "resultado": "fallo", "mensaje": "Una huella no coincide." })));
    h1.push(entrada(
        6,
        desde + 86_400 * 5 + 7200,
        "restauracion",
        json!({ "repo": "r1", "resultado": "ok", "archivos": 120, "bytes": 52_000_000u64, "duracion_s": 240 }),
    ));
    h1.push(entrada(7, desde - 86_400 * 3, "copia", json!({ "copia": "k1", "repo": "r1", "resultado": "ok", "anadido": 3_500_000_000u64 })));
    st.db.guardar_historial(&ctx, "e1", &h1).unwrap();
    let mut h2 = Vec::new();
    for d in 0..7 {
        h2.push(entrada(8, desde + d * 86_400 + 22 * 3600, "copia", json!({ "copia": "k1", "repo": "r1", "resultado": "ok", "anadido": 250_000_000u64 })));
    }
    st.db.guardar_historial(&ctx, "e2", &h2).unwrap();
    st.db.guardar_informe(&ctx, "e1", &json!({ "repos": [{ "id": "r1", "espacio": { "en_disco_bytes": 120_000_000_000u64 } }] })).unwrap();
    // E3 (14k): una prueba funcional en el periodo (con incidencias, RTO fuera del objetivo) y otra de antes.
    st.db
        .poner_ajuste_cliente(
            &ctx,
            "pruebas_funcionales",
            &json!({ "activo": true, "meses": 6, "aviso_dias": 30, "rto_objetivo_min": 120, "rpo_objetivo_min": 1440 }).to_string(),
            "Ana",
        )
        .unwrap();
    for (id, fecha, resultado, rto) in [("pf-1", "2026-10-08", "parcial", 180), ("pf-0", "2026-04-02", "ok", 60)] {
        let datos = json!({ "fecha": fecha, "motivo": "programada", "alcance": "repos", "repos": [{ "equipo": "e1", "repo": "r1" }], "que": "Facturas", "donde": "Máquina de pruebas", "quien": "Teo", "resultado": resultado, "rto_min": rto, "rpo_min": 600 });
        let t = crate::pruebas_funcionales::dia(fecha).unwrap();
        let g = crate::almacen::PruebaGuardada { id: id.into(), fecha: t, datos: datos.to_string(), creada: t, actualizada: t, por: "Ana".into() };
        assert!(st.db.guardar_prueba_funcional(&ctx, &g, 10).unwrap());
    }
    // La regla que calculó la consola.
    use EstadoParte::*;
    guardar_foto_regla(
        st.db.as_ref(),
        &c.id,
        &FotoRegla {
            calculada: AHORA - 3600,
            por: "Ana".into(),
            copias: vec![
                CopiaFoto { equipo: "e1".into(), copia: "k1".into(), partes: [Cumple; 5] },
                CopiaFoto { equipo: "e1".into(), copia: "k2".into(), partes: [Cumple, Cumple, Falta, Falta, Atrasado] },
                CopiaFoto { equipo: "e2".into(), copia: "k1".into(), partes: [Cumple; 5] },
                CopiaFoto { equipo: "otro".into(), copia: "k1".into(), partes: [Falta; 5] },
            ],
        },
    )
    .unwrap();
    // El correo del servidor.
    let correo = CambioCanal {
        tipo: Some(TipoCanal::Correo),
        config: Some(ConfigCanal { host: Some("smtp.ejemplo.com".into()), remitente: Some("Resguardo <avisos@ejemplo.com>".into()), ..Default::default() }),
        secretos: Some(std::collections::BTreeMap::from([("contrasena".to_string(), "CONTRASENA-SMTP".to_string())])),
        ..Default::default()
    };
    let mut a = na::Ajustes { url_consola: Some("https://copias.ejemplo.com".into()), ..Default::default() };
    a.canales.push(na::aplicar(None, &correo, &st.notif.clave, "servidor", "Ana", AHORA).unwrap().0);
    na::guardar_ajustes(st.db.as_ref(), &a).unwrap();
    guardar_prestador(st.db.as_ref(), &Prestador { nombre: Some("Soporte Ejemplo".into()), ..Default::default() }).unwrap();
    Prueba { _dir: dir, st, falso, cliente: c.id }
}

/// Un PNG de verdad (con canal alfa), hecho con el codificador de `png`.
fn png(ancho: u32, alto: u32) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, ancho, alto);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        let mut w = e.write_header().unwrap();
        let px: Vec<u8> = (0..ancho * alto).flat_map(|i| [15, 118, 110, if i % 7 == 0 { 0 } else { 255 }]).collect();
        w.write_image_data(&px).unwrap();
    }
    out
}

#[test]
fn periodos() {
    // 2026-10-14 es miércoles: la semana anterior va del lunes 5 al lunes 12; el mes, septiembre.
    let (d, h) = periodo_anterior(Periodo::Semanal, AHORA);
    assert_eq!((local(d).date().to_string(), local(h).date().to_string()), ("2026-10-05".into(), "2026-10-12".into()));
    assert_eq!(local(d).time().to_string(), "00:00:00");
    let (d, h) = periodo_anterior(Periodo::Mensual, AHORA);
    assert_eq!((local(d).date().to_string(), local(h).date().to_string()), ("2026-09-01".into(), "2026-10-01".into()));
    let (d, h) = periodo_actual(Periodo::Mensual, AHORA);
    assert_eq!((local(d).date().to_string(), local(h).date().to_string()), ("2026-10-01".into(), "2026-11-01".into()));
    assert_eq!(clave_periodo(Periodo::Semanal, periodo_anterior(Periodo::Semanal, AHORA).0), "semanal:2026-W41");
    assert_eq!(clave_periodo(Periodo::Mensual, periodo_anterior(Periodo::Mensual, AHORA).0), "mensual:2026-09");
    // El semanal de los lunes ya tocaba; el de los jueves, aún no. El mensual, desde el día 1.
    let p = |periodo, dia| Programa { activo: true, periodo, dia_semana: dia, ..Default::default() };
    let mediodia = notif::de_local(local(AHORA).date().and_hms_opt(12, 0, 0).unwrap());
    assert!(toca(&p(Periodo::Semanal, 1), "08:00", mediodia));
    assert!(toca(&p(Periodo::Semanal, 3), "08:00", mediodia));
    assert!(!toca(&p(Periodo::Semanal, 3), "13:00", mediodia));
    assert!(!toca(&p(Periodo::Semanal, 4), "08:00", mediodia));
    assert!(toca(&p(Periodo::Mensual, 1), "08:00", mediodia));
    // Destinatarios.
    assert_eq!(valida_destinatarios(&[" ana@ejemplo.com ".into(), "ANA@ejemplo.com".into(), "".into()]).unwrap(), vec!["ana@ejemplo.com".to_string()]);
    assert!(valida_destinatarios(&["no es un correo".into()]).is_err());
    assert!(valida_destinatarios(&["a@b.com, c@d.com".into()]).is_err());
    assert!(valida_destinatarios(&(0..21).map(|i| format!("p{i}@ejemplo.com")).collect::<Vec<_>>()).is_err());
    assert!(id_valido("e1") && id_valido("3f2a-9c:k1") && !id_valido("") && !id_valido("a b") && !id_valido(&"x".repeat(101)));
}

#[test]
fn datos_del_periodo() {
    let p = servidor();
    let (desde, hasta) = periodo_anterior(Periodo::Semanal, AHORA);
    let i = datos::preparar(p.st.db.as_ref(), &p.cliente, Periodo::Semanal, desde, hasta, AHORA).unwrap().unwrap();
    assert_eq!(i.cliente, "Oficina Norte");
    // 7 + 7 de pc-01, 2 fallidas y 7 de servidor-01: 23 copias, 20 correctas, 1 con avisos, 2 fallidas.
    assert_eq!((i.copias.total(), i.copias.correctas(), i.copias.aviso, i.copias.fallo), (23, 20, 1, 2));
    assert_eq!(i.dias.len(), 7);
    assert_eq!(i.dias.iter().map(|d| d.copias.total()).sum::<u32>(), 23);
    assert_eq!(i.espacio.nuevos, 7 * 1_000_000_000 + 7 * 250_000_000);
    assert_eq!(i.espacio.nuevos_anterior, 3_500_000_000);
    assert_eq!(i.espacio.protegido, 120_000_000_000);
    // La copia que falló, una sola vez con sus dos veces y el mensaje sin la ruta ni la dirección.
    assert_eq!(i.fallos.len(), 1);
    let f = &i.fallos[0];
    assert_eq!((f.equipo.as_str(), f.copia.as_str(), f.veces), ("pc-01", "Contabilidad", 2));
    assert!(!f.mensaje.contains("Users") && !f.mensaje.contains("ClaveSecreta") && !f.mensaje.contains("10.0.0.2"), "{}", f.mensaje);
    assert_eq!((i.verificaciones.ok, i.verificaciones.fallo, i.pruebas.ok, i.pruebas.fallo), (1, 0, 0, 1));
    assert_eq!(i.pruebas.fallidas[0].repo, "Principal");
    assert_eq!((i.restauraciones.ok, i.restauraciones.bytes, i.restauraciones.duracion_media_s), (1, 52_000_000, Some(240)));
    // La regla: solo los equipos que siguen (el «otro» no cuenta).
    let r = i.regla.as_ref().unwrap();
    assert_eq!((r.total, r.cumplen, r.faltan), (3, 2, [0, 0, 1, 1, 1]));
    assert_eq!(r.sin_cumplir[0].copia, "Contabilidad");
    assert_eq!(r.sin_cumplir[0].falta, "Falta: 1 fuera del sitio · 1 inmutable o aislado. No está al día: 0 errores.");
    assert_eq!(i.prestador.as_deref(), Some("Soporte Ejemplo"));
    // E3 (14k): solo la del periodo, con su alcance por nombre y si cumple los objetivos.
    let f = &i.funcionales;
    assert_eq!(f.lista.len(), 1);
    assert_eq!((f.lista[0].alcance.as_str(), f.lista[0].resultado.as_str(), f.lista[0].quien.as_str()), ("Principal", "parcial", "Teo"));
    assert_eq!((f.lista[0].cumple_rto, f.lista[0].cumple_rpo), (Some(false), Some(true)));
    assert_eq!((f.rto_objetivo_min, f.ultima), (Some(120), crate::pruebas_funcionales::dia("2026-10-08")));
    assert_eq!(f.fase.as_deref(), Some("al_dia"));
    // Cabe en la cola (va entero en el envío).
    assert!(serde_json::to_string(&i).unwrap().len() < 64 * 1024);
}

#[test]
fn el_pdf() {
    let p = servidor();
    let (desde, hasta) = periodo_anterior(Periodo::Semanal, AHORA);
    let i = datos::preparar(p.st.db.as_ref(), &p.cliente, Periodo::Semanal, desde, hasta, AHORA).unwrap().unwrap();
    let logo = png(120, 40);
    let b = pdf::pdf(&i, Some(&logo));
    assert!(b.starts_with(b"%PDF-") && b.windows(5).any(|w| w == b"%%EOF"), "es un PDF");
    assert!(b.windows(8).any(|w| w == b"/Subtype") && b.windows(6).any(|w| w == b"/Image"), "con el logo");
    // Lo que lleva escrito: lo que se pide y nada de lo que no.
    let (paginas, textos) = pdf::textos(&i, Some(&logo));
    let todo = textos.join("\n");
    for esperado in [
        "Informe semanal de copias",
        "Oficina Norte · del 5 al 11 de octubre de 2026",
        "copias correctas",
        "Copias por día",
        "Datos nuevos por día",
        "Regla 3-2-1-1-0",
        "Verificaciones, pruebas de restauración y restauraciones",
        "Pruebas funcionales (simulacros de recuperación)",
        "Con incidencias",
        "3 h (no cumple)",
        "Objetivos: datos de vuelta en 2 h",
        "perder 24 h como mucho.",
        "La siguiente toca antes del 6 de abril de 2027.",
        "Equipos",
        "Copias que fallaron",
        "Avisos importantes sin resolver",
        "pc-01",
        "servidor-01",
        "Contabilidad",
        "Página 1 de",
    ] {
        assert!(todo.contains(esperado), "falta «{esperado}» en:\n{todo}");
    }
    assert!(todo.contains("Preparado por Soporte Ejemplo con Resguardo"));
    for prohibido in ["restic", "Users", "ClaveSecreta", "10.0.0.2", "libro.xlsx", "CONTRASENA"] {
        assert!(!todo.to_lowercase().contains(&prohibido.to_lowercase()), "«{prohibido}» en el PDF");
    }
    assert!(paginas >= 2, "{paginas}");
    // Sin logo, o con uno que no es un PNG, sale igual (sin imagen).
    let sin = pdf::pdf(&i, Some(b"<svg><script>alert(1)</script></svg>"));
    assert!(sin.starts_with(b"%PDF-") && !sin.windows(6).any(|w| w == b"/Image"));
    // Un informe vacío (cliente sin equipos) también.
    let mut vacio = i.clone();
    vacio.equipos.clear();
    vacio.copias = datos::Cuenta::default();
    vacio.dias.iter_mut().for_each(|d| *d = datos::Dia { inicio: d.inicio, ..Default::default() });
    vacio.regla = None;
    assert!(pdf::pdf(&vacio, None).starts_with(b"%PDF-"));
    // El de ejemplo para revisarlo a mano: RESGUARDO_INFORME_EJEMPLO=<archivo.pdf>.
    if let Some(ruta) = std::env::var_os("RESGUARDO_INFORME_EJEMPLO") {
        std::fs::write(ruta, &b).unwrap();
    }
}

#[test]
fn programado_por_correo_con_el_pdf_adjunto() {
    let p = servidor();
    let db = p.st.db.as_ref();
    let destinatarios = vec!["direccion@ejemplo.com".to_string(), "it@ejemplo.com".to_string()];
    let previo = programa(db, &p.cliente).unwrap();
    let prog = Programa { activo: true, periodo: Periodo::Semanal, dia_semana: 1, destinatarios: destinatarios.clone(), ..Default::default() };
    // Encenderlo hoy no manda el de la semana pasada (ya terminada): el primero, el lunes que viene.
    guardar_programa(db, &p.cliente, &previo, &prog, AHORA).unwrap();
    notif::pasada(&p.st.db, &p.st.notif, AHORA).unwrap();
    assert!(p.falso.enviados().is_empty());
    // El lunes siguiente, a las 9: sale a los dos, con el PDF adjunto.
    let lunes = periodo_actual(Periodo::Semanal, AHORA).1 + 9 * 3600;
    notif::pasada(&p.st.db, &p.st.notif, lunes).unwrap();
    // (El resumen semanal de las personas también sale ese lunes: aquí solo cuentan los informes.)
    let informes = || -> Vec<(String, String, String)> {
        p.falso
            .enviados()
            .into_iter()
            .filter_map(|e| match e {
                Enviado::Correo { para, asunto, crudo, .. } if asunto.starts_with("Informe ") => Some((para, asunto, crudo)),
                _ => None,
            })
            .collect()
    };
    let correos = informes();
    assert_eq!(correos.len(), 2, "{correos:?}");
    let (para, asunto, crudo) = &correos[0];
    assert!(destinatarios.iter().any(|d| para.contains(d.as_str())));
    assert!(asunto.starts_with("Informe semanal de copias · Oficina Norte · del 12 al 18 de octubre"), "{asunto}");
    assert!(crudo.contains("multipart/mixed") && crudo.contains("application/pdf"), "con el adjunto");
    assert!(crudo.contains("informe-semanal-2026-10-12.pdf"));
    assert!(!crudo.contains("CONTRASENA-SMTP"));
    // El adjunto es un PDF (en base64: «%PDF-» empieza por «JVBERi0»).
    assert!(crudo.contains("JVBERi0"), "el PDF va dentro");
    // Una segunda pasada no lo repite.
    notif::pasada(&p.st.db, &p.st.notif, lunes + 60).unwrap();
    assert_eq!(informes().len(), 2);
    let u = ultimo(db, &p.cliente).unwrap().unwrap();
    assert_eq!((u.destinatarios, u.a_mano), (2, false));
    // «Enviar ahora» (a mano), con el periodo que se diga.
    let (d, h) = periodo_anterior(Periodo::Mensual, AHORA);
    assert_eq!(encolar(db, &p.cliente, Periodo::Mensual, d, h, &["it@ejemplo.com".into()], AHORA, Some("Ana")).unwrap(), Ok(1));
    notif::pasada(&p.st.db, &p.st.notif, AHORA + 5).unwrap();
    assert!(p
        .falso
        .enviados()
        .iter()
        .any(|e| matches!(e, Enviado::Correo { asunto, .. } if asunto.starts_with("Informe mensual de copias · Oficina Norte · septiembre de 2026"))));
    // Sin destinatarios o sin canal de correo, se dice por qué.
    assert!(encolar(db, &p.cliente, Periodo::Mensual, d, h, &[], AHORA, None).unwrap().is_err());
    na::guardar_ajustes(db, &na::Ajustes::default()).unwrap();
    assert!(encolar(db, &p.cliente, Periodo::Mensual, d, h, &["it@ejemplo.com".into()], AHORA, None).unwrap().unwrap_err().contains("canal de correo"));
}

#[test]
fn el_logo_del_prestador_o_el_del_cliente() {
    let p = servidor();
    let db = p.st.db.as_ref();
    assert!(logo(db, &p.cliente, Logo::Prestador).is_none());
    let del_prestador = png(30, 10);
    let mut pr = prestador(db).unwrap();
    pr.logo = Some(B64.encode(&del_prestador));
    guardar_prestador(db, &pr).unwrap();
    assert_eq!(logo(db, &p.cliente, Logo::Prestador), Some(del_prestador.clone()));
    assert_eq!(logo(db, &p.cliente, Logo::Ninguno), None);
    // El cliente sin logo propio: el del prestador.
    assert_eq!(logo(db, &p.cliente, Logo::Cliente), Some(del_prestador));
}
