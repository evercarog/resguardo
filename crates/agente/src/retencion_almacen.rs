//! Retención en el almacén (v1.22; docs/compartir.md, «Retención en el
//! almacén», y docs/api-servidor.md §5).
//!
//! Un almacén («Este equipo guarda copias») sirve los repositorios de los
//! equipos con rest-server `--append-only --private-repos`: un equipo no puede
//! borrar nada allí, ni siquiera lo suyo antiguo, y por eso `aplicar_retencion`
//! desde el equipo no se puede (403). La retención la aplica el propio almacén,
//! en local (`restic forget --prune` sobre `<carpeta>/<usuario>/<repo>`):
//!
//! - **Con una clave de restic propia del almacén**, no con la contraseña del
//!   repositorio: la consola la genera, el equipo dueño la añade al
//!   repositorio (`clave_almacen`, con su contraseña y la clave de
//!   administración; `restic key add` es una escritura, así que vale con solo
//!   añadir) y el almacén la recibe sellada en `retencion_almacen` (clave de
//!   administración, con la espera de lo destructivo). El almacén puede leer
//!   esos repositorios con ella: es la contrapartida (compartir.md). Se revoca
//!   quitando la regla (el almacén borra el archivo de su clave).
//! - **Con horario**: la regla (cuántas versiones y cuándo, p. ej. los domingos
//!   a las 03:00) la autoriza una vez la clave de administración; después se
//!   aplica sola. «Aplicar ahora» (`aplicar_retencion_almacen`) también espera.
//! - **Sin fiarse de la hora de las versiones**: la escribe el equipo, y uno
//!   comprometido podría subir versiones falsas (con fecha futura, o la última
//!   hora de cada día pasado) para que la regla borrase las buenas. El almacén
//!   decide qué quitar (`planear`, las reglas de `restic forget`) solo con las
//!   versiones cuya hora cuadra con su subida (el archivo `snapshots/<id>`) y
//!   las quita por id; las demás no se tocan.
//! - Opcionalmente, `restic check` después de podar.
//!
//! Lo que el almacén guarda (reglas y claves) va protegido como los demás
//! secretos del agente, en `privado/almacen-retencion.bin`. El resumen solo
//! lleva usuario, nombre del repositorio, regla, horario y resultados.

use crate::gestion_v2::{Plazo, Retencion, SIEMPRE};
use chrono::{DateTime, Datelike, Duration, Local, NaiveTime, Offset, TimeZone, Timelike};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const ARCHIVO: &str = "almacen-retencion.bin";
/// Lo que puede tardar un `forget --prune` o un `check` de un repositorio grande.
const LIMITE: std::time::Duration = std::time::Duration::from_secs(6 * 3600);

/// Cuándo se aplica: días de la semana (1 = lunes … 7 = domingo) y hora local.
/// v1.40 (`admite: "retencion_almacen_horario"`): o, con `reglas`, cuando toque
/// cualquiera de ellas (las de los horarios de copia); `dias` y `hora` siguen
/// para un agente anterior (que no conoce `reglas` y las ignora).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Horario {
    pub dias: Vec<u8>,
    pub hora: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reglas: Vec<crate::gestion_v2::Regla>,
}

impl Horario {
    /// Con reglas: su horario del motor.
    fn plan(&self) -> Option<crate::plans::PlanSchedule> {
        if self.reglas.is_empty() {
            return None;
        }
        crate::gestion_v2::Horario { dias: vec![], horas: vec![], reglas: self.reglas.clone() }.plan_schedule().ok()
    }

    fn valido(&self) -> Result<(), String> {
        if !self.reglas.is_empty() {
            let plan = crate::gestion_v2::Horario { dias: vec![], horas: vec![], reglas: self.reglas.clone() }.plan_schedule()?;
            return plan.validate();
        }
        if self.dias.is_empty() || self.dias.iter().any(|d| !(1..=7).contains(d)) {
            return Err("Elige al menos un día (1 = lunes … 7 = domingo).".into());
        }
        hora(&self.hora).map(|_| ())
    }

    /// «los domingos a las 03:00», «cada día a las 03:00», «lunes y jueves a las 03:00».
    pub fn texto(&self) -> String {
        if !self.reglas.is_empty() {
            return if self.reglas.len() == 1 { "según su horario".into() } else { format!("según su horario ({} reglas)", self.reglas.len()) };
        }
        const NOMBRES: [&str; 7] = ["lunes", "martes", "miércoles", "jueves", "viernes", "sábados", "domingos"];
        let mut dias = self.dias.clone();
        dias.sort();
        dias.dedup();
        let cuando = match dias.len() {
            7 => "cada día".to_string(),
            1 => format!("los {}", NOMBRES[usize::from(dias[0] - 1)]),
            _ => {
                let n: Vec<&str> = dias.iter().map(|d| NOMBRES[usize::from(d - 1)]).collect();
                format!("{} y {}", n[..n.len() - 1].join(", "), n[n.len() - 1])
            }
        };
        format!("{cuando} a las {}", self.hora)
    }

    /// Los huecos (día y hora) de un día concreto, si ese día toca.
    fn hueco_de(&self, dia: chrono::NaiveDate) -> Option<DateTime<Local>> {
        let n = dia.weekday().number_from_monday() as u8;
        if !self.dias.contains(&n) {
            return None;
        }
        Local.from_local_datetime(&dia.and_time(hora(&self.hora).ok()?)).earliest()
    }

    /// El último hueco que ya pasó (como mucho, hace una semana).
    pub fn ultimo(&self, ahora: DateTime<Local>) -> Option<DateTime<Local>> {
        if !self.reglas.is_empty() {
            return self.plan()?.latest_slot(ahora);
        }
        (0..=7).filter_map(|atras| self.hueco_de(ahora.date_naive() - Duration::days(atras))).find(|h| *h <= ahora)
    }

    /// El próximo hueco (después de `ahora`).
    pub fn proximo(&self, ahora: DateTime<Local>) -> Option<DateTime<Local>> {
        if !self.reglas.is_empty() {
            return self.plan()?.next_slot(ahora);
        }
        (0..=7).filter_map(|n| self.hueco_de(ahora.date_naive() + Duration::days(n))).find(|h| *h > ahora)
    }
}

fn hora(h: &str) -> Result<NaiveTime, String> {
    if h.len() != 5 {
        return Err("Hora no válida (HH:MM).".into());
    }
    NaiveTime::parse_from_str(h, "%H:%M").map_err(|_| "Hora no válida (HH:MM).".to_string())
}

/// Una regla de retención de un repositorio de este almacén.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Entrada {
    pub usuario: String,
    /// Su carpeta dentro de la del usuario (`siigo`, o `ana/portatil`).
    pub repo: String,
    /// La contraseña de la clave de restic propia del almacén.
    pub clave: String,
    /// El id de esa clave en el repositorio (`keys/<id>`), cuando ya abre.
    #[serde(default)]
    pub clave_id: Option<String>,
    /// La clave anterior (si se cambió): se borra cuando la nueva ya abre.
    #[serde(default)]
    pub clave_anterior: Option<String>,
    /// ¿La clave abre el repositorio? `None`: aún no se ha probado.
    #[serde(default)]
    pub clave_ok: Option<bool>,
    pub retencion: Retencion,
    pub horario: Horario,
    /// `restic check` después de podar.
    #[serde(default)]
    pub verificar: bool,
    /// Cuándo se puso (los huecos de antes no cuentan).
    pub desde: String,
    #[serde(default)]
    pub ultima: Option<String>,
    /// "ok" o "fallo".
    #[serde(default)]
    pub resultado: Option<String>,
    #[serde(default)]
    pub mensaje: Option<String>,
    #[serde(default)]
    pub versiones: Option<usize>,
}

impl Entrada {
    fn es(&self, usuario: &str, repo: &str) -> bool {
        self.usuario == usuario && self.repo == repo
    }

    /// ¿Toca aplicarla? Si hay un hueco ya pasado posterior a cuando se puso
    /// y a la última vez (también si el equipo estaba apagado a esa hora).
    pub fn toca(&self, ahora: DateTime<Local>) -> bool {
        let Some(hueco) = self.horario.ultimo(ahora) else { return false };
        let fecha = |s: &Option<String>| s.as_deref().and_then(|x| DateTime::parse_from_rfc3339(x).ok()).map(|d| d.with_timezone(&Local));
        let desde = fecha(&Some(self.desde.clone()));
        desde.is_some_and(|d| hueco > d) && fecha(&self.ultima).is_none_or(|u| hueco > u)
    }

    /// Lo que se ve en el resumen (sin la clave ni rutas).
    pub fn resumen(&self, ahora: DateTime<Local>) -> Value {
        json!({
            "usuario": self.usuario, "repo": self.repo,
            "retencion": self.retencion, "texto": self.retencion.texto(),
            "horario": self.horario, "horario_texto": self.horario.texto(), "verificar": self.verificar,
            "clave": match self.clave_ok { Some(true) => "ok", Some(false) => "pendiente", None => "sin_probar" },
            "ultima": self.ultima, "resultado": self.resultado, "mensaje": self.mensaje, "versiones": self.versiones,
            "proxima": self.horario.proximo(ahora).map(|p| p.to_rfc3339()),
        })
    }
}

// ---------- Guardado (protegido, con candado) ----------

static CANDADO: Mutex<()> = Mutex::new(());

fn archivo() -> PathBuf {
    crate::agent::private_dir().join(ARCHIVO)
}

pub fn cargar() -> Vec<Entrada> {
    let _g = CANDADO.lock();
    leer()
}

fn leer() -> Vec<Entrada> {
    std::fs::read(archivo()).ok().and_then(|b| crate::platform::unprotect(&b).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub(crate) fn escribir(lista: &[Entrada]) -> Result<(), String> {
    crate::agent::prepare_dir()?;
    let enc = crate::platform::protect(&serde_json::to_vec(lista).map_err(|e| e.to_string())?)?;
    let tmp = crate::agent::private_dir().join(format!("{ARCHIVO}.tmp"));
    let _ = std::fs::remove_file(&tmp);
    std::fs::write(&tmp, enc).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, archivo()).map_err(|e| e.to_string())
}

/// Cambia la lista con el candado puesto (leer, cambiar y guardar de una vez).
fn cambiar<T>(f: impl FnOnce(&mut Vec<Entrada>) -> Result<T, String>) -> Result<T, String> {
    let _g = CANDADO.lock().map_err(|_| "Candado roto.".to_string())?;
    let mut lista = leer();
    let r = f(&mut lista)?;
    escribir(&lista)?;
    Ok(r)
}

// ---------- Dónde está cada repositorio ----------

/// Nombre de una carpeta: letras, cifras, `.`, `-` y `_`, sin ser `.` ni `..`.
pub(crate) fn segmento_valido(s: &str) -> bool {
    !s.is_empty() && s.len() <= 128 && s != "." && s != ".." && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// La carpeta del repositorio `repo` del usuario `usuario` (que tiene que ser
/// un usuario de este almacén), sin enlaces en el camino y con su `config`.
pub fn carpeta_repo(base: &Path, usuarios: &[String], usuario: &str, repo: &str) -> Result<PathBuf, String> {
    if !segmento_valido(usuario) || !usuarios.iter().any(|u| u == usuario) {
        return Err(format!("«{usuario}» no es un equipo de este almacén."));
    }
    let partes: Vec<&str> = repo.split('/').collect();
    if partes.len() > 4 || !partes.iter().all(|p| segmento_valido(p)) {
        return Err("Nombre de repositorio no válido.".into());
    }
    let mut p = base.join(usuario);
    if crate::platform::is_reparse_point(&p) {
        return Err("La carpeta de ese equipo es un enlace: no se aplica la retención a través de enlaces.".into());
    }
    for parte in &partes {
        p = p.join(parte);
        if crate::platform::is_reparse_point(&p) {
            return Err("La carpeta del repositorio es un enlace: no se aplica la retención a través de enlaces.".into());
        }
    }
    if !p.join("config").is_file() {
        return Err(format!("No hay ningún repositorio «{usuario}/{repo}» en este almacén."));
    }
    Ok(p)
}

fn carpeta_en_este_almacen(usuario: &str, repo: &str) -> Result<PathBuf, String> {
    let c = crate::server::load();
    if !c.enabled {
        return Err("Este equipo no guarda copias.".into());
    }
    let usuarios: Vec<String> = c.users.iter().map(|u| u.name.clone()).collect();
    carpeta_repo(Path::new(&c.path), &usuarios, usuario, repo)
}

// ---------- restic en local, con la clave del almacén ----------

fn acceso(carpeta: &Path, clave: &str) -> crate::restic::Access {
    crate::restic::Access::new(carpeta.display().to_string(), clave)
}

fn restic(acc: &crate::restic::Access, args: &[&str]) -> Result<crate::restic::RawOutput, String> {
    let mut todos = vec!["--no-cache"];
    todos.extend_from_slice(args);
    crate::restic::run_raw(acc, &todos, LIMITE)
}

/// Error de restic para el resultado (lo ve el servidor): sin rutas.
fn error_publico(out: &crate::restic::RawOutput) -> String {
    crate::web::public_message(&crate::restic::exit_error(out.code, &out.stderr))
}

/// ¿Abre la clave el repositorio? `Ok(false)` si la contraseña no vale (aún no
/// la añadió el equipo dueño); `Err` si falla otra cosa.
fn clave_abre(acc: &crate::restic::Access) -> Result<bool, String> {
    let out = restic(acc, &["cat", "config", "--no-lock"])?;
    if out.code == Some(0) {
        return Ok(true);
    }
    // restic ≥ 0.17: código 12 = contraseña equivocada.
    if out.code == Some(12) || out.stderr.contains("wrong password") || out.stderr.contains("no key found") {
        return Ok(false);
    }
    Err(error_publico(&out))
}

/// El id de la clave con la que se abrió (`restic key list --json`, la «current»).
fn id_de_la_clave(acc: &crate::restic::Access) -> Option<String> {
    let out = restic(acc, &["key", "list", "--json", "--no-lock"]).ok().filter(|o| o.code == Some(0))?;
    let lista: Vec<Value> = serde_json::from_slice(&out.stdout).ok()?;
    lista.iter().find(|k| k["current"] == true).and_then(|k| k["id"].as_str()).filter(|id| id_clave_valido(id)).map(str::to_string)
}

fn id_clave_valido(id: &str) -> bool {
    id.len() == 64 && id.chars().all(|c| c.is_ascii_hexdigit())
}

/// Borra el archivo de una clave del repositorio (en local: el almacén sí puede).
fn borrar_clave(carpeta: &Path, id: &str) {
    if id_clave_valido(id) {
        let _ = std::fs::remove_file(carpeta.join("keys").join(id));
    }
}

fn versiones_del_repo(acc: &crate::restic::Access) -> Result<Vec<crate::restic::Snapshot>, String> {
    let out = restic(acc, &["snapshots", "--json", "--no-lock"])?;
    if out.code != Some(0) {
        return Err(error_publico(&out));
    }
    serde_json::from_slice(&out.stdout).map_err(|_| "Respuesta inesperada de restic al listar las versiones.".to_string())
}

// ---------- Qué versiones quitar (sin fiarse de la hora que pone el equipo) ----------
//
// La hora de cada versión la escribe el equipo dueño en el repositorio, y un
// equipo comprometido puede subir (solo añadir) versiones falsas con la hora
// que quiera: con fecha futura, o la última hora de cada día, semana, mes y
// año ya pasados. `restic forget --keep-daily …` las tomaría como las más
// recientes de cada hueco y borraría las de verdad: el solo añadir no
// serviría de nada. Por eso el almacén decide aquí qué quitar y solo cuenta
// las versiones cuya hora cuadra con la de su subida (la del archivo
// `snapshots/<id>`, que escribe el rest-server del almacén): las demás
// («sospechosas») no se quitan ni desplazan a ninguna otra.

/// Una versión hecha antes de esto respecto a su subida no cuenta (una copia
/// muy larga, o traída de otro repositorio con `restic copy`).
const ANTES_DE_SUBIR_MAX_H: i64 = 48;
/// Ni una con la hora más de esto por delante de su subida (relojes).
const DESPUES_DE_SUBIR_MAX_H: i64 = 24;

/// Una versión del repositorio: su id, su grupo (como `restic forget`: equipo
/// y carpetas), la hora que dice y cuándo se subió al almacén.
#[derive(Clone, Debug)]
pub struct Version {
    pub id: String,
    pub grupo: String,
    pub hora: Option<DateTime<chrono::FixedOffset>>,
    pub subida: Option<DateTime<chrono::Utc>>,
}

impl Version {
    /// Con la hora de la versión y la del archivo en el almacén (`carpeta`).
    fn de(s: &crate::restic::Snapshot, carpeta: &Path) -> Version {
        let mut carpetas = s.paths.clone();
        carpetas.sort();
        let subida = id_clave_valido(&s.id)
            .then(|| std::fs::symlink_metadata(carpeta.join("snapshots").join(&s.id)).ok())
            .flatten()
            .filter(|m| m.is_file())
            .and_then(|m| m.modified().ok())
            .map(DateTime::<chrono::Utc>::from);
        Version {
            id: s.id.clone(),
            grupo: serde_json::to_string(&(&s.hostname, carpetas)).unwrap_or_default(),
            hora: DateTime::parse_from_rfc3339(&s.time).ok(),
            subida,
        }
    }

    /// ¿Su hora no cuadra con su subida (o no se sabe)? Entonces no se quita ni cuenta.
    pub fn sospechosa(&self) -> bool {
        match (self.hora, self.subida) {
            (Some(h), Some(s)) => {
                let h = h.with_timezone(&chrono::Utc);
                h > s + Duration::hours(DESPUES_DE_SUBIR_MAX_H) || h < s - Duration::hours(ANTES_DE_SUBIR_MAX_H)
            }
            _ => true,
        }
    }
}

/// Lo que se quita: los ids y cuántas sospechosas se dejaron sin tocar.
#[derive(Debug, Default, PartialEq)]
pub struct Plan {
    pub quitar: Vec<String>,
    pub sospechosas: usize,
}

pub(crate) type Hora = DateTime<chrono::FixedOffset>;
pub(crate) type Hueco = fn(&Hora) -> i64;

/// Los huecos de restic (`ymdh`, `ymd`, `yw`, `ym`, `y`), en la hora de la propia versión.
pub(crate) const HUECOS: [Hueco; 5] = [
    |t| (i64::from(t.year()) * 10_000 + i64::from(t.month()) * 100 + i64::from(t.day())) * 100 + i64::from(t.hour()),
    |t| i64::from(t.year()) * 10_000 + i64::from(t.month()) * 100 + i64::from(t.day()),
    |t| i64::from(t.iso_week().year()) * 100 + i64::from(t.iso_week().week()),
    |t| i64::from(t.year()) * 100 + i64::from(t.month()),
    |t| i64::from(t.year()),
];

/// `t.AddDate(-años, -meses, -días).Add(-horas)` de Go, como restic para los
/// plazos: el mes y el día se normalizan (31 de marzo menos un mes = 3 de marzo)
/// y, si la hora de la versión está en la zona de este equipo (Go la lee así),
/// se cuenta en hora local (con su cambio de horario); si no, con su desfase.
pub fn restar_plazo(t: &Hora, p: &Plazo) -> Option<Hora> {
    fn en<Tz: TimeZone>(zona: &Tz, t: &chrono::NaiveDateTime, p: &Plazo) -> Option<DateTime<Tz>> {
        let m0 = t.month0() as i32 - p.meses;
        let ano = t.year() - p.anos + m0.div_euclid(12);
        let primero = chrono::NaiveDate::from_ymd_opt(ano, m0.rem_euclid(12) as u32 + 1, 1)?;
        let dia = primero.checked_add_signed(Duration::days(i64::from(t.day()) - 1 - i64::from(p.dias)))?;
        let hora = dia.and_time(t.time());
        // Una hora que no existe (al adelantar el reloj): la de una hora después, menos una hora.
        let d = match zona.from_local_datetime(&hora) {
            chrono::LocalResult::Single(d) | chrono::LocalResult::Ambiguous(d, _) => d,
            chrono::LocalResult::None => zona.from_local_datetime(&(hora + Duration::hours(1))).earliest()? - Duration::hours(1),
        };
        Some(d - Duration::hours(i64::from(p.horas)))
    }
    let local = Local.offset_from_utc_datetime(&t.naive_utc()).fix() == *t.offset();
    if local {
        en(&Local, &t.with_timezone(&Local).naive_local(), p).map(|d| d.fixed_offset())
    } else {
        en(t.offset(), &t.naive_local(), p)
    }
}

/// Las reglas de `restic forget` (agrupando por equipo y carpetas, como restic
/// por defecto), solo sobre las versiones que no son sospechosas. De la más
/// reciente a la más antigua:
/// - **cantidades** (`--keep-hourly/daily/weekly/monthly/yearly`): la primera
///   de cada hora, día, semana ISO, mes y año mientras queden de ese tipo
///   (`SIEMPRE`: sin límite), y también la más antigua si aún queda alguna;
/// - **plazos** (`--keep-within-hourly 15d`…): la primera de cada hueco entre
///   las que son posteriores a «la más reciente menos el plazo» (la más
///   reciente de confianza: una falsa con fecha futura no mueve el plazo), y
///   también la más antigua si entra en él.
pub fn planear(versiones: &[Version], r: &Retencion) -> Plan {
    let sospechosas = versiones.iter().filter(|v| v.sospechosa()).count();
    let mut grupos: std::collections::BTreeMap<&str, Vec<(&Version, Hora)>> = Default::default();
    for v in versiones.iter().filter(|v| !v.sospechosa()) {
        if let Some(h) = v.hora {
            grupos.entry(v.grupo.as_str()).or_default().push((v, h));
        }
    }
    let plazos: Vec<Option<Plazo>> = r.plazos.lista().iter().map(|p| p.and_then(Plazo::leer)).collect();
    let mut quitar = Vec::new();
    for (_, mut lista) in grupos {
        lista.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.0.id.cmp(&a.0.id)));
        let Some(mas_reciente) = lista.first().map(|x| x.1) else { continue };
        let limites: Vec<Option<Hora>> = plazos.iter().map(|p| p.as_ref().and_then(|p| restar_plazo(&mas_reciente, p))).collect();
        let mut quedan: Vec<(i64, Option<i64>)> = r.cantidades().iter().map(|n| (*n, None)).collect();
        let mut en_plazo: Vec<Option<i64>> = vec![None; HUECOS.len()];
        let ultima = lista.len().saturating_sub(1);
        for (n, (v, t)) in lista.iter().enumerate() {
            let mut guardar = false;
            for (i, hueco) in HUECOS.iter().enumerate() {
                let (cuantas, anterior) = &mut quedan[i];
                if *cuantas > 0 || *cuantas == SIEMPRE {
                    let h = hueco(t);
                    if *anterior != Some(h) || n == ultima {
                        guardar = true;
                        *anterior = Some(h);
                        if *cuantas > 0 {
                            *cuantas -= 1;
                        }
                    }
                }
                if let Some(limite) = limites[i] {
                    if *t > limite {
                        let h = hueco(t);
                        if en_plazo[i] != Some(h) || n == ultima {
                            guardar = true;
                            en_plazo[i] = Some(h);
                        }
                    }
                }
            }
            if !guardar {
                quitar.push(v.id.clone());
            }
        }
    }
    Plan { quitar, sospechosas }
}

/// Pone al día la clave de la entrada: si abre, su id; y borra la anterior.
fn comprobar_clave(carpeta: &Path, e: &mut Entrada) -> Result<bool, String> {
    let acc = acceso(carpeta, &e.clave);
    let abre = clave_abre(&acc)?;
    e.clave_ok = Some(abre);
    if abre {
        if e.clave_id.is_none() {
            e.clave_id = id_de_la_clave(&acc);
        }
        if let Some(anterior) = e.clave_anterior.take() {
            if Some(&anterior) != e.clave_id.as_ref() {
                borrar_clave(carpeta, &anterior);
            }
        }
    }
    Ok(abre)
}

pub const SIN_CLAVE: &str = "La clave del almacén aún no abre este repositorio: falta que el equipo dueño la añada \
                             (en la consola, vuelve a guardar «Retención en el almacén» con el equipo encendido).";

/// Lo que pasó en una vuelta, para la bitácora («Retención en detalle»).
#[derive(Default)]
pub struct Detalle {
    /// Las versiones antes y después (`None` si no se pudieron leer).
    pub antes: Option<Vec<crate::restic::Snapshot>>,
    pub despues: Option<Vec<crate::restic::Snapshot>>,
    pub motivos: std::collections::HashMap<String, Option<String>>,
    pub liberado: Option<u64>,
    pub sospechosas: usize,
}

/// Aplica la retención de `e` en `carpeta`: comprueba la clave, `forget --prune`
/// con su regla y, si se pidió, `check`. Devuelve el mensaje (sin rutas).
pub fn ejecutar(carpeta: &Path, e: &mut Entrada) -> Result<String, String> {
    ejecutar_con(carpeta, e, &mut Detalle::default())
}

/// Lo mismo, apuntando en `d` lo que había, lo que quedó y lo que liberó.
pub fn ejecutar_con(carpeta: &Path, e: &mut Entrada, d: &mut Detalle) -> Result<String, String> {
    if !comprobar_clave(carpeta, e)? {
        return Err(SIN_CLAVE.into());
    }
    let acc = acceso(carpeta, &e.clave);
    let lista = versiones_del_repo(&acc)?;
    let antes = lista.len();
    let versiones: Vec<Version> = lista.iter().map(|s| Version::de(s, carpeta)).collect();
    let plan = planear(&versiones, &e.retencion);
    // Por qué se va cada una (solo las de confianza: las sospechosas no cuentan).
    let candidatas: Vec<crate::retencion_registro::Candidata> = versiones
        .iter()
        .filter(|v| !v.sospechosa())
        .filter_map(|v| Some(crate::retencion_registro::Candidata { id: v.id.clone(), grupo: v.grupo.clone(), hora: v.hora? }))
        .collect();
    d.motivos = crate::retencion_registro::motivos(&candidatas, &e.retencion);
    d.sospechosas = plan.sospechosas;
    d.antes = Some(lista);
    if !plan.quitar.is_empty() {
        // Por ids (lo decidido arriba), en tandas; después, un solo `prune`.
        for tanda in plan.quitar.chunks(100) {
            let mut args: Vec<&str> = vec!["forget", "--retry-lock", "30m", "--"];
            args.extend(tanda.iter().map(String::as_str));
            let out = restic(&acc, &args)?;
            if out.code != Some(0) {
                d.despues = versiones_del_repo(&acc).ok();
                return Err(format!("No se pudo aplicar la retención: {}", error_publico(&out)));
            }
        }
        let out = restic(&acc, &["prune", "--retry-lock", "30m"])?;
        d.despues = versiones_del_repo(&acc).ok();
        if out.code != Some(0) {
            return Err(format!("No se pudo aplicar la retención: {}", error_publico(&out)));
        }
        d.liberado = crate::retencion_registro::liberado(&String::from_utf8_lossy(&out.stdout));
    } else {
        d.despues = versiones_del_repo(&acc).ok();
        d.liberado = Some(0);
    }
    let despues = d.despues.as_ref().map_or(antes, Vec::len);
    e.versiones = Some(despues);
    let quitadas = antes.saturating_sub(despues);
    let mut m = match quitadas {
        0 => format!("Retención aplicada: ninguna versión sobraba (quedan {despues})."),
        1 => format!("Retención aplicada: 1 versión quitada, quedan {despues}."),
        n => format!("Retención aplicada: {n} versiones quitadas, quedan {despues}."),
    };
    match plan.sospechosas {
        0 => {}
        1 => m.push_str(" 1 versión con una fecha que no cuadra con su subida (traída de otro repositorio, o el reloj del equipo) no se toca."),
        n => m.push_str(&format!(" {n} versiones con una fecha que no cuadra con su subida (traídas de otro repositorio, o el reloj del equipo) no se tocan.")),
    }
    if e.verificar {
        let out = restic(&acc, &["check", "--retry-lock", "30m"])?;
        if out.code != Some(0) {
            return Err(format!("{m} Pero la comprobación encontró errores: {}", error_publico(&out)));
        }
        m.push_str(" Comprobado sin errores.");
    }
    Ok(m)
}

// ---------- Órdenes ----------

fn texto(c: &Value, k: &str) -> String {
    c[k].as_str().unwrap_or("").trim().to_string()
}

/// `retencion_almacen` (en el almacén, clave de administración):
/// - `{usuario, repo, clave?, retencion: {diarias, semanales, mensuales, anuales}, horario: {dias, hora}, verificar?}`
///   pone o cambia la regla (destructiva: espera). `clave` es la contraseña de
///   la clave propia del almacén; sin ella se conserva la que ya tiene.
/// - `{usuario, repo, quitar: true}` la quita y borra la clave del almacén del repositorio.
pub fn configurar(c: &Value) -> Result<String, String> {
    let (usuario, repo) = (texto(c, "usuario"), texto(c, "repo"));
    let carpeta = carpeta_en_este_almacen(&usuario, &repo)?;
    if c["quitar"] == true {
        let quitada = cambiar(|l| {
            let pos = l.iter().position(|e| e.es(&usuario, &repo));
            Ok(pos.map(|p| l.remove(p)))
        })?;
        let Some(e) = quitada else { return Ok(format!("«{usuario}/{repo}» no tenía retención en este almacén.")) };
        for id in e.clave_id.iter().chain(e.clave_anterior.iter()) {
            borrar_clave(&carpeta, id);
        }
        return Ok(format!("Este almacén ya no aplica la retención de «{usuario}/{repo}» y ha borrado su clave del repositorio."));
    }
    let retencion: Retencion = serde_json::from_value(c["retencion"].clone()).map_err(|e| format!("Retención no válida: {e}"))?;
    retencion.valida()?;
    let horario: Horario = serde_json::from_value(c["horario"].clone()).map_err(|_| "Falta el horario: {dias, hora}.".to_string())?;
    horario.valido()?;
    let verificar = c["verificar"].as_bool().unwrap_or(true);
    let clave = c["clave"].as_str().filter(|s| !s.is_empty()).map(str::to_string);
    if clave.as_ref().is_some_and(|k| k.chars().count() < 20 || k.chars().count() > 200 || k.chars().any(char::is_control)) {
        return Err("Clave del almacén no válida.".into());
    }
    let previa = cargar().into_iter().find(|e| e.es(&usuario, &repo));
    let mut e = match (previa, clave) {
        (Some(p), None) => Entrada { retencion, horario, verificar, ..p },
        (Some(p), Some(k)) if p.clave == k => Entrada { retencion, horario, verificar, ..p },
        (p, Some(k)) => Entrada {
            usuario: usuario.clone(),
            repo: repo.clone(),
            clave: k,
            clave_id: None,
            // Si cambia la clave, la anterior se borra cuando la nueva ya abra.
            clave_anterior: p.as_ref().and_then(|p| p.clave_id.clone().or(p.clave_anterior.clone())),
            clave_ok: None,
            retencion,
            horario,
            verificar,
            desde: String::new(),
            ultima: p.as_ref().and_then(|p| p.ultima.clone()),
            resultado: p.as_ref().and_then(|p| p.resultado.clone()),
            mensaje: p.as_ref().and_then(|p| p.mensaje.clone()),
            versiones: p.as_ref().and_then(|p| p.versiones),
        },
        (None, None) => return Err("Falta la clave del almacén para este repositorio.".into()),
    };
    e.desde = Local::now().to_rfc3339();
    let abre = comprobar_clave(&carpeta, &mut e)?;
    let m = format!("Este almacén aplicará la retención de «{usuario}/{repo}» ({}) {}.", e.retencion.texto(), e.horario.texto());
    cambiar(|l| {
        l.retain(|x| !x.es(&usuario, &repo));
        l.push(e);
        Ok(())
    })?;
    Ok(if abre { m } else { format!("{m} {SIN_CLAVE}") })
}

/// Repositorios con la retención aplicándose ahora (orden o horario).
static EN_MARCHA: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Aplica ahora la regla guardada de `usuario/repo` y anota el resultado (pedida con una orden).
pub fn aplicar(usuario: &str, repo: &str) -> Result<String, String> {
    aplicar_por(usuario, repo, "orden")
}

/// Lo mismo, diciendo en la bitácora quién la aplicó: `"orden"` o `"automatica"` (su horario).
pub fn aplicar_por(usuario: &str, repo: &str, por: &'static str) -> Result<String, String> {
    let carpeta = carpeta_en_este_almacen(usuario, repo)?;
    let mut e = cargar()
        .into_iter()
        .find(|e| e.es(usuario, repo))
        .ok_or_else(|| format!("«{usuario}/{repo}» no tiene retención en este almacén: configúrala antes."))?;
    let clave = format!("{usuario}/{repo}");
    {
        let mut en = EN_MARCHA.lock().map_err(|_| "Candado roto.".to_string())?;
        if en.contains(&clave) {
            return Err(format!("Ya se está aplicando la retención de «{clave}»."));
        }
        en.push(clave.clone());
    }
    let inicio = Local::now();
    let mut detalle = Detalle::default();
    let r = ejecutar_con(&carpeta, &mut e, &mut detalle);
    if let Some(antes) = &detalle.antes {
        crate::retencion_registro::anotar(&crate::retencion_registro::Vuelta {
            origen: "almacen",
            por,
            repo,
            usuario: Some(usuario),
            regla: Some(&e.retencion),
            inicio,
            antes,
            despues: detalle.despues.as_deref(),
            motivos: &detalle.motivos,
            copias: &Default::default(),
            liberado: detalle.liberado,
            sospechosas: Some(detalle.sospechosas),
            resultado: match &r {
                Ok(m) => Ok(m.as_str()),
                Err(m) => Err(m.as_str()),
            },
        });
    }
    if let Ok(mut en) = EN_MARCHA.lock() {
        en.retain(|x| *x != clave);
    }
    // Se anota sobre lo guardado ahora (la regla pudo cambiar mientras tanto).
    let _ = cambiar(|l| {
        if let Some(x) = l.iter_mut().find(|x| x.es(usuario, repo)) {
            if x.clave == e.clave {
                (x.clave_ok, x.clave_id, x.clave_anterior) = (e.clave_ok, e.clave_id.clone(), e.clave_anterior.clone());
            }
            x.versiones = e.versiones.or(x.versiones);
            x.ultima = Some(Local::now().to_rfc3339());
            x.resultado = Some(if r.is_ok() { "ok" } else { "fallo" }.into());
            x.mensaje = Some(match &r {
                Ok(m) | Err(m) => m.clone(),
            });
        }
        Ok(())
    });
    crate::agent::log(&match &r {
        Ok(m) => format!("Retención en el almacén de «{clave}»: {m}"),
        Err(m) => format!("ERROR: retención en el almacén de «{clave}»: {m}"),
    });
    r
}

/// `aplicar_retencion_almacen {usuario, repo}` (destructiva): la comprobación rápida; lo largo, en otro hilo.
pub fn validar_aplicar(c: &Value) -> Result<(String, String), String> {
    let (usuario, repo) = (texto(c, "usuario"), texto(c, "repo"));
    carpeta_en_este_almacen(&usuario, &repo)?;
    if !cargar().iter().any(|e| e.es(&usuario, &repo)) {
        return Err(format!("«{usuario}/{repo}» no tiene retención en este almacén: configúrala antes."));
    }
    Ok((usuario, repo))
}

/// Lo llama el servicio en cada vuelta: aplica, en otro hilo y de una en una,
/// las reglas a las que les toca.
pub fn si_toca() {
    if !crate::server::load().enabled {
        return;
    }
    let ahora = Local::now();
    let tocan: Vec<(String, String)> = cargar().into_iter().filter(|e| e.toca(ahora)).map(|e| (e.usuario, e.repo)).collect();
    if tocan.is_empty() {
        return;
    }
    static HILO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if HILO.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        for (u, r) in tocan {
            let _ = aplicar_por(&u, &r, "automatica");
        }
        HILO.store(false, std::sync::atomic::Ordering::SeqCst);
    });
}

/// `guarda_copias.retenciones` del resumen.
pub fn resumen() -> Vec<Value> {
    let ahora = Local::now();
    cargar().iter().map(|e| e.resumen(ahora)).collect()
}

// ---------- En el equipo dueño ----------

/// `clave_almacen {repo, clave}` (contraseña del repositorio y clave de
/// administración): añade al repositorio una clave de restic con esa
/// contraseña (`restic key add`), la que usará el almacén para aplicar la
/// retención. Es una escritura: vale con un servidor de solo añadir.
pub fn anadir_clave(v: &crate::servidor_v2::Vinculo, repo: &str, c: &Value) -> Result<String, String> {
    let r = v.repos_v2.iter().find(|r| r.id == repo).ok_or("Ese repositorio no lo gestiona este servidor.")?;
    if r.solo_lectura {
        return Err("Ese repositorio es de otro equipo (importado): su retención la decide el equipo que copia en él.".into());
    }
    let clave = c["clave"].as_str().unwrap_or("");
    if clave.chars().count() < 20 || clave.chars().count() > 200 || clave.chars().any(char::is_control) {
        return Err("Clave del almacén no válida.".into());
    }
    let acc = crate::gestion_v2::acceso(v, repo)?;
    crate::agent::prepare_dir()?;
    let archivo = crate::agent::private_dir().join(format!("clave-almacen-{}.tmp", uuid::Uuid::new_v4().simple()));
    std::fs::write(&archivo, clave).map_err(|e| format!("No se pudo preparar la clave: {e}"))?;
    let ruta = archivo.display().to_string();
    let out = crate::restic::run_raw(&acc, &["key", "add", "--new-password-file", &ruta, "--user", "resguardo-almacen"], std::time::Duration::from_secs(300));
    let _ = std::fs::remove_file(&archivo);
    let out = out?;
    if out.code != Some(0) {
        return Err(format!("No se pudo añadir la clave del almacén: {}", crate::web::public_message(&crate::restic::exit_error(out.code, &out.stderr))));
    }
    Ok(format!("Clave del almacén añadida a «{}»: el almacén ya puede aplicar su retención.", r.nombre))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn en(fecha: &str) -> DateTime<Local> {
        // Hora local del equipo (las pruebas valen en cualquier zona horaria).
        Local.from_local_datetime(&chrono::NaiveDateTime::parse_from_str(fecha, "%Y-%m-%d %H:%M").unwrap()).unwrap()
    }

    fn entrada(desde: &str) -> Entrada {
        Entrada {
            usuario: "caja-1".into(),
            repo: "caja".into(),
            clave: "x".repeat(32),
            clave_id: None,
            clave_anterior: None,
            clave_ok: None,
            retencion: Retencion { diarias: 7, semanales: 4, mensuales: 12, anuales: 2, ..Default::default() },
            // Los domingos a las 03:00.
            horario: Horario { dias: vec![7], hora: "03:00".into(), reglas: vec![] },
            verificar: true,
            desde: en(desde).to_rfc3339(),
            ultima: None,
            resultado: None,
            mensaje: None,
            versiones: None,
        }
    }

    #[test]
    fn horario_con_reglas() {
        use chrono::TimeZone;
        let r: Horario =
            serde_json::from_value(serde_json::json!({ "dias": [7], "hora": "03:00", "reglas": [{ "tipo": "mensual", "dia": 1, "hora": "04:00" }] })).unwrap();
        assert!(r.valido().is_ok());
        assert_eq!(r.texto(), "según su horario");
        let ahora = Local.with_ymd_and_hms(2026, 10, 7, 12, 0, 0).unwrap();
        assert_eq!(r.proximo(ahora), Local.with_ymd_and_hms(2026, 11, 1, 4, 0, 0).earliest());
        assert_eq!(r.ultimo(ahora), Local.with_ymd_and_hms(2026, 10, 1, 4, 0, 0).earliest());
        let malo: Horario =
            serde_json::from_value(serde_json::json!({ "dias": [7], "hora": "03:00", "reglas": [{ "tipo": "mensual", "dia": 31, "hora": "04:00" }] })).unwrap();
        assert!(malo.valido().is_err());
        // Sin reglas, como siempre (y no se escriben).
        let h: Horario = serde_json::from_value(serde_json::json!({ "dias": [7], "hora": "03:00" })).unwrap();
        assert!(h.reglas.is_empty() && !serde_json::to_string(&h).unwrap().contains("reglas"));
    }

    #[test]
    fn horario_semanal() {
        let h = Horario { dias: vec![7], hora: "03:00".into(), reglas: vec![] };
        assert_eq!(h.texto(), "los domingos a las 03:00");
        assert_eq!(Horario { dias: vec![4, 1], hora: "22:30".into(), reglas: vec![] }.texto(), "lunes y jueves a las 22:30");
        assert_eq!(Horario { dias: (1..=7).collect(), hora: "03:00".into(), reglas: vec![] }.texto(), "cada día a las 03:00");
        // Sábado 3 de octubre de 2026: el último domingo fue el 27 de septiembre; el próximo, el 4.
        let sab = Local.with_ymd_and_hms(2026, 10, 3, 12, 0, 0).unwrap();
        assert_eq!(h.ultimo(sab).unwrap(), Local.with_ymd_and_hms(2026, 9, 27, 3, 0, 0).unwrap());
        assert_eq!(h.proximo(sab).unwrap(), Local.with_ymd_and_hms(2026, 10, 4, 3, 0, 0).unwrap());
        // El mismo domingo, antes de la hora: el de la semana pasada.
        let dom = Local.with_ymd_and_hms(2026, 10, 4, 2, 0, 0).unwrap();
        assert_eq!(h.ultimo(dom).unwrap(), Local.with_ymd_and_hms(2026, 9, 27, 3, 0, 0).unwrap());
        assert!(Horario { dias: vec![], hora: "03:00".into(), reglas: vec![] }.valido().is_err());
        assert!(Horario { dias: vec![8], hora: "03:00".into(), reglas: vec![] }.valido().is_err());
        assert!(Horario { dias: vec![1], hora: "3:00".into(), reglas: vec![] }.valido().is_err());
        assert!(Horario { dias: vec![1], hora: "25:00".into(), reglas: vec![] }.valido().is_err());
    }

    #[test]
    fn cuando_toca() {
        // Puesta el sábado: no se aplica por el domingo anterior.
        let e = entrada("2026-10-03 10:00");
        assert!(!e.toca(Local.with_ymd_and_hms(2026, 10, 3, 23, 0, 0).unwrap()));
        // El domingo después de las 03:00, sí; también si el equipo estuvo apagado y es lunes.
        assert!(e.toca(Local.with_ymd_and_hms(2026, 10, 4, 3, 0, 0).unwrap()));
        assert!(e.toca(Local.with_ymd_and_hms(2026, 10, 5, 9, 0, 0).unwrap()));
        // Ya aplicada ese domingo (aunque fallara): hasta el siguiente, nada.
        let hecha = Entrada { ultima: Some(Local.with_ymd_and_hms(2026, 10, 4, 3, 5, 0).unwrap().to_rfc3339()), ..e.clone() };
        assert!(!hecha.toca(Local.with_ymd_and_hms(2026, 10, 10, 9, 0, 0).unwrap()));
        assert!(hecha.toca(Local.with_ymd_and_hms(2026, 10, 11, 3, 1, 0).unwrap()));
        // El resumen no lleva la clave.
        let r = e.resumen(Local.with_ymd_and_hms(2026, 10, 3, 12, 0, 0).unwrap());
        assert!(!r.to_string().contains(&e.clave));
        assert_eq!(r["horario_texto"], "los domingos a las 03:00");
        assert_eq!(r["clave"], "sin_probar");
        assert!(r["proxima"].as_str().unwrap().starts_with("2026-10-04T03:00:00"));
    }

    #[test]
    fn carpetas_validas() {
        let base = std::env::temp_dir().join(format!("resguardo-retalm-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(base.join("caja-1/caja")).unwrap();
        std::fs::write(base.join("caja-1/caja/config"), b"x").unwrap();
        std::fs::create_dir_all(base.join("caja-1/vacio")).unwrap();
        let u = vec!["caja-1".to_string()];
        assert!(carpeta_repo(&base, &u, "caja-1", "caja").is_ok());
        assert!(carpeta_repo(&base, &u, "otro", "caja").unwrap_err().contains("no es un equipo"));
        assert!(carpeta_repo(&base, &u, "caja-1", "vacio").unwrap_err().contains("No hay ningún repositorio"));
        for malo in ["..", "../caja-1/caja", "caja/../caja", "", "c:\\x", "a b"] {
            assert!(carpeta_repo(&base, &u, "caja-1", malo).is_err(), "{malo}");
        }
        assert!(carpeta_repo(&base, &["..".to_string()], "..", "caja").is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

    fn version(id: &str, hora: DateTime<Local>, subida: DateTime<Local>) -> Version {
        Version { id: id.into(), grupo: "PC|/datos".into(), hora: Some(hora.fixed_offset()), subida: Some(subida.with_timezone(&chrono::Utc)) }
    }

    /// Un equipo comprometido sube versiones falsas (solo puede añadir) con la
    /// hora que quiere: no desplazan a las de verdad.
    #[test]
    fn versiones_falsas_no_hacen_borrar_las_buenas() {
        let ahora = Local.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let regla = Retencion { diarias: 7, semanales: 4, mensuales: 6, anuales: 2, ..Default::default() };
        // Una copia buena cada día a las 03:00 durante 400 días, subida a los 10 minutos.
        let buenas: Vec<Version> = (0..400)
            .map(|d| {
                let h = Local.with_ymd_and_hms(2026, 10, 4, 3, 0, 0).unwrap() - Duration::days(d);
                version(&format!("{d:064x}"), h, h + Duration::minutes(10))
            })
            .collect();
        let normal = planear(&buenas, &regla);
        assert_eq!(normal.sospechosas, 0);
        assert!(!normal.quitar.is_empty() && normal.quitar.len() < 400);
        // Falsas, subidas ahora: la última hora de cada día pasado y fechas futuras.
        let mut con_falsas = buenas.clone();
        for d in 0..400 {
            let h = Local.with_ymd_and_hms(2026, 10, 4, 23, 59, 0).unwrap() - Duration::days(d);
            con_falsas.push(version(&format!("{:064x}", 1_000_000 + d), h, ahora));
        }
        for d in 2..50 {
            con_falsas.push(version(&format!("{:064x}", 2_000_000 + d), ahora + Duration::days(d), ahora));
        }
        let plan = planear(&con_falsas, &regla);
        let es_buena = |id: &str| buenas.iter().any(|b| b.id == id);
        let buenas_quitadas: Vec<&String> = plan.quitar.iter().filter(|id| es_buena(id)).collect();
        // De las buenas solo se quitan, además de las de siempre, las de los últimos días
        // que admite el margen de subida (48 h): las semanales, mensuales y anuales siguen ahí.
        let margen: Vec<String> = (0..3).map(|d| format!("{d:064x}")).collect();
        assert!(buenas_quitadas.iter().all(|id| normal.quitar.contains(id) || margen.contains(id)), "{buenas_quitadas:?}");
        // Las sospechosas (subidas mucho después de su hora, o con fecha futura) nunca se quitan.
        assert!(plan.quitar.iter().all(|id| es_buena(id) || con_falsas.iter().any(|v| v.id == **id && !v.sospechosa())));
        assert_eq!(plan.sospechosas, 397 + 48);
        // Sin saber cuándo se subió, tampoco se quita.
        let sin_subida = Version { subida: None, ..buenas[300].clone() };
        assert!(sin_subida.sospechosa());
        assert!(planear(&[sin_subida], &regla).quitar.is_empty());
    }

    fn siigo() -> Retencion {
        Retencion {
            mensuales: SIEMPRE,
            plazos: crate::gestion_v2::Plazos { horarias: Some("15d".into()), diarias: Some("1y".into()), ..Default::default() },
            ..Default::default()
        }
    }

    /// `AddDate` de Go: el mes y el día se normalizan como en Go (y restic).
    #[test]
    fn restar_plazos_como_go() {
        // Un desfase que no es el de este equipo: se cuenta con ese desfase.
        let zona = if Local::now().offset().fix().local_minus_utc() == 13 * 3600 + 45 * 60 { "+05:30" } else { "+13:45" };
        let t = |s: &str| DateTime::parse_from_rfc3339(&format!("{s}{zona}")).unwrap();
        let p = |s: &str| Plazo::leer(s).unwrap();
        assert_eq!(restar_plazo(&t("2026-03-31T10:00:00"), &p("1m")), Some(t("2026-03-03T10:00:00")), "31 de febrero = 3 de marzo");
        assert_eq!(restar_plazo(&t("2024-03-31T10:00:00"), &p("1m")), Some(t("2024-03-02T10:00:00")), "bisiesto");
        assert_eq!(restar_plazo(&t("2026-01-15T10:00:00"), &p("1y2m")), Some(t("2024-11-15T10:00:00")));
        assert_eq!(restar_plazo(&t("2026-10-04T03:00:00"), &p("15d")), Some(t("2026-09-19T03:00:00")));
        assert_eq!(restar_plazo(&t("2026-10-04T03:00:00"), &p("1d36h")), Some(t("2026-10-01T15:00:00")));
        assert_eq!(restar_plazo(&t("2028-02-29T00:30:00"), &p("1y")), Some(t("2027-03-01T00:30:00")));
        // Los plazos que valen (los de restic: y, m, d, h; nunca cero).
        assert_eq!(Plazo::leer("1y6m"), Some(Plazo { anos: 1, meses: 6, ..Default::default() }));
        for malo in ["", "0d", "15", "d", "15w", "1.5d", "-1d", "15 d", "999999999999d", "300y"] {
            assert_eq!(Plazo::leer(malo), None, "{malo}");
        }
        assert_eq!(p("1y6m").texto(), "1 año y 6 meses");
        assert_eq!(p("15d").texto(), "15 días");
        assert_eq!(siigo().texto(), "horarias 15 días · diarias 1 año · mensuales siempre");
        assert_eq!(
            Retencion { diarias: 7, semanales: 4, mensuales: 12, anuales: 2, ..Default::default() }.texto(),
            "7 diarias · 4 semanales · 12 mensuales · 2 anuales"
        );
    }

    /// «Programas contables» con una copia cada hora durante 2 años: las horarias de los
    /// últimos 15 días, una por día del último año y una por mes de antes. Y
    /// las falsas (con fecha futura) no mueven el plazo.
    #[test]
    fn plazos_como_siigo() {
        let ultima = Local.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let buenas: Vec<Version> = (0..2 * 365 * 24)
            .map(|h| {
                let t = ultima - Duration::hours(h);
                version(&format!("{h:064x}"), t, t + Duration::minutes(5))
            })
            .collect();
        let plan = planear(&buenas, &siigo());
        let quedan: Vec<&Version> = buenas.iter().filter(|v| !plan.quitar.contains(&v.id)).collect();
        // Todas las horas de los últimos 15 días (360, contando la más reciente) …
        let hace_15 = ultima - Duration::days(15);
        assert_eq!(quedan.iter().filter(|v| v.hora.unwrap() > hace_15).count(), 360);
        // … una por día del resto del año (la última de cada día) …
        let hace_1y = Local.with_ymd_and_hms(2025, 10, 4, 12, 0, 0).unwrap();
        let del_ano: Vec<&&Version> = quedan.iter().filter(|v| v.hora.unwrap() <= hace_15 && v.hora.unwrap() > hace_1y).collect();
        assert!((349..=351).contains(&del_ano.len()), "{}", del_ano.len());
        assert!(del_ano.iter().all(|v| v.hora.unwrap().hour() == 23 || v.hora.unwrap().date_naive() == hace_1y.date_naive()));
        // … y una por mes de antes (la última de cada mes, y la más antigua).
        let antes = quedan.iter().filter(|v| v.hora.unwrap() <= hace_1y).count();
        assert!((12..=14).contains(&antes), "{antes}");
        assert!(quedan.len() < 730, "{}", quedan.len());

        // Falsas con fecha futura (subidas ahora): sospechosas, no mueven «la más reciente».
        let mut con_falsas = buenas.clone();
        for d in 1..40 {
            con_falsas.push(version(&format!("{:064x}", 9_000_000 + d), ultima + Duration::days(d * 30), ultima));
        }
        let con = planear(&con_falsas, &siigo());
        assert_eq!(con.sospechosas, 39);
        let mut a = plan.quitar.clone();
        let mut b = con.quitar.clone();
        a.sort();
        b.sort();
        assert_eq!(a, b, "las falsas no cambian lo que se quita");
    }

    /// Más reglas (o cantidades y plazos mayores) nunca quitan más.
    #[test]
    fn mas_reglas_nunca_quitan_mas() {
        use proptest::prelude::*;
        let base = Local.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let regla = (
            proptest::collection::vec(prop_oneof![Just(0i64), 1..6i64, Just(SIEMPRE)], 5),
            proptest::collection::vec(proptest::option::of(prop_oneof![Just("36h"), Just("3d"), Just("20d"), Just("2m"), Just("1y")]), 5),
        );
        proptest!(ProptestConfig::with_cases(64), |(horas in proptest::collection::vec(0i64..20_000, 1..120), a in regla.clone(), extra in regla)| {
            let vs: Vec<Version> = horas.iter().enumerate().map(|(i, h)| {
                let t = base - Duration::hours(*h) - Duration::minutes(i as i64 % 50);
                version(&format!("{i:064x}"), t, t + Duration::minutes(5))
            }).collect();
            let mk = |(n, p): &(Vec<i64>, Vec<Option<&str>>)| Retencion {
                horarias: n[0], diarias: n[1], semanales: n[2], mensuales: n[3], anuales: n[4],
                plazos: crate::gestion_v2::Plazos {
                    horarias: p[0].map(Into::into), diarias: p[1].map(Into::into), semanales: p[2].map(Into::into), mensuales: p[3].map(Into::into), anuales: p[4].map(Into::into),
                },
            };
            let r1 = mk(&a);
            // r2 = r1 con más: cada cantidad la mayor (SIEMPRE gana) y cada plazo, el de r1 o el extra.
            let mut r2 = r1.clone();
            let mas = |x: i64, y: i64| if x == SIEMPRE || y == SIEMPRE { SIEMPRE } else { x.max(y) };
            r2.horarias = mas(r1.horarias, extra.0[0]);
            r2.diarias = mas(r1.diarias, extra.0[1]);
            r2.semanales = mas(r1.semanales, extra.0[2]);
            r2.mensuales = mas(r1.mensuales, extra.0[3]);
            r2.anuales = mas(r1.anuales, extra.0[4]);
            let mayor = |x: &Option<String>, y: Option<&str>| match (x.as_deref(), y) {
                (None, y) => y.map(String::from),
                (Some(x), None) => Some(x.to_string()),
                (Some(x), Some(y)) => {
                    let h = |s: &str| { let p = Plazo::leer(s).unwrap(); i64::from(p.anos) * 8766 + i64::from(p.meses) * 744 + i64::from(p.dias) * 24 + i64::from(p.horas) };
                    Some(if h(x) >= h(y) { x } else { y }.to_string())
                }
            };
            r2.plazos.horarias = mayor(&r1.plazos.horarias, extra.1[0]);
            r2.plazos.diarias = mayor(&r1.plazos.diarias, extra.1[1]);
            r2.plazos.semanales = mayor(&r1.plazos.semanales, extra.1[2]);
            r2.plazos.mensuales = mayor(&r1.plazos.mensuales, extra.1[3]);
            r2.plazos.anuales = mayor(&r1.plazos.anuales, extra.1[4]);
            let p1 = planear(&vs, &r1);
            let p2 = planear(&vs, &r2);
            prop_assert!(p2.quitar.iter().all(|id| p1.quitar.contains(id)), "{r1:?} → {r2:?}");
        });
    }

    /// Repositorio local con versiones de verdad (`restic backup --time`) en esas
    /// horas, de dos equipos, cada una «subida» a los 5 minutos de su hora.
    fn repo_con_versiones(nombre: &str, horas: &[DateTime<Local>]) -> (PathBuf, PathBuf, crate::restic::Access, Vec<crate::restic::Snapshot>) {
        let base = std::env::temp_dir().join(format!("resguardo-retalm-{nombre}-{}", uuid::Uuid::new_v4().simple()));
        let carpeta = base.join("caja-1").join("caja");
        let datos = base.join("datos");
        std::fs::create_dir_all(&datos).unwrap();
        let acc = crate::restic::Access::new(carpeta.display().to_string(), "contraseña del repo");
        assert_eq!(crate::restic::run_raw(&acc, &["init"], LIMITE).unwrap().code, Some(0));
        for (i, t) in horas.iter().enumerate() {
            std::fs::write(datos.join("f.txt"), format!("versión {i}")).unwrap();
            let host = if i % 4 == 0 { "OTRO" } else { "CAJA-1" };
            let cuando = t.format("%Y-%m-%d %H:%M:%S").to_string();
            let d = datos.display().to_string();
            let b = crate::restic::run_raw(&acc, &["backup", "--host", host, "--time", &cuando, &d], LIMITE).unwrap();
            assert_eq!(b.code, Some(0), "{}", b.stderr);
        }
        let lista = versiones_del_repo(&acc).unwrap();
        for s in &lista {
            let t = DateTime::parse_from_rfc3339(&s.time).unwrap() + Duration::minutes(5);
            let ruta = carpeta.join("snapshots").join(&s.id);
            let mut permisos = std::fs::metadata(&ruta).unwrap().permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            permisos.set_readonly(false);
            std::fs::set_permissions(&ruta, permisos).unwrap();
            let f = std::fs::File::options().write(true).open(&ruta).unwrap();
            f.set_modified(std::time::SystemTime::from(t)).unwrap();
        }
        (base, carpeta, acc, lista)
    }

    /// Lo que quitaría `restic forget --dry-run` con esa regla.
    fn quita_restic(acc: &crate::restic::Access, r: &Retencion) -> Vec<String> {
        let mut args = vec!["forget".to_string(), "--dry-run".into(), "--json".into(), "--no-lock".into()];
        args.extend(r.politica().args());
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = crate::restic::run_raw(acc, &refs, LIMITE).unwrap();
        assert_eq!(out.code, Some(0), "{}", out.stderr);
        let grupos: Vec<Value> = serde_json::from_slice(&out.stdout).unwrap();
        let mut v: Vec<String> =
            grupos.iter().flat_map(|g| g["remove"].as_array().cloned().unwrap_or_default()).map(|s| s["id"].as_str().unwrap().to_string()).collect();
        v.sort();
        v
    }

    /// Plazos y horarias: `planear` decide lo mismo que `restic forget --dry-run`
    /// con reglas al azar (y la de los programas contables) sobre versiones de verdad.
    #[test]
    fn plazos_como_restic_forget() {
        let _real = crate::restic::tests::real_repo_lock();
        if crate::restic::version().is_err() {
            return;
        }
        // Varias por hora en los dos últimos días, unas cada día en el último mes
        // y mes y medio, y otras espaciadas durante dos años y medio.
        let ultima = Local.with_ymd_and_hms(2026, 3, 31, 18, 20, 0).unwrap();
        let mut horas: Vec<DateTime<Local>> = Vec::new();
        for i in 0..16 {
            horas.push(ultima - Duration::minutes(i * 37 + (i % 3) * 11));
        }
        for d in 1..40 {
            horas.push(ultima - Duration::days(d) - Duration::hours(d % 7));
        }
        for k in 1..22 {
            horas.push(ultima - Duration::days(40 + k * 41) + Duration::hours(k % 5));
        }
        let (base, carpeta, acc, lista) = repo_con_versiones("plazos", &horas);
        let versiones: Vec<Version> = lista.iter().map(|s| Version::de(s, &carpeta)).collect();
        assert!(versiones.iter().all(|v| !v.sospechosa()));

        let mut reglas = vec![
            siigo(),
            Retencion {
                plazos: crate::gestion_v2::Plazos { diarias: Some("30d".into()), semanales: Some("6m".into()), ..Default::default() },
                ..Default::default()
            },
            Retencion { horarias: 5, diarias: 3, mensuales: SIEMPRE, ..Default::default() },
            Retencion {
                plazos: crate::gestion_v2::Plazos {
                    horarias: Some("36h".into()),
                    mensuales: Some("1y2m".into()),
                    anuales: Some("2y".into()),
                    ..Default::default()
                },
                ..Default::default()
            },
        ];
        // Y reglas al azar (siempre las mismas).
        let mut semilla: u64 = 0x5eed_1234;
        let mut azar = |n: u64| {
            semilla = semilla.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            (semilla >> 33) % n
        };
        const CANT: [i64; 6] = [0, 0, 1, 2, 4, SIEMPRE];
        const PLAZ: [Option<&str>; 8] = [None, None, None, Some("1d"), Some("40h"), Some("12d"), Some("3m"), Some("1y")];
        while reglas.len() < 28 {
            let mut n = [0i64; 5];
            let mut p: [Option<String>; 5] = Default::default();
            for i in 0..5 {
                n[i] = CANT[azar(6) as usize];
                p[i] = PLAZ[azar(8) as usize].map(String::from);
            }
            let [ph, pd, pw, pm, py] = p;
            let r = Retencion {
                horarias: n[0],
                diarias: n[1],
                semanales: n[2],
                mensuales: n[3],
                anuales: n[4],
                plazos: crate::gestion_v2::Plazos { horarias: ph, diarias: pd, semanales: pw, mensuales: pm, anuales: py },
            };
            if r.valida().is_ok() {
                reglas.push(r);
            }
        }
        let mut con_algo = 0;
        for r in &reglas {
            let mut nuestro = planear(&versiones, r).quitar;
            nuestro.sort();
            con_algo += usize::from(!nuestro.is_empty());
            assert_eq!(nuestro, quita_restic(&acc, r), "lo mismo que restic forget con {r:?}");
        }
        assert!(con_algo >= reglas.len() / 2, "reglas que quitan algo: {con_algo} de {}", reglas.len());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Las reglas de `planear` son las de `restic forget` (con restic de verdad).
    #[test]
    fn planear_como_restic_forget() {
        let _real = crate::restic::tests::real_repo_lock();
        if crate::restic::version().is_err() {
            return;
        }
        let base = std::env::temp_dir().join(format!("resguardo-retalm-plan-{}", uuid::Uuid::new_v4().simple()));
        let carpeta = base.join("caja-1").join("caja");
        let datos = base.join("datos");
        std::fs::create_dir_all(&datos).unwrap();
        let acc = crate::restic::Access::new(carpeta.display().to_string(), "contraseña del repo");
        assert_eq!(crate::restic::run_raw(&acc, &["init"], LIMITE).unwrap().code, Some(0));
        // 30 versiones repartidas en 400 días (a veces dos el mismo día) y de dos equipos.
        for i in 0..30i64 {
            std::fs::write(datos.join("f.txt"), format!("versión {i}")).unwrap();
            let t = Local.with_ymd_and_hms(2025, 9, 1, 8, 0, 0).unwrap() + Duration::days(i * 13 + (i % 3)) + Duration::hours(i % 5);
            let host = if i % 4 == 0 { "OTRO" } else { "CAJA-1" };
            let cuando = t.format("%Y-%m-%d %H:%M:%S").to_string();
            let d = datos.display().to_string();
            let b = crate::restic::run_raw(&acc, &["backup", "--host", host, "--time", &cuando, &d], LIMITE).unwrap();
            assert_eq!(b.code, Some(0), "{}", b.stderr);
        }
        // Cada archivo de versión, «subido» a los 5 minutos de su hora.
        let lista = versiones_del_repo(&acc).unwrap();
        for s in &lista {
            let t = DateTime::parse_from_rfc3339(&s.time).unwrap() + Duration::minutes(5);
            let ruta = carpeta.join("snapshots").join(&s.id);
            // restic deja sus archivos de solo lectura.
            let mut permisos = std::fs::metadata(&ruta).unwrap().permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            permisos.set_readonly(false);
            std::fs::set_permissions(&ruta, permisos).unwrap();
            let f = std::fs::File::options().write(true).open(&ruta).unwrap();
            f.set_modified(std::time::SystemTime::from(t)).unwrap();
        }
        let regla = Retencion { diarias: 3, semanales: 2, mensuales: 4, anuales: 1, ..Default::default() };
        let versiones: Vec<Version> = lista.iter().map(|s| Version::de(s, &carpeta)).collect();
        let plan = planear(&versiones, &regla);
        assert_eq!(plan.sospechosas, 0);
        let mut args = vec!["forget".to_string(), "--dry-run".into(), "--json".into(), "--no-lock".into()];
        args.extend(regla.politica().args());
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = crate::restic::run_raw(&acc, &refs, LIMITE).unwrap();
        assert_eq!(out.code, Some(0), "{}", out.stderr);
        let grupos: Vec<Value> = serde_json::from_slice(&out.stdout).unwrap();
        let mut de_restic: Vec<String> =
            grupos.iter().flat_map(|g| g["remove"].as_array().cloned().unwrap_or_default()).map(|s| s["id"].as_str().unwrap().to_string()).collect();
        let mut nuestro = plan.quitar.clone();
        de_restic.sort();
        nuestro.sort();
        assert!(!nuestro.is_empty());
        assert_eq!(nuestro, de_restic, "lo mismo que restic forget");

        // Un equipo comprometido añade versiones falsas con fecha de ayer y futura: con
        // la clave del almacén, la retención no borra ninguna de las buenas.
        let e_clave = "z".repeat(32);
        let f = base.join("nueva.txt");
        std::fs::write(&f, &e_clave).unwrap();
        let add = crate::restic::run_raw(&acc, &["key", "add", "--new-password-file", &f.display().to_string()], LIMITE).unwrap();
        assert_eq!(add.code, Some(0), "{}", add.stderr);
        for (i, t) in [Local::now() + Duration::days(40), Local::now() - Duration::days(30), Local::now() - Duration::days(400)].iter().enumerate() {
            std::fs::write(datos.join("f.txt"), format!("basura {i}")).unwrap();
            let cuando = t.format("%Y-%m-%d %H:%M:%S").to_string();
            let d = datos.display().to_string();
            assert_eq!(crate::restic::run_raw(&acc, &["backup", "--host", "CAJA-1", "--time", &cuando, &d], LIMITE).unwrap().code, Some(0));
        }
        let mut e =
            Entrada { usuario: "caja-1".into(), repo: "caja".into(), clave: e_clave, verificar: false, retencion: regla, ..entrada("2026-10-03 10:00") };
        let m = ejecutar(&carpeta, &mut e).unwrap();
        assert!(m.contains("3 versiones con una fecha que no cuadra"), "{m}");
        let quedan: Vec<String> = versiones_del_repo(&acc).unwrap().into_iter().map(|s| s.id).collect();
        let buenas_quedan: Vec<&String> = lista.iter().map(|s| &s.id).filter(|id| !plan.quitar.contains(id)).collect();
        assert!(buenas_quedan.iter().all(|id| quedan.contains(id)), "las buenas que tocaba guardar siguen ahí");
        assert_eq!(quedan.len(), buenas_quedan.len() + 3, "y las falsas no se borran (se ven en el mensaje)");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Con restic de verdad, en un repositorio local: la clave propia, podar con la regla y revocar.
    #[test]
    fn retencion_real_con_clave_propia() {
        let _real = crate::restic::tests::real_repo_lock();
        if crate::restic::version().is_err() {
            return;
        }
        let base = std::env::temp_dir().join(format!("resguardo-retalm-real-{}", uuid::Uuid::new_v4().simple()));
        let carpeta = base.join("caja-1").join("caja");
        let datos = base.join("datos");
        std::fs::create_dir_all(&datos).unwrap();
        let dueno = crate::restic::Access::new(carpeta.display().to_string(), "contraseña del equipo dueño");
        assert_eq!(crate::restic::run_raw(&dueno, &["init"], LIMITE).unwrap().code, Some(0));
        for i in 0..3 {
            std::fs::write(datos.join("f.txt"), format!("versión {i}")).unwrap();
            let b = crate::restic::run_raw(&dueno, &["backup", "--host", "CAJA-1", &datos.display().to_string()], LIMITE).unwrap();
            assert_eq!(b.code, Some(0), "{}", b.stderr);
        }
        let mut e = entrada("2026-10-03 10:00");
        e.retencion = Retencion { diarias: 1, ..Default::default() };
        // Sin la clave añadida, no abre (y no toca nada).
        assert_eq!(ejecutar(&carpeta, &mut e).unwrap_err(), SIN_CLAVE);
        assert_eq!(e.clave_ok, Some(false));
        // El dueño añade la clave del almacén (como hace `clave_almacen`).
        let f = base.join("nueva.txt");
        std::fs::write(&f, &e.clave).unwrap();
        let add =
            crate::restic::run_raw(&dueno, &["key", "add", "--new-password-file", &f.display().to_string(), "--user", "resguardo-almacen"], LIMITE).unwrap();
        assert_eq!(add.code, Some(0), "{}", add.stderr);
        let m = ejecutar(&carpeta, &mut e).unwrap();
        assert!(m.contains("2 versiones quitadas, quedan 1") && m.contains("Comprobado"), "{m}");
        assert!(!m.contains(&base.display().to_string()), "sin rutas: {m}");
        let id = e.clave_id.clone().expect("id de la clave del almacén");
        assert!(carpeta.join("keys").join(&id).is_file());
        // Cambiar la clave: la anterior se borra cuando la nueva abre.
        let nueva = "y".repeat(32);
        std::fs::write(&f, &nueva).unwrap();
        assert_eq!(crate::restic::run_raw(&dueno, &["key", "add", "--new-password-file", &f.display().to_string()], LIMITE).unwrap().code, Some(0));
        let mut e2 = Entrada { clave: nueva, clave_id: None, clave_anterior: Some(id.clone()), ..e.clone() };
        ejecutar(&carpeta, &mut e2).unwrap();
        assert!(!carpeta.join("keys").join(&id).exists(), "la clave anterior ya no está");
        assert!(e.clave_id != e2.clave_id);
        // Revocar: sin su archivo, la clave ya no abre; la del dueño sigue abriendo.
        borrar_clave(&carpeta, e2.clave_id.as_deref().unwrap());
        assert!(!clave_abre(&acceso(&carpeta, &e2.clave)).unwrap());
        assert_eq!(crate::restic::snapshots(&dueno).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&base);
    }
}
