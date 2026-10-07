//! Trabajos de espejo (plan 0.7.26, bloque 4; docs/espejo.md «Trabajos de espejo»).
//!
//! Un trabajo es un espejo con todo lo suyo: quién lo hace (el almacén, con lo
//! que guarda en sus zonas, o el propio equipo, con los repositorios de sus
//! discos), qué repositorios, adónde, cuándo (horario, en cadena o después de
//! otro trabajo, después de cada copia nueva, con retraso), su retención
//! (nunca borra, con retraso de N días o igual que el origen), su freno, la
//! verificación y el límite de velocidad. Puede haber varios por repositorio
//! y por destino. El motor de cada vuelta es el de siempre (espejo_motor.rs):
//! copia archivos cifrados sin abrirlos, así que nunca necesita contraseñas.
//!
//! Compatibilidad: los destinos del espejo de antes (`espejo.destinos[]`) se
//! leen como trabajos equivalentes ([`de_destino`]), con el mismo archivo de
//! estado. Al guardar trabajos se guarda también `destinos` con una vista
//! compatible ([`a_destino`]) para un agente anterior (vuelta atrás de una
//! actualización) y para las consolas anteriores, que solo leen eso.

use crate::espejo_motor::{AccionFreno, Alcance, Freno, Lado, Opciones};
use crate::gestion_v2::Horario;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// `resumen.admite`: trabajos de espejo en el almacén (`guarda_copias { espejo: { trabajos } }`).
pub const ADMITE_TRABAJOS: &str = "espejo_trabajos";
/// `resumen.admite`: espejos que hace el propio equipo (`guarda_copias { espejo_equipo }`).
pub const ADMITE_EQUIPO: &str = "espejo_equipo";

pub const QUIEN_ALMACEN: &str = "almacen";
pub const QUIEN_EQUIPO: &str = "equipo";
/// Como mucho, trabajos en un equipo.
pub const MAX_TRABAJOS: usize = 50;
/// Como mucho, minutos de retraso.
pub const RETRASO_MAX_MIN: u32 = 24 * 60;
/// Días de «con retraso»: de 1 a 3650.
pub const RETRASO_DIAS_MAX: u32 = 3650;

fn si() -> bool {
    true
}
fn almacen() -> String {
    QUIEN_ALMACEN.into()
}
fn es_cero(n: &u32) -> bool {
    *n == 0
}
fn es_cero64(n: &u64) -> bool {
    *n == 0
}

/// Qué repositorios.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(tag = "tipo", rename_all = "lowercase")]
pub enum Que {
    /// Todos (también los que lleguen después).
    #[default]
    Todos,
    /// Los de estos equipos (sus usuarios en el almacén; solo en el almacén).
    Equipos { equipos: Vec<String> },
    /// Estos repositorios: `<usuario>/<repo>` en el almacén, ids en el equipo.
    Repos { repos: Vec<String> },
}

/// Adónde: otra carpeta del equipo, otra zona del almacén o una nube conectada en quien lo hace.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Adonde {
    /// "carpeta", "zona" (solo en el almacén: `carpeta` es su id o "principal") o "nube".
    pub tipo: String,
    pub carpeta: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nube: Option<String>,
}

/// Cuándo. Se puede combinar el horario con lo demás.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Cuando {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horario: Option<Horario>,
    /// Después de cada copia nueva de sus repositorios (una versión nueva en `snapshots/`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tras_copia: bool,
    /// En cadena: cuando termina **bien** ese otro trabajo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cadena: Option<String>,
    /// Después de ese otro trabajo, salga bien o no.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub despues: Option<String>,
    /// Con retraso (minutos) tras la copia nueva o el otro trabajo.
    #[serde(default, skip_serializing_if = "es_cero")]
    pub retraso_min: u32,
}

/// Retención.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(tag = "modo", rename_all = "lowercase")]
pub enum Retencion {
    /// Nunca borra.
    #[default]
    Nunca,
    /// Sigue al original con retraso: lo que ya no está se borra pasados `dias`.
    Retraso { dias: u32 },
    /// Igual que el origen: lo que ya no está se borra en la vuelta siguiente.
    Igual,
}

impl Retencion {
    /// Días hasta borrar (para comparar): nunca = sin fin, igual = 0.
    fn dias_hasta_borrar(&self) -> u64 {
        match self {
            Retencion::Nunca => u64::MAX,
            Retencion::Retraso { dias } => u64::from(*dias),
            Retencion::Igual => 0,
        }
    }
}

/// Lo que se recuerda de un trabajo entre vueltas (no lo manda la consola).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct EstadoTrabajo {
    /// Cuándo terminó la última vuelta (RFC 3339).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ultima: Option<String>,
    /// Cuándo empezó la última vuelta (desde ahí se cuenta el horario).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inicio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resultado: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verificacion: Option<crate::espejo::Verificacion>,
    #[serde(default, skip_serializing_if = "es_cero64")]
    pub danados_origen: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub por_borrar: Option<crate::espejo::PorBorrar>,
    /// El freno de la última vuelta, si saltó.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freno: Option<String>,
    /// Lo que conserva el freno «avisar»: archivos y bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retenidos: Option<(u64, u64)>,
    /// El espacio de la nube tras la última vuelta, y cuándo se leyó.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cuota: Option<(crate::espacio::Espacio, String)>,
    /// «Hacer ahora» (`espejo_ahora`): toca en cuanto se pueda.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pedido_ahora: bool,
}

/// Un trabajo de espejo.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Trabajo {
    pub id: String,
    #[serde(default)]
    pub nombre: String,
    #[serde(default = "si")]
    pub activo: bool,
    /// "almacen" o "equipo".
    #[serde(default = "almacen")]
    pub quien: String,
    #[serde(default)]
    pub que: Que,
    /// En el almacén: de qué zona copia (sin ella, la principal).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zona: Option<String>,
    pub adonde: Adonde,
    #[serde(default)]
    pub cuando: Cuando,
    #[serde(default)]
    pub retencion: Retencion,
    #[serde(default)]
    pub freno: Freno,
    /// % del destino que se comprueba cada día (sin él, 5 en carpetas y 0 en nubes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verificar_pct: Option<u8>,
    /// Límite de velocidad hacia una nube (KiB/s).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limite_kib: Option<u32>,
    /// Orden en la lista (y en qué orden se hacen los que tocan a la vez).
    #[serde(default)]
    pub orden: u32,
    /// Destino con bloqueo de objetos sin plazo conocido (o que no se debe tocar): nunca se borra.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bloqueo: bool,
    /// Bloqueo de objetos de N días: sin «igual que el origen» y con retraso de más de N días.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bloqueo_dias: Option<u32>,
    /// Con una selección de repositorios: los que había al elegirla (los demás son nuevos).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vistos: Vec<String>,
    #[serde(default)]
    pub estado: EstadoTrabajo,
}

impl Trabajo {
    pub fn es_equipo(&self) -> bool {
        self.quien == QUIEN_EQUIPO
    }

    /// El % que se comprueba cada día.
    pub fn pct_verificar(&self) -> u8 {
        self.verificar_pct.unwrap_or(if self.adonde.tipo == "nube" { 0 } else { crate::espejo::VERIFICAR_CARPETA }).min(100)
    }

    /// Las opciones del motor.
    pub fn opciones(&self) -> Opciones {
        Opciones {
            verificar_pct: self.pct_verificar(),
            retencion_dias: match self.retencion {
                Retencion::Retraso { dias } => Some(dias),
                _ => None,
            },
            igual: self.retencion == Retencion::Igual,
            bloqueo: self.bloqueo,
            bloqueo_dias: self.bloqueo_dias,
            freno: self.freno.clone(),
        }
    }

    /// Qué repositorios del origen (en el almacén).
    pub fn alcance(&self) -> Alcance {
        match &self.que {
            Que::Todos => Alcance::Todos,
            Que::Equipos { equipos } => Alcance::Repos(equipos.clone()),
            Que::Repos { repos } => Alcance::Repos(repos.clone()),
        }
    }

    /// El horario (vacío = sin horario).
    pub fn plan(&self) -> Option<crate::plans::PlanSchedule> {
        self.cuando.horario.as_ref().filter(|h| !h.horas.is_empty() || !h.reglas.is_empty()).and_then(|h| h.plan_schedule().ok())
    }

    /// Desde cuándo se cuenta: el comienzo de la última vuelta (o, si no, cuándo terminó).
    pub fn desde(&self) -> Option<chrono::DateTime<chrono::Local>> {
        fecha(self.estado.inicio.as_deref().or(self.estado.ultima.as_deref())?)
    }

    /// Lo que identifica su destino (dos trabajos al mismo sitio).
    pub fn clave_destino(&self) -> String {
        let carpeta = self.adonde.carpeta.trim().trim_end_matches(['/', '\\']);
        let carpeta = if cfg!(windows) || self.adonde.tipo == "nube" { carpeta.to_lowercase() } else { carpeta.to_string() };
        format!(
            "{}|{}|{}|{}|{}",
            self.quien,
            self.adonde.tipo,
            self.adonde.nube.as_deref().unwrap_or_default(),
            carpeta,
            self.zona.as_deref().unwrap_or("principal")
        )
    }

    /// «Disco E:», «nube Dropbox Sur», «zona z0e0e0e»… (para los registros).
    pub fn texto_destino(&self) -> String {
        let t = match (&self.adonde.nube, self.adonde.tipo.as_str()) {
            (Some(n), "nube") => format!("{n}:{}", self.adonde.carpeta),
            (_, "zona") => format!("zona {}", self.adonde.carpeta),
            _ => self.adonde.carpeta.clone(),
        };
        match &self.zona {
            Some(z) => format!("{t} (desde la zona {z})"),
            None => t,
        }
    }

    pub fn ok(&self) -> bool {
        self.estado.resultado.as_deref().is_some_and(|r| !r.starts_with("ERROR"))
    }
}

/// Una fecha RFC 3339 (o solo AAAA-MM-DD, de versiones anteriores).
pub fn fecha(t: &str) -> Option<chrono::DateTime<chrono::Local>> {
    if let Ok(d) = chrono::DateTime::parse_from_rfc3339(t) {
        return Some(d.with_timezone(&chrono::Local));
    }
    let dia = chrono::NaiveDate::parse_from_str(t.get(..10)?, "%Y-%m-%d").ok()?;
    chrono::TimeZone::from_local_datetime(&chrono::Local, &dia.and_hms_opt(0, 0, 0)?).earliest()
}

// ---------- Compatibilidad con el espejo por destinos ----------

/// El id de un destino de antes: el mismo de su archivo de estado (`espejo-<id>.json`),
/// así lo que llevaba anotado (lo que falta, la rotación) sigue valiendo.
pub fn id_legado(d: &crate::espejo::Destino) -> String {
    use sha2::{Digest, Sha256};
    let zona = d.zona.as_deref().map(|z| format!("|{z}")).unwrap_or_default();
    let h = Sha256::digest(format!("{}|{}|{}{zona}", d.tipo, d.nube.as_deref().unwrap_or_default(), d.carpeta.trim()).as_bytes());
    h.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// El nombre que se pone a un trabajo sin nombre.
pub fn nombre_por_defecto(a: &Adonde) -> String {
    let n = match (a.tipo.as_str(), &a.nube) {
        ("nube", Some(n)) => format!("Espejo a «{n}»"),
        ("zona", _) if a.carpeta == "principal" => "Espejo a la zona principal".into(),
        ("zona", _) => format!("Espejo a la zona {}", a.carpeta.trim()),
        _ => format!("Espejo a {}", a.carpeta.trim()),
    };
    // Como mucho 80 letras (una carpeta larga): el final, que es lo que la distingue.
    if n.chars().count() <= 80 {
        return n;
    }
    let cola: String = n.chars().rev().take(79).collect::<Vec<_>>().into_iter().rev().collect();
    format!("…{cola}")
}

/// El horario «cada día a esa hora» de antes.
pub fn horario_diario(hora: &str) -> Horario {
    Horario { dias: vec![1, 2, 3, 4, 5, 6, 7], horas: vec![hora.to_string()], reglas: vec![] }
}

/// Un destino del espejo de antes como trabajo equivalente (mismo comportamiento).
pub fn de_destino(d: &crate::espejo::Destino, hora: &str, limite_kib: Option<u32>, orden: u32) -> Trabajo {
    let carpeta = if d.tipo == "nube" { d.carpeta.trim().trim_matches('/').to_string() } else { d.carpeta.trim().to_string() };
    let adonde = Adonde { tipo: d.tipo.clone(), carpeta, nube: d.nube.clone().filter(|_| d.tipo == "nube") };
    Trabajo {
        id: id_legado(d),
        nombre: nombre_por_defecto(&adonde),
        activo: true,
        quien: QUIEN_ALMACEN.into(),
        que: match &d.repos {
            None => Que::Todos,
            Some(l) => Que::Repos { repos: l.clone() },
        },
        zona: d.zona.clone(),
        cuando: Cuando { horario: Some(d.horario.clone().unwrap_or_else(|| horario_diario(hora))), tras_copia: d.tras_copia, ..Default::default() },
        retencion: match d.retencion_dias.filter(|_| !d.bloqueo) {
            None => Retencion::Nunca,
            Some(dias) => Retencion::Retraso { dias },
        },
        // Antes el % contaba siempre (con ≥ 20 archivos): el freno nuevo, con sus mínimos.
        freno: Freno::default(),
        verificar_pct: d.verificar_pct,
        limite_kib: limite_kib.filter(|_| d.tipo == "nube"),
        orden,
        bloqueo: d.bloqueo,
        bloqueo_dias: None,
        vistos: d.vistos.clone(),
        estado: EstadoTrabajo {
            ultima: d.ultima.clone(),
            inicio: d.inicio.clone(),
            resultado: d.resultado.clone(),
            verificacion: d.verificacion.clone(),
            danados_origen: d.danados_origen,
            por_borrar: d.por_borrar.clone(),
            freno: d.freno.clone(),
            retenidos: None,
            cuota: d.cuota.clone(),
            pedido_ahora: false,
        },
        adonde,
    }
}

/// La vista de un trabajo como destino de antes (para un agente anterior tras una
/// vuelta atrás y para las consolas anteriores). Si no se puede decir igual, lo más
/// parecido sin borrar antes: «igual que el origen» se ve como 7 días; «en cadena» o
/// «después de» otro, como «después de cada copia»; los de ciertos equipos, como su
/// selección de usuarios.
pub fn a_destino(t: &Trabajo) -> crate::espejo::Destino {
    let bloqueo = t.bloqueo || (t.bloqueo_dias.is_some() && t.retencion == Retencion::Nunca);
    crate::espejo::Destino {
        tipo: t.adonde.tipo.clone(),
        carpeta: t.adonde.carpeta.clone(),
        zona: t.zona.clone(),
        nube: t.adonde.nube.clone(),
        ultima: t.estado.ultima.clone(),
        resultado: t.estado.resultado.clone(),
        cuota: t.estado.cuota.clone(),
        horario: t.cuando.horario.clone(),
        tras_copia: t.cuando.tras_copia || t.cuando.cadena.is_some() || t.cuando.despues.is_some(),
        inicio: t.estado.inicio.clone(),
        repos: match &t.que {
            Que::Todos => None,
            Que::Equipos { equipos } => Some(equipos.clone()),
            Que::Repos { repos } => Some(repos.clone()),
        },
        vistos: t.vistos.clone(),
        verificar_pct: t.verificar_pct,
        verificacion: t.estado.verificacion.clone(),
        danados_origen: t.estado.danados_origen,
        retencion_dias: if bloqueo {
            None
        } else {
            match t.retencion {
                Retencion::Nunca => None,
                Retencion::Retraso { dias } => Some(dias.max(crate::espejo_motor::RETENCION_MIN)),
                Retencion::Igual => Some(crate::espejo_motor::RETENCION_MIN),
            }
        },
        bloqueo,
        por_borrar: t.estado.por_borrar.clone(),
        freno: t.estado.freno.clone(),
    }
}

/// ¿Se puede decir este trabajo, sin perder nada, con un destino de antes? (Una
/// consola anterior solo puede cambiar el espejo si todos lo son.)
pub fn exacto(t: &Trabajo, todos: &[Trabajo], limite_kib: Option<u32>) -> bool {
    t.quien == QUIEN_ALMACEN
        && t.activo
        && !matches!(t.que, Que::Equipos { .. })
        && t.cuando.cadena.is_none()
        && t.cuando.despues.is_none()
        && t.cuando.retraso_min <= 12
        && t.cuando.horario.as_ref().is_some_and(|h| !h.horas.is_empty() || !h.reglas.is_empty())
        && match t.retencion {
            Retencion::Nunca => true,
            Retencion::Retraso { dias } => dias >= crate::espejo_motor::RETENCION_MIN && !t.bloqueo,
            Retencion::Igual => false,
        }
        && t.freno == Freno::default()
        && t.bloqueo_dias.is_none()
        && t.nombre == nombre_por_defecto(&t.adonde)
        && (t.adonde.tipo != "nube" || t.limite_kib == limite_kib)
        && todos.iter().filter(|o| o.clave_destino() == t.clave_destino()).count() == 1
}

// ---------- Lo que pide la consola ----------

fn id_valido(id: &str) -> bool {
    (1..=40).contains(&id.len()) && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn lista_valida(l: &[String], valido: impl Fn(&str) -> bool, que: &str) -> Result<Vec<String>, String> {
    let mut v: Vec<String> = Vec::new();
    for r in l {
        let r = r.trim();
        if !valido(r) {
            return Err(format!("{que} no válido en un espejo: «{}».", r.chars().take(60).collect::<String>()));
        }
        if !v.iter().any(|x| x == r) {
            v.push(r.to_string());
        }
    }
    if v.is_empty() {
        return Err(format!("Elige al menos un {} para el espejo (o todos).", que.to_lowercase()));
    }
    if v.len() > 500 {
        return Err("Demasiados repositorios en un espejo.".into());
    }
    Ok(v)
}

/// Los trabajos de una orden (`{ trabajos: [...] }`), ya comprobados (sin mirar
/// el equipo: eso lo hace quien los guarda). `quien`: los que puede hacer.
pub fn leer_pedido(v: &Value, quien: &str) -> Result<Vec<Trabajo>, String> {
    let lista = v["trabajos"].as_array().ok_or("«trabajos» tiene que ser una lista.")?;
    if lista.len() > MAX_TRABAJOS {
        return Err(format!("Como mucho {MAX_TRABAJOS} espejos."));
    }
    let mut out: Vec<Trabajo> = Vec::new();
    for (i, x) in lista.iter().enumerate() {
        let mut t: Trabajo = serde_json::from_value(x.clone()).map_err(|_| format!("El espejo {} no es válido.", i + 1))?;
        // Lo que recuerda el equipo no lo dice la consola.
        t.estado = EstadoTrabajo::default();
        validar(&mut t, quien)?;
        if out.iter().any(|o| o.id == t.id) {
            return Err("Hay dos espejos con el mismo id.".into());
        }
        out.push(t);
    }
    validar_conjunto(&out)?;
    Ok(out)
}

/// Un trabajo, comprobado y normalizado (sin mirar el equipo).
pub fn validar(t: &mut Trabajo, quien: &str) -> Result<(), String> {
    if t.quien != quien {
        return Err(if quien == QUIEN_EQUIPO { "Ese espejo lo tiene que hacer el almacén." } else { "Ese espejo lo tiene que hacer el propio equipo." }.into());
    }
    if !id_valido(&t.id) {
        return Err("Id de espejo no válido.".into());
    }
    t.nombre = t.nombre.trim().to_string();
    if t.nombre.is_empty() {
        t.nombre = nombre_por_defecto(&t.adonde);
    }
    if t.nombre.chars().count() > 80 || t.nombre.chars().any(char::is_control) {
        return Err("Nombre de espejo no válido (hasta 80 letras).".into());
    }
    let equipo = t.es_equipo();
    t.que = match std::mem::take(&mut t.que) {
        Que::Todos => Que::Todos,
        Que::Equipos { .. } if equipo => return Err("Un espejo del propio equipo elige repositorios, no equipos.".into()),
        Que::Equipos { equipos } => Que::Equipos { equipos: lista_valida(&equipos, |e| crate::espejo_motor::repo_valido(e) && !e.contains('/'), "Equipo")? },
        Que::Repos { repos } if equipo => Que::Repos { repos: lista_valida(&repos, crate::gestion_v2::id_valido, "Repositorio")? },
        Que::Repos { repos } => Que::Repos { repos: lista_valida(&repos, crate::espejo_motor::repo_valido, "Repositorio")? },
    };
    t.zona = match t.zona.take() {
        None => None,
        Some(z) if z == "principal" => None,
        Some(_) if equipo => return Err("Un espejo del propio equipo no copia de una zona.".into()),
        Some(z) if crate::server::zona_id_valido(&z) => Some(z),
        Some(_) => return Err("Zona de origen del espejo no válida.".into()),
    };
    t.adonde.carpeta = t.adonde.carpeta.trim().to_string();
    match t.adonde.tipo.as_str() {
        "carpeta" => {
            t.adonde.nube = None;
            if t.adonde.carpeta.is_empty() {
                return Err("Falta la carpeta del espejo.".into());
            }
        }
        "zona" if equipo => return Err("Un espejo del propio equipo va a una carpeta o a una nube.".into()),
        "zona" => {
            t.adonde.nube = None;
            if t.adonde.carpeta != "principal" && !crate::server::zona_id_valido(&t.adonde.carpeta) {
                return Err("Zona del espejo no válida.".into());
            }
            if t.adonde.carpeta == t.zona.as_deref().unwrap_or("principal") {
                return Err("Un espejo no puede copiar una zona en sí misma: elige otra zona.".into());
            }
        }
        "nube" => {
            let n = t.adonde.nube.as_deref().map(str::trim).unwrap_or_default().to_string();
            if n.is_empty() || n.chars().count() > 80 {
                return Err("A un espejo en la nube le falta el nombre de la nube.".into());
            }
            t.adonde.nube = Some(n);
            t.adonde.carpeta = t.adonde.carpeta.trim_matches('/').to_string();
            if !crate::nube::carpeta_remota_valida(&t.adonde.carpeta) {
                return Err("Carpeta de la nube no válida (por ejemplo, Resguardo/Sur).".into());
            }
        }
        _ => return Err("Adónde no válido (carpeta, zona o nube).".into()),
    }
    // Cuándo.
    if t.cuando.horario.as_ref().is_some_and(|h| h.horas.is_empty() && h.reglas.is_empty()) {
        t.cuando.horario = None;
    }
    if let Some(h) = &t.cuando.horario {
        h.plan_schedule().and_then(|p| p.validate()).map_err(|e| format!("Horario del espejo no válido: {e}"))?;
    }
    if t.cuando.cadena.is_some() && t.cuando.despues.is_some() {
        return Err("Un espejo va «en cadena» o «después de» otro, no las dos cosas.".into());
    }
    if let Some(o) = t.cuando.cadena.as_ref().or(t.cuando.despues.as_ref()) {
        if !id_valido(o) || *o == t.id {
            return Err("Un espejo no puede ir después de sí mismo.".into());
        }
    }
    if t.cuando.retraso_min > RETRASO_MAX_MIN {
        return Err("El retraso del espejo va de 0 a 1440 minutos (un día).".into());
    }
    if t.cuando.horario.is_none() && !t.cuando.tras_copia && t.cuando.cadena.is_none() && t.cuando.despues.is_none() {
        return Err(format!("Di cuándo se hace «{}»: con horario, después de cada copia o después de otro espejo.", t.nombre));
    }
    // Retención, bloqueo y freno.
    if let Retencion::Retraso { dias } = t.retencion {
        if !(1..=RETRASO_DIAS_MAX).contains(&dias) {
            return Err(format!("El retraso del borrado va de 1 a {RETRASO_DIAS_MAX} días."));
        }
    }
    if t.bloqueo && t.retencion != Retencion::Nunca {
        return Err("Un destino con bloqueo de objetos sin plazo no puede borrar: elige «Nunca borra».".into());
    }
    if let Some(n) = t.bloqueo_dias {
        if !(1..=36_500).contains(&n) {
            return Err("Los días del bloqueo de objetos no son válidos.".into());
        }
        match t.retencion {
            Retencion::Igual => {
                return Err(format!("Con bloqueo de objetos de {n} días no se puede «igual que el origen»: elige un retraso de más de {n} días."))
            }
            Retencion::Retraso { dias } if dias <= n => {
                return Err(format!("Con bloqueo de objetos de {n} días, el retraso tiene que ser de más de {n} días (nunca se borra algo bloqueado)."))
            }
            _ => {}
        }
    }
    t.freno.validar()?;
    if t.verificar_pct.is_some_and(|p| p > 100) {
        return Err("El % de verificación del espejo tiene que ir de 0 a 100.".into());
    }
    t.limite_kib = t.limite_kib.filter(|k| *k > 0);
    // Lo que había al elegir: solo con una selección de repositorios.
    if !matches!(t.que, Que::Repos { .. }) {
        t.vistos.clear();
    } else {
        t.vistos = lista_valida(&t.vistos, |r| if equipo { crate::gestion_v2::id_valido(r) } else { crate::espejo_motor::repo_valido(r) }, "Repositorio")
            .unwrap_or_default();
    }
    Ok(())
}

/// Lo que se comprueba de todos juntos: «en cadena»/«después de» a uno que existe y
/// sin círculos; dos al mismo destino, con la misma retención (uno no puede borrar lo
/// que el otro promete guardar).
pub fn validar_conjunto(ts: &[Trabajo]) -> Result<(), String> {
    for t in ts {
        let antes = |x: &Trabajo| x.cuando.cadena.clone().or_else(|| x.cuando.despues.clone());
        if let Some(o) = antes(t) {
            if !ts.iter().any(|x| x.id == o) {
                return Err(format!("«{}» va después de un espejo que ya no está.", t.nombre));
            }
        }
        let mut actual = t.id.clone();
        for _ in 0..=ts.len() {
            match ts.iter().find(|x| x.id == actual).and_then(antes) {
                Some(sig) if sig == t.id => {
                    return Err(format!("Los espejos de «{}» se cierran en un círculo: alguno tiene que empezar con su horario.", t.nombre))
                }
                Some(sig) => actual = sig,
                None => break,
            }
        }
        if let Some(o) =
            ts.iter().find(|o| o.id != t.id && o.clave_destino() == t.clave_destino() && (o.retencion != t.retencion || o.bloqueo_dias != t.bloqueo_dias))
        {
            return Err(format!("«{}» y «{}» van al mismo destino: tienen que tener la misma retención.", t.nombre, o.nombre));
        }
    }
    Ok(())
}

/// ¿Reduce la protección pasar de `antes` a `ahora`? (orden con espera). Quitar o
/// pausar un espejo, cambiar su destino, dejar fuera repositorios, borrar antes,
/// quitar el bloqueo o aflojar el freno.
pub fn reduce(antes: &[Trabajo], ahora: &[Trabajo]) -> bool {
    antes.iter().filter(|a| a.activo).any(|a| {
        let Some(n) = ahora.iter().find(|n| n.id == a.id) else { return true };
        if !n.activo || n.clave_destino() != a.clave_destino() {
            return true;
        }
        let menos = match (&a.que, &n.que) {
            (_, Que::Todos) => false,
            (Que::Todos, _) => true,
            (Que::Equipos { equipos: x }, Que::Equipos { equipos: y }) | (Que::Repos { repos: x }, Que::Repos { repos: y }) => x.iter().any(|r| !y.contains(r)),
            _ => true,
        };
        let borra_antes = n.retencion.dias_hasta_borrar() < a.retencion.dias_hasta_borrar();
        let bloqueo = (a.bloqueo && !n.bloqueo) || a.bloqueo_dias.is_some_and(|d| n.bloqueo_dias.is_none_or(|m| m < d));
        let (f, g) = (&a.freno, &n.freno);
        let freno = g.pct > f.pct
            || g.min_archivos > f.min_archivos
            || g.min_faltan > f.min_faltan
            || (f.accion == AccionFreno::Confirmar && g.accion == AccionFreno::Avisar);
        menos || borra_antes || bloqueo || freno
    })
}

/// Con una selección: lo que había al elegirla (lo que dice la consola; si no, lo de
/// antes con la misma selección o lo que hay ahora, `hay`). Los elegidos, siempre.
pub fn fijar_vistos(nuevos: &mut [Trabajo], anteriores: &[Trabajo], hay: &dyn Fn(&Trabajo) -> Vec<String>) {
    for t in nuevos.iter_mut() {
        let Que::Repos { repos } = t.que.clone() else {
            t.vistos.clear();
            continue;
        };
        if t.vistos.is_empty() {
            t.vistos = match anteriores.iter().find(|a| a.id == t.id && a.que == t.que) {
                Some(a) => a.vistos.clone(),
                None => hay(t),
            };
        }
        for r in repos {
            if !t.vistos.contains(&r) {
                t.vistos.push(r);
            }
        }
        t.vistos.sort();
    }
}

/// Los trabajos de una orden de antes (por destinos) toman el id, el nombre y el orden
/// del trabajo actual al mismo destino: así siguen siendo el mismo (con su estado).
pub fn mapear_ids(nuevos: &mut [Trabajo], actuales: &[Trabajo]) {
    for t in nuevos.iter_mut() {
        if let Some(a) = actuales.iter().find(|a| a.clave_destino() == t.clave_destino()) {
            t.id = a.id.clone();
            t.nombre = a.nombre.clone();
            t.orden = a.orden;
        }
    }
}

/// Lo que se conserva de una vuelta anterior: el estado de cada trabajo que sigue
/// con el mismo destino; uno nuevo (o que cambia de destino) empieza sin nada anotado.
pub fn conservar_estado(nuevos: &mut [Trabajo], anteriores: &[Trabajo], olvidar: &dyn Fn(&Trabajo)) {
    for t in nuevos.iter_mut() {
        match anteriores.iter().find(|a| a.id == t.id && a.clave_destino() == t.clave_destino()) {
            Some(a) => t.estado = a.estado.clone(),
            None => olvidar(t),
        }
    }
}

// ---------- Cuándo toca ----------

/// Por qué toca una vuelta.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Motivo {
    Horario,
    TrasCopia,
    /// En cadena o después de otro espejo.
    TrasOtro,
    /// «Hacer ahora».
    Ahora,
}

/// ¿Toca una vuelta de `t`? `todos`: los trabajos de quien lo hace (para «en cadena» y
/// «después de»); `novedades`: la primera y la última versión nueva de sus repositorios
/// desde su última vuelta.
pub fn toca(t: &Trabajo, todos: &[Trabajo], ahora: chrono::DateTime<chrono::Local>, novedades: Option<(SystemTime, SystemTime)>) -> Option<Motivo> {
    if !t.activo {
        return None;
    }
    if t.estado.pedido_ahora {
        return Some(Motivo::Ahora);
    }
    let desde = t.desde();
    if let Some(plan) = t.plan() {
        let por_horario = match desde {
            Some(s) => plan.is_due(s, ahora),
            None => plan.latest_slot(ahora).is_some(),
        };
        if por_horario {
            return Some(Motivo::Horario);
        }
    }
    let retraso = chrono::Duration::minutes(i64::from(t.cuando.retraso_min));
    // En cadena (si salió bien) o después de otro espejo (siempre), con su retraso.
    if let Some((otro, solo_bien)) = t.cuando.cadena.as_ref().map(|o| (o, true)).or(t.cuando.despues.as_ref().map(|o| (o, false))) {
        if let Some(o) = todos.iter().find(|x| x.id == *otro) {
            let fin = o.estado.ultima.as_deref().and_then(fecha);
            let nuevo = fin.is_some_and(|f| desde.is_none_or(|d| f > d) && ahora.signed_duration_since(f) >= retraso);
            if nuevo && (!solo_bien || o.ok()) {
                return Some(Motivo::TrasOtro);
            }
        }
    }
    let (primera, ultima) = novedades.filter(|_| t.cuando.tras_copia)?;
    let hueco = desde.is_none_or(|s| ahora.signed_duration_since(s) >= chrono::Duration::minutes(crate::plans::MIN_GAP_MIN));
    let espera = crate::espejo::ESPERA_TRAS_COPIA.max(Duration::from_secs(u64::from(t.cuando.retraso_min) * 60));
    let espera_max = crate::espejo::ESPERA_MAXIMA_TRAS_COPIA.max(espera);
    let ahora_s: SystemTime = ahora.into();
    let pasado = |x: SystemTime| ahora_s.duration_since(x).unwrap_or_default();
    (hueco && (pasado(ultima) >= espera || pasado(primera) >= espera_max)).then_some(Motivo::TrasCopia)
}

/// La próxima vuelta por horario (RFC 3339), si tiene horario.
pub fn proxima(t: &Trabajo) -> Option<String> {
    t.plan().and_then(|p| p.next_slot(chrono::Local::now())).map(|x| x.to_rfc3339())
}

// ---------- Una vuelta ----------

/// El archivo con lo que recuerda un trabajo entre vueltas (en la carpeta privada).
/// Los del almacén: `espejo-<id>.json` (el mismo que tenía su destino de antes); los
/// del equipo, uno por repositorio.
pub fn archivo_estado(t: &Trabajo, repo: Option<&str>) -> PathBuf {
    let nombre = match repo {
        None => format!("espejo-{}.json", t.id),
        Some(r) => {
            use sha2::{Digest, Sha256};
            let h: String = Sha256::digest(r.as_bytes()).iter().take(6).map(|b| format!("{b:02x}")).collect();
            format!("espejo-eq-{}-{h}.json", t.id)
        }
    };
    crate::agent::private_dir().join(nombre)
}

pub fn leer_estado(p: &Path) -> crate::espejo_motor::Estado {
    std::fs::read(p).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn guardar_estado(p: &Path, e: &crate::espejo_motor::Estado) {
    let tmp = p.with_extension("json.tmp");
    if let Ok(b) = serde_json::to_vec(e) {
        let _ = std::fs::create_dir_all(p.parent().unwrap_or(Path::new(".")));
        if crate::agent::write_new(&tmp, &b).is_ok() {
            let _ = std::fs::rename(&tmp, p);
        }
    }
}

/// Olvida lo anotado de un trabajo (uno nuevo, o que cambia de destino).
pub fn olvidar_estado(t: &Trabajo) {
    let _ = std::fs::remove_file(archivo_estado(t, None));
    let prefijo = format!("espejo-eq-{}-", t.id);
    if let Ok(rd) = std::fs::read_dir(crate::agent::private_dir()) {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().starts_with(&prefijo) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

/// Una vuelta de `origen` (con `alcance`) al destino de `t` (dentro de `sub`, si se da),
/// con su archivo de estado. `carpeta_zona`: la carpeta de una zona del almacén.
pub fn vuelta(
    t: &Trabajo,
    origen: &Path,
    alcance: &Alcance,
    sub: Option<&str>,
    carpeta_zona: &dyn Fn(&str) -> Option<String>,
    estado: &Path,
    avance: &mut dyn FnMut(Option<u64>, Option<u64>),
) -> Result<crate::espejo_motor::Resumen, String> {
    let trabajo = crate::agent::private_dir();
    let nube;
    let carpeta_nube;
    let carpeta_destino;
    let lado = if t.adonde.tipo == "nube" {
        let nombre = t.adonde.nube.as_deref().unwrap_or_default();
        nube = crate::nube::buscar(nombre).ok_or_else(|| format!("la nube «{nombre}» ya no está conectada en este equipo."))?;
        if !crate::nube::carpeta_remota_valida(&t.adonde.carpeta) {
            return Err("Carpeta de la nube no válida.".into());
        }
        let base = t.adonde.carpeta.trim().trim_matches('/');
        carpeta_nube = match sub {
            Some(s) if base.is_empty() => s.to_string(),
            Some(s) => format!("{base}/{s}"),
            None => base.to_string(),
        };
        Lado::Nube { nube: &nube, carpeta: &carpeta_nube, trabajo: &trabajo, limite_kib: t.limite_kib }
    } else {
        let base =
            if t.adonde.tipo == "zona" { carpeta_zona(&t.adonde.carpeta).ok_or("esa zona ya no está en este almacén.")? } else { t.adonde.carpeta.clone() };
        crate::platform::carpeta_local_valida(&base)?;
        let raiz = Path::new(&base);
        // En pruebas (RESGUARDO_AGENT_DIR, sin administrador) la carpeta es de quien corre la prueba.
        if raiz.exists() && !crate::agent::test_mode() && !crate::platform::owned_by_admins(raiz) {
            return Err("la carpeta del espejo no es de Administradores (vuelve a guardar el espejo para corregirla).".into());
        }
        carpeta_destino = match sub {
            Some(s) => raiz.join(s),
            None => raiz.to_path_buf(),
        };
        Lado::Carpeta(&carpeta_destino)
    };
    let mut e = leer_estado(estado);
    let r = crate::espejo_motor::vuelta(origen, &lado, alcance, &t.opciones(), &mut e, avance);
    guardar_estado(estado, &e);
    r
}

/// Lo que queda en el estado de un trabajo tras una vuelta (o varias, en el equipo).
pub fn anotar_resultado(t: &mut Trabajo, texto: &str, fin: &str, resumenes: &[crate::espejo_motor::Resumen]) {
    t.estado.ultima = Some(fin.to_string());
    t.estado.resultado = Some(texto.to_string());
    if resumenes.is_empty() {
        return;
    }
    let suma = |f: &dyn Fn(&crate::espejo_motor::Resumen) -> u64| resumenes.iter().map(f).sum::<u64>();
    let verificados = suma(&|r| r.verificados);
    if verificados > 0 {
        t.estado.verificacion =
            Some(crate::espejo::Verificacion { ultima: fin.to_string(), archivos: verificados, mal: suma(&|r| r.mal_destino.len() as u64 + r.reparados) });
    }
    t.estado.danados_origen = suma(&|r| r.danados_origen.len() as u64);
    let por_borrar = suma(&|r| r.por_borrar);
    t.estado.por_borrar = (por_borrar > 0).then(|| crate::espejo::PorBorrar {
        archivos: por_borrar,
        bytes: suma(&|r| r.por_borrar_bytes),
        primero: resumenes.iter().filter_map(|r| r.primer_borrado.clone()).min(),
    });
    let frenos: Vec<String> = resumenes.iter().filter_map(|r| r.freno.clone()).collect();
    t.estado.freno = (!frenos.is_empty()).then(|| frenos.join(". "));
    let retenidos = suma(&|r| r.retenidos);
    t.estado.retenidos = (retenidos > 0).then(|| (retenidos, suma(&|r| r.retenidos_bytes)));
}

/// Lo que se ve de un trabajo en el resumen (sin secretos ni estado interno).
pub fn resumen(t: &Trabajo, espacio: Value) -> Value {
    let mut v = json!({
        "id": t.id, "nombre": t.nombre, "activo": t.activo, "quien": t.quien, "que": t.que, "adonde": t.adonde,
        "cuando": t.cuando, "retencion": t.retencion, "freno": t.freno, "verificar_pct": t.pct_verificar(),
        "limite_kib": t.limite_kib, "orden": t.orden, "bloqueo": t.bloqueo, "bloqueo_dias": t.bloqueo_dias,
        "ultima": t.estado.ultima, "inicio": t.estado.inicio, "resultado": t.estado.resultado,
        "verificacion": t.estado.verificacion, "danados_origen": t.estado.danados_origen,
        "por_borrar": t.estado.por_borrar, "freno_aviso": t.estado.freno,
        "retenidos": t.estado.retenidos.map(|(a, b)| json!({ "archivos": a, "bytes": b })),
        "proxima": proxima(t), "espacio": espacio,
    });
    if let Some(z) = &t.zona {
        v["zona"] = z.clone().into();
    }
    if matches!(t.que, Que::Repos { .. }) {
        v["vistos"] = t.vistos.clone().into();
    }
    v
}

/// El texto de una vuelta (o de las de cada repositorio de un espejo del equipo).
pub fn texto_de(t: &Trabajo, motivo: Motivo, hechos: &[(Option<String>, Result<String, String>)]) -> String {
    let por = match motivo {
        Motivo::TrasCopia => " (después de una copia nueva)",
        Motivo::TrasOtro => " (después de otro espejo)",
        Motivo::Ahora => " (pedido desde la consola)",
        Motivo::Horario => "",
    };
    let partes: Vec<String> = hechos
        .iter()
        .map(|(repo, r)| {
            let m = match r {
                Ok(t) | Err(t) => t.clone(),
            };
            match repo {
                Some(x) => format!("{x}: {m}"),
                None => m,
            }
        })
        .collect();
    let errores = hechos.iter().filter(|(_, r)| r.is_err()).count();
    if errores > 0 {
        format!("ERROR: espejo «{}» en {}{por}: {}", t.nombre, t.texto_destino(), partes.join(" · "))
    } else if partes.is_empty() {
        format!("Espejo «{}» en {}{por}: no había ningún repositorio que copiar.", t.nombre, t.texto_destino())
    } else {
        format!("Espejo «{}» hecho en {}{por}: {}", t.nombre, t.texto_destino(), partes.join(" · "))
    }
}

// ---------- Espejos del propio equipo (quien: equipo) ----------

/// Los espejos que hace el propio equipo con los repositorios de sus discos
/// (`espejo-equipo.json` en la carpeta del agente). Funcionan solo con el agente.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct EspejoEquipo {
    #[serde(default)]
    pub trabajos: Vec<Trabajo>,
}

const ARCHIVO_EQUIPO: &str = "espejo-equipo.json";

pub fn cargar_equipo() -> EspejoEquipo {
    std::fs::read(crate::agent::agent_dir().join(ARCHIVO_EQUIPO)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn guardar_equipo(e: &EspejoEquipo) -> Result<(), String> {
    crate::agent::write_json(ARCHIVO_EQUIPO, e)
}

/// Los repositorios de este equipo en sus discos: (id, carpeta). Los de destinos
/// «local» de la consola; los demás (almacén, nube…) no se pueden copiar así.
pub fn repos_locales() -> Vec<(String, PathBuf)> {
    let Some(v) = crate::servidor_v2::cargar() else { return Vec::new() };
    v.repos_v2
        .iter()
        .filter_map(|r| {
            let d = v.destinos.iter().find(|d| d.id == r.destino && d.tipo == "local")?;
            let p = crate::gestion_v2::ubicacion(d, r.ubicacion_origen.as_deref().unwrap_or(&r.id)).ok()?;
            Some((r.id.clone(), PathBuf::from(p)))
        })
        .collect()
}

/// Las carpetas de los destinos «local» del equipo (donde están sus repositorios).
fn bases_locales() -> Vec<String> {
    crate::servidor_v2::cargar().map(|v| v.destinos.iter().filter(|d| d.tipo == "local").map(|d| d.donde.trim().to_string()).collect()).unwrap_or_default()
}

/// Los repositorios de un espejo del equipo: todos los locales o los elegidos.
pub fn repos_de(t: &Trabajo, locales: &[(String, PathBuf)]) -> Vec<(String, Option<PathBuf>)> {
    match &t.que {
        Que::Repos { repos } => repos.iter().map(|r| (r.clone(), locales.iter().find(|(id, _)| id == r).map(|(_, p)| p.clone()))).collect(),
        _ => locales.iter().map(|(id, p)| (id.clone(), Some(p.clone()))).collect(),
    }
}

/// ¿Una carpeta está dentro de la otra (o son la misma)?
pub fn se_solapan(a: &str, b: &str) -> bool {
    let norma = |x: &str| if cfg!(windows) { x.to_lowercase() } else { x.to_string() };
    let (a, b) = (norma(a.trim()), norma(b.trim()));
    let (a, b) = (Path::new(&a), Path::new(&b));
    !a.as_os_str().is_empty() && !b.as_os_str().is_empty() && (a.starts_with(b) || b.starts_with(a))
}

/// Guarda (o quita, con `None` o una lista vacía) los espejos del propio equipo.
pub fn poner_equipo(nuevos: Option<Vec<Trabajo>>) -> Result<String, String> {
    crate::agent::require_admin()?;
    let anterior = cargar_equipo();
    let Some(mut nuevos) = nuevos.filter(|n| !n.is_empty()) else {
        guardar_equipo(&EspejoEquipo::default())?;
        return Ok("Espejos del equipo quitados (lo ya copiado se queda en su destino).".into());
    };
    for t in nuevos.iter_mut() {
        validar(t, QUIEN_EQUIPO)?;
    }
    validar_conjunto(&nuevos)?;
    let nubes = crate::nube::cargar();
    let bases = bases_locales();
    for t in nuevos.iter() {
        match t.adonde.tipo.as_str() {
            "carpeta" => {
                crate::platform::carpeta_local_valida(&t.adonde.carpeta)?;
                if bases.iter().any(|b| se_solapan(b, &t.adonde.carpeta)) {
                    return Err("La carpeta del espejo no puede estar dentro de la carpeta de los repositorios de este equipo (ni al revés).".into());
                }
                if crate::server::carpeta_del_sistema(Path::new(&t.adonde.carpeta)) {
                    return Err("La carpeta del espejo no puede estar en la carpeta de Windows, de los programas o de Resguardo.".into());
                }
            }
            "nube" => {
                let nombre = t.adonde.nube.as_deref().unwrap_or_default();
                if !nubes.iter().any(|n| n.nombre == nombre) {
                    return Err(format!("No hay ninguna nube «{nombre}» conectada en este equipo: conéctala antes."));
                }
            }
            _ => return Err("Un espejo del propio equipo va a una carpeta o a una nube.".into()),
        }
    }
    for t in nuevos.iter().filter(|t| t.adonde.tipo == "carpeta") {
        crate::platform::carpeta_privada(Path::new(&t.adonde.carpeta))?;
    }
    conservar_estado(&mut nuevos, &anterior.trabajos, &olvidar_estado);
    let locales: Vec<String> = repos_locales().into_iter().map(|(id, _)| id).collect();
    fijar_vistos(&mut nuevos, &anterior.trabajos, &|_| locales.clone());
    nuevos.sort_by_key(|t| t.orden);
    let n = nuevos.len();
    guardar_equipo(&EspejoEquipo { trabajos: nuevos })?;
    Ok(if n == 1 { "Espejo del equipo guardado.".into() } else { format!("{n} espejos del equipo guardados.") })
}

/// Lo que se ve de los espejos del equipo en el resumen (`null` si no hay).
pub fn resumen_equipo() -> Value {
    let e = cargar_equipo();
    if e.trabajos.is_empty() {
        return Value::Null;
    }
    let trabajos: Vec<Value> = e
        .trabajos
        .iter()
        .map(|t| {
            let espacio = match (&t.adonde.tipo[..], &t.estado.cuota) {
                ("carpeta", _) => crate::espacio::json_de(&t.adonde.carpeta),
                (_, Some((q, leido))) => q.json(leido),
                _ => Value::Null,
            };
            resumen(t, espacio)
        })
        .collect();
    json!({ "trabajos": trabajos })
}

/// Anota en la configuración algo de un trabajo del equipo (se vuelve a leer: pudo cambiar).
fn anotar_equipo(id: &str, f: impl FnOnce(&mut Trabajo)) {
    let mut e = cargar_equipo();
    if let Some(t) = e.trabajos.iter_mut().find(|t| t.id == id) {
        f(t);
        let _ = guardar_equipo(&e);
    }
}

/// Las versiones nuevas de un repositorio de una carpeta (para «después de cada copia nueva»).
fn novedades_repo(carpeta: &Path, desde: Option<SystemTime>) -> Option<(SystemTime, SystemTime)> {
    let padre = carpeta.parent()?;
    let nombre = carpeta.file_name()?.to_string_lossy().into_owned();
    crate::espejo::novedades(padre, &[nombre], desde)
}

/// Lo llama el servicio en cada vuelta: los espejos del equipo que tocan, uno detrás
/// de otro en otro hilo, cada uno repositorio a repositorio.
pub fn si_toca_equipo() {
    static EN_MARCHA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    static ULTIMA_MIRADA: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
    if EN_MARCHA.load(std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let e = cargar_equipo();
    if e.trabajos.iter().all(|t| !t.activo) {
        return;
    }
    let locales = repos_locales();
    let ahora = chrono::Local::now();
    let mirar = e.trabajos.iter().any(|t| t.cuando.tras_copia) && {
        let mut u = ULTIMA_MIRADA.lock().unwrap_or_else(|p| p.into_inner());
        let ya = u.is_some_and(|t| t.elapsed() < Duration::from_secs(60));
        if !ya {
            *u = Some(std::time::Instant::now());
        }
        !ya
    };
    let mut toca_ya: Vec<(Trabajo, Motivo)> = e
        .trabajos
        .iter()
        .filter_map(|t| {
            let nuevas = if mirar && t.cuando.tras_copia {
                let desde = t.desde().map(SystemTime::from);
                repos_de(t, &locales).into_iter().filter_map(|(_, p)| novedades_repo(&p?, desde)).reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)))
            } else {
                None
            };
            toca(t, &e.trabajos, ahora, nuevas).map(|m| (t.clone(), m))
        })
        .collect();
    toca_ya.sort_by_key(|(t, _)| t.orden);
    if toca_ya.is_empty() || EN_MARCHA.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        for (t, motivo) in toca_ya {
            hacer_equipo(&t, motivo, &locales);
        }
        EN_MARCHA.store(false, std::sync::atomic::Ordering::SeqCst);
    });
}

/// Una vuelta de un espejo del equipo: cada repositorio a `<destino>/<id>`.
pub fn hacer_equipo(t: &Trabajo, motivo: Motivo, locales: &[(String, PathBuf)]) -> String {
    let inicio = chrono::Local::now().to_rfc3339();
    anotar_equipo(&t.id, |x| (x.estado.inicio, x.estado.pedido_ahora) = (Some(inicio.clone()), false));
    let nombre_ventana = t.adonde.nube.clone().unwrap_or_else(|| "Disco o carpeta del equipo".into());
    let guarda = crate::escritorio::en_marcha::empezar(if t.adonde.tipo == "nube" { "nube" } else { "espejo" }, &format!("eq-{}", t.id), &nombre_ventana);
    let carpeta = t.adonde.tipo != "nube";
    let mut hechos: Vec<(Option<String>, Result<String, String>)> = Vec::new();
    let mut resumenes = Vec::new();
    for (id, origen) in repos_de(t, locales) {
        let r = match origen {
            None => Err("ya no está en este equipo (o no está en un disco suyo).".to_string()),
            Some(o) => vuelta(t, &o, &Alcance::Todos, Some(&id), &|_| None, &archivo_estado(t, Some(&id)), &mut |l, s| {
                if carpeta {
                    guarda.progreso(l, None)
                } else {
                    guarda.ritmos(l, s)
                }
            }),
        };
        let texto = r.and_then(|r| {
            let x = crate::espejo::texto_de(&r);
            resumenes.push(r);
            x
        });
        hechos.push((Some(id), texto));
    }
    let texto = texto_de(t, motivo, &hechos);
    if texto.starts_with("ERROR") {
        drop(guarda);
    } else {
        guarda.terminar("ok");
    }
    crate::agent::log(&texto);
    let cuota = if t.adonde.tipo == "nube" { t.adonde.nube.as_deref().and_then(crate::nube::buscar).and_then(|n| crate::nube::cuota(&n)) } else { None };
    let fin = chrono::Local::now().to_rfc3339();
    anotar_equipo(&t.id, |x| {
        anotar_resultado(x, &texto, &fin, &resumenes);
        if let Some(q) = cuota {
            x.estado.cuota = Some((q, fin.clone()));
        }
    });
    crate::bitacora::espejo(&fin, &texto);
    texto
}

/// «Hacer ahora» (`guarda_copias { espejo_ahora: { trabajo, quien? } }`): ese trabajo
/// toca en cuanto se pueda (no reduce la protección: solo copia).
pub fn pedir_ahora(v: &Value) -> Result<String, String> {
    let id = v["trabajo"].as_str().ok_or("Falta el espejo.")?;
    let nombre;
    if v["quien"] == QUIEN_EQUIPO {
        let mut e = cargar_equipo();
        let t = e.trabajos.iter_mut().find(|t| t.id == id).ok_or("Ese espejo ya no está.")?;
        if !t.activo {
            return Err("Ese espejo está en pausa: actívalo antes.".into());
        }
        t.estado.pedido_ahora = true;
        nombre = t.nombre.clone();
        guardar_equipo(&e)?;
        #[cfg(not(test))]
        si_toca_equipo();
    } else {
        let mut c = crate::server::load();
        let mut ts = c.espejo.as_ref().map(|e| e.trabajos_efectivos()).unwrap_or_default();
        let t = ts.iter_mut().find(|t| t.id == id).ok_or("Ese espejo ya no está.")?;
        if !t.activo {
            return Err("Ese espejo está en pausa: actívalo antes.".into());
        }
        t.estado.pedido_ahora = true;
        nombre = t.nombre.clone();
        c.espejo = Some(crate::espejo::Espejo::de_trabajos(ts));
        crate::server::save(&c)?;
        #[cfg(not(test))]
        crate::espejo::si_toca();
    }
    Ok(format!("«{nombre}» empieza en cuanto pueda."))
}

/// Confirma el freno de un trabajo (`espejo_freno { trabajo, quien? }`): en la próxima
/// vuelta se anota lo que falta y se borrará a su tiempo.
pub fn aceptar_freno(t: &Trabajo) -> Result<String, String> {
    if t.retencion == Retencion::Nunca || t.bloqueo {
        return Err("Ese espejo nunca borra nada: no hay nada que confirmar.".into());
    }
    let archivos: Vec<PathBuf> = if t.es_equipo() {
        repos_de(t, &repos_locales()).into_iter().map(|(id, _)| archivo_estado(t, Some(&id))).collect()
    } else {
        vec![archivo_estado(t, None)]
    };
    for p in archivos {
        let mut e = leer_estado(&p);
        e.aceptar_freno = true;
        guardar_estado(&p, &e);
    }
    let cuando = match t.retencion {
        Retencion::Retraso { dias } => format!("pasados {dias} días"),
        _ => "la vez siguiente".into(),
    };
    Ok(format!("Confirmado: la próxima vez que se haga «{}» se anota lo que ya no está y se borrará del espejo {cuando}.", t.nombre))
}

#[cfg(test)]
mod tests;
