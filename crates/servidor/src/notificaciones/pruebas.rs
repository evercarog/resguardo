//! Pruebas de punta a punta de las notificaciones, con la base de datos de
//! verdad (en una carpeta temporal) y un transporte falso: lo que llega a quién,
//! agrupar lo repetido, «Volvió a funcionar», tope por hora, horas de silencio,
//! reintentos y resúmenes.

use super::ajustes::{self, CambioCanal, ConfigCanal, PrefsPersona, Reglas, TipoCanal};
use super::transporte::{Enviado, Fallo, Falso};
use super::*;
use crate::almacen::{ClienteCtx, EquipoNuevo, Rol};
use serde_json::json;
use std::collections::BTreeMap;

struct Prueba {
    _dir: tempfile::TempDir,
    st: St,
    falso: Arc<Falso>,
    ctx: ClienteCtx,
    ana: String,
}

const T0: Ts = 1_791_100_800; // 2026-10-04 08:00 UTC

fn equipo(st: &St, ctx: &ClienteCtx, id: &str, nombre: &str, contacto: Ts) {
    st.db
        .crear_equipo(
            ctx,
            &EquipoNuevo {
                id: id.into(),
                nombre: nombre.into(),
                so: "windows".into(),
                version: "0.7.11".into(),
                box_pub: "b".into(),
                sign_pub: "s".into(),
                sal_equipo: "x".into(),
                secreto_hash: "h".into(),
            },
        )
        .unwrap();
    st.db.confirmar_equipo(ctx, id, "etiqueta").unwrap();
    st.db.contacto_equipo(ctx, id, contacto).unwrap();
}

/// Un servidor con un cliente («Ferretería Altamar»), tres personas (propietaria, técnico y de lectura),
/// un equipo, el correo del servidor y un webhook para los críticos.
fn servidor() -> Prueba {
    let dir = tempfile::tempdir().unwrap();
    let st = crate::preparar(dir.path(), Default::default()).unwrap();
    let falso = Arc::new(Falso::default());
    st.notif.poner_transporte(falso.clone());
    let ana = st.db.crear_cuenta("ana@ejemplo.com", "Ana", "h", true).unwrap();
    let tom = st.db.crear_cuenta("tom@ejemplo.com", "Tom", "h", false).unwrap();
    let leo = st.db.crear_cuenta("leo@ejemplo.com", "Leo", "h", false).unwrap();
    let c = st.db.crear_cliente("Ferretería Altamar", "c2Fs", 24).unwrap();
    st.db.poner_rol(&ana.id, &c.id, Rol::Propietario).unwrap();
    st.db.poner_rol(&tom.id, &c.id, Rol::Tecnico).unwrap();
    st.db.poner_rol(&leo.id, &c.id, Rol::Lectura).unwrap();
    let ctx = ClienteCtx::autorizado(&c.id);
    equipo(&st, &ctx, "e1", "PC-Contabilidad", T0);
    let correo = CambioCanal {
        tipo: Some(TipoCanal::Correo),
        config: Some(ConfigCanal { host: Some("smtp.ejemplo.com".into()), remitente: Some("Resguardo <avisos@ejemplo.com>".into()), ..Default::default() }),
        secretos: Some(BTreeMap::from([("contrasena".to_string(), "CONTRASENA-SMTP".to_string())])),
        ..Default::default()
    };
    let webhook = CambioCanal {
        tipo: Some(TipoCanal::Webhook),
        secretos: Some(BTreeMap::from([
            ("url".to_string(), "https://hooks.ejemplo.com/T0/SECRETO-URL".to_string()),
            ("secreto".to_string(), "secreto-para-firmar-1234".to_string()),
        ])),
        reglas: Some(Reglas { severidades: vec![Severidad::Critico], ..Default::default() }),
        ..Default::default()
    };
    // El resumen semanal, un miércoles: las pruebas van de domingo (T0) a lunes y así no
    // dependen de la zona horaria de la máquina (en UTC, el lunes a las 08:00 ya tocaba).
    let mut a = ajustes::Ajustes { url_consola: Some("https://copias.ejemplo.com".into()), dia_semanal: 3, ..Default::default() };
    for c in [correo, webhook] {
        a.canales.push(ajustes::aplicar(None, &c, &st.notif.clave, "servidor", "Ana", T0).unwrap().0);
    }
    ajustes::guardar_ajustes(st.db.as_ref(), &a).unwrap();
    // Sin horas de silencio en las pruebas (salvo las que las ponen).
    for cuenta in [&ana.id, &tom.id, &leo.id] {
        ajustes::guardar_prefs_persona(st.db.as_ref(), cuenta, &PrefsPersona { silencio: None, resumen_diario: false, resumen_semanal: true }).unwrap();
    }
    Prueba { _dir: dir, st, falso, ctx, ana: ana.id }
}

impl Prueba {
    fn pasada(&self, t: Ts) {
        pasada(&self.st.db, &self.st.notif, t).unwrap();
    }
    fn enviados(&self) -> Vec<Enviado> {
        self.falso.enviados()
    }
    fn correos(&self) -> Vec<(String, String)> {
        self.enviados()
            .into_iter()
            .filter_map(|e| match e {
                Enviado::Correo { para, texto, html, crudo, .. } => {
                    // Lo mismo en las dos partes y nada de secretos en el correo entero.
                    assert!(!crudo.contains("CONTRASENA-SMTP") && !html.contains("Users"));
                    Some((para, texto))
                }
                _ => None,
            })
            .collect()
    }
    fn webhooks(&self) -> Vec<serde_json::Value> {
        self.enviados()
            .into_iter()
            .filter_map(|e| match e {
                Enviado::Http(p) => Some(serde_json::from_slice(&p.cuerpo).unwrap()),
                _ => None,
            })
            .collect()
    }
    fn informe(&self, copias: serde_json::Value) {
        let datos = json!({ "copias": copias });
        if let Some(e) = evento_estado(&self.st.notif, &self.ctx, "e1", problemas::Fuente::Informe, &datos) {
            apuntar(self.st.db.as_ref(), &e).unwrap();
        }
    }
}

fn copia(estado: &str, cuando: &str) -> serde_json::Value {
    json!([{ "id": "c1", "nombre": "Documentos", "estado": estado, "cuando": cuando, "mensaje": "No se pudo leer C:\\Users\\Ana\\secreto.txt" }])
}

#[test]
fn cada_uno_recibe_lo_suyo_y_lo_repetido_se_agrupa() {
    let p = servidor();
    aviso_a(p.st.db.as_ref(), &p.ctx, Some("e1"), "intentos_fallidos", "5 intentos con la clave de administración mal", T0).unwrap();
    p.pasada(T0);
    // Crítico: la propietaria y el técnico por correo (el de lectura, no) y el webhook.
    let mut para: Vec<String> = p.correos().into_iter().map(|(a, _)| a).collect();
    para.sort();
    assert_eq!(para, vec!["ana@ejemplo.com".to_string(), "tom@ejemplo.com".to_string()]);
    let w = p.webhooks();
    assert_eq!(w.len(), 1);
    assert_eq!((w[0]["evento"].as_str(), w[0]["severidad"].as_str()), (Some("aviso"), Some("critico")));
    assert_eq!(w[0]["avisos"][0]["equipo"]["nombre"], "PC-Contabilidad");
    assert_eq!(w[0]["avisos"][0]["enlace"], format!("https://copias.ejemplo.com/c/{}/equipos/e1", p.ctx.id()));
    // Nada de secretos en lo que sale.
    for e in p.enviados() {
        let t = format!("{e:?}");
        assert!(!t.contains("CONTRASENA-SMTP") && !t.contains("secreto-para-firmar"), "{t}");
    }
    // Lo mismo otra vez, al rato: se cuenta, no se manda.
    aviso_a(p.st.db.as_ref(), &p.ctx, Some("e1"), "intentos_fallidos", "otra vez", T0 + 600).unwrap();
    p.pasada(T0 + 600);
    assert_eq!(p.enviados().len(), 3);
    let inc = p.st.db.notif_incidente(&format!("{}|e1|intentos_fallidos|", p.ctx.id())).unwrap().unwrap();
    assert_eq!(inc.veces, 2);
    // Un día después sigue pasando: se recuerda («Sigue pasando»).
    aviso_a(p.st.db.as_ref(), &p.ctx, Some("e1"), "intentos_fallidos", "y otra", T0 + 600 + 23 * 3600).unwrap();
    p.pasada(T0 + 600 + 23 * 3600);
    assert_eq!(p.enviados().len(), 3, "dentro del día, nada");
    // Uno importante: solo la propietaria (el técnico recibe críticos; el webhook, críticos).
    aviso_a(p.st.db.as_ref(), &p.ctx, Some("e1"), "equipo_sin_contacto", "«PC-Contabilidad» lleva más de 24 h sin conectar", T0 + 24 * 3600).unwrap();
    p.pasada(T0 + 24 * 3600);
    let c = p.correos();
    assert_eq!(c.len(), 3);
    assert_eq!(c[2].0, "ana@ejemplo.com");
    assert!(c[2].1.contains("no conecta con el servidor"), "{}", c[2].1);
    // El registro lo cuenta (y sin secretos).
    let r = p.st.db.notif_registro(None, 100).unwrap();
    assert_eq!(r.iter().filter(|e| e.estado == "enviado").count(), 4);
}

#[test]
fn fallo_de_copia_y_volvio_a_funcionar() {
    let p = servidor();
    p.informe(copia("fallo", "2026-10-04T06:00:00Z"));
    p.pasada(T0 + 60);
    let c = p.correos();
    assert_eq!(c.len(), 2, "propietaria y técnico");
    let texto = &c[0].1;
    assert!(texto.contains("Falló la copia «Documentos» en «PC-Contabilidad»"), "{texto}");
    assert!(!texto.contains("Users") && !texto.contains("secreto.txt"), "sin rutas: {texto}");
    // También entra en la lista de avisos de la consola.
    let avisos = p.st.db.avisos(&p.ctx, true).unwrap();
    assert!(avisos.iter().any(|a| a.tipo == "copia_fallida" && !a.mensaje.contains("Users")), "{avisos:?}");
    // El mismo informe otra vez: ni se apunta.
    p.informe(copia("fallo", "2026-10-04T06:00:00Z"));
    assert!(p.st.db.notif_eventos(10).unwrap().is_empty());
    // Otra vuelta que falla: se cuenta sin avisar.
    p.informe(copia("fallo", "2026-10-04T07:00:00Z"));
    p.pasada(T0 + 3600 + 60);
    assert_eq!(p.correos().len(), 2);
    // Vuelve a ir bien: «Volvió a funcionar» a quien recibió el aviso.
    p.informe(copia("ok", "2026-10-04T08:00:00Z"));
    p.pasada(T0 + 2 * 3600 + 60);
    let c = p.correos();
    assert_eq!(c.len(), 4);
    let t = &c[3].1;
    assert!(t.contains("Volvió a funcionar la copia «Documentos» en «PC-Contabilidad»"), "{t}");
    assert!(t.contains("Falló 2 veces"), "{t}");
    let w = p.webhooks();
    assert_eq!(w.last().unwrap()["evento"], "recuperacion");
    assert_eq!(w.last().unwrap()["avisos"][0]["resuelto"], true);
    assert!(p.st.db.notif_incidentes_abiertos(None).unwrap().is_empty());
    // Ir bien otra vez no repite nada.
    p.informe(copia("ok", "2026-10-04T09:00:00Z"));
    p.pasada(T0 + 3 * 3600 + 60);
    assert_eq!(p.correos().len(), 4);
}

#[test]
fn tope_por_hora_y_lo_demas_agrupado() {
    let p = servidor();
    let mut a = ajustes::ajustes(p.st.db.as_ref()).unwrap();
    a.max_por_hora = 2;
    ajustes::guardar_ajustes(p.st.db.as_ref(), &a).unwrap();
    for (i, e) in ["e2", "e3", "e4", "e5", "e6"].iter().enumerate() {
        equipo(&p.st, &p.ctx, e, &format!("PC-{i}"), T0);
    }
    // Cinco avisos críticos, uno tras otro (una pasada entre cada uno).
    for (i, e) in ["e2", "e3", "e4", "e5", "e6"].iter().enumerate() {
        aviso_a(p.st.db.as_ref(), &p.ctx, Some(e), "bloqueo", "bloqueado", T0 + i as Ts * 60).unwrap();
        p.pasada(T0 + i as Ts * 60);
    }
    // El webhook: dos en la primera hora; lo demás espera.
    assert_eq!(p.webhooks().len(), 2);
    let pendientes = p.st.db.notif_envios_debidos(T0 + 10 * 3600, 100).unwrap();
    assert!(pendientes.iter().all(|e| e.nota.as_deref().is_some_and(|n| n.contains("Tope de 2"))), "{pendientes:?}");
    // Pasada la hora: el resto, en un solo mensaje.
    p.pasada(T0 + 3600 + 2);
    let w = p.webhooks();
    assert_eq!(w.len(), 3);
    assert_eq!(w[2]["evento"], "grupo");
    assert_eq!(w[2]["avisos"].as_array().unwrap().len(), 3);
    assert!(w[2]["titulo"].as_str().unwrap().starts_with("3 avisos de Resguardo"));
    // Los agrupados quedan como enviados (agrupados) en el registro.
    let r = p.st.db.notif_registro(None, 100).unwrap();
    assert_eq!(r.iter().filter(|e| e.nota.as_deref() == Some("Agrupado con 2 más.")).count(), 3 * 3, "webhook y dos correos, de 3 en 3");
}

#[test]
fn reintentos_con_espera_creciente_y_fallo_definitivo() {
    let p = servidor();
    // Solo el webhook (sin correo).
    let mut a = ajustes::ajustes(p.st.db.as_ref()).unwrap();
    a.canales.retain(|c| c.tipo == TipoCanal::Webhook);
    ajustes::guardar_ajustes(p.st.db.as_ref(), &a).unwrap();
    p.falso.fallar(Fallo { permanente: false, texto: "No respondió a tiempo.".into() });
    p.falso.fallar(Fallo { permanente: false, texto: "No respondió a tiempo.".into() });
    aviso_a(p.st.db.as_ref(), &p.ctx, Some("e1"), "bloqueo", "x", T0).unwrap();
    p.pasada(T0);
    let e = &p.st.db.notif_registro(None, 10).unwrap()[0];
    assert_eq!((e.estado.as_str(), e.intentos, e.siguiente), ("pendiente", 1, T0 + 60));
    assert_eq!(e.error.as_deref(), Some("No respondió a tiempo."));
    p.pasada(T0 + 30);
    assert_eq!(p.st.db.notif_registro(None, 10).unwrap()[0].intentos, 1, "aún no toca");
    p.pasada(T0 + 60);
    let e = &p.st.db.notif_registro(None, 10).unwrap()[0];
    assert_eq!((e.intentos, e.siguiente), (2, T0 + 60 + 120));
    p.pasada(T0 + 180);
    let e = &p.st.db.notif_registro(None, 10).unwrap()[0];
    assert_eq!((e.estado.as_str(), e.intentos), ("enviado", 3));
    assert_eq!(p.webhooks().len(), 1);
    // Un error que no se arregla solo: fallido sin más intentos.
    p.falso.fallar(Fallo { permanente: true, texto: "El servicio no aceptó las credenciales (HTTP 401).".into() });
    aviso_a(p.st.db.as_ref(), &p.ctx, Some("e1"), "cambio_inusual", "x", T0 + 200).unwrap();
    p.pasada(T0 + 200);
    let e = &p.st.db.notif_registro(None, 10).unwrap()[0];
    assert_eq!((e.estado.as_str(), e.intentos), ("fallido", 1));
    // Y uno que siempre falla: a los 8 intentos, fallido.
    for _ in 0..8 {
        p.falso.fallar(Fallo { permanente: false, texto: "No se pudo conectar.".into() });
    }
    aviso_a(p.st.db.as_ref(), &p.ctx, Some("e1"), "servicio_detenido", "x", T0 + 300).unwrap();
    let mut t = T0 + 300;
    for _ in 0..8 {
        p.pasada(t);
        t += 3600;
    }
    let e = &p.st.db.notif_registro(None, 10).unwrap().into_iter().find(|e| e.titulo.contains("servicio")).unwrap();
    assert_eq!((e.estado.as_str(), e.intentos), ("fallido", 8));
}

#[test]
fn horas_de_silencio_y_lo_arreglado_antes_no_sale() {
    let p = servidor();
    // Ana: silencio ahora mismo (de una hora antes a una hora después), también para los críticos.
    let h = chrono::Timelike::hour(&local(T0));
    let silencio = Silencio { desde: format!("{:02}:00", (h + 23) % 24), hasta: format!("{:02}:00", (h + 1) % 24), salvo_criticos: false };
    ajustes::guardar_prefs_persona(p.st.db.as_ref(), &p.ana, &PrefsPersona { silencio: Some(silencio.clone()), ..Default::default() }).unwrap();
    p.informe(copia("fallo", "2026-10-04T06:00:00Z"));
    p.pasada(T0);
    let para: Vec<String> = p.correos().into_iter().map(|(a, _)| a).collect();
    assert_eq!(para, vec!["tom@ejemplo.com".to_string()], "Ana, en silencio");
    let fin = de_local(reglas::fin_silencio(&silencio, local(T0)));
    let de_ana = p.st.db.notif_registro(None, 10).unwrap().into_iter().find(|e| e.destino == "ana@ejemplo.com").unwrap();
    assert_eq!((de_ana.estado.as_str(), de_ana.siguiente), ("pendiente", fin));
    // Se arregla antes del final del silencio: a Ana no le llega nada (ni el aviso ni el arreglo);
    // a Tom, que recibió el aviso, sí el arreglo.
    p.informe(copia("ok", "2026-10-04T06:20:00Z"));
    p.pasada(T0 + 1200);
    p.pasada(fin + 1);
    let para: Vec<String> = p.correos().into_iter().map(|(a, _)| a).collect();
    assert_eq!(para, vec!["tom@ejemplo.com".to_string(), "tom@ejemplo.com".to_string()]);
    let de_ana = p.st.db.notif_registro(None, 10).unwrap().into_iter().find(|e| e.destino == "ana@ejemplo.com").unwrap();
    assert_eq!(de_ana.estado, "descartado");
}

#[test]
fn resumen_semanal_por_persona() {
    let p = servidor();
    let ahora = T0 + 3600;
    let fecha = |t: Ts| chrono::DateTime::from_timestamp(t, 0).unwrap().to_rfc3339();
    p.st.db
        .guardar_informe(
            &p.ctx,
            "e1",
            &json!({ "copias": [{ "id": "c1", "nombre": "Documentos", "estado": "ok", "cuando": fecha(ahora - 600) }],
                     "repos": [{ "id": "r1", "espacio": { "en_disco_bytes": 2_500_000_000u64 },
                                 "ejecuciones": [{ "hora": fecha(ahora - 600), "resultado": "ok" }, { "hora": fecha(ahora - 86_400), "resultado": "fallo" }] }] }),
        )
        .unwrap();
    p.st.db.contacto_equipo(&p.ctx, "e1", ahora - 60).unwrap();
    // v1.4x (9b): la cabeza de la auditoría del cliente va en el resumen («ancla»).
    p.st.db.auditar(&p.ctx, "cuenta:ana@ejemplo.com", "renombrar_cliente", "", "{}").unwrap();
    let cabeza = crate::ancla::de_cliente(p.st.db.as_ref(), &p.ctx).unwrap().expect("con auditoría");
    let a = ajustes::ajustes(p.st.db.as_ref()).unwrap();
    let n = resumen::generar(p.st.db.as_ref(), &a, resumen::Periodo::Semanal, ahora).unwrap();
    assert_eq!(n, 1, "solo la propietaria (el técnico y el de lectura no tienen resumen por defecto)");
    p.pasada(ahora);
    let c = p.correos();
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].0, "ana@ejemplo.com");
    let t = &c[0].1;
    assert!(t.contains("Resumen semanal de copias"), "{t}");
    assert!(t.contains("PC-Contabilidad") && t.contains("1 fallo") && t.contains("2,5"), "{t}");
    assert!(t.contains(&cabeza.linea()) && t.contains("Comprobar con un ancla"), "{t}");
    // No cuenta para el tope ni se agrupa con avisos: sale suelto.
    let correo = a.canales.iter().find(|c| c.tipo == TipoCanal::Correo).unwrap().id.clone();
    assert!(p.st.db.notif_entregas_desde(&correo, "ana@ejemplo.com", 0).unwrap().is_empty());
    // A la hora del resumen, una vez por día y semana.
    let mut a2 = a.clone();
    a2.hora_resumen = "00:00".into();
    a2.dia_semanal = chrono::Datelike::weekday(&local(ahora)).number_from_monday() as u8;
    ajustes::guardar_ajustes(p.st.db.as_ref(), &a2).unwrap();
    resumen::si_toca(p.st.db.as_ref(), ahora).unwrap();
    resumen::si_toca(p.st.db.as_ref(), ahora + 60).unwrap();
    let pendientes = p.st.db.notif_envios_debidos(ahora + 120, 100).unwrap();
    assert_eq!(pendientes.iter().filter(|e| e.tipo == "resumen").count(), 1, "el semanal, una vez (el diario no lo quiere nadie)");
}

#[test]
fn el_arreglo_no_llega_a_quien_ya_no_esta() {
    let p = servidor();
    p.informe(copia("fallo", "2026-10-04T06:00:00Z"));
    p.pasada(T0);
    assert_eq!(p.correos().len(), 2, "propietaria y técnico");
    // Al técnico lo quitan del cliente antes de que se arregle.
    let tom = p.st.db.cuenta_por_correo("tom@ejemplo.com").unwrap().unwrap();
    p.st.db.quitar_miembro(&tom.id, p.ctx.id()).unwrap();
    p.informe(copia("ok", "2026-10-04T07:00:00Z"));
    p.pasada(T0 + 3600);
    let para: Vec<String> = p.correos().into_iter().skip(2).map(|(a, _)| a).collect();
    assert_eq!(para, vec!["ana@ejemplo.com".to_string()]);
}

#[test]
fn equipo_que_vuelve_a_conectar() {
    let p = servidor();
    aviso_a(p.st.db.as_ref(), &p.ctx, Some("e1"), "equipo_sin_contacto", "«PC-Contabilidad» lleva más de 24 h sin conectar", T0 + 60).unwrap();
    p.pasada(T0 + 60);
    assert_eq!(p.correos().len(), 1);
    p.st.db.contacto_equipo(&p.ctx, "e1", T0 + 3600).unwrap();
    p.pasada(T0 + 3600 + 10);
    let c = p.correos();
    assert_eq!(c.len(), 2);
    assert!(c[1].1.contains("«PC-Contabilidad» volvió a conectar"), "{}", c[1].1);
}

#[test]
fn la_orden_destructiva_se_cierra_al_terminar_sin_volvio_a_funcionar() {
    let p = servidor();
    let orden = |seq: u64| {
        let o =
            p.st.db
                .insertar_orden(
                    &p.ctx,
                    &crate::almacen::OrdenNueva {
                        id: format!("o{seq}"),
                        equipo_id: "e1".into(),
                        tipo: "aplicar_retencion_almacen".into(),
                        seq,
                        sellado: "eA==".into(),
                        emitida_por: p.ana.clone(),
                        not_before: Some(T0 + 24 * 3600),
                        caduca: T0 + 6 * 24 * 3600,
                        sesion: None,
                        relevo: None,
                    },
                )
                .unwrap();
        aviso_a(p.st.db.as_ref(), &p.ctx, Some("e1"), "orden_destructiva", "Pendiente en «PC-Contabilidad»: aplicar_retencion_almacen", T0).unwrap();
        o
    };
    let clave = format!("{}|e1|orden_destructiva|", p.ctx.id());
    let abierto = || p.st.db.notif_incidente(&clave).unwrap().is_some_and(|i| i.abierto);
    let o1 = orden(1);
    p.pasada(T0);
    assert_eq!(p.correos().len(), 2, "propietaria y técnico");
    assert!(abierto());
    // Mientras sigue pendiente, sigue abierto.
    p.pasada(T0 + 60);
    assert!(abierto());
    // Se aplicó: se cierra, sin «Volvió a funcionar».
    let hecha = crate::almacen::ResultadoOrden { orden: o1.id.clone(), estado: "hecha".into(), mensaje: None, detalle: None, firma: "f".into() };
    assert!(p.st.db.resultado_orden(&p.ctx, "e1", &hecha).unwrap());
    // El aviso de la consola («puedes cancelarla…») sigue abierto mientras está pendiente…
    let pendientes = || p.st.db.avisos(&p.ctx, true).unwrap().into_iter().filter(|a| a.tipo == "orden_destructiva").count();
    assert_eq!(pendientes(), 1);
    p.pasada(T0 + 120);
    assert!(!abierto());
    // … y se cierra solo al aplicarse (queda en la lista, como visto por Resguardo).
    assert_eq!(pendientes(), 0);
    assert!(p.st.db.avisos(&p.ctx, false).unwrap().iter().any(|a| a.tipo == "orden_destructiva" && a.visto_por.as_deref() == Some(super::AVISO_RESUELTO)));
    assert_eq!(p.enviados().len(), 3, "los dos correos y el webhook del aviso, nada más");
    // Otra destructiva después: se avisa de nuevo (no es «lo mismo otra vez»).
    let o2 = orden(2);
    p.pasada(T0 + 180);
    assert_eq!(p.correos().len(), 4);
    assert!(abierto());
    // Cancelada: también se cierra; y una que caduca sin aplicarse, igual.
    assert!(p.st.db.cancelar_orden(&p.ctx, &o2.id, &p.ana, T0 + 200).unwrap());
    p.pasada(T0 + 240);
    assert!(!abierto());
    orden(3);
    p.pasada(T0 + 300);
    assert!(abierto());
    p.pasada(T0 + 7 * 24 * 3600);
    assert!(!abierto(), "caducó");
    let r = p.st.db.notif_registro(None, 100).unwrap();
    assert!(r.iter().all(|e| e.tipo != "recuperacion"), "{r:?}");
}

/// v1.32: los correos de un cliente con marca llevan su logo dentro (cid:) y su
/// acento; el webhook no cambia.
#[test]
fn correo_con_la_marca_del_cliente() {
    let p = servidor();
    // Un PNG mínimo (firma, IHDR de 40 × 20 y IEND), como lo deja la consola.
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
    png.extend_from_slice(b"IHDR");
    png.extend_from_slice(&40u32.to_be_bytes());
    png.extend_from_slice(&20u32.to_be_bytes());
    png.extend_from_slice(&[8, 6, 0, 0, 0, 1, 2, 3, 4]);
    png.extend_from_slice(&[0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82]);
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &png);
    p.st.db.poner_valor(&format!("marca:{}", p.ctx.id()), &json!({ "acento": "rose", "logo": b64, "huella": "0123456789abcdef" }).to_string()).unwrap();
    aviso_a(p.st.db.as_ref(), &p.ctx, Some("e1"), "intentos_fallidos", "5 intentos con la clave de administración mal", T0).unwrap();
    p.pasada(T0);
    let correos: Vec<(String, String)> = p
        .enviados()
        .into_iter()
        .filter_map(|e| match e {
            Enviado::Correo { html, crudo, .. } => Some((html, crudo)),
            _ => None,
        })
        .collect();
    assert_eq!(correos.len(), 2);
    for (html, crudo) in &correos {
        assert!(html.contains("cid:logo-cliente@resguardo") && html.contains("width=\"64\" height=\"32\""), "{html}");
        assert!(html.contains("&nbsp;Ferretería Altamar</strong>") && html.contains("border-top:3px solid #d6336c;"), "{html}");
        assert!(crudo.contains("multipart/related") && crudo.contains("Content-ID: <logo-cliente@resguardo>"), "{crudo}");
        assert!(!html.contains("src=\"http"));
    }
    // El webhook, igual que siempre (sin imagen).
    let w = p.webhooks();
    assert_eq!(w.len(), 1);
    assert!(!w[0].to_string().contains("cid:") && !w[0].to_string().contains(&b64));
}
