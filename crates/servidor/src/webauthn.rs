//! 0.8.1 (H3, plan-mejoras 12c): llaves de acceso (passkeys, WebAuthn nivel 2) como
//! segundo factor para entrar en la consola, en vez del código TOTP
//! (docs/passkeys.md).
//!
//! Solo lo necesario, con `ring` (que ya usan rustls y ACME): sin `webauthn-rs`, que
//! arrastra OpenSSL. Aquí:
//!
//! - un lector de CBOR mínimo (solo longitudes definidas, con límites de tamaño y
//!   profundidad) para el objeto de atestación y la clave COSE;
//! - la clave pública COSE: ES256 (P-256), EdDSA (Ed25519) y RS256 (RSA ≥ 2048);
//! - `verificar_registro`: tipo `webauthn.create`, reto, origen, `rpIdHash`, presencia
//!   del usuario y la clave. La atestación no se comprueba (se pide `none`: no hace
//!   falta saber el fabricante del autenticador y así no se guarda nada que lo
//!   identifique);
//! - `verificar_asercion`: tipo `webauthn.get`, reto, origen, `rpIdHash`, presencia del
//!   usuario, la firma sobre `authenticatorData ‖ SHA-256(clientDataJSON)` y el
//!   contador de firmas (si lo lleva, tiene que subir: si no, puede ser un clon).
//!
//! Nada de esto escribe en ningún registro: ni ids de credenciales, ni claves, ni firmas.

use ring::signature::{self, UnparsedPublicKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Un valor CBOR (lo justo para WebAuthn).
#[derive(Debug, Clone, PartialEq)]
pub enum Cbor {
    Uint(u64),
    Neg(i128),
    Bytes(Vec<u8>),
    Text(String),
    Array(Vec<Cbor>),
    Map(Vec<(Cbor, Cbor)>),
    Bool(bool),
    Null,
}

const MAX_PROFUNDIDAD: u8 = 8;
const MAX_ELEMENTOS: u64 = 256;
const MAX_TROZO: u64 = 64 * 1024;

fn byte(b: &[u8], pos: &mut usize) -> Result<u8, &'static str> {
    let x = *b.get(*pos).ok_or("CBOR cortado")?;
    *pos += 1;
    Ok(x)
}

fn argumento(b: &[u8], pos: &mut usize, info: u8) -> Result<u64, &'static str> {
    let n = match info {
        0..=23 => return Ok(u64::from(info)),
        24 => 1,
        25 => 2,
        26 => 4,
        27 => 8,
        _ => return Err("CBOR con longitud indefinida o reservada"),
    };
    let trozo = b.get(*pos..*pos + n).ok_or("CBOR cortado")?;
    *pos += n;
    Ok(trozo.iter().fold(0u64, |a, x| (a << 8) | u64::from(*x)))
}

/// Lee un valor CBOR desde `pos` (y deja `pos` justo detrás).
pub fn leer_cbor(b: &[u8], pos: &mut usize) -> Result<Cbor, &'static str> {
    leer(b, pos, 0)
}

fn leer(b: &[u8], pos: &mut usize, prof: u8) -> Result<Cbor, &'static str> {
    if prof > MAX_PROFUNDIDAD {
        return Err("CBOR demasiado anidado");
    }
    let ini = byte(b, pos)?;
    let (mayor, info) = (ini >> 5, ini & 0x1f);
    Ok(match mayor {
        0 => Cbor::Uint(argumento(b, pos, info)?),
        1 => Cbor::Neg(-1 - i128::from(argumento(b, pos, info)?)),
        2 | 3 => {
            let n = argumento(b, pos, info)?;
            if n > MAX_TROZO {
                return Err("CBOR demasiado grande");
            }
            let n = n as usize;
            let t = b.get(*pos..*pos + n).ok_or("CBOR cortado")?.to_vec();
            *pos += n;
            if mayor == 2 {
                Cbor::Bytes(t)
            } else {
                Cbor::Text(String::from_utf8(t).map_err(|_| "CBOR con texto no válido")?)
            }
        }
        4 => {
            let n = argumento(b, pos, info)?;
            if n > MAX_ELEMENTOS {
                return Err("CBOR demasiado grande");
            }
            let mut v = Vec::new();
            for _ in 0..n {
                v.push(leer(b, pos, prof + 1)?);
            }
            Cbor::Array(v)
        }
        5 => {
            let n = argumento(b, pos, info)?;
            if n > MAX_ELEMENTOS {
                return Err("CBOR demasiado grande");
            }
            let mut v = Vec::new();
            for _ in 0..n {
                let k = leer(b, pos, prof + 1)?;
                v.push((k, leer(b, pos, prof + 1)?));
            }
            Cbor::Map(v)
        }
        7 => match info {
            20 => Cbor::Bool(false),
            21 => Cbor::Bool(true),
            22 => Cbor::Null,
            _ => return Err("CBOR con un valor que no se admite"),
        },
        _ => return Err("CBOR con etiquetas, que no se admiten"),
    })
}

impl Cbor {
    fn en_mapa(&self, k: &Cbor) -> Option<&Cbor> {
        match self {
            Cbor::Map(v) => v.iter().find(|(x, _)| x == k).map(|(_, y)| y),
            _ => None,
        }
    }
    fn texto(&self, k: &str) -> Option<&Cbor> {
        self.en_mapa(&Cbor::Text(k.to_string()))
    }
    fn entero(&self, k: i64) -> Option<&Cbor> {
        self.en_mapa(&if k >= 0 { Cbor::Uint(k as u64) } else { Cbor::Neg(i128::from(k)) })
    }
    fn bytes(&self) -> Option<&[u8]> {
        match self {
            Cbor::Bytes(b) => Some(b),
            _ => None,
        }
    }
    fn como_i64(&self) -> Option<i64> {
        match self {
            Cbor::Uint(n) => i64::try_from(*n).ok(),
            Cbor::Neg(n) => i64::try_from(*n).ok(),
            _ => None,
        }
    }
}

/// Algoritmos COSE que se admiten.
pub const ES256: i64 = -7;
pub const EDDSA: i64 = -8;
pub const RS256: i64 = -257;
pub const ALGORITMOS: [i64; 3] = [ES256, EDDSA, RS256];

/// Una clave pública de una llave de acceso.
#[derive(Debug, Clone, PartialEq)]
pub enum ClavePublica {
    /// P-256, como punto sin comprimir (`04 ‖ x ‖ y`).
    Es256(Vec<u8>),
    Ed25519(Vec<u8>),
    Rs256 {
        n: Vec<u8>,
        e: Vec<u8>,
    },
}

impl ClavePublica {
    pub fn alg(&self) -> i64 {
        match self {
            ClavePublica::Es256(_) => ES256,
            ClavePublica::Ed25519(_) => EDDSA,
            ClavePublica::Rs256 { .. } => RS256,
        }
    }

    /// La clave de un mapa COSE (RFC 9053).
    pub fn de_cose(c: &Cbor) -> Result<ClavePublica, &'static str> {
        let kty = c.entero(1).and_then(Cbor::como_i64).ok_or("Clave sin tipo")?;
        let alg = c.entero(3).and_then(Cbor::como_i64).ok_or("Clave sin algoritmo")?;
        match (kty, alg) {
            (2, ES256) => {
                if c.entero(-1).and_then(Cbor::como_i64) != Some(1) {
                    return Err("Curva no admitida");
                }
                let x = c.entero(-2).and_then(Cbor::bytes).filter(|x| x.len() == 32).ok_or("Clave no válida")?;
                let y = c.entero(-3).and_then(Cbor::bytes).filter(|y| y.len() == 32).ok_or("Clave no válida")?;
                let mut p = vec![4u8];
                p.extend_from_slice(x);
                p.extend_from_slice(y);
                Ok(ClavePublica::Es256(p))
            }
            (1, EDDSA) => {
                if c.entero(-1).and_then(Cbor::como_i64) != Some(6) {
                    return Err("Curva no admitida");
                }
                let x = c.entero(-2).and_then(Cbor::bytes).filter(|x| x.len() == 32).ok_or("Clave no válida")?;
                Ok(ClavePublica::Ed25519(x.to_vec()))
            }
            (3, RS256) => {
                let n = c.entero(-1).and_then(Cbor::bytes).ok_or("Clave no válida")?;
                let e = c.entero(-2).and_then(Cbor::bytes).ok_or("Clave no válida")?;
                let bits = n.iter().skip_while(|b| **b == 0).count() * 8;
                if !(2048..=8192).contains(&bits) || e.is_empty() || e.len() > 8 {
                    return Err("Clave RSA no admitida");
                }
                Ok(ClavePublica::Rs256 { n: n.to_vec(), e: e.to_vec() })
            }
            _ => Err("Algoritmo no admitido (ES256, EdDSA o RS256)"),
        }
    }

    /// ¿Es buena la firma de `mensaje`?
    pub fn verifica(&self, mensaje: &[u8], firma: &[u8]) -> bool {
        match self {
            ClavePublica::Es256(p) => UnparsedPublicKey::new(&signature::ECDSA_P256_SHA256_ASN1, p).verify(mensaje, firma).is_ok(),
            ClavePublica::Ed25519(p) => UnparsedPublicKey::new(&signature::ED25519, p).verify(mensaje, firma).is_ok(),
            ClavePublica::Rs256 { n, e } => {
                signature::RsaPublicKeyComponents { n: n.as_slice(), e: e.as_slice() }.verify(&signature::RSA_PKCS1_2048_8192_SHA256, mensaje, firma).is_ok()
            }
        }
    }
}

/// Lo que se lee de `authenticatorData`.
#[derive(Debug, Clone)]
pub struct DatosAutenticador {
    pub rp_id_hash: [u8; 32],
    /// Presencia del usuario (UP).
    pub presencia: bool,
    /// Usuario verificado (UV: PIN, huella…).
    pub verificado: bool,
    pub contador: u32,
    /// Al registrar: el id de la credencial y su clave COSE (en bytes).
    pub credencial: Option<(Vec<u8>, Vec<u8>)>,
}

const UP: u8 = 0x01;
const UV: u8 = 0x04;
const AT: u8 = 0x40;
const ED: u8 = 0x80;
const MAX_ID: usize = 1023;

pub fn leer_datos(b: &[u8]) -> Result<DatosAutenticador, &'static str> {
    if b.len() < 37 {
        return Err("Datos del autenticador cortos");
    }
    let mut rp_id_hash = [0u8; 32];
    rp_id_hash.copy_from_slice(&b[..32]);
    let flags = b[32];
    let contador = u32::from_be_bytes([b[33], b[34], b[35], b[36]]);
    let mut pos = 37;
    let credencial = if flags & AT != 0 {
        // aaguid (16) + longitud del id (2) + id + clave COSE.
        let l = b.get(pos + 16..pos + 18).ok_or("Datos del autenticador cortos")?;
        let n = usize::from(u16::from_be_bytes([l[0], l[1]]));
        if n == 0 || n > MAX_ID {
            return Err("Id de credencial no válido");
        }
        pos += 18;
        let id = b.get(pos..pos + n).ok_or("Datos del autenticador cortos")?.to_vec();
        pos += n;
        let ini = pos;
        leer_cbor(b, &mut pos)?;
        Some((id, b[ini..pos].to_vec()))
    } else {
        None
    };
    if flags & ED != 0 {
        leer_cbor(b, &mut pos)?;
    }
    if pos != b.len() {
        return Err("Datos del autenticador con bytes de más");
    }
    Ok(DatosAutenticador { rp_id_hash, presencia: flags & UP != 0, verificado: flags & UV != 0, contador, credencial })
}

#[derive(Deserialize)]
struct DatosCliente {
    #[serde(rename = "type")]
    tipo: String,
    challenge: String,
    origin: String,
    #[serde(default, rename = "crossOrigin")]
    cross_origin: Option<bool>,
}

/// Base64url sin relleno (lo que usa WebAuthn).
pub fn b64url(b: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

pub fn de_b64url(t: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(t.trim_end_matches('=')).ok()
}

/// Comprueba `clientDataJSON`: el tipo, el reto (de un solo uso, lo da quien llama), el
/// origen exacto y que no venga de otro sitio incrustado.
fn comprobar_cliente(json: &[u8], tipo: &str, reto: &[u8], origen: &str) -> Result<(), &'static str> {
    if json.len() > 4096 {
        return Err("Datos del navegador demasiado grandes");
    }
    let c: DatosCliente = serde_json::from_slice(json).map_err(|_| "Datos del navegador no válidos")?;
    if c.tipo != tipo {
        return Err("Tipo de operación equivocado");
    }
    let dado = de_b64url(&c.challenge).ok_or("Reto no válido")?;
    if !bool::from(subtle::ConstantTimeEq::ct_eq(dado.as_slice(), reto)) {
        return Err("El reto no es el que se dio (o ya se usó)");
    }
    if !c.origin.eq_ignore_ascii_case(origen) {
        return Err("La llave se usó desde otra dirección");
    }
    if c.cross_origin == Some(true) {
        return Err("La llave se usó desde otro sitio");
    }
    Ok(())
}

/// Una llave recién registrada, lista para guardar.
#[derive(Debug, Clone)]
pub struct Registro {
    pub id: Vec<u8>,
    pub cose: Vec<u8>,
    pub alg: i64,
    pub contador: u32,
}

/// Comprueba el alta de una llave (`navigator.credentials.create`).
pub fn verificar_registro(client_data_json: &[u8], attestation_object: &[u8], reto: &[u8], origen: &str, rp_id: &str) -> Result<Registro, &'static str> {
    comprobar_cliente(client_data_json, "webauthn.create", reto, origen)?;
    if attestation_object.len() > 16 * 1024 {
        return Err("Atestación demasiado grande");
    }
    let mut pos = 0;
    let ao = leer_cbor(attestation_object, &mut pos)?;
    if pos != attestation_object.len() {
        return Err("Atestación con bytes de más");
    }
    let auth = ao.texto("authData").and_then(Cbor::bytes).ok_or("Falta authData")?;
    let d = leer_datos(auth)?;
    if d.rp_id_hash != <[u8; 32]>::from(Sha256::digest(rp_id.as_bytes())) {
        return Err("La llave es de otro sitio (rpId)");
    }
    if !d.presencia {
        return Err("Falta la presencia del usuario");
    }
    let (id, cose) = d.credencial.ok_or("Falta la credencial")?;
    let mut p = 0;
    let clave = ClavePublica::de_cose(&leer_cbor(&cose, &mut p)?)?;
    Ok(Registro { id, cose, alg: clave.alg(), contador: d.contador })
}

/// Comprueba una entrada con una llave (`navigator.credentials.get`). Devuelve el contador nuevo.
#[allow(clippy::too_many_arguments)]
pub fn verificar_asercion(
    client_data_json: &[u8],
    authenticator_data: &[u8],
    firma: &[u8],
    reto: &[u8],
    origen: &str,
    rp_id: &str,
    cose: &[u8],
    contador_guardado: u32,
) -> Result<u32, &'static str> {
    comprobar_cliente(client_data_json, "webauthn.get", reto, origen)?;
    let d = leer_datos(authenticator_data)?;
    if d.rp_id_hash != <[u8; 32]>::from(Sha256::digest(rp_id.as_bytes())) {
        return Err("La llave es de otro sitio (rpId)");
    }
    if !d.presencia {
        return Err("Falta la presencia del usuario");
    }
    let mut p = 0;
    let clave = ClavePublica::de_cose(&leer_cbor(cose, &mut p)?)?;
    let mut mensaje = authenticator_data.to_vec();
    mensaje.extend_from_slice(&Sha256::digest(client_data_json));
    if firma.len() > 1024 || !clave.verifica(&mensaje, firma) {
        return Err("La firma no es correcta");
    }
    // El contador: si el autenticador lo lleva (alguno de los dos no es 0), tiene que subir.
    if (d.contador != 0 || contador_guardado != 0) && d.contador <= contador_guardado {
        return Err("El contador de la llave no subió: podría ser una copia");
    }
    Ok(d.contador)
}

/// El origen esperado y el rpId a partir de la cabecera `Host` (`consola.ejemplo.com:8443`).
/// Con una IP no hay llaves de acceso: los navegadores no las admiten.
pub fn origen_y_rp(host: &str) -> Result<(String, String), &'static str> {
    let host = host.trim().to_ascii_lowercase();
    if host.is_empty() || host.len() > 253 || !host.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':')) {
        return Err("Dirección no válida");
    }
    let nombre = match host.rsplit_once(':') {
        Some((n, puerto)) if puerto.chars().all(|c| c.is_ascii_digit()) => n.to_string(),
        _ => host.clone(),
    };
    if nombre.parse::<std::net::Ipv4Addr>().is_ok() || host.starts_with('[') || nombre.contains(':') {
        return Err("Las llaves de acceso no funcionan si entras por una IP: entra por el nombre de la consola.");
    }
    if nombre.is_empty() || nombre.starts_with('.') || nombre.ends_with('.') {
        return Err("Dirección no válida");
    }
    Ok((format!("https://{host}"), nombre))
}

#[cfg(test)]
pub mod pruebas {
    //! Un autenticador simulado (P-256 y Ed25519 con `ring`) para las pruebas.
    use super::*;
    use ring::rand::SystemRandom;
    use ring::signature::{EcdsaKeyPair, Ed25519KeyPair, KeyPair, ECDSA_P256_SHA256_ASN1_SIGNING};

    /// Escribe CBOR (lo justo para las pruebas).
    pub fn cbor(c: &Cbor) -> Vec<u8> {
        fn cab(out: &mut Vec<u8>, mayor: u8, n: u64) {
            let m = mayor << 5;
            if n < 24 {
                out.push(m | n as u8);
            } else if n < 256 {
                out.extend([m | 24, n as u8]);
            } else if n < 65536 {
                out.push(m | 25);
                out.extend((n as u16).to_be_bytes());
            } else {
                out.push(m | 26);
                out.extend((n as u32).to_be_bytes());
            }
        }
        let mut out = Vec::new();
        match c {
            Cbor::Uint(n) => cab(&mut out, 0, *n),
            Cbor::Neg(n) => cab(&mut out, 1, (-1 - *n) as u64),
            Cbor::Bytes(b) => {
                cab(&mut out, 2, b.len() as u64);
                out.extend(b);
            }
            Cbor::Text(t) => {
                cab(&mut out, 3, t.len() as u64);
                out.extend(t.as_bytes());
            }
            Cbor::Array(v) => {
                cab(&mut out, 4, v.len() as u64);
                for x in v {
                    out.extend(cbor(x));
                }
            }
            Cbor::Map(v) => {
                cab(&mut out, 5, v.len() as u64);
                for (k, x) in v {
                    out.extend(cbor(k));
                    out.extend(cbor(x));
                }
            }
            Cbor::Bool(b) => out.push(if *b { 0xf5 } else { 0xf4 }),
            Cbor::Null => out.push(0xf6),
        }
        out
    }

    pub enum Par {
        P256(EcdsaKeyPair),
        Ed(Ed25519KeyPair),
    }

    /// Un autenticador con una sola llave.
    pub struct Autenticador {
        pub par: Par,
        pub id: Vec<u8>,
        pub contador: u32,
        pub rp_id: String,
        pub origen: String,
    }

    impl Autenticador {
        pub fn p256(rp_id: &str, origen: &str) -> Self {
            let rng = SystemRandom::new();
            let pk = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng).unwrap();
            let par = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pk.as_ref(), &rng).unwrap();
            Autenticador { par: Par::P256(par), id: vec![7u8; 32], contador: 0, rp_id: rp_id.into(), origen: origen.into() }
        }
        pub fn ed25519(rp_id: &str, origen: &str) -> Self {
            let rng = SystemRandom::new();
            let pk = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
            let par = Ed25519KeyPair::from_pkcs8(pk.as_ref()).unwrap();
            Autenticador { par: Par::Ed(par), id: vec![9u8; 20], contador: 5, rp_id: rp_id.into(), origen: origen.into() }
        }
        pub fn cose(&self) -> Vec<u8> {
            let i = |n: i64| if n >= 0 { Cbor::Uint(n as u64) } else { Cbor::Neg(i128::from(n)) };
            match &self.par {
                Par::P256(p) => {
                    let pk = p.public_key().as_ref();
                    cbor(&Cbor::Map(vec![
                        (i(1), i(2)),
                        (i(3), i(-7)),
                        (i(-1), i(1)),
                        (i(-2), Cbor::Bytes(pk[1..33].to_vec())),
                        (i(-3), Cbor::Bytes(pk[33..65].to_vec())),
                    ]))
                }
                Par::Ed(p) => cbor(&Cbor::Map(vec![(i(1), i(1)), (i(3), i(-8)), (i(-1), i(6)), (i(-2), Cbor::Bytes(p.public_key().as_ref().to_vec()))])),
            }
        }
        pub fn datos(&self, flags: u8, con_credencial: bool) -> Vec<u8> {
            let mut d = Sha256::digest(self.rp_id.as_bytes()).to_vec();
            d.push(flags | if con_credencial { AT } else { 0 });
            d.extend(self.contador.to_be_bytes());
            if con_credencial {
                d.extend([0u8; 16]);
                d.extend((self.id.len() as u16).to_be_bytes());
                d.extend(&self.id);
                d.extend(self.cose());
            }
            d
        }
        pub fn cliente(&self, tipo: &str, reto: &[u8]) -> Vec<u8> {
            serde_json::to_vec(&serde_json::json!({ "type": tipo, "challenge": b64url(reto), "origin": self.origen, "crossOrigin": false })).unwrap()
        }
        /// `navigator.credentials.create`: (clientDataJSON, attestationObject).
        pub fn crear(&self, reto: &[u8]) -> (Vec<u8>, Vec<u8>) {
            let ao = Cbor::Map(vec![
                (Cbor::Text("fmt".into()), Cbor::Text("none".into())),
                (Cbor::Text("attStmt".into()), Cbor::Map(vec![])),
                (Cbor::Text("authData".into()), Cbor::Bytes(self.datos(UP | UV, true))),
            ]);
            (self.cliente("webauthn.create", reto), cbor(&ao))
        }
        /// `navigator.credentials.get`: (clientDataJSON, authenticatorData, firma). Sube el contador.
        pub fn firmar(&mut self, reto: &[u8]) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
            if self.contador > 0 {
                self.contador += 1;
            }
            let cd = self.cliente("webauthn.get", reto);
            let ad = self.datos(UP, false);
            let mut m = ad.clone();
            m.extend(Sha256::digest(&cd));
            let firma = match &self.par {
                Par::P256(p) => p.sign(&SystemRandom::new(), &m).unwrap().as_ref().to_vec(),
                Par::Ed(p) => p.sign(&m).as_ref().to_vec(),
            };
            (cd, ad, firma)
        }
    }

    const RP: &str = "consola.ejemplo.com";
    const ORIGEN: &str = "https://consola.ejemplo.com:8443";

    #[test]
    fn registro_y_entrada_con_p256_y_ed25519() {
        for mut a in [Autenticador::p256(RP, ORIGEN), Autenticador::ed25519(RP, ORIGEN)] {
            let reto = [3u8; 32];
            let (cd, ao) = a.crear(&reto);
            let r = verificar_registro(&cd, &ao, &reto, ORIGEN, RP).unwrap();
            assert_eq!(r.id, a.id);
            let reto2 = [4u8; 32];
            let (cd, ad, firma) = a.firmar(&reto2);
            let n = verificar_asercion(&cd, &ad, &firma, &reto2, ORIGEN, RP, &r.cose, r.contador).unwrap();
            assert_eq!(n, a.contador);
        }
    }

    #[test]
    fn lo_que_se_rechaza() {
        let mut a = Autenticador::p256(RP, ORIGEN);
        let reto = [3u8; 32];
        let (cd, ao) = a.crear(&reto);
        assert!(verificar_registro(&cd, &ao, &[5u8; 32], ORIGEN, RP).is_err(), "otro reto");
        assert!(verificar_registro(&cd, &ao, &reto, "https://otra.ejemplo.com", RP).is_err(), "otro origen");
        assert!(verificar_registro(&cd, &ao, &reto, ORIGEN, "otra.ejemplo.com").is_err(), "otro rpId");
        let (cd_get, _, _) = a.firmar(&reto);
        assert!(verificar_registro(&cd_get, &ao, &reto, ORIGEN, RP).is_err(), "tipo get al registrar");
        let r = verificar_registro(&cd, &ao, &reto, ORIGEN, RP).unwrap();
        // Entrada: sin presencia, firma cambiada, otro origen, contador que no sube.
        let reto2 = [6u8; 32];
        let (cd, ad, firma) = a.firmar(&reto2);
        assert!(verificar_asercion(&cd, &ad, &firma, &reto2, ORIGEN, RP, &r.cose, r.contador).is_ok());
        let mut sin_up = ad.clone();
        sin_up[32] &= !UP;
        assert!(verificar_asercion(&cd, &sin_up, &firma, &reto2, ORIGEN, RP, &r.cose, r.contador).is_err(), "sin presencia (y firma ya no cuadra)");
        let mut mala = firma.clone();
        let ult = mala.len() - 1;
        mala[ult] ^= 1;
        assert!(verificar_asercion(&cd, &ad, &mala, &reto2, ORIGEN, RP, &r.cose, r.contador).is_err(), "firma cambiada");
        assert!(verificar_asercion(&cd, &ad, &firma, &reto2, "https://phishing.ejemplo.net", RP, &r.cose, r.contador).is_err());
        // Un autenticador con contador: una entrada vieja (mismo contador) no vale otra vez.
        let mut e = Autenticador::ed25519(RP, ORIGEN);
        let (cd, ao) = e.crear(&reto);
        let r = verificar_registro(&cd, &ao, &reto, ORIGEN, RP).unwrap();
        let (cd, ad, firma) = e.firmar(&reto2);
        let n = verificar_asercion(&cd, &ad, &firma, &reto2, ORIGEN, RP, &r.cose, r.contador).unwrap();
        assert!(verificar_asercion(&cd, &ad, &firma, &reto2, ORIGEN, RP, &r.cose, n).is_err(), "contador que no sube: posible copia");
        // crossOrigin.
        let x = serde_json::to_vec(&serde_json::json!({ "type": "webauthn.get", "challenge": b64url(&reto2), "origin": ORIGEN, "crossOrigin": true })).unwrap();
        assert!(comprobar_cliente(&x, "webauthn.get", &reto2, ORIGEN).is_err());
    }

    #[test]
    fn cbor_con_limites() {
        let mut p = 0;
        assert!(leer_cbor(&[0x9f], &mut p).is_err(), "longitud indefinida");
        let mut p = 0;
        assert!(leer_cbor(&[0x5a, 0xff, 0xff, 0xff, 0xff], &mut p).is_err(), "trozo enorme");
        let mut anidado = vec![0x81u8; 20];
        anidado.push(0x00);
        let mut p = 0;
        assert!(leer_cbor(&anidado, &mut p).is_err(), "demasiado anidado");
        let mut p = 0;
        assert!(leer_cbor(&[0xc0, 0x00], &mut p).is_err(), "etiquetas");
        let mut p = 0;
        assert_eq!(leer_cbor(&cbor(&Cbor::Neg(-257)), &mut p).unwrap(), Cbor::Neg(-257));
    }

    #[test]
    fn origen_por_nombre_y_no_por_ip() {
        assert_eq!(origen_y_rp("Consola.Ejemplo.com:8443").unwrap(), ("https://consola.ejemplo.com:8443".into(), "consola.ejemplo.com".into()));
        assert_eq!(origen_y_rp("localhost:8443").unwrap().1, "localhost");
        assert!(origen_y_rp("192.0.2.10:8443").is_err());
        assert!(origen_y_rp("[2001:db8::1]:8443").is_err());
        assert!(origen_y_rp("mal host").is_err());
    }
}
