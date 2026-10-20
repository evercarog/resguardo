//! Los errores de las copias, en palabras (plan-mejoras 15oo).
//!
//! Una sola tabla, `crates/protocolo/vectors/errores-copia.json`, para el agente
//! (al terminar una orden del motor de copias), el servidor (avisos y correos de
//! agentes anteriores, que mandaban el texto crudo) y la consola y la ventana
//! (`consola/src/lib/errorCopia.ts`, que importa el mismo archivo). Las pruebas
//! de ese archivo las pasan los dos lados.
//!
//! [`en_palabras`] recibe lo que sea (la salida de error del motor, una línea
//! JSON `exit_error`, un «Falló: …» de un agente 0.8.0 cortado a 160 caracteres o
//! una frase ya en español) y devuelve la frase con qué hacer y, aparte, el texto
//! técnico completo para enseñarlo plegado.

use serde::Deserialize;
use std::sync::OnceLock;

/// La tabla compartida (también la usa la consola).
const TABLA: &str = include_str!("../../protocolo/vectors/errores-copia.json");

#[derive(Deserialize, Default)]
struct Tabla {
    desconocido: String,
    marca_detalle: String,
    reglas: Vec<Regla>,
}

#[derive(Deserialize)]
struct Regla {
    caso: String,
    #[serde(default)]
    alguna: Vec<String>,
    #[serde(default)]
    todas: Vec<String>,
    #[serde(default)]
    ninguna: Vec<String>,
    frase: String,
    #[serde(default)]
    frase_linux: Option<String>,
}

impl Regla {
    fn cumple(&self, minusculas: &str) -> bool {
        (self.alguna.is_empty() || self.alguna.iter().any(|a| minusculas.contains(a.as_str())))
            && self.todas.iter().all(|t| minusculas.contains(t.as_str()))
            && !self.ninguna.iter().any(|n| minusculas.contains(n.as_str()))
    }
}

fn tabla() -> &'static Tabla {
    static T: OnceLock<Tabla> = OnceLock::new();
    // Va dentro del programa y una prueba comprueba que se lee: nunca falla de verdad.
    T.get_or_init(|| serde_json::from_str(TABLA).unwrap_or_default())
}

/// Frase para un error que la tabla no reconoce.
pub fn desconocido() -> &'static str {
    &tabla().desconocido
}

/// «Detalle técnico:», lo que separa la frase del texto original en una sola línea.
pub fn marca_detalle() -> &'static str {
    &tabla().marca_detalle
}

/// Un error dicho para personas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnPalabras {
    /// El caso de la tabla, `desconocido` o `en_palabras` (ya venía en español).
    pub caso: String,
    pub frase: String,
    /// El texto técnico completo (sin JSON alrededor), si lo hay y no es la frase.
    pub detalle: Option<String>,
}

/// Las líneas útiles del texto técnico: el `message` de las líneas JSON de error
/// (también de una línea JSON cortada), sin las de progreso y sin el «Falló: » de
/// los agentes anteriores. Una por línea.
pub fn texto_tecnico(texto: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for linea in texto.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let linea = linea.strip_prefix("Falló:").map(str::trim_start).unwrap_or(linea);
        let util = if linea.starts_with('{') {
            match serde_json::from_str::<serde_json::Value>(linea) {
                Ok(v) => match v["message_type"].as_str() {
                    Some("exit_error") => v["message"].as_str().map(String::from),
                    Some("error") => v["error"]["message"].as_str().or(v["message"].as_str()).map(String::from),
                    Some(_) => None,
                    None => Some(linea.to_string()),
                },
                // Cortada (el `mensaje_corto` de 160 de un agente anterior): el `message` hasta donde llegue.
                Err(_) => Some(mensaje_cortado(linea).unwrap_or_else(|| linea.to_string())),
            }
        } else {
            Some(linea.to_string())
        };
        if let Some(u) = util.map(|u| u.trim().to_string()).filter(|u| !u.is_empty()) {
            if !out.contains(&u) {
                out.push(u);
            }
        }
    }
    out.join("\n")
}

/// El `message` de una línea JSON que no se puede leer entera.
fn mensaje_cortado(linea: &str) -> Option<String> {
    let resto = &linea[linea.find(r#""message":""#)? + 11..];
    let mut out = String::new();
    let mut escapado = false;
    for c in resto.chars() {
        match (escapado, c) {
            (true, 'n') => out.push(' '),
            (true, c) => out.push(c),
            (false, '\\') => {
                escapado = true;
                continue;
            }
            (false, '"') => break,
            (false, c) => out.push(c),
        }
        escapado = false;
    }
    Some(out)
}

/// ¿Parece un texto técnico (y no una frase ya en español)?
fn parece_tecnico(texto: &str) -> bool {
    texto.starts_with("Falló:") || texto.contains("message_type") || texto.contains("Fatal:")
}

/// A qué equipo intentó conectar: « (192.0.2.10)» o vacío.
fn destino(tecnico: &str) -> String {
    let host = |s: &str| -> Option<String> {
        let s = s.trim_start_matches('"');
        // Hasta la ruta; sin el usuario y la contraseña; sin el puerto.
        let s = &s[..s.find(|c: char| c == '/' || c == '"' || c.is_whitespace()).unwrap_or(s.len())];
        let s = s.rsplit_once('@').map_or(s, |(_, h)| h);
        let fin = if s.starts_with('[') { s.find(']').map(|i| i + 1)? } else { s.find(':').unwrap_or(s.len()) };
        let h = &s[..fin];
        let valido = !h.is_empty() && h.len() <= 64 && h.chars().all(|c| c.is_ascii_alphanumeric() || ".-[]:".contains(c));
        valido.then(|| h.to_string())
    };
    let h = tecnico.find("dial tcp ").and_then(|i| host(&tecnico[i + 9..])).or_else(|| tecnico.find("://").and_then(|i| host(&tecnico[i + 3..])));
    h.map(|h| format!(" ({h})")).unwrap_or_default()
}

/// La frase de la regla que cumple el texto técnico, si hay alguna.
pub fn buscar(tecnico: &str, windows: bool) -> Option<(String, String)> {
    let s = tecnico.to_lowercase();
    let r = tabla().reglas.iter().find(|r| r.cumple(&s))?;
    let frase = match (&r.frase_linux, windows) {
        (Some(l), false) => l,
        _ => &r.frase,
    };
    Some((r.caso.clone(), frase.replace("{destino}", &destino(tecnico))))
}

/// ¿Cumple el texto (en minúsculas) la regla de ese caso? (p. ej. «disco_lleno»).
pub fn es_caso(minusculas: &str, caso: &str) -> bool {
    tabla().reglas.iter().filter(|r| r.caso == caso).any(|r| r.cumple(minusculas))
}

/// Cualquier error, en palabras (ver el módulo). En Windows o no según el
/// sistema que lo escribió: la consola y el servidor no lo saben y usan Windows.
pub fn en_palabras_en(texto: &str, windows: bool) -> EnPalabras {
    let t = texto.trim();
    let marca = marca_detalle();
    if let Some((frase, detalle)) = t.split_once(marca) {
        let frase = frase.trim().to_string();
        let caso = if frase == desconocido() { "desconocido" } else { "en_palabras" };
        let detalle = Some(detalle.trim().to_string()).filter(|d| !d.is_empty());
        return EnPalabras { caso: caso.into(), frase, detalle };
    }
    let tecnico = texto_tecnico(t);
    if let Some((caso, frase)) = buscar(&tecnico, windows) {
        let detalle = Some(tecnico).filter(|d| !d.is_empty() && *d != frase);
        return EnPalabras { caso, frase, detalle };
    }
    if parece_tecnico(t) {
        return EnPalabras { caso: "desconocido".into(), frase: desconocido().into(), detalle: Some(tecnico).filter(|d| !d.is_empty()) };
    }
    EnPalabras { caso: "en_palabras".into(), frase: t.to_string(), detalle: None }
}

/// Como [`en_palabras_en`], para textos que llegan de un equipo (no se sabe su sistema).
pub fn en_palabras(texto: &str) -> EnPalabras {
    en_palabras_en(texto, true)
}

/// La frase y el detalle en una sola línea (lo que cabe en un mensaje de un
/// solo texto): «frase Detalle técnico: …», o solo la frase.
pub fn en_una_linea(e: &EnPalabras) -> String {
    match &e.detalle {
        Some(d) => format!("{} {} {}", e.frase, marca_detalle(), d.replace('\n', " · ")),
        None => e.frase.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// Las pruebas compartidas con la consola (`consola/scripts/vectores-errores.ts`).
    #[test]
    fn vectores_compartidos() {
        let doc: Value = serde_json::from_str(TABLA).unwrap();
        assert!(!tabla().reglas.is_empty(), "la tabla se lee");
        for p in doc["pruebas"].as_array().unwrap() {
            let e = en_palabras(p["entrada"].as_str().unwrap());
            assert_eq!(e.caso, p["caso"].as_str().unwrap(), "{}", p["que"]);
            assert_eq!(e.frase, p["frase"].as_str().unwrap(), "{}", p["que"]);
            if let Some(d) = p["detalle"].as_str() {
                assert_eq!(e.detalle.as_deref(), Some(d), "{}", p["que"]);
            }
        }
    }

    #[test]
    fn sin_la_palabra_prohibida_y_estable() {
        for r in &tabla().reglas {
            for f in std::iter::once(&r.frase).chain(r.frase_linux.iter()) {
                assert!(!f.to_lowercase().contains("restic"), "{f}");
                // Una frase ya traducida no se vuelve a traducir.
                let f = f.replace("{destino}", "");
                assert_eq!(en_palabras(&f).frase, f, "{}", r.caso);
                assert!(en_palabras(&f).detalle.is_none(), "{}", r.caso);
            }
        }
        assert!(!desconocido().to_lowercase().contains("restic"));
    }

    #[test]
    fn una_linea_y_vuelta() {
        let e = en_palabras("Fatal: algo raro\nmás detalle");
        assert_eq!(e.caso, "desconocido");
        let l = en_una_linea(&e);
        assert!(!l.contains('\n'), "{l}");
        let de_vuelta = en_palabras(&l);
        assert_eq!((de_vuelta.caso.as_str(), de_vuelta.frase.as_str()), ("desconocido", desconocido()));
        assert_eq!(de_vuelta.detalle.as_deref(), Some("Fatal: algo raro · más detalle"));
    }

    #[test]
    fn destino_de_la_conexion() {
        assert_eq!(destino("dial tcp 192.0.2.10:8002: connectex"), " (192.0.2.10)");
        assert_eq!(destino("dial tcp [2001:db8::1]:8000: i/o timeout"), " ([2001:db8::1])");
        assert_eq!(destino("Get \"https://ana:***@almacen.ejemplo.test:8000/x\": EOF"), " (almacen.ejemplo.test)");
        assert_eq!(destino("Post [ruta]: EOF"), "");
    }

    #[test]
    fn linux_con_su_frase() {
        let w = en_palabras_en("Fatal: open /srv/x: permission denied", true);
        let l = en_palabras_en("Fatal: open /srv/x: permission denied", false);
        assert!(w.frase.starts_with("Windows") && l.frase.contains("root"), "{w:?} {l:?}");
    }
}
