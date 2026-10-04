//! Lo que dicen los mensajes: correo en HTML (adaptable al móvil, claro u
//! oscuro, con los colores de la consola) con su versión en texto, texto para
//! Telegram y ntfy, y el JSON del webhook.
//!
//! Solo metadatos: estado, nombres de clientes, equipos y copias, horas y
//! mensajes ya limpios ([`texto_publico`]: sin rutas, direcciones con
//! credenciales ni fichas largas). Nunca contraseñas ni claves.

use super::resumen::{Periodo, Resumen, ResumenCliente, ResumenEquipo};
use super::{Alerta, Mensaje, Severidad};
use crate::almacen::Ts;
use chrono::FixedOffset;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fmt::Write;

/// Cómo escribir: la dirección pública de la consola (para los enlaces), la zona
/// del servidor y la marca de los clientes de los mensajes (si la tienen).
#[derive(Clone, Debug)]
pub struct Formato {
    pub url_consola: Option<String>,
    pub zona: FixedOffset,
    /// Cliente → su marca (v1.32). Un correo de un solo cliente sale con la suya;
    /// uno que junta varios (un resumen de todos, un grupo de avisos), neutro.
    pub marcas: BTreeMap<String, MarcaCorreo>,
}

/// La marca de un cliente en el correo: su nombre, el acento (claro y oscuro,
/// los de la consola) y su logo (PNG, que va **dentro** del correo como
/// `cid:`: nunca una imagen de fuera).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MarcaCorreo {
    pub nombre: String,
    pub acento: Option<(&'static str, &'static str)>,
    pub logo: Option<Vec<u8>>,
}

/// El `Content-ID` del logo del cliente en el correo (multipart/related).
pub const CID_LOGO: &str = "logo-cliente@resguardo";

/// Los colores de un acento de la consola (docs/diseno.md §2): (claro, oscuro).
pub fn colores_acento(acento: &str) -> Option<(&'static str, &'static str)> {
    Some(match acento {
        "teal" => ("#0f766e", "#3cc4ad"),
        "blue" => ("#2563eb", "#6aa1ff"),
        "indigo" => ("#4f46e5", "#8e8cff"),
        "violet" => ("#7c3aed", "#b38bff"),
        "rose" => ("#d6336c", "#ff7aa6"),
        "amber" => ("#b45309", "#f2b33d"),
        "graphite" => ("#3f3f46", "#d4d4d8"),
        _ => return None,
    })
}

/// Un mensaje listo para cualquier canal.
#[derive(Clone, Debug, PartialEq)]
pub struct Salida {
    /// `aviso`, `recuperacion`, `grupo`, `resumen` o `prueba`.
    pub evento: &'static str,
    pub asunto: String,
    pub texto: String,
    pub html: String,
    pub severidad: Severidad,
    pub enlace: Option<String>,
    /// El logo del cliente (PNG) que el HTML enseña como `cid:` [`CID_LOGO`]: el
    /// correo lo lleva dentro (multipart/related). Los demás canales no lo usan.
    pub logo: Option<Vec<u8>>,
}

// ---------- Texto sin rutas ni secretos ----------

fn es_ruta(t: &str) -> bool {
    let b = t.as_bytes();
    t.contains('\\')
        || (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b.len() == 2 || b[2] == b'/'))
        || (t.starts_with('/') && t[1..].contains('/'))
        || t.starts_with("~/")
}

fn es_direccion(t: &str) -> bool {
    t.contains("://") || (t.contains('@') && t.contains(':'))
}

fn es_ficha(t: &str) -> bool {
    t.len() >= 32 && t.chars().all(|c| c.is_ascii_alphanumeric() || "+/=_-".contains(c))
}

/// Un mensaje de un equipo, listo para salir: sin rutas (`[ruta]`), sin direcciones
/// (`[dirección]`, pueden llevar usuario y contraseña) ni fichas largas (`[…]`), sin
/// caracteres de control y de 500 caracteres como mucho. El agente ya los manda sin
/// rutas (`public_message`); esto es la segunda barrera.
pub fn texto_publico(s: &str) -> String {
    let limpio: String = s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let mut out: Vec<String> = Vec::new();
    let mut ultimo: Option<&str> = None;
    for tok in limpio.split(' ').filter(|t| !t.is_empty()) {
        let nucleo = tok.trim_matches(|c: char| "«»\"'()[]{}<>,;:.".contains(c));
        // Lo que sigue a una ruta y tiene barras es el resto de la ruta (tenía espacios),
        // salvo que abra otra cosa: «(…», «"…».
        let sigue_ruta = ultimo == Some("[ruta]") && (nucleo.contains('/') || nucleo.contains('\\')) && !tok.starts_with(['(', '«', '"', '\'', '[']);
        let reemplazo = if es_ruta(nucleo) {
            Some("[ruta]")
        } else if es_direccion(nucleo) {
            Some("[dirección]")
        } else if sigue_ruta {
            Some("[ruta]")
        } else if es_ficha(nucleo) {
            Some("[…]")
        } else {
            None
        };
        match reemplazo {
            Some(r) if ultimo == Some(r) => {
                // Una ruta con espacios: un solo «[ruta]», con lo que la cerraba.
                let cola = &tok[tok.find(nucleo).map_or(tok.len(), |i| i + nucleo.len())..];
                if let Some(u) = out.last_mut() {
                    let base = u.trim_end_matches(|c: char| "«»\"'()[]{}<>,;:.".contains(c) && c != ']').to_string();
                    *u = format!("{base}{cola}");
                }
            }
            Some(r) => {
                out.push(if nucleo.is_empty() { r.to_string() } else { tok.replacen(nucleo, r, 1) });
                ultimo = Some(r);
            }
            None => {
                out.push(tok.to_string());
                ultimo = None;
            }
        }
    }
    let t = out.join(" ");
    if t.chars().count() > 500 {
        format!("{}…", t.chars().take(499).collect::<String>())
    } else {
        t
    }
}

// ---------- Formatos ----------

const MESES: [&str; 12] = ["ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic"];

fn en_zona(ts: Ts, z: FixedOffset) -> chrono::DateTime<FixedOffset> {
    chrono::DateTime::from_timestamp(ts, 0).unwrap_or_default().with_timezone(&z)
}

/// «4 oct, 10:32».
pub fn fecha_corta(ts: Ts, z: FixedOffset) -> String {
    use chrono::{Datelike, Timelike};
    let d = en_zona(ts, z);
    format!("{} {}, {:02}:{:02}", d.day(), MESES[d.month0() as usize], d.hour(), d.minute())
}

/// «4 oct 2026, 10:32».
pub fn fecha_larga(ts: Ts, z: FixedOffset) -> String {
    use chrono::{Datelike, Timelike};
    let d = en_zona(ts, z);
    format!("{} {} {}, {:02}:{:02}", d.day(), MESES[d.month0() as usize], d.year(), d.hour(), d.minute())
}

/// «4 oct».
fn dia(ts: Ts, z: FixedOffset) -> String {
    use chrono::Datelike;
    let d = en_zona(ts, z);
    format!("{} {}", d.day(), MESES[d.month0() as usize])
}

/// Bytes en unidades decimales, como la consola: «5,4 GB».
pub fn tamano(n: u64) -> String {
    const U: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let (mut v, mut i) = (n as f64, 0);
    while v >= 1000.0 && i < U.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    let t = if v < 10.0 && i > 0 { format!("{v:.1}") } else { format!("{v:.0}") };
    format!("{}\u{a0}{}", t.replace('.', ","), U[i])
}

fn plural(n: u64, uno: &str, varios: &str) -> String {
    if n == 1 {
        format!("1 {uno}")
    } else {
        format!("{n} {varios}")
    }
}

pub fn enlace(f: &Formato, ruta: &str) -> Option<String> {
    f.url_consola.as_ref().map(|u| format!("{}{ruta}", u.trim_end_matches('/')))
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            _ => o.push(c),
        }
    }
    o
}

// ---------- Colores (los de la consola, `ui/src/estilos.css`) ----------

#[derive(Clone, Copy)]
enum Tono {
    Critico,
    Importante,
    Informativo,
    Bien,
    Neutro,
}

impl Tono {
    fn de(s: Severidad) -> Self {
        match s {
            Severidad::Critico => Tono::Critico,
            Severidad::Importante => Tono::Importante,
            Severidad::Informativo => Tono::Informativo,
        }
    }
    fn clase(self) -> &'static str {
        match self {
            Tono::Critico => "critico",
            Tono::Importante => "importante",
            Tono::Informativo => "informativo",
            Tono::Bien => "bien",
            Tono::Neutro => "neutro",
        }
    }
    /// (texto, fondo) en claro.
    fn claro(self) -> (&'static str, &'static str) {
        match self {
            Tono::Critico => ("#c42121", "#fdecec"),
            Tono::Importante => ("#a04e09", "#fdf1e4"),
            Tono::Informativo => ("#2257d6", "#e9effc"),
            Tono::Bien => ("#126c35", "#e6f4ea"),
            Tono::Neutro => ("#52525b", "#f4f4f5"),
        }
    }
}

const TONOS: [Tono; 5] = [Tono::Critico, Tono::Importante, Tono::Informativo, Tono::Bien, Tono::Neutro];

fn estilos(marca: Option<&MarcaCorreo>) -> String {
    let mut claro = String::new();
    for t in TONOS {
        let (c, f) = t.claro();
        let _ = write!(claro, ".chip-{0}{{color:{c};background:{f}}}", t.clase());
    }
    // En oscuro: los de la consola en oscuro.
    let oscuro = ".chip-critico{color:#f87171!important;background:#3a1717!important}\
                  .chip-importante{color:#fbbf24!important;background:#3a2c0d!important}\
                  .chip-informativo{color:#60a5fa!important;background:#14233f!important}\
                  .chip-bien{color:#4ade80!important;background:#10301d!important}\
                  .chip-neutro{color:#a6a6b0!important;background:#232328!important}";
    format!(
        "body{{margin:0;padding:0;-webkit-text-size-adjust:100%}}{claro}\
         a{{color:#0f766e}}\
         @media (prefers-color-scheme:dark){{\
         body,.fondo{{background:#0f0f11!important}}.tarjeta{{background:#151518!important;border-color:#25252a!important}}\
         .t1{{color:#ededf0!important}}.t2{{color:#a6a6b0!important}}.t3{{color:#96969f!important}}\
         .linea{{border-color:#25252a!important}}.suave{{background:#1b1b1f!important}}\
         .boton{{background:#3cc4ad!important;color:#0b0b0d!important}}a{{color:#3cc4ad}}{oscuro}{acento}}}\
         @media (max-width:620px){{.marco{{width:100%!important}}.relleno{{padding:20px!important}}\
         }}",
        // El acento del cliente, en oscuro.
        acento = marca
            .and_then(|m| m.acento)
            .map(|(_, d)| format!(".acento{{background:{d}!important}}.borde-acento{{border-top-color:{d}!important}}"))
            .unwrap_or_default(),
    )
}

const FUENTE: &str = "'Inter','Segoe UI',system-ui,-apple-system,Roboto,'Helvetica Neue',Arial,sans-serif";

fn chip(t: Tono, texto: &str) -> String {
    let (c, f) = t.claro();
    format!(
        "<span class=\"chip-{}\" style=\"display:inline-block;padding:2px 10px;border-radius:999px;font-size:12px;line-height:18px;font-weight:600;color:{c};background:{f}\">{}</span>",
        t.clase(),
        esc(texto)
    )
}

fn boton(url: &str, texto: &str) -> String {
    format!(
        "<table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" style=\"margin:24px 0 4px\"><tr><td class=\"boton\" style=\"border-radius:8px;background:#0f766e\">\
         <a class=\"boton\" href=\"{}\" style=\"display:inline-block;padding:10px 18px;border-radius:8px;background:#0f766e;color:#ffffff;font-weight:600;font-size:14px;text-decoration:none\">{}</a>\
         </td></tr></table>",
        esc(url),
        esc(texto)
    )
}

/// El tamaño del logo en la cabecera: 32 px de alto (y como mucho 180 de ancho),
/// con su proporción (de la cabecera IHDR del PNG). Con `width` y `height` en el
/// HTML: algunos clientes de correo no hacen caso del CSS.
fn medidas_logo(png: &[u8]) -> (u32, u32) {
    let (ancho, alto) = match png.get(16..24) {
        Some(b) => (u32::from_be_bytes([b[0], b[1], b[2], b[3]]).max(1), u32::from_be_bytes([b[4], b[5], b[6], b[7]]).max(1)),
        None => (1, 1),
    };
    let w = (ancho as u64 * 32 / alto as u64).clamp(1, 180) as u32;
    let h = if w == 180 { (alto as u64 * 180 / ancho as u64).clamp(1, 32) as u32 } else { 32 };
    (w, h)
}

/// La cabecera: «Resguardo Server», o la marca del cliente (su logo, o un
/// cuadro con su acento, y su nombre) con «Resguardo» al lado.
fn cabecera(marca: Option<&MarcaCorreo>) -> String {
    let Some(m) = marca else {
        return "<tr><td style=\"padding:0 4px 14px;font-size:14px;line-height:20px\" class=\"t1\">\
                <span style=\"display:inline-block;width:10px;height:10px;border-radius:3px;background:#0f766e;vertical-align:middle\"></span>\
                <strong class=\"t1\" style=\"color:#18181b;vertical-align:middle\">&nbsp;Resguardo</strong> <span class=\"t3\" style=\"color:#666670;vertical-align:middle\">Server</span></td></tr>"
            .to_string();
    };
    let simbolo = match &m.logo {
        Some(png) => {
            let (w, h) = medidas_logo(png);
            format!(
                "<img src=\"cid:{CID_LOGO}\" width=\"{w}\" height=\"{h}\" alt=\"{}\" style=\"display:inline-block;width:{w}px;height:{h}px;border:0;outline:none;vertical-align:middle\">",
                esc(&m.nombre)
            )
        }
        None => format!(
            "<span class=\"acento\" style=\"display:inline-block;width:12px;height:12px;border-radius:3px;background:{};vertical-align:middle\"></span>",
            m.acento.map_or("#0f766e", |(c, _)| c)
        ),
    };
    format!(
        "<tr><td style=\"padding:0 4px 14px;font-size:14px;line-height:20px\" class=\"t1\">{simbolo}\
         <strong class=\"t1\" style=\"color:#18181b;vertical-align:middle\">&nbsp;{}</strong> <span class=\"t3\" style=\"color:#666670;vertical-align:middle\">· Resguardo</span></td></tr>",
        esc(&m.nombre)
    )
}

/// La página entera: cabecera, tarjeta con el contenido y pie.
fn pagina(f: &Formato, marca: Option<&MarcaCorreo>, asunto: &str, previo: &str, cuerpo: &str) -> String {
    let ajustes =
        enlace(f, "/ajustes").map(|u| format!(" <a href=\"{}\" style=\"color:#666670\" class=\"t3\">Elige qué te llega</a>.", esc(&u))).unwrap_or_default();
    format!(
        "<!doctype html>\n<html lang=\"es\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <meta name=\"color-scheme\" content=\"light dark\"><meta name=\"supported-color-schemes\" content=\"light dark\">\
         <title>{asunto}</title><style>{estilos}</style></head>\n\
         <body class=\"fondo\" style=\"margin:0;padding:0;background:#f7f7f8\">\
         <div style=\"display:none;max-height:0;overflow:hidden;opacity:0\">{previo}</div>\
         <table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" class=\"fondo\" style=\"background:#f7f7f8\"><tr><td align=\"center\" style=\"padding:24px 12px\">\
         <table role=\"presentation\" class=\"marco\" width=\"600\" cellpadding=\"0\" cellspacing=\"0\" style=\"width:600px;max-width:600px;font-family:{FUENTE}\">\
         {cabecera}\
         <tr><td class=\"tarjeta relleno{clase_borde}\" style=\"background:#ffffff;border:1px solid #e9e9ec;{borde}border-radius:12px;padding:28px\">\n{cuerpo}\n</td></tr>\
         <tr><td class=\"t3\" style=\"padding:16px 4px;color:#666670;font-size:12px;line-height:18px\">\
         Resguardo Server. Este mensaje solo cuenta el estado de las copias: nunca lleva contraseñas, claves ni nombres de archivos.{ajustes}</td></tr>\
         </table></td></tr></table></body></html>\n",
        asunto = esc(asunto),
        estilos = estilos(marca),
        previo = esc(previo),
        cabecera = cabecera(marca),
        // Con el acento del cliente: una línea de su color arriba de la tarjeta.
        clase_borde = if marca.and_then(|m| m.acento).is_some() { " borde-acento" } else { "" },
        borde = marca.and_then(|m| m.acento).map(|(c, _)| format!("border-top:3px solid {c};")).unwrap_or_default(),
    )
}

fn h1(t: &str) -> String {
    format!("<h1 class=\"t1\" style=\"margin:14px 0 8px;font-size:22px;line-height:28px;font-weight:650;color:#18181b\">{}</h1>", esc(t))
}

fn p(t: &str) -> String {
    format!("<p class=\"t2\" style=\"margin:0 0 12px;font-size:14px;line-height:20px;color:#52525b\">{}</p>", esc(t))
}

fn filas(datos: &[(&str, String)]) -> String {
    let mut s = String::from(
        "<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" style=\"margin:8px 0 0;font-size:14px;line-height:20px\">",
    );
    for (k, v) in datos {
        let _ = write!(
            s,
            "<tr><td class=\"t3 linea\" style=\"padding:8px 12px 8px 0;border-top:1px solid #e9e9ec;color:#666670;width:110px;vertical-align:top\">{}</td>\
             <td class=\"t1 linea\" style=\"padding:8px 0;border-top:1px solid #e9e9ec;color:#18181b\">{}</td></tr>",
            esc(k),
            esc(v)
        );
    }
    s.push_str("</table>");
    s
}

// ---------- Avisos ----------

fn sitio(a: &Alerta) -> String {
    match &a.equipo {
        Some(e) => format!("{} · {e}", a.cliente),
        None => a.cliente.clone(),
    }
}

fn veces(a: &Alerta, z: FixedOffset) -> Option<String> {
    (a.veces > 1).then(|| format!("{} desde el {}", plural(a.veces as u64, "vez", "veces"), fecha_corta(a.desde, z)))
}

fn alerta(m: &Mensaje) -> Option<(&Alerta, bool)> {
    match m {
        Mensaje::Aviso(a) => Some((a, false)),
        Mensaje::Recuperacion(a) => Some((a, true)),
        _ => None,
    }
}

fn una_alerta(a: &Alerta, resuelto: bool, f: &Formato, m: Option<&MarcaCorreo>) -> Salida {
    let enlace = enlace(f, &a.ruta);
    let asunto = if resuelto { format!("{} · {}", a.titulo, a.cliente) } else { format!("{}: {} · {}", a.severidad.texto(), a.titulo, a.cliente) };
    let mut datos = vec![("Cliente", a.cliente.clone())];
    if let Some(e) = &a.equipo {
        datos.push(("Equipo", e.clone()));
    }
    datos.push(("Cuándo", fecha_larga(a.hora, f.zona)));
    if let (Some(v), false) = (veces(a, f.zona), resuelto) {
        datos.push(("Veces", v));
    }
    let mut texto = format!("{}\n\n", a.titulo);
    if !a.texto.is_empty() {
        let _ = write!(texto, "{}\n\n", a.texto);
    }
    for (k, v) in &datos {
        let _ = writeln!(texto, "{k}: {v}");
    }
    if !resuelto {
        let _ = writeln!(texto, "Gravedad: {}", a.severidad.texto());
    }
    if let Some(u) = &enlace {
        let _ = write!(texto, "\nVer en la consola: {u}\n");
    }
    texto.push_str(PIE_TEXTO);
    let (tono, etiqueta) = if resuelto { (Tono::Bien, "Resuelto".to_string()) } else { (Tono::de(a.severidad), a.severidad.texto().to_string()) };
    let mut cuerpo = chip(tono, &etiqueta);
    cuerpo.push_str(&h1(&a.titulo));
    if !a.texto.is_empty() {
        cuerpo.push_str(&p(&a.texto));
    }
    cuerpo.push_str(&filas(&datos));
    if let Some(u) = &enlace {
        cuerpo.push_str(&boton(u, "Ver en la consola"));
    }
    let html = pagina(f, m, &asunto, &format!("{} · {}", a.texto, sitio(a)), &cuerpo);
    Salida {
        evento: if resuelto { "recuperacion" } else { "aviso" },
        asunto,
        texto,
        html,
        severidad: if resuelto { Severidad::Informativo } else { a.severidad },
        enlace,
        logo: logo_de(m),
    }
}

const PIE_TEXTO: &str = "\n—\nResguardo Server. Este mensaje solo cuenta el estado de las copias: nunca lleva contraseñas, claves ni nombres de archivos.\n";

fn grupo(alertas: &[(&Alerta, bool)], f: &Formato, m: Option<&MarcaCorreo>) -> Salida {
    let n = alertas.len();
    let sev = alertas.iter().filter(|(_, r)| !r).map(|(a, _)| a.severidad).max().unwrap_or(Severidad::Informativo);
    let criticos = alertas.iter().filter(|(a, r)| !r && a.severidad == Severidad::Critico).count();
    let resueltos = alertas.iter().filter(|(_, r)| *r).count();
    let mut asunto = format!("{n} avisos de Resguardo");
    if criticos > 0 {
        let _ = write!(asunto, " ({})", plural(criticos as u64, "crítico", "críticos"));
    }
    let un_cliente = alertas.iter().all(|(a, _)| a.cliente_id == alertas[0].0.cliente_id);
    let enlace = if un_cliente { enlace(f, &format!("/c/{}/avisos", alertas[0].0.cliente_id)) } else { enlace(f, "/") };
    let explicacion = "Se juntaron en un solo mensaje para no llenarte la bandeja (tope de avisos por hora u horas de silencio).";
    let mut texto = format!("{asunto}\n\n{explicacion}\n\n");
    let mut lista = String::new();
    for (a, resuelto) in alertas.iter().take(50) {
        let etiqueta = if *resuelto { "Resuelto" } else { a.severidad.texto() };
        let _ = writeln!(texto, "- [{etiqueta}] {} ({}, {})", a.titulo, sitio(a), fecha_corta(a.hora, f.zona));
        if !a.texto.is_empty() {
            let _ = writeln!(texto, "  {}", a.texto);
        }
        let tono = if *resuelto { Tono::Bien } else { Tono::de(a.severidad) };
        let _ = write!(
            lista,
            "<tr><td class=\"linea\" style=\"padding:12px 0;border-top:1px solid #e9e9ec\">{}\
             <div class=\"t1\" style=\"margin:6px 0 2px;font-size:14px;line-height:20px;font-weight:600;color:#18181b\">{}</div>\
             <div class=\"t3\" style=\"font-size:12px;line-height:18px;color:#666670\">{} · {}</div>{}</td></tr>",
            chip(tono, etiqueta),
            esc(&a.titulo),
            esc(&sitio(a)),
            esc(&fecha_corta(a.hora, f.zona)),
            if a.texto.is_empty() {
                String::new()
            } else {
                format!("<div class=\"t2\" style=\"margin-top:4px;font-size:13px;line-height:18px;color:#52525b\">{}</div>", esc(&a.texto))
            }
        );
    }
    if n > 50 {
        let _ = writeln!(texto, "… y {} más en la consola.", n - 50);
        let _ = write!(lista, "<tr><td class=\"t3\" style=\"padding:12px 0;color:#666670;font-size:13px\">… y {} más en la consola.</td></tr>", n - 50);
    }
    if let Some(u) = &enlace {
        let _ = write!(texto, "\nVer en la consola: {u}\n");
    }
    texto.push_str(PIE_TEXTO);
    let etiqueta = if resueltos == n { "Resueltos".to_string() } else { plural(n as u64, "aviso", "avisos") };
    let tono = if resueltos == n { Tono::Bien } else { Tono::de(sev) };
    let mut cuerpo = chip(tono, &etiqueta);
    cuerpo.push_str(&h1(&asunto));
    cuerpo.push_str(&p(explicacion));
    let _ = write!(cuerpo, "<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\">{lista}</table>");
    if let Some(u) = &enlace {
        cuerpo.push_str(&boton(u, "Ver los avisos en la consola"));
    }
    let html = pagina(f, m, &asunto, explicacion, &cuerpo);
    Salida { evento: "grupo", asunto, texto, html, severidad: sev, enlace, logo: logo_de(m) }
}

// ---------- Resúmenes ----------

/// El asunto con las fechas en la zona del servidor (para el registro).
pub fn asunto_resumen(r: &Resumen) -> String {
    asunto_resumen_en(r, super::zona(r.hasta))
}

fn nombre_periodo(r: &Resumen) -> &'static str {
    match r.periodo {
        Periodo::Diario => "Resumen diario de copias",
        Periodo::Semanal => "Resumen semanal de copias",
    }
}

fn asunto_resumen_en(r: &Resumen, z: FixedOffset) -> String {
    let con_fallos = r.clientes.iter().flat_map(|c| &c.equipos).filter(|e| matches!(e.estado.as_str(), "fallo" | "sin_contacto")).count();
    let mut a = match r.periodo {
        Periodo::Diario => format!("{} · {}", nombre_periodo(r), dia(r.hasta, z)),
        Periodo::Semanal => format!("{} · {} – {}", nombre_periodo(r), dia(r.desde, z), dia(r.hasta, z)),
    };
    if con_fallos > 0 {
        let _ = write!(a, " · {} con problemas", plural(con_fallos as u64, "equipo", "equipos"));
    } else {
        a.push_str(" · todo bien");
    }
    a
}

fn estado_equipo(e: &ResumenEquipo) -> (Tono, &'static str) {
    match e.estado.as_str() {
        "fallo" => (Tono::Critico, "Con fallos"),
        "sin_contacto" => (Tono::Importante, "Sin contacto"),
        "aviso" => (Tono::Importante, "Revisar"),
        "sin_datos" => (Tono::Neutro, "Sin datos"),
        _ => (Tono::Bien, "Bien"),
    }
}

fn linea_equipo(e: &ResumenEquipo, z: FixedOffset) -> String {
    let mut partes = vec![match e.ultima_ok {
        Some(t) => format!("Última copia correcta: {}", fecha_corta(t, z)),
        None => "Sin copias correctas".into(),
    }];
    if e.fallos > 0 {
        partes.push(plural(e.fallos as u64, "fallo", "fallos"));
    }
    if e.copias_ok > 0 {
        partes.push(plural(e.copias_ok as u64, "copia correcta", "copias correctas"));
    }
    if let Some(b) = e.bytes {
        partes.push(tamano(b));
    }
    if e.estado == "sin_contacto" {
        partes.push(match e.ultimo_contacto {
            Some(t) => format!("conectó por última vez el {}", fecha_corta(t, z)),
            None => "nunca ha conectado".into(),
        });
    }
    partes.join(" · ")
}

fn cifra(valor: &str, etiqueta: &str) -> String {
    format!(
        "<td class=\"cifra suave\" style=\"padding:12px;background:#f4f4f5;border-radius:8px;width:50%;vertical-align:top\">\
         <div class=\"t1\" style=\"font-size:20px;line-height:26px;font-weight:650;color:#18181b\">{}</div>\
         <div class=\"t3\" style=\"font-size:12px;line-height:16px;color:#666670\">{}</div></td>",
        esc(valor),
        esc(etiqueta)
    )
}

fn resumen(r: &Resumen, f: &Formato, m: Option<&MarcaCorreo>) -> Salida {
    let z = f.zona;
    let asunto = asunto_resumen_en(r, z);
    let equipos: usize = r.clientes.iter().map(|c| c.equipos.len()).sum();
    let ok: u32 = r.clientes.iter().map(|c| c.copias_ok).sum();
    let fallos: u32 = r.clientes.iter().map(|c| c.fallos).sum();
    let bytes: u64 = r.clientes.iter().map(|c| c.bytes).sum();
    let periodo = match r.periodo {
        Periodo::Diario => format!("Últimas 24 horas, hasta el {}.", fecha_larga(r.hasta, z)),
        Periodo::Semanal => format!("Del {} al {}.", dia(r.desde, z), fecha_larga(r.hasta, z)),
    };
    let enlace = enlace(f, "/");
    let mut texto = format!("{}\n{periodo}\n\n", nombre_periodo(r));
    let _ = writeln!(
        texto,
        "{} · {} · {} · {} en uso\n",
        plural(equipos as u64, "equipo", "equipos"),
        plural(ok as u64, "copia correcta", "copias correctas"),
        plural(fallos as u64, "fallo", "fallos"),
        tamano(bytes)
    );
    let mut cuerpo = chip(if fallos > 0 { Tono::Critico } else { Tono::Bien }, if fallos > 0 { "Hay fallos" } else { "Todo bien" });
    cuerpo.push_str(&h1(nombre_periodo(r)));
    cuerpo.push_str(&p(&periodo));
    let _ = write!(
        cuerpo,
        "<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"6\" style=\"margin:4px -6px 8px\"><tr>{}{}</tr><tr>{}{}</tr></table>",
        cifra(&equipos.to_string(), "equipos"),
        cifra(&ok.to_string(), "copias correctas"),
        cifra(&fallos.to_string(), "fallos"),
        cifra(&tamano(bytes), "en uso"),
    );
    for c in &r.clientes {
        cuerpo.push_str(&bloque_cliente(c, f));
        let _ = writeln!(texto, "== {} ==", c.nombre);
        for e in &c.equipos {
            let (_, estado) = estado_equipo(e);
            let _ = writeln!(texto, "- {} [{estado}]: {}", e.nombre, linea_equipo(e, z));
        }
        if !c.atencion.is_empty() {
            texto.push_str("Necesita atención:\n");
            for a in &c.atencion {
                let _ = writeln!(texto, "  · {a}");
            }
        }
        if let Some(u) = super::contenido::enlace(f, &format!("/c/{}", c.id)) {
            let _ = writeln!(texto, "Ver en la consola: {u}");
        }
        texto.push('\n');
    }
    if let Some(u) = &enlace {
        cuerpo.push_str(&boton(u, "Abrir la consola"));
    }
    texto.push_str(PIE_TEXTO.trim_start_matches('\n'));
    let previo = format!("{} · {} · {}", plural(equipos as u64, "equipo", "equipos"), plural(fallos as u64, "fallo", "fallos"), tamano(bytes));
    let html = pagina(f, m, &asunto, &previo, &cuerpo);
    Salida { evento: "resumen", asunto, texto, html, severidad: Severidad::Informativo, enlace, logo: logo_de(m) }
}

fn bloque_cliente(c: &ResumenCliente, f: &Formato) -> String {
    let mut s = format!(
        "<h2 class=\"t1\" style=\"margin:24px 0 4px;font-size:16px;line-height:22px;font-weight:650;color:#18181b\">{}</h2>\
         <div class=\"t3\" style=\"font-size:12px;line-height:18px;color:#666670\">{} · {} · {}</div>",
        esc(&c.nombre),
        plural(c.equipos.len() as u64, "equipo", "equipos"),
        plural(c.fallos as u64, "fallo", "fallos"),
        tamano(c.bytes)
    );
    s.push_str("<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" style=\"margin-top:8px\">");
    for e in &c.equipos {
        let (tono, estado) = estado_equipo(e);
        let _ = write!(
            s,
            "<tr><td class=\"linea\" style=\"padding:10px 8px 10px 0;border-top:1px solid #e9e9ec;vertical-align:top\">\
             <div class=\"t1\" style=\"font-size:14px;line-height:20px;font-weight:600;color:#18181b\">{}</div>\
             <div class=\"t3\" style=\"font-size:12px;line-height:18px;color:#666670\">{}</div></td>\
             <td class=\"linea\" align=\"right\" style=\"padding:10px 0;border-top:1px solid #e9e9ec;vertical-align:top;white-space:nowrap\">{}</td></tr>",
            esc(&e.nombre),
            esc(&linea_equipo(e, f.zona)),
            chip(tono, estado)
        );
    }
    s.push_str("</table>");
    if !c.atencion.is_empty() {
        s.push_str(
            "<div class=\"suave\" style=\"margin-top:10px;padding:12px 14px;background:#f4f4f5;border-radius:8px\">\
             <div class=\"t1\" style=\"font-size:13px;line-height:18px;font-weight:600;color:#18181b;margin-bottom:4px\">Necesita atención</div>",
        );
        for a in &c.atencion {
            let _ = write!(s, "<div class=\"t2\" style=\"font-size:13px;line-height:20px;color:#52525b\">• {}</div>", esc(a));
        }
        s.push_str("</div>");
    }
    if let Some(u) = enlace(f, &format!("/c/{}", c.id)) {
        let _ = write!(
            s,
            "<div style=\"margin-top:8px;font-size:13px\"><a href=\"{}\" style=\"color:#0f766e\">Ver {} en la consola</a></div>",
            esc(&u),
            esc(&c.nombre)
        );
    }
    s
}

fn prueba(quien: &str, f: &Formato, m: Option<&MarcaCorreo>) -> Salida {
    let asunto = "Prueba de notificaciones de Resguardo".to_string();
    let explicacion = format!("Es un mensaje de prueba de Resguardo Server que pidió {quien}. Si lo ves, este canal está listo para los avisos.");
    let enlace = enlace(f, "/");
    let mut texto = format!("Las notificaciones funcionan\n\n{explicacion}\n");
    if let Some(u) = &enlace {
        let _ = write!(texto, "\nConsola: {u}\n");
    }
    texto.push_str(PIE_TEXTO);
    let mut cuerpo = chip(Tono::Bien, "Prueba");
    cuerpo.push_str(&h1("Las notificaciones funcionan"));
    cuerpo.push_str(&p(&explicacion));
    if let Some(u) = &enlace {
        cuerpo.push_str(&boton(u, "Abrir la consola"));
    }
    let html = pagina(f, m, &asunto, &explicacion, &cuerpo);
    Salida { evento: "prueba", asunto, texto, html, severidad: Severidad::Informativo, enlace, logo: logo_de(m) }
}

fn logo_de(m: Option<&MarcaCorreo>) -> Option<Vec<u8>> {
    m.and_then(|m| m.logo.clone())
}

/// Los clientes de los que hablan los mensajes (id y nombre), sin repetir.
pub fn clientes_de(ms: &[Mensaje]) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    let mut poner = |id: &str, nombre: &str| {
        if !v.iter().any(|(x, _)| x == id) {
            v.push((id.to_string(), nombre.to_string()));
        }
    };
    for m in ms {
        match m {
            Mensaje::Aviso(a) | Mensaje::Recuperacion(a) => poner(&a.cliente_id, &a.cliente),
            Mensaje::Resumen(r) => r.clientes.iter().for_each(|c| poner(&c.id, &c.nombre)),
            Mensaje::Prueba { .. } => {}
        }
    }
    v
}

/// La marca con la que sale: la del cliente si todo es de uno solo (y la tiene);
/// lo que junta varios clientes, neutro. La prueba de un canal de un cliente
/// (sin cliente en el mensaje) sale con la suya si es la única que se dio.
fn marca_de<'a>(ms: &[Mensaje], f: &'a Formato) -> Option<&'a MarcaCorreo> {
    match clientes_de(ms).as_slice() {
        [] => (f.marcas.len() == 1).then(|| f.marcas.values().next()).flatten(),
        [(id, _)] => f.marcas.get(id),
        _ => None,
    }
}

/// Uno o varios mensajes (agrupados) en un solo mensaje para cualquier canal.
pub fn componer(ms: &[Mensaje], f: &Formato) -> Salida {
    let marca = marca_de(ms, f);
    match ms {
        [Mensaje::Resumen(r)] => resumen(r, f, marca),
        [Mensaje::Prueba { quien }] => prueba(quien, f, marca),
        [m] => {
            let (a, resuelto) = alerta(m).expect("aviso o recuperación");
            una_alerta(a, resuelto, f, marca)
        }
        _ => {
            let alertas: Vec<(&Alerta, bool)> = ms.iter().filter_map(alerta).collect();
            if alertas.len() == 1 {
                una_alerta(alertas[0].0, alertas[0].1, f, marca)
            } else {
                grupo(&alertas, f, marca)
            }
        }
    }
}

// ---------- Otros canales ----------

/// Texto para Telegram (HTML de Telegram: solo `<b>`, `<a>`…), de 4000 caracteres como mucho.
pub fn telegram(s: &Salida) -> String {
    let marca = match (s.evento, s.severidad) {
        ("recuperacion" | "prueba", _) => "✅",
        ("resumen", _) => "📋",
        (_, Severidad::Critico) => "🔴",
        (_, Severidad::Importante) => "🟠",
        _ => "🔵",
    };
    // El texto ya lleva el título en su primera línea (salvo el pie, que aquí sobra).
    let cuerpo = s.texto.split("\n—\n").next().unwrap_or(&s.texto).trim();
    let cuerpo: String = cuerpo.lines().skip(1).collect::<Vec<_>>().join("\n");
    let cuerpo: String = cuerpo.trim().chars().take(3500).collect();
    let mut t = format!("{marca} <b>{}</b>\n{}", esc(&s.asunto), esc(&cuerpo));
    if let Some(u) = &s.enlace {
        let _ = write!(t, "\n\n<a href=\"{}\">Ver en la consola</a>", esc(u));
    }
    t
}

/// El cuerpo del webhook (v1).
pub fn webhook(ms: &[Mensaje], s: &Salida, f: &Formato, id: &str, ahora: Ts) -> Value {
    let fecha = |t: Ts| chrono::DateTime::from_timestamp(t, 0).map(|d| d.with_timezone(&f.zona).to_rfc3339()).unwrap_or_default();
    let avisos: Vec<Value> = ms
        .iter()
        .filter_map(alerta)
        .map(|(a, resuelto)| {
            json!({
                "tipo": a.tipo, "severidad": a.severidad.clave(), "resuelto": resuelto, "titulo": a.titulo, "texto": a.texto,
                "cliente": { "id": a.cliente_id, "nombre": a.cliente },
                "equipo": a.equipo_id.as_ref().map(|id| json!({ "id": id, "nombre": a.equipo })),
                "veces": a.veces, "desde": fecha(a.desde), "hora": fecha(a.hora), "enlace": enlace(f, &a.ruta),
            })
        })
        .collect();
    let resumen = ms.iter().find_map(|m| match m {
        Mensaje::Resumen(r) => Some(json!({
            "periodo": r.periodo, "desde": fecha(r.desde), "hasta": fecha(r.hasta),
            "clientes": r.clientes.iter().map(|c| json!({
                "id": c.id, "nombre": c.nombre, "copias_ok": c.copias_ok, "fallos": c.fallos, "bytes": c.bytes, "atencion": c.atencion,
                "equipos": c.equipos.iter().map(|e| json!({
                    "id": e.id, "nombre": e.nombre, "estado": e.estado, "ultima_ok": e.ultima_ok.map(fecha),
                    "ultimo_contacto": e.ultimo_contacto.map(fecha), "copias_ok": e.copias_ok, "fallos": e.fallos, "bytes": e.bytes,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })),
        _ => None,
    });
    json!({
        "version": 1, "id": id, "evento": s.evento, "creado": fecha(ahora), "severidad": s.severidad.clave(),
        "titulo": s.asunto, "texto": s.texto, "enlace": s.enlace, "avisos": avisos, "resumen": resumen,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_rutas_ni_secretos() {
        let t = texto_publico(
            "No se pudo leer C:\\Users\\Ana Pérez\\Documentos\\claves.txt: acceso denegado (rest:https://copias:Contraseña1@10.0.0.2:8000/ana) \
             ni «/home/ana/Mis documentos/x.odt»; token ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789ab y \\\\nas\\copias\\b\u{7}",
        );
        for prohibido in ["Users", "Pérez", "claves.txt", "Contraseña1", "10.0.0.2", "home", "Mis", "ghp_", "nas", "\u{7}"] {
            assert!(!t.contains(prohibido), "«{prohibido}» en: {t}");
        }
        assert!(t.starts_with("No se pudo leer [ruta]: acceso denegado ([dirección])"), "{t}");
        assert!(t.contains("«[ruta]»"), "{t}");
        assert_eq!(texto_publico("No se pudo leer C:\\Users\\Ana\\x.txt (rest:https://u:p@h/r)"), "No se pudo leer [ruta] ([dirección])");
        // Lo normal se queda como está.
        assert_eq!(texto_publico("La copia terminó con 3 avisos (12 archivos nuevos)."), "La copia terminó con 3 avisos (12 archivos nuevos).");
        assert_eq!(texto_publico("versión 0.7.11, 1/2 hecho"), "versión 0.7.11, 1/2 hecho");
        assert_eq!(texto_publico(&"copia ".repeat(200)).chars().count(), 500);
    }

    #[test]
    fn formatos() {
        assert_eq!(tamano(5_368_709_120), "5,4\u{a0}GB");
        assert_eq!(tamano(999), "999\u{a0}B");
        assert_eq!(tamano(12_500_000), "12\u{a0}MB");
        let z = FixedOffset::east_opt(2 * 3600).unwrap();
        // 2026-10-04 08:32 UTC → 10:32 en +02:00.
        let t = chrono::DateTime::parse_from_rfc3339("2026-10-04T08:32:00Z").unwrap().timestamp();
        assert_eq!(fecha_larga(t, z), "4 oct 2026, 10:32");
        assert_eq!(fecha_corta(t, z), "4 oct, 10:32");
    }

    // ---------- Muestras (lo que se manda, tal cual; `muestras/`) ----------

    const HASTA: Ts = 1_791_100_800;

    fn formato() -> Formato {
        Formato { url_consola: Some("https://copias.ejemplo.com:8443".into()), zona: FixedOffset::east_opt(2 * 3600).unwrap(), marcas: BTreeMap::new() }
    }

    /// Un PNG de 64 × 16 (solo la firma y la cabecera: lo que mira el correo).
    fn png_de_prueba() -> Vec<u8> {
        let mut b = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
        b.extend_from_slice(b"IHDR");
        b.extend_from_slice(&64u32.to_be_bytes());
        b.extend_from_slice(&16u32.to_be_bytes());
        b.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        b
    }

    fn con_marca(logo: bool) -> Formato {
        let mut f = formato();
        f.marcas.insert("cl-1".into(), MarcaCorreo { nombre: "Altamar & Asociados".into(), acento: colores_acento("violet"), logo: logo.then(png_de_prueba) });
        f
    }

    #[test]
    fn correo_con_la_marca_del_cliente() {
        // Un aviso de un cliente con marca: su logo (dentro del correo, cid:) y su acento en la cabecera.
        let s = componer(&[Mensaje::Aviso(alerta_de_prueba())], &con_marca(true));
        sin_secretos(&s);
        assert_eq!(s.logo.as_deref(), Some(png_de_prueba().as_slice()));
        assert!(s.html.contains(&format!("src=\"cid:{CID_LOGO}\" width=\"128\" height=\"32\"")), "{}", s.html);
        assert!(s.html.contains("alt=\"Altamar &amp; Asociados\""));
        assert!(s.html.contains("border-top:3px solid #7c3aed;") && s.html.contains(".borde-acento{border-top-color:#b38bff!important}"));
        // Ninguna imagen de fuera.
        assert!(!s.html.contains("src=\"http"), "{}", s.html);
        muestra("aviso-marca.html", &s.html);
        // Lo demás, igual que sin marca (el texto no cambia).
        let neutro = componer(&[Mensaje::Aviso(alerta_de_prueba())], &formato());
        assert_eq!((s.asunto.as_str(), s.texto.as_str()), (neutro.asunto.as_str(), neutro.texto.as_str()));
        assert!(neutro.logo.is_none() && !neutro.html.contains("cid:"));

        // Solo el acento: un cuadro de su color (también en oscuro).
        let s = componer(&[Mensaje::Aviso(alerta_de_prueba())], &con_marca(false));
        assert!(s.logo.is_none() && !s.html.contains("cid:"));
        assert!(s.html.contains("background:#7c3aed;vertical-align:middle") && s.html.contains(".acento{background:#b38bff!important}"));

        // Varios clientes en un mismo correo: neutro.
        let mut otro = alerta_de_prueba();
        otro.cliente_id = "cl-2".into();
        otro.cliente = "Otro cliente".into();
        let s = componer(&[Mensaje::Aviso(alerta_de_prueba()), Mensaje::Aviso(otro)], &con_marca(true));
        assert!(s.logo.is_none() && !s.html.contains("cid:") && !s.html.contains("#7c3aed"));
        assert!(s.html.contains("&nbsp;Resguardo</strong>"));
        // Un resumen de varios clientes, también.
        let mut r = Resumen {
            periodo: Periodo::Semanal,
            desde: HASTA - 7 * 86_400,
            hasta: HASTA,
            clientes: vec![crate::notificaciones::resumen::tests::cliente_de_prueba(HASTA)],
        };
        let mut c2 = r.clientes[0].clone();
        c2.id = "cl-2".into();
        r.clientes.push(c2);
        let s = componer(&[Mensaje::Resumen(r.clone())], &con_marca(true));
        assert!(s.logo.is_none() && !s.html.contains("#7c3aed"));
        // El de un solo cliente (el suyo), con su marca.
        r.clientes.truncate(1);
        let mut f = con_marca(true);
        let marca = f.marcas.remove("cl-1").unwrap();
        f.marcas.insert(r.clientes[0].id.clone(), marca);
        assert!(componer(&[Mensaje::Resumen(r)], &f).logo.is_some());
    }

    fn alerta_de_prueba() -> Alerta {
        Alerta {
            tipo: "copia_fallida".into(),
            severidad: Severidad::Critico,
            titulo: "Falló la copia «Documentos» en «PC-Contabilidad»".into(),
            // Como llega de un equipo, pasado por `texto_publico`.
            texto: texto_publico("No se pudo leer C:\\Users\\Ana\\Documentos\\claves.txt (rest:https://copias:ClaveDelRepo@10.0.0.2:8000/ana)"),
            cliente_id: "cl-1".into(),
            cliente: "Altamar & Asociados".into(),
            equipo_id: Some("e1".into()),
            equipo: Some("PC-Contabilidad".into()),
            veces: 3,
            desde: HASTA - 2 * 86_400,
            hora: HASTA - 600,
            ruta: "/c/cl-1/equipos/e1".into(),
        }
    }

    /// Compara con `muestras/<nombre>`. Para rehacerlas: `RESGUARDO_ACTUALIZAR_MUESTRAS=1 cargo test -p resguardo-servidor muestras`.
    fn muestra(nombre: &str, contenido: &str) {
        let ruta = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/notificaciones/muestras").join(nombre);
        if std::env::var_os("RESGUARDO_ACTUALIZAR_MUESTRAS").is_some() {
            std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
            std::fs::write(&ruta, contenido).unwrap();
            return;
        }
        let esperado = std::fs::read_to_string(&ruta).unwrap_or_else(|_| panic!("Falta {} (RESGUARDO_ACTUALIZAR_MUESTRAS=1 para crearla)", ruta.display()));
        assert!(esperado.replace("\r\n", "\n") == contenido, "{nombre} cambió (RESGUARDO_ACTUALIZAR_MUESTRAS=1 si es a propósito):\n{contenido}");
    }

    fn sin_secretos(s: &Salida) {
        for t in [&s.asunto, &s.texto, &s.html] {
            for prohibido in ["C:\\", "Users", "claves.txt", "ClaveDelRepo", "10.0.0.2", "SQL\\", "base.bak"] {
                assert!(!t.contains(prohibido), "«{prohibido}» en: {t}");
            }
        }
        // Claro y oscuro, y adaptable al móvil.
        assert!(s.html.contains("prefers-color-scheme:dark") && s.html.contains("max-width:620px") && s.html.contains("name=\"color-scheme\""));
    }

    #[test]
    fn muestras_de_correo() {
        let f = formato();
        let s = componer(&[Mensaje::Aviso(alerta_de_prueba())], &f);
        sin_secretos(&s);
        assert_eq!(s.asunto, "Crítico: Falló la copia «Documentos» en «PC-Contabilidad» · Altamar & Asociados");
        assert_eq!(s.enlace.as_deref(), Some("https://copias.ejemplo.com:8443/c/cl-1/equipos/e1"));
        assert!(s.html.contains("Altamar &amp; Asociados"), "HTML escapado");
        muestra("aviso.html", &s.html);
        muestra("aviso.txt", &s.texto);

        let mut ok = alerta_de_prueba();
        ok.titulo = "Volvió a funcionar la copia «Documentos» en «PC-Contabilidad»".into();
        ok.texto = "Falló 3 veces desde el 2 oct, 10:00; ahora vuelve a ir bien.".into();
        ok.severidad = Severidad::Informativo;
        let s = componer(&[Mensaje::Recuperacion(ok.clone())], &f);
        sin_secretos(&s);
        assert_eq!(s.evento, "recuperacion");
        muestra("recuperacion.txt", &s.texto);

        let mut otra = alerta_de_prueba();
        otra.tipo = "equipo_sin_contacto".into();
        otra.severidad = Severidad::Importante;
        otra.titulo = "«Servidor» no conecta con el servidor".into();
        otra.texto = "«Servidor» lleva más de 24 h sin conectar con el servidor.".into();
        otra.veces = 1;
        let s = componer(&[Mensaje::Aviso(alerta_de_prueba()), Mensaje::Aviso(otra), Mensaje::Recuperacion(ok)], &f);
        sin_secretos(&s);
        assert_eq!(s.evento, "grupo");
        assert_eq!(s.asunto, "3 avisos de Resguardo (1 crítico)");
        muestra("grupo.html", &s.html);
        muestra("grupo.txt", &s.texto);

        let r = Resumen {
            periodo: Periodo::Semanal,
            desde: HASTA - 7 * 86_400,
            hasta: HASTA,
            clientes: vec![crate::notificaciones::resumen::tests::cliente_de_prueba(HASTA)],
        };
        let s = componer(&[Mensaje::Resumen(r)], &f);
        sin_secretos(&s);
        assert_eq!(s.asunto, "Resumen semanal de copias · 27 sep – 4 oct · 2 equipos con problemas");
        muestra("resumen-semanal.html", &s.html);
        muestra("resumen-semanal.txt", &s.texto);

        let s = componer(&[Mensaje::Prueba { quien: "Ana".into() }], &f);
        sin_secretos(&s);
        muestra("prueba.txt", &s.texto);
        assert!(telegram(&s).starts_with("✅ <b>Prueba de notificaciones de Resguardo</b>"));
    }
}
