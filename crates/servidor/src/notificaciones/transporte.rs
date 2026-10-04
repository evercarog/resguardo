//! Cómo sale cada mensaje: correo por SMTP (lettre, con rustls: sin OpenSSL)
//! y peticiones HTTPS (ureq, con rustls) para el webhook, ntfy y Telegram.
//!
//! Lo que se manda se prepara aquí sin red ([`peticion`], [`correo`]) para
//! poder probarlo; el [`Transporte`] solo lo lleva (el de verdad, [`Real`], o
//! uno falso en las pruebas). Cada envío tiene su límite de tiempo y sus
//! errores se cuentan en palabras, sin la dirección ni ningún secreto.

use super::ajustes::{Canal, TipoCanal};
use super::contenido::{self, Formato, Salida};
use super::Mensaje;
use crate::almacen::Ts;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::BTreeMap;
use std::time::Duration;

/// Lo más que espera cada envío.
pub const ESPERA: Duration = Duration::from_secs(20);

/// Por qué no salió. `permanente`: reintentar no lo arregla (credenciales, dirección…).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fallo {
    pub permanente: bool,
    pub texto: String,
}

impl Fallo {
    fn temporal(t: impl Into<String>) -> Self {
        Self { permanente: false, texto: t.into() }
    }
    fn definitivo(t: impl Into<String>) -> Self {
        Self { permanente: true, texto: t.into() }
    }
}

/// Una petición HTTP POST lista para salir.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeticionHttp {
    pub url: String,
    pub cabeceras: Vec<(String, String)>,
    pub cuerpo: Vec<u8>,
}

/// Un correo listo para salir.
#[derive(Clone, Debug)]
pub struct Correo {
    pub host: String,
    pub puerto: u16,
    pub seguridad: String,
    pub usuario: Option<String>,
    pub contrasena: Option<String>,
    pub mensaje: lettre::Message,
    /// Lo mismo que va dentro de `mensaje`, sin codificar (para las pruebas).
    pub asunto: String,
    pub texto: String,
    pub html: String,
}

pub trait Transporte: Send + Sync {
    fn http(&self, p: &PeticionHttp) -> Result<(), Fallo>;
    fn correo(&self, c: &Correo) -> Result<(), Fallo>;
}

/// Firma del webhook: `t=<unix>,v1=<hex HMAC-SHA256(secreto, "<unix>.<cuerpo>")>`. El que
/// recibe comprueba la firma y que `t` sea reciente (contra repeticiones).
pub fn firma_webhook(secreto: &str, t: Ts, cuerpo: &[u8]) -> String {
    let mut m = Hmac::<Sha256>::new_from_slice(secreto.as_bytes()).expect("HMAC acepta cualquier longitud");
    m.update(t.to_string().as_bytes());
    m.update(b".");
    m.update(cuerpo);
    let h: String = m.finalize().into_bytes().iter().map(|b| format!("{b:02x}")).collect();
    format!("t={t},v1={h}")
}

fn agente_usuario() -> String {
    format!("Resguardo-Server/{}", env!("CARGO_PKG_VERSION"))
}

/// La petición de un canal HTTP (webhook, ntfy o Telegram).
#[allow(clippy::too_many_arguments)]
pub fn peticion(
    canal: &Canal,
    secretos: &BTreeMap<String, String>,
    ms: &[Mensaje],
    s: &Salida,
    f: &Formato,
    id: &str,
    ahora: Ts,
) -> Result<PeticionHttp, Fallo> {
    let falta = || Fallo::definitivo("Al canal le faltan datos (vuelve a escribir sus secretos).");
    let json = |v: &serde_json::Value| serde_json::to_vec(v).unwrap_or_default();
    let mut cabeceras = vec![("Content-Type".to_string(), "application/json".to_string()), ("User-Agent".to_string(), agente_usuario())];
    match canal.tipo {
        TipoCanal::Webhook => {
            let url = secretos.get("url").ok_or_else(falta)?.clone();
            let cuerpo = json(&contenido::webhook(ms, s, f, id, ahora));
            cabeceras.push(("X-Resguardo-Evento".into(), s.evento.into()));
            cabeceras.push(("X-Resguardo-Entrega".into(), id.into()));
            if let Some(sec) = secretos.get("secreto") {
                cabeceras.push(("X-Resguardo-Firma".into(), firma_webhook(sec, ahora, &cuerpo)));
            }
            Ok(PeticionHttp { url, cabeceras, cuerpo })
        }
        TipoCanal::Ntfy => {
            let url = secretos.get("url").ok_or_else(falta)?;
            let (https, host, resto) = super::ajustes::partes_url(url).ok_or_else(falta)?;
            let resto = resto.split(['?', '#']).next().unwrap_or_default().trim_matches('/').to_string();
            let (prefijo, tema) = resto.rsplit_once('/').map_or((String::new(), resto.clone()), |(a, b)| (format!("/{a}"), b.to_string()));
            let (prioridad, etiqueta) = match (s.evento, s.severidad) {
                ("recuperacion" | "prueba", _) => (3, "white_check_mark"),
                ("resumen", _) => (2, "clipboard"),
                (_, super::Severidad::Critico) => (5, "rotating_light"),
                (_, super::Severidad::Importante) => (4, "warning"),
                _ => (3, "information_source"),
            };
            let texto: String = s.texto.split("\n—\n").next().unwrap_or(&s.texto).chars().take(3500).collect();
            let mut v = serde_json::json!({ "topic": tema, "title": s.asunto, "message": texto, "priority": prioridad, "tags": [etiqueta] });
            if let Some(e) = &s.enlace {
                v["click"] = serde_json::json!(e);
            }
            if let Some(t) = secretos.get("token") {
                cabeceras.push(("Authorization".into(), format!("Bearer {t}")));
            }
            Ok(PeticionHttp { url: format!("{}://{host}{prefijo}", if https { "https" } else { "http" }), cabeceras, cuerpo: json(&v) })
        }
        TipoCanal::Telegram => {
            let token = secretos.get("token").ok_or_else(falta)?;
            let chat = canal.config.chat_id.clone().ok_or_else(falta)?;
            let chat_v = chat.parse::<i64>().map(serde_json::Value::from).unwrap_or_else(|_| serde_json::Value::from(chat));
            let v = serde_json::json!({ "chat_id": chat_v, "text": contenido::telegram(s), "parse_mode": "HTML", "disable_web_page_preview": true });
            Ok(PeticionHttp { url: format!("https://api.telegram.org/bot{token}/sendMessage"), cabeceras, cuerpo: json(&v) })
        }
        TipoCanal::Correo => Err(Fallo::definitivo("Un canal de correo no va por HTTP.")),
    }
}

/// El cuerpo: texto y HTML (multipart/alternative); con el logo del cliente, el
/// HTML y su imagen van juntos (multipart/related, `Content-ID` [`contenido::CID_LOGO`]):
/// el correo no carga nada de fuera.
fn cuerpo(s: &Salida) -> lettre::message::MultiPart {
    use lettre::message::header::ContentType;
    use lettre::message::{Attachment, MultiPart, SinglePart};
    match &s.logo {
        None => MultiPart::alternative_plain_html(s.texto.clone(), s.html.clone()),
        Some(png) => MultiPart::alternative().singlepart(SinglePart::plain(s.texto.clone())).multipart(
            MultiPart::related()
                .singlepart(SinglePart::html(s.html.clone()))
                .singlepart(Attachment::new_inline(contenido::CID_LOGO.to_string()).body(png.clone(), ContentType::parse("image/png").expect("tipo válido"))),
        ),
    }
}

/// El correo (multiparte: texto y HTML, y el logo del cliente si lo tiene) para una persona.
pub fn correo(canal: &Canal, secretos: &BTreeMap<String, String>, para: &str, s: &Salida) -> Result<Correo, Fallo> {
    use lettre::message::header::{HeaderName, HeaderValue};
    use lettre::message::Mailbox;
    let falta = |q: &str| Fallo::definitivo(format!("Al canal de correo le falta {q}."));
    let de: Mailbox = canal.config.remitente.as_deref().ok_or_else(|| falta("el remitente"))?.parse().map_err(|_| falta("un remitente válido"))?;
    let a: Mailbox = para.parse().map_err(|_| Fallo::definitivo("La dirección de la persona no es válida."))?;
    let mensaje = lettre::Message::builder()
        .from(de)
        .to(a)
        .subject(s.asunto.clone())
        .raw_header(HeaderValue::new(HeaderName::new_from_ascii_str("Auto-Submitted"), "auto-generated".into()))
        .raw_header(HeaderValue::new(HeaderName::new_from_ascii_str("X-Auto-Response-Suppress"), "All".into()))
        .multipart(cuerpo(s))
        .map_err(|_| Fallo::definitivo("No se pudo preparar el correo."))?;
    Ok(Correo {
        host: canal.config.host.clone().ok_or_else(|| falta("el servidor"))?,
        puerto: canal.config.puerto.unwrap_or(587),
        seguridad: canal.config.seguridad.clone().unwrap_or_else(|| "starttls".into()),
        usuario: canal.config.usuario.clone(),
        contrasena: secretos.get("contrasena").cloned(),
        mensaje,
        asunto: s.asunto.clone(),
        texto: s.texto.clone(),
        html: s.html.clone(),
    })
}

/// Escribe y manda uno o varios mensajes (agrupados) por un canal a un destino.
#[allow(clippy::too_many_arguments)]
pub fn enviar(
    t: &dyn Transporte,
    canal: &Canal,
    secretos: &BTreeMap<String, String>,
    destino: &str,
    ms: &[Mensaje],
    f: &Formato,
    id: &str,
    ahora: Ts,
) -> Result<(), Fallo> {
    let s = contenido::componer(ms, f);
    let r = match canal.tipo {
        TipoCanal::Correo => t.correo(&correo(canal, secretos, destino, &s)?),
        _ => t.http(&peticion(canal, secretos, ms, &s, f, id, ahora)?),
    };
    // Por si algún texto de error repitiera un secreto: fuera.
    r.map_err(|mut e| {
        for v in secretos.values().filter(|v| v.len() >= 4) {
            e.texto = e.texto.replace(v.as_str(), "•••");
        }
        e
    })
}

// ---------- El de verdad ----------

/// Por SMTP y HTTPS de verdad.
pub struct Real;

fn http_estado(estado: u16, servicio: &str) -> Result<(), Fallo> {
    match estado {
        200..=299 => Ok(()),
        401 | 403 => Err(Fallo::definitivo(format!("{servicio} no aceptó las credenciales (HTTP {estado})."))),
        404 => Err(Fallo::definitivo(format!("{servicio} no encuentra la dirección o el chat (HTTP 404)."))),
        408 | 425 | 429 => Err(Fallo::temporal(format!("{servicio} pide esperar (HTTP {estado})."))),
        500..=599 => Err(Fallo::temporal(format!("{servicio} tuvo un problema (HTTP {estado})."))),
        _ => Err(Fallo::definitivo(format!("{servicio} rechazó el mensaje (HTTP {estado})."))),
    }
}

fn http_error(e: &ureq::Error) -> Fallo {
    use ureq::Error as E;
    // Nunca el texto de ureq: puede llevar la dirección (y en Telegram, el token).
    Fallo::temporal(match e {
        E::HostNotFound => "No se encuentra el servidor (DNS).",
        E::Timeout(_) => "No respondió a tiempo.",
        E::ConnectionFailed | E::Io(_) => "No se pudo conectar.",
        E::Tls(_) | E::Rustls(_) => "Problema con el certificado (TLS).",
        _ => "Error de red.",
    })
}

impl Transporte for Real {
    fn http(&self, p: &PeticionHttp) -> Result<(), Fallo> {
        use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
        let tls = TlsConfig::builder().provider(TlsProvider::Rustls).root_certs(RootCerts::WebPki).build();
        let agente =
            ureq::Agent::config_builder().timeout_global(Some(ESPERA)).http_status_as_error(false).max_redirects(0).tls_config(tls).build().new_agent();
        let mut r = agente.post(&p.url);
        for (k, v) in &p.cabeceras {
            r = r.header(k, v);
        }
        let servicio = if p.url.starts_with("https://api.telegram.org/") { "Telegram" } else { "El servicio" };
        let resp = r.send(&p.cuerpo[..]).map_err(|e| http_error(&e))?;
        http_estado(resp.status().as_u16(), servicio)
    }

    fn correo(&self, c: &Correo) -> Result<(), Fallo> {
        use lettre::transport::smtp::authentication::Credentials;
        use lettre::transport::smtp::client::{Tls, TlsParameters};
        use lettre::{SmtpTransport, Transport};
        let parametros = || TlsParameters::new(c.host.clone()).map_err(|_| Fallo::definitivo("No se pudo preparar TLS para el servidor de correo."));
        let tls = match c.seguridad.as_str() {
            "tls" => Tls::Wrapper(parametros()?),
            "ninguna" => Tls::None,
            _ => Tls::Required(parametros()?),
        };
        let mut b = SmtpTransport::builder_dangerous(c.host.clone()).port(c.puerto).tls(tls).timeout(Some(ESPERA));
        if let (Some(u), Some(p)) = (&c.usuario, &c.contrasena) {
            b = b.credentials(Credentials::new(u.clone(), p.clone()));
        }
        b.build().send(&c.mensaje).map(|_| ()).map_err(|e| {
            let codigo = e.status().map(|c| format!(" ({c})")).unwrap_or_default();
            if e.is_permanent() {
                let auth = e.status().is_some_and(|c| c.to_string().starts_with("535") || c.to_string().starts_with("534") || c.to_string().starts_with("530"));
                Fallo::definitivo(if auth {
                    format!("El servidor de correo no aceptó el usuario o la contraseña{codigo}.")
                } else {
                    format!("El servidor de correo rechazó el mensaje{codigo}.")
                })
            } else if e.is_transient() {
                Fallo::temporal(format!("El servidor de correo pide reintentar más tarde{codigo}."))
            } else if e.is_timeout() {
                Fallo::temporal("El servidor de correo no respondió a tiempo.")
            } else if e.is_tls() {
                Fallo::temporal("Problema con el certificado (TLS) del servidor de correo.")
            } else {
                Fallo::temporal("No se pudo conectar con el servidor de correo.")
            }
        })
    }
}

// ---------- Falso, para las pruebas ----------

/// Guarda lo que «manda» y falla cuando se le dice.
#[derive(Default)]
pub struct Falso {
    pub enviados: std::sync::Mutex<Vec<Enviado>>,
    /// Los próximos envíos fallan con esto (uno por envío, de delante hacia atrás).
    pub fallos: std::sync::Mutex<Vec<Fallo>>,
}

#[derive(Clone, Debug)]
pub enum Enviado {
    Http(PeticionHttp),
    Correo { para: String, asunto: String, texto: String, html: String, crudo: String },
}

impl Falso {
    fn siguiente_fallo(&self) -> Option<Fallo> {
        let mut f = self.fallos.lock().unwrap_or_else(|e| e.into_inner());
        (!f.is_empty()).then(|| f.remove(0))
    }
    pub fn enviados(&self) -> Vec<Enviado> {
        self.enviados.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    pub fn fallar(&self, f: Fallo) {
        self.fallos.lock().unwrap_or_else(|e| e.into_inner()).push(f);
    }
}

impl Transporte for Falso {
    fn http(&self, p: &PeticionHttp) -> Result<(), Fallo> {
        if let Some(f) = self.siguiente_fallo() {
            return Err(f);
        }
        self.enviados.lock().unwrap_or_else(|e| e.into_inner()).push(Enviado::Http(p.clone()));
        Ok(())
    }
    fn correo(&self, c: &Correo) -> Result<(), Fallo> {
        if let Some(f) = self.siguiente_fallo() {
            return Err(f);
        }
        let crudo = String::from_utf8_lossy(&c.mensaje.formatted()).to_string();
        let para = c.mensaje.envelope().to().iter().map(|a| a.to_string()).collect::<Vec<_>>().join(",");
        let e = Enviado::Correo { para, asunto: c.asunto.clone(), texto: c.texto.clone(), html: c.html.clone(), crudo };
        self.enviados.lock().unwrap_or_else(|e| e.into_inner()).push(e);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notificaciones::ajustes::{ConfigCanal, Reglas};

    fn canal(tipo: TipoCanal, config: ConfigCanal) -> Canal {
        Canal {
            id: "k".into(),
            tipo,
            nombre: "n".into(),
            activo: true,
            config,
            secretos: Default::default(),
            reglas: Reglas::default(),
            actualizado: 0,
            por: String::new(),
        }
    }

    fn formato() -> Formato {
        Formato { url_consola: Some("https://copias.ejemplo.com".into()), zona: chrono::FixedOffset::east_opt(0).unwrap(), marcas: Default::default() }
    }

    fn prueba() -> Vec<Mensaje> {
        vec![Mensaje::Prueba { quien: "ana@ejemplo.com".into() }]
    }

    #[test]
    fn firma_hmac_del_webhook() {
        // Calculado aparte: hmac.new(b"secreto-compartido-123", b'1791100800.{"a":1}', hashlib.sha256).hexdigest().
        assert_eq!(firma_webhook("secreto-compartido-123", 1_791_100_800, br#"{"a":1}"#), format!("t=1791100800,v1={}", VECTOR_FIRMA));
        // El cuerpo firmado es el que sale, con sus cabeceras.
        let c = canal(TipoCanal::Webhook, ConfigCanal::default());
        let sec =
            BTreeMap::from([("url".to_string(), "https://hooks.ejemplo.com/x".to_string()), ("secreto".to_string(), "secreto-compartido-123".to_string())]);
        let ms = prueba();
        let s = contenido::componer(&ms, &formato());
        let p = peticion(&c, &sec, &ms, &s, &formato(), "id-1", 1_791_100_800).unwrap();
        let firma = p.cabeceras.iter().find(|(k, _)| k == "X-Resguardo-Firma").unwrap().1.clone();
        assert_eq!(firma, firma_webhook("secreto-compartido-123", 1_791_100_800, &p.cuerpo));
        let v: serde_json::Value = serde_json::from_slice(&p.cuerpo).unwrap();
        assert_eq!((v["version"].as_i64(), v["evento"].as_str(), v["id"].as_str()), (Some(1), Some("prueba"), Some("id-1")));
        assert!(p.cabeceras.contains(&("X-Resguardo-Evento".into(), "prueba".into())));
        // Sin secreto, sin firma.
        let sec = BTreeMap::from([("url".to_string(), "https://hooks.ejemplo.com/x".to_string())]);
        let p = peticion(&c, &sec, &ms, &s, &formato(), "id-1", 1).unwrap();
        assert!(!p.cabeceras.iter().any(|(k, _)| k == "X-Resguardo-Firma"));
    }

    /// HMAC-SHA256 de referencia, calculado con Python y con `openssl dgst -sha256 -hmac`
    /// (ver `firma_hmac_del_webhook` y docs/api-servidor.md, «Webhook»).
    pub const VECTOR_FIRMA: &str = "d0524a455d5fce0165253c72ecdb2ae9f8f8e523d44886ab7a258a155dd39931";

    #[test]
    fn ntfy_y_telegram() {
        let ms = prueba();
        let s = contenido::componer(&ms, &formato());
        let c = canal(TipoCanal::Ntfy, ConfigCanal::default());
        let sec =
            BTreeMap::from([("url".to_string(), "https://ntfy.ejemplo.com/avisos/copias-x7".to_string()), ("token".to_string(), "tk_secreto".to_string())]);
        let p = peticion(&c, &sec, &ms, &s, &formato(), "i", 1).unwrap();
        assert_eq!(p.url, "https://ntfy.ejemplo.com/avisos");
        let v: serde_json::Value = serde_json::from_slice(&p.cuerpo).unwrap();
        assert_eq!(v["topic"], "copias-x7");
        assert_eq!(v["click"], "https://copias.ejemplo.com/");
        assert!(p.cabeceras.contains(&("Authorization".into(), "Bearer tk_secreto".into())));
        let c = canal(TipoCanal::Telegram, ConfigCanal { chat_id: Some("-1001234".into()), ..Default::default() });
        let sec = BTreeMap::from([("token".to_string(), "123:ABC".to_string())]);
        let p = peticion(&c, &sec, &ms, &s, &formato(), "i", 1).unwrap();
        assert_eq!(p.url, "https://api.telegram.org/bot123:ABC/sendMessage");
        let v: serde_json::Value = serde_json::from_slice(&p.cuerpo).unwrap();
        assert_eq!(v["chat_id"], -1001234);
        assert_eq!(v["parse_mode"], "HTML");
        assert!(v["text"].as_str().unwrap().contains("<b>Prueba de notificaciones de Resguardo</b>"));
    }

    #[test]
    fn errores_sin_secretos() {
        struct Chivato;
        impl Transporte for Chivato {
            fn http(&self, p: &PeticionHttp) -> Result<(), Fallo> {
                Err(Fallo::temporal(format!("falló {}", p.url)))
            }
            fn correo(&self, _: &Correo) -> Result<(), Fallo> {
                Ok(())
            }
        }
        let c = canal(TipoCanal::Telegram, ConfigCanal { chat_id: Some("5".into()), ..Default::default() });
        let sec = BTreeMap::from([("token".to_string(), "123:TOKENSECRETO".to_string())]);
        let e = enviar(&Chivato, &c, &sec, "", &prueba(), &formato(), "i", 1).unwrap_err();
        assert!(!e.texto.contains("TOKENSECRETO"), "{}", e.texto);
        assert_eq!(http_estado(204, "x"), Ok(()));
        assert!(!http_estado(429, "x").unwrap_err().permanente);
        assert!(http_estado(401, "x").unwrap_err().permanente);
        assert!(!http_estado(503, "x").unwrap_err().permanente);
    }

    #[test]
    fn correo_multiparte() {
        let c = canal(
            TipoCanal::Correo,
            ConfigCanal {
                host: Some("smtp.ejemplo.com".into()),
                puerto: Some(587),
                seguridad: Some("starttls".into()),
                remitente: Some("Resguardo <avisos@ejemplo.com>".into()),
                ..Default::default()
            },
        );
        let s = contenido::componer(&prueba(), &formato());
        let m = correo(&c, &BTreeMap::new(), "ana@ejemplo.com", &s).unwrap();
        let crudo = String::from_utf8_lossy(&m.mensaje.formatted()).to_string();
        assert!(crudo.contains("multipart/alternative"), "{crudo}");
        assert!(crudo.contains("text/plain") && crudo.contains("text/html"));
        assert!(crudo.contains("Auto-Submitted: auto-generated"));
        assert!(!crudo.contains("multipart/related") && !crudo.contains("Content-ID"));
        assert!(correo(&c, &BTreeMap::new(), "no es un correo", &s).is_err());

        // Con el logo del cliente: el HTML y la imagen, juntos (multipart/related con su Content-ID).
        let mut f = formato();
        f.marcas.insert(
            "x".into(),
            contenido::MarcaCorreo { nombre: "Cliente".into(), acento: contenido::colores_acento("blue"), logo: Some(b"\x89PNG\r\n\x1a\nfalso".to_vec()) },
        );
        let s = contenido::componer(&prueba(), &f);
        let m = correo(&c, &BTreeMap::new(), "ana@ejemplo.com", &s).unwrap();
        let crudo = String::from_utf8_lossy(&m.mensaje.formatted()).to_string();
        assert!(crudo.contains("multipart/alternative") && crudo.contains("multipart/related"), "{crudo}");
        assert!(crudo.contains(&format!("Content-ID: <{}>", contenido::CID_LOGO)), "{crudo}");
        assert!(crudo.contains("image/png") && crudo.contains("Content-Disposition: inline"), "{crudo}");
        assert!(s.html.contains(&format!("cid:{}", contenido::CID_LOGO)));
    }
}
