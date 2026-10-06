//! Lo que se configura: canales (del servidor y de cada cliente), ajustes del
//! servidor y preferencias de cada persona. Todo se guarda como JSON en la
//! tabla `servidor` de `control.db` (claves `notif:…`), con los secretos de
//! cada canal cifrados (`cifrado`). Lo que sale por la API pasa por
//! [`vista_canal`], que nunca lleva un secreto: solo «configurado».

use super::cifrado::{contexto, Clave};
use super::{Severidad, Silencio};
use crate::almacen::{Almacen, Rol, Ts, R};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const CLAVE_AJUSTES: &str = "notif:ajustes";
pub fn clave_cliente(cliente: &str) -> String {
    format!("notif:cliente:{cliente}")
}
pub fn clave_persona(cuenta: &str) -> String {
    format!("notif:persona:{cuenta}")
}
pub fn clave_pref(cliente: &str, cuenta: &str) -> String {
    format!("notif:pref:{cliente}:{cuenta}")
}

/// Canales como mucho, en cada ámbito (servidor o cliente).
pub const MAX_CANALES: usize = 20;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TipoCanal {
    Correo,
    Webhook,
    Ntfy,
    Telegram,
}

impl TipoCanal {
    pub fn texto(self) -> &'static str {
        match self {
            TipoCanal::Correo => "correo",
            TipoCanal::Webhook => "webhook",
            TipoCanal::Ntfy => "ntfy",
            TipoCanal::Telegram => "telegram",
        }
    }
    /// Los secretos que admite cada tipo.
    pub fn campos_secretos(self) -> &'static [&'static str] {
        match self {
            TipoCanal::Correo => &["contrasena"],
            TipoCanal::Webhook => &["url", "secreto"],
            TipoCanal::Ntfy => &["url", "token"],
            TipoCanal::Telegram => &["token"],
        }
    }
}

/// Lo que no es secreto de un canal (según su tipo, unos campos u otros).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct ConfigCanal {
    /// Correo: servidor SMTP, puerto, `starttls` | `tls` | `ninguna`, usuario y remitente.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub puerto: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seguridad: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usuario: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remitente: Option<String>,
    /// Telegram: el chat (número, o `@canal`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_id: Option<String>,
    /// Webhook y ntfy: el servidor de la dirección (lo único que se enseña de ella; la
    /// dirección entera es un secreto: en ntfy, quien sabe el tema lo lee).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub servidor: Option<String>,
}

/// Quién recibe qué por un canal compartido (webhook, ntfy, Telegram).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Reglas {
    #[serde(default = "severidades_por_defecto")]
    pub severidades: Vec<Severidad>,
    /// Solo en los canales del servidor: de qué clientes (`None`: de todos).
    #[serde(default)]
    pub clientes: Option<Vec<String>>,
    #[serde(default)]
    pub silencio: Option<Silencio>,
    #[serde(default)]
    pub resumen_diario: bool,
    #[serde(default)]
    pub resumen_semanal: bool,
}

fn severidades_por_defecto() -> Vec<Severidad> {
    vec![Severidad::Critico, Severidad::Importante]
}

impl Default for Reglas {
    fn default() -> Self {
        Self { severidades: severidades_por_defecto(), clientes: None, silencio: None, resumen_diario: false, resumen_semanal: false }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Canal {
    pub id: String,
    pub tipo: TipoCanal,
    pub nombre: String,
    #[serde(default = "verdadero")]
    pub activo: bool,
    #[serde(default)]
    pub config: ConfigCanal,
    /// Campo → secreto cifrado (`Clave::cerrar`).
    #[serde(default)]
    pub secretos: BTreeMap<String, String>,
    #[serde(default)]
    pub reglas: Reglas,
    #[serde(default)]
    pub actualizado: Ts,
    #[serde(default)]
    pub por: String,
}

fn verdadero() -> bool {
    true
}

/// Ajustes del servidor (los pone su propietario).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Ajustes {
    /// Dirección pública de la consola, para los enlaces de los mensajes.
    #[serde(default)]
    pub url_consola: Option<String>,
    #[serde(default = "max_por_defecto")]
    pub max_por_hora: u32,
    /// Hora de los resúmenes («HH:MM», hora del servidor).
    #[serde(default = "hora_por_defecto")]
    pub hora_resumen: String,
    /// Día del resumen semanal (1 = lunes … 7 = domingo).
    #[serde(default = "dia_por_defecto")]
    pub dia_semanal: u8,
    #[serde(default)]
    pub canales: Vec<Canal>,
}

fn max_por_defecto() -> u32 {
    super::reglas::MAX_POR_HORA
}
fn hora_por_defecto() -> String {
    "08:00".into()
}
fn dia_por_defecto() -> u8 {
    1
}

impl Default for Ajustes {
    fn default() -> Self {
        Self { url_consola: None, max_por_hora: max_por_defecto(), hora_resumen: hora_por_defecto(), dia_semanal: dia_por_defecto(), canales: Vec::new() }
    }
}

/// Canales propios de un cliente (los pone su propietario). Un correo propio sustituye al
/// del servidor para las personas de ese cliente; los demás se suman a los del servidor.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AjustesCliente {
    #[serde(default)]
    pub canales: Vec<Canal>,
}

/// Preferencias de una persona que valen para todos sus clientes.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PrefsPersona {
    #[serde(default)]
    pub silencio: Option<Silencio>,
    #[serde(default)]
    pub resumen_diario: bool,
    #[serde(default = "verdadero")]
    pub resumen_semanal: bool,
}

impl Default for PrefsPersona {
    fn default() -> Self {
        Self { silencio: None, resumen_diario: false, resumen_semanal: true }
    }
}

/// Lo que una persona recibe por correo de un cliente.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PrefsCliente {
    /// Avisos inmediatos de estas gravedades.
    pub inmediatos: Vec<Severidad>,
    /// ¿Entra este cliente en sus resúmenes?
    pub resumen: bool,
}

impl PrefsCliente {
    /// Si la persona no ha dicho nada: según su papel.
    pub fn por_defecto(rol: Rol) -> Self {
        match rol {
            Rol::Propietario | Rol::Administrador => Self { inmediatos: vec![Severidad::Critico, Severidad::Importante], resumen: true },
            Rol::Tecnico => Self { inmediatos: vec![Severidad::Critico], resumen: false },
            Rol::Lectura => Self { inmediatos: vec![], resumen: false },
        }
    }
}

// ---------- Leer y guardar ----------

fn leer<T: serde::de::DeserializeOwned + Default>(db: &dyn Almacen, clave: &str) -> R<T> {
    Ok(db.valor(clave)?.filter(|v| !v.is_empty()).and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default())
}

fn guardar<T: Serialize>(db: &dyn Almacen, clave: &str, v: &T) -> R<()> {
    db.poner_valor(clave, &serde_json::to_string(v).map_err(|e| e.to_string())?)
}

pub fn ajustes(db: &dyn Almacen) -> R<Ajustes> {
    leer(db, CLAVE_AJUSTES)
}
pub fn guardar_ajustes(db: &dyn Almacen, a: &Ajustes) -> R<()> {
    guardar(db, CLAVE_AJUSTES, a)
}
pub fn ajustes_cliente(db: &dyn Almacen, cliente: &str) -> R<AjustesCliente> {
    leer(db, &clave_cliente(cliente))
}
pub fn guardar_ajustes_cliente(db: &dyn Almacen, cliente: &str, a: &AjustesCliente) -> R<()> {
    guardar(db, &clave_cliente(cliente), a)
}
pub fn prefs_persona(db: &dyn Almacen, cuenta: &str) -> R<PrefsPersona> {
    leer(db, &clave_persona(cuenta))
}
pub fn guardar_prefs_persona(db: &dyn Almacen, cuenta: &str, p: &PrefsPersona) -> R<()> {
    guardar(db, &clave_persona(cuenta), p)
}
/// Las de esa persona en ese cliente (las de su papel si no dijo nada).
pub fn prefs_cliente(db: &dyn Almacen, cliente: &str, cuenta: &str, rol: Rol) -> R<(PrefsCliente, bool)> {
    let v: Option<PrefsCliente> = db.valor(&clave_pref(cliente, cuenta))?.filter(|v| !v.is_empty()).and_then(|v| serde_json::from_str(&v).ok());
    Ok(match v {
        Some(p) => (p, true),
        None => (PrefsCliente::por_defecto(rol), false),
    })
}
pub fn guardar_prefs_cliente(db: &dyn Almacen, cliente: &str, cuenta: &str, p: &PrefsCliente) -> R<()> {
    guardar(db, &clave_pref(cliente, cuenta), p)
}

// ---------- Validar ----------

fn texto_ok(s: &str, max: usize) -> bool {
    !s.trim().is_empty() && s.chars().count() <= max && !s.chars().any(char::is_control)
}

/// «HH:MM» de verdad.
pub fn hora_valida(h: &str) -> bool {
    super::reglas::minutos(h).is_some()
}

fn host_valido(h: &str) -> bool {
    !h.is_empty() && h.len() <= 253 && h.chars().all(|c| c.is_ascii_alphanumeric() || ".-:[]".contains(c)) && !h.starts_with('-')
}

/// El nombre o la IP, sin el puerto ni los corchetes de IPv6.
fn solo_host(host: &str) -> String {
    let h = match host.strip_prefix('[') {
        Some(r) => r.split(']').next().unwrap_or(r),
        None if host.matches(':').count() == 1 => host.split(':').next().unwrap_or(host),
        None => host,
    };
    h.trim_end_matches('.').to_ascii_lowercase()
}

/// Solo el propio equipo (para lo que va sin cifrar).
pub fn es_local(host: &str) -> bool {
    let h = solo_host(host);
    h == "localhost" || h.ends_with(".localhost") || h.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// Este equipo o la red local (IP privadas, de enlace local o de CGNAT, y nombres sin
/// dominio o de dominios locales). Los canales de un cliente no pueden mandar ahí: su
/// propietario no debe poder usar el servidor para llegar a lo que hay en su red.
///
/// Esto solo mira el texto (al guardar el canal). Un nombre público puede resolver a una
/// IP privada: al enviar, `transporte` resuelve el nombre una vez, comprueba cada IP con
/// [`ip_de_red_local`] y conecta a esa misma IP (sin volver a preguntar al DNS).
pub fn es_red_local(host: &str) -> bool {
    let h = solo_host(host);
    match h.parse::<std::net::IpAddr>() {
        Ok(ip) => ip_de_red_local(ip),
        Err(_) => !h.contains('.') || [".localhost", ".local", ".lan", ".internal", ".home.arpa", ".corp"].iter().any(|d| h.ends_with(d)),
    }
}

/// Una IP a la que un canal de un cliente no puede llegar: este equipo, redes privadas,
/// enlace local (con la de metadatos de las nubes, `169.254.169.254`), CGNAT, reservadas,
/// multidifusión, ULA de IPv6 (`fd00:ec2::254`, la de metadatos en IPv6, está ahí), y
/// las IPv6 que llevan dentro una IPv4 (mapeadas, compatibles, NAT64 y 6to4), que se
/// miran por la IPv4 que llevan. Teredo (`2001::/32`) también: esconde la IPv4.
pub fn ip_de_red_local(ip: std::net::IpAddr) -> bool {
    use std::net::{IpAddr, Ipv4Addr};
    match ip {
        IpAddr::V4(v) => {
            let o = v.octets();
            v.is_private()
                || v.is_loopback()
                || v.is_link_local()
                || v.is_unspecified()
                || v.is_broadcast()
                || v.is_multicast()
                || o[0] == 0
                // CGNAT, 100.64.0.0/10 (también la de metadatos de algunas nubes).
                || (o[0] == 100 && (o[1] & 0xc0) == 64)
                // Asignaciones del IETF, 192.0.0.0/24.
                || (o[0] == 192 && o[1] == 0 && o[2] == 0)
                // Pruebas de rendimiento, 198.18.0.0/15.
                || (o[0] == 198 && (o[1] & 0xfe) == 18)
                // Reservadas, 240.0.0.0/4.
                || o[0] >= 240
        }
        IpAddr::V6(v) => {
            let s = v.segments();
            let v4 = |a: u16, b: u16| Ipv4Addr::new((a >> 8) as u8, a as u8, (b >> 8) as u8, b as u8);
            v.is_loopback()
                || v.is_unspecified()
                || v.is_multicast()
                // ULA, fc00::/7.
                || (s[0] & 0xfe00) == 0xfc00
                // Enlace local, fe80::/10, y la antigua de sitio, fec0::/10.
                || (s[0] & 0xffc0) == 0xfe80
                || (s[0] & 0xffc0) == 0xfec0
                // Mapeadas (::ffff:a.b.c.d) y compatibles (::a.b.c.d).
                || v.to_ipv4_mapped().is_some_and(|m| ip_de_red_local(IpAddr::V4(m)))
                || (s[..6].iter().all(|x| *x == 0) && ip_de_red_local(IpAddr::V4(v4(s[6], s[7]))))
                // NAT64, 64:ff9b::/96 y 64:ff9b:1::/48.
                || (s[0] == 0x64 && s[1] == 0xff9b && (s[2..6].iter().all(|x| *x == 0) && ip_de_red_local(IpAddr::V4(v4(s[6], s[7])))))
                || (s[0] == 0x64 && s[1] == 0xff9b && s[2] == 1)
                // 6to4, 2002::/16: la IPv4 va en los segmentos 1 y 2.
                || (s[0] == 0x2002 && ip_de_red_local(IpAddr::V4(v4(s[1], s[2]))))
                // Teredo, 2001::/32.
                || (s[0] == 0x2001 && s[1] == 0)
        }
    }
}

const SOLO_SERVIDOR: &str = "Los canales de un cliente no pueden mandar a este equipo ni a la red local: usa una dirección pública (o pide al propietario del servidor que lo ponga en los canales del servidor).";

/// Partes de una dirección http(s): (https, servidor con puerto, resto). Sin espacios ni
/// caracteres de control.
pub fn partes_url(url: &str) -> Option<(bool, String, String)> {
    if url.len() > 2048 || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return None;
    }
    let (https, resto) = match url.strip_prefix("https://") {
        Some(r) => (true, r),
        None => (false, url.strip_prefix("http://")?),
    };
    let fin = resto.find(['/', '?', '#']).unwrap_or(resto.len());
    let autoridad = &resto[..fin];
    let host = autoridad.rsplit_once('@').map_or(autoridad, |(_, h)| h);
    host_valido(host).then(|| (https, host.to_ascii_lowercase(), resto[fin..].to_string()))
}

/// La dirección pública de la consola: http(s), sin usuario, consulta ni fragmento; sin la `/` final.
pub fn valida_url_consola(url: &str) -> Result<String, String> {
    let url = url.trim().trim_end_matches('/');
    match partes_url(url) {
        Some((_, _, resto)) if !url.contains('@') && !resto.contains(['?', '#']) => Ok(url.to_string()),
        _ => Err("La dirección de la consola tiene que empezar por https:// (o http://), sin usuario ni «?» (p. ej. https://copias.empresa.com:8443).".into()),
    }
}

pub fn valida_reglas(r: &Reglas, del_servidor: bool) -> Result<Reglas, String> {
    let mut r = r.clone();
    r.severidades.sort();
    r.severidades.dedup();
    if !del_servidor {
        r.clientes = None;
    }
    if let Some(c) = &r.clientes {
        if c.len() > 500 || c.iter().any(|id| id.is_empty() || id.len() > 64 || !id.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-')) {
            return Err("Lista de clientes no válida.".into());
        }
    }
    if let Some(s) = &r.silencio {
        if !hora_valida(&s.desde) || !hora_valida(&s.hasta) {
            return Err("Las horas de silencio van como HH:MM (p. ej. 22:00 a 07:00).".into());
        }
    }
    Ok(r)
}

/// Comprueba lo que no es secreto de un canal de ese tipo.
pub fn valida_config(tipo: TipoCanal, c: &ConfigCanal) -> Result<ConfigCanal, String> {
    let mut c = c.clone();
    match tipo {
        TipoCanal::Correo => {
            let host = c.host.as_deref().map(str::trim).unwrap_or_default().to_ascii_lowercase();
            if !host_valido(&host) {
                return Err("Escribe el servidor de correo (SMTP), p. ej. smtp.empresa.com.".into());
            }
            let seguridad = c.seguridad.clone().unwrap_or_else(|| "starttls".into());
            if !matches!(seguridad.as_str(), "starttls" | "tls" | "ninguna") {
                return Err("Seguridad del correo: STARTTLS, TLS o ninguna.".into());
            }
            if seguridad == "ninguna" && !es_local(&host) {
                return Err("Sin cifrar solo se admite un servidor de correo en este mismo equipo (localhost): usa STARTTLS o TLS.".into());
            }
            let puerto = c.puerto.unwrap_or(if seguridad == "tls" { 465 } else { 587 });
            if puerto == 0 {
                return Err("Puerto no válido.".into());
            }
            let remitente = c.remitente.as_deref().map(str::trim).unwrap_or_default().to_string();
            if remitente.parse::<lettre::message::Mailbox>().is_err() || remitente.len() > 320 {
                return Err("Escribe el remitente, p. ej. «Resguardo <copias@empresa.com>».".into());
            }
            let usuario = c.usuario.as_deref().map(str::trim).filter(|u| !u.is_empty()).map(str::to_string);
            if usuario.as_deref().is_some_and(|u| !texto_ok(u, 254)) {
                return Err("Usuario no válido.".into());
            }
            c = ConfigCanal { host: Some(host), puerto: Some(puerto), seguridad: Some(seguridad), usuario, remitente: Some(remitente), ..Default::default() };
        }
        TipoCanal::Telegram => {
            let chat = c.chat_id.as_deref().map(str::trim).unwrap_or_default().to_string();
            let numero = chat.strip_prefix('-').unwrap_or(&chat);
            let ok = (!numero.is_empty() && numero.len() <= 20 && numero.chars().all(|ch| ch.is_ascii_digit()))
                || (chat.starts_with('@') && chat.len() >= 6 && chat.len() <= 64 && chat[1..].chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_'));
            if !ok {
                return Err("El chat de Telegram es un número (p. ej. -1001234567890) o @nombre_del_canal.".into());
            }
            c = ConfigCanal { chat_id: Some(chat), ..Default::default() };
        }
        // La dirección es un secreto: aquí solo queda su servidor (lo pone `aplicar`).
        TipoCanal::Webhook | TipoCanal::Ntfy => c = ConfigCanal { servidor: c.servidor.clone(), ..Default::default() },
    }
    Ok(c)
}

/// Comprueba un secreto de ese tipo y campo; devuelve lo que se puede enseñar (el servidor de una dirección).
pub fn valida_secreto(tipo: TipoCanal, campo: &str, v: &str) -> Result<Option<String>, String> {
    if !tipo.campos_secretos().contains(&campo) {
        return Err(format!("Este canal no tiene «{campo}»."));
    }
    if v.len() > 2048 || v.chars().any(char::is_control) {
        return Err("Valor no válido.".into());
    }
    match (tipo, campo) {
        (TipoCanal::Webhook | TipoCanal::Ntfy, "url") => {
            let (https, host, resto) = partes_url(v.trim()).ok_or("Escribe la dirección completa, con https://.")?;
            if !https && !es_local(&host) {
                return Err("La dirección tiene que ser https:// (http:// solo en este mismo equipo).".into());
            }
            if tipo == TipoCanal::Ntfy {
                let tema = resto.trim_start_matches('/');
                if tema.is_empty() || tema.len() > 64 || !tema.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-') {
                    return Err("La dirección de ntfy es la del tema, p. ej. https://ntfy.sh/copias-empresa-x7k2.".into());
                }
            }
            Ok(Some(host))
        }
        (TipoCanal::Telegram, "token") => {
            let ok = v.split_once(':').is_some_and(|(n, t)| {
                !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) && t.len() >= 30 && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            });
            if ok {
                Ok(None)
            } else {
                Err("El token del bot de Telegram tiene la forma 123456789:AA… (te lo da @BotFather).".into())
            }
        }
        (TipoCanal::Webhook, "secreto") if v.len() < 16 => Err("El secreto para firmar tiene que tener al menos 16 caracteres.".into()),
        _ => Ok(None),
    }
}

/// ¿Tiene lo necesario para enviar?
pub fn completo(c: &Canal) -> bool {
    match c.tipo {
        TipoCanal::Correo => c.config.host.is_some() && c.config.remitente.is_some(),
        TipoCanal::Webhook | TipoCanal::Ntfy => c.secretos.contains_key("url"),
        TipoCanal::Telegram => c.secretos.contains_key("token") && c.config.chat_id.is_some(),
    }
}

/// Un cambio de canal tal como llega de la consola.
#[derive(Deserialize, Default, Debug)]
pub struct CambioCanal {
    pub tipo: Option<TipoCanal>,
    pub nombre: Option<String>,
    pub activo: Option<bool>,
    pub config: Option<ConfigCanal>,
    /// Campo → valor nuevo; `""` lo quita; lo que no viene se queda como estaba.
    pub secretos: Option<BTreeMap<String, String>>,
    pub reglas: Option<Reglas>,
    /// Código de la aplicación de autenticación (cambiar a dónde va o un secreto).
    pub codigo: Option<String>,
}

/// Lo que cambió (para la auditoría y para saber si hace falta el código).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Cambiado {
    /// Cambió a dónde va (configuración o secretos): hace falta un código recién sacado.
    pub sensible: bool,
    pub secretos: Vec<String>,
}

/// Aplica el cambio sobre el canal (`None`: uno nuevo). No guarda nada.
pub fn aplicar(previo: Option<&Canal>, c: &CambioCanal, clave: &Clave, ambito: &str, por: &str, ahora: Ts) -> Result<(Canal, Cambiado), String> {
    let tipo = match (previo, c.tipo) {
        (Some(p), Some(t)) if p.tipo != t => return Err("No se puede cambiar el tipo de un canal: crea otro.".into()),
        (Some(p), _) => p.tipo,
        (None, Some(t)) => t,
        (None, None) => return Err("Falta el tipo de canal.".into()),
    };
    let mut canal = previo.cloned().unwrap_or_else(|| Canal {
        id: uuid::Uuid::new_v4().to_string(),
        tipo,
        nombre: String::new(),
        activo: true,
        config: ConfigCanal::default(),
        secretos: BTreeMap::new(),
        reglas: Reglas::default(),
        actualizado: ahora,
        por: String::new(),
    });
    let mut cambiado = Cambiado { sensible: previo.is_none(), ..Default::default() };
    if let Some(n) = &c.nombre {
        if !texto_ok(n, 80) {
            return Err("Escribe un nombre para el canal (hasta 80 caracteres).".into());
        }
        canal.nombre = n.trim().to_string();
    }
    if canal.nombre.is_empty() {
        canal.nombre = match tipo {
            TipoCanal::Correo => "Correo",
            TipoCanal::Webhook => "Webhook",
            TipoCanal::Ntfy => "ntfy",
            TipoCanal::Telegram => "Telegram",
        }
        .into();
    }
    if let Some(a) = c.activo {
        canal.activo = a;
    }
    if let Some(r) = &c.reglas {
        canal.reglas = valida_reglas(r, ambito == "servidor")?;
    }
    if let Some(cfg) = &c.config {
        let mut nueva = valida_config(tipo, cfg)?;
        if ambito != "servidor" && nueva.host.as_deref().is_some_and(es_red_local) {
            return Err(SOLO_SERVIDOR.into());
        }
        // El servidor de la dirección lo pone el secreto, no la consola.
        nueva.servidor = canal.config.servidor.clone();
        if nueva != canal.config {
            cambiado.sensible = true;
        }
        canal.config = nueva;
    }
    if let Some(s) = &c.secretos {
        for (campo, valor) in s {
            let ctx = contexto(ambito, &canal.id, campo);
            if valor.is_empty() {
                if canal.secretos.remove(campo).is_some() {
                    cambiado.secretos.push(campo.clone());
                }
                if campo == "url" {
                    canal.config.servidor = None;
                }
                continue;
            }
            let visible = valida_secreto(tipo, campo, valor)?;
            if ambito != "servidor" && visible.as_deref().is_some_and(es_red_local) {
                return Err(SOLO_SERVIDOR.into());
            }
            let valor = if campo == "url" { valor.trim() } else { valor.as_str() };
            canal.secretos.insert(campo.clone(), clave.cerrar(&ctx, valor));
            cambiado.secretos.push(campo.clone());
            if campo == "url" {
                canal.config.servidor = visible;
            }
        }
        if !cambiado.secretos.is_empty() {
            cambiado.sensible = true;
        }
    }
    if previo.is_none() && tipo == TipoCanal::Correo && c.config.is_none() {
        return Err("Faltan los datos del servidor de correo.".into());
    }
    canal.actualizado = ahora;
    canal.por = por.to_string();
    Ok((canal, cambiado))
}

/// El canal como lo ve la consola: sin un solo secreto, solo cuáles están puestos.
pub fn vista_canal(c: &Canal) -> Value {
    let secretos: serde_json::Map<String, Value> = c.secretos.keys().map(|k| (k.clone(), json!("configurado"))).collect();
    json!({
        "id": c.id, "tipo": c.tipo, "nombre": c.nombre, "activo": c.activo, "config": c.config,
        "secretos": secretos, "reglas": c.reglas, "completo": completo(c),
        "actualizado": crate::api::fecha(c.actualizado), "por": c.por,
    })
}

/// Los secretos de un canal, abiertos, para enviar (solo en memoria y mientras se envía).
pub fn abrir_secretos(c: &Canal, clave: &Clave, ambito: &str) -> BTreeMap<String, String> {
    c.secretos.iter().filter_map(|(campo, v)| clave.abrir(&contexto(ambito, &c.id, campo), v).map(|t| (campo.clone(), t))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clave() -> Clave {
        Clave::fija([3; 32])
    }

    fn correo() -> CambioCanal {
        CambioCanal {
            tipo: Some(TipoCanal::Correo),
            nombre: Some("Correo de la oficina".into()),
            config: Some(ConfigCanal {
                host: Some("SMTP.Empresa.com".into()),
                seguridad: Some("starttls".into()),
                usuario: Some("avisos@empresa.com".into()),
                remitente: Some("Resguardo <avisos@empresa.com>".into()),
                ..Default::default()
            }),
            secretos: Some(BTreeMap::from([("contrasena".to_string(), "contraseña-smtp-SECRETA".to_string())])),
            ..Default::default()
        }
    }

    #[test]
    fn la_vista_nunca_lleva_secretos() {
        let (canal, cambiado) = aplicar(None, &correo(), &clave(), "servidor", "ana@ejemplo.com", 1).unwrap();
        assert!(cambiado.sensible);
        assert_eq!(cambiado.secretos, vec!["contrasena".to_string()]);
        assert_eq!(canal.config.host.as_deref(), Some("smtp.empresa.com"));
        assert_eq!(canal.config.puerto, Some(587));
        // Guardado: cifrado.
        let guardado = serde_json::to_string(&canal).unwrap();
        assert!(!guardado.contains("SECRETA"), "{guardado}");
        // Vista: «configurado», nunca el valor ni el cifrado.
        let v = vista_canal(&canal).to_string();
        assert!(!v.contains("SECRETA") && !v.contains("n1:"), "{v}");
        assert!(v.contains("\"contrasena\":\"configurado\""), "{v}");
        // Y se puede abrir para enviar.
        assert_eq!(abrir_secretos(&canal, &clave(), "servidor")["contrasena"], "contraseña-smtp-SECRETA");
        // Con otra clave, o en otro ámbito, no.
        assert!(abrir_secretos(&canal, &Clave::fija([4; 32]), "servidor").is_empty());
        assert!(abrir_secretos(&canal, &clave(), "cliente:x").is_empty());
    }

    #[test]
    fn que_cambio_pide_codigo() {
        let (canal, _) = aplicar(None, &correo(), &clave(), "servidor", "ana", 1).unwrap();
        // Nombre, encendido y reglas: no.
        let c = CambioCanal { nombre: Some("Otro nombre".into()), activo: Some(false), reglas: Some(Reglas::default()), ..Default::default() };
        let (c2, cambiado) = aplicar(Some(&canal), &c, &clave(), "servidor", "ana", 2).unwrap();
        assert!(!cambiado.sensible);
        assert!(!c2.activo);
        assert_eq!(c2.secretos, canal.secretos);
        // La misma configuración otra vez: no.
        let c = CambioCanal { config: correo().config, ..Default::default() };
        assert!(!aplicar(Some(&canal), &c, &clave(), "servidor", "ana", 2).unwrap().1.sensible);
        // Otro servidor de correo (se llevaría la contraseña guardada): sí.
        let mut cfg = correo().config.unwrap();
        cfg.host = Some("smtp.otro.com".into());
        let c = CambioCanal { config: Some(cfg), ..Default::default() };
        assert!(aplicar(Some(&canal), &c, &clave(), "servidor", "ana", 2).unwrap().1.sensible);
        // Un secreto nuevo o quitarlo: sí.
        let c = CambioCanal { secretos: Some(BTreeMap::from([("contrasena".to_string(), String::new())])), ..Default::default() };
        let (c3, cambiado) = aplicar(Some(&canal), &c, &clave(), "servidor", "ana", 2).unwrap();
        assert!(cambiado.sensible && c3.secretos.is_empty());
        // No se cambia de tipo ni se admiten campos de otro tipo.
        let c = CambioCanal { tipo: Some(TipoCanal::Webhook), ..Default::default() };
        assert!(aplicar(Some(&canal), &c, &clave(), "servidor", "ana", 2).is_err());
        let c = CambioCanal { secretos: Some(BTreeMap::from([("token".to_string(), "x".to_string())])), ..Default::default() };
        assert!(aplicar(Some(&canal), &c, &clave(), "servidor", "ana", 2).is_err());
    }

    #[test]
    fn validaciones() {
        // Sin cifrar, solo en este equipo.
        let mut c = correo();
        c.config.as_mut().unwrap().seguridad = Some("ninguna".into());
        assert!(aplicar(None, &c, &clave(), "servidor", "a", 1).is_err());
        c.config.as_mut().unwrap().host = Some("localhost".into());
        assert!(aplicar(None, &c, &clave(), "servidor", "a", 1).is_ok());
        // Webhook: https, y de la dirección solo se enseña el servidor.
        let w = |url: &str| CambioCanal {
            tipo: Some(TipoCanal::Webhook),
            secretos: Some(BTreeMap::from([("url".to_string(), url.to_string())])),
            ..Default::default()
        };
        assert!(aplicar(None, &w("http://hooks.ejemplo.com/x"), &clave(), "servidor", "a", 1).is_err());
        assert!(aplicar(None, &w("ftp://hooks.ejemplo.com/x"), &clave(), "servidor", "a", 1).is_err());
        let (canal, _) = aplicar(None, &w("https://hooks.ejemplo.com/servicios/T000/B000/XXXXSECRETO"), &clave(), "servidor", "a", 1).unwrap();
        assert_eq!(canal.config.servidor.as_deref(), Some("hooks.ejemplo.com"));
        assert!(!vista_canal(&canal).to_string().contains("XXXXSECRETO"));
        assert!(aplicar(None, &w("http://127.0.0.1:9000/x"), &clave(), "servidor", "a", 1).is_ok());
        // ntfy: la dirección del tema.
        let n = |url: &str| CambioCanal {
            tipo: Some(TipoCanal::Ntfy),
            secretos: Some(BTreeMap::from([("url".to_string(), url.to_string())])),
            ..Default::default()
        };
        assert!(aplicar(None, &n("https://ntfy.sh/"), &clave(), "servidor", "a", 1).is_err());
        assert!(aplicar(None, &n("https://ntfy.sh/copias-empresa_1"), &clave(), "servidor", "a", 1).is_ok());
        // Telegram: token y chat con su forma.
        let t = |token: &str, chat: &str| CambioCanal {
            tipo: Some(TipoCanal::Telegram),
            config: Some(ConfigCanal { chat_id: Some(chat.into()), ..Default::default() }),
            secretos: Some(BTreeMap::from([("token".to_string(), token.to_string())])),
            ..Default::default()
        };
        assert!(aplicar(None, &t("123456:ABCDEFGHIJKLMNOPQRSTUVWXYZabcdef123", "-1001234567890"), &clave(), "servidor", "a", 1).is_ok());
        assert!(aplicar(None, &t("no-es-un-token", "-100"), &clave(), "servidor", "a", 1).is_err());
        assert!(aplicar(None, &t("123456:ABCDEFGHIJKLMNOPQRSTUVWXYZabcdef123", "mi chat"), &clave(), "servidor", "a", 1).is_err());
        // La consola.
        assert_eq!(valida_url_consola("https://copias.empresa.com:8443/").unwrap(), "https://copias.empresa.com:8443");
        assert!(valida_url_consola("https://u:p@copias.empresa.com").is_err());
        assert!(valida_url_consola("https://copias.empresa.com/?x=1").is_err());
        assert!(valida_url_consola("javascript:alert(1)").is_err());
        assert!(es_local("127.0.0.1:25") && es_local("localhost") && es_local("[::1]:25") && !es_local("smtp.empresa.com"));
        // Los canales de un cliente, nunca a este equipo ni a la red local; los del servidor, sí.
        for local in
            ["10.0.0.5", "192.168.1.20:8080", "172.20.0.1", "169.254.169.254", "127.0.0.1:9000", "[::1]", "[fd00::1]", "intranet", "nas.local", "100.64.0.1"]
        {
            assert!(es_red_local(local), "{local}");
            let w = |amb: &str| {
                aplicar(
                    None,
                    &CambioCanal {
                        tipo: Some(TipoCanal::Webhook),
                        secretos: Some(BTreeMap::from([("url".to_string(), format!("https://{local}/x"))])),
                        ..Default::default()
                    },
                    &clave(),
                    amb,
                    "a",
                    1,
                )
            };
            assert!(w("cliente:c1").is_err(), "{local}");
            assert!(w("servidor").is_ok(), "{local}");
        }
        for publico in ["hooks.ejemplo.com", "8.8.8.8", "[2001:db8::1]", "ntfy.sh:443"] {
            assert!(!es_red_local(publico), "{publico}");
        }
        // Las IP de dentro, también escondidas en IPv6.
        for ip in [
            "169.254.169.254",
            "100.100.100.200",
            "0.0.0.0",
            "224.0.0.1",
            "198.18.0.1",
            "255.255.255.255",
            "::ffff:127.0.0.1",
            "::ffff:169.254.169.254",
            "::127.0.0.1",
            "::",
            "fd00:ec2::254",
            "fe80::1",
            "fec0::1",
            "ff02::1",
            "64:ff9b::a00:1",
            "2002:c0a8:0101::1",
            "2001:0:4136:e378::1",
        ] {
            assert!(ip_de_red_local(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["8.8.8.8", "1.1.1.1", "2606:4700:4700::1111", "::ffff:8.8.8.8", "64:ff9b::808:808", "2002:0808:0808::1"] {
            assert!(!ip_de_red_local(ip.parse().unwrap()), "{ip}");
        }
        let mut c = correo();
        c.config.as_mut().unwrap().host = Some("192.168.1.10".into());
        assert!(aplicar(None, &c, &clave(), "cliente:c1", "a", 1).is_err());
        assert!(aplicar(None, &c, &clave(), "servidor", "a", 1).is_ok());
    }
}
