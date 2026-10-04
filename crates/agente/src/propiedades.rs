//! Pruebas de propiedades (proptest) de las rutas que llegan al agente en las
//! órdenes y sesiones (de la consola, a través del servidor): dónde restaurar,
//! la ruta de un repositorio al adoptarlo y las carpetas del almacén. Lo que
//! se acepta nunca sale de donde debe (`..`, otra unidad, una opción…).

use proptest::prelude::*;

/// Rutas raras: con `..`, `\`, `:`, unidades, recursos de red, guiones, NUL…
fn ruta() -> impl Strategy<Value = String> {
    let trozo = prop_oneof![
        "[a-zA-Z0-9 _.-]{0,8}",
        prop::sample::select(vec!["..", ".", "", "-x", "--help", "C:", "c:\\", "\\\\equipo\\c$", "a:b", "x\0y", "CON", "~"]).prop_map(String::from),
        ".{0,6}",
    ];
    (prop::option::of(prop::sample::select(vec!["/", "//", "\\", "C:\\"])), prop::collection::vec(trozo, 0..6))
        .prop_map(|(inicio, partes)| format!("{}{}", inicio.unwrap_or(""), partes.join("/")))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// La ruta de una versión como ruta del equipo: si se acepta, es absoluta y
    /// sin `..` (ni `.`), en Windows en una unidad local (nada de `\\equipo\…`).
    #[test]
    fn ruta_local_no_sale(r in ruta()) {
        if let Ok(l) = crate::sesiones_v2::ruta_local(&r) {
            if cfg!(windows) {
                let b = l.as_bytes();
                prop_assert!(b.len() >= 3 && b[0].is_ascii_alphabetic() && &b[1..3] == b":\\", "{}", l);
                prop_assert!(!l[3..].contains(':') && !l.contains('/'), "{}", l);
                prop_assert!(!l[3..].split('\\').any(|c| c == ".." || c == "."), "{}", l);
            } else {
                prop_assert!(l.starts_with('/') && !l.contains('\0'), "{}", l);
                prop_assert!(!l.split('/').any(|c| c == ".." || c == "."), "{}", l);
            }
        }
    }

    /// La ruta de un repositorio dentro de su destino (adoptar): sin `..`, `\`,
    /// `:` ni nada que parezca una opción o cambie de destino.
    #[test]
    fn ruta_de_repositorio_al_adoptar(r in ruta()) {
        if crate::adoptar_v2::ruta_valida(&r) && !r.is_empty() {
            prop_assert!(r.chars().count() <= 200);
            prop_assert!(!r.contains(['\\', ':', '@', '?', '#', '%']));
            prop_assert!(!r.chars().any(char::is_control));
            for s in r.split('/') {
                prop_assert!(!s.trim().is_empty() && s != "." && s != ".." && !s.starts_with('-'), "{}", r);
            }
        }
    }

    /// Las carpetas del almacén (usuario y repositorio): un nombre suelto, nunca
    /// un camino.
    #[test]
    fn carpetas_del_almacen(s in ruta()) {
        if crate::retencion_almacen::segmento_valido(&s) {
            prop_assert!(s != "." && s != ".." && !s.is_empty());
            prop_assert!(!s.contains(['/', '\\', ':', '\0']));
        }
        let base = std::path::Path::new("/no-existe-resguardo");
        if let Ok(p) = crate::retencion_almacen::carpeta_repo(base, &["ana".to_string()], "ana", &s) {
            prop_assert!(p.starts_with(base));
            prop_assert!(!p.components().any(|c| matches!(c, std::path::Component::ParentDir)));
        }
    }
}
