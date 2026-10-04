//! Pruebas de propiedades (proptest) de lo que el servidor recibe de fuera:
//! el progreso y el historial que mandan los agentes, el contenido de una copia
//! de la consola al restaurarla, fechas, correos, códigos TOTP e IP. Con
//! cualquier entrada: un error o un resultado acotado, nunca un `panic`.

use proptest::prelude::*;
use serde_json::Value;

/// Un JSON cualquiera, con los nombres de campo que se esperan de vez en cuando.
fn json() -> impl Strategy<Value = Value> {
    let hoja = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::from),
        any::<i64>().prop_map(Value::from),
        any::<u64>().prop_map(Value::from),
        any::<f64>().prop_map(Value::from),
        ".{0,40}".prop_map(Value::from),
        "[\\x00-\\x1f\\u{80}-\\u{9f}a-z]{0,300}".prop_map(Value::from),
        prop::sample::select(vec!["copia", "subiendo", "aviso", "copia_fallida", "2026-10-04T10:00:00+02:00", "r1", "abc-123"]).prop_map(Value::from),
    ];
    hoja.prop_recursive(3, 48, 8, |dentro| {
        prop_oneof![
            prop::collection::vec(dentro.clone(), 0..10).prop_map(Value::Array),
            prop::collection::btree_map(
                prop_oneof![
                    prop::sample::select(vec![
                        "tipo",
                        "fase",
                        "repo",
                        "copia",
                        "nombre",
                        "etapa",
                        "porcentaje",
                        "archivos",
                        "bytes",
                        "id",
                        "hora",
                        "aviso",
                        "mensaje",
                        "ruta"
                    ])
                    .prop_map(String::from),
                    "[a-z_]{0,8}",
                ],
                dentro,
                0..8
            )
            .prop_map(|m| Value::Object(m.into_iter().collect())),
        ]
    })
}

/// Una entrada de historial que a veces está bien formada.
fn entrada() -> impl Strategy<Value = Value> {
    prop_oneof![
        json(),
        (
            prop_oneof!["[a-zA-Z0-9-]{1,64}", ".{0,70}"],
            prop_oneof![
                (946_684_800i64..4_102_444_800).prop_map(|t| chrono::DateTime::from_timestamp(t, 0).unwrap().to_rfc3339()),
                ".{0,30}"
            ],
            prop::sample::select(crate::agentes::TIPOS_HISTORIAL.iter().map(|s| s.to_string()).chain(["otro".into()]).collect::<Vec<_>>()),
            prop_oneof![Just("copia_fallida".to_string()), ".{0,15}"],
            ".{0,700}",
            json(),
        )
            .prop_map(|(id, hora, tipo, aviso, mensaje, extra)| serde_json::json!({ "id": id, "hora": hora, "tipo": tipo, "aviso": aviso, "mensaje": mensaje, "extra": extra })),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// El progreso de un agente, sea el que sea: o error, o tareas con solo los
    /// campos conocidos, textos cortos sin controles y números finitos.
    #[test]
    fn progreso_limpio(v in json()) {
        if let Ok(tareas) = crate::progreso::limpiar(&v) {
            prop_assert!(tareas.len() <= crate::progreso::MAX_TAREAS);
            for t in tareas {
                let o = t.as_object().expect("objeto");
                for (k, x) in o {
                    match x {
                        Value::String(s) => {
                            prop_assert!(s.chars().count() <= 120, "{}", k);
                            prop_assert!(!s.chars().any(char::is_control), "{}", k);
                        }
                        Value::Number(n) => prop_assert!(n.as_u64().is_some() || n.as_f64().is_some_and(|f| (0.0..=1.0).contains(&f)), "{}", k),
                        _ => prop_assert!(false, "campo {} con {}", k, x),
                    }
                }
                prop_assert!(o.contains_key("repo"));
                prop_assert!(!o.contains_key("ruta"));
            }
        }
    }

    /// Una entrada de historial aceptada cumple lo que se guarda: id corto,
    /// hora razonable, tipo conocido, tamaño acotado y avisos de un tipo conocido.
    #[test]
    fn historial_acotado(v in entrada()) {
        let ahora = crate::almacen::ahora();
        if let Some(e) = crate::agentes::entrada_historial(&v, ahora) {
            prop_assert!(!e.id.is_empty() && e.id.len() <= 64 && e.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
            prop_assert!(e.hora >= 946_684_800 && e.hora <= ahora + 86_400);
            prop_assert!(crate::agentes::TIPOS_HISTORIAL.contains(&e.tipo.as_str()));
            prop_assert!(e.datos.len() <= 4 * 1024);
            if let Some((tipo, mensaje)) = &e.aviso {
                prop_assert_eq!(e.tipo.as_str(), "aviso");
                prop_assert!(!tipo.is_empty());
                prop_assert!(mensaje.chars().count() <= 500 && !mensaje.chars().any(char::is_control));
            }
        }
    }

    /// Fechas, correos, contraseñas, TOTP e IP cualesquiera.
    #[test]
    fn textos_de_fuera(s in ".{0,80}", ts in any::<i64>(), ultimo in any::<i64>(), ahora in any::<i64>(), ip in any::<std::net::IpAddr>()) {
        let _ = crate::api::de_fecha(&s);
        let _ = crate::api::fecha(ts);
        let _ = crate::auth::valida_correo(&s);
        let _ = crate::auth::valida_contrasena(&s);
        let _ = crate::auth::de_base32(&s);
        let _ = crate::auth::comprueba_totp(&s, &s, ultimo, ahora);
        let _ = crate::auth::comprueba_totp(&crate::auth::nuevo_secreto_totp(), &s, ultimo, ahora);
        let _ = crate::estado::clave_ip(Some(ip));
        let _ = crate::auth::uri_totp(&s, &s);
    }
}

// ---------- Contenido de una copia de la consola al restaurarla ----------

/// Una entrada del contenido: ruta (válida o no), largo que dice y datos que trae.
fn entrada_copia() -> impl Strategy<Value = (String, u64, Vec<u8>)> {
    (
        prop_oneof![
            prop::sample::select(vec!["control.db", "identidad.key", "respaldo-consola.json", "clientes/a.db", "tls/ca.crt", "paquetes/x.bin"])
                .prop_map(String::from),
            prop::sample::select(vec![
                "../fuera",
                "clientes/../../fuera",
                "/etc/passwd",
                "C:\\Windows\\x",
                "tls/",
                "otra/x",
                "clientes/.oculto",
                "clientes/a/b"
            ])
            .prop_map(String::from),
            ".{0,40}",
        ],
        prop_oneof![0u64..64, any::<u64>()],
        prop::collection::vec(any::<u8>(), 0..64),
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// El contenido descifrado de una copia, troceado como sea: nunca escribe
    /// fuera de la carpeta de destino ni algo que no sea una ruta permitida.
    #[test]
    fn desempaquetar_no_sale_de_su_carpeta(
        entradas in prop::collection::vec(entrada_copia(), 0..6),
        basura in prop::collection::vec(any::<u8>(), 0..64),
        trozo in 1usize..50,
        fin in any::<bool>(),
    ) {
        let mut datos = Vec::new();
        for (ruta, largo, contenido) in &entradas {
            datos.extend_from_slice(&(ruta.len().min(u16::MAX as usize) as u16).to_be_bytes());
            datos.extend_from_slice(ruta.as_bytes());
            datos.extend_from_slice(&largo.to_be_bytes());
            datos.extend_from_slice(contenido);
        }
        if fin {
            datos.extend_from_slice(&0u16.to_be_bytes());
        }
        datos.extend_from_slice(&basura);
        let base = tempfile::tempdir().unwrap();
        let destino = base.path().join("destino");
        std::fs::create_dir_all(&destino).unwrap();
        let _ = crate::respaldo::desempaquetar_para_pruebas(&destino, datos.chunks(trozo));
        // Nada fuera de `destino`…
        let fuera: Vec<_> = std::fs::read_dir(base.path()).unwrap().flatten().map(|e| e.file_name()).collect();
        prop_assert_eq!(fuera, vec![std::ffi::OsString::from("destino")]);
        // …y dentro, solo rutas permitidas.
        let mut pendientes = vec![destino.clone()];
        while let Some(d) = pendientes.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    pendientes.push(p);
                } else {
                    let rel = p.strip_prefix(&destino).unwrap().to_string_lossy().replace('\\', "/");
                    prop_assert!(crate::respaldo::ruta_valida(&rel), "{}", rel);
                }
            }
        }
    }
}
