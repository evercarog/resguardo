//! Instalador «listo para vincular» (docs/api-servidor.md, v1.17).
//!
//! Resguardo Server añade al final del instalador genérico del agente una
//! cola con lo necesario para vincularlo sin escribir nada: la dirección del
//! servidor, la huella de su autoridad TLS, el cliente, el nombre del equipo
//! y un código de emparejamiento de un solo uso (24 h). Nada más: ninguna
//! clave ni contraseña. El emparejamiento sigue pidiendo comparar el número
//! de comprobación (SAS) en la consola.
//!
//! Formato (al final del archivo, después de los datos del instalador NSIS,
//! que no los lee):
//!
//! ```text
//! "RESGUARDO-COLA-1" ‖ u32 big-endian (n) ‖ JSON (n bytes, UTF-8) ‖ u32 big-endian (n) ‖ "RESGUARDO-FIN-01"
//! ```
//!
//! La longitud va dos veces (delante, para leerlo de corrido; detrás, para
//! encontrarlo desde el final) y tiene que coincidir. Como mucho 4 KiB de
//! JSON. Cualquier campo de más, o uno que no cuadre, invalida la cola.

use serde::{Deserialize, Serialize};

pub const INICIO: &[u8; 16] = b"RESGUARDO-COLA-1";
pub const FIN: &[u8; 16] = b"RESGUARDO-FIN-01";
pub const MAX_JSON: usize = 4096;
/// Lo que ocupa la cola sin el JSON.
pub const MARCO: usize = 16 + 4 + 4 + 16;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DatosInstalador {
    pub v: u32,
    /// `https://servidor:puerto` (sin ruta).
    pub servidor: String,
    /// SHA-256 de la autoridad TLS del servidor, `AB:CD:…` (32 pares).
    pub huella_ca: String,
    pub cliente: String,
    /// Nombre del equipo en la consola.
    pub nombre: String,
    /// Código de emparejamiento (un solo uso, caduca).
    pub codigo: String,
}

impl DatosInstalador {
    /// Comprueba cada campo: lo que no cuadre, fuera (va a una línea de órdenes).
    pub fn validar(&self) -> Result<(), String> {
        if self.v != 1 {
            return Err("Versión de la cola no admitida.".into());
        }
        let s = &self.servidor;
        let resto = s.strip_prefix("https://").ok_or("La dirección del servidor tiene que empezar por https://.")?;
        let host_ok = !resto.is_empty()
            && resto.len() <= 200
            && resto.chars().all(|c| c.is_ascii_alphanumeric() || "-.:[]".contains(c))
            && !resto.starts_with(':')
            && !resto.ends_with(':');
        if !host_ok {
            return Err("Dirección del servidor no válida (https://servidor:puerto, sin ruta).".into());
        }
        let pares: Vec<&str> = self.huella_ca.split(':').collect();
        if pares.len() != 32 || !pares.iter().all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_hexdigit())) {
            return Err("Huella de la autoridad TLS no válida.".into());
        }
        if self.cliente.is_empty() || self.cliente.len() > 64 || !self.cliente.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err("Cliente no válido.".into());
        }
        let n = self.nombre.chars().count();
        if n == 0 || n > 80 || self.nombre.trim() != self.nombre || self.nombre.chars().any(|c| c.is_control() || c == '"') {
            return Err("Nombre del equipo no válido.".into());
        }
        let c = &self.codigo;
        if !(8..=20).contains(&c.len()) || !c.chars().all(|x| x.is_ascii_alphanumeric() || x == '-') {
            return Err("Código de emparejamiento no válido.".into());
        }
        Ok(())
    }
}

/// La cola que se añade al instalador.
pub fn cola(d: &DatosInstalador) -> Result<Vec<u8>, String> {
    d.validar()?;
    let json = serde_json::to_vec(d).map_err(|e| e.to_string())?;
    if json.len() > MAX_JSON {
        return Err("Datos del instalador demasiado largos.".into());
    }
    let n = (json.len() as u32).to_be_bytes();
    let mut out = Vec::with_capacity(json.len() + MARCO);
    out.extend_from_slice(INICIO);
    out.extend_from_slice(&n);
    out.extend_from_slice(&json);
    out.extend_from_slice(&n);
    out.extend_from_slice(FIN);
    Ok(out)
}

/// Busca la cola al final de `fin` (los últimos bytes del archivo, o el
/// archivo entero). `Ok(None)`: no hay cola (un instalador genérico).
/// `Err`: hay marca de cola pero está mal (no se usa nada de ella).
pub fn leer_cola(fin: &[u8]) -> Result<Option<DatosInstalador>, String> {
    if fin.len() < MARCO || &fin[fin.len() - 16..] != FIN {
        return Ok(None);
    }
    let mal = || "La cola del instalador está dañada: descárgalo otra vez desde la consola.".to_string();
    let n_detras = u32::from_be_bytes(fin[fin.len() - 20..fin.len() - 16].try_into().map_err(|_| mal())?) as usize;
    if n_detras == 0 || n_detras > MAX_JSON || fin.len() < MARCO + n_detras {
        return Err(mal());
    }
    let ini = fin.len() - MARCO - n_detras;
    let c = &fin[ini..];
    if &c[..16] != INICIO || u32::from_be_bytes(c[16..20].try_into().map_err(|_| mal())?) as usize != n_detras {
        return Err(mal());
    }
    let d: DatosInstalador = serde_json::from_slice(&c[20..20 + n_detras]).map_err(|_| mal())?;
    d.validar()?;
    Ok(Some(d))
}

/// Cuántos bytes del final hay que leer como mucho para encontrar la cola.
pub const LEER_DEL_FINAL: usize = MAX_JSON + MARCO;

#[cfg(test)]
mod pruebas {
    use super::*;

    fn datos() -> DatosInstalador {
        DatosInstalador {
            v: 1,
            servidor: "https://192.168.1.20:8443".into(),
            huella_ca: vec!["AB"; 32].join(":"),
            cliente: "0a0e1b2c-0000-4000-8000-0000000000a1".into(),
            nombre: "SERVIDOR-01".into(),
            codigo: "ABCD-EFGH-JK".into(),
        }
    }

    #[test]
    fn ida_y_vuelta_al_final_de_un_instalador() {
        let mut exe = b"MZ...datos del instalador NSIS...".to_vec();
        exe.extend(cola(&datos()).unwrap());
        assert_eq!(leer_cola(&exe).unwrap(), Some(datos()));
        // Solo los últimos bytes (como lo lee el agente).
        let cola_sola = &exe[exe.len().saturating_sub(LEER_DEL_FINAL)..];
        assert_eq!(leer_cola(cola_sola).unwrap(), Some(datos()));
    }

    #[test]
    fn sin_cola_es_un_instalador_generico() {
        assert_eq!(leer_cola(b"").unwrap(), None);
        assert_eq!(leer_cola(b"MZ instalador normal sin nada al final").unwrap(), None);
        assert_eq!(leer_cola(&[0u8; 100]).unwrap(), None);
    }

    #[test]
    fn colas_rotas_o_con_trampas() {
        let buena = cola(&datos()).unwrap();
        // Longitud de detrás que no coincide con la de delante.
        let mut x = buena.clone();
        let l = x.len();
        x[l - 20..l - 16].copy_from_slice(&5u32.to_be_bytes());
        assert!(leer_cola(&x).is_err());
        // Longitud enorme (más de 4 KiB) o cero.
        let mut x = buena.clone();
        x[l - 20..l - 16].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(leer_cola(&x).is_err());
        x[l - 20..l - 16].copy_from_slice(&0u32.to_be_bytes());
        assert!(leer_cola(&x).is_err());
        // Más larga que el archivo.
        assert!(leer_cola(&buena[10..]).is_err());
        // Sin la marca de inicio.
        let mut x = buena.clone();
        x[0] = b'X';
        assert!(leer_cola(&x).is_err());
        // JSON alterado o con un campo de más.
        let mut x = buena.clone();
        x[25] ^= 0xff;
        assert!(leer_cola(&x).is_err());
        let json = br#"{"v":1,"servidor":"https://a:1","huella_ca":"x","cliente":"c","nombre":"n","codigo":"ABCD-EFGH","contrasena":"x"}"#;
        let mut x = INICIO.to_vec();
        x.extend((json.len() as u32).to_be_bytes());
        x.extend(json);
        x.extend((json.len() as u32).to_be_bytes());
        x.extend(FIN);
        assert!(leer_cola(&x).is_err());
    }

    /// `vectors/instalador.json`: la consola arma la cola en el navegador (v1.48,
    /// consola/src/lib/cola.ts) y `npm run test:vectores` comprueba que da estos mismos
    /// bytes. Para regenerarlo: `RESGUARDO_GENERAR_VECTORES=1 cargo test -p resguardo-protocolo instalador`.
    #[test]
    fn vector_compartido_con_la_consola() {
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD;
        let casos = [
            datos(),
            DatosInstalador {
                servidor: "https://[fd00::1]:8443".into(),
                nombre: "Recepción \\ 2 · ñandú/€".into(),
                codigo: "WXYZ-2345-6789-ABCD".into(),
                ..datos()
            },
        ];
        let malos = [
            DatosInstalador { codigo: "AB".into(), ..datos() },
            DatosInstalador { codigo: "ABCD-EFGH-JKMN-PQRS-T".into(), ..datos() },
            DatosInstalador { codigo: "ABCD_EFGH".into(), ..datos() },
            DatosInstalador { nombre: "con \"comillas\"".into(), ..datos() },
            DatosInstalador { nombre: " espacio".into(), ..datos() },
            DatosInstalador { nombre: "control\u{85}".into(), ..datos() },
            DatosInstalador { nombre: "ñ".repeat(81), ..datos() },
            DatosInstalador { servidor: "https://srv/ruta".into(), ..datos() },
            DatosInstalador { servidor: "https://srv:".into(), ..datos() },
            DatosInstalador { servidor: "http://srv:8443".into(), ..datos() },
            DatosInstalador { huella_ca: "AB:CD".into(), ..datos() },
            DatosInstalador { cliente: "../x".into(), ..datos() },
        ];
        for m in &malos {
            assert!(m.validar().is_err(), "{m:?}");
        }
        let v = serde_json::json!({
            "nota": "Cola del instalador listo (crates/protocolo/src/instalador.rs). La consola la arma igual (consola/src/lib/cola.ts).",
            "casos": casos.iter().map(|d| serde_json::json!({ "datos": d, "cola": b64.encode(cola(d).unwrap()) })).collect::<Vec<_>>(),
            "malos": malos,
        });
        let ruta = concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/instalador.json");
        let texto = serde_json::to_string_pretty(&v).unwrap() + "\n";
        if std::env::var("RESGUARDO_GENERAR_VECTORES").is_ok() {
            std::fs::write(ruta, &texto).unwrap();
        }
        let guardado = std::fs::read_to_string(ruta).expect("falta vectors/instalador.json");
        assert_eq!(guardado.replace("\r\n", "\n"), texto, "vectors/instalador.json no coincide: regenéralo solo si el formato cambió a propósito");
    }

    #[test]
    fn campos_que_no_valen() {
        let malos = [
            DatosInstalador { servidor: "http://192.168.1.20:8443".into(), ..datos() },
            DatosInstalador { servidor: "https://srv/ruta".into(), ..datos() },
            DatosInstalador { servidor: "https://srv\" & calc".into(), ..datos() },
            DatosInstalador { huella_ca: "AB:CD".into(), ..datos() },
            DatosInstalador { cliente: "../x".into(), ..datos() },
            DatosInstalador { nombre: "".into(), ..datos() },
            DatosInstalador { nombre: "con \"comillas\"".into(), ..datos() },
            DatosInstalador { nombre: "x".repeat(81), ..datos() },
            DatosInstalador { codigo: "AB".into(), ..datos() },
            DatosInstalador { codigo: "ABCD EFGH JK".into(), ..datos() },
            DatosInstalador { v: 2, ..datos() },
        ];
        for m in malos {
            assert!(cola(&m).is_err(), "{m:?}");
        }
        assert!(cola(&DatosInstalador { servidor: "https://[fd00::1]:8443".into(), nombre: "Recepción 2".into(), ..datos() }).is_ok());
    }
}
