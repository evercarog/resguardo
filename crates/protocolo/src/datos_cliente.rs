//! 0.7.26 (bloque 8): los **datos comunes del cliente**, iguales en todas sus
//! consolas (docs/consolas-multiples.md §6.5).
//!
//! Los colores de las etiquetas, el catálogo de destinos (nombre, tipo, marcas y
//! días del bloqueo) y las plantillas de copia eran de cada consola: en la local
//! «Servidor» salía azul y en la en línea sin color. Ahora viajan por los equipos
//! del cliente, como el nombre del equipo (v1.56): cada consola manda sus cambios
//! a todos sus equipos con una orden (`datos_cliente`; `datos_cliente_admin` para
//! el tipo y las marcas de un destino, que cuentan en la regla 3-2-1), cada equipo
//! los guarda en un documento pequeño (como mucho [`MAX_BYTES`]) y lo sube con su
//! configuración, y cada consola junta lo de todos sus equipos: **gana el cambio
//! más reciente de cada dato** (y, a la misma hora, el de la identidad mayor).
//!
//! Lo comparten el agente (que guarda el documento) y el servidor (que lo junta):
//! aquí están las claves, la validación de cada valor y el orden entre dos
//! cambios. La consola web hace lo mismo en `consola/src/lib/datosComunes.ts`, y
//! los dos pasan los vectores de `vectors/datos-cliente.json`.
//!
//! Nada de secretos ni de direcciones de consolas: colores, nombres, tipos de
//! destino (con el servidor o el bucket de uno de red, sin credenciales, lo mismo
//! que ya dice el resumen de los equipos) y plantillas **cifradas** tal cual las
//! guardó su consola (con su sal y su cliente para que otra consola, con la misma
//! clave de administración, pueda abrirlas en el navegador; nunca se descifran en
//! el servidor ni en el equipo).

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::cmp::Ordering;
use std::collections::BTreeMap;

/// Como mucho, lo que ocupa el documento en un equipo (JSON).
pub const MAX_BYTES: usize = 256 * 1024;
/// Como mucho, datos en el documento.
pub const MAX_ENTRADAS: usize = 1000;
/// Como mucho, datos en una orden (y el sobre de una orden no pasa de 64 KiB).
pub const MAX_ENTRADAS_ORDEN: usize = 100;
/// Una plantilla cifrada que se comparte: hasta 32 KiB (en base64). Las más
/// grandes siguen siendo de cada consola (no caben en una orden).
pub const MAX_PLANTILLA_B64: usize = 44 * 1024;
/// Colores de la paleta de las etiquetas (`--et-0` … `--et-6`).
pub const N_COLORES: u64 = 7;
/// Tipos del catálogo de destinos (`destinos.tipo` en el servidor).
pub const CLASES_DESTINO: [&str; 7] = ["zona", "rest", "s3", "b2", "sftp", "nube", "local"];

/// Qué es cada dato, por el principio de su clave.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clase {
    /// `etiqueta.color:<etiqueta en minúsculas>` → `{ nombre, color }` o `null` (el automático).
    EtiquetaColor,
    /// `etiqueta.plantilla:<etiqueta>` → `{ nombre, plantilla }` o `null` (sin plantilla).
    EtiquetaPlantilla,
    /// `destino:<id del catálogo>` → `{ nombre, clase, donde? }` o `null` (el nombre de siempre).
    Destino,
    /// `destino.regla:<id>` → `{ clase, atributos }` o `null` (lo deducido). **Pide la clave de administración.**
    DestinoRegla,
    /// `plantilla:<id>` → `{ cifrado, sal, cliente }` o `null` (borrada).
    Plantilla,
    /// 0.8.0 (D1, 14ll): `sede:<id>` → `{ nombre, notas?, principal? }` o `null` (quitada).
    Sede,
    /// 0.8.0 (D1): `sede.equipo:<id del equipo>` → `{ sede }` o `null` (la de por defecto).
    /// **Pide la clave de administración** (cuenta en la regla 3-2-1, como el tipo de un destino).
    SedeEquipo,
    /// 0.8.0 (D1): `sede.destino:<id del catálogo>` → `{ sede }` o `null` (la de su equipo, o
    /// ninguna). **Pide la clave de administración.**
    SedeDestino,
    /// 0.8.0 (D2, 14ss): `etiqueta.categoria:<id>` → `{ nombre, etiquetas: [..] }` o `null`
    /// (una categoría de etiquetas: un solo valor por equipo).
    EtiquetaCategoria,
    /// 0.8.0 (D2): `orden.equipos:<ámbito>` (hoy, `todos`) → `{ ids: [..] }` o `null` (el orden
    /// a mano de los equipos, igual en todas las consolas).
    OrdenEquipos,
    /// 0.8.1 (B2, 15y): `exclusiones:<id>` → `{ nombre, reglas: [..] }` o `null` (quitado). Un
    /// grupo de reglas de «No copiar» del cliente («Música»: `*.mp3`, `*.flac`…), igual en todas
    /// sus consolas. Solo el dato: las copias guardan las reglas que aplican (el equipo no
    /// depende de él). Solo a agentes con [`ADMITE_EXCLUSIONES`].
    GrupoExclusiones,
    /// 0.8.1 (B5, 15pp): `nota.observacion:<tipo>:<objeto>` → `{ texto }` o `null` (sin
    /// observación). La observación de una copia o un repositorio (`<equipo>/<id>`) o de un
    /// destino (su id), igual en todas las consolas. Solo a agentes con [`ADMITE_NOTAS`].
    NotaObservacion,
    /// 0.8.1 (B5): `nota.comentario:<id>` → `{ tipo, objeto, texto, autor, creado, editado? }`
    /// o `null` (borrado). Un comentario de la bitácora de un equipo, una copia, un repositorio
    /// o un destino, con su autor; la consola de origen va en la entrada. Solo con [`ADMITE_NOTAS`].
    NotaComentario,
}

impl Clase {
    pub fn prefijo(self) -> &'static str {
        match self {
            Clase::EtiquetaColor => "etiqueta.color:",
            Clase::EtiquetaPlantilla => "etiqueta.plantilla:",
            Clase::Destino => "destino:",
            Clase::DestinoRegla => "destino.regla:",
            Clase::Plantilla => "plantilla:",
            Clase::Sede => "sede:",
            Clase::SedeEquipo => "sede.equipo:",
            Clase::SedeDestino => "sede.destino:",
            Clase::EtiquetaCategoria => "etiqueta.categoria:",
            Clase::OrdenEquipos => "orden.equipos:",
            Clase::GrupoExclusiones => "exclusiones:",
            Clase::NotaObservacion => "nota.observacion:",
            Clase::NotaComentario => "nota.comentario:",
        }
    }
}

const CLASES: [Clase; 13] = [
    Clase::EtiquetaColor,
    Clase::EtiquetaPlantilla,
    Clase::DestinoRegla,
    Clase::Destino,
    Clase::Plantilla,
    Clase::Sede,
    Clase::SedeEquipo,
    Clase::SedeDestino,
    Clase::EtiquetaCategoria,
    Clase::OrdenEquipos,
    Clase::GrupoExclusiones,
    Clase::NotaObservacion,
    Clase::NotaComentario,
];

/// 0.8.0 (D1, D2): `resumen.admite` de un agente que guarda las sedes, las categorías de
/// etiqueta y el orden de los equipos. A uno anterior no se le mandan (rechazaría la orden entera).
pub const ADMITE_SEDES: &str = "sedes";
/// ¿Es de las clases nuevas de la 0.8.0 (solo para agentes con [`ADMITE_SEDES`])?
pub fn es_de_sedes(clave: &str) -> bool {
    matches!(clase(clave).map(|x| x.0), Some(Clase::Sede | Clase::SedeEquipo | Clase::SedeDestino | Clase::EtiquetaCategoria | Clase::OrdenEquipos))
}
/// Como mucho, sedes, categorías y equipos en el orden a mano.
pub const MAX_SEDES: usize = 50;
pub const MAX_ETIQUETAS_CATEGORIA: usize = 50;
pub const MAX_ORDEN: usize = 500;

/// 0.8.1 (B2, 15y): `resumen.admite` de un agente que guarda los grupos de exclusiones
/// (`exclusiones:<id>`). A uno anterior no se le mandan (rechazaría la orden entera).
pub const ADMITE_EXCLUSIONES: &str = "grupos_exclusiones";
/// ¿Es un grupo de exclusiones (solo para agentes con [`ADMITE_EXCLUSIONES`])?
pub fn es_de_exclusiones(clave: &str) -> bool {
    matches!(clase(clave).map(|x| x.0), Some(Clase::GrupoExclusiones))
}
/// Como mucho, reglas en un grupo de exclusiones y caracteres en cada regla.
pub const MAX_REGLAS_GRUPO: usize = 100;
pub const MAX_LARGO_REGLA: usize = 200;

/// Una regla de «No copiar» (un patrón de exclusión, como los de una copia): sin espacios
/// alrededor, de 1 a [`MAX_LARGO_REGLA`] caracteres, sin caracteres de control y que no lo
/// excluya todo (`*`, `*.*`, `/`, `**/*`…: solo comodines, puntos y barras).
pub fn regla_exclusion_valida(r: &str) -> Result<String, &'static str> {
    let r = r.trim();
    let n = r.chars().count();
    if n == 0 || n > MAX_LARGO_REGLA {
        return Err("Cada regla: de 1 a 200 caracteres.");
    }
    if r.chars().any(char::is_control) {
        return Err("Una regla no puede llevar caracteres de control.");
    }
    if r.chars().all(|c| matches!(c, '*' | '?' | '.' | '/' | '\\')) {
        return Err("Esa regla lo dejaría todo sin copiar.");
    }
    Ok(r.to_string())
}

/// 0.8.1 (B5, 15pp): `resumen.admite` de un agente que guarda observaciones y comentarios
/// (`nota.observacion:`, `nota.comentario:`). A uno anterior no se le mandan.
pub const ADMITE_NOTAS: &str = "notas_comunes";
/// ¿Es una observación o un comentario (solo para agentes con [`ADMITE_NOTAS`])?
pub fn es_de_notas(clave: &str) -> bool {
    matches!(clase(clave).map(|x| x.0), Some(Clase::NotaObservacion | Clase::NotaComentario))
}
/// Caracteres de una observación o de un comentario (como en el servidor).
pub const MAX_TEXTO_NOTA: usize = 2000;
/// Tipos con observación común (la del equipo ya va con `observacion_equipo`, v1.56).
pub const TIPOS_OBSERVACION: [&str; 3] = ["repositorio", "copia", "destino"];
/// Tipos con comentarios comunes.
pub const TIPOS_COMENTARIO: [&str; 4] = ["equipo", "repositorio", "copia", "destino"];

/// Una parte de un objeto (equipo, id de una copia o de un repositorio, id de un destino):
/// letras, cifras y `-_.:@`, de 1 a 128, sin `.` ni `..` solos.
fn parte_objeto_valida(x: &str) -> bool {
    !x.is_empty() && x.len() <= 128 && x != "." && x != ".." && x.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '@'))
}

/// El objeto de una nota según su tipo: `<equipo>/<id>` (copia, repositorio), el id del
/// equipo o el del destino.
pub fn objeto_nota_valido(tipo: &str, objeto: &str) -> bool {
    match tipo {
        "repositorio" | "copia" => objeto.split_once('/').is_some_and(|(e, id)| parte_objeto_valida(e) && parte_objeto_valida(id)),
        "equipo" | "destino" => parte_objeto_valida(objeto),
        _ => false,
    }
}

/// Un dato y quién lo cambió por última vez.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Entrada {
    /// El valor (`null`: quitado, lo de siempre).
    pub valor: Value,
    /// Cuándo (RFC 3339; lo pone la consola que lo cambió).
    pub cambiado: String,
    /// El nombre que esa consola tiene en el equipo (nunca su dirección).
    #[serde(default)]
    pub consola: String,
    /// Su identidad (Ed25519 en base64): desempata y dice si fue «esta consola».
    #[serde(default)]
    pub identidad: String,
    /// Quién lo pidió, si la consola lo dice (informativo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub por: Option<String>,
    /// Un valor que esa consola ya tenía antes de compartir (no un cambio hecho
    /// para todas): cualquier cambio de verdad le gana, y una consola que tenga
    /// otro valor lo enseña como diferencia en vez de pisarlo (§6.5, «Primera vez»).
    #[serde(default, skip_serializing_if = "es_falso")]
    pub semilla: bool,
}

fn es_falso(b: &bool) -> bool {
    !*b
}

/// El documento de un equipo: un dato por clave.
pub type Documento = BTreeMap<String, Entrada>;

/// La clase de una clave y su sujeto (la etiqueta, el id del destino o de la plantilla), si es válida.
pub fn clase(clave: &str) -> Option<(Clase, &str)> {
    let (c, sujeto) = CLASES.iter().find_map(|c| clave.strip_prefix(c.prefijo()).map(|s| (*c, s)))?;
    let ok = match c {
        Clase::EtiquetaColor | Clase::EtiquetaPlantilla => normalizar_etiqueta(sujeto).is_some_and(|n| n.to_lowercase() == sujeto),
        Clase::Destino | Clase::DestinoRegla => id_destino_valido(sujeto),
        Clase::Plantilla | Clase::Sede | Clase::SedeEquipo | Clase::EtiquetaCategoria | Clase::GrupoExclusiones => id_simple(sujeto),
        Clase::SedeDestino => id_destino_valido(sujeto),
        Clase::OrdenEquipos => sujeto == "todos",
        Clase::NotaObservacion => sujeto.split_once(':').is_some_and(|(t, o)| TIPOS_OBSERVACION.contains(&t) && objeto_nota_valido(t, o)),
        Clase::NotaComentario => id_simple(sujeto),
    };
    ok.then_some((c, sujeto))
}

/// ¿Cambiar este dato pide la clave de administración? (Lo que cuenta en la regla 3-2-1: el
/// tipo y las marcas de un destino y, desde la 0.8.0, la sede de un equipo o de un destino.)
pub fn pide_admin(clave: &str) -> bool {
    [Clase::DestinoRegla, Clase::SedeEquipo, Clase::SedeDestino].iter().any(|c| clave.starts_with(c.prefijo()))
}

/// Una etiqueta limpia como las de los equipos: sin espacios de más, de 1 a 32
/// caracteres, sin comas ni caracteres de control.
pub fn normalizar_etiqueta(x: &str) -> Option<String> {
    let t = x.split_whitespace().collect::<Vec<_>>().join(" ");
    (!t.is_empty() && t.chars().count() <= 32 && !t.chars().any(|c| c.is_control() || c == ',')).then_some(t)
}

/// La clave del color de una etiqueta (`etiqueta.color:sede norte`).
pub fn clave_color(etiqueta: &str) -> Option<String> {
    normalizar_etiqueta(etiqueta).map(|n| format!("{}{}", Clase::EtiquetaColor.prefijo(), n.to_lowercase()))
}

/// La clave de la plantilla por defecto de una etiqueta.
pub fn clave_plantilla_etiqueta(etiqueta: &str) -> Option<String> {
    normalizar_etiqueta(etiqueta).map(|n| format!("{}{}", Clase::EtiquetaPlantilla.prefijo(), n.to_lowercase()))
}

/// Id del catálogo de destinos: minúsculas, cifras y `:_.-`, hasta 120 (`zona:<equipo>:<zona>`…).
pub fn id_destino_valido(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 120
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, ':' | '_' | '.' | '-'))
        && !id.split(':').any(|p| p.is_empty() || p == "." || p == "..")
}

/// Id de una plantilla o de un cliente: letras, cifras, `-` y `_`, hasta 64.
pub fn id_simple(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Un texto corto de quien lo cambió (consola, persona): sin caracteres de control, hasta `max`.
pub fn texto_corto(t: &str, max: usize) -> String {
    t.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

/// Un texto de una persona (nombre o notas), sin espacios de más alrededor, de `min` a `max`
/// caracteres y sin caracteres de control (con `lineas`, se admiten los saltos de línea).
fn texto_valido(v: Option<&Value>, min: usize, max: usize, lineas: bool) -> Option<String> {
    let t = v?.as_str()?.trim();
    let n = t.chars().count();
    (n >= min && n <= max && !t.chars().any(|c| c.is_control() && !(lineas && matches!(c, '\n' | '\r' | '\t')))).then(|| t.to_string())
}

fn nombre_destino(n: &str) -> Option<String> {
    let n = n.trim();
    (n.chars().count() <= 80 && !n.chars().any(char::is_control)).then(|| n.to_string())
}

/// ¿Parece una ruta de una carpeta local o de red? (`C:\…`, `\\nas\…`, `/srv/…`, `~/…`).
fn parece_ruta_local(d: &str) -> bool {
    let b = d.as_bytes();
    (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && !d.contains("://")) || d.starts_with('\\') || d.starts_with('/') || d.starts_with('~')
}

/// `donde` de un destino de red (servidor o bucket), sin credenciales ni rutas locales.
/// Los demás tipos no llevan dirección.
pub fn donde_valido(clase: &str, donde: Option<&str>) -> Result<Option<String>, &'static str> {
    let d = donde.map(str::trim).filter(|d| !d.is_empty());
    match clase {
        "rest" | "s3" | "b2" | "sftp" => {}
        _ if d.is_none() => return Ok(None),
        _ => return Err("Solo los destinos de red (rest-server, S3, B2, SFTP) llevan dirección."),
    }
    let Some(d) = d else { return Err("Falta la dirección del destino (servidor o bucket).") };
    if d.chars().count() > 300 || d.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("Dirección no válida (hasta 300 caracteres, sin espacios).");
    }
    // `https://usuario:clave@host` o `usuario@host:clave`: nada de credenciales.
    let sin_esquema = d.split_once("://").map_or(d, |(_, r)| r);
    let autoridad = sin_esquema.split('/').next().unwrap_or_default();
    if autoridad.contains('@') && (clase != "sftp" || autoridad.split_once('@').is_some_and(|(u, _)| u.contains(':'))) {
        return Err("La dirección no puede llevar usuario ni contraseña: las credenciales van selladas para cada equipo.");
    }
    if parece_ruta_local(d) {
        return Err("Una carpeta local no va en el catálogo: se elige al crear el repositorio en su equipo.");
    }
    Ok(Some(d.to_string()))
}

/// Los atributos de un destino para la regla 3-2-1-1-0 (los mismos que el catálogo
/// del servidor), validados y sin campos de más. `{}` → `None` (lo deducido).
pub fn atributos_validos(a: &Value) -> Result<Option<Value>, &'static str> {
    let Some(m) = a.as_object() else { return Err("Atributos no válidos.") };
    let mut out = Map::new();
    for (k, v) in m {
        match k.as_str() {
            "lugar" => {
                let l = v.as_str().filter(|l| ["este_equipo", "oficina", "otra_sede", "nube"].contains(l)).ok_or("Lugar no válido.")?;
                out.insert(k.clone(), json!(l));
            }
            "inmutable" => {
                let i =
                    v.as_str().filter(|i| ["solo_anadir", "object_lock", "instantaneas", "desconectado", "no"].contains(i)).ok_or("Inmutable no válido.")?;
                out.insert(k.clone(), json!(i));
            }
            "soporte" => {
                let s = v.as_str().ok_or("Soporte no válido.")?.trim();
                if s.chars().count() > 60 || s.chars().any(char::is_control) {
                    return Err("El soporte: hasta 60 caracteres, sin caracteres de control.");
                }
                if !s.is_empty() {
                    out.insert(k.clone(), json!(s));
                }
            }
            "tipo" => {
                let t = v.as_str().filter(|t| ["local", "fuera", "nube"].contains(t)).ok_or("Tipo no válido: local, fuera o nube.")?;
                out.insert(k.clone(), json!(t));
            }
            "aislado" => {
                out.insert(k.clone(), json!(v.as_bool().ok_or("Aislado: sí o no.")?));
            }
            "bloqueo_dias" => {
                let d = v.as_u64().filter(|d| (1..=36_500).contains(d)).ok_or("Días de bloqueo: de 1 a 36 500.")?;
                out.insert(k.clone(), json!(d));
            }
            "aislado_dias" => {
                let d = v.as_u64().filter(|d| (1..=365).contains(d)).ok_or("Días sin conectarse: de 1 a 365.")?;
                out.insert(k.clone(), json!(d));
            }
            _ => return Err("Atributo desconocido."),
        }
    }
    Ok((!out.is_empty()).then_some(Value::Object(out)))
}

fn clase_destino(v: &Value) -> Result<String, &'static str> {
    v.as_str().filter(|c| CLASES_DESTINO.contains(c)).map(str::to_string).ok_or("Tipo de destino no válido.")
}

fn solo_campos(m: &Map<String, Value>, permitidos: &[&str]) -> Result<(), &'static str> {
    if m.keys().all(|k| permitidos.contains(&k.as_str())) {
        Ok(())
    } else {
        Err("Campos de más en el valor.")
    }
}

/// El valor de un dato, validado y limpio (`null` siempre vale: quitarlo).
pub fn valor_valido(clave: &str, valor: &Value) -> Result<Value, String> {
    let Some((c, sujeto)) = clase(clave) else { return Err("Dato común desconocido.".into()) };
    if valor.is_null() {
        return Ok(Value::Null);
    }
    let m = valor.as_object().ok_or("Valor no válido.")?;
    let r: Result<Value, &str> = (|| match c {
        Clase::EtiquetaColor | Clase::EtiquetaPlantilla => {
            let campo = if c == Clase::EtiquetaColor { "color" } else { "plantilla" };
            solo_campos(m, &["nombre", campo])?;
            let nombre = m.get("nombre").and_then(Value::as_str).and_then(normalizar_etiqueta).ok_or("Etiqueta no válida.")?;
            if nombre.to_lowercase() != sujeto {
                return Err("La etiqueta no es la de la clave.");
            }
            let x = if c == Clase::EtiquetaColor {
                json!(m.get("color").and_then(Value::as_u64).filter(|n| *n < N_COLORES).ok_or("Color no válido.")?)
            } else {
                json!(m.get("plantilla").and_then(Value::as_str).filter(|p| id_simple(p)).ok_or("Id de plantilla no válido.")?)
            };
            Ok(json!({ "nombre": nombre, campo: x }))
        }
        Clase::Destino => {
            solo_campos(m, &["nombre", "clase", "donde"])?;
            let nombre = nombre_destino(m.get("nombre").and_then(Value::as_str).unwrap_or("")).ok_or("Nombre no válido (hasta 80 caracteres).")?;
            let clase = clase_destino(m.get("clase").unwrap_or(&Value::Null))?;
            let donde = donde_valido(&clase, m.get("donde").and_then(Value::as_str))?;
            let mut v = json!({ "nombre": nombre, "clase": clase });
            if let Some(d) = donde {
                v["donde"] = json!(d);
            }
            Ok(v)
        }
        Clase::DestinoRegla => {
            solo_campos(m, &["clase", "atributos"])?;
            let clase = clase_destino(m.get("clase").unwrap_or(&Value::Null))?;
            let a = atributos_validos(m.get("atributos").unwrap_or(&Value::Null))?.ok_or("Sin atributos: quita el dato (null).")?;
            Ok(json!({ "clase": clase, "atributos": a }))
        }
        Clase::Plantilla => {
            solo_campos(m, &["cifrado", "sal", "cliente"])?;
            let cifrado = m.get("cifrado").and_then(Value::as_str).ok_or("Falta la plantilla cifrada.")?;
            let bytes = B64.decode(cifrado).map_err(|_| "Plantilla no válida (base64).")?;
            // nonce (24) + etiqueta (16) como mínimo.
            if cifrado.len() > MAX_PLANTILLA_B64 || bytes.len() < 40 {
                return Err("Plantilla demasiado grande o vacía (hasta 32 KiB).");
            }
            let sal = m.get("sal").and_then(Value::as_str).ok_or("Falta la sal.")?;
            if !B64.decode(sal).is_ok_and(|s| (8..=64).contains(&s.len())) {
                return Err("Sal no válida.");
            }
            let cliente = m.get("cliente").and_then(Value::as_str).filter(|x| id_simple(x)).ok_or("Cliente no válido.")?;
            Ok(json!({ "cifrado": cifrado, "sal": sal, "cliente": cliente }))
        }
        Clase::Sede => {
            solo_campos(m, &["nombre", "notas", "principal"])?;
            let nombre = texto_valido(m.get("nombre"), 1, 60, false).ok_or("El nombre de la sede: de 1 a 60 caracteres.")?;
            let mut v = json!({ "nombre": nombre });
            if let Some(n) = m.get("notas").filter(|n| !n.is_null()) {
                let n = texto_valido(Some(n), 0, 500, true).ok_or("Las notas de la sede: hasta 500 caracteres.")?;
                if !n.is_empty() {
                    v["notas"] = json!(n);
                }
            }
            match m.get("principal") {
                None | Some(Value::Null) | Some(Value::Bool(false)) => {}
                Some(Value::Bool(true)) => v["principal"] = json!(true),
                _ => return Err("Principal: sí o no."),
            }
            Ok(v)
        }
        Clase::SedeEquipo | Clase::SedeDestino => {
            solo_campos(m, &["sede"])?;
            let sede = m.get("sede").and_then(Value::as_str).filter(|x| id_simple(x)).ok_or("Sede no válida.")?;
            Ok(json!({ "sede": sede }))
        }
        Clase::EtiquetaCategoria => {
            solo_campos(m, &["nombre", "etiquetas"])?;
            let nombre =
                m.get("nombre").and_then(Value::as_str).and_then(normalizar_etiqueta).ok_or("El nombre de la categoría: de 1 a 32 caracteres, sin comas.")?;
            let xs = m.get("etiquetas").and_then(Value::as_array).ok_or("Faltan las etiquetas de la categoría (una lista).")?;
            if xs.len() > MAX_ETIQUETAS_CATEGORIA {
                return Err("Como mucho 50 etiquetas en una categoría.");
            }
            let mut out: Vec<String> = Vec::new();
            for x in xs {
                let t = x.as_str().and_then(normalizar_etiqueta).ok_or("Etiqueta no válida en la categoría.")?;
                if !out.iter().any(|o| o.to_lowercase() == t.to_lowercase()) {
                    out.push(t);
                }
            }
            Ok(json!({ "nombre": nombre, "etiquetas": out }))
        }
        Clase::OrdenEquipos => {
            solo_campos(m, &["ids"])?;
            let xs = m.get("ids").and_then(Value::as_array).ok_or("Faltan los equipos (una lista).")?;
            if xs.len() > MAX_ORDEN {
                return Err("Como mucho 500 equipos en el orden.");
            }
            let mut out: Vec<&str> = Vec::new();
            for x in xs {
                let id = x.as_str().filter(|x| id_simple(x)).ok_or("Id de equipo no válido.")?;
                if !out.contains(&id) {
                    out.push(id);
                }
            }
            Ok(json!({ "ids": out }))
        }
        Clase::GrupoExclusiones => {
            solo_campos(m, &["nombre", "reglas"])?;
            let nombre = texto_valido(m.get("nombre"), 1, 60, false).ok_or("El nombre del grupo: de 1 a 60 caracteres.")?;
            let xs = m.get("reglas").and_then(Value::as_array).ok_or("Faltan las reglas del grupo (una lista).")?;
            if xs.is_empty() || xs.len() > MAX_REGLAS_GRUPO {
                return Err("Un grupo lleva de 1 a 100 reglas.");
            }
            let mut out: Vec<String> = Vec::new();
            for x in xs {
                let r = regla_exclusion_valida(x.as_str().ok_or("Regla no válida.")?)?;
                if !out.contains(&r) {
                    out.push(r);
                }
            }
            Ok(json!({ "nombre": nombre, "reglas": out }))
        }
        Clase::NotaObservacion => {
            solo_campos(m, &["texto"])?;
            let texto = texto_valido(m.get("texto"), 1, MAX_TEXTO_NOTA, true).ok_or("La observación: de 1 a 2000 caracteres (sin texto, quita el dato).")?;
            Ok(json!({ "texto": texto }))
        }
        Clase::NotaComentario => {
            solo_campos(m, &["tipo", "objeto", "texto", "autor", "creado", "editado"])?;
            let tipo = m
                .get("tipo")
                .and_then(Value::as_str)
                .filter(|t| TIPOS_COMENTARIO.contains(t))
                .ok_or("Tipo no válido (equipo, repositorio, copia o destino).")?;
            let objeto = m.get("objeto").and_then(Value::as_str).filter(|o| objeto_nota_valido(tipo, o)).ok_or("Objeto no válido.")?;
            let texto = texto_valido(m.get("texto"), 1, MAX_TEXTO_NOTA, true).ok_or("El comentario: de 1 a 2000 caracteres.")?;
            let autor = texto_valido(m.get("autor"), 1, 80, false).ok_or("El autor: de 1 a 80 caracteres.")?;
            let creado = m.get("creado").and_then(Value::as_str).filter(|c| milis(c).is_some()).ok_or("Fecha no válida.")?;
            let mut v = json!({ "tipo": tipo, "objeto": objeto, "texto": texto, "autor": autor, "creado": creado });
            match m.get("editado") {
                None | Some(Value::Null) => {}
                Some(x) => v["editado"] = json!(x.as_str().filter(|c| milis(c).is_some()).ok_or("Fecha no válida.")?),
            }
            Ok(v)
        }
    })();
    r.map_err(str::to_string)
}

/// La hora de un cambio en milisegundos (`None` si no es RFC 3339).
pub fn milis(cambiado: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(cambiado).ok().map(|d| d.timestamp_millis())
}

/// El orden entre dos cambios del mismo dato: un cambio de verdad gana a una
/// semilla; entre iguales, el más reciente; a la misma hora, la identidad mayor
/// (así todas las consolas y todos los equipos eligen lo mismo).
pub fn comparar(a: &Entrada, b: &Entrada) -> Ordering {
    (!a.semilla, milis(&a.cambiado).unwrap_or(i64::MIN), a.identidad.as_str()).cmp(&(!b.semilla, milis(&b.cambiado).unwrap_or(i64::MIN), b.identidad.as_str()))
}

/// ¿`a` gana a `b`?
pub fn gana(a: &Entrada, b: &Entrada) -> bool {
    comparar(a, b) == Ordering::Greater
}

/// Pone `e` en el documento si gana a lo que hubiera. Devuelve si cambió.
pub fn fusionar(doc: &mut Documento, clave: &str, e: Entrada) -> bool {
    match doc.get(clave) {
        Some(x) if !gana(&e, x) => false,
        _ => {
            doc.insert(clave.to_string(), e);
            true
        }
    }
}

/// Lo que ocupa el documento (JSON).
pub fn bytes(doc: &Documento) -> usize {
    serde_json::to_string(doc).map(|s| s.len()).unwrap_or(usize::MAX)
}

/// Un documento que llega de fuera (un equipo, otra versión): solo las entradas
/// válidas, limpias; lo demás se ignora.
pub fn leer_documento(v: &Value) -> Documento {
    let mut doc = Documento::new();
    let Some(m) = v.as_object() else { return doc };
    for (clave, e) in m.iter().take(MAX_ENTRADAS) {
        let Ok(mut e) = serde_json::from_value::<Entrada>(e.clone()) else { continue };
        let Ok(valor) = valor_valido(clave, &e.valor) else { continue };
        if milis(&e.cambiado).is_none() {
            continue;
        }
        e.valor = valor;
        e.consola = texto_corto(&e.consola, 60);
        e.identidad = texto_corto(&e.identidad, 100);
        e.por = e.por.as_deref().map(|p| texto_corto(p, 60)).filter(|p| !p.is_empty());
        doc.insert(clave.clone(), e);
    }
    doc
}

/// Huella corta del documento (para el resumen: cambia si cambia algo).
pub fn huella(doc: &Documento) -> String {
    use sha2::{Digest, Sha256};
    let h = Sha256::digest(serde_json::to_vec(doc).unwrap_or_default());
    h.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(valor: Value, cambiado: &str, identidad: &str, semilla: bool) -> Entrada {
        Entrada { valor, cambiado: cambiado.into(), consola: String::new(), identidad: identidad.into(), por: None, semilla }
    }

    #[test]
    fn claves() {
        assert_eq!(clave_color("  Sede   Norte ").as_deref(), Some("etiqueta.color:sede norte"));
        assert_eq!(clase("etiqueta.color:sede norte"), Some((Clase::EtiquetaColor, "sede norte")));
        assert!(clase("etiqueta.color:Sede Norte").is_none(), "en minúsculas");
        assert!(clase("etiqueta.color:a,b").is_none());
        assert_eq!(clase("destino.regla:zona:e1:principal").map(|x| x.0), Some(Clase::DestinoRegla));
        assert_eq!(clase("destino:destino-1a2b").map(|x| x.0), Some(Clase::Destino));
        assert!(clase("destino:../x").is_none());
        assert_eq!(clase("plantilla:pla-123").map(|x| x.0), Some(Clase::Plantilla));
        assert!(clase("plantilla:a/b").is_none());
        assert!(clase("otra:cosa").is_none());
        assert!(pide_admin("destino.regla:x") && !pide_admin("destino:x") && !pide_admin("etiqueta.color:x"));
    }

    #[test]
    fn valores() {
        assert_eq!(
            valor_valido("etiqueta.color:servidor", &json!({ "nombre": " Servidor ", "color": 2 })).unwrap(),
            json!({ "nombre": "Servidor", "color": 2 })
        );
        assert!(valor_valido("etiqueta.color:servidor", &json!({ "nombre": "Otro", "color": 2 })).is_err(), "otra etiqueta");
        assert!(valor_valido("etiqueta.color:servidor", &json!({ "nombre": "Servidor", "color": 7 })).is_err());
        assert!(valor_valido("etiqueta.color:servidor", &json!({ "nombre": "Servidor", "color": 1, "x": 1 })).is_err());
        assert_eq!(valor_valido("etiqueta.color:servidor", &Value::Null).unwrap(), Value::Null);
        assert!(valor_valido("etiqueta.plantilla:servidor", &json!({ "nombre": "Servidor", "plantilla": "pla-1" })).is_ok());
        assert!(valor_valido("etiqueta.plantilla:servidor", &json!({ "nombre": "Servidor", "plantilla": "a b" })).is_err());
        // Destinos: sin credenciales ni rutas locales.
        assert_eq!(
            valor_valido("destino:destino-1", &json!({ "nombre": " Nube sur ", "clase": "b2", "donde": "copias-sur" })).unwrap(),
            json!({ "nombre": "Nube sur", "clase": "b2", "donde": "copias-sur" })
        );
        assert!(valor_valido("destino:destino-1", &json!({ "nombre": "x", "clase": "rest", "donde": "https://ana:secreta@nas.ejemplo.com" })).is_err());
        assert!(valor_valido("destino:destino-1", &json!({ "nombre": "x", "clase": "local", "donde": "D:\\Copias" })).is_err());
        assert!(valor_valido("destino:destino-1", &json!({ "nombre": "x", "clase": "luna" })).is_err());
        assert!(valor_valido("destino:destino-1", &json!({ "nombre": "x", "clase": "zona", "clave": "K001" })).is_err());
        assert_eq!(
            valor_valido(
                "destino.regla:zona:e1:principal",
                &json!({ "clase": "zona", "atributos": { "tipo": "fuera", "aislado": true, "aislado_dias": 14, "soporte": " " } })
            )
            .unwrap(),
            json!({ "clase": "zona", "atributos": { "tipo": "fuera", "aislado": true, "aislado_dias": 14 } })
        );
        assert!(valor_valido("destino.regla:x", &json!({ "clase": "zona", "atributos": {} })).is_err(), "vacío: null");
        assert!(valor_valido("destino.regla:x", &json!({ "clase": "zona", "atributos": { "bloqueo_dias": 0 } })).is_err());
        assert!(valor_valido("destino.regla:x", &json!({ "clase": "zona", "atributos": { "clave": "x" } })).is_err());
        // Plantillas: cifradas, con su sal y su cliente.
        let cifrado = B64.encode([7u8; 60]);
        let sal = B64.encode([1u8; 16]);
        assert!(valor_valido("plantilla:pla-1", &json!({ "cifrado": cifrado, "sal": sal, "cliente": "c-1" })).is_ok());
        assert!(valor_valido("plantilla:pla-1", &json!({ "cifrado": B64.encode([7u8; 10]), "sal": sal, "cliente": "c-1" })).is_err());
        assert!(valor_valido("plantilla:pla-1", &json!({ "cifrado": B64.encode(vec![7u8; 40 * 1024]), "sal": sal, "cliente": "c-1" })).is_err(), "grande");
        assert!(valor_valido("plantilla:pla-1", &json!({ "cifrado": cifrado, "sal": "x", "cliente": "c-1" })).is_err());
    }

    /// 0.8.0 (D1, D2): sedes, sede de equipos y destinos, categorías de etiqueta y orden a mano.
    #[test]
    fn sedes_categorias_y_orden() {
        assert_eq!(
            valor_valido("sede:norte", &json!({ "nombre": " Oficina Norte ", "notas": " Planta baja\ny sótano ", "principal": true })).unwrap(),
            json!({ "nombre": "Oficina Norte", "notas": "Planta baja\ny sótano", "principal": true })
        );
        assert_eq!(valor_valido("sede:norte", &json!({ "nombre": "Norte", "notas": "", "principal": false })).unwrap(), json!({ "nombre": "Norte" }));
        assert!(valor_valido("sede:norte", &json!({ "nombre": "" })).is_err(), "sin nombre");
        assert!(valor_valido("sede:norte", &json!({ "nombre": "a".repeat(61) })).is_err());
        assert!(valor_valido("sede:norte", &json!({ "nombre": "Norte\u{7}" })).is_err());
        assert!(valor_valido("sede:norte", &json!({ "nombre": "Norte", "notas": "x".repeat(501) })).is_err());
        assert!(valor_valido("sede:norte", &json!({ "nombre": "Norte", "direccion": "Calle 1" })).is_err(), "campos de más");
        assert!(valor_valido("sede:norte", &json!({ "nombre": "Norte", "principal": "si" })).is_err());
        assert_eq!(valor_valido("sede.equipo:e-1", &json!({ "sede": "norte" })).unwrap(), json!({ "sede": "norte" }));
        assert!(valor_valido("sede.destino:zona:e1:principal", &json!({ "sede": "a b" })).is_err());
        assert!(valor_valido("sede.destino:zona:e1:principal", &json!({ "sede": "norte", "x": 1 })).is_err());
        assert_eq!(
            valor_valido("etiqueta.categoria:tipo", &json!({ "nombre": " Tipo ", "etiquetas": ["Servidor", " Puesto ", "servidor"] })).unwrap(),
            json!({ "nombre": "Tipo", "etiquetas": ["Servidor", "Puesto"] }),
            "sin repetir"
        );
        assert!(valor_valido("etiqueta.categoria:tipo", &json!({ "nombre": "Tipo", "etiquetas": ["a,b"] })).is_err());
        assert!(valor_valido("etiqueta.categoria:tipo", &json!({ "nombre": "Tipo" })).is_err());
        let muchas: Vec<String> = (0..51).map(|i| format!("e{i}")).collect();
        assert!(valor_valido("etiqueta.categoria:tipo", &json!({ "nombre": "Tipo", "etiquetas": muchas })).is_err());
        assert_eq!(valor_valido("orden.equipos:todos", &json!({ "ids": ["b", "a", "b"] })).unwrap(), json!({ "ids": ["b", "a"] }));
        assert!(valor_valido("orden.equipos:todos", &json!({ "ids": ["../a"] })).is_err());
        let ids: Vec<String> = (0..501).map(|i| format!("e{i}")).collect();
        assert!(valor_valido("orden.equipos:todos", &json!({ "ids": ids })).is_err());
        assert!(es_de_sedes("sede:norte") && es_de_sedes("orden.equipos:todos") && !es_de_sedes("etiqueta.color:x") && !es_de_sedes("inventada:x"));
        assert!(pide_admin("sede.equipo:e-1") && pide_admin("sede.destino:x") && !pide_admin("sede:x") && !pide_admin("etiqueta.categoria:x"));
    }

    /// 0.8.1 (B5, 15pp): observaciones y comentarios comunes.
    #[test]
    fn observaciones_y_comentarios() {
        assert_eq!(clase("nota.observacion:copia:e-1/docs").map(|x| x.0), Some(Clase::NotaObservacion));
        assert_eq!(clase("nota.observacion:destino:zona:e-1:principal").map(|x| x.0), Some(Clase::NotaObservacion));
        assert!(clase("nota.observacion:equipo:e-1").is_none(), "la del equipo va con observacion_equipo");
        assert!(clase("nota.observacion:copia:e-1").is_none(), "falta el id");
        assert!(clase("nota.observacion:copia:../x").is_none() && clase("nota.observacion:copia:e 1/x").is_none());
        assert!(clase("nota.comentario:a/b").is_none() && clase("nota.comentario:k-1").is_some());
        assert!(es_de_notas("nota.comentario:k-1") && !es_de_notas("sede:x") && !pide_admin("nota.observacion:copia:e/x"));
        assert_eq!(
            valor_valido("nota.observacion:copia:e/x", &json!({ "texto": " Va al NAS\nde abajo " })).unwrap(),
            json!({ "texto": "Va al NAS\nde abajo" })
        );
        assert!(valor_valido("nota.observacion:copia:e/x", &json!({ "texto": "" })).is_err());
        assert!(valor_valido("nota.observacion:copia:e/x", &json!({ "texto": "x".repeat(2001) })).is_err());
        assert!(valor_valido("nota.observacion:copia:e/x", &json!({ "texto": "a", "por": "b" })).is_err());
        let k = json!({ "tipo": "repositorio", "objeto": "e-1/docs", "texto": "Revisado", "autor": "Ana", "creado": "2026-10-20T10:00:00Z" });
        assert_eq!(valor_valido("nota.comentario:k-1", &k).unwrap(), k);
        for (campo, malo) in [("objeto", json!("e-1")), ("tipo", json!("cliente")), ("creado", json!("ayer")), ("autor", json!(""))] {
            let mut x = k.clone();
            x[campo] = malo;
            assert!(valor_valido("nota.comentario:k-1", &x).is_err(), "{campo}");
        }
        let mut ed = k.clone();
        ed["editado"] = json!("2026-10-20T10:05:00Z");
        assert!(valor_valido("nota.comentario:k-1", &ed).is_ok());
    }

    /// 0.8.1 (B2, 15y): grupos de exclusiones del cliente.
    #[test]
    fn grupos_de_exclusiones() {
        assert_eq!(clase("exclusiones:musica").map(|x| x.0), Some(Clase::GrupoExclusiones));
        assert!(clase("exclusiones:a/b").is_none());
        assert!(es_de_exclusiones("exclusiones:musica") && !es_de_exclusiones("sede:x") && !es_de_sedes("exclusiones:musica"));
        assert!(!pide_admin("exclusiones:musica"));
        assert_eq!(
            valor_valido("exclusiones:musica", &json!({ "nombre": " Música ", "reglas": [" *.mp3 ", "*.flac", "*.mp3"] })).unwrap(),
            json!({ "nombre": "Música", "reglas": ["*.mp3", "*.flac"] }),
            "limpio y sin repetir"
        );
        assert!(valor_valido("exclusiones:musica", &json!({ "nombre": "", "reglas": ["*.mp3"] })).is_err());
        assert!(valor_valido("exclusiones:musica", &json!({ "nombre": "a".repeat(61), "reglas": ["*.mp3"] })).is_err());
        assert!(valor_valido("exclusiones:musica", &json!({ "nombre": "Música", "reglas": [] })).is_err(), "sin reglas");
        assert!(valor_valido("exclusiones:musica", &json!({ "nombre": "Música" })).is_err());
        assert!(valor_valido("exclusiones:musica", &json!({ "nombre": "Música", "reglas": ["*.mp3"], "copias": ["x"] })).is_err(), "campos de más");
        assert!(valor_valido("exclusiones:musica", &json!({ "nombre": "Música", "reglas": ["a\u{7}b"] })).is_err());
        assert!(valor_valido("exclusiones:musica", &json!({ "nombre": "Música", "reglas": ["x".repeat(201)] })).is_err());
        assert!(valor_valido("exclusiones:musica", &json!({ "nombre": "Música", "reglas": [3] })).is_err());
        let muchas: Vec<String> = (0..101).map(|i| format!("*.e{i}")).collect();
        assert!(valor_valido("exclusiones:musica", &json!({ "nombre": "Música", "reglas": muchas })).is_err());
        for todo in ["*", "*.*", "**/*", "/", "\\", " ** "] {
            assert!(regla_exclusion_valida(todo).is_err(), "{todo}");
        }
        assert!(regla_exclusion_valida("Downloads/*.exe").is_ok() && regla_exclusion_valida("~$*").is_ok());
    }

    #[test]
    fn gana_el_mas_reciente_y_un_cambio_a_una_semilla() {
        let mut doc = Documento::new();
        let k = "etiqueta.color:servidor";
        assert!(fusionar(&mut doc, k, e(json!(1), "2026-10-07T10:00:00Z", "a", false)));
        assert!(!fusionar(&mut doc, k, e(json!(2), "2026-10-07T09:00:00Z", "b", false)), "más antiguo");
        assert!(!fusionar(&mut doc, k, e(json!(3), "2026-10-07T12:00:00+02:00", "a", false)), "la misma hora y la misma identidad");
        assert!(fusionar(&mut doc, k, e(json!(4), "2026-10-07T10:00:00Z", "b", false)), "a la misma hora, la identidad mayor");
        assert!(!fusionar(&mut doc, k, e(json!(5), "2026-10-08T10:00:00Z", "z", true)), "una semilla nunca gana a un cambio");
        assert_eq!(doc[k].valor, json!(4));
        let mut d2 = Documento::new();
        assert!(fusionar(&mut d2, k, e(json!(5), "2026-10-08T10:00:00Z", "z", true)));
        assert!(fusionar(&mut d2, k, e(json!(1), "2026-10-01T10:00:00Z", "a", false)), "un cambio gana a una semilla aunque sea anterior");
        assert!(milis("ayer").is_none());
    }

    #[test]
    fn leer_ignora_lo_que_no_vale() {
        let d = leer_documento(&json!({
            "etiqueta.color:servidor": { "valor": { "nombre": "Servidor", "color": 2 }, "cambiado": "2026-10-07T10:00:00Z", "consola": "Oficina\u{7}", "identidad": "a" },
            "etiqueta.color:x": { "valor": { "nombre": "x", "color": 99 }, "cambiado": "2026-10-07T10:00:00Z" },
            "inventada:x": { "valor": null, "cambiado": "2026-10-07T10:00:00Z" },
            "destino:d1": { "valor": null, "cambiado": "no es una hora" },
        }));
        assert_eq!(d.len(), 1);
        assert_eq!(d["etiqueta.color:servidor"].consola, "Oficina");
        assert_eq!(huella(&d).len(), 16);
    }

    /// Los vectores que también pasa la consola (`scripts/vectores-datos-comunes.ts`).
    #[test]
    fn vectores_compartidos() {
        let v: Value = serde_json::from_str(include_str!("../vectors/datos-cliente.json")).unwrap();
        for caso in v["fusion"].as_array().unwrap() {
            let mut doc = Documento::new();
            for x in caso["entradas"].as_array().unwrap() {
                let en: Entrada = serde_json::from_value(x.clone()).unwrap();
                fusionar(&mut doc, "etiqueta.color:servidor", en);
            }
            assert_eq!(doc["etiqueta.color:servidor"].valor, caso["gana"], "{}", caso["nombre"]);
        }
        for caso in v["exclusiones"].as_array().unwrap() {
            let r = valor_valido("exclusiones:grupo", &caso["valor"]);
            assert_eq!(r.is_ok(), caso["valido"] == true, "{}", caso["nombre"]);
            if let Some(l) = caso.get("limpio") {
                assert_eq!(&r.unwrap(), l, "{}", caso["nombre"]);
            }
        }
        for caso in v["claves"].as_array().unwrap() {
            let c = caso["clave"].as_str().unwrap();
            assert_eq!(clase(c).is_some(), caso["valida"] == true, "{c}");
            assert_eq!(pide_admin(c), caso["pide_admin"] == true, "{c}");
        }
    }
}
