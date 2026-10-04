//! Pruebas de propiedades (proptest) de todo lo del protocolo que llega de
//! fuera: sobres sellados, mensajes firmados, órdenes v2, cifrados simétricos,
//! paquetes de cliente, la cola del instalador y la copia de la consola.
//!
//! Lo que se exige: con cualquier entrada, un error y nunca un `panic`; sin
//! reservar memoria por lo que diga la entrada (las longitudes se comprueban
//! antes); lo bien formado, ida y vuelta; y lo tocado o recortado, rechazado.

use base64::Engine;
use proptest::prelude::*;
use resguardo_protocolo::orden_v2::{self, Autorizacion, Contexto, OrdenV2};
use resguardo_protocolo::{cifrado, claves, derivaciones, instalador, mensajes, ordenes, paquete, respaldo_consola as rc, simetrico};
use serde_json::Value;
use std::io::Write;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
/// Las claves de vectors/v1.json (como en fuzz/).
const SECRETO: &str = "CQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQk=";
const CONSOLA_PUB: &str = "6kpsY+KcUgq+9VB7Ey7F+ZVHdq6+vnuSQh7qaRRG0iw=";

/// Un JSON cualquiera (anidado, con números raros y textos con lo que sea).
fn json() -> impl Strategy<Value = Value> {
    let hoja = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::from),
        any::<i64>().prop_map(Value::from),
        any::<u64>().prop_map(Value::from),
        any::<f64>().prop_map(Value::from),
        ".{0,24}".prop_map(Value::from),
    ];
    hoja.prop_recursive(4, 48, 6, |dentro| {
        prop_oneof![
            prop::collection::vec(dentro.clone(), 0..6).prop_map(Value::Array),
            prop::collection::btree_map("[a-z_]{0,10}", dentro, 0..6).prop_map(|m| Value::Object(m.into_iter().collect())),
        ]
    })
}

/// Una fecha RFC 3339 cerca de `ahora` (o un texto cualquiera).
fn fecha(ahora: i64) -> impl Strategy<Value = String> {
    prop_oneof![
        4 => (-30 * 86_400i64..30 * 86_400).prop_map(move |d| chrono::DateTime::from_timestamp(ahora + d, 0).unwrap().to_rfc3339()),
        1 => ".{0,30}",
    ]
}

fn tipo_orden() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => prop::sample::select(ordenes::TIPOS.iter().map(|t| t.nombre.to_string()).collect::<Vec<_>>()),
        1 => "[a-z_]{0,20}",
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Sobres y mensajes firmados: cualquier texto o byte, un error y nada más.
    #[test]
    fn sobres_y_mensajes_cualesquiera(datos in prop::collection::vec(any::<u8>(), 0..2048), texto in ".{0,256}", ultimo in any::<u64>()) {
        let _ = claves::open_bytes(SECRETO, &texto);
        let _ = claves::open_bytes(&texto, SECRETO);
        let _ = claves::public_of(&texto);
        let _ = claves::seal_bytes(&texto, &datos);
        let _ = mensajes::open_message(&texto, SECRETO, CONSOLA_PUB, "equipo-1", ultimo, chrono::Local::now());
        prop_assert!(mensajes::verify_signed(&datos, CONSOLA_PUB, "equipo-1", ultimo, chrono::Local::now()).is_err());
        let _ = mensajes::verify_signed(&datos, &texto, "equipo-1", ultimo, chrono::Local::now());
        let _ = mensajes::code_hash(&texto);
        let _ = derivaciones::huella_para_sas(&texto);
        let _ = derivaciones::sas_v3(&texto, &texto, &texto, &texto);
        // El sobre abierto con base64 cualquiera.
        let _ = claves::open_bytes(SECRETO, &B64.encode(&datos));
    }

    /// Una orden sellada para otro, o un sobre cualquiera, no se abre.
    #[test]
    fn ordenes_selladas_cualesquiera(datos in prop::collection::vec(any::<u8>(), 0..4096), ultimo in any::<u64>(), seq in any::<u64>()) {
        let cx = Contexto { cliente: "c", equipo: "e", ultimo_seq: ultimo, ahora: chrono::Utc::now().timestamp(), tipo_meta: "copiar_ahora", seq_meta: seq };
        prop_assert!(orden_v2::validar(&datos, &cx).is_err() || serde_json::from_slice::<OrdenV2>(&datos).is_ok());
        prop_assert!(orden_v2::abrir(&B64.encode(&datos), SECRETO, &cx).is_err());
    }

    /// Una orden bien formada con campos cualesquiera: si se acepta, cumple todo
    /// lo que el equipo exige (para él, `seq` creciente sin saltos enormes, los
    /// metadatos del servidor, sin caducar, con su espera si es destructiva).
    #[test]
    fn ordenes_aceptadas_cumplen_las_reglas(
        v in prop_oneof![9 => Just(2u32), 1 => any::<u32>()],
        cliente in prop_oneof![4 => Just("c".to_string()), 1 => "[a-z]{0,3}"],
        equipo in prop_oneof![4 => Just("e".to_string()), 1 => "[a-z]{0,3}"],
        ultimo in prop_oneof![0u64..10, any::<u64>()],
        salto in prop_oneof![1u64..5, any::<u64>()],
        nonce in prop_oneof![4 => "[a-zA-Z0-9+/=]{0,24}", 1 => ".{0,80}"],
        tipo in tipo_orden(),
        (emitida, caduca, not_before) in {
            let ahora = chrono::Utc::now().timestamp();
            let rfc = |t: i64| chrono::DateTime::from_timestamp(t, 0).unwrap().to_rfc3339();
            prop_oneof![
                // Casi todas cerca de lo que se acepta (emitida hace un rato, caduca en unos días).
                4 => (-7 * 86_400i64..900, -3_600i64..9 * 86_400, prop::option::of(-3_600i64..3 * 86_400))
                    .prop_map(move |(e, c, nb)| (rfc(ahora + e), rfc(ahora + e + c), nb.map(|n| rfc(ahora + n)))),
                1 => (fecha(ahora), fecha(ahora), prop::option::of(fecha(ahora))),
            ]
        },
        cuerpo in json(),
        meta_igual in prop::bool::weighted(0.8),
    ) {
        let ahora = chrono::Utc::now().timestamp();
        let seq = ultimo.wrapping_add(salto);
        let o = OrdenV2 { v, cliente, equipo, seq, nonce, emitida, caduca, not_before, tipo: tipo.clone(), cuerpo, autorizacion: Autorizacion::default(), responder_a: None };
        let tipo_meta = if meta_igual { tipo.clone() } else { "copiar_ahora".to_string() };
        let cx = Contexto { cliente: "c", equipo: "e", ultimo_seq: ultimo, ahora, tipo_meta: &tipo_meta, seq_meta: seq };
        let plano = serde_json::to_vec(&o).unwrap();
        if let Ok(a) = orden_v2::validar(&plano, &cx) {
            prop_assert_eq!(a.v, 2);
            prop_assert!(a.cliente == "c" && a.equipo == "e");
            prop_assert!(a.seq > ultimo && a.seq - ultimo <= orden_v2::MAX_SALTO_SEQ);
            prop_assert_eq!(&a.tipo, &tipo_meta);
            let t = ordenes::tipo(&a.tipo).expect("tipo conocido");
            let ts = |s: &str| chrono::DateTime::parse_from_rfc3339(s).unwrap().timestamp();
            prop_assert!(ts(&a.caduca) > ahora);
            prop_assert!(ts(&a.emitida) <= ahora + orden_v2::HOLGURA_S);
            prop_assert!(!t.destructiva || a.not_before.is_some());
            prop_assert!(a.nonce.len() <= 64);
        }
        // Sellada para el equipo, igual.
        let pubk = claves::public_of(SECRETO).unwrap();
        let sellada = orden_v2::sellar(&o, &pubk).unwrap();
        prop_assert_eq!(orden_v2::abrir(&sellada, SECRETO, &cx).is_ok(), orden_v2::validar(&plano, &cx).is_ok());
    }

    /// Cifrado simétrico (sesiones, relé, configuración y `K_cfg`): ida y vuelta,
    /// y nada tocado se acepta.
    #[test]
    fn simetrico_ida_y_vuelta_y_alterado(clave in any::<[u8; 32]>(), datos in prop::collection::vec(any::<u8>(), 0..1024), aad in ".{0,40}", n in any::<u64>(), ultimo in any::<bool>(), pos in any::<prop::sample::Index>(), basura in prop::collection::vec(any::<u8>(), 0..128)) {
        let nonce = simetrico::nonce_aleatorio();
        let c = simetrico::cifrar_trozo(&clave, &aad, n, ultimo, &datos, &nonce);
        prop_assert_eq!(&simetrico::descifrar_trozo(&clave, &aad, n, ultimo, &c).unwrap(), &datos);
        prop_assert!(simetrico::descifrar_trozo(&clave, &aad, n, !ultimo, &c).is_err(), "el último lo dice");
        prop_assert!(simetrico::descifrar_trozo(&clave, &aad, n.wrapping_add(1), ultimo, &c).is_err());
        let mut tocado = c.clone();
        let i = pos.index(tocado.len());
        tocado[i] ^= 0x01;
        prop_assert!(simetrico::descifrar_trozo(&clave, &aad, n, ultimo, &tocado).is_err());
        prop_assert!(simetrico::descifrar_trozo(&clave, &aad, n, ultimo, &c[..i]).is_err());
        let _ = simetrico::descifrar_con(&clave, &basura, aad.as_bytes());
        let _ = simetrico::descifrar_mensaje(&clave, &aad, &basura);
        // `K_cfg` (secretbox en base64).
        let b = cifrado::cifrar(&clave, &datos).unwrap();
        prop_assert_eq!(&cifrado::descifrar(&clave, &b).unwrap(), &datos);
        let _ = cifrado::descifrar(&clave, &aad);
        let _ = cifrado::descifrar(&clave, &B64.encode(&basura));
    }

    /// Paquete de cliente: cualquier archivo, un error; el bueno, ida y vuelta;
    /// recortado por donde sea, rechazado.
    #[test]
    fn paquete_de_cliente(clave in any::<[u8; 32]>(), json in prop::collection::vec(any::<u8>(), 0..4096), basura in prop::collection::vec(any::<u8>(), 0..512), corte in any::<prop::sample::Index>()) {
        let _ = paquete::descifrar(&clave, &basura);
        let mut con_magia = paquete::MAGIA.to_vec();
        con_magia.extend_from_slice(&basura);
        let _ = paquete::descifrar(&clave, &con_magia);
        let _ = paquete::cabecera(&con_magia);
        let p = paquete::cifrar(&clave, "BAQEBAQEBAQEBAQEBAQEBA==", &json);
        let (sal, dentro) = paquete::descifrar(&clave, &p).unwrap();
        prop_assert_eq!(sal.as_str(), "BAQEBAQEBAQEBAQEBAQEBA==");
        prop_assert_eq!(&dentro, &json);
        let i = corte.index(p.len());
        prop_assert!(paquete::descifrar(&clave, &p[..i]).is_err());
    }

    /// Cola del instalador: cualquier final de archivo, un error o nada; una
    /// cola buena detrás de cualquier instalador se lee entera; lo que se lee
    /// siempre está validado.
    #[test]
    fn cola_del_instalador(
        basura in prop::collection::vec(any::<u8>(), 0..6000),
        servidor in prop_oneof![Just("https://192.168.1.20:8443".to_string()), "https://[a-z0-9.:-]{1,30}", ".{0,40}"],
        cliente in prop_oneof![Just("0a0e1b2c-0000-4000-8000-0000000000a1".to_string()), ".{0,70}"],
        nombre in ".{0,90}",
        codigo in prop_oneof![Just("ABCD-EFGH-JK".to_string()), ".{0,25}"],
        pos in any::<prop::sample::Index>(),
    ) {
        if let Ok(Some(d)) = instalador::leer_cola(&basura) {
            prop_assert!(d.validar().is_ok());
        }
        let d = instalador::DatosInstalador { v: 1, servidor, huella_ca: vec!["AB"; 32].join(":"), cliente, nombre, codigo };
        if let Ok(cola) = instalador::cola(&d) {
            let mut archivo = basura.clone();
            archivo.extend_from_slice(&cola);
            prop_assert_eq!(instalador::leer_cola(&archivo).unwrap(), Some(d.clone()));
            let fin = &archivo[archivo.len().saturating_sub(instalador::LEER_DEL_FINAL)..];
            prop_assert_eq!(instalador::leer_cola(fin).unwrap(), Some(d));
            // Un byte cambiado: o se rechaza o lo leído sigue validado.
            let i = archivo.len() - cola.len() + pos.index(cola.len());
            archivo[i] ^= 0x20;
            if let Ok(Some(otra)) = instalador::leer_cola(&archivo) {
                prop_assert!(otra.validar().is_ok());
            }
        } else {
            prop_assert!(d.validar().is_err());
        }
    }
}

// ---------- Copia de la consola ----------

fn secreto_prueba() -> [u8; 32] {
    [5u8; 32]
}

fn copia(datos: &[u8]) -> Vec<u8> {
    let id = ed25519_dalek::SigningKey::from_bytes(&[42u8; 32]);
    let mut c =
        rc::Cifrador::nuevo(Vec::new(), &B64.encode([1u8; 16]), &rc::publica_de(&secreto_prueba()), "2026-10-04T03:30:00+02:00", &id, "0.7.11").unwrap();
    c.write_all(datos).unwrap();
    c.terminar().unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Cualquier archivo: un error, nunca un `panic` (con la magia buena o no).
    #[test]
    fn copia_de_la_consola_cualquiera(basura in prop::collection::vec(any::<u8>(), 0..4096), v2 in any::<bool>()) {
        let mut archivo = if v2 { rc::MAGIA.to_vec() } else { rc::MAGIA_V1.to_vec() };
        archivo.extend_from_slice(&basura);
        prop_assert!(rc::comprobar(&archivo[..]).is_err());
        prop_assert!(rc::descifrar(&archivo[..], &secreto_prueba(), &mut Vec::new()).is_err());
        let _ = rc::comprobar(&basura[..]);
        let _ = rc::leer_cabecera(&mut &basura[..]);
        let _ = rc::huella(&String::from_utf8_lossy(&basura));
        let _ = rc::coincide_huella(&String::from_utf8_lossy(&basura), &String::from_utf8_lossy(&basura));
    }

    /// Ida y vuelta con cualquier contenido; y recortada por donde sea, o con
    /// cualquier byte cambiado, ni se comprueba ni se descifra.
    #[test]
    fn copia_de_la_consola_ida_y_vuelta_y_alterada(datos in prop::collection::vec(any::<u8>(), 0..20_000), pos in any::<prop::sample::Index>(), bit in 0u8..8) {
        let archivo = copia(&datos);
        let mut salida = Vec::new();
        let c = rc::descifrar(&archivo[..], &secreto_prueba(), &mut salida).unwrap();
        prop_assert!(c.firmada);
        prop_assert_eq!(&salida, &datos);
        prop_assert_eq!(rc::comprobar(&archivo[..]).unwrap(), c);
        let i = pos.index(archivo.len());
        prop_assert!(rc::comprobar(&archivo[..i]).is_err(), "recortada en {}", i);
        prop_assert!(rc::descifrar(&archivo[..i], &secreto_prueba(), &mut Vec::new()).is_err());
        let mut tocada = archivo.clone();
        tocada[i] ^= 1 << bit;
        prop_assert!(rc::comprobar(&tocada[..]).is_err(), "byte {} cambiado", i);
        prop_assert!(rc::descifrar(&tocada[..], &secreto_prueba(), &mut Vec::new()).is_err());
    }

    /// Una longitud de trozo enorme no reserva memoria: se rechaza al leerla.
    #[test]
    fn copia_de_la_consola_con_trozo_enorme(largo in (5u32 * 1024 * 1024)..u32::MAX) {
        let archivo = copia(b"x");
        let fin = rc::MAGIA.len() + archivo[rc::MAGIA.len()..].iter().position(|b| *b == b'\n').unwrap() + 1;
        let mut mala = archivo[..fin].to_vec();
        mala.extend_from_slice(&largo.to_be_bytes());
        mala.extend_from_slice(&[0u8; 64]);
        let e = rc::comprobar(&mala[..]).unwrap_err();
        prop_assert!(e.contains("demasiado grande") || e.contains("incompleta"), "{}", e);
    }
}
