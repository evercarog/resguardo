//! Pruebas de propiedades (proptest) de lo que el motor lee de fuera: la
//! salida de restic (nombres, tamaños y mensajes que vienen del repositorio,
//! que puede ser de otro) y los horarios (llegan en la configuración).
//! Con cualquier entrada: un resultado o un error, nunca un `panic` ni un
//! desbordamiento.

use crate::plans::{PlanSchedule, ScheduleRule};
use crate::{restic, retention, sizes};
use chrono::{FixedOffset, Local, TimeZone};
use proptest::prelude::*;

fn hora() -> impl Strategy<Value = String> {
    prop_oneof![4 => (0u32..24, 0u32..60).prop_map(|(h, m)| format!("{h:02}:{m:02}")), 1 => ".{0,8}"]
}

fn fecha() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => (1990i32..2100, 1u32..13, 1u32..29).prop_map(|(y, m, d)| format!("{y:04}-{m:02}-{d:02}")),
        // Años enormes o negativos (lo más lejos que admite el calendario).
        1 => ("[+-]?[0-9]{1,6}", 1u32..13, 1u32..29).prop_map(|(y, m, d)| format!("{y}-{m:02}-{d:02}")),
        1 => ".{0,12}",
    ]
}

fn dias() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(prop_oneof![9 => 0u8..7, 1 => any::<u8>()], 0..9)
}

fn regla() -> impl Strategy<Value = ScheduleRule> {
    prop_oneof![
        (dias(), prop::collection::vec(hora(), 0..6)).prop_map(|(days, times)| ScheduleRule::At { days, times }),
        (dias(), prop_oneof![prop::sample::select(vec![5u32, 10, 15, 20, 30, 60, 120, 1440]), any::<u32>()], hora(), hora())
            .prop_map(|(days, every_min, from, to)| ScheduleRule::Every { days, every_min, from, to }),
        (prop_oneof![1u32..400, any::<u32>()], fecha(), hora()).prop_map(|(every, start, time)| ScheduleRule::EveryDays { every, start, time }),
        (any::<i8>(), hora()).prop_map(|(day, time)| ScheduleRule::Monthly { day, time }),
    ]
}

fn horario() -> impl Strategy<Value = PlanSchedule> {
    prop_oneof![
        prop::collection::vec(regla(), 0..4).prop_map(PlanSchedule::from_rules),
        (dias(), prop::sample::select(vec!["at", "every", "otro"]), prop::collection::vec(hora(), 0..4), any::<u32>(), hora(), hora())
            .prop_map(|(days, mode, times, every_hours, from, to)| PlanSchedule { days, mode: mode.into(), times, every_hours, from, to, rules: vec![] }),
    ]
}

/// Un momento entre 2000 y 2100, en una zona horaria fija cualquiera.
fn momento() -> impl Strategy<Value = chrono::DateTime<FixedOffset>> {
    (946_684_800i64..4_102_444_800, -12i32..14).prop_map(|(t, h)| FixedOffset::east_opt(h * 3600).unwrap().timestamp_opt(t, 0).unwrap())
}

/// Un JSON cualquiera en una línea (lo que restic podría escribir con otro repositorio u otra versión).
fn linea_json() -> impl Strategy<Value = String> {
    let hoja = prop_oneof![
        Just(serde_json::Value::Null),
        any::<i64>().prop_map(serde_json::Value::from),
        any::<u64>().prop_map(serde_json::Value::from),
        any::<f64>().prop_map(serde_json::Value::from),
        ".{0,20}".prop_map(serde_json::Value::from),
        prop::sample::select(vec!["file", "dir", "node", "summary", "status", "/", "/C/Users"]).prop_map(serde_json::Value::from),
    ];
    hoja.prop_recursive(3, 32, 6, |dentro| {
        prop_oneof![
            prop::collection::vec(dentro.clone(), 0..5).prop_map(serde_json::Value::Array),
            prop::collection::btree_map(
                prop::sample::select(vec!["path", "name", "type", "size", "struct_type", "message_type", "id", "time", "keep", "remove", "reasons"])
                    .prop_map(String::from),
                dentro,
                0..6
            )
            .prop_map(|m| serde_json::Value::Object(m.into_iter().collect())),
        ]
    })
    .prop_map(|v| v.to_string())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Cualquier horario (válido o no) y cualquier momento: la última vez ya
    /// pasó, la próxima aún no, y nada hace `panic`.
    #[test]
    fn horarios_cualesquiera(h in horario(), ahora in momento(), desde in momento()) {
        let valido = h.validate().is_ok();
        if let Some(u) = h.latest_slot(ahora) {
            prop_assert!(u <= ahora);
        }
        if let Some(p) = h.next_slot(ahora) {
            prop_assert!(p > ahora);
        }
        let _ = h.is_due(desde, ahora);
        let _ = h.times_of_day();
        let horas = h.max_gap_hours();
        prop_assert!(horas >= 1);
        // Con la zona del equipo (cambios de hora incluidos).
        let local = Local.timestamp_opt(ahora.timestamp(), 0).unwrap();
        let _ = (h.latest_slot(local), h.next_slot(local));
        // Un horario válido de reglas por días de la semana siempre tiene próxima vez.
        if valido && h.effective_rules().iter().all(|r| matches!(r, ScheduleRule::At { .. } | ScheduleRule::Every { .. })) {
            prop_assert!(h.next_slot(ahora).is_some());
        }
    }

    /// La salida de restic, sea la que sea: un resultado o un error.
    #[test]
    fn salida_de_restic_cualquiera(lineas in prop::collection::vec(prop_oneof![linea_json(), ".{0,80}"], 0..12), bytes in prop::collection::vec(any::<u8>(), 0..512)) {
        let texto = lineas.join("\n");
        for salida in [texto.as_bytes(), &bytes[..]] {
            let _ = restic::parse_stats(salida);
            let _ = restic::parse_snapshots(salida);
            let _ = retention::grupos_de_forget(salida).map(|g| g.len());
            for dir in ["/", "/C/Users", "/home/ana"] {
                // Solo los hijos directos de la carpeta pedida.
                for e in restic::parse_ls(salida, dir) {
                    let prefijo = if dir == "/" { "/".to_string() } else { format!("{dir}/") };
                    let resto = e.path.strip_prefix(&prefijo).expect("dentro de la carpeta");
                    prop_assert!(!resto.is_empty() && !resto.contains('/'));
                }
            }
        }
        for l in &lineas {
            let _ = sizes::archivo_de_ls(l);
        }
    }

    /// «Lo que más ocupa» con cualquier lista: sin desbordar (los tamaños los
    /// pone el repositorio) y con el tope pedido.
    #[test]
    fn lo_que_mas_ocupa(items in prop::collection::vec((prop_oneof!["(/[a-zA-Z0-9 ]{0,6}){0,5}", ".{0,20}"], any::<u64>()), 0..64), limite in 1usize..50) {
        let n = items.len() as u64;
        let r = sizes::summarize(items.into_iter(), limite);
        prop_assert!(r.files.len() <= limite && r.folders.len() <= limite);
        prop_assert_eq!(r.total_files, n);
    }

    /// Mensajes de error de restic (stderr): cualquier texto, sin `panic`; las
    /// direcciones con contraseña nunca la enseñan.
    #[test]
    fn mensajes_de_restic(
        stderr in prop_oneof![
            ".{0,200}",
            (any::<u64>(), any::<u64>()).prop_map(|(h, m)| format!("Fatal: unable to create lock\nlock was created at 2026-01-01 ({h}h{m}m3s ago)\nlocked by PID {h} on EQUIPO by ana")),
        ],
        codigo in prop::option::of(any::<i32>()),
        clave in "[a-zA-Z0-9]{4,12}",
        texto in ".{0,60}",
    ) {
        let _ = restic::exit_error(codigo, &stderr);
        let con_clave = format!("{texto} rest:https://ana:{clave}@servidor:8000/ana/ {texto}");
        let limpio = restic::redact_credentials(&con_clave);
        prop_assert!(!limpio.contains(&format!(":{clave}@")), "{}", limpio);
        let _ = restic::redact_credentials(&texto);
        let _ = restic::has_embedded_password(&texto);
        let _ = restic::ruta_en_version(&texto);
        let _ = restic::valid_snapshot_id(&texto);
        // Una carpeta dentro de una versión aceptada es absoluta y sin «..».
        if let Ok(d) = restic::snapshot_dir(&texto) {
            prop_assert!(d.starts_with('/') && !d.contains('\0') && !d.split('/').any(|s| s == ".."));
        }
        if restic::valid_snapshot_id(&texto) {
            prop_assert!(!texto.starts_with('-'));
        }
    }
}
