//! Copia de la consola (`.resguardo-consola`; docs/api-servidor.md, §2,
//! «Copia de la consola»): la carpeta de datos de Resguardo Server (la base de
//! datos, `identidad.key` y la autoridad TLS) cifrada para la **clave de
//! respaldo de la consola**, que elige el propietario y que el servidor nunca
//! guarda.
//!
//! El servidor hace la copia cada noche sin conocer la clave: solo guarda la
//! clave **pública** X25519 que se deriva de ella (la calcula la consola, en el
//! navegador) y su sal. Cada copia lleva una clave de archivo al azar, sellada
//! para esa pública (`crypto_box_seal`, como las órdenes); solo quien sabe la
//! clave de respaldo puede abrirla.
//!
//! Desde la versión 2 (Resguardo Server 0.7.11) la copia va además **firmada
//! con la identidad Ed25519 del servidor** (la que fijan los agentes): la
//! pública de la clave de respaldo está en claro en la cabecera (y en el
//! servidor), así que sin firma cualquiera que la tuviera podría fabricar otra
//! copia que también se abre con la clave. La restauración enseña la huella de
//! la identidad y no sigue si no es la que se espera (la del kit).
//!
//! ```text
//! secreto  = HKDF-SHA256(ikm = Argon2id(NFC(clave), sal, m=64 MiB, t=3, p=1), salt = "", info = "resguardo-respaldo-consola-v1")
//! publica  = X25519(secreto)                          (clave pública de crypto_box)
//!
//! archivo  = "RESGUARDO-CONSOLA-2\n" ‖ cabecera (JSON en una línea, ≤ 4 KiB) ‖ "\n" ‖ trozo₀ ‖ … ‖ trozoₙ ‖ 0xFFFFFFFF ‖ firma(64)
//! cabecera = { "v": 2, "sal", "publica", "clave": crypto_box_seal(publica, K), "creado", "identidad", "version" }
//! trozoₙ   = u32 big-endian (longitud) ‖ nonce(24) ‖ XChaCha20-Poly1305(K, nonce, datosₙ,
//!            aad = "resguardo-consola-v2|" + hex(SHA-256(cabecera)) + "|" + n + "|" + (último ? "1" : "0"))
//! firma    = Ed25519(identidad, "resguardo-consola-v2-firma|" ‖ SHA-256(todo lo anterior a 0xFFFFFFFF))
//! ```
//!
//! Los datos van en trozos de 4 MiB (siempre al menos uno). La cabecera va en
//! claro (para derivar la clave y decir de qué servidor es) pero queda atada a
//! los trozos: cambiarla rompe el descifrado. La versión 1 («RESGUARDO-CONSOLA-1»,
//! `"v": 1`, aad con «resguardo-consola-v1|») es igual sin la marca ni la firma:
//! se sigue pudiendo restaurar, con un aviso.

use crate::claves::B64;
use crate::{derivaciones, simetrico};
use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

/// La de las copias que se hacen ahora (versión 2, firmadas).
pub const MAGIA: &[u8] = b"RESGUARDO-CONSOLA-2\n";
/// La de las copias anteriores a 0.7.11 (sin firma).
pub const MAGIA_V1: &[u8] = b"RESGUARDO-CONSOLA-1\n";
const ID_V1: &str = "resguardo-consola-v1";
const ID_V2: &str = "resguardo-consola-v2";
const CONTEXTO_FIRMA: &[u8] = b"resguardo-consola-v2-firma|";
/// Lo que va en lugar de la longitud de un trozo para decir «ahora va la firma».
const MARCA_FIRMA: u32 = u32::MAX;
const LARGO_FIRMA: usize = 64;
const MAX_CABECERA: usize = 4096;
/// Un trozo cifrado no pasa de esto (4 MiB de datos, el nonce y la etiqueta).
const MAX_TROZO: usize = simetrico::TROZO + simetrico::NONCE + simetrico::ETIQUETA;
/// Largo mínimo de la clave de respaldo (la consola pide además que sea fuerte).
pub const MIN_CLAVE: usize = 12;

/// Lo que va en claro al principio del archivo.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Cabecera {
    pub v: u32,
    /// Sal de la clave de respaldo (base64, 16 bytes).
    pub sal: String,
    /// Clave pública X25519 derivada de la clave de respaldo (base64).
    pub publica: String,
    /// La clave del archivo, sellada para `publica` (base64).
    pub clave: String,
    /// Cuándo se hizo (RFC 3339).
    pub creado: String,
    /// Identidad Ed25519 del servidor (base64), la que fijan los agentes.
    pub identidad: String,
    /// Versión de Resguardo Server que la hizo.
    pub version: String,
}

/// Lo que se sabe de una copia tras recorrerla entera.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comprobada {
    pub cabecera: Cabecera,
    /// `true`: versión 2 y la firma es de `cabecera.identidad`. `false`: versión 1 (sin firma).
    pub firmada: bool,
}

/// El secreto X25519 de la clave de respaldo (normalizada a NFC) con su sal.
pub fn secreto(clave: &str, sal_b64: &str) -> Result<[u8; 32], String> {
    let sal = B64.decode(sal_b64).map_err(|_| "Sal no válida.".to_string())?;
    Ok(derivaciones::hkdf32(&derivaciones::argon2id(clave, &sal)?, "resguardo-respaldo-consola-v1"))
}

/// La clave pública (base64) de un secreto.
pub fn publica_de(secreto: &[u8; 32]) -> String {
    B64.encode(crypto_box::SecretKey::from(*secreto).public_key().as_bytes())
}

/// La clave pública (base64) de la clave de respaldo: lo único que guarda el servidor.
pub fn publica(clave: &str, sal_b64: &str) -> Result<String, String> {
    Ok(publica_de(&secreto(clave, sal_b64)?))
}

/// ¿Es una clave pública X25519 válida (32 bytes en base64)?
pub fn publica_valida(b64: &str) -> bool {
    B64.decode(b64).is_ok_and(|v| v.len() == 32)
}

fn hash_hex(b: &[u8]) -> String {
    Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect()
}

/// Huella corta de una identidad (base64): los 8 primeros bytes en hexadecimal,
/// `AB:CD:…`. Es la que enseña la consola (y el kit de la copia de la consola).
pub fn huella(identidad_b64: &str) -> String {
    match B64.decode(identidad_b64) {
        Ok(b) if b.len() == 32 => b[..8].iter().map(|x| format!("{x:02X}")).collect::<Vec<_>>().join(":"),
        _ => "(identidad no válida)".into(),
    }
}

/// ¿Es `dada` la identidad `identidad_b64`? Vale la identidad entera (base64) o
/// su huella en hexadecimal (la corta del kit, o más larga), sin importar los
/// separadores ni las mayúsculas. Menos de 8 bytes (16 cifras) no vale.
pub fn coincide_huella(identidad_b64: &str, dada: &str) -> bool {
    let Ok(id) = B64.decode(identidad_b64) else { return false };
    if id.len() != 32 {
        return false;
    }
    let dada = dada.trim();
    if B64.decode(dada).is_ok_and(|b| b == id) {
        return true;
    }
    let cifras: String = dada.chars().filter(|c| !matches!(c, ':' | '-' | ' ')).collect::<String>().to_ascii_uppercase();
    cifras.len() >= 16 && cifras.len().is_multiple_of(2) && cifras.chars().all(|c| c.is_ascii_hexdigit()) && hex(&id).starts_with(&cifras)
}

/// Escribe el archivo cifrado mientras se le dan los datos (sin tenerlos en memoria).
pub struct Cifrador<W: Write> {
    salida: W,
    k: [u8; 32],
    id: String,
    n: u64,
    buf: Vec<u8>,
    /// Lo escrito hasta ahora (lo que firma la identidad).
    hash: Sha256,
    /// La identidad del servidor; `None` solo en la versión 1 (pruebas).
    firma: Option<SigningKey>,
}

impl<W: Write> Cifrador<W> {
    /// Empieza el archivo para la clave pública `publica` (base64) con su `sal`,
    /// firmado con la identidad del servidor `identidad`.
    pub fn nuevo(salida: W, sal: &str, publica: &str, creado: &str, identidad: &SigningKey, version: &str) -> Result<Self, String> {
        Self::empezar(salida, sal, publica, creado, B64.encode(identidad.verifying_key().to_bytes()), version, Some(identidad.clone()))
    }

    /// Una copia de la versión 1 (sin firma), como las de antes de 0.7.11. Solo
    /// para probar que se siguen leyendo.
    #[doc(hidden)]
    pub fn nuevo_v1_sin_firma(salida: W, sal: &str, publica: &str, creado: &str, identidad: &str, version: &str) -> Result<Self, String> {
        Self::empezar(salida, sal, publica, creado, identidad.to_string(), version, None)
    }

    fn empezar(mut salida: W, sal: &str, publica: &str, creado: &str, identidad: String, version: &str, firma: Option<SigningKey>) -> Result<Self, String> {
        let mut k = [0u8; 32];
        crypto_box::aead::rand_core::RngCore::fill_bytes(&mut crypto_box::aead::OsRng, &mut k);
        let v = if firma.is_some() { 2 } else { 1 };
        let cabecera = Cabecera {
            v,
            sal: sal.into(),
            publica: publica.into(),
            clave: crate::claves::seal_bytes(publica, &k)?,
            creado: creado.into(),
            identidad,
            version: version.into(),
        };
        let json = serde_json::to_vec(&cabecera).map_err(|e| e.to_string())?;
        if json.len() > MAX_CABECERA {
            return Err("Cabecera de la copia demasiado larga.".into());
        }
        let magia = if v == 2 { MAGIA } else { MAGIA_V1 };
        let mut hash = Sha256::new();
        for parte in [magia, &json[..], b"\n"] {
            salida.write_all(parte).map_err(|e| e.to_string())?;
            hash.update(parte);
        }
        let id = format!("{}|{}", if v == 2 { ID_V2 } else { ID_V1 }, hash_hex(&json));
        Ok(Self { salida, k, id, n: 0, buf: Vec::with_capacity(simetrico::TROZO), hash, firma })
    }

    fn trozo(&mut self, datos: &[u8], ultimo: bool) -> std::io::Result<()> {
        let c = simetrico::cifrar_trozo(&self.k, &self.id, self.n, ultimo, datos, &simetrico::nonce_aleatorio());
        let largo = (c.len() as u32).to_be_bytes();
        self.salida.write_all(&largo)?;
        self.salida.write_all(&c)?;
        self.hash.update(largo);
        self.hash.update(&c);
        self.n += 1;
        Ok(())
    }

    /// Cierra el archivo (el último trozo lo dice), lo firma y devuelve la salida.
    pub fn terminar(mut self) -> Result<W, String> {
        let resto = std::mem::take(&mut self.buf);
        self.trozo(&resto, true).map_err(|e| e.to_string())?;
        self.k = [0u8; 32];
        if let Some(clave) = self.firma.take() {
            let resumen = std::mem::take(&mut self.hash).finalize();
            let firma = clave.sign(&[CONTEXTO_FIRMA, &resumen[..]].concat());
            self.salida.write_all(&MARCA_FIRMA.to_be_bytes()).and_then(|_| self.salida.write_all(&firma.to_bytes())).map_err(|e| e.to_string())?;
        }
        self.salida.flush().map_err(|e| e.to_string())?;
        Ok(self.salida)
    }
}

impl<W: Write> Write for Cifrador<W> {
    fn write(&mut self, datos: &[u8]) -> std::io::Result<usize> {
        self.buf.extend_from_slice(datos);
        // Se guarda siempre un trozo entero: solo al terminar se sabe cuál es el último.
        while self.buf.len() > simetrico::TROZO {
            let resto = self.buf.split_off(simetrico::TROZO);
            let lleno = std::mem::replace(&mut self.buf, resto);
            self.trozo(&lleno, false)?;
        }
        Ok(datos.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Lee el archivo llevando la cuenta (SHA-256) de lo que firma la identidad.
struct Lector<R: Read> {
    r: R,
    hash: Sha256,
}

/// Lo que viene después de un trozo.
enum Siguiente {
    Fin,
    Firma,
    Trozo(usize),
}

impl<R: Read> Lector<R> {
    fn exacto(&mut self, buf: &mut [u8]) -> std::io::Result<()> {
        self.r.read_exact(buf)?;
        self.hash.update(&*buf);
        Ok(())
    }

    /// Lee hasta llenar `buf`: cuántos bytes había (menos solo si se acabó el archivo).
    fn hasta(&mut self, buf: &mut [u8]) -> Result<usize, String> {
        let mut leidos = 0;
        while leidos < buf.len() {
            match self.r.read(&mut buf[leidos..]) {
                Ok(0) => break,
                Ok(x) => leidos += x,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        Ok(leidos)
    }

    fn siguiente(&mut self) -> Result<Siguiente, String> {
        let mut l = [0u8; 4];
        match self.hasta(&mut l)? {
            0 => Ok(Siguiente::Fin),
            4 => {
                let n = u32::from_be_bytes(l);
                if n == MARCA_FIRMA {
                    // La marca no entra en lo firmado.
                    return Ok(Siguiente::Firma);
                }
                self.hash.update(l);
                Ok(Siguiente::Trozo(n as usize))
            }
            _ => Err("Copia incompleta.".into()),
        }
    }
}

/// Lee la cabecera del archivo (sin descifrar nada).
pub fn leer_cabecera(entrada: &mut impl Read) -> Result<(Cabecera, Vec<u8>), String> {
    let mut l = Lector { r: entrada, hash: Sha256::new() };
    leer_cabecera_de(&mut l)
}

fn leer_cabecera_de<R: Read>(l: &mut Lector<R>) -> Result<(Cabecera, Vec<u8>), String> {
    let no_es = || "No es una copia de la consola de Resguardo.".to_string();
    let mut magia = vec![0u8; MAGIA.len()];
    l.exacto(&mut magia).map_err(|_| no_es())?;
    let version = match &magia[..] {
        m if m == MAGIA => 2,
        m if m == MAGIA_V1 => 1,
        m if m.starts_with(b"RESGUARDO-CONSOLA-") => {
            return Err("Esta copia es de una versión más nueva de Resguardo Server: actualiza antes de restaurarla.".into())
        }
        _ => return Err(no_es()),
    };
    let mut json = Vec::new();
    let mut b = [0u8; 1];
    loop {
        l.exacto(&mut b).map_err(|_| "Copia dañada (cabecera).".to_string())?;
        if b[0] == b'\n' {
            break;
        }
        json.push(b[0]);
        if json.len() > MAX_CABECERA {
            return Err("Copia dañada (cabecera).".into());
        }
    }
    let c: Cabecera = serde_json::from_slice(&json).map_err(|_| "Copia dañada (cabecera).".to_string())?;
    if c.v != version {
        return Err("Copia dañada (la versión de la cabecera no es la del archivo).".into());
    }
    Ok((c, json))
}

/// Recorre el archivo: la cabecera, cada trozo (a `cada_trozo(n, último, cifrado)`)
/// y, en la versión 2, la firma de la identidad de la cabecera. Falla si falta o
/// sobra algo, o si la firma no es buena.
fn recorrer(entrada: impl Read, mut cada_trozo: impl FnMut(&Cabecera, &[u8], u64, bool, &[u8]) -> Result<(), String>) -> Result<Comprobada, String> {
    let mut l = Lector { r: entrada, hash: Sha256::new() };
    let (cab, json) = leer_cabecera_de(&mut l)?;
    let firmada = cab.v == 2;
    let mut siguiente = l.siguiente()?;
    let mut n = 0u64;
    loop {
        let largo = match siguiente {
            Siguiente::Trozo(largo) => largo,
            Siguiente::Fin | Siguiente::Firma => return Err("Copia incompleta.".into()),
        };
        if largo > MAX_TROZO {
            return Err("Copia dañada (trozo demasiado grande).".into());
        }
        let mut trozo = vec![0u8; largo];
        l.exacto(&mut trozo).map_err(|_| "Copia incompleta.".to_string())?;
        siguiente = l.siguiente()?;
        let ultimo = match (&siguiente, firmada) {
            (Siguiente::Trozo(_), _) => false,
            (Siguiente::Fin, false) | (Siguiente::Firma, true) => true,
            (Siguiente::Fin, true) => return Err("Copia incompleta (le falta la firma del servidor).".into()),
            (Siguiente::Firma, false) => return Err("Copia dañada.".into()),
        };
        cada_trozo(&cab, &json, n, ultimo, &trozo)?;
        if ultimo {
            break;
        }
        n += 1;
    }
    if firmada {
        let mut firma = [0u8; LARGO_FIRMA];
        if l.hasta(&mut firma)? != LARGO_FIRMA {
            return Err("Copia incompleta (la firma del servidor está cortada).".into());
        }
        if l.hasta(&mut [0u8; 1])? != 0 {
            return Err("Copia dañada (sobran datos después de la firma).".into());
        }
        let clave: [u8; 32] = B64.decode(&cab.identidad).ok().and_then(|v| v.try_into().ok()).ok_or("Copia dañada (identidad no válida).")?;
        let vk = VerifyingKey::from_bytes(&clave).map_err(|_| "Copia dañada (identidad no válida).".to_string())?;
        let resumen = std::mem::take(&mut l.hash).finalize();
        vk.verify_strict(&[CONTEXTO_FIRMA, &resumen[..]].concat(), &Signature::from_bytes(&firma))
            .map_err(|_| "La firma de la copia no es de la identidad que dice su cabecera: el archivo está dañado o no lo hizo ese servidor.".to_string())?;
    }
    Ok(Comprobada { cabecera: cab, firmada })
}

/// Comprueba la estructura y la firma de una copia **sin la clave de respaldo**
/// (lo primero de la restauración: decir de qué servidor es antes de pedir nada).
pub fn comprobar(entrada: impl Read) -> Result<Comprobada, String> {
    recorrer(entrada, |_, _, _, _, _| Ok(()))
}

/// Descifra el archivo con el secreto de la clave de respaldo y escribe los datos en `salida`.
/// Falla (sin dar por buena la copia) si la clave no es la suya, si falta o sobra algo, si
/// alguien tocó la cabecera o los trozos, o si la firma (versión 2) no es de su identidad.
/// Lo escrito en `salida` antes de un error no vale: quien llama lo descarta.
pub fn descifrar(entrada: impl Read, secreto: &[u8; 32], mut salida: impl Write) -> Result<Comprobada, String> {
    let mut k: Option<([u8; 32], String)> = None;
    recorrer(entrada, |cab, json, n, ultimo, trozo| {
        if k.is_none() {
            if publica_de(secreto) != cab.publica {
                return Err("La clave de respaldo no es la de esta copia.".into());
            }
            let clave: [u8; 32] = crate::claves::open_bytes(&B64.encode(secreto), &cab.clave)
                .ok()
                .and_then(|v| v.try_into().ok())
                .ok_or("La clave de respaldo no es la de esta copia.")?;
            k = Some((clave, format!("{}|{}", if cab.v == 2 { ID_V2 } else { ID_V1 }, hash_hex(json))));
        }
        let (clave, id) = k.as_ref().expect("puesta arriba");
        let datos =
            simetrico::descifrar_trozo(clave, id, n, ultimo, trozo).map_err(|_| "La copia está dañada o incompleta (no se puede descifrar).".to_string())?;
        salida.write_all(&datos).map_err(|e| e.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLAVE: &str = "caballo bateria grapa correcta";

    fn sal() -> String {
        B64.encode([8u8; 16])
    }

    fn identidad() -> SigningKey {
        SigningKey::from_bytes(&[42u8; 32])
    }

    fn cifrar(datos: &[u8], publica: &str) -> Vec<u8> {
        let mut c = Cifrador::nuevo(Vec::new(), &sal(), publica, "2026-10-03T03:30:00+02:00", &identidad(), "0.7.11").unwrap();
        // En varias escrituras de tamaños raros, como al empaquetar archivos.
        for parte in datos.chunks(1_000_003) {
            c.write_all(parte).unwrap();
        }
        c.terminar().unwrap()
    }

    fn cifrar_v1(datos: &[u8], publica: &str) -> Vec<u8> {
        let mut c = Cifrador::nuevo_v1_sin_firma(Vec::new(), &sal(), publica, "2026-10-03T03:30:00+02:00", "IDENTIDAD", "0.7.8").unwrap();
        c.write_all(datos).unwrap();
        c.terminar().unwrap()
    }

    fn fin_cabecera(archivo: &[u8]) -> usize {
        MAGIA.len() + archivo[MAGIA.len()..].iter().position(|b| *b == b'\n').unwrap()
    }

    #[test]
    fn ida_y_vuelta_y_clave_equivocada() {
        let secreto = secreto(CLAVE, &sal()).unwrap();
        let publica = publica_de(&secreto);
        assert_eq!(publica, super::publica(&crate::derivaciones::normalizar_clave(CLAVE), &sal()).unwrap());
        let id = B64.encode(identidad().verifying_key().to_bytes());
        for datos in [Vec::new(), b"hola".to_vec(), vec![7u8; simetrico::TROZO], vec![9u8; 2 * simetrico::TROZO + 5]] {
            let archivo = cifrar(&datos, &publica);
            assert!(archivo.starts_with(MAGIA));
            let mut salida = Vec::new();
            let c = descifrar(&archivo[..], &secreto, &mut salida).unwrap();
            assert_eq!(salida, datos);
            assert!(c.firmada);
            let cab = c.cabecera;
            assert_eq!((cab.v, cab.identidad.as_str(), cab.version.as_str(), cab.sal.as_str()), (2, id.as_str(), "0.7.11", sal().as_str()));
            // Sin la clave: la misma cabecera y la firma comprobada.
            assert_eq!(comprobar(&archivo[..]).unwrap(), Comprobada { cabecera: cab, firmada: true });
        }
        let archivo = cifrar(b"datos de la consola", &publica);
        // Otra clave no abre nada.
        let otro = super::secreto("otra clave de respaldo distinta", &sal()).unwrap();
        assert!(descifrar(&archivo[..], &otro, &mut Vec::new()).unwrap_err().contains("no es la de esta copia"));
        // Recortada, con un byte cambiado o con la cabecera tocada: no.
        assert!(descifrar(&archivo[..archivo.len() - 1], &secreto, &mut Vec::new()).is_err());
        assert!(comprobar(&archivo[..archivo.len() - 1]).is_err());
        let mut tocado = archivo.clone();
        let p = tocado.len() - LARGO_FIRMA - 10;
        tocado[p] ^= 1;
        assert!(descifrar(&tocado[..], &secreto, &mut Vec::new()).is_err());
        assert!(comprobar(&tocado[..]).unwrap_err().contains("firma"), "la firma cubre los trozos");
        let fin = fin_cabecera(&archivo);
        let mut otra_cab = archivo[..fin].to_vec();
        let p = otra_cab.windows(9).position(|w| w == b"\"version\"").unwrap();
        otra_cab[p + 12] = b'9';
        otra_cab.extend_from_slice(&archivo[fin..]);
        assert!(descifrar(&otra_cab[..], &secreto, &mut Vec::new()).is_err(), "la cabecera va atada a los trozos");
        assert!(comprobar(&otra_cab[..]).is_err(), "y a la firma");
        // Sin quitar el último trozo de uno de varios.
        let grande = cifrar(&vec![1u8; simetrico::TROZO + 10], &publica);
        let largo0 = u32::from_be_bytes(grande[fin + 1..fin + 5].try_into().unwrap()) as usize;
        assert!(descifrar(&grande[..fin + 5 + largo0], &secreto, &mut Vec::new()).is_err(), "sin el último trozo");
        assert!(descifrar(&b"otra cosa"[..], &secreto, &mut Vec::new()).is_err());
        // Sobra algo después de la firma.
        let mut largo = archivo.clone();
        largo.push(0);
        assert!(comprobar(&largo[..]).unwrap_err().contains("sobran"));
    }

    /// Quien tenga la pública de la clave de respaldo puede fabricar una copia que
    /// se abre con la clave, pero no firmarla con la identidad del servidor.
    #[test]
    fn la_firma_es_de_la_identidad_de_la_cabecera() {
        let secreto = secreto(CLAVE, &sal()).unwrap();
        let publica = publica_de(&secreto);
        let buena = cifrar(b"consola buena", &publica);
        // Otra identidad firma una copia que dice ser de la buena: no.
        let otra = SigningKey::from_bytes(&[7u8; 32]);
        let falsa = Cifrador::nuevo(Vec::new(), &sal(), &publica, "x", &otra, "0.7.11").unwrap().terminar().unwrap();
        let fin_f = fin_cabecera(&falsa);
        let cab_buena = &buena[..fin_cabecera(&buena)];
        let mut mezclada = cab_buena.to_vec();
        mezclada.extend_from_slice(&falsa[fin_f..]);
        assert!(comprobar(&mezclada[..]).is_err());
        // La firma de otra copia de la misma identidad tampoco vale.
        let otra_buena = cifrar(b"otra copia", &publica);
        let mut cambiada = buena[..buena.len() - LARGO_FIRMA].to_vec();
        cambiada.extend_from_slice(&otra_buena[otra_buena.len() - LARGO_FIRMA..]);
        assert!(comprobar(&cambiada[..]).unwrap_err().contains("firma"));
        // Quitarle la firma (pasarla por una de la versión 1) rompe el descifrado.
        let mut v1 = MAGIA_V1.to_vec();
        let cab = String::from_utf8(buena[MAGIA.len()..fin_cabecera(&buena)].to_vec()).unwrap().replace("\"v\":2", "\"v\":1");
        v1.extend_from_slice(cab.as_bytes());
        v1.extend_from_slice(&buena[fin_cabecera(&buena)..buena.len() - LARGO_FIRMA - 4]);
        assert!(!comprobar(&v1[..]).unwrap().firmada);
        assert!(descifrar(&v1[..], &secreto, &mut Vec::new()).is_err(), "la cabecera y el aad atan la versión");
    }

    #[test]
    fn las_copias_v1_se_siguen_leyendo_sin_firma() {
        let secreto = secreto(CLAVE, &sal()).unwrap();
        let archivo = cifrar_v1(b"copia antigua", &publica_de(&secreto));
        assert!(archivo.starts_with(MAGIA_V1));
        let mut salida = Vec::new();
        let c = descifrar(&archivo[..], &secreto, &mut salida).unwrap();
        assert_eq!(salida, b"copia antigua");
        assert!(!c.firmada);
        assert_eq!((c.cabecera.v, c.cabecera.identidad.as_str()), (1, "IDENTIDAD"));
        // Con una marca de firma en una v1: dañada.
        let mut con_marca = archivo.clone();
        con_marca.extend_from_slice(&MARCA_FIRMA.to_be_bytes());
        assert!(comprobar(&con_marca[..]).is_err());
        // Una versión que aún no existe.
        let mut nueva = b"RESGUARDO-CONSOLA-3\n".to_vec();
        nueva.extend_from_slice(&archivo[MAGIA_V1.len()..]);
        assert!(comprobar(&nueva[..]).unwrap_err().contains("más nueva"));
        // Cabecera que dice otra versión que la magia.
        let mut mal = MAGIA.to_vec();
        mal.extend_from_slice(&archivo[MAGIA_V1.len()..]);
        assert!(comprobar(&mal[..]).unwrap_err().contains("versión"));
    }

    #[test]
    fn huellas() {
        let id = B64.encode(identidad().verifying_key().to_bytes());
        let h = huella(&id);
        assert_eq!(h.len(), 8 * 3 - 1);
        assert!(coincide_huella(&id, &h));
        assert!(coincide_huella(&id, &h.to_lowercase().replace(':', "")));
        assert!(coincide_huella(&id, &format!("  {id} ")), "la identidad entera");
        let otra = B64.encode(SigningKey::from_bytes(&[1u8; 32]).verifying_key().to_bytes());
        assert!(!coincide_huella(&otra, &h));
        assert!(!coincide_huella(&id, &h[..11]), "menos de 8 bytes no vale");
        assert!(!coincide_huella(&id, ""));
        assert!(!coincide_huella("no es base64", &h));
        assert_eq!(huella("corta"), "(identidad no válida)");
    }

    #[test]
    fn la_clave_se_normaliza() {
        // «ñ» compuesta o descompuesta: la misma clave pública.
        let a = publica("contraseña larga de la consola", &sal()).unwrap();
        let b = publica("contrasen\u{303}a larga de la consola", &sal()).unwrap();
        assert_eq!(a, b);
        assert!(publica_valida(&a) && !publica_valida("corta"));
    }
}
