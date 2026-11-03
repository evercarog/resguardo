//! Cuentas: contraseñas (Argon2id), TOTP (RFC 6238), sesiones con cookie y
//! la comprobación de que una cuenta es miembro de un cliente.

use crate::almacen::{ahora, Almacen, ClienteCtx, Cuenta, NuevaSesion, Rol, Sesion, Ts};
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
/// Duración de una sesión a medias (falta el TOTP).
pub const SESION_PENDIENTE_MIN: i64 = 10;

// ---------- Plazos de las sesiones (16k) ----------

/// Una sesión recién abierta no manda sobre las demás durante este tiempo: no
/// puede cerrar otras sesiones ni cambiar la contraseña, el segundo paso ni las
/// llaves de acceso (como Telegram). Las sesiones antiguas sí pueden cerrarla.
pub const SESION_NUEVA_S: i64 = 24 * 3600;
/// Cada cuánto, como mucho, se anota el uso de una sesión (no una escritura por petición).
pub const TOCAR_CADA_S: i64 = 60;
/// Clave del ajuste del servidor con los plazos.
pub const CLAVE_PLAZOS: &str = "sesiones_plazos";

/// Plazos de las sesiones, que ajusta el propietario del servidor (con topes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Plazos {
    /// Sin usarla este tiempo, caduca (sin «Recordar»).
    pub inactividad_horas: i64,
    /// Máximo absoluto sin «Recordar», aunque se use.
    pub maximo_horas: i64,
    /// Máximo absoluto con «Recordar este dispositivo» (0: no se ofrece).
    pub recordar_dias: i64,
}

impl Default for Plazos {
    fn default() -> Self {
        Plazos { inactividad_horas: 8, maximo_horas: 24, recordar_dias: 14 }
    }
}

impl Plazos {
    /// Dentro de los topes: inactividad 1–24 h; máximo entre la inactividad y 7 días;
    /// recordar 0–30 días.
    pub fn con_topes(self) -> Self {
        let inactividad_horas = self.inactividad_horas.clamp(1, 24);
        Plazos { inactividad_horas, maximo_horas: self.maximo_horas.clamp(inactividad_horas, 7 * 24), recordar_dias: self.recordar_dias.clamp(0, 30) }
    }

    pub fn de_texto(v: Option<&str>) -> Self {
        v.and_then(|v| serde_json::from_str::<Plazos>(v).ok()).unwrap_or_default().con_topes()
    }

    /// Máximo absoluto de una sesión nueva.
    pub fn maximo(&self, recordar: bool, creada: Ts) -> Ts {
        if recordar && self.recordar_dias > 0 {
            creada + self.recordar_dias * 86_400
        } else {
            creada + self.maximo_horas * 3600
        }
    }

    /// Hasta cuándo vale sin usarla, desde `ahora` (con «Recordar», hasta 7 días sin
    /// usarla); nunca más allá del máximo.
    pub fn expira(&self, recordar: bool, ahora: Ts, maximo: Ts) -> Ts {
        let inactividad = if recordar && self.recordar_dias > 0 { self.recordar_dias.min(7) * 86_400 } else { self.inactividad_horas * 3600 };
        (ahora + inactividad).min(maximo)
    }
}

pub fn plazos(db: &dyn Almacen) -> Plazos {
    Plazos::de_texto(db.valor(CLAVE_PLAZOS).ok().flatten().as_deref())
}

/// Una sesión que aún no lleva 24 h abierta (salvo en una cuenta que tampoco las
/// lleva: no hay sesiones antiguas que proteger). Devuelve hasta cuándo es «nueva».
pub fn sesion_nueva_hasta(sesion_creada: Ts, cuenta_creada: Option<Ts>, ahora: Ts) -> Option<Ts> {
    let hasta = sesion_creada + SESION_NUEVA_S;
    let cuenta_nueva = cuenta_creada.is_some_and(|c| ahora - c < SESION_NUEVA_S);
    (hasta > ahora && !cuenta_nueva).then_some(hasta)
}

/// Navegador y sistema en pocas palabras, del `User-Agent` («Chrome · Android»).
/// Solo esto se guarda: nunca el texto entero.
pub fn resumen_agente(ua: &str) -> String {
    let ua: String = ua.chars().take(512).collect();
    let tiene = |x: &str| ua.contains(x);
    let navegador = if tiene("Edg/") || tiene("EdgA/") || tiene("EdgiOS/") {
        "Edge"
    } else if tiene("OPR/") || tiene("Opera") {
        "Opera"
    } else if tiene("SamsungBrowser/") {
        "Samsung Internet"
    } else if tiene("Firefox/") || tiene("FxiOS/") {
        "Firefox"
    } else if tiene("Chrome/") || tiene("CriOS/") || tiene("Chromium/") {
        "Chrome"
    } else if tiene("Safari/") {
        "Safari"
    } else {
        ""
    };
    let sistema = if tiene("Android") {
        "Android"
    } else if tiene("iPhone") || tiene("iPod") {
        "iPhone"
    } else if tiene("iPad") {
        "iPad"
    } else if tiene("Windows") {
        "Windows"
    } else if tiene("CrOS") {
        "ChromeOS"
    } else if tiene("Mac OS X") || tiene("Macintosh") {
        "macOS"
    } else if tiene("Linux") {
        "Linux"
    } else {
        ""
    };
    match (navegador, sistema) {
        ("", "") => "Navegador desconocido".into(),
        (n, "") => n.into(),
        ("", s) => format!("Navegador · {s}"),
        (n, s) => format!("{n} · {s}"),
    }
}

/// De dónde viene una sesión (para la lista de sesiones).
#[derive(Clone, Debug, Default)]
pub struct Origen {
    pub agente: String,
    pub ip: String,
}

impl Origen {
    pub fn de(headers: &axum::http::HeaderMap, ip: &str) -> Self {
        let ua = headers.get(axum::http::header::USER_AGENT).and_then(|v| v.to_str().ok()).unwrap_or("");
        Origen { agente: resumen_agente(ua), ip: ip.chars().take(64).collect() }
    }
}

/// Lo que se guarda de una sesión a medias (tras la contraseña): el «Recordar» y
/// el origen pasan a la completa.
pub fn nueva_pendiente(token_hash: &str, cuenta: &str, recordar: bool, origen: &Origen) -> NuevaSesion {
    let t = ahora();
    let fin = t + SESION_PENDIENTE_MIN * 60;
    NuevaSesion {
        token_hash: token_hash.into(),
        cuenta: cuenta.into(),
        aal: 1,
        expira: fin,
        maximo: fin,
        creada: t,
        recordar,
        agente: origen.agente.clone(),
        ip: origen.ip.clone(),
    }
}

/// La sesión completa que sale de una a medias (al dar el segundo paso): empieza ahora.
pub fn nueva_completa(db: &dyn Almacen, token_hash: &str, pendiente: &Sesion) -> NuevaSesion {
    let p = plazos(db);
    let t = ahora();
    let maximo = p.maximo(pendiente.recordar, t);
    NuevaSesion {
        token_hash: token_hash.into(),
        cuenta: pendiente.cuenta_id.clone(),
        aal: 2,
        expira: p.expira(pendiente.recordar, t, maximo),
        maximo,
        creada: t,
        recordar: pendiente.recordar,
        agente: pendiente.agente.clone(),
        ip: pendiente.ip.clone(),
    }
}

/// La misma sesión con otra ficha (al cambiar la contraseña o el autenticador):
/// conserva cuándo se entró, el «Recordar» y su máximo (no se alarga).
pub fn misma_con_otra_ficha(db: &dyn Almacen, token_hash: &str, s: &Sesion) -> NuevaSesion {
    let p = plazos(db);
    NuevaSesion {
        token_hash: token_hash.into(),
        cuenta: s.cuenta_id.clone(),
        aal: 2,
        expira: p.expira(s.recordar, ahora(), s.maximo),
        maximo: s.maximo,
        creada: s.creada,
        recordar: s.recordar,
        agente: s.agente.clone(),
        ip: s.ip.clone(),
    }
}

/// Segundos que le quedan a la cookie: hasta el máximo de la sesión. Sin
/// «Recordar», `None`: una cookie de sesión, que se va al cerrar el navegador.
pub fn max_edad(n: &NuevaSesion) -> Option<i64> {
    n.recordar.then(|| (n.maximo - ahora()).max(60))
}
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

/// La cookie dura hasta el máximo de la sesión (sin `Max-Age`, hasta cerrar el
/// navegador): la base de datos decide cuánto vale de verdad (10 min a medias,
/// sin el TOTP; caduca antes si no se usa).
pub fn cookie_sesion(st: &St, ficha: &str, max_edad_s: Option<i64>) -> String {
    format!(
        "{}={ficha}; Path=/; HttpOnly; SameSite=Strict{}{}",
        nombre_cookie(&st.identidad_pub, st.opciones.https),
        max_edad_s.map(|s| format!("; Max-Age={s}")).unwrap_or_default(),
        if st.opciones.https { "; Secure" } else { "" }
    )
}

pub fn cookie_borrar(st: &St) -> String {
    cookie_sesion(st, "", Some(0))
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
    pub sesion: Sesion,
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
    Ok(SesionActual { cuenta, token_hash, aal: sesion.aal, sesion })
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
        if let Err(espera) = st.limites.intento_o_espera(&format!("cuenta-min:{}", s.cuenta.id), MAX_PETICIONES_CUENTA_MIN, std::time::Duration::from_secs(60))
        {
            return Err(ErrorApi::demasiados_esperar(
                "cuenta",
                "Demasiadas peticiones de tu cuenta en el último minuto (por seguridad hay un máximo). Si tienes muchas pestañas de la consola abiertas, cierra alguna; vuelve a intentarlo en un momento.",
                espera,
            ));
        }
        // Se renueva con el uso (16k), nunca más allá de su máximo; el uso se anota
        // como mucho una vez por minuto.
        let t = ahora();
        if t - s.sesion.usada >= TOCAR_CADA_S {
            let (th, recordar, maximo) = (s.token_hash.clone(), s.sesion.recordar, s.sesion.maximo);
            let _ = st
                .db(move |db| {
                    let expira = plazos(db).expira(recordar, t, maximo);
                    db.tocar_sesion(&th, expira, t)
                })
                .await;
        }
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

    /// Lo delicado de la cuenta (cerrar otras sesiones, cambiar la contraseña, el segundo
    /// paso o las llaves de acceso) no se puede desde una sesión de menos de 24 h (16k).
    pub async fn sesion_asentada(&self, st: &St) -> Res<()> {
        let id = self.id().to_string();
        let creada_cuenta = st.db(move |db| db.cuenta_creada(&id)).await?;
        match sesion_nueva_hasta(self.0.sesion.creada, creada_cuenta, ahora()) {
            None => Ok(()),
            Some(hasta) => {
                let horas = (hasta - ahora() + 3599) / 3600;
                Err(ErrorApi::nuevo(
                    axum::http::StatusCode::FORBIDDEN,
                    "sesion_nueva",
                    format!(
                        "Esta sesión es nueva: por seguridad, durante sus primeras 24 horas no puede cerrar otras sesiones, cambiar la contraseña, el segundo paso o las llaves de acceso, restablecer el segundo paso de otros ni cambiar los plazos de las sesiones. Podrás en unas {} h, o ahora desde un dispositivo donde ya tenías la sesión abierta.",
                        horas.max(1)
                    ),
                )
                .con(serde_json::json!({ "desde": crate::api::fecha(hasta) })))
            }
        }
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
    fn plazos_de_las_sesiones() {
        let p = Plazos::default();
        let t = 1_000_000;
        // Sin recordar: 8 h sin usarla, 24 h como mucho.
        let max = p.maximo(false, t);
        assert_eq!(max, t + 24 * 3600);
        assert_eq!(p.expira(false, t, max), t + 8 * 3600);
        // El uso no la alarga más allá del máximo.
        assert_eq!(p.expira(false, t + 20 * 3600, max), max);
        // Con recordar: 14 días como mucho, 7 sin usarla.
        let max = p.maximo(true, t);
        assert_eq!(max, t + 14 * 86_400);
        assert_eq!(p.expira(true, t, max), t + 7 * 86_400);
        assert_eq!(p.expira(true, t + 13 * 86_400, max), max);
        // Con «recordar» apagado por el propietario, como sin recordar.
        let p0 = Plazos { recordar_dias: 0, ..p };
        assert_eq!(p0.maximo(true, t), t + 24 * 3600);
        // Topes: nada de sesiones eternas ni de cero horas.
        let raro = Plazos { inactividad_horas: 0, maximo_horas: 10_000, recordar_dias: 365 }.con_topes();
        assert_eq!(raro, Plazos { inactividad_horas: 1, maximo_horas: 168, recordar_dias: 30 });
        let corto = Plazos { inactividad_horas: 12, maximo_horas: 2, recordar_dias: 1 }.con_topes();
        assert_eq!(corto.maximo_horas, 12, "el máximo nunca es menor que la inactividad");
        assert_eq!(Plazos::de_texto(Some("no es json")), Plazos::default());
        assert_eq!(Plazos::de_texto(None), Plazos::default());
    }

    #[test]
    fn sesion_nueva_durante_24_horas() {
        let t = 10_000_000;
        // Sesión de hace 1 h en una cuenta vieja: nueva hasta las 24 h.
        assert_eq!(sesion_nueva_hasta(t - 3600, Some(t - 90 * 86_400), t), Some(t - 3600 + SESION_NUEVA_S));
        // Pasadas las 24 h, ya manda.
        assert_eq!(sesion_nueva_hasta(t - SESION_NUEVA_S, Some(t - 90 * 86_400), t), None);
        // Una cuenta recién creada no tiene sesiones antiguas que proteger.
        assert_eq!(sesion_nueva_hasta(t - 60, Some(t - 3600), t), None);
        assert!(sesion_nueva_hasta(t - 60, None, t).is_some());
    }

    #[test]
    fn resumen_del_navegador() {
        let android = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Mobile Safari/537.36";
        assert_eq!(resumen_agente(android), "Chrome · Android");
        let win_edge = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36 Edg/129.0.0.0";
        assert_eq!(resumen_agente(win_edge), "Edge · Windows");
        let iphone = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.6 Mobile/15E148 Safari/604.1";
        assert_eq!(resumen_agente(iphone), "Safari · iPhone");
        let firefox = "Mozilla/5.0 (X11; Linux x86_64; rv:131.0) Gecko/20100101 Firefox/131.0";
        assert_eq!(resumen_agente(firefox), "Firefox · Linux");
        assert_eq!(resumen_agente(""), "Navegador desconocido");
        assert_eq!(resumen_agente("curl/8.0"), "Navegador desconocido");
        // Nada del texto original pasa tal cual.
        assert_eq!(resumen_agente("<script>alert(1)</script> Windows"), "Navegador · Windows");
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
