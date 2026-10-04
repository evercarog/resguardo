//! Lo que muestra el icono de la bandeja de un equipo gestionado (`--tray`).
//!
//! El servicio (SYSTEM) puede leer el estado cifrado y escribe cada poco
//! `gestionado-bandeja.json` en la carpeta del agente; la bandeja corre como
//! el usuario y solo lee ese archivo. Aquí está todo lo que no depende de
//! Windows, para poder probarlo: el estado que se escribe ([`componer`]), el
//! nivel y el icono que le tocan ([`nivel`], [`variante`], [`pintar`]), los
//! textos ([`tooltip`], [`linea_estado`], [`fila_copia`]) y los avisos al
//! fallar o recuperarse una copia ([`avisos`]).
//!
//! La bandeja no tiene ninguna autoridad nueva: «Copiar ahora» deja una
//! solicitud en `solicitudes\` (la carpeta en la que los usuarios ya pueden
//! escribir y que el agente solo atiende para copias que tiene programadas,
//! ver `agent::request_backup`), y lo demás es leer.

use chrono::{DateTime, Datelike, FixedOffset};
use serde::{Deserialize, Serialize};

/// Archivo que escribe el servicio y lee la bandeja.
pub const ARCHIVO: &str = "gestionado-bandeja.json";

/// Sin noticias del servicio en este tiempo: no está en marcha.
const SERVICIO_CALLADO_MIN: i64 = 15;
/// Sin una copia correcta en este tiempo (y con copias asignadas): atención.
const COPIA_VIEJA_DIAS: i64 = 7;
/// Una copia «en curso» sin noticias en este tiempo se da por terminada (el proceso murió).
const COPIA_COLGADA_MIN: i64 = 30;

/// Cómo está el equipo, de mejor a peor (el icono).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Nivel {
    /// Copias correctas y recientes.
    AlDia,
    /// Sin vincular, sin copias asignadas o antes de la primera copia.
    #[default]
    Esperando,
    /// Una copia en marcha.
    Copiando,
    /// Avisos, copias antiguas o el servicio sin responder.
    Atencion,
    /// Alguna copia falló.
    Error,
}

/// Una copia (un plan) tal como la ve la bandeja: sin rutas ni mensajes.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct CopiaBandeja {
    /// `repo#plan` (la clave de las solicitudes de «Copiar ahora»).
    pub clave: String,
    pub nombre: String,
    /// Resultado de la última vez: "ok", "warning", "error" o "" (aún ninguna).
    #[serde(default)]
    pub resultado: String,
    /// Cuándo terminó la última vez (RFC 3339).
    #[serde(default)]
    pub cuando: Option<String>,
    /// La próxima vez que toca (RFC 3339).
    #[serde(default)]
    pub proxima: Option<String>,
    #[serde(default)]
    pub pausada: bool,
}

/// La copia que está en marcha.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct EnCurso {
    pub clave: String,
    pub nombre: String,
    /// De 0 a 1, si restic ya lo sabe.
    #[serde(default)]
    pub progreso: Option<f64>,
}

/// `gestionado-bandeja.json`. Los cuatro primeros campos son los de siempre
/// (una bandeja anterior los sigue entendiendo); el resto, de la 0.7.14.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EstadoBandeja {
    /// La línea de estado (para bandejas anteriores).
    #[serde(default)]
    pub text: String,
    /// Quién gestiona el equipo y qué puede hacer con sus copias.
    #[serde(default)]
    pub privacy: String,
    /// La consola deja mostrar el icono.
    #[serde(default = "si")]
    pub show: bool,
    /// La consola pide avisos cuando una copia falla o se recupera.
    #[serde(default)]
    pub toasts: bool,
    /// Vinculado con una consola (Resguardo Server o la consola web).
    #[serde(default)]
    pub vinculado: bool,
    /// Un aviso que manda sobre todo lo demás («ya no gestiona este equipo»).
    #[serde(default)]
    pub aviso: Option<String>,
    /// Dirección de la consola (solo https://).
    #[serde(default)]
    pub consola: Option<String>,
    #[serde(default)]
    pub copias: Vec<CopiaBandeja>,
    #[serde(default)]
    pub en_curso: Option<EnCurso>,
    /// Se puede pedir «Copiar ahora» (existe la carpeta de solicitudes).
    #[serde(default)]
    pub pedir: bool,
    /// Cuándo lo escribió el servicio (RFC 3339).
    #[serde(default)]
    pub escrito: Option<String>,
}

fn si() -> bool {
    true
}

impl Default for EstadoBandeja {
    fn default() -> Self {
        EstadoBandeja {
            text: "Resguardo Agente".into(),
            privacy: String::new(),
            show: true,
            toasts: false,
            vinculado: false,
            aviso: None,
            consola: None,
            copias: Vec::new(),
            en_curso: None,
            pedir: false,
            escrito: None,
        }
    }
}

fn fecha(s: Option<&str>) -> Option<DateTime<FixedOffset>> {
    s.and_then(|s| DateTime::parse_from_rfc3339(s).ok())
}

// ---------- Lo que escribe el servicio ----------

/// De dónde sale el estado (todo lo lee el servicio).
pub struct Fuentes<'a> {
    /// Emparejado con la consola web (fase 5).
    pub fase5: Option<&'a crate::endpoint::EndpointState>,
    /// Vinculado con Resguardo Server.
    pub vinculo: Option<&'a crate::servidor_v2::Vinculo>,
    pub config: &'a crate::agent::AgentConfig,
    pub estado: &'a crate::agent::AgentState,
    /// Existe la carpeta de solicitudes (se puede pedir «Copiar ahora»).
    pub solicitudes: bool,
}

/// El anfitrión de una dirección («copias.ejemplo.com:8443»).
fn anfitrion(url: &str) -> &str {
    let sin = url.split_once("://").map_or(url, |(_, r)| r);
    sin.split(['/', '?', '#']).next().unwrap_or(sin)
}

/// Las copias que tiene programadas el agente, con su último resultado y la próxima vez.
pub fn copias_de(config: &crate::agent::AgentConfig, estado: &crate::agent::AgentState, ahora: &DateTime<FixedOffset>) -> Vec<CopiaBandeja> {
    let local = ahora.with_timezone(&chrono::Local);
    let mut out = Vec::new();
    for repo in &config.repos {
        let pausa = repo.active_pause(local);
        for plan in &repo.plans {
            let clave = crate::plans::plan_key(&repo.id, &plan.id);
            let run = estado.runs.get(&clave);
            let desde = match pausa {
                None => Some(local),
                Some(p) => fecha(p.until.as_deref()).map(|u| u.with_timezone(&chrono::Local).max(local)),
            };
            out.push(CopiaBandeja {
                clave,
                nombre: plan.name.clone(),
                resultado: run.map(|r| r.result.clone()).unwrap_or_default(),
                cuando: run.map(|r| r.finished.clone()).filter(|f| !f.is_empty()),
                proxima: desde.and_then(|d| plan.schedule.next_slot(d)).map(|t| t.to_rfc3339()),
                pausada: pausa.is_some(),
            });
        }
    }
    out
}

/// La copia en marcha, si tiene noticias recientes (si no, el proceso murió).
fn en_curso_de(estado: &crate::agent::AgentState, copias: &[CopiaBandeja], ahora: &DateTime<FixedOffset>) -> Option<EnCurso> {
    let r = estado.running.as_ref()?;
    let visto = fecha(r.updated.as_deref()).or_else(|| fecha(Some(&r.started)))?;
    if (*ahora - visto).num_minutes() > COPIA_COLGADA_MIN {
        return None;
    }
    let clave = crate::plans::plan_key(&r.repo_id, r.plan_id.as_deref().unwrap_or_default());
    let nombre = copias.iter().find(|c| c.clave == clave).map(|c| c.nombre.clone()).unwrap_or_else(|| "Copia de seguridad".into());
    Some(EnCurso { clave, nombre, progreso: r.percent.map(|p| p.clamp(0.0, 1.0)) })
}

/// El estado que escribe el servicio para la bandeja.
pub fn componer(f: &Fuentes, ahora: &DateTime<FixedOffset>) -> EstadoBandeja {
    let copias = copias_de(f.config, f.estado, ahora);
    let en_curso = en_curso_de(f.estado, &copias, ahora);
    let mut e = EstadoBandeja { copias, en_curso, pedir: f.solicitudes, escrito: Some(ahora.to_rfc3339()), ..Default::default() };
    if let Some(v) = f.vinculo {
        e.vinculado = true;
        let host = anfitrion(&v.url).to_string();
        if v.modo == "gestionado" && !v.otras.is_empty() {
            // v1.3x: varias consolas a la vez (todas pueden ver y restaurar).
            let otras: Vec<String> = v.otras.iter().map(|o| anfitrion(&o.url).to_string()).collect();
            e.privacy =
                format!("Este equipo lo gestionan varias consolas ({host}, {}): sus administradores pueden ver y restaurar sus copias.", otras.join(", "));
            if v.url.starts_with("https://") {
                e.consola = Some(v.url.clone());
            }
        } else if v.modo == "gestionado" {
            e.privacy = format!("Este equipo lo gestiona Resguardo Server ({host}): su administrador puede ver y restaurar sus copias.");
            if v.url.starts_with("https://") {
                e.consola = Some(v.url.clone());
            }
        } else {
            e.aviso = Some("Ya no lo gestiona ningún servidor: sigue copiando por su cuenta.".into());
        }
        let bandeja = v.config_v1.as_ref().and_then(|c| c.get("bandeja")).filter(|b| b.is_object());
        e.show = bandeja.and_then(|b| b["visible"].as_bool()).unwrap_or(true);
        e.toasts = bandeja.and_then(|b| b["avisos"].as_bool()).unwrap_or(false);
    } else if let Some(s) = f.fase5 {
        e.vinculado = true;
        e.privacy = format!("Este equipo lo gestiona «{}»: sus copias se guardan allí y su administrador puede restaurarlas.", s.console_name);
        if s.stopped {
            e.aviso = Some(format!("«{}» ya no gestiona este equipo.", s.console_name));
        }
        e.show = s.config.as_ref().is_none_or(|c| c.tray);
        e.toasts = s.config.as_ref().is_some_and(|c| c.tray_toasts);
    }
    e.text = linea_estado(&e, ahora);
    e
}

// ---------- Nivel e icono ----------

/// ¿El servicio dejó de escribir hace tiempo? (Sin fecha, de una versión anterior: no se sabe.)
fn servicio_callado(e: &EstadoBandeja, ahora: &DateTime<FixedOffset>) -> bool {
    fecha(e.escrito.as_deref()).is_some_and(|t| (*ahora - t).num_minutes() > SERVICIO_CALLADO_MIN)
}

/// La última copia correcta (o con avisos) de todas.
fn ultima_buena(e: &EstadoBandeja) -> Option<DateTime<FixedOffset>> {
    e.copias.iter().filter(|c| c.resultado == "ok" || c.resultado == "warning").filter_map(|c| fecha(c.cuando.as_deref())).max()
}

/// La próxima copia de todas.
fn proxima(e: &EstadoBandeja) -> Option<DateTime<FixedOffset>> {
    e.copias.iter().filter_map(|c| fecha(c.proxima.as_deref())).min()
}

pub fn nivel(e: &EstadoBandeja, ahora: &DateTime<FixedOffset>) -> Nivel {
    if !e.vinculado {
        return Nivel::Esperando;
    }
    if servicio_callado(e, ahora) {
        return Nivel::Atencion;
    }
    if e.en_curso.is_some() {
        return Nivel::Copiando;
    }
    if e.copias.iter().any(|c| c.resultado == "error") {
        return Nivel::Error;
    }
    if e.aviso.is_some() || e.copias.iter().any(|c| c.resultado == "warning") {
        return Nivel::Atencion;
    }
    match ultima_buena(e) {
        None => Nivel::Esperando,
        Some(t) if (*ahora - t).num_days() >= COPIA_VIEJA_DIAS => Nivel::Atencion,
        Some(_) => Nivel::AlDia,
    }
}

/// El icono: el nivel y, copiando, el progreso en octavos (para no repintar a cada momento).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Variante {
    pub nivel: Nivel,
    /// De 1 a 8 (octavos del anillo); `None` si aún no se sabe el progreso.
    pub octavos: Option<u8>,
}

pub fn variante(e: &EstadoBandeja, ahora: &DateTime<FixedOffset>) -> Variante {
    let nivel = nivel(e, ahora);
    let octavos = match (nivel, e.en_curso.as_ref().and_then(|c| c.progreso)) {
        // Al menos un octavo (se ve que ha empezado) y ocho solo al terminar.
        (Nivel::Copiando, Some(p)) => Some(((p * 8.0).floor() as u8).clamp(1, 8)),
        _ => None,
    };
    Variante { nivel, octavos }
}

// ---------- Textos ----------

const DIAS: [&str; 7] = ["lunes", "martes", "miércoles", "jueves", "viernes", "sábado", "domingo"];
const MESES: [&str; 12] = ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"];

/// «hace un momento», «hace 12 min», «hace 3 h», «hace 2 días».
pub fn hace(t: DateTime<FixedOffset>, ahora: &DateTime<FixedOffset>) -> String {
    let min = (*ahora - t).num_minutes();
    match min {
        i64::MIN..=0 => "hace un momento".into(),
        1..=59 => format!("hace {min} min"),
        60..=1439 => format!("hace {} h", min / 60),
        1440..=2879 => "hace 1 día".into(),
        _ => format!("hace {} días", min / 1440),
    }
}

/// «a las 15:00», «mañana a las 09:00», «el lunes a las 09:00» o «el 12 de octubre».
pub fn cuando_sera(t: DateTime<FixedOffset>, ahora: &DateTime<FixedOffset>) -> String {
    let dias = (t.date_naive() - ahora.date_naive()).num_days();
    let hora = t.format("%H:%M");
    match dias {
        i64::MIN..=0 => format!("a las {hora}"),
        1 => format!("mañana a las {hora}"),
        2..=6 => format!("el {} a las {hora}", DIAS[t.weekday().num_days_from_monday() as usize]),
        _ => format!("el {} de {}", t.day(), MESES[t.month0() as usize]),
    }
}

fn por_ciento(p: f64) -> String {
    format!("{} %", (p * 100.0).floor().clamp(0.0, 100.0) as u32)
}

/// El estado en una línea (el menú y las bandejas anteriores).
pub fn linea_estado(e: &EstadoBandeja, ahora: &DateTime<FixedOffset>) -> String {
    if !e.vinculado {
        return "Sin vincular a ninguna consola.".into();
    }
    if servicio_callado(e, ahora) {
        return "El servicio de Resguardo no está en marcha.".into();
    }
    if let Some(c) = &e.en_curso {
        return match c.progreso {
            Some(p) => format!("Copiando «{}»… {}", c.nombre, por_ciento(p)),
            None => format!("Copiando «{}»…", c.nombre),
        };
    }
    if let Some(a) = &e.aviso {
        return a.clone();
    }
    let fallidas: Vec<&CopiaBandeja> = e.copias.iter().filter(|c| c.resultado == "error").collect();
    match fallidas.as_slice() {
        [] => {}
        [una] => return format!("Falló la copia «{}».", una.nombre),
        varias => return format!("Fallaron {} copias.", varias.len()),
    }
    if let Some(c) = e.copias.iter().find(|c| c.resultado == "warning") {
        return format!("La copia «{}» terminó con avisos.", c.nombre);
    }
    if e.copias.is_empty() {
        return "Esperando a que la consola asigne copias.".into();
    }
    match ultima_buena(e) {
        None => "Esperando la primera copia.".into(),
        Some(t) if (*ahora - t).num_days() >= COPIA_VIEJA_DIAS => format!("La última copia correcta fue {}.", hace(t, ahora)),
        Some(_) => "Tus archivos están protegidos.".into(),
    }
}

/// Corta un texto a `max` unidades UTF-16 (lo que cabe en los campos de Windows), con «…».
pub fn cortar(s: &str, max: usize) -> String {
    if s.encode_utf16().count() <= max {
        return s.to_string();
    }
    let mut out = String::new();
    let mut n = 0;
    for ch in s.chars() {
        if n + ch.len_utf16() + 1 > max {
            break;
        }
        n += ch.len_utf16();
        out.push(ch);
    }
    out.trim_end().to_string() + "…"
}

/// El texto al pasar el ratón: «Resguardo · última copia hace 12 min · próxima a las 15:00».
pub fn tooltip(e: &EstadoBandeja, ahora: &DateTime<FixedOffset>) -> String {
    let mut partes: Vec<String> = vec!["Resguardo".into()];
    let prox = || proxima(e).map(|t| format!("próxima {}", cuando_sera(t, ahora)));
    if !e.vinculado {
        partes.push("sin vincular a ninguna consola".into());
    } else if servicio_callado(e, ahora) {
        partes.push("el servicio no está en marcha".into());
    } else if let Some(c) = &e.en_curso {
        partes.push(match c.progreso {
            Some(p) => format!("copiando «{}» ({})", c.nombre, por_ciento(p)),
            None => format!("copiando «{}»", c.nombre),
        });
    } else if let Some(a) = &e.aviso {
        partes.push(a.trim_end_matches('.').to_string());
    } else if e.copias.is_empty() {
        partes.push("sin copias asignadas".into());
    } else {
        let fallida = e.copias.iter().filter(|c| c.resultado == "error").max_by_key(|c| fecha(c.cuando.as_deref()));
        match (fallida, ultima_buena(e)) {
            (Some(c), _) => partes.push(match fecha(c.cuando.as_deref()) {
                Some(t) => format!("falló «{}» {}", c.nombre, hace(t, ahora)),
                None => format!("falló «{}»", c.nombre),
            }),
            (None, Some(t)) => partes.push(format!("última copia {}", hace(t, ahora))),
            (None, None) => partes.push("esperando la primera copia".into()),
        }
        partes.extend(prox());
    }
    // Windows no deja más de 127 caracteres.
    cortar(&partes.join(" · "), 127)
}

/// Una fila del submenú «Copias»: «Documentos — correcta hace 12 min».
pub fn fila_copia(c: &CopiaBandeja, en_curso: Option<&EnCurso>, ahora: &DateTime<FixedOffset>) -> String {
    let estado = if let Some(r) = en_curso.filter(|r| r.clave == c.clave) {
        match r.progreso {
            Some(p) => format!("copiando… {}", por_ciento(p)),
            None => "copiando…".into(),
        }
    } else {
        let cuando = fecha(c.cuando.as_deref()).map(|t| format!(" {}", hace(t, ahora))).unwrap_or_default();
        let base = match c.resultado.as_str() {
            "ok" => format!("correcta{cuando}"),
            "warning" => format!("con avisos{cuando}"),
            "error" => format!("falló{cuando}"),
            _ => "aún sin copias".into(),
        };
        if c.pausada {
            format!("{base} · en pausa")
        } else {
            base
        }
    };
    cortar(&format!("{} — {estado}", c.nombre), 120)
}

/// Lo que hace una opción del menú.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Accion {
    /// Abrir la consola en el navegador (solo https://).
    Consola(String),
    /// Abrir el registro del agente (el que pueden leer todos: sin rutas ni contraseñas).
    Registro,
    /// Dejar la solicitud de «Copiar ahora» de estas copias (`repo#plan`).
    Copiar(Vec<String>),
    /// Quitar el icono hasta el próximo inicio de sesión.
    Ocultar,
}

/// Parte un texto en líneas de hasta `ancho` caracteres, por los espacios.
pub fn en_lineas(texto: &str, ancho: usize) -> Vec<String> {
    let mut lineas: Vec<String> = Vec::new();
    for palabra in texto.split_whitespace() {
        match lineas.last_mut() {
            Some(l) if l.chars().count() + 1 + palabra.chars().count() <= ancho => {
                l.push(' ');
                l.push_str(palabra);
            }
            _ => lineas.push(palabra.to_string()),
        }
    }
    lineas
}

/// El menú del icono, descrito (Windows lo construye a partir de esto).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entrada {
    /// Un texto que no se puede pulsar.
    Texto(String),
    Separador,
    Submenu(String, Vec<Entrada>),
    Opcion(String, Accion),
}

/// El menú: el estado en una línea, quién gestiona el equipo, las copias
/// (solo lectura), «Copiar ahora» (si se puede pedir), «Abrir la consola»,
/// «Ver el registro» y «Ocultar el icono».
///
/// `elevado`: la bandeja corre como administrador (la abrió el instalador).
/// Entonces no abre programas (un Bloc de notas de administrador en la
/// sesión de un usuario le daría acceso a todo el equipo).
pub fn menu(e: &EstadoBandeja, ahora: &DateTime<FixedOffset>, elevado: bool) -> Vec<Entrada> {
    let mut m = vec![Entrada::Texto(linea_estado(e, ahora))];
    // Quién gestiona el equipo, en líneas cortas (un menú no parte el texto).
    m.extend(en_lineas(&e.privacy, 60).into_iter().map(Entrada::Texto));
    m.push(Entrada::Separador);
    if e.vinculado {
        let filas: Vec<Entrada> = if e.copias.is_empty() {
            vec![Entrada::Texto("Aún no hay copias asignadas".into())]
        } else {
            e.copias.iter().map(|c| Entrada::Texto(fila_copia(c, e.en_curso.as_ref(), ahora))).collect()
        };
        m.push(Entrada::Submenu("Copias".into(), filas));
        let pedibles: Vec<&CopiaBandeja> = if e.pedir { e.copias.iter().filter(|c| !c.pausada).collect() } else { vec![] };
        match pedibles.as_slice() {
            [] => {}
            [una] => m.push(Entrada::Opcion("Copiar ahora".into(), Accion::Copiar(vec![una.clave.clone()]))),
            varias => {
                let mut sub = vec![Entrada::Opcion("Todas".into(), Accion::Copiar(varias.iter().map(|c| c.clave.clone()).collect())), Entrada::Separador];
                sub.extend(varias.iter().map(|c| Entrada::Opcion(cortar(&c.nombre, 80), Accion::Copiar(vec![c.clave.clone()]))));
                m.push(Entrada::Submenu("Copiar ahora".into(), sub));
            }
        }
    }
    if let Some(url) = e.consola.as_ref().filter(|u| u.starts_with("https://")) {
        m.push(Entrada::Opcion("Abrir la consola".into(), Accion::Consola(url.clone())));
    }
    if !elevado {
        m.push(Entrada::Opcion("Ver el registro".into(), Accion::Registro));
    }
    m.push(Entrada::Separador);
    m.push(Entrada::Opcion("Ocultar el icono hasta el próximo inicio de sesión".into(), Accion::Ocultar));
    m
}

/// Un aviso de Windows (globo o notificación).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Aviso {
    pub titulo: String,
    pub texto: String,
    pub error: bool,
}

/// Lo que hay que avisar entre dos lecturas: una copia que falla o que se recupera.
/// Solo cuenta una ejecución nueva (otra fecha), no volver a leer la misma.
pub fn avisos(antes: &EstadoBandeja, ahora: &EstadoBandeja) -> Vec<Aviso> {
    let mut out = Vec::new();
    for c in &ahora.copias {
        let previa = antes.copias.iter().find(|p| p.clave == c.clave);
        if previa.is_some_and(|p| p.cuando == c.cuando) || c.cuando.is_none() {
            continue;
        }
        let fallaba = previa.is_some_and(|p| p.resultado == "error");
        match (fallaba, c.resultado.as_str()) {
            (false, "error") => out.push(Aviso {
                titulo: cortar(&format!("Falló la copia «{}»", c.nombre), 63),
                texto: "Resguardo lo volverá a intentar. Si se repite, avisa a quien gestiona este equipo.".into(),
                error: true,
            }),
            (true, "ok" | "warning") => out.push(Aviso {
                titulo: cortar(&format!("La copia «{}» vuelve a funcionar", c.nombre), 63),
                texto: "Tus archivos vuelven a estar protegidos.".into(),
                error: false,
            }),
            _ => {}
        }
    }
    out
}

// ---------- El icono, pintado ----------

/// El escudo del logotipo (assets/logo.svg), en su cuadro de 64×64, hecho polígono.
fn escudo() -> Vec<(f32, f32)> {
    fn bezier(out: &mut Vec<(f32, f32)>, p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)) {
        for i in 1..=16 {
            let t = i as f32 / 16.0;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            out.push((a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0, a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1));
        }
    }
    let mut p = vec![(32.0, 11.5), (16.5, 17.0), (16.5, 30.2)];
    bezier(&mut p, (16.5, 30.2), (16.5, 40.3), (23.0, 49.1), (32.0, 52.5));
    bezier(&mut p, (32.0, 52.5), (41.0, 49.1), (47.5, 40.3), (47.5, 30.2));
    p.push((47.5, 17.0));
    p
}

fn dentro(poli: &[(f32, f32)], x: f32, y: f32) -> bool {
    let mut dentro = false;
    let mut j = poli.len() - 1;
    for i in 0..poli.len() {
        let ((xi, yi), (xj, yj)) = (poli[i], poli[j]);
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            dentro = !dentro;
        }
        j = i;
    }
    dentro
}

/// Distancia de un punto a un segmento.
fn a_segmento(x: f32, y: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let t = (((x - a.0) * dx + (y - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    ((x - a.0 - t * dx).powi(2) + (y - a.1 - t * dy).powi(2)).sqrt()
}

/// Colores del escudo (arriba y abajo) de cada nivel.
fn colores(n: Nivel) -> ([f32; 3], [f32; 3]) {
    let c = |r: u8, g: u8, b: u8| [r as f32, g as f32, b as f32];
    match n {
        // El verde azulado de Resguardo (assets/logo.svg).
        Nivel::AlDia | Nivel::Copiando => (c(0x19, 0xa5, 0x8e), c(0x0a, 0x5d, 0x52)),
        Nivel::Esperando => (c(0x8a, 0x97, 0xa8), c(0x4b, 0x55, 0x63)),
        Nivel::Atencion => (c(0xf5, 0xa5, 0x24), c(0xb4, 0x53, 0x09)),
        Nivel::Error => (c(0xe5, 0x48, 0x48), c(0xa1, 0x1d, 0x1d)),
    }
}

/// Opacidad del glifo blanco en un punto del cuadro de 64 (0 a 1).
fn glifo(v: Variante, x: f32, y: f32) -> f32 {
    const C: (f32, f32) = (32.0, 30.5);
    let trazo = |d: f32, ancho: f32| if d <= ancho / 2.0 { 1.0 } else { 0.0 };
    match v.nivel {
        Nivel::AlDia => trazo(a_segmento(x, y, (24.5, 31.0), (29.8, 36.3)).min(a_segmento(x, y, (29.8, 36.3), (39.5, 25.5))), 4.6),
        Nivel::Error => trazo(a_segmento(x, y, (26.5, 25.0), (37.5, 36.0)).min(a_segmento(x, y, (37.5, 25.0), (26.5, 36.0))), 4.6),
        Nivel::Atencion => {
            let palo = trazo(a_segmento(x, y, (32.0, 22.0), (32.0, 31.5)), 4.8);
            let punto = trazo(((x - 32.0).powi(2) + (y - 38.5).powi(2)).sqrt(), 5.4);
            palo.max(punto)
        }
        Nivel::Esperando => [26.0, 32.0, 38.0].iter().map(|&px| trazo(((x - px).powi(2) + (y - C.1).powi(2)).sqrt(), 4.4)).fold(0.0, f32::max),
        Nivel::Copiando => {
            // Un anillo: tenue entero y, encima, lo copiado (desde arriba, en el sentido del reloj).
            let (dx, dy) = (x - C.0, y - C.1);
            let r = (dx * dx + dy * dy).sqrt();
            if (r - 8.0).abs() > 2.1 {
                return 0.0;
            }
            let angulo = dx.atan2(-dy).rem_euclid(std::f32::consts::TAU);
            let hecho = v.octavos.map_or(0.3, |o| o as f32 / 8.0);
            if angulo <= hecho * std::f32::consts::TAU {
                1.0
            } else {
                0.38
            }
        }
    }
}

/// El icono de una variante, `lado`×`lado` píxeles RGBA (sin premultiplicar),
/// con suavizado de 4×4 muestras por píxel.
pub fn pintar(v: Variante, lado: u32) -> Vec<u8> {
    let poli = escudo();
    let n = lado as usize;
    let mut rgba = vec![0u8; n * n * 4];
    // El escudo mide 31×41 en su cuadro: ocupa el 96 % del alto del icono y, un
    // poco más ancho que en el logotipo, se lee mejor a 16 píxeles.
    let escala = lado as f32 * 0.96 / 41.0;
    let (arriba, abajo) = colores(v.nivel);
    const M: usize = 4;
    for py in 0..n {
        for px in 0..n {
            let (mut cubre, mut blanco) = (0.0f32, 0.0f32);
            for sy in 0..M {
                for sx in 0..M {
                    let fx = px as f32 + (sx as f32 + 0.5) / M as f32;
                    let fy = py as f32 + (sy as f32 + 0.5) / M as f32;
                    let x = 32.0 + (fx - lado as f32 / 2.0) / (escala * 1.18);
                    let y = 32.0 + (fy - lado as f32 / 2.0) / escala;
                    if dentro(&poli, x, y) {
                        cubre += 1.0;
                        blanco += glifo(v, x, y);
                    }
                }
            }
            if cubre == 0.0 {
                continue;
            }
            let t = (py as f32 + 0.5) / lado as f32;
            let w = blanco / cubre;
            let i = (py * n + px) * 4;
            for k in 0..3 {
                let base = arriba[k] + (abajo[k] - arriba[k]) * t;
                rgba[i + k] = (base + (255.0 - base) * w).round() as u8;
            }
            rgba[i + 3] = (cubre / (M * M) as f32 * 255.0).round() as u8;
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ahora() -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339("2026-10-05T14:20:00+02:00").unwrap()
    }

    fn copia(nombre: &str, resultado: &str, cuando: Option<&str>, proxima: Option<&str>) -> CopiaBandeja {
        CopiaBandeja {
            clave: format!("r#{nombre}"),
            nombre: nombre.into(),
            resultado: resultado.into(),
            cuando: cuando.map(Into::into),
            proxima: proxima.map(Into::into),
            pausada: false,
        }
    }

    fn vinculado(copias: Vec<CopiaBandeja>) -> EstadoBandeja {
        EstadoBandeja { vinculado: true, copias, escrito: Some("2026-10-05T14:19:30+02:00".into()), ..Default::default() }
    }

    #[test]
    fn tooltip_ultima_y_proxima() {
        let e = vinculado(vec![
            copia("Documentos", "ok", Some("2026-10-05T14:08:00+02:00"), Some("2026-10-05T15:00:00+02:00")),
            copia("Correo", "ok", Some("2026-10-05T09:00:00+02:00"), Some("2026-10-06T09:00:00+02:00")),
        ]);
        assert_eq!(tooltip(&e, &ahora()), "Resguardo · última copia hace 12 min · próxima a las 15:00");
        assert_eq!(nivel(&e, &ahora()), Nivel::AlDia);
        assert_eq!(linea_estado(&e, &ahora()), "Tus archivos están protegidos.");
    }

    #[test]
    fn tooltip_proxima_otro_dia() {
        let e = vinculado(vec![copia("D", "ok", Some("2026-10-04T21:00:00+02:00"), Some("2026-10-06T09:00:00+02:00"))]);
        assert_eq!(tooltip(&e, &ahora()), "Resguardo · última copia hace 17 h · próxima mañana a las 09:00");
        let e = vinculado(vec![copia("D", "ok", Some("2026-10-02T14:00:00+02:00"), Some("2026-10-09T09:00:00+02:00"))]);
        assert_eq!(tooltip(&e, &ahora()), "Resguardo · última copia hace 3 días · próxima el viernes a las 09:00");
        let e = vinculado(vec![copia("D", "ok", Some("2026-10-05T14:19:50+02:00"), Some("2026-10-20T09:00:00+02:00"))]);
        assert_eq!(tooltip(&e, &ahora()), "Resguardo · última copia hace un momento · próxima el 20 de octubre");
    }

    #[test]
    fn copiando_con_progreso() {
        let mut e = vinculado(vec![copia("Documentos", "ok", Some("2026-10-05T10:00:00+02:00"), None)]);
        e.en_curso = Some(EnCurso { clave: "r#Documentos".into(), nombre: "Documentos".into(), progreso: Some(0.427) });
        assert_eq!(tooltip(&e, &ahora()), "Resguardo · copiando «Documentos» (42 %)");
        assert_eq!(linea_estado(&e, &ahora()), "Copiando «Documentos»… 42 %");
        assert_eq!(variante(&e, &ahora()), Variante { nivel: Nivel::Copiando, octavos: Some(3) });
        assert_eq!(fila_copia(&e.copias[0], e.en_curso.as_ref(), &ahora()), "Documentos — copiando… 42 %");
        // Al empezar se ve al menos un octavo; sin progreso, el anillo sin medida.
        e.en_curso.as_mut().unwrap().progreso = Some(0.0);
        assert_eq!(variante(&e, &ahora()).octavos, Some(1));
        e.en_curso.as_mut().unwrap().progreso = None;
        assert_eq!(variante(&e, &ahora()), Variante { nivel: Nivel::Copiando, octavos: None });
        assert_eq!(linea_estado(&e, &ahora()), "Copiando «Documentos»…");
    }

    #[test]
    fn error_manda_sobre_correctas() {
        let e = vinculado(vec![
            copia("Documentos", "ok", Some("2026-10-05T14:00:00+02:00"), Some("2026-10-05T15:00:00+02:00")),
            copia("Contabilidad", "error", Some("2026-10-05T12:10:00+02:00"), Some("2026-10-05T16:00:00+02:00")),
        ]);
        assert_eq!(nivel(&e, &ahora()), Nivel::Error);
        assert_eq!(linea_estado(&e, &ahora()), "Falló la copia «Contabilidad».");
        assert_eq!(tooltip(&e, &ahora()), "Resguardo · falló «Contabilidad» hace 2 h · próxima a las 15:00");
        assert_eq!(fila_copia(&e.copias[1], None, &ahora()), "Contabilidad — falló hace 2 h");
        let mut dos = e.clone();
        dos.copias[0].resultado = "error".into();
        assert_eq!(linea_estado(&dos, &ahora()), "Fallaron 2 copias.");
    }

    #[test]
    fn atencion_avisos_copia_vieja_y_servicio_callado() {
        let e = vinculado(vec![copia("Documentos", "warning", Some("2026-10-05T14:00:00+02:00"), None)]);
        assert_eq!(nivel(&e, &ahora()), Nivel::Atencion);
        assert_eq!(linea_estado(&e, &ahora()), "La copia «Documentos» terminó con avisos.");
        assert_eq!(fila_copia(&e.copias[0], None, &ahora()), "Documentos — con avisos hace 20 min");

        let e = vinculado(vec![copia("Documentos", "ok", Some("2026-09-25T14:00:00+02:00"), None)]);
        assert_eq!(nivel(&e, &ahora()), Nivel::Atencion);
        assert_eq!(linea_estado(&e, &ahora()), "La última copia correcta fue hace 10 días.");

        let mut e = vinculado(vec![copia("Documentos", "ok", Some("2026-10-05T13:00:00+02:00"), None)]);
        e.escrito = Some("2026-10-05T13:30:00+02:00".into());
        assert_eq!(nivel(&e, &ahora()), Nivel::Atencion);
        assert_eq!(tooltip(&e, &ahora()), "Resguardo · el servicio no está en marcha");
        // Un archivo de una versión anterior (sin fecha) no se da por callado.
        e.escrito = None;
        assert_eq!(nivel(&e, &ahora()), Nivel::AlDia);
    }

    #[test]
    fn esperando_sin_vincular_o_sin_copias() {
        let e = EstadoBandeja::default();
        assert_eq!(nivel(&e, &ahora()), Nivel::Esperando);
        assert_eq!(tooltip(&e, &ahora()), "Resguardo · sin vincular a ninguna consola");
        let e = vinculado(vec![]);
        assert_eq!(nivel(&e, &ahora()), Nivel::Esperando);
        assert_eq!(linea_estado(&e, &ahora()), "Esperando a que la consola asigne copias.");
        let e = vinculado(vec![copia("Documentos", "", None, Some("2026-10-05T15:00:00+02:00"))]);
        assert_eq!(nivel(&e, &ahora()), Nivel::Esperando);
        assert_eq!(tooltip(&e, &ahora()), "Resguardo · esperando la primera copia · próxima a las 15:00");
        assert_eq!(fila_copia(&e.copias[0], None, &ahora()), "Documentos — aún sin copias");
    }

    #[test]
    fn aviso_de_la_consola() {
        let mut e = vinculado(vec![copia("Documentos", "ok", Some("2026-10-05T14:00:00+02:00"), None)]);
        e.aviso = Some("«Altamar» ya no gestiona este equipo.".into());
        assert_eq!(nivel(&e, &ahora()), Nivel::Atencion);
        assert_eq!(linea_estado(&e, &ahora()), "«Altamar» ya no gestiona este equipo.");
        assert_eq!(tooltip(&e, &ahora()), "Resguardo · «Altamar» ya no gestiona este equipo");
    }

    #[test]
    fn tooltip_cabe_en_windows() {
        let nombre = "Una copia con un nombre larguísimo que no cabe de ninguna manera en el texto del icono";
        let e = vinculado(vec![copia(nombre, "error", Some("2026-10-05T12:10:00+02:00"), Some("2026-10-05T16:00:00+02:00"))]);
        let t = tooltip(&e, &ahora());
        assert!(t.encode_utf16().count() <= 127, "{t}");
        assert!(t.ends_with('…'));
        assert_eq!(cortar("corto", 127), "corto");
    }

    #[test]
    fn avisos_al_fallar_y_al_recuperarse() {
        let antes = vinculado(vec![copia("Documentos", "ok", Some("2026-10-05T13:00:00+02:00"), None)]);
        // Misma ejecución: nada.
        assert!(avisos(&antes, &antes).is_empty());
        let falla = vinculado(vec![copia("Documentos", "error", Some("2026-10-05T14:00:00+02:00"), None)]);
        let a = avisos(&antes, &falla);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].titulo, "Falló la copia «Documentos»");
        assert!(a[0].error);
        // Vuelve a fallar: no se repite el aviso.
        let otra = vinculado(vec![copia("Documentos", "error", Some("2026-10-05T15:00:00+02:00"), None)]);
        assert!(avisos(&falla, &otra).is_empty());
        let vuelve = vinculado(vec![copia("Documentos", "ok", Some("2026-10-05T16:00:00+02:00"), None)]);
        let a = avisos(&otra, &vuelve);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].titulo, "La copia «Documentos» vuelve a funcionar");
        assert!(!a[0].error);
        // Una copia nueva que falla a la primera también avisa.
        let nueva = vinculado(vec![copia("Correo", "error", Some("2026-10-05T16:00:00+02:00"), None)]);
        assert_eq!(avisos(&vinculado(vec![]), &nueva).len(), 1);
    }

    #[test]
    fn iconos_distintos_por_nivel() {
        let variantes = [
            Variante { nivel: Nivel::AlDia, octavos: None },
            Variante { nivel: Nivel::Esperando, octavos: None },
            Variante { nivel: Nivel::Copiando, octavos: Some(3) },
            Variante { nivel: Nivel::Copiando, octavos: Some(6) },
            Variante { nivel: Nivel::Atencion, octavos: None },
            Variante { nivel: Nivel::Error, octavos: None },
        ];
        let pintados: Vec<Vec<u8>> = variantes.iter().map(|&v| pintar(v, 32)).collect();
        for (i, p) in pintados.iter().enumerate() {
            assert_eq!(p.len(), 32 * 32 * 4);
            // Esquinas transparentes y el centro del escudo opaco.
            assert_eq!(p[3], 0, "{:?}", variantes[i]);
            assert_eq!(p[(32 * 32 - 1) * 4 + 3], 0);
            assert_eq!(p[(16 * 32 + 16) * 4 + 3], 255);
            for q in &pintados[i + 1..] {
                assert_ne!(p, q);
            }
        }
        // El color dice el nivel: rojo para el error, ámbar para la atención.
        let px = |p: &[u8], x: usize, y: usize| [p[(y * 32 + x) * 4], p[(y * 32 + x) * 4 + 1], p[(y * 32 + x) * 4 + 2]];
        let rojo = px(&pintados[5], 16, 27);
        assert!(rojo[0] > 150 && rojo[1] < 80 && rojo[2] < 80, "{rojo:?}");
        let ambar = px(&pintados[4], 16, 27);
        assert!(ambar[0] > 150 && ambar[1] > 70 && ambar[1] < ambar[0] && ambar[2] < 60, "{ambar:?}");
        // Y a 16 píxeles (100 %) también sale.
        assert_eq!(pintar(variantes[0], 16).len(), 16 * 16 * 4);
    }

    #[test]
    fn componer_desde_el_agente() {
        let config: crate::agent::AgentConfig = serde_json::from_value(serde_json::json!({
            "repos": [{
                "id": "r1", "name": "Servidor", "location": "rest:https://x/", "paths": [], "schedule": { "kind": "plans" },
                "enabled_at": "2026-10-01T00:00:00+02:00",
                "plans": [{ "id": "p1", "name": "Documentos", "paths": ["C:\\Datos"],
                            "schedule": { "days": [0,1,2,3,4,5,6], "mode": "at", "times": ["15:00"] },
                            "enabled_at": "2026-10-01T00:00:00+02:00" }]
            }]
        }))
        .unwrap();
        let estado: crate::agent::AgentState = serde_json::from_value(serde_json::json!({
            "runs": { "r1#p1": { "started": "2026-10-05T14:00:00+02:00", "finished": "2026-10-05T14:08:00+02:00", "result": "ok", "message": "C:\\Datos: 3 archivos" } },
            "running": { "repo_id": "r1", "plan_id": "p1", "started": "2026-10-05T14:15:00+02:00", "percent": 0.5, "updated": "2026-10-05T14:19:00+02:00" }
        }))
        .unwrap();
        let vinculo = crate::servidor_v2::Vinculo {
            url: "https://copias.ejemplo.com:8443".into(),
            modo: "gestionado".into(),
            config_v1: Some(serde_json::json!({ "v": 1, "bandeja": { "visible": false, "avisos": true } })),
            ..Default::default()
        };
        let f = Fuentes { fase5: None, vinculo: Some(&vinculo), config: &config, estado: &estado, solicitudes: true };
        let e = componer(&f, &ahora());
        assert!(e.vinculado && e.pedir && !e.show && e.toasts);
        assert_eq!(e.consola.as_deref(), Some("https://copias.ejemplo.com:8443"));
        assert!(e.privacy.contains("copias.ejemplo.com:8443"), "{}", e.privacy);
        assert_eq!(e.copias.len(), 1);
        let c = &e.copias[0];
        assert_eq!((c.clave.as_str(), c.nombre.as_str(), c.resultado.as_str()), ("r1#p1", "Documentos", "ok"));
        assert!(c.proxima.is_some());
        assert_eq!(e.en_curso.as_ref().map(|r| r.nombre.as_str()), Some("Documentos"));
        assert_eq!(e.text, "Copiando «Documentos»… 50 %");
        // Lo que lee el usuario no lleva rutas ni mensajes del agente.
        let json = serde_json::to_string(&e).unwrap();
        assert!(!json.contains("Datos"), "{json}");
        // Una copia «en curso» sin noticias desde hace mucho no se cree.
        let viejo = ahora() + chrono::Duration::hours(2);
        assert!(componer(&f, &viejo).en_curso.is_none());
        // v1.3x: con otra consola, salen las dos.
        let dos = crate::servidor_v2::Vinculo {
            otras: vec![crate::consolas_v2::Enlace { url: "https://consola.ejemplo.com".into(), secreto: "s".into(), ..Default::default() }],
            ..vinculo.clone()
        };
        let e = componer(&Fuentes { vinculo: Some(&dos), ..f }, &ahora());
        assert!(e.privacy.contains("copias.ejemplo.com:8443") && e.privacy.contains("consola.ejemplo.com"), "{}", e.privacy);
        // Desvinculado: sigue copiando solo, sin consola.
        let local = crate::servidor_v2::Vinculo { modo: "local".into(), ..vinculo.clone() };
        let e = componer(&Fuentes { vinculo: Some(&local), ..f }, &ahora());
        assert!(e.consola.is_none() && e.aviso.is_some());
    }

    fn opciones(m: &[Entrada]) -> Vec<String> {
        m.iter()
            .flat_map(|x| match x {
                Entrada::Opcion(t, _) => vec![t.clone()],
                Entrada::Submenu(t, sub) => std::iter::once(format!("{t} ▸")).chain(opciones(sub)).collect(),
                _ => vec![],
            })
            .collect()
    }

    #[test]
    fn menu_segun_el_estado() {
        let mut e = vinculado(vec![copia("Documentos", "ok", Some("2026-10-05T14:08:00+02:00"), None)]);
        e.privacy = "Este equipo lo gestiona Resguardo Server (x): …".into();
        e.consola = Some("https://copias.ejemplo.com:8443".into());
        e.pedir = true;
        let m = menu(&e, &ahora(), false);
        assert_eq!(m[0], Entrada::Texto("Tus archivos están protegidos.".into()));
        assert_eq!(m[1], Entrada::Texto(e.privacy.clone()));
        assert_eq!(m[3], Entrada::Submenu("Copias".into(), vec![Entrada::Texto("Documentos — correcta hace 12 min".into())]));
        assert_eq!(opciones(&m), ["Copias ▸", "Copiar ahora", "Abrir la consola", "Ver el registro", "Ocultar el icono hasta el próximo inicio de sesión"]);
        assert!(m.contains(&Entrada::Opcion("Copiar ahora".into(), Accion::Copiar(vec!["r#Documentos".into()]))));

        // Varias copias: un submenú, sin las que están en pausa.
        e.copias.push(copia("Correo", "", None, None));
        e.copias.push(CopiaBandeja { pausada: true, ..copia("Fotos", "ok", None, None) });
        let m = menu(&e, &ahora(), false);
        let sub = m.iter().find_map(|x| match x {
            Entrada::Submenu(t, s) if t == "Copiar ahora" => Some(s.clone()),
            _ => None,
        });
        let sub = sub.expect("submenú «Copiar ahora»");
        assert_eq!(sub[0], Entrada::Opcion("Todas".into(), Accion::Copiar(vec!["r#Documentos".into(), "r#Correo".into()])));
        assert_eq!(opciones(&sub), ["Todas", "Documentos", "Correo"]);

        // Sin carpeta de solicitudes, sin consola https y como administrador: solo lo que es leer.
        e.pedir = false;
        e.consola = Some("http://inseguro".into());
        let m = menu(&e, &ahora(), true);
        assert_eq!(opciones(&m), ["Copias ▸", "Ocultar el icono hasta el próximo inicio de sesión"]);

        // La línea de quién lo gestiona, partida para que el menú no sea enorme.
        e.privacy = "Este equipo lo gestiona Resguardo Server (copias.ejemplo.com:8443): su administrador puede ver y restaurar sus copias.".into();
        let m = menu(&e, &ahora(), false);
        assert_eq!(m[1], Entrada::Texto("Este equipo lo gestiona Resguardo Server".into()));
        assert_eq!(m[2], Entrada::Texto("(copias.ejemplo.com:8443): su administrador puede ver y".into()));
        assert_eq!(m[3], Entrada::Texto("restaurar sus copias.".into()));
        assert_eq!(m[4], Entrada::Separador);
        assert!(en_lineas("", 60).is_empty());

        // Sin vincular: ni copias ni consola.
        let m = menu(&EstadoBandeja::default(), &ahora(), false);
        assert_eq!(m[0], Entrada::Texto("Sin vincular a ninguna consola.".into()));
        assert_eq!(opciones(&m), ["Ver el registro", "Ocultar el icono hasta el próximo inicio de sesión"]);
    }

    #[test]
    fn archivo_de_una_version_anterior() {
        let e: EstadoBandeja = serde_json::from_str(r#"{"text":"Tus archivos están protegidos.","privacy":"x","show":true,"toasts":false}"#).unwrap();
        assert_eq!(e.text, "Tus archivos están protegidos.");
        assert!(!e.vinculado && e.copias.is_empty() && e.escrito.is_none());
    }

    /// `RESGUARDO_ICONOS_BANDEJA=<carpeta> cargo test -p resguardo-agente iconos_a_disco`
    /// deja los iconos en BMP de 32 bits para verlos.
    #[test]
    fn iconos_a_disco() {
        let Ok(dir) = std::env::var("RESGUARDO_ICONOS_BANDEJA") else { return };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let todas = [
            ("al-dia", Variante { nivel: Nivel::AlDia, octavos: None }),
            ("esperando", Variante { nivel: Nivel::Esperando, octavos: None }),
            ("copiando-3", Variante { nivel: Nivel::Copiando, octavos: Some(3) }),
            ("copiando-6", Variante { nivel: Nivel::Copiando, octavos: Some(6) }),
            ("atencion", Variante { nivel: Nivel::Atencion, octavos: None }),
            ("error", Variante { nivel: Nivel::Error, octavos: None }),
        ];
        for lado in [16u32, 24, 32, 64] {
            for (nombre, v) in todas {
                let rgba = pintar(v, lado);
                let mut bmp = Vec::new();
                let datos = lado * lado * 4;
                bmp.extend_from_slice(b"BM");
                bmp.extend_from_slice(&(54 + datos).to_le_bytes());
                bmp.extend_from_slice(&[0, 0, 0, 0]);
                bmp.extend_from_slice(&54u32.to_le_bytes());
                bmp.extend_from_slice(&40u32.to_le_bytes());
                bmp.extend_from_slice(&(lado as i32).to_le_bytes());
                bmp.extend_from_slice(&(-(lado as i32)).to_le_bytes());
                bmp.extend_from_slice(&1u16.to_le_bytes());
                bmp.extend_from_slice(&32u16.to_le_bytes());
                bmp.extend_from_slice(&[0u8; 24]);
                for p in rgba.chunks(4) {
                    bmp.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
                }
                std::fs::write(dir.join(format!("{nombre}-{lado}.bmp")), bmp).unwrap();
            }
        }
    }
}
