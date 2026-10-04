//! La ventana y los avisos del agente en el escritorio (docs/agente-ventana.md).
//!
//! Aquí está lo que no depende de Windows, para poder probarlo:
//! - los ajustes [`Escritorio`] (`ventana` y `avisos`), de la configuración
//!   gestionada o deducidos de `bandeja` (consolas anteriores);
//! - lo que está en marcha ([`Actividad`]) y lo último terminado ([`Hecha`]),
//!   sacado de `state.json`, `tasks.json` y del registro en memoria del servicio
//!   ([`en_marcha`]: restauraciones, espejo y nube, que corren dentro de él);
//! - la serie del ritmo de los últimos minutos ([`Serie`], acotada);
//! - qué avisar entre dos lecturas ([`avisos`]) y cuándo abrir la ventana sola
//!   ([`abrir_al_empezar`]);
//! - `gestionado-ventana.json` ([`EstadoVentana`]): lo que lee la ventana.
//!
//! Todo lo que sale de aquí lo lee el usuario: sin rutas, sin mensajes de
//! restic ni del agente, sin secretos. Solo nombres (los de la consola o los
//! que puso el administrador en modo local) y cifras.

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};

/// Lo que escribe el servicio para la ventana (solo si no está apagada).
pub const ARCHIVO_VENTANA: &str = "gestionado-ventana.json";

/// Ventana de la serie del ritmo.
pub const SERIE_SEGUNDOS: i64 = 300;
/// Como mucho tantos puntos (5 minutos a uno cada 2 s).
pub const SERIE_MAX: usize = 150;
/// Días del historial pequeño.
pub const HISTORIAL_DIAS: i64 = 14;

// ---------- Ajustes ----------

/// ¿Ventana? (`escritorio.ventana`).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Ventana {
    /// Sin ventana: la bandeja de siempre.
    Off,
    /// Desde el icono, cuando el usuario quiera.
    #[default]
    SiempreDisponible,
    /// Y además se abre sola al empezar una tarea.
    AlTrabajar,
}

/// ¿Qué avisos? (`escritorio.avisos`).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Avisos {
    Off,
    /// Al fallar y al recuperarse.
    #[default]
    Errores,
    /// También al empezar y al terminar bien.
    Todo,
}

/// `escritorio` de la configuración.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Escritorio {
    #[serde(default)]
    pub ventana: Ventana,
    #[serde(default)]
    pub avisos: Avisos,
}

impl Escritorio {
    /// Valida un `escritorio` que llega (de la consola o del equipo): un objeto
    /// con valores conocidos, sin campos de más.
    pub fn validar(v: &Value) -> Result<Escritorio, String> {
        let o = v.as_object().ok_or("«escritorio» no válido: se espera { ventana, avisos }.")?;
        if let Some(k) = o.keys().find(|k| *k != "ventana" && *k != "avisos") {
            return Err(format!("«escritorio» no admite «{k}»."));
        }
        let ventana = match o.get("ventana").and_then(Value::as_str) {
            Some("off") => Ventana::Off,
            Some("siempre_disponible") => Ventana::SiempreDisponible,
            Some("al_trabajar") => Ventana::AlTrabajar,
            _ => return Err("«escritorio.ventana» tiene que ser off, siempre_disponible o al_trabajar.".into()),
        };
        let avisos = match o.get("avisos").and_then(Value::as_str) {
            Some("off") => Avisos::Off,
            Some("errores") => Avisos::Errores,
            Some("todo") => Avisos::Todo,
            _ => return Err("«escritorio.avisos» tiene que ser off, errores o todo.".into()),
        };
        Ok(Escritorio { ventana, avisos })
    }

    /// Los de una configuración gestionada (`Configuracion` v1). Sin
    /// `escritorio` (consola anterior), de `bandeja`: avisos de errores si
    /// `bandeja.avisos`, y ventana desde el icono si el icono se ve.
    pub fn de_config(c: &Value) -> Escritorio {
        if let Some(e) = c.get("escritorio").and_then(|e| Escritorio::validar(e).ok()) {
            return e;
        }
        let b = c.get("bandeja").filter(|b| b.is_object());
        Escritorio::de_bandeja(b.and_then(|b| b["visible"].as_bool()).unwrap_or(true), b.and_then(|b| b["avisos"].as_bool()).unwrap_or(false))
    }

    /// De lo que decía la bandeja (también la de la consola web, fase 5).
    pub fn de_bandeja(visible: bool, avisos: bool) -> Escritorio {
        Escritorio { ventana: if visible { Ventana::SiempreDisponible } else { Ventana::Off }, avisos: if avisos { Avisos::Errores } else { Avisos::Off } }
    }

    pub fn a_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

// ---------- Lo que está en marcha ----------

/// Una tarea en marcha, tal como la ve el usuario.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Actividad {
    /// Única por ejecución (para abrir la ventana una vez por tarea).
    pub id: String,
    /// Estable entre ejecuciones de lo mismo (`copia:r1#k1`): agrupa los avisos.
    pub clave: String,
    /// `copia`, `restauracion`, `verificacion`, `copia_externa`, `espejo` o `nube`.
    pub tipo: String,
    pub nombre: String,
    /// `preparando`, `antes_de_copiar`, `escaneando`, `subiendo`, `terminando` o `en_marcha`.
    pub fase: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub porcentaje: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archivos: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archivos_total: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes_total: Option<u64>,
    /// Bytes por segundo que procesa restic (lo que lee o salta), si lo sabe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub velocidad: Option<u64>,
    /// Lectura real del disco (bytes/s), si el sistema la da.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lectura: Option<u64>,
    /// Lo que se escribe o sube al destino (bytes/s).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subida: Option<u64>,
    /// Archivos por segundo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archivos_s: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quedan_s: Option<u64>,
    #[serde(default)]
    pub empezo: String,
}

/// Lo último terminado de cada tarea.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Hecha {
    pub clave: String,
    pub tipo: String,
    pub nombre: String,
    /// "ok", "warning" o "error".
    pub resultado: String,
    pub cuando: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
}

/// Tipo de una tarea de `tasks.rs`.
pub fn tipo_tarea(kind: &str) -> &'static str {
    match kind {
        "offsite" => "copia_externa",
        // verify, verify_offsite, restore_test: comprobar que las copias están bien.
        _ => "verificacion",
    }
}

fn redondeo(p: f64) -> f64 {
    (p.clamp(0.0, 1.0) * 1000.0).round() / 1000.0
}

/// La copia en marcha (de `state.json`), si está viva.
pub fn actividad_copia(r: &crate::agent::RunningCopy, nombre: &str, ahora: DateTime<chrono::Local>) -> Option<Actividad> {
    // Las mismas reglas que el progreso para la consola (viva, fase, cifras).
    let t = crate::progreso_v2::tarea_copia(r, Some(nombre), ahora)?;
    let clave = crate::plans::plan_key(&r.repo_id, r.plan_id.as_deref().unwrap_or_default());
    Some(Actividad {
        id: format!("copia:{clave}@{}", r.started),
        clave: format!("copia:{clave}"),
        tipo: "copia".into(),
        nombre: nombre.into(),
        fase: t["fase"].as_str().unwrap_or("preparando").into(),
        porcentaje: t["porcentaje"].as_f64(),
        archivos: t["archivos"].as_u64(),
        archivos_total: t["archivos_total"].as_u64(),
        bytes: t["bytes"].as_u64(),
        bytes_total: t["bytes_total"].as_u64(),
        velocidad: t["velocidad"].as_u64(),
        lectura: t["lectura"].as_u64(),
        subida: t["subida"].as_u64(),
        archivos_s: t["archivos_s"].as_u64(),
        quedan_s: t["quedan_s"].as_u64(),
        empezo: r.started.clone(),
    })
}

/// Una tarea larga (de `tasks.json`).
pub fn actividad_tarea(t: &crate::tasks::RunningTask, nombre: &str) -> Actividad {
    let tipo = tipo_tarea(&t.kind);
    Actividad {
        id: format!("{}:{}@{}", t.kind, t.repo_id, t.started),
        clave: format!("{}:{}", t.kind, t.repo_id),
        tipo: tipo.into(),
        nombre: nombre.into(),
        fase: if t.percent.is_none() && t.done == 0 { "preparando" } else { "en_marcha" }.into(),
        porcentaje: t.percent.map(redondeo),
        bytes: t.bytes_done,
        bytes_total: t.bytes_total,
        quedan_s: t.eta_s,
        empezo: t.started.clone(),
        ..Default::default()
    }
}

/// Lo que corre dentro del propio servicio (restauraciones, espejo, nube):
/// un registro en memoria que el servicio vuelca en los archivos.
pub mod en_marcha {
    use super::{Actividad, Hecha};
    use std::sync::Mutex;

    static EN_CURSO: Mutex<Vec<Actividad>> = Mutex::new(Vec::new());
    static HECHAS: Mutex<Vec<Hecha>> = Mutex::new(Vec::new());
    static CONTADOR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    /// Mientras vive, la tarea cuenta como en marcha. Al soltarlo sin
    /// [`Guarda::terminar`] (un error con `?`, un pánico) queda como fallida.
    pub struct Guarda {
        id: String,
        resultado: Option<String>,
    }

    /// Empieza una tarea: `tipo` como en [`Actividad::tipo`]; `clave`, lo que no cambia entre ejecuciones.
    pub fn empezar(tipo: &str, clave: &str, nombre: &str) -> Guarda {
        let n = CONTADOR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let ahora = chrono::Local::now().to_rfc3339();
        let id = format!("{tipo}:{clave}@{ahora}#{n}");
        let a = Actividad {
            id: id.clone(),
            clave: format!("{tipo}:{clave}"),
            tipo: tipo.into(),
            nombre: nombre.chars().take(80).collect(),
            fase: "en_marcha".into(),
            empezo: ahora,
            ..Default::default()
        };
        if let Ok(mut v) = EN_CURSO.lock() {
            v.push(a);
        }
        Guarda { id, resultado: None }
    }

    impl Guarda {
        /// Cifras de cómo va (si se saben).
        pub fn progreso(&self, bytes: Option<u64>, bytes_total: Option<u64>) {
            if let Ok(mut v) = EN_CURSO.lock() {
                if let Some(a) = v.iter_mut().find(|a| a.id == self.id) {
                    a.bytes = bytes;
                    a.bytes_total = bytes_total;
                    a.porcentaje = match (bytes, bytes_total) {
                        (Some(b), Some(t)) if t > 0 => Some(super::redondeo(b as f64 / t as f64)),
                        _ => None,
                    };
                }
            }
        }

        /// Termina con "ok", "warning" o "error".
        pub fn terminar(mut self, resultado: &str) {
            self.resultado = Some(resultado.into());
        }
    }

    impl Drop for Guarda {
        fn drop(&mut self) {
            let Ok(mut v) = EN_CURSO.lock() else { return };
            let Some(i) = v.iter().position(|a| a.id == self.id) else { return };
            let a = v.remove(i);
            drop(v);
            let hecha = Hecha {
                clave: a.clave,
                tipo: a.tipo,
                nombre: a.nombre,
                resultado: self.resultado.clone().unwrap_or_else(|| "error".into()),
                cuando: chrono::Local::now().to_rfc3339(),
                bytes: a.bytes,
            };
            if let Ok(mut h) = HECHAS.lock() {
                h.retain(|x| x.clave != hecha.clave);
                h.push(hecha);
                // Solo lo último de cada tarea, y no muchas.
                let sobran = h.len().saturating_sub(16);
                h.drain(..sobran);
            }
        }
    }

    pub fn actividades() -> Vec<Actividad> {
        EN_CURSO.lock().map(|v| v.clone()).unwrap_or_default()
    }

    pub fn hechas() -> Vec<Hecha> {
        HECHAS.lock().map(|v| v.clone()).unwrap_or_default()
    }
}

// ---------- La serie del ritmo ----------

/// Una muestra: `[segundo Unix, lectura B/s, escritura B/s, archivos/s, tipo]`.
/// «Escritura» es lo que va al destino: la subida (copia, copia externa, espejo,
/// nube) o lo que se escribe en el disco al restaurar.
pub type Punto = (i64, u64, u64, u64, String);

/// En una verificación, el ritmo que se deduce de los bytes es lectura; en las
/// demás tareas sin ritmos propios, escritura (lo que sube o se restaura).
fn ritmo_es_lectura(tipo: &str) -> bool {
    tipo == "verificacion"
}

/// Los ritmos de los últimos minutos.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Serie {
    pub puntos: VecDeque<Punto>,
    /// Bytes de cada tarea en la muestra anterior (para las que no dan ritmo).
    #[serde(skip)]
    previos: HashMap<String, (i64, u64)>,
}

impl Serie {
    /// Una muestra en `t` con lo que está en marcha: la suma de cada tarea. Si
    /// una tarea no da sus ritmos (restic los da en las copias), la diferencia
    /// de bytes desde la muestra anterior. Sin nada en marcha no se añade nada
    /// (la gráfica se queda con lo último y lo va soltando).
    pub fn muestra(&mut self, t: i64, actividades: &[Actividad]) {
        let (mut lectura, mut escritura, mut archivos) = (0u64, 0u64, 0u64);
        let mut principal: Option<(&str, u64)> = None;
        let mut vistos = HashMap::new();
        for a in actividades {
            let delta = a.bytes.and_then(|b| {
                let (t0, b0) = *self.previos.get(&a.id)?;
                let dt = t - t0;
                (dt > 0 && b >= b0).then(|| (b - b0) / dt as u64)
            });
            if let Some(b) = a.bytes {
                vistos.insert(a.id.clone(), (t, b));
            }
            let (l, e) = if a.lectura.is_some() || a.subida.is_some() || a.velocidad.is_some() {
                (a.lectura.or(a.velocidad).unwrap_or(0), a.subida.unwrap_or(0))
            } else if ritmo_es_lectura(&a.tipo) {
                (delta.unwrap_or(0), 0)
            } else {
                (0, delta.unwrap_or(0))
            };
            lectura = lectura.saturating_add(l);
            escritura = escritura.saturating_add(e);
            archivos = archivos.saturating_add(a.archivos_s.unwrap_or(0));
            if principal.is_none_or(|(_, m)| l.max(e) > m) {
                principal = Some((&a.tipo, l.max(e)));
            }
        }
        self.previos = vistos;
        if let Some((tipo, _)) = principal {
            // Una muestra por segundo como mucho.
            if self.puntos.back().is_some_and(|p| p.0 >= t) {
                self.puntos.pop_back();
            }
            self.puntos.push_back((t, lectura, escritura, archivos, tipo.to_string()));
        } else if let Some(ultimo) = self.puntos.back().filter(|p| p.0 < t && (p.1, p.2, p.3) != (0, 0, 0)) {
            // Terminó: la onda baja a cero (una vez) en vez de quedarse arriba.
            let tipo = ultimo.4.clone();
            self.puntos.push_back((t, 0, 0, 0, tipo));
        }
        self.recortar(t);
    }

    /// Fuera lo de hace más de [`SERIE_SEGUNDOS`] y lo que pase de [`SERIE_MAX`].
    pub fn recortar(&mut self, ahora: i64) {
        while self.puntos.front().is_some_and(|p| p.0 < ahora - SERIE_SEGUNDOS) {
            self.puntos.pop_front();
        }
        while self.puntos.len() > SERIE_MAX {
            self.puntos.pop_front();
        }
    }
}

// ---------- El historial pequeño ----------

/// Un día del historial: cuántas copias salieron bien, con avisos y mal.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Dia {
    pub dia: String,
    pub ok: u32,
    pub aviso: u32,
    pub fallo: u32,
}

/// Los últimos [`HISTORIAL_DIAS`] días (también los que no hubo nada), de las copias del historial.
pub fn historial(entradas: &[crate::history::Entry], hoy: chrono::NaiveDate) -> Vec<Dia> {
    let desde = hoy - chrono::Duration::days(HISTORIAL_DIAS - 1);
    let mut dias: Vec<Dia> = (0..HISTORIAL_DIAS).map(|i| Dia { dia: (desde + chrono::Duration::days(i)).to_string(), ..Default::default() }).collect();
    for e in entradas.iter().filter(|e| e.kind == "backup") {
        let Ok(t) = DateTime::parse_from_rfc3339(&e.finished) else { continue };
        let d = t.with_timezone(&chrono::Local).date_naive();
        if d < desde || d > hoy {
            continue;
        }
        let x = &mut dias[(d - desde).num_days() as usize];
        match e.result.as_str() {
            "ok" => x.ok += 1,
            "warning" => x.aviso += 1,
            "error" => x.fallo += 1,
            _ => {}
        }
    }
    dias
}

// ---------- gestionado-ventana.json ----------

/// Lo que lee la ventana, además de `gestionado-bandeja.json`.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct EstadoVentana {
    pub v: u32,
    #[serde(default)]
    pub escrito: Option<String>,
    /// `[segundo Unix, lectura B/s, escritura B/s, archivos/s, tipo]`.
    #[serde(default)]
    pub serie: Vec<Punto>,
    #[serde(default)]
    pub historial: Vec<Dia>,
}

// ---------- Avisos ----------

/// Un aviso para el escritorio.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvisoEscritorio {
    pub titulo: String,
    pub texto: String,
    pub error: bool,
    /// Agrupa en el centro de actividades (`copias`, `restauraciones`…).
    pub grupo: &'static str,
    /// Una por tarea: «terminó» reemplaza a «empezó».
    pub etiqueta: String,
}

fn grupo(tipo: &str) -> &'static str {
    match tipo {
        "copia" => "copias",
        "restauracion" => "restauraciones",
        "verificacion" => "verificaciones",
        _ => "subidas",
    }
}

/// «la copia», «la verificación»… (para «Falló …»).
fn sustantivo(tipo: &str) -> &'static str {
    match tipo {
        "copia" => "la copia",
        "restauracion" => "la restauración",
        "verificacion" => "la verificación",
        "copia_externa" => "la copia externa",
        "espejo" => "el espejo",
        "nube" => "la subida a la nube",
        _ => "la tarea",
    }
}

fn mayuscula(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|p| p.to_uppercase().chain(c).collect()).unwrap_or_default()
}

fn al_empezar(a: &Actividad) -> (String, String) {
    let titulo = match a.tipo.as_str() {
        "copia" => format!("Copiando «{}»", a.nombre),
        "restauracion" => format!("Restaurando desde «{}»", a.nombre),
        "verificacion" => format!("Verificando «{}»", a.nombre),
        "copia_externa" => format!("Subiendo la copia externa de «{}»", a.nombre),
        "espejo" => format!("Copiando el espejo a «{}»", a.nombre),
        "nube" => format!("Subiendo a «{}»", a.nombre),
        _ => format!("Empezó «{}»", a.nombre),
    };
    (titulo, "Puedes seguir trabajando: te avisaremos al terminar.".into())
}

fn texto_bytes(h: &Hecha) -> String {
    match h.bytes.filter(|b| *b > 0) {
        Some(b) if h.tipo == "copia" => format!(" · {} nuevos", crate::tasks::human_bytes(b)),
        Some(b) => format!(" · {}", crate::tasks::human_bytes(b)),
        None => String::new(),
    }
}

/// Lo que hay que avisar entre dos lecturas, según el nivel elegido:
/// - `errores`: una tarea que falla (y antes no fallaba) o que se recupera;
/// - `todo`: además, una tarea que empieza y una que termina bien.
///
/// Solo cuentan las ejecuciones nuevas (otra fecha) y las tareas nuevas (otro
/// id): volver a leer lo mismo no avisa dos veces. Lo primero que se lee al
/// arrancar la bandeja (`antes` vacío) no avisa de lo que ya había terminado.
pub fn avisos(
    antes_actividades: &[Actividad],
    antes_hechas: &[Hecha],
    actividades: &[Actividad],
    hechas: &[Hecha],
    nivel: Avisos,
    primera: bool,
) -> Vec<AvisoEscritorio> {
    let mut out = Vec::new();
    if nivel == Avisos::Off || primera {
        return out;
    }
    if nivel == Avisos::Todo {
        for a in actividades.iter().filter(|a| !antes_actividades.iter().any(|x| x.id == a.id)) {
            let (titulo, texto) = al_empezar(a);
            out.push(AvisoEscritorio { titulo: crate::bandeja::cortar(&titulo, 63), texto, error: false, grupo: grupo(&a.tipo), etiqueta: a.clave.clone() });
        }
    }
    for h in hechas {
        let previa = antes_hechas.iter().find(|p| p.clave == h.clave);
        if previa.is_some_and(|p| p.cuando == h.cuando) || h.cuando.is_empty() {
            continue;
        }
        let fallaba = previa.is_some_and(|p| p.resultado == "error");
        let que = sustantivo(&h.tipo);
        let aviso = match (fallaba, h.resultado.as_str()) {
            (false, "error") => Some((
                format!("Falló {que} «{}»", h.nombre),
                if h.tipo == "restauracion" {
                    "No se restauró todo. Vuelve a intentarlo o avisa a quien gestiona este equipo.".to_string()
                } else {
                    "Resguardo lo volverá a intentar. Si se repite, avisa a quien gestiona este equipo.".to_string()
                },
                true,
            )),
            (true, "ok" | "warning") => {
                Some((format!("{} «{}» vuelve a funcionar", mayuscula(que), h.nombre), "Tus archivos vuelven a estar protegidos.".into(), false))
            }
            (false, "ok") if nivel == Avisos::Todo => Some((
                match h.tipo.as_str() {
                    "copia" => format!("Copia «{}» terminada", h.nombre),
                    "restauracion" => format!("Restauración desde «{}» terminada", h.nombre),
                    "verificacion" => format!("«{}» verificado: todo en orden", h.nombre),
                    "copia_externa" => format!("Copia externa de «{}» terminada", h.nombre),
                    "espejo" => format!("Espejo en «{}» terminado", h.nombre),
                    "nube" => format!("Subida a «{}» terminada", h.nombre),
                    _ => format!("«{}» terminada", h.nombre),
                },
                format!("Correcta{}", texto_bytes(h)),
                false,
            )),
            (false, "warning") if nivel == Avisos::Todo => Some((
                format!("{} «{}» terminó con avisos", mayuscula(que), h.nombre),
                "Algunos archivos no se pudieron leer. Lo verás en el historial.".into(),
                false,
            )),
            _ => None,
        };
        if let Some((titulo, texto, error)) = aviso {
            out.push(AvisoEscritorio { titulo: crate::bandeja::cortar(&titulo, 63), texto, error, grupo: grupo(&h.tipo), etiqueta: h.clave.clone() });
        }
    }
    out
}

/// ¿Se abre la ventana sola? Con `al_trabajar`, cuando empieza una tarea que
/// no estaba (una vez por tarea: si el usuario la cierra, no vuelve a salir
/// hasta la siguiente). `vistas` guarda las ya vistas.
pub fn abrir_al_empezar(vistas: &mut HashSet<String>, actividades: &[Actividad], ventana: Ventana, primera: bool) -> bool {
    let nuevas: Vec<&Actividad> = actividades.iter().filter(|a| !vistas.contains(&a.id)).collect();
    let abrir = ventana == Ventana::AlTrabajar && !primera && !nuevas.is_empty();
    for a in nuevas {
        vistas.insert(a.id.clone());
    }
    // Sin crecer sin fin: solo las que siguen en marcha.
    vistas.retain(|id| actividades.iter().any(|a| &a.id == id));
    abrir
}

/// Lo último terminado de cada copia, verificación y copia externa (de los
/// estados que ya escriben los procesos del agente) más lo del registro.
pub fn hechas_de(config: &crate::agent::AgentConfig, estado: &crate::agent::AgentState, tareas: &crate::tasks::TasksState) -> Vec<Hecha> {
    let mut out = Vec::new();
    for repo in &config.repos {
        for plan in &repo.plans {
            let key = crate::plans::plan_key(&repo.id, &plan.id);
            if let Some(r) = estado.runs.get(&key).filter(|r| !r.finished.is_empty()) {
                out.push(Hecha {
                    clave: format!("copia:{key}"),
                    tipo: "copia".into(),
                    nombre: plan.name.clone(),
                    resultado: r.result.clone(),
                    cuando: r.finished.clone(),
                    bytes: r.data_added,
                });
            }
        }
        for kind in ["verify", "verify_offsite", "restore_test", "offsite"] {
            if let Some(r) = tareas.runs.get(&crate::tasks::key(kind, &repo.id)).filter(|r| !r.finished.is_empty()) {
                out.push(Hecha {
                    clave: format!("{kind}:{}", repo.id),
                    tipo: tipo_tarea(kind).into(),
                    nombre: repo.name.clone(),
                    resultado: r.result.clone(),
                    cuando: r.finished.clone(),
                    bytes: None,
                });
            }
        }
    }
    out.extend(en_marcha::hechas());
    out
}

/// Lo que está en marcha ahora: la copia, la tarea larga y lo del registro.
pub fn actividades_de(config: &crate::agent::AgentConfig, estado: &crate::agent::AgentState, tareas: &crate::tasks::TasksState) -> Vec<Actividad> {
    let ahora = chrono::Local::now();
    let mut out = Vec::new();
    if let Some(r) = estado.running.as_ref() {
        let nombre = config
            .repos
            .iter()
            .find(|x| x.id == r.repo_id)
            .and_then(|x| x.plans.iter().find(|p| Some(&p.id) == r.plan_id.as_ref()))
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "Copia de seguridad".into());
        out.extend(actividad_copia(r, &nombre, ahora));
    }
    if let Some(t) = tareas.live_running() {
        let nombre = config.repos.iter().find(|x| x.id == t.repo_id).map(|x| x.name.clone()).unwrap_or_else(|| "Copias".into());
        out.push(actividad_tarea(t, &nombre));
    }
    out.extend(en_marcha::actividades());
    out
}

/// ¿Hace falta muestrear a menudo? Solo con algo en marcha y la ventana o los avisos de «empezó» encendidos.
pub fn muestreo_rapido(e: Escritorio, hay_actividad: bool) -> bool {
    hay_actividad && (e.ventana != Ventana::Off || e.avisos == Avisos::Todo)
}

/// ¿Se enseña un aviso ahora? Con una presentación o un juego a pantalla
/// completa (`SHQueryUserNotificationState`: 2 ocupado, 3 Direct3D a pantalla
/// completa, 4 presentación) solo los de error. «No molestar» lo respeta
/// Windows por su cuenta (los guarda en el centro de actividades).
pub fn pasa_aviso(estado_usuario: i32, error: bool) -> bool {
    error || !matches!(estado_usuario, 2..=4)
}

/// Un PNG (RGBA, sin comprimir) del icono, para los avisos de Windows.
pub fn png(rgba: &[u8], lado: u32) -> Vec<u8> {
    fn crc32(datos: &[u8]) -> u32 {
        let mut c = 0xffff_ffffu32;
        for b in datos {
            c ^= *b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    }
    fn trozo(out: &mut Vec<u8>, tipo: &[u8; 4], datos: &[u8]) {
        out.extend_from_slice(&(datos.len() as u32).to_be_bytes());
        let mut t = tipo.to_vec();
        t.extend_from_slice(datos);
        out.extend_from_slice(&t);
        out.extend_from_slice(&crc32(&t).to_be_bytes());
    }
    // Filas con su byte de filtro (0) y, en zlib, bloques «stored» de hasta 65535 bytes.
    let fila = lado as usize * 4;
    let mut crudo = Vec::with_capacity((fila + 1) * lado as usize);
    for y in 0..lado as usize {
        crudo.push(0);
        crudo.extend_from_slice(&rgba[y * fila..(y + 1) * fila]);
    }
    let mut z = vec![0x78, 0x01];
    let trozos: Vec<&[u8]> = crudo.chunks(65_535).collect();
    for (i, t) in trozos.iter().enumerate() {
        z.push(u8::from(i + 1 == trozos.len()));
        z.extend_from_slice(&(t.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(t.len() as u16)).to_le_bytes());
        z.extend_from_slice(t);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for x in &crudo {
        a = (a + *x as u32) % 65_521;
        b = (b + a) % 65_521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&lado.to_be_bytes());
    ihdr.extend_from_slice(&lado.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    trozo(&mut out, b"IHDR", &ihdr);
    trozo(&mut out, b"IDAT", &z);
    trozo(&mut out, b"IEND", &[]);
    out
}

/// Fecha de un texto RFC 3339.
pub fn fecha(s: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(s).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ajustes_de_la_configuracion_y_compatibles() {
        let e = Escritorio::de_config(&json!({ "v": 1, "escritorio": { "ventana": "al_trabajar", "avisos": "todo" } }));
        assert_eq!(e, Escritorio { ventana: Ventana::AlTrabajar, avisos: Avisos::Todo });
        // Sin `escritorio` (consola anterior): de la bandeja.
        assert_eq!(
            Escritorio::de_config(&json!({ "v": 1, "bandeja": { "visible": true, "avisos": true } })),
            Escritorio { ventana: Ventana::SiempreDisponible, avisos: Avisos::Errores }
        );
        assert_eq!(
            Escritorio::de_config(&json!({ "v": 1, "bandeja": { "visible": false, "avisos": false } })),
            Escritorio { ventana: Ventana::Off, avisos: Avisos::Off }
        );
        assert_eq!(Escritorio::de_config(&json!({ "v": 1 })), Escritorio { ventana: Ventana::SiempreDisponible, avisos: Avisos::Off });
        // Uno roto no vale: se queda con lo de la bandeja.
        assert_eq!(Escritorio::de_config(&json!({ "v": 1, "escritorio": { "ventana": "grande" } })).ventana, Ventana::SiempreDisponible);
        // Ida y vuelta.
        let j = e.a_json();
        assert_eq!(j, json!({ "ventana": "al_trabajar", "avisos": "todo" }));
        assert_eq!(Escritorio::validar(&j).unwrap(), e);
        assert!(Escritorio::validar(&json!({ "ventana": "off", "avisos": "off", "otro": 1 })).is_err());
        assert!(Escritorio::validar(&json!({ "ventana": "off" })).is_err());
        assert!(Escritorio::validar(&json!("off")).is_err());
    }

    fn act(id: &str, tipo: &str, bytes: Option<u64>, velocidad: Option<u64>) -> Actividad {
        Actividad {
            id: id.into(),
            clave: format!("{tipo}:x"),
            tipo: tipo.into(),
            nombre: "Documentos".into(),
            fase: "subiendo".into(),
            bytes,
            velocidad,
            ..Default::default()
        }
    }

    #[test]
    fn serie_acotada_y_con_ritmo() {
        let mut s = Serie::default();
        // Lo que da restic, tal cual: lectura y subida medidas, archivos por segundo.
        let mut a = act("a", "copia", Some(10), Some(500));
        a.lectura = Some(400);
        a.subida = Some(120);
        a.archivos_s = Some(7);
        s.muestra(1000, &[a]);
        assert_eq!(s.puntos.back().unwrap(), &(1000, 400, 120, 7, "copia".to_string()));
        // Sin lectura medida, la de restic.
        s.muestra(1001, &[act("a", "copia", Some(10), Some(500))]);
        assert_eq!(s.puntos.back().unwrap().1, 500);
        // Sin ritmos: la diferencia de bytes entre muestras, como escritura (restaurar)...
        let mut s = Serie::default();
        s.muestra(1000, &[act("b", "restauracion", Some(1_000), None)]);
        assert_eq!(s.puntos.back().unwrap().2, 0, "la primera muestra no sabe el ritmo");
        s.muestra(1002, &[act("b", "restauracion", Some(5_000), None)]);
        assert_eq!(s.puntos.back().unwrap(), &(1002, 0, 2_000, 0, "restauracion".to_string()));
        // ... o como lectura (verificar).
        let mut v = Serie::default();
        v.muestra(10, &[act("v", "verificacion", Some(0), None)]);
        v.muestra(12, &[act("v", "verificacion", Some(800), None)]);
        assert_eq!(v.puntos.back().unwrap(), &(12, 400, 0, 0, "verificacion".to_string()));
        // Nada en marcha: baja a cero una vez y ya no se añade más.
        s.muestra(1004, &[]);
        assert_eq!(s.puntos.back().unwrap(), &(1004, 0, 0, 0, "restauracion".to_string()));
        s.muestra(1006, &[]);
        assert_eq!(s.puntos.len(), 3);
        // Una hora de muestras cada 2 s: nunca más de 150 ni de hace más de 5 minutos.
        let mut s = Serie::default();
        for i in 0..1800 {
            s.muestra(i * 2, &[act("c", "copia", None, Some(i as u64))]);
            assert!(s.puntos.len() <= SERIE_MAX);
        }
        assert!(s.puntos.front().unwrap().0 >= 3598 - SERIE_SEGUNDOS);
        assert_eq!(s.puntos.back().unwrap().0, 3598);
        // Dos a la vez: se suman y manda la más rápida.
        let mut s = Serie::default();
        s.muestra(10, &[act("d", "verificacion", None, Some(100)), act("e", "copia", None, Some(900))]);
        assert_eq!(s.puntos.back().unwrap(), &(10, 1000, 0, 0, "copia".to_string()));
        // En el mismo segundo, la última manda.
        s.muestra(10, &[act("e", "copia", None, Some(50))]);
        assert_eq!(s.puntos.len(), 1);
        // Se recorta también sin muestras nuevas.
        s.recortar(10 + SERIE_SEGUNDOS + 1);
        assert!(s.puntos.is_empty());
    }

    fn hecha(clave: &str, tipo: &str, resultado: &str, cuando: &str) -> Hecha {
        Hecha {
            clave: clave.into(),
            tipo: tipo.into(),
            nombre: "Documentos".into(),
            resultado: resultado.into(),
            cuando: cuando.into(),
            bytes: Some(1_500_000),
        }
    }

    #[test]
    fn que_se_avisa_segun_el_nivel() {
        let a = act("copia:r#k@1", "copia", None, None);
        let antes = vec![hecha("copia:r#k", "copia", "ok", "1")];
        // Empieza una copia: solo con «todo».
        assert!(avisos(&[], &antes, std::slice::from_ref(&a), &antes, Avisos::Errores, false).is_empty());
        let v = avisos(&[], &antes, std::slice::from_ref(&a), &antes, Avisos::Todo, false);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].titulo, "Copiando «Documentos»");
        assert_eq!((v[0].grupo, v[0].etiqueta.as_str()), ("copias", "copia:x"));
        // La misma lectura otra vez: nada.
        assert!(avisos(std::slice::from_ref(&a), &antes, std::slice::from_ref(&a), &antes, Avisos::Todo, false).is_empty());
        // Termina bien: solo con «todo», con lo nuevo.
        let bien = vec![hecha("copia:r#k", "copia", "ok", "2")];
        assert!(avisos(&[], &antes, &[], &bien, Avisos::Errores, false).is_empty());
        let v = avisos(&[], &antes, &[], &bien, Avisos::Todo, false);
        assert_eq!(v[0].titulo, "Copia «Documentos» terminada");
        assert_eq!(v[0].texto, "Correcta · 1,4 MB nuevos");
        // Falla: con «errores» y con «todo»; no con «off».
        let mal = vec![hecha("copia:r#k", "copia", "error", "3")];
        for nivel in [Avisos::Errores, Avisos::Todo] {
            let v = avisos(&[], &bien, &[], &mal, nivel, false);
            assert_eq!(v.len(), 1);
            assert_eq!(v[0].titulo, "Falló la copia «Documentos»");
            assert!(v[0].error);
        }
        assert!(avisos(&[], &bien, &[], &mal, Avisos::Off, false).is_empty());
        // Vuelve a fallar: no se repite.
        assert!(avisos(&[], &mal, &[], &[hecha("copia:r#k", "copia", "error", "4")], Avisos::Todo, false).is_empty());
        // Se recupera.
        let v = avisos(&[], &mal, &[], &[hecha("copia:r#k", "copia", "ok", "5")], Avisos::Errores, false);
        assert_eq!(v[0].titulo, "La copia «Documentos» vuelve a funcionar");
        assert!(!v[0].error);
        // Otros tipos.
        let v = avisos(&[], &[], &[], &[hecha("restauracion:x", "restauracion", "error", "1")], Avisos::Errores, false);
        assert_eq!(v[0].titulo, "Falló la restauración «Documentos»");
        assert_eq!(v[0].grupo, "restauraciones");
        let v = avisos(&[], &[], &[], &[hecha("verify:r", "verificacion", "ok", "1")], Avisos::Todo, false);
        assert_eq!(v[0].titulo, "«Documentos» verificado: todo en orden");
        let v = avisos(&[], &[], &[], &[hecha("offsite:r", "copia_externa", "warning", "1")], Avisos::Todo, false);
        assert_eq!(v[0].titulo, "La copia externa «Documentos» terminó con avisos");
        let v = avisos(&[], &[], &[], &[hecha("nube:n", "nube", "ok", "1")], Avisos::Todo, false);
        assert_eq!(v[0].titulo, "Subida a «Documentos» terminada");
        // Lo primero que lee la bandeja al arrancar no avisa.
        assert!(avisos(&[], &[], std::slice::from_ref(&a), &mal, Avisos::Todo, true).is_empty());
    }

    #[test]
    fn la_ventana_se_abre_sola_una_vez_por_tarea() {
        let mut vistas = HashSet::new();
        let a = act("copia:r#k@1", "copia", None, None);
        // Al arrancar la bandeja con una copia ya en marcha: no (ni se abre luego por ella).
        assert!(!abrir_al_empezar(&mut vistas, std::slice::from_ref(&a), Ventana::AlTrabajar, true));
        assert!(!abrir_al_empezar(&mut vistas, std::slice::from_ref(&a), Ventana::AlTrabajar, false));
        // Termina y empieza otra: sí, una vez.
        assert!(!abrir_al_empezar(&mut vistas, &[], Ventana::AlTrabajar, false));
        assert!(vistas.is_empty(), "no crece sin fin");
        let b = act("copia:r#k@2", "copia", None, None);
        assert!(abrir_al_empezar(&mut vistas, std::slice::from_ref(&b), Ventana::AlTrabajar, false));
        assert!(!abrir_al_empezar(&mut vistas, std::slice::from_ref(&b), Ventana::AlTrabajar, false));
        // Con otros ajustes, nunca sola.
        let c = act("verify:r@3", "verificacion", None, None);
        assert!(!abrir_al_empezar(&mut vistas, std::slice::from_ref(&c), Ventana::SiempreDisponible, false));
        let d = act("verify:r@4", "verificacion", None, None);
        assert!(!abrir_al_empezar(&mut vistas, std::slice::from_ref(&d), Ventana::Off, false));
    }

    #[test]
    fn registro_de_lo_que_corre_en_el_servicio() {
        let g = en_marcha::empezar("restauracion", "prueba-registro", "Servidor");
        g.progreso(Some(50), Some(200));
        let a = en_marcha::actividades().into_iter().find(|a| a.clave == "restauracion:prueba-registro").expect("en marcha");
        assert_eq!((a.tipo.as_str(), a.porcentaje), ("restauracion", Some(0.25)));
        g.terminar("ok");
        assert!(!en_marcha::actividades().iter().any(|a| a.clave == "restauracion:prueba-registro"));
        let h = en_marcha::hechas().into_iter().find(|h| h.clave == "restauracion:prueba-registro").expect("hecha");
        assert_eq!(h.resultado, "ok");
        // Soltada sin terminar (un error con `?`): fallida.
        {
            let _g = en_marcha::empezar("espejo", "prueba-registro-2", "Disco");
        }
        assert_eq!(en_marcha::hechas().into_iter().find(|h| h.clave == "espejo:prueba-registro-2").unwrap().resultado, "error");
    }

    #[test]
    fn historial_de_catorce_dias() {
        let e =
            |cuando: &str, r: &str, kind: &str| crate::history::Entry { kind: kind.into(), result: r.into(), finished: cuando.into(), ..Default::default() };
        let hoy = chrono::NaiveDate::from_ymd_opt(2026, 10, 5).unwrap();
        let h = historial(
            &[
                e("2026-10-05T10:00:00+02:00", "ok", "backup"),
                e("2026-10-05T12:00:00+02:00", "error", "backup"),
                e("2026-10-04T12:00:00+02:00", "warning", "backup"),
                e("2026-10-04T12:00:00+02:00", "ok", "verify"),
                e("2026-09-01T12:00:00+02:00", "ok", "backup"),
            ],
            hoy,
        );
        assert_eq!(h.len(), 14);
        assert_eq!(h[13], Dia { dia: "2026-10-05".into(), ok: 1, aviso: 0, fallo: 1 });
        assert_eq!(h[12], Dia { dia: "2026-10-04".into(), ok: 0, aviso: 1, fallo: 0 });
        assert_eq!(h[0].dia, "2026-09-22");
    }

    #[test]
    fn muestrear_a_menudo_solo_si_hace_falta() {
        let todo_off = Escritorio { ventana: Ventana::Off, avisos: Avisos::Off };
        assert!(!muestreo_rapido(todo_off, true), "con todo apagado, el servicio como antes");
        assert!(!muestreo_rapido(Escritorio::default(), false));
        assert!(muestreo_rapido(Escritorio::default(), true));
        assert!(muestreo_rapido(Escritorio { ventana: Ventana::Off, avisos: Avisos::Todo }, true));
        assert!(!muestreo_rapido(Escritorio { ventana: Ventana::Off, avisos: Avisos::Errores }, true));
    }

    #[test]
    fn avisos_en_una_presentacion() {
        for estado in [1, 5, 6, 7] {
            assert!(pasa_aviso(estado, false));
        }
        for estado in [2, 3, 4] {
            assert!(!pasa_aviso(estado, false), "{estado}");
            assert!(pasa_aviso(estado, true), "los errores, siempre");
        }
    }

    #[test]
    fn png_del_icono() {
        let rgba = crate::bandeja::pintar(crate::bandeja::Variante { nivel: crate::bandeja::Nivel::AlDia, octavos: None }, 64);
        let p = png(&rgba, 64);
        assert_eq!(&p[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        assert_eq!(&p[12..16], b"IHDR");
        assert_eq!(&p[p.len() - 8..p.len() - 4], b"IEND");
        // 64 filas de 257 bytes (con el byte de filtro), sin comprimir.
        assert!(p.len() > 64 * 257);
        // CRC del IEND, conocido.
        assert_eq!(&p[p.len() - 4..], &[0xae, 0x42, 0x60, 0x82]);
    }

    #[test]
    fn actividad_de_una_copia_sin_rutas() {
        let ahora = chrono::Local::now();
        let r = crate::agent::RunningCopy {
            repo_id: "r1".into(),
            plan_id: Some("k1".into()),
            started: ahora.to_rfc3339(),
            percent: Some(0.5),
            files_done: 10,
            total_files: 20,
            bytes_done: 100,
            total_bytes: 200,
            seconds_remaining: Some(30),
            updated: Some(ahora.to_rfc3339()),
            phase: None,
            bytes_per_s: Some(1234),
            read_bps: Some(1000),
            upload_bps: Some(300),
            files_per_s: Some(4),
        };
        let a = actividad_copia(&r, "Documentos", ahora).unwrap();
        assert_eq!((a.tipo.as_str(), a.clave.as_str(), a.fase.as_str()), ("copia", "copia:r1#k1", "subiendo"));
        assert_eq!((a.porcentaje, a.velocidad, a.quedan_s), (Some(0.5), Some(1234), Some(30)));
        assert_eq!((a.lectura, a.subida, a.archivos_s), (Some(1000), Some(300), Some(4)));
        let t = crate::tasks::RunningTask {
            repo_id: "r1".into(),
            kind: "offsite".into(),
            started: "x".into(),
            stage: r"C:\secreto".into(),
            percent: Some(0.2),
            ..Default::default()
        };
        let a = actividad_tarea(&t, "Servidor");
        assert_eq!((a.tipo.as_str(), a.fase.as_str()), ("copia_externa", "en_marcha"));
        assert!(!serde_json::to_string(&a).unwrap().contains("secreto"));
    }
}
