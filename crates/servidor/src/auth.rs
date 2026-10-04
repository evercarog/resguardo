//! Cuentas: contraseñas (Argon2id), TOTP (RFC 6238), sesiones con cookie y
//! la comprobación de que una cuenta es miembro de un cliente.

use crate::almacen::{ahora, ClienteCtx, Cuenta, Rol};
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::{Digest, Sha256};

/// Prefijo de la cookie de sesión. El nombre lleva además 6 caracteres de la
/// identidad del servidor (`resguardo_sesion_ab12cd`): dos servidores en el
/// mismo nombre de equipo (otro puerto) no se pisan la sesión. Con HTTPS va con
/// el prefijo `__Host-` (el navegador solo la acepta `Secure`, con `Path=/` y sin
/// `Domain`: ni un subdominio ni una página por HTTP pueden ponerla o pisarla).
pub const COOKIE: &str = "resguardo_sesion";

pub fn nombre_cookie(identidad_pub: &str, https: bool) -> String {
    let id: String = identidad_pub.chars().filter(char::is_ascii_alphanumeric).take(6).collect();
    format!("{}{COOKIE}_{id}", if https { "__Host-" } else { "" })
}
/// Duración de una sesión completa (se renueva con el uso) y de una a medias (falta el TOTP).
pub const SESION_HORAS: i64 = 12;
pub const SESION_PENDIENTE_MIN: i64 = 10;
/// Peticiones de una cuenta (con sesión completa) por minuto.
pub const MAX_PETICIONES_CUENTA_MIN: u32 = 1_200;

// ---------- Contraseñas ----------

pub fn hash_contrasena(contrasena: &str) -> Result<String, String> {
    let sal = SaltString::generate(&mut OsRng);
    Argon2::default().hash_password(contrasena.as_bytes(), &sal).map(|h| h.to_string()).map_err(|e| e.to_string())
}

pub fn comprueba_contrasena(contrasena: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|h| Argon2::default().verify_password(contrasena.as_bytes(), &h).is_ok())
}

/// Hash fijo para gastar el mismo tiempo cuando la cuenta no existe (no revela qué correos hay).
pub fn hash_senuelo() -> &'static str {
    static H: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    H.get_or_init(|| hash_contrasena("señuelo-que-nadie-usa").unwrap_or_default())
}

pub fn valida_contrasena(c: &str) -> Result<(), ErrorApi> {
    if c.chars().count() < 12 {
        return Err(ErrorApi::datos("La contraseña debe tener al menos 12 caracteres."));
    }
    if c.len() > 512 {
        return Err(ErrorApi::datos("La contraseña es demasiado larga."));
    }
    Ok(())
}

pub fn valida_correo(c: &str) -> Result<String, ErrorApi> {
    let c = c.trim().to_lowercase();
    let ok =
        c.len() <= 254 && c.split_once('@').is_some_and(|(u, d)| !u.is_empty() && d.contains('.') && !d.starts_with('.') && !c.contains(char::is_whitespace));
    if ok {
        Ok(c)
    } else {
        Err(ErrorApi::datos("Escribe un correo válido."))
    }
}

// ---------- Fichas aleatorias ----------

pub fn ficha() -> String {
    let mut b = [0u8; 32];
    rand::rng().fill_bytes(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn hash_ficha(f: &str) -> String {
    Sha256::digest(f.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

// ---------- TOTP (RFC 6238: SHA-1, 6 cifras, 30 s) ----------

const B32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

pub fn base32(bytes: &[u8]) -> String {
    let mut out = String::new();
    let (mut buf, mut bits) = (0u32, 0u32);
    for &b in bytes {
        buf = (buf << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            out.push(B32[((buf >> (bits - 5)) & 31) as usize] as char);
            bits -= 5;
        }
    }
    if bits > 0 {
        out.push(B32[((buf << (5 - bits)) & 31) as usize] as char);
    }
    out
}

pub fn de_base32(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut buf, mut bits) = (0u32, 0u32);
    for ch in s.chars().filter(|c| *c != '=' && !c.is_whitespace()) {
        let v = B32.iter().position(|b| *b as char == ch.to_ascii_uppercase())? as u32;
        buf = (buf << 5) | v;
        bits += 5;
        if bits >= 8 {
            out.push((buf >> (bits - 8)) as u8);
            bits -= 8;
        }
    }
    Some(out)
}

pub fn nuevo_secreto_totp() -> String {
    let mut b = [0u8; 20];
    rand::rng().fill_bytes(&mut b);
    base32(&b)
}

pub fn hotp(secreto: &[u8], contador: u64) -> u32 {
    let mut m = Hmac::<sha1::Sha1>::new_from_slice(secreto).expect("HMAC acepta cualquier longitud");
    m.update(&contador.to_be_bytes());
    let h = m.finalize().into_bytes();
    let o = (h[19] & 0x0f) as usize;
    let n = ((h[o] as u32 & 0x7f) << 24) | ((h[o + 1] as u32) << 16) | ((h[o + 2] as u32) << 8) | h[o + 3] as u32;
    n % 1_000_000
}

/// Comprueba un código para «ahora» (±1 paso). Devuelve el paso aceptado si es posterior a `ultimo`.
pub fn comprueba_totp(secreto_b32: &str, codigo: &str, ultimo: i64, ahora_s: i64) -> Option<i64> {
    let codigo: String = codigo.chars().filter(|c| c.is_ascii_digit()).collect();
    if codigo.len() != 6 {
        return None;
    }
    let esperado: u32 = codigo.parse().ok()?;
    let secreto = de_base32(secreto_b32)?;
    let paso = ahora_s / 30;
    (paso - 1..=paso + 1).find(|&p| p > ultimo && p >= 0 && hotp(&secreto, p as u64) == esperado)
}

pub fn uri_totp(correo: &str, secreto: &str) -> String {
    let etiqueta: String =
        correo.chars().map(|c| if c.is_ascii_alphanumeric() || "@.-_".contains(c) { c.to_string() } else { format!("%{:02X}", c as u32 & 0xff) }).collect();
    format!("otpauth://totp/Resguardo:{etiqueta}?secret={secreto}&issuer=Resguardo&algorithm=SHA1&digits=6&period=30")
}

/// 10 códigos de recuperación `xxxx-xxxx` y sus hashes.
pub fn codigos_recuperacion() -> (Vec<String>, Vec<String>) {
    const A: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
    let mut codigos = Vec::new();
    for _ in 0..10 {
        let mut b = [0u8; 8];
        rand::rng().fill_bytes(&mut b);
        let s: String = b.iter().map(|x| A[*x as usize % A.len()] as char).collect();
        codigos.push(format!("{}-{}", &s[..4], &s[4..]));
    }
    let hashes = codigos.iter().map(|c| hash_ficha(c)).collect();
    (codigos, hashes)
}

// ---------- Cookies ----------

/// La cookie dura lo que una sesión completa: la base de datos decide cuánto
/// vale de verdad (10 min a medias, sin el TOTP; 12 h desde el último uso).
pub fn cookie_sesion(st: &St, ficha: &str, max_edad_s: i64) -> String {
    format!(
        "{}={ficha}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_edad_s}{}",
        nombre_cookie(&st.identidad_pub, st.opciones.https),
        if st.opciones.https { "; Secure" } else { "" }
    )
}

pub fn cookie_borrar(st: &St) -> String {
    cookie_sesion(st, "", 0)
}

/// Una sesión completa nueva para la cuenta (al terminar de entrar o al cambiar
/// la contraseña o el autenticador): la ficha de antes deja de valer, así que
/// quien la hubiera visto o fijado antes no gana nada. Devuelve (ficha, su hash).
pub fn ficha_nueva() -> (String, String) {
    let f = ficha();
    let h = hash_ficha(&f);
    (f, h)
}

fn ficha_de(headers: &axum::http::HeaderMap, nombre: &str) -> Option<String> {
    headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|p| p.trim().split_once('='))
        .find(|(k, _)| *k == nombre)
        .map(|(_, v)| v.to_string())
        .filter(|v| v.len() == 64 && v.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Sesión, completa o a medias.
pub struct SesionActual {
    pub cuenta: Cuenta,
    pub token_hash: String,
    pub aal: u8,
}

async fn sesion_de(parts: &Parts, st: &St) -> Res<SesionActual> {
    sesion_de_cabeceras(&parts.headers, st).await
}

/// La sesión de estas cabeceras (completa o a medias).
pub async fn sesion_de_cabeceras(headers: &axum::http::HeaderMap, st: &St) -> Res<SesionActual> {
    let ficha = ficha_de(headers, &nombre_cookie(&st.identidad_pub, st.opciones.https)).ok_or_else(ErrorApi::sin_sesion)?;
    let token_hash = hash_ficha(&ficha);
    let th = token_hash.clone();
    let sesion = st.db(move |db| db.sesion(&th)).await?.ok_or_else(ErrorApi::sin_sesion)?;
    let cid = sesion.cuenta_id.clone();
    let cuenta = st.db(move |db| db.cuenta(&cid)).await?.ok_or_else(ErrorApi::sin_sesion)?;
    Ok(SesionActual { cuenta, token_hash, aal: sesion.aal })
}

/// Una sesión a medias (tras la contraseña, antes del TOTP).
pub struct Pendiente(pub SesionActual);

impl FromRequestParts<St> for Pendiente {
    type Rejection = ErrorApi;
    async fn from_request_parts(parts: &mut Parts, st: &St) -> Res<Self> {
        Ok(Pendiente(sesion_de(parts, st).await?))
    }
}

/// Una sesión completa (contraseña y TOTP).
pub struct Usuario(pub SesionActual);

impl FromRequestParts<St> for Usuario {
    type Rejection = ErrorApi;
    async fn from_request_parts(parts: &mut Parts, st: &St) -> Res<Self> {
        let s = sesion_de(parts, st).await?;
        if s.aal < 2 {
            return Err(ErrorApi::necesita_totp());
        }
        // Peticiones por cuenta y minuto (v1.34): una consola abierta en varias
        // pestañas cabe de sobra; una sesión robada que barre la API, no.
        if !st.limites.intento(&format!("cuenta-min:{}", s.cuenta.id), MAX_PETICIONES_CUENTA_MIN, std::time::Duration::from_secs(60)) {
            return Err(ErrorApi::demasiados());
        }
        // Se renueva con el uso.
        let th = s.token_hash.clone();
        let _ = st.db(move |db| db.elevar_sesion(&th, ahora() + SESION_HORAS * 3600)).await;
        Ok(Usuario(s))
    }
}

impl Usuario {
    pub fn id(&self) -> &str {
        &self.0.cuenta.id
    }
    pub fn nombre(&self) -> &str {
        &self.0.cuenta.nombre
    }

    /// La cuenta es miembro del cliente con al menos `minimo`: el contexto para llegar a sus datos.
    pub async fn miembro(&self, st: &St, cliente: &str, minimo: Rol) -> Res<(ClienteCtx, Rol)> {
        let (cuenta, cl) = (self.id().to_string(), cliente.to_string());
        let rol = st.db(move |db| db.rol(&cuenta, &cl)).await?.ok_or_else(ErrorApi::no_existe)?;
        if rol < minimo {
            return Err(ErrorApi::prohibido());
        }
        Ok((ClienteCtx::autorizado(cliente), rol))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totp_rfc6238() {
        // RFC 6238, apéndice B (SHA-1): secreto "12345678901234567890", T = 59 s → 94287082 (8 cifras) → 287082.
        let secreto = base32(b"12345678901234567890");
        assert_eq!(hotp(&de_base32(&secreto).unwrap(), 59 / 30), 287082);
        assert_eq!(comprueba_totp(&secreto, "287 082", 0, 59), Some(1));
        // No se acepta dos veces el mismo paso.
        assert_eq!(comprueba_totp(&secreto, "287082", 1, 59), None);
        assert_eq!(comprueba_totp(&secreto, "000000", 0, 59), None);
        assert_eq!(de_base32(&base32(b"hola mundo")).unwrap(), b"hola mundo");
    }

    #[test]
    fn cookie_con_prefijo_host_solo_con_https() {
        assert_eq!(nombre_cookie("ab+12/cdEF", false), "resguardo_sesion_ab12cd");
        assert_eq!(nombre_cookie("ab+12/cdEF", true), "__Host-resguardo_sesion_ab12cd");
    }

    #[test]
    fn contrasenas() {
        let h = hash_contrasena("una contraseña larga").unwrap();
        assert!(h.starts_with("$argon2id$"));
        assert!(comprueba_contrasena("una contraseña larga", &h));
        assert!(!comprueba_contrasena("otra", &h));
        assert!(valida_contrasena("corta").is_err());
        assert_eq!(valida_correo(" Ana@Ejemplo.COM ").unwrap(), "ana@ejemplo.com");
        assert!(valida_correo("ana").is_err());
    }
}
