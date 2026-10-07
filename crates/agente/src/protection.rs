//! Salud de la protección de un destino: una lista de comprobaciones (copias
//! al día, protección contra borrado, copia externa, verificación, prueba de
//! restauración, kit de recuperación y retención) con su estado.
//!
//! Las reglas viven solo aquí: la app y el informe a la web reúnen los datos
//! (`Facts`) cada uno con lo que tiene a mano y llaman a `evaluate`.

use chrono::{DateTime, Duration, Local};
use serde::{Deserialize, Serialize};

/// Resultado de una tarea (copia, subida, verificación, prueba).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RunFact {
    pub ok: bool,
    /// RFC 3339.
    pub finished: String,
    pub message: String,
}

impl RunFact {
    pub fn from_record(r: &crate::agent::RunRecord) -> Self {
        Self { ok: r.result != "error", finished: r.finished.clone(), message: r.message.clone() }
    }
}

/// Lo que se sabe de un destino.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    /// "local", "rest", "sftp", "s3", "b2", "azure", "gs"…
    pub kind: String,
    /// Destinos que suben aquí su copia externa (nombres).
    pub offsite_sources: Vec<String>,
    /// Está en el agente (copias automáticas o «solo vigilar»).
    pub scheduled: bool,
    pub monitor: bool,
    pub plans: usize,
    pub paused: bool,
    /// Últimas copias automáticas de sus copias (una por copia).
    pub backups: Vec<RunFact>,
    /// REST: ¿servidor de solo añadir? (`None`: sin comprobar).
    pub append_only: Option<bool>,
    /// Nube: el usuario declaró que el bucket tiene bloqueo de objetos.
    pub object_lock: bool,
    pub has_offsite: bool,
    pub offsite_run: Option<RunFact>,
    pub offsite_held: bool,
    /// RFC 3339 de la versión más reciente del destino.
    pub last_snapshot: Option<String>,
    pub has_verify: bool,
    pub verify_run: Option<RunFact>,
    pub has_cloud_verify: bool,
    pub cloud_verify_run: Option<RunFact>,
    pub has_restore_test: bool,
    pub restore_test_run: Option<RunFact>,
    /// Kit guardado y vigente / guardado pero ya no vale (cambió la ubicación).
    pub kit_ok: bool,
    pub kit_stale: bool,
    pub has_retention: bool,
    /// v1.41: un destino local, qué disco es (unidad, extraíble, de la red). `None`: no es local.
    pub disk: Option<crate::espacio::Disco>,
    /// Tarea 8: por dónde pasan los datos de una copia (la regla 3-2-1-1-0). Quien
    /// la ve entera (la consola, con todos los equipos) la arma; `None`: no se evalúa.
    pub regla: Option<EntradaRegla>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Ok,
    Warn,
    Bad,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    /// "copias", "borrado", "externa", "verificacion", "restauracion", "kit", "retencion".
    pub id: String,
    pub state: State,
    /// Título corto.
    pub label: String,
    /// Una línea que explica el estado.
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Protection {
    /// Comprobaciones en verde.
    pub score: usize,
    pub total: usize,
    pub items: Vec<Item>,
    /// Tarea 8: la regla 3-2-1-1-0, si los `Facts` traen su entrada. Va aparte de
    /// `items` (no cambia la puntuación ni la posición de las comprobaciones).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regla: Option<Regla321>,
}

fn item(id: &str, state: State, label: &str, detail: impl Into<String>) -> Item {
    Item { id: id.into(), state, label: label.into(), detail: detail.into() }
}

fn parse(t: &str) -> Option<DateTime<Local>> {
    DateTime::parse_from_rfc3339(t).ok().map(|d| d.with_timezone(&Local))
}

fn ago(t: &str, now: DateTime<Local>) -> String {
    let Some(t) = parse(t) else { return String::new() };
    let d = now.signed_duration_since(t);
    if d < Duration::hours(1) {
        "hace un momento".into()
    } else if d < Duration::hours(36) {
        format!("hace {} h", d.num_hours())
    } else {
        format!("hace {} días", d.num_days())
    }
}

/// Nombres en una frase: «X», «X y Y».
fn names(list: &[String]) -> String {
    match list {
        [] => String::new(),
        [one] => format!("«{one}»"),
        _ => {
            let (last, rest) = list.split_last().unwrap();
            format!("{} y «{last}»", rest.iter().map(|n| format!("«{n}»")).collect::<Vec<_>>().join(", "))
        }
    }
}

pub fn evaluate(f: &Facts, now: DateTime<Local>) -> Protection {
    let cloud = matches!(f.kind.as_str(), "s3" | "b2" | "azure" | "gs");
    let target = !f.offsite_sources.is_empty() && f.plans == 0;
    let mut items = Vec::new();

    // 1. Copias automáticas activas y al día.
    items.push(if target {
        item("copias", State::Ok, "Copias", format!("Recibe la copia externa de {}.", names(&f.offsite_sources)))
    } else if !f.scheduled {
        item("copias", State::Bad, "Copias automáticas", "Sin copias automáticas: solo se copia cuando alguien lo hace a mano.")
    } else if f.monitor {
        item("copias", State::Ok, "Copias automáticas", "«Solo vigilar»: las hace otro programa y Resguardo las revisa.")
    } else if f.backups.iter().any(|b| !b.ok) {
        item("copias", State::Bad, "Copias automáticas", "La última copia automática de alguna copia falló.")
    } else if f.paused {
        item("copias", State::Warn, "Copias automáticas", "En pausa.")
    } else if f.backups.is_empty() {
        item("copias", State::Warn, "Copias automáticas", "Programadas, todavía sin ninguna hecha.")
    } else {
        item("copias", State::Ok, "Copias automáticas", "Activas y al día.")
    });

    // 2. Protección contra borrado (un ransomware no puede borrar las versiones).
    items.push(match f.kind.as_str() {
        "rest" => match f.append_only {
            Some(true) => item("borrado", State::Ok, "Protegida contra borrado", "El servidor es de solo añadir: desde este equipo no se puede borrar nada."),
            Some(false) => item(
                "borrado",
                State::Warn,
                "Protegida contra borrado",
                "El servidor permite borrar: arranca rest-server con --append-only para que nadie pueda borrar las versiones desde aquí.",
            ),
            None => item("borrado", State::Unknown, "Protegida contra borrado", "Sin comprobar: no se pudo consultar al servidor."),
        },
        _ if cloud && f.object_lock => item("borrado", State::Ok, "Protegida contra borrado", "El bucket tiene bloqueo de objetos (Object Lock)."),
        _ if cloud => item(
            "borrado",
            State::Warn,
            "Protegida contra borrado",
            "Activa el bloqueo de objetos (Object Lock) en el bucket y márcalo aquí para que nadie pueda borrar las versiones.",
        ),
        _ => item(
            "borrado",
            State::Warn,
            "Protegida contra borrado",
            "Un ransomware podría borrarla; usa un servidor de solo añadir o una copia externa con bloqueo.",
        ),
    });

    // 3. Copia externa configurada y al día.
    items.push(if target {
        item("externa", State::Ok, "Copia externa", format!("Este repositorio es la copia externa de {}.", names(&f.offsite_sources)))
    } else if !f.has_offsite {
        item("externa", State::Bad, "Copia externa", "Todas las versiones están en un solo sitio: configura una copia externa (la nube u otro disco).")
    } else if f.offsite_held {
        item("externa", State::Bad, "Copia externa", "Subida frenada por un cambio inusual: revísalo.")
    } else {
        match &f.offsite_run {
            None => item("externa", State::Warn, "Copia externa", "Configurada, todavía sin ninguna subida."),
            Some(r) if !r.ok => item("externa", State::Bad, "Copia externa", format!("La última subida falló: {}", r.message)),
            Some(r) => {
                let behind = match (parse(&r.finished), f.last_snapshot.as_deref().and_then(parse)) {
                    (Some(up), Some(snap)) => snap - up > Duration::hours(24),
                    _ => false,
                };
                if behind {
                    item(
                        "externa",
                        State::Warn,
                        "Copia externa",
                        format!("Atrasada: la última subida fue {} y hay versiones más nuevas.", ago(&r.finished, now)),
                    )
                } else {
                    item("externa", State::Ok, "Copia externa", format!("Al día (última subida {}).", ago(&r.finished, now)))
                }
            }
        }
    });

    // 4. Verificación (del destino y, si la hay, de su copia en la nube).
    let (has_v, run_v) = if target { (f.has_cloud_verify, f.cloud_verify_run.clone()) } else { (f.has_verify, f.verify_run.clone()) };
    items.push(if !has_v {
        item(
            "verificacion",
            State::Warn,
            "Verificación",
            if target { "Sin verificación: prográmala en el origen, en «Copia externa»." } else { "Sin verificación programada." },
        )
    } else if run_v.as_ref().is_some_and(|r| !r.ok) || (!target && f.cloud_verify_run.as_ref().is_some_and(|r| !r.ok)) {
        item("verificacion", State::Bad, "Verificación", "La última verificación encontró errores o no se pudo hacer.")
    } else if let Some(r) = run_v {
        let cloud_note = if !target && f.has_cloud_verify { " (también la copia en la nube)" } else { "" };
        item("verificacion", State::Ok, "Verificación", format!("Sin errores{cloud_note}, {}.", ago(&r.finished, now)))
    } else {
        item("verificacion", State::Warn, "Verificación", "Programada, todavía sin ninguna hecha.")
    });

    // 5. Prueba de restauración (en un destino que solo recibe una copia externa, no aplica).
    if !target {
        items.push(if !f.has_restore_test {
            item("restauracion", State::Warn, "Prueba de restauración", "Sin prueba: nadie comprueba que las copias se puedan recuperar de verdad.")
        } else {
            match &f.restore_test_run {
                None => item("restauracion", State::Warn, "Prueba de restauración", "Programada, todavía sin ninguna hecha."),
                Some(r) if !r.ok => item("restauracion", State::Bad, "Prueba de restauración", format!("La última falló: {}", r.message)),
                Some(r) if parse(&r.finished).is_some_and(|t| now - t > Duration::days(45)) => {
                    item("restauracion", State::Warn, "Prueba de restauración", format!("La última correcta fue {}.", ago(&r.finished, now)))
                }
                Some(r) => item("restauracion", State::Ok, "Prueba de restauración", format!("Correcta, {}.", ago(&r.finished, now))),
            }
        });
    }

    // 6. Kit de recuperación.
    items.push(if f.kit_ok {
        item("kit", State::Ok, "Kit de recuperación", "Guardado.")
    } else if f.kit_stale {
        item("kit", State::Warn, "Kit de recuperación", "La ubicación cambió desde el último kit: guarda uno nuevo.")
    } else {
        item("kit", State::Bad, "Kit de recuperación", "Sin kit: si pierdes este equipo no podrás abrir las copias.")
    });

    // 7. Retención.
    items.push(if !f.has_retention {
        item("retencion", State::Warn, "Retención", "Sin política: el repositorio crece sin fin.")
    } else if f.kind == "rest" && f.append_only == Some(true) {
        item("retencion", State::Ok, "Retención", "Configurada; se aplica en el servidor.")
    } else {
        item("retencion", State::Ok, "Retención", "Configurada.")
    });

    // 8. v1.41: fuera del equipo que protege (solo un destino local; los demás ya lo están).
    // Al final, para no mover las anteriores: quien lea `items` por posición sigue igual.
    if f.kind == "local" && !target {
        let disk = f.disk.clone().unwrap_or_default();
        let unidad = disk.unidad.as_deref().map(|u| format!(" ({u})")).unwrap_or_default();
        items.push(if disk.red {
            item("lugar", State::Ok, "Fuera de este equipo", "En una carpeta de otra máquina de la red.")
        } else if disk.extraible == Some(true) {
            item("lugar", State::Ok, "Fuera de este equipo", format!("En un disco extraíble{unidad}: guárdalo lejos del equipo cuando no copie."))
        } else if f.has_offsite {
            item("lugar", State::Ok, "Fuera de este equipo", format!("En este mismo equipo{unidad}, pero con copia externa."))
        } else {
            item(
                "lugar",
                State::Warn,
                "Fuera de este equipo",
                format!(
                    "En este mismo equipo{unidad}: si se daña o lo cifra un ransomware, se pierden los archivos y las copias. Guárdalas en un almacén de otro equipo o añade una copia externa."
                ),
            )
        });
    }

    Protection { score: items.iter().filter(|i| i.state == State::Ok).count(), total: items.len(), items, regla: f.regla.as_ref().map(|e| regla_321(e, now)) }
}

// ---------- Regla 3-2-1-1-0 (tarea 8, docs/regla-3-2-1.md) ----------
//
// Por copia: 3 copias de los datos (contando los originales), en 2 soportes
// distintos, 1 fuera de la oficina, 1 inmutable o fuera del alcance de los
// equipos y 0 errores al verificar y probar la restauración. Solo cuentan los
// destinos al día. Es una guía: nada se bloquea por no cumplirla, y nunca
// cuenta el sistema operativo ni el sistema de archivos (8e).
//
// La consola arma la entrada (ve todos los equipos, el espejo del almacén y el
// catálogo de destinos) y la evalúa con `consola/src/lib/regla321.ts`; las dos
// pasan los vectores de `crates/protocolo/vectors/regla-321.json`.

/// Dónde está un destino.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lugar {
    #[default]
    EsteEquipo,
    /// Otro equipo de la oficina.
    Oficina,
    OtraSede,
    Nube,
}

/// 0.7.26 (bloque 2): el tipo de un destino, uno solo. `Local` (este equipo u otro
/// de la oficina), `Fuera` (fuera del sitio: otra sede, un servidor de fuera) o `Nube`.
/// Sin él se deduce de `lugar` (`este_equipo`/`oficina` → local, `otra_sede` → fuera, `nube` → nube).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoDestino {
    Local,
    Fuera,
    Nube,
}

impl TipoDestino {
    /// El tipo que corresponde a un `lugar` de antes.
    pub fn de_lugar(l: Lugar) -> Self {
        match l {
            Lugar::EsteEquipo | Lugar::Oficina => Self::Local,
            Lugar::OtraSede => Self::Fuera,
            Lugar::Nube => Self::Nube,
        }
    }
}

/// Días sin conectarse tras los que un medio aislado avisa (si no se dice otra cosa).
pub const DIAS_AISLADO: u32 = 30;

/// Si un destino es inmutable (o está fuera del alcance de los equipos).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Inmutable {
    /// Rest-server de solo añadir: desde el equipo no se puede borrar.
    SoloAnadir,
    /// Bloqueo de objetos (Object Lock) en la nube.
    ObjectLock,
    /// Instantáneas inmutables fuera de su alcance (las hace el anfitrión; lo dice la persona).
    Instantaneas,
    /// Un disco que se desconecta (y se rota); lo dice la persona.
    Desconectado,
    #[default]
    No,
}

/// Los originales (las carpetas del equipo): cuentan como una copia.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrigenRegla {
    pub equipo: String,
    /// Equipo + disco (`equipo:<id>:origen`).
    pub soporte: String,
}

/// Un destino al que llegan los datos de la copia (un paso del camino o de la cadena).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PasoRegla {
    pub id: String,
    #[serde(default)]
    pub nombre: String,
    /// `copia`, `espejo`, `externa`, `derivada` o `paso` (parte B de la tarea 7): solo informa.
    #[serde(default)]
    pub tipo: String,
    #[serde(default)]
    pub lugar: Lugar,
    #[serde(default)]
    pub inmutable: Inmutable,
    pub soporte: String,
    /// El equipo que lo guarda (sin él: la nube o un servidor de fuera).
    #[serde(default)]
    pub equipo: Option<String>,
    /// RFC 3339: la última vez que se puso al día bien.
    #[serde(default)]
    pub ultima_ok: Option<String>,
    /// Cada cuánto le toca (sin él, cada día).
    #[serde(default)]
    pub cada_horas: Option<f64>,
    /// Su comprobación encontró datos dañados (el espejo verifica sin contraseñas).
    #[serde(default)]
    pub verificacion_mal: bool,
    /// 0.7.26: el tipo (local, fuera, nube). Sin él, el de `lugar`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tipo_destino: Option<TipoDestino>,
    /// 0.7.26: marca «Aislado» (un medio que se desconecta y se rota). `inmutable:
    /// "desconectado"` de antes se lee igual.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub aislado: bool,
    /// 0.7.26: la última vez que el agente vio el medio conectado (RFC 3339). Sin
    /// ella no se sabe (un agente anterior): no avisa.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conectado: Option<String>,
    /// 0.7.26: días sin conectarse tras los que avisa (sin él, 30).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aislado_dias: Option<u32>,
}

impl PasoRegla {
    /// ¿Cuenta como fuera del sitio? (Fuera del sitio o Nube.)
    pub fn es_fuera(&self) -> bool {
        self.tipo_destino.unwrap_or_else(|| TipoDestino::de_lugar(self.lugar)) != TipoDestino::Local
    }
    /// ¿Marca «Inmutable»? (solo añadir, bloqueo de objetos, instantáneas fuera de su alcance).
    pub fn es_inmutable(&self) -> bool {
        !matches!(self.inmutable, Inmutable::No | Inmutable::Desconectado)
    }
    /// ¿Marca «Aislado»?
    pub fn es_aislado(&self) -> bool {
        self.aislado || self.inmutable == Inmutable::Desconectado
    }
    /// Los días sin conectarse que se aceptan (1 a 365; sin él, 30).
    pub fn dias_aislado(&self) -> u32 {
        self.aislado_dias.filter(|d| (1..=365).contains(d)).unwrap_or(DIAS_AISLADO)
    }
}

/// La verificación o la prueba de restauración del repositorio de la copia.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PruebaRegla {
    pub configurada: bool,
    #[serde(default)]
    pub ultima_ok: Option<String>,
    /// La última falló.
    #[serde(default)]
    pub fallo: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EntradaRegla {
    pub origen: OrigenRegla,
    #[serde(default)]
    pub pasos: Vec<PasoRegla>,
    #[serde(default)]
    pub verificacion: PruebaRegla,
    #[serde(default)]
    pub prueba_restauracion: PruebaRegla,
}

/// Una parte de la regla: `copias` (3), `soportes` (2), `fuera` (1), `inmutable` (1), `errores` (0).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParteRegla {
    pub id: String,
    pub meta: u32,
    /// Lo que hay con los destinos al día (en `errores`, los problemas).
    pub valor: u32,
    /// Lo que habría si todos estuvieran al día y lo programado hubiera salido bien.
    pub valor_config: u32,
    pub cumple: bool,
    pub cumple_config: bool,
    /// Qué hacer (código; vacío si cumple): `anadir_destino`, `poner_al_dia`, `otro_soporte`,
    /// `anadir_fuera`, `anadir_inmutable`, `programar_verificacion`, `programar_prueba`,
    /// `revisar_verificacion`, `revisar_prueba`, `revisar_destino`.
    pub accion: String,
    pub detalle: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Regla321 {
    pub cumple: bool,
    pub cumple_config: bool,
    /// La configuración cumple pero hoy algo no está al día o falló (un aviso no urgente).
    pub dejo_de_cumplir: bool,
    pub partes: Vec<ParteRegla>,
    /// Los pasos que no están al día.
    pub atrasados: Vec<String>,
    /// `mismo_equipo` (dos soportes en el mismo equipo: un incendio se los lleva a la vez),
    /// `inmutable_local` (lo inmutable o aislado está todo en el sitio) y
    /// `aislado_sin_conectar` (un medio aislado no se ha conectado en sus N días).
    pub avisos: Vec<String>,
}

/// Días como mucho desde la última verificación o prueba correcta.
pub const DIAS_PRUEBA_REGLA: i64 = 45;

/// Horas que puede pasar un paso sin ponerse al día: su horario y la mitad, más 12 h.
pub fn margen_horas(cada_horas: Option<f64>) -> f64 {
    let c = cada_horas.filter(|c| c.is_finite() && *c > 0.0).unwrap_or(24.0);
    c * 1.5 + 12.0
}

/// ¿Está al día este paso? (su última vez bien, dentro de su horario más un margen).
/// Un medio aislado se conecta de vez en cuando: le basta con sus N días.
pub fn paso_al_dia(p: &PasoRegla, ahora: DateTime<Local>) -> bool {
    let Some(t) = p.ultima_ok.as_deref().and_then(parse) else { return false };
    let mut margen = margen_horas(p.cada_horas);
    if p.es_aislado() {
        margen = margen.max(f64::from(p.dias_aislado()) * 24.0);
    }
    (ahora - t).num_seconds() as f64 / 3600.0 <= margen
}

/// ¿Un medio aislado lleva más de sus N días sin conectarse? Sin fecha no se sabe: no.
pub fn aislado_sin_conectar(p: &PasoRegla, ahora: DateTime<Local>) -> bool {
    p.es_aislado() && p.conectado.as_deref().and_then(parse).is_some_and(|t| ahora - t > Duration::days(i64::from(p.dias_aislado())))
}

/// ¿Correcta y reciente (45 días)?
fn prueba_bien(p: &PruebaRegla, ahora: DateTime<Local>) -> bool {
    p.configurada && !p.fallo && p.ultima_ok.as_deref().and_then(parse).is_some_and(|t| ahora - t <= Duration::days(DIAS_PRUEBA_REGLA))
}

fn parte(id: &str, meta: u32, valor: u32, valor_config: u32, accion_falta: &str, al_dia: &str, detalle: String) -> ParteRegla {
    let (cumple, cumple_config) = if id == "errores" { (valor == 0, valor_config == 0) } else { (valor >= meta, valor_config >= meta) };
    let accion = if cumple {
        ""
    } else if cumple_config {
        al_dia
    } else {
        accion_falta
    };
    ParteRegla { id: id.into(), meta, valor, valor_config, cumple, cumple_config, accion: accion.into(), detalle }
}

pub fn regla_321(e: &EntradaRegla, ahora: DateTime<Local>) -> Regla321 {
    use std::collections::{BTreeMap, BTreeSet};
    let al_dia: Vec<&PasoRegla> = e.pasos.iter().filter(|p| paso_al_dia(p, ahora)).collect();
    let todos: Vec<&PasoRegla> = e.pasos.iter().collect();
    let atrasados: Vec<String> = e.pasos.iter().filter(|p| !paso_al_dia(p, ahora)).map(|p| p.id.clone()).collect();
    let soportes = |l: &[&PasoRegla]| {
        let mut s: BTreeSet<&str> = l.iter().map(|p| p.soporte.as_str()).collect();
        s.insert(e.origen.soporte.as_str());
        s.len() as u32
    };
    // El «1 fuera»: Fuera del sitio y Nube. El otro «1»: Inmutable o Aislado.
    let fuera = |l: &[&PasoRegla]| l.iter().filter(|p| p.es_fuera()).count() as u32;
    let inmutables = |l: &[&PasoRegla]| l.iter().filter(|p| p.es_inmutable() || p.es_aislado()).count() as u32;
    let n = |x: u32| if x == 1 { "1 destino".to_string() } else { format!("{x} destinos") };

    let (c, cc) = (1 + al_dia.len() as u32, 1 + todos.len() as u32);
    let (s, sc) = (soportes(&al_dia), soportes(&todos));
    let (f, fc) = (fuera(&al_dia), fuera(&todos));
    let (i, ic) = (inmutables(&al_dia), inmutables(&todos));

    // El «0»: lo que falla o falta al verificar y al probar la restauración.
    let mut problemas: Vec<&str> = Vec::new();
    let mut estructurales = 0u32;
    for (p, programar, revisar) in
        [(&e.verificacion, "programar_verificacion", "revisar_verificacion"), (&e.prueba_restauracion, "programar_prueba", "revisar_prueba")]
    {
        if !p.configurada {
            problemas.push(programar);
            estructurales += 1;
        } else if !prueba_bien(p, ahora) {
            problemas.push(revisar);
        }
    }
    if e.pasos.iter().any(|p| p.verificacion_mal) {
        problemas.push("revisar_destino");
    }
    // Lo que falta programar va antes que lo que hay que revisar.
    problemas.sort_by_key(|p| !p.starts_with("programar"));
    let primero = problemas.first().copied().unwrap_or_default();
    let errores = parte(
        "errores",
        0,
        problemas.len() as u32,
        estructurales,
        primero,
        primero,
        if problemas.is_empty() {
            "Verificación y prueba de restauración recientes y sin errores.".into()
        } else {
            format!("{} por resolver al verificar o probar la restauración.", problemas.len())
        },
    );

    let partes = vec![
        parte("copias", 3, c, cc, "anadir_destino", "poner_al_dia", format!("{c} de 3: los originales y {} al día.", n(c - 1))),
        parte("soportes", 2, s, sc, "otro_soporte", "poner_al_dia", format!("{s} de 2 soportes distintos (equipo y disco).")),
        parte("fuera", 1, f, fc, "anadir_fuera", "poner_al_dia", format!("{} fuera de la oficina.", n(f))),
        parte("inmutable", 1, i, ic, "anadir_inmutable", "poner_al_dia", format!("{} inmutable o fuera del alcance de los equipos.", n(i))),
        errores,
    ];
    let cumple = partes.iter().all(|p| p.cumple);
    let cumple_config = partes.iter().all(|p| p.cumple_config);

    // Avisos, con lo configurado (no cambian porque un paso vaya atrasado).
    let mut avisos = Vec::new();
    let mut por_equipo: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    por_equipo.entry(e.origen.equipo.as_str()).or_default().insert(e.origen.soporte.as_str());
    for p in &todos {
        if let Some(eq) = p.equipo.as_deref().filter(|x| !x.is_empty()) {
            por_equipo.entry(eq).or_default().insert(p.soporte.as_str());
        }
    }
    if por_equipo.values().any(|s| s.len() >= 2) {
        avisos.push("mismo_equipo".to_string());
    }
    if ic > 0 && !todos.iter().any(|p| (p.es_inmutable() || p.es_aislado()) && p.es_fuera()) {
        avisos.push("inmutable_local".to_string());
    }
    if todos.iter().any(|p| aislado_sin_conectar(p, ahora)) {
        avisos.push("aislado_sin_conectar".to_string());
    }
    Regla321 { cumple, cumple_config, dejo_de_cumplir: cumple_config && !cumple, partes, atrasados, avisos }
}

// ---------- Datos del agente ----------

/// Lo que sabe el agente de un destino (configuración, últimas ejecuciones,
/// tareas y freno). La app completa después lo suyo (retención, kit…).
pub fn agent_facts(
    repo_id: &str,
    location: &str,
    config: &crate::agent::AgentConfig,
    state: &crate::agent::AgentState,
    tasks: &crate::tasks::TasksState,
    guard: &crate::tasks::GuardState,
    now: DateTime<Local>,
) -> Facts {
    let entry = config.repos.iter().find(|r| r.id == repo_id);
    let task = |kind: &str| tasks.runs.get(&crate::tasks::key(kind, repo_id)).map(RunFact::from_record);
    // Si es destino de una copia externa, su verificación es la de la nube del origen.
    let sources: Vec<&crate::agent::AgentRepo> =
        config.repos.iter().filter(|r| r.offsite.as_ref().is_some_and(|o| o.provider == format!("destino:{repo_id}"))).collect();
    let cloud_verify_src = sources.iter().find(|s| s.offsite.as_ref().is_some_and(|o| o.verify.is_some()));
    let mut f = Facts {
        kind: crate::kit::kind_of(location).into(),
        offsite_sources: sources.iter().map(|s| s.name.clone()).collect(),
        scheduled: entry.is_some(),
        monitor: entry.is_some_and(|e| matches!(e.schedule, crate::agent::Schedule::Monitor { .. })),
        plans: entry.map_or(0, |e| e.plans.len()),
        paused: entry.is_some_and(|e| e.active_pause(now).is_some()),
        backups: entry
            .map(|e| e.plans.iter().filter_map(|p| state.runs.get(&crate::plans::plan_key(repo_id, &p.id))).map(RunFact::from_record).collect())
            .unwrap_or_default(),
        append_only: state.append_only.get(repo_id).and_then(|c| c.append_only),
        has_offsite: entry.is_some_and(|e| e.offsite.is_some()),
        offsite_run: task("offsite"),
        offsite_held: guard.holds.contains_key(repo_id),
        last_snapshot: state.web.snapshots.get(repo_id).and_then(|s| s.last_time.clone()),
        has_verify: entry.is_some_and(|e| e.verify.is_some()),
        verify_run: task("verify"),
        has_restore_test: entry.is_some_and(|e| e.restore_test.is_some()),
        restore_test_run: task("restore_test"),
        has_cloud_verify: entry.is_some_and(|e| e.offsite.as_ref().is_some_and(|o| o.verify.is_some())),
        cloud_verify_run: task("verify_offsite"),
        ..Default::default()
    };
    if f.kind == "local" {
        f.disk = Some(crate::espacio::disco_de(location));
    }
    if f.plans == 0 {
        if let Some(src) = cloud_verify_src {
            f.has_cloud_verify = true;
            f.cloud_verify_run = tasks.runs.get(&crate::tasks::key("verify_offsite", &src.id)).map(RunFact::from_record);
        }
    }
    if let Some(e) = entry {
        f.object_lock = e.object_lock;
        f.has_retention = e.has_retention;
        f.kit_ok = e.kit.as_ref().is_some_and(|k| k.matches(location, None));
        f.kit_stale = e.kit.is_some() && !f.kit_ok;
    }
    f
}

// ---------- ¿Servidor REST de solo añadir? ----------

/// Resultado de la comprobación (se guarda para no repetirla más de una vez al día).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppendOnlyCheck {
    pub checked_at: String,
    /// `None`: no se pudo saber.
    pub append_only: Option<bool>,
}

impl AppendOnlyCheck {
    /// Un resultado vale un día; «no se pudo saber», solo una hora (sin red, o
    /// un agente anterior que no usaba la autoridad propia del almacén).
    pub fn fresh(&self, now: DateTime<Local>) -> bool {
        let vale = if self.append_only.is_some() { Duration::hours(24) } else { Duration::hours(1) };
        parse(&self.checked_at).is_some_and(|t| now - t < vale)
    }
}

pub(crate) fn base64(input: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Dirección HTTP de un objeto inexistente del repositorio (`data/<64 hex al azar>`).
pub fn probe_url(location: &str) -> Option<String> {
    let rest = location.strip_prefix("rest:")?;
    if !(rest.starts_with("http://") || rest.starts_with("https://")) {
        return None;
    }
    // Sin `usuario:contraseña@` en la dirección (van en la cabecera).
    let (scheme, after) = rest.split_once("://")?;
    let (authority, path) = after.split_at(after.find('/').unwrap_or(after.len()));
    let host = authority.rsplit('@').next().unwrap_or(authority);
    let mut rng = crate::restore_test::Rng::from_clock();
    let id: String = (0..4).map(|_| format!("{:016x}", rng.next())).collect();
    Some(format!("{scheme}://{host}{}/data/{id}", path.trim_end_matches('/')))
}

/// ¿El rest-server está en modo `--append-only`? Pide borrar un objeto que no
/// existe (nombre al azar): en solo añadir responde 403 antes de mirar nada;
/// si no, 200 (borrar algo inexistente no es un error). Nunca se borra nada.
/// `None` si no se puede saber (sin red, credenciales…).
///
/// `cacert`: el archivo PEM con la autoridad propia del servidor (la de un
/// almacén, «Este equipo guarda copias», o el `ca_pem` de un destino), la
/// misma que restic usa con `--cacert`. Sin ella, las autoridades del sistema.
pub fn probe_append_only(location: &str, auth: Option<(&str, &str)>, cacert: Option<&str>) -> Option<bool> {
    let url = probe_url(location)?;
    let tls = match cacert {
        Some(path) => tls_con_autoridad(&std::fs::read(path).ok()?)?,
        None => crate::web::tls_sistema(),
    };
    let agent =
        ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(15))).http_status_as_error(false).tls_config(tls).build().new_agent();
    let mut req = agent.delete(&url);
    if let Some((user, pass)) = auth {
        req = req.header("Authorization", format!("Basic {}", base64(format!("{user}:{pass}").as_bytes())));
    }
    let status = req.call().ok()?.status().as_u16();
    estado_solo_anadir(status)
}

/// TLS que solo confía en las autoridades de un PEM (todas las que traiga).
pub(crate) fn tls_con_autoridad(pem: &[u8]) -> Option<ureq::tls::TlsConfig> {
    use ureq::tls::{PemItem, RootCerts, TlsConfig, TlsProvider};
    let certs: Vec<_> = ureq::tls::parse_pem(pem)
        .filter_map(|i| match i {
            Ok(PemItem::Certificate(c)) => Some(c),
            _ => None,
        })
        .collect();
    (!certs.is_empty()).then(|| TlsConfig::builder().provider(TlsProvider::Rustls).root_certs(RootCerts::Specific(std::sync::Arc::new(certs))).build())
}

/// La respuesta de rest-server al borrar un objeto que no existe.
fn estado_solo_anadir(status: u16) -> Option<bool> {
    match status {
        403 => Some(true),
        200..=299 | 404 => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Local> {
        parse("2026-10-01T12:00:00-05:00").unwrap()
    }

    fn ok(t: &str) -> Option<RunFact> {
        Some(RunFact { ok: true, finished: t.into(), message: String::new() })
    }

    fn completo() -> Facts {
        Facts {
            kind: "rest".into(),
            scheduled: true,
            plans: 2,
            backups: vec![ok("2026-10-01T11:00:00-05:00").unwrap()],
            append_only: Some(true),
            has_offsite: true,
            offsite_run: ok("2026-10-01T11:30:00-05:00"),
            last_snapshot: Some("2026-10-01T11:00:00-05:00".into()),
            has_verify: true,
            verify_run: ok("2026-09-28T03:00:00-05:00"),
            has_restore_test: true,
            restore_test_run: ok("2026-09-27T04:00:00-05:00"),
            kit_ok: true,
            has_retention: true,
            ..Default::default()
        }
    }

    #[test]
    fn todo_en_verde() {
        let p = evaluate(&completo(), now());
        assert_eq!((p.score, p.total), (7, 7), "{:#?}", p.items);
        assert_eq!(p.items.iter().find(|i| i.id == "retencion").unwrap().detail, "Configurada; se aplica en el servidor.");
    }

    #[test]
    fn lo_que_falta_se_ve() {
        let mut f = completo();
        f.kind = "local".into();
        f.has_offsite = false;
        f.kit_ok = false;
        f.restore_test_run = Some(RunFact { ok: false, finished: "2026-09-30T04:00:00-05:00".into(), message: "x".into() });
        let p = evaluate(&f, now());
        let state = |id: &str| p.items.iter().find(|i| i.id == id).unwrap().state.clone();
        assert_eq!(state("borrado"), State::Warn);
        assert_eq!(state("externa"), State::Bad);
        assert_eq!(state("kit"), State::Bad);
        assert_eq!(state("restauracion"), State::Bad);
        assert_eq!(p.score, 3);
        // Copia externa atrasada: hay versiones más de 24 h posteriores a la última subida.
        let mut f = completo();
        f.offsite_run = ok("2026-09-28T11:00:00-05:00");
        let p = evaluate(&f, now());
        assert_eq!(p.items.iter().find(|i| i.id == "externa").unwrap().state, State::Warn);
        // REST sin comprobar.
        let mut f = completo();
        f.append_only = None;
        assert_eq!(evaluate(&f, now()).items[1].state, State::Unknown);
    }

    #[test]
    fn copias_en_el_mismo_equipo() {
        let lugar = |f: &Facts| evaluate(f, now()).items.into_iter().find(|i| i.id == "lugar");
        // Fuera del equipo (un servidor, la nube): no hace falta decirlo.
        assert!(lugar(&completo()).is_none());
        assert_eq!(evaluate(&completo(), now()).total, 7);
        // En una carpeta del propio equipo, sin copia externa: aviso, con la unidad.
        let mut f = completo();
        f.kind = "local".into();
        f.has_offsite = false;
        f.disk = Some(crate::espacio::Disco { unidad: Some("D:".into()), extraible: Some(false), red: false });
        let i = lugar(&f).unwrap();
        assert_eq!(i.state, State::Warn);
        assert!(i.detail.starts_with("En este mismo equipo (D:)"), "{}", i.detail);
        assert_eq!(evaluate(&f, now()).total, 8);
        // Sin saber qué disco es (un agente que no pudo mirarlo): también aviso.
        f.disk = None;
        assert_eq!(lugar(&f).unwrap().state, State::Warn);
        // Un disco USB, una carpeta de la red o con copia externa: bien.
        f.disk = Some(crate::espacio::Disco { unidad: Some("E:".into()), extraible: Some(true), red: false });
        assert_eq!(lugar(&f).unwrap().state, State::Ok);
        f.disk = Some(crate::espacio::Disco { unidad: None, extraible: Some(false), red: true });
        assert_eq!(lugar(&f).unwrap().state, State::Ok);
        f.disk = None;
        f.has_offsite = true;
        assert!(lugar(&f).unwrap().detail.contains("con copia externa"));
    }

    #[test]
    fn destino_de_una_copia_externa() {
        let f = Facts {
            kind: "s3".into(),
            offsite_sources: vec!["Siigo".into()],
            object_lock: true,
            has_cloud_verify: true,
            cloud_verify_run: ok("2026-09-30T05:00:00-05:00"),
            kit_ok: true,
            has_retention: true,
            ..Default::default()
        };
        let p = evaluate(&f, now());
        assert_eq!(p.total, 6, "sin prueba de restauración propia");
        assert_eq!(p.score, 6, "{:#?}", p.items);
        assert!(p.items[0].detail.contains("Recibe la copia externa de «Siigo»"));
    }

    /// Los vectores compartidos con la consola (`consola/scripts/vectores-regla.ts`).
    #[test]
    fn regla_321_vectores_compartidos() {
        let doc: serde_json::Value = serde_json::from_str(include_str!("../../protocolo/vectors/regla-321.json")).unwrap();
        let ahora = parse(doc["ahora"].as_str().unwrap()).unwrap();
        let vectores = doc["vectores"].as_array().unwrap();
        assert!(vectores.len() >= 10);
        for v in vectores {
            let nombre = v["nombre"].as_str().unwrap();
            let e: EntradaRegla = serde_json::from_value(v["entrada"].clone()).unwrap_or_else(|x| panic!("{nombre}: {x}"));
            let r = regla_321(&e, ahora);
            let mut obtenido = serde_json::to_value(&r).unwrap();
            for p in obtenido["partes"].as_array_mut().unwrap() {
                p.as_object_mut().unwrap().remove("detalle");
            }
            assert_eq!(obtenido, v["esperado"], "vector «{nombre}»: {}", v["que"]);
        }
    }

    #[test]
    fn regla_321_dentro_de_la_salud() {
        // Sin entrada, como siempre (nada nuevo en el informe).
        let p = evaluate(&completo(), now());
        assert!(p.regla.is_none());
        assert!(!serde_json::to_string(&p).unwrap().contains("regla"), "no cambia lo que se manda");
        // Con entrada: la regla va aparte y no toca la puntuación.
        let mut f = completo();
        f.regla = Some(EntradaRegla {
            origen: OrigenRegla { equipo: "e1".into(), soporte: "equipo:e1:origen".into() },
            pasos: vec![PasoRegla {
                id: "zona".into(),
                lugar: Lugar::Oficina,
                inmutable: Inmutable::SoloAnadir,
                soporte: "equipo:a:D:".into(),
                equipo: Some("a".into()),
                ultima_ok: Some("2026-10-01T11:00:00-05:00".into()),
                ..Default::default()
            }],
            verificacion: PruebaRegla { configurada: true, ultima_ok: Some("2026-09-28T03:00:00-05:00".into()), fallo: false },
            prueba_restauracion: PruebaRegla { configurada: false, ..Default::default() },
        });
        let p = evaluate(&f, now());
        assert_eq!((p.score, p.total), (7, 7));
        let r = p.regla.unwrap();
        assert!(!r.cumple && !r.cumple_config && !r.dejo_de_cumplir);
        let accion = |id: &str| r.partes.iter().find(|x| x.id == id).unwrap().accion.clone();
        assert_eq!((accion("copias"), accion("fuera"), accion("errores")), ("anadir_destino".into(), "anadir_fuera".into(), "programar_prueba".into()));
        assert_eq!(r.avisos, ["inmutable_local"]);
        // Nunca cuenta el sistema de archivos ni el sistema operativo: no están en la entrada.
        assert_eq!(margen_horas(None), 48.0);
        assert_eq!(margen_horas(Some(1.0)), 13.5);
        assert_eq!(margen_horas(Some(f64::NAN)), 48.0);
    }

    #[test]
    fn comprobacion_del_servidor() {
        let u = probe_url("rest:https://ana:clave@nas.local:8000/siigo/").unwrap();
        assert!(u.starts_with("https://nas.local:8000/siigo/data/"), "{u}");
        assert_eq!(u.rsplit('/').next().unwrap().len(), 64);
        assert!(!u.contains("clave") && !u.contains("ana@"));
        assert!(probe_url("s3:x").is_none());
        assert_eq!(base64(b"usuario:clave"), "dXN1YXJpbzpjbGF2ZQ==");
        assert_eq!(base64(b"ab"), "YWI=");
        let c = AppendOnlyCheck { checked_at: "2026-10-01T00:00:00-05:00".into(), append_only: Some(true) };
        assert!(c.fresh(now()));
        assert!(!AppendOnlyCheck { checked_at: "2026-09-29T00:00:00-05:00".into(), append_only: None }.fresh(now()));
        // «No se sabe» se vuelve a mirar a la hora; un resultado, al día.
        assert!(!AppendOnlyCheck { checked_at: "2026-10-01T10:00:00-05:00".into(), append_only: None }.fresh(now()));
        assert!(AppendOnlyCheck { checked_at: "2026-10-01T10:00:00-05:00".into(), append_only: Some(false) }.fresh(now()));
        assert_eq!(
            (estado_solo_anadir(403), estado_solo_anadir(200), estado_solo_anadir(404), estado_solo_anadir(401)),
            (Some(true), Some(false), Some(false), None)
        );
    }

    #[test]
    fn autoridad_propia_del_almacen() {
        let (ca, _, _) = resguardo_motor::tls::generar_ca("Prueba").unwrap();
        assert!(tls_con_autoridad(ca.as_bytes()).is_some());
        assert!(tls_con_autoridad(b"no es un certificado").is_none());
        // Un archivo de autoridad que no existe: no se sabe (no se cae a las del sistema).
        assert_eq!(probe_append_only("rest:https://127.0.0.1:9/x/", None, Some("no-existe.pem")), None);
    }
}
