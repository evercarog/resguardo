//! Paquete de exportación de un cliente (`.resguardo-cliente`; docs/plataforma.md,
//! §3.4, y docs/api-servidor.md, §11). Lo cifra y lo descifra **el navegador**
//! con `K_exp`; los servidores solo guardan y sirven el cifrado.
//!
//! Formato (binario):
//!
//! ```text
//! "RESGUARDO-CLIENTE-1\n" ‖ sal_cliente (base64) ‖ "\n" ‖ trozo₀ ‖ trozo₁ ‖ …
//! trozoₙ = u32 big-endian (longitud de lo cifrado) ‖ nonce(24) ‖ XChaCha20-Poly1305(K_exp, nonce, datosₙ,
//!          aad = "resguardo-cliente-v1|" + n + "|" + (último ? "1" : "0"))
//! ```
//!
//! Los datos (JSON en UTF-8) van en trozos de 4 MiB; siempre hay al menos uno.
//! La sal va en claro para poder derivar `K_exp` con la clave de administración.

use crate::simetrico;

pub const MAGIA: &[u8] = b"RESGUARDO-CLIENTE-1\n";
const ID: &str = "resguardo-cliente-v1";

/// Cifra el paquete. `nonce(n)` da el nonce de cada trozo (aleatorio en uso normal).
pub fn cifrar_con(k_exp: &[u8; 32], sal_cliente_b64: &str, json: &[u8], mut nonce: impl FnMut(u64) -> [u8; simetrico::NONCE]) -> Vec<u8> {
    let mut out = Vec::with_capacity(json.len() + 64);
    out.extend_from_slice(MAGIA);
    out.extend_from_slice(sal_cliente_b64.as_bytes());
    out.push(b'\n');
    let trozos: Vec<&[u8]> = if json.is_empty() { vec![&[][..]] } else { json.chunks(simetrico::TROZO).collect() };
    let total = trozos.len() as u64;
    for (n, datos) in trozos.into_iter().enumerate() {
        let n = n as u64;
        let c = simetrico::cifrar_trozo(k_exp, ID, n, n + 1 == total, datos, &nonce(n));
        out.extend_from_slice(&(c.len() as u32).to_be_bytes());
        out.extend_from_slice(&c);
    }
    out
}

/// Cifra el paquete con nonces aleatorios.
pub fn cifrar(k_exp: &[u8; 32], sal_cliente_b64: &str, json: &[u8]) -> Vec<u8> {
    cifrar_con(k_exp, sal_cliente_b64, json, |_| simetrico::nonce_aleatorio())
}

/// La sal del cliente (para derivar `K_exp`) y dónde empiezan los trozos.
pub fn cabecera(paquete: &[u8]) -> Result<(String, usize), String> {
    let resto = paquete.strip_prefix(MAGIA).ok_or("No es un paquete de cliente de Resguardo.")?;
    let fin = resto.iter().position(|b| *b == b'\n').filter(|p| *p <= 128).ok_or("Paquete dañado (cabecera).")?;
    let sal = std::str::from_utf8(&resto[..fin]).map_err(|_| "Paquete dañado (sal).")?.to_string();
    Ok((sal, MAGIA.len() + fin + 1))
}

/// Descifra el paquete: (sal del cliente, JSON). Falla si falta o sobra algún trozo.
pub fn descifrar(k_exp: &[u8; 32], paquete: &[u8]) -> Result<(String, Vec<u8>), String> {
    let (sal, mut i) = cabecera(paquete)?;
    let mut out = Vec::new();
    let mut n = 0u64;
    loop {
        let len = paquete.get(i..i + 4).ok_or("Paquete incompleto.")?;
        let len = u32::from_be_bytes([len[0], len[1], len[2], len[3]]) as usize;
        i += 4;
        let trozo = paquete.get(i..i + len).ok_or("Paquete incompleto.")?;
        i += len;
        let ultimo = i == paquete.len();
        let datos = simetrico::descifrar_trozo(k_exp, ID, n, ultimo, trozo)
            .map_err(|_| "No se pudo descifrar el paquete: la clave de administración no es la de este cliente, o el archivo está dañado.".to_string())?;
        out.extend_from_slice(&datos);
        if ultimo {
            return Ok((sal, out));
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_y_vuelta_y_recortes() {
        let k = [3u8; 32];
        let p = cifrar(&k, "BAQEBAQEBAQEBAQEBAQEBA==", br#"{"v":1}"#);
        assert_eq!(cabecera(&p).unwrap().0, "BAQEBAQEBAQEBAQEBAQEBA==");
        assert_eq!(descifrar(&k, &p).unwrap().1, br#"{"v":1}"#);
        assert!(descifrar(&[4u8; 32], &p).is_err(), "otra clave");
        assert!(descifrar(&k, &p[..p.len() - 1]).is_err(), "recortado");
        // Varios trozos: quitar el último no pasa por bueno.
        let grande = vec![b'x'; simetrico::TROZO + 10];
        let p = cifrar(&k, "s", &grande);
        assert_eq!(descifrar(&k, &p).unwrap().1, grande);
        let (_, inicio) = cabecera(&p).unwrap();
        let len0 = u32::from_be_bytes(p[inicio..inicio + 4].try_into().unwrap()) as usize;
        assert!(descifrar(&k, &p[..inicio + 4 + len0]).is_err(), "sin el último trozo");
        assert!(descifrar(&k, b"otra cosa").is_err());
        assert_eq!(descifrar(&k, &cifrar(&k, "s", b"")).unwrap().1, b"");
    }
}
