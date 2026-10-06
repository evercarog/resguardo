//! Espejo del Servidor de copias (copia externa): lo que guarda el
//! rest-server de este equipo se copia, archivo a archivo, a uno o varios
//! destinos: otra carpeta (p. ej. el segundo disco del almacén) o una nube
//! conectada con rclone (Dropbox, Google Drive; nube.rs, que sube con
//! `rclone copy --immutable`: nunca `sync`, nunca reescribe ni borra).
//! Diseño completo en docs/espejo.md.
//!
//! Es seguro con repositorios de restic en «solo añadir»: sus archivos no
//! cambian una vez escritos (su nombre es su hash). Así que:
//! - nunca se borra nada en el espejo (lo borrado en el origen, que en solo
//!   añadir no debería pasar, sigue en el espejo);
//! - no se copian los `locks` ni lo modificado en los últimos 10 minutos (una
//!   subida a medias: entrará en la vuelta siguiente);
//! - cada archivo se copia a un nombre temporal y se renombra al final;
//! - un archivo que ya está con el mismo tamaño no se vuelve a copiar; si
//!   está con otro tamaño, tampoco se reemplaza (igual que en la nube con
//!   `--immutable`): alguien ha cambiado una copia ya escrita, y el espejo
//!   termina con error para que se revise.
//!
//! Cuándo (docs/espejo.md §3a): cada destino con su horario (el de las
//! copias, `gestion_v2::Horario`) o, sin él, cada día a `hora`; y, si se
//! pide, «después de cada copia nueva» (una versión nueva en `snapshots/`).
//!
//! No necesita las contraseñas de los repositorios: no abre nada, solo copia
//! archivos cifrados.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Lo que se ha modificado hace menos de esto se deja para la próxima vuelta.
const RECIENTE: Duration = Duration::from_secs(10 * 60);

/// Un destino del espejo.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Destino {
    /// "carpeta" (ruta local completa) o "nube" (carpeta dentro de la nube `nube`).
    pub tipo: String,
    pub carpeta: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nube: Option<String>,
    #[serde(default)]
    pub ultima: Option<String>,
    #[serde(default)]
    pub resultado: Option<String>,
    /// v1.31: el espacio de la nube (`rclone about`) tras el último espejo a
    /// ella, y cuándo se leyó. Las carpetas se miden al hacer el resumen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cuota: Option<(crate::espacio::Espacio, String)>,
    /// §3a: cuándo, con el horario de las copias. Sin él, cada día a `Espejo::hora`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horario: Option<crate::gestion_v2::Horario>,
    /// §3a: también «después de cada copia nueva» (una versión nueva en `snapshots/`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tras_copia: bool,
    /// Cuándo empezó la última vuelta a este destino (RFC 3339): desde ahí se
    /// cuenta el horario y lo que es «una copia nueva».
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inicio: Option<String>,
}

/// Lo que se espera desde la última versión nueva antes de empezar el espejo
/// «después de cada copia» (agrupa varias copias seguidas; más que [`RECIENTE`],
/// para que la última ya se copie).
pub const ESPERA_TRAS_COPIA: Duration = Duration::from_secs(12 * 60);
/// Como mucho, lo que se espera desde la primera versión sin copiar aunque sigan llegando.
pub const ESPERA_MAXIMA_TRAS_COPIA: Duration = Duration::from_secs(60 * 60);

impl Destino {
    /// El horario de este destino: el suyo o, sin él, cada día a `hora`.
    pub fn plan(&self, hora: &str) -> Option<crate::plans::PlanSchedule> {
        match &self.horario {
            Some(h) => h.plan_schedule().ok(),
            None => {
                chrono::NaiveTime::parse_from_str(hora, "%H:%M").ok()?;
                Some(crate::plans::PlanSchedule {
                    days: (0..7).collect(),
                    mode: "at".into(),
                    times: vec![hora.to_string()],
                    every_hours: 1,
                    from: String::new(),
                    to: String::new(),
                    rules: vec![],
                })
            }
        }
    }

    /// Desde cuándo se cuenta: el comienzo de la última vuelta o, en las de
    /// antes (sin `inicio`), cuándo terminó (RFC 3339, o solo AAAA-MM-DD).
    pub fn desde(&self) -> Option<chrono::DateTime<chrono::Local>> {
        let t = self.inicio.as_deref().or(self.ultima.as_deref())?;
        if let Ok(d) = chrono::DateTime::parse_from_rfc3339(t) {
            return Some(d.with_timezone(&chrono::Local));
        }
        let dia = chrono::NaiveDate::parse_from_str(t.get(..10)?, "%Y-%m-%d").ok()?;
        chrono::TimeZone::from_local_datetime(&chrono::Local, &dia.and_hms_opt(0, 0, 0)?).earliest()
    }

    pub fn mismo(&self, o: &Destino) -> bool {
        self.tipo == o.tipo && self.carpeta == o.carpeta && self.nube == o.nube
    }
    pub fn texto(&self) -> String {
        match &self.nube {
            Some(n) => format!("{n}:{}", self.carpeta),
            None => self.carpeta.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Espejo {
    /// Forma de 0.7.0 (una sola carpeta): se lee y pasa a `destinos`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub carpeta: String,
    /// «HH:MM», hora local.
    pub hora: String,
    /// Cuándo terminó la última vuelta (RFC 3339; versiones anteriores: solo AAAA-MM-DD).
    #[serde(default)]
    pub ultima: Option<String>,
    #[serde(default)]
    pub resultado: Option<String>,
    #[serde(default)]
    pub destinos: Vec<Destino>,
    /// Límite de subida a la nube, en KiB/s (no se aplica a las carpetas).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limite_kib: Option<u32>,
}

impl Espejo {
    /// Los destinos, contando la forma antigua de una sola carpeta.
    pub fn destinos(&self) -> Vec<Destino> {
        let mut d = self.destinos.clone();
        if d.is_empty() && !self.carpeta.is_empty() {
            d.push(Destino {
                tipo: "carpeta".into(),
                carpeta: self.carpeta.clone(),
                nube: None,
                ultima: self.ultima.clone(),
                resultado: self.resultado.clone(),
                ..Default::default()
            });
        }
        d
    }

    /// Pasa la forma antigua a `destinos`.
    pub fn normalizar(&mut self) {
        if self.destinos.is_empty() && !self.carpeta.is_empty() {
            self.destinos = self.destinos();
        }
        self.carpeta.clear();
    }

    /// ¿Quita `nuevo` alguno de los destinos de este espejo? (orden destructiva)
    pub fn quita_destinos(&self, nuevo: Option<&Espejo>) -> bool {
        let nuevos = nuevo.map(Espejo::destinos).unwrap_or_default();
        self.destinos().iter().any(|d| !nuevos.iter().any(|n| n.mismo(d)))
    }

    /// Lo que se ve en el resumen de la consola (sin secretos).
    pub fn resumen(&self) -> serde_json::Value {
        let destinos: Vec<_> = self
            .destinos()
            .iter()
            .map(|d| {
                // v1.31: libre y total (carpeta: su volumen, ahora; nube: la cuenta, tras el último espejo).
                let espacio = match (&d.tipo[..], &d.cuota) {
                    ("carpeta", _) => crate::espacio::json_de(&d.carpeta),
                    (_, Some((e, leido))) => e.json(leido),
                    _ => serde_json::Value::Null,
                };
                let mut v = serde_json::json!({ "tipo": d.tipo, "carpeta": d.carpeta, "nube": d.nube, "ultima": d.ultima, "resultado": d.resultado, "espacio": espacio });
                // §3a: su horario (si tiene uno propio), «después de cada copia» y la próxima vuelta por horario.
                if let Some(h) = &d.horario {
                    v["horario"] = serde_json::to_value(h).unwrap_or_default();
                }
                v["tras_copia"] = d.tras_copia.into();
                v["proxima"] = d.plan(&self.hora).and_then(|p| p.next_slot(chrono::Local::now())).map(|t| t.to_rfc3339()).into();
                v
            })
            .collect();
        serde_json::json!({
            "hora": self.hora, "ultima": self.ultima, "resultado": self.resultado, "limite_kib": self.limite_kib, "destinos": destinos,
        })
    }
}

/// El espejo pedido en una orden `guarda_copias {espejo: …}`:
/// - `null` (o sin carpeta ni destinos): quitarlo;
/// - `{carpeta, hora}`: la forma de 0.7.0, una carpeta;
/// - `{destinos: [{tipo:"carpeta", carpeta} | {tipo:"nube", nube, carpeta}], hora, limite_kib?}`;
/// - cada destino, además (docs/espejo.md): `horario?` (el de las copias) y `tras_copia?`.
pub fn pedido(v: &serde_json::Value) -> Result<Option<Espejo>, String> {
    if v.is_null() {
        return Ok(None);
    }
    let hora = v["hora"].as_str().unwrap_or("02:00").to_string();
    let limite_kib = match &v["limite_kib"] {
        serde_json::Value::Null => None,
        l => Some(l.as_u64().and_then(|k| u32::try_from(k).ok()).ok_or("Límite de subida no válido (KiB/s).")?),
    }
    .filter(|k| *k > 0);
    let mut destinos = Vec::new();
    if let Some(c) = v["carpeta"].as_str() {
        destinos.push(Destino { tipo: "carpeta".into(), carpeta: c.into(), ..Default::default() });
    }
    if let Some(lista) = v.get("destinos").filter(|l| !l.is_null()) {
        for d in lista.as_array().ok_or("«destinos» tiene que ser una lista.")? {
            let carpeta = d["carpeta"].as_str().ok_or("A un destino del espejo le falta la carpeta.")?.to_string();
            let mut nuevo = match d["tipo"].as_str() {
                Some("carpeta") => Destino { tipo: "carpeta".into(), carpeta, ..Default::default() },
                Some("nube") => {
                    let nube = d["nube"].as_str().ok_or("A un destino en la nube le falta el nombre de la nube.")?.to_string();
                    Destino { tipo: "nube".into(), carpeta, nube: Some(nube), ..Default::default() }
                }
                _ => return Err("Tipo de destino del espejo no válido (carpeta o nube).".into()),
            };
            leer_opciones(d, &mut nuevo)?;
            destinos.push(nuevo);
        }
    }
    if destinos.is_empty() {
        return Ok(None);
    }
    Ok(Some(Espejo { hora, destinos, limite_kib, ..Default::default() }))
}

/// Las opciones de un destino (docs/espejo.md), ya comprobadas.
fn leer_opciones(d: &serde_json::Value, nuevo: &mut Destino) -> Result<(), String> {
    if let Some(h) = d.get("horario").filter(|h| !h.is_null()) {
        let h: crate::gestion_v2::Horario = serde_json::from_value(h.clone()).map_err(|_| "Horario del espejo no válido.".to_string())?;
        h.plan_schedule()?.validate()?;
        nuevo.horario = Some(h);
    }
    nuevo.tras_copia = match &d["tras_copia"] {
        serde_json::Value::Null => false,
        t => t.as_bool().ok_or("«tras_copia» tiene que ser verdadero o falso.")?,
    };
    Ok(())
}

#[derive(Debug, Default, PartialEq)]
pub struct Resumen {
    pub copiados: u64,
    pub bytes: u64,
    pub iguales: u64,
    pub recientes: u64,
    /// Ya estaban en el espejo con otro tamaño: no se reemplazan.
    pub distintos: u64,
}

/// Copia `origen` en `destino` con las reglas de arriba.
pub fn copiar(origen: &Path, destino: &Path) -> Result<Resumen, String> {
    copiar_con(origen, destino, &mut |_| {})
}

/// Lo mismo, diciendo los bytes copiados hasta ahora tras cada archivo
/// (la ventana del equipo saca de ahí el ritmo).
pub fn copiar_con(origen: &Path, destino: &Path, avance: &mut dyn FnMut(u64)) -> Result<Resumen, String> {
    let mut r = Resumen::default();
    let ahora = SystemTime::now();
    let mut pendientes: Vec<PathBuf> = vec![PathBuf::new()];
    while let Some(rel) = pendientes.pop() {
        let dir = origen.join(&rel);
        for e in std::fs::read_dir(&dir).map_err(|e| format!("No se pudo leer {}: {e}", dir.display()))?.flatten() {
            let nombre = e.file_name();
            if nombre == "locks" || nombre.to_string_lossy().ends_with(".tmp-espejo") {
                continue;
            }
            let Ok(m) = std::fs::symlink_metadata(e.path()) else { continue };
            let rel_e = rel.join(&nombre);
            if m.file_type().is_symlink() || crate::platform::is_reparse_point(&e.path()) {
                continue;
            }
            if m.is_dir() {
                // En el destino, nunca a través de un enlace (desviaría lo que escribe SYSTEM).
                if crate::platform::is_reparse_point(&destino.join(&rel_e)) {
                    return Err(format!("{} es un enlace: el espejo no escribe a través de enlaces.", destino.join(&rel_e).display()));
                }
                pendientes.push(rel_e);
                continue;
            }
            if m.modified().ok().and_then(|t| ahora.duration_since(t).ok()).is_none_or(|d| d < RECIENTE) {
                r.recientes += 1;
                continue;
            }
            let dest = destino.join(&rel_e);
            if let Ok(d) = std::fs::symlink_metadata(&dest) {
                // Nunca se reescribe lo que ya está: los archivos de restic no
                // cambian, así que otro tamaño es un daño (en el origen o aquí).
                if d.len() == m.len() && d.is_file() {
                    r.iguales += 1;
                } else {
                    r.distintos += 1;
                }
                continue;
            }
            if let Some(p) = dest.parent() {
                std::fs::create_dir_all(p).map_err(|e| format!("No se pudo crear {}: {e}", p.display()))?;
            }
            if crate::platform::is_reparse_point(&dest) {
                return Err(format!("{} es un enlace: el espejo no escribe a través de enlaces.", dest.display()));
            }
            let tmp = dest.with_file_name(format!("{}.tmp-espejo", nombre.to_string_lossy()));
            // Un temporal que ya estuviera (o un enlace con su nombre) se quita antes.
            if std::fs::symlink_metadata(&tmp).is_ok() {
                std::fs::remove_file(&tmp).map_err(|e| format!("No se pudo quitar {}: {e}", tmp.display()))?;
            }
            if let Err(err) = std::fs::copy(e.path(), &tmp) {
                // Lo copiado a medias no se queda (en un disco lleno, ocuparía lo poco que queda).
                let _ = std::fs::remove_file(&tmp);
                return Err(error_al_copiar(&err, destino));
            }
            std::fs::rename(&tmp, &dest).map_err(|e| format!("No se pudo terminar {}: {e}", dest.display()))?;
            r.copiados += 1;
            r.bytes += m.len();
            avance(r.bytes);
        }
    }
    Ok(r)
}

/// El motivo de un archivo que no se pudo copiar al espejo, con qué hacer.
fn error_al_copiar(e: &std::io::Error, destino: &Path) -> String {
    let lleno = e.kind() == std::io::ErrorKind::StorageFull
        || e.raw_os_error() == Some(if cfg!(windows) { 112 } else { 28 })
        || resguardo_motor::restic::sin_espacio(&e.to_string().to_lowercase());
    if lleno {
        format!(
            "no queda espacio en el disco del espejo ({}). Libera espacio en él o elige otra carpeta con más sitio; \
             lo que ya está en el espejo se conserva y lo que falta se copiará en la próxima vuelta.",
            destino.display()
        )
    } else {
        format!("no se pudo copiar a {}: {e}", destino.display())
    }
}

/// « · con su horario», « · después de cada copia nueva»… (para la línea de órdenes).
pub fn cuando_texto(d: &Destino) -> String {
    let mut t = String::new();
    if d.horario.is_some() {
        t += " · con su horario";
    }
    if d.tras_copia {
        t += " · después de cada copia nueva";
    }
    t
}

/// Por qué toca una vuelta a un destino.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Motivo {
    Horario,
    TrasCopia,
}

/// ¿Toca una vuelta a `d`? `novedades`: la primera y la última versión nueva
/// (archivos de `snapshots/`) posteriores a su última vuelta, si las hay.
/// - Por horario: si pasó un hueco desde la última vuelta (o nunca la hubo);
///   si el equipo estaba apagado, una sola vez al encenderse.
/// - «Después de cada copia»: cuando lleva [`ESPERA_TRAS_COPIA`] sin llegar
///   otra versión, o a las [`ESPERA_MAXIMA_TRAS_COPIA`] de la primera.
/// - Entre dos vueltas del mismo destino, al menos `MIN_GAP_MIN` minutos.
pub fn toca(e: &Espejo, d: &Destino, ahora: chrono::DateTime<chrono::Local>, novedades: Option<(SystemTime, SystemTime)>) -> Option<Motivo> {
    let desde = d.desde();
    if let Some(plan) = d.plan(&e.hora) {
        let por_horario = match desde {
            Some(s) => plan.is_due(s, ahora),
            None => plan.latest_slot(ahora).is_some(),
        };
        if por_horario {
            return Some(Motivo::Horario);
        }
    }
    let (primera, ultima) = novedades.filter(|_| d.tras_copia)?;
    let hueco = desde.is_none_or(|s| ahora.signed_duration_since(s) >= chrono::Duration::minutes(crate::plans::MIN_GAP_MIN));
    let ahora_s: SystemTime = ahora.into();
    let pasado = |t: SystemTime| ahora_s.duration_since(t).unwrap_or_default();
    (hueco && (pasado(ultima) >= ESPERA_TRAS_COPIA || pasado(primera) >= ESPERA_MAXIMA_TRAS_COPIA)).then_some(Motivo::TrasCopia)
}

/// Los repositorios que guarda el almacén: `<usuario>` (repositorio en la
/// carpeta del usuario) o `<usuario>/<repo>`, sin seguir enlaces.
pub fn repos_en(origen: &Path) -> Vec<String> {
    let es_repo = |p: &Path| p.join("config").is_file() && p.join("snapshots").is_dir();
    let subdirs = |d: &Path| -> Vec<(String, PathBuf)> {
        let Ok(rd) = std::fs::read_dir(d) else { return Vec::new() };
        let mut v: Vec<(String, PathBuf)> = rd
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()) && !crate::platform::is_reparse_point(&e.path()))
            .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
            .collect();
        v.sort();
        v
    };
    let mut out = Vec::new();
    for (u, du) in subdirs(origen) {
        if es_repo(&du) {
            out.push(u);
            continue;
        }
        for (r, dr) in subdirs(&du) {
            if es_repo(&dr) {
                out.push(format!("{u}/{r}"));
            }
        }
    }
    out
}

/// La primera y la última versión nueva (fecha de los archivos de
/// `snapshots/`) posteriores a `desde`, en los repositorios `repos`.
pub fn novedades(origen: &Path, repos: &[String], desde: Option<SystemTime>) -> Option<(SystemTime, SystemTime)> {
    let mut r: Option<(SystemTime, SystemTime)> = None;
    for repo in repos {
        let Ok(rd) = std::fs::read_dir(origen.join(repo).join("snapshots")) else { continue };
        for e in rd.flatten() {
            let Some(t) = e.metadata().ok().filter(|m| m.is_file()).and_then(|m| m.modified().ok()) else { continue };
            if desde.is_some_and(|d| t <= d) {
                continue;
            }
            r = Some(match r {
                None => (t, t),
                Some((a, b)) => (a.min(t), b.max(t)),
            });
        }
    }
    r
}

// Copia a un destino, contando cómo va en `guarda` (la ventana del equipo: los
/// bytes copiados a una carpeta; lo que lee y sube rclone a una nube). Devuelve
/// el texto del resultado.
fn copiar_a(origen: &Path, d: &Destino, limite_kib: Option<u32>, guarda: &crate::escritorio::en_marcha::Guarda) -> Result<String, String> {
    if d.tipo == "nube" {
        let nombre = d.nube.as_deref().unwrap_or_default();
        let n = crate::nube::buscar(nombre).ok_or_else(|| format!("la nube «{nombre}» ya no está conectada en este equipo."))?;
        return crate::nube::copiar(&n, origen, &d.carpeta, limite_kib, &mut |l, s| guarda.ritmos(l, s));
    }
    // La carpeta de destino: local, sin enlaces en el camino y de Administradores.
    crate::platform::carpeta_local_valida(&d.carpeta)?;
    let destino = Path::new(&d.carpeta);
    // En pruebas (RESGUARDO_AGENT_DIR, sin administrador) la carpeta es de quien
    // corre la prueba, como en `carpeta_privada`.
    if destino.exists() && !crate::agent::test_mode() && !crate::platform::owned_by_admins(destino) {
        return Err("la carpeta del espejo no es de Administradores (vuelve a poner el espejo para corregirla).".into());
    }
    let r = copiar_con(origen, destino, &mut |b| guarda.progreso(Some(b), None))?;
    let texto =
        format!("{} archivos nuevos ({} MB), {} ya estaban, {} se dejan para la próxima vez.", r.copiados, r.bytes / (1024 * 1024), r.iguales, r.recientes);
    if r.distintos > 0 {
        return Err(format!(
            "{texto} Pero {} ya estaban en el espejo con otro tamaño y no se han reemplazado: alguien ha cambiado copias ya escritas (en el Servidor de copias o en el espejo). Revísalo.",
            r.distintos
        ));
    }
    Ok(texto)
}

/// El resultado de todo el espejo (para las consolas anteriores y el aviso
/// `espejo_fallido`): el del único destino, o cuántos fallan en su última vuelta.
pub(crate) fn resultado_global(destinos: &[Destino]) -> Option<String> {
    let hechos: Vec<&String> = destinos.iter().filter_map(|d| d.resultado.as_ref()).collect();
    let errores = hechos.iter().filter(|r| r.starts_with("ERROR")).count();
    match (destinos.len(), hechos.len(), errores) {
        (_, 0, _) => None,
        (1, _, _) => Some(hechos[0].clone()),
        (n, _, 0) => Some(format!("Espejo hecho en los {n} destinos.")),
        (n, _, f) => Some(format!("ERROR: espejo del Servidor de copias: {f} de {n} destinos con error.")),
    }
}

/// Anota en la configuración algo de un destino (se vuelve a leer: pudo cambiar mientras se copiaba).
fn anotar(d: &Destino, f: impl FnOnce(&mut Destino)) {
    let mut c = crate::server::load();
    if let Some(esp) = c.espejo.as_mut() {
        esp.normalizar();
        if let Some(x) = esp.destinos.iter_mut().find(|x| x.mismo(d)) {
            f(x);
        }
        let _ = crate::server::save(&c);
    }
}

/// Lo llama el servicio en cada vuelta: los destinos a los que toca, uno
/// detrás de otro en otro hilo, anotando el resultado de cada uno.
pub fn si_toca() {
    static EN_MARCHA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    static ULTIMA_MIRADA: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
    if EN_MARCHA.load(std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let c = crate::server::load();
    let Some(e) = c.espejo.clone().filter(|_| c.enabled) else { return };
    let ahora = chrono::Local::now();
    let origen = PathBuf::from(&c.path);
    // «Después de cada copia»: mirar `snapshots/` como mucho una vez por minuto.
    let mirar = e.destinos().iter().any(|d| d.tras_copia) && {
        let mut u = ULTIMA_MIRADA.lock().unwrap_or_else(|p| p.into_inner());
        let ya = u.is_some_and(|t| t.elapsed() < Duration::from_secs(60));
        if !ya {
            *u = Some(std::time::Instant::now());
        }
        !ya
    };
    let todos = if mirar { repos_en(&origen) } else { Vec::new() };
    let toca_ya: Vec<(Destino, Motivo)> = e
        .destinos()
        .into_iter()
        .filter_map(|d| {
            let nuevas = if mirar && d.tras_copia { novedades(&origen, &todos, d.desde().map(SystemTime::from)) } else { None };
            toca(&e, &d, ahora, nuevas).map(|m| (d, m))
        })
        .collect();
    if toca_ya.is_empty() || EN_MARCHA.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let limite_kib = e.limite_kib;
    std::thread::spawn(move || {
        for (i, (d, motivo)) in toca_ya.into_iter().enumerate() {
            // El comienzo, antes de empezar: si el servicio se para a medias, no se repite en bucle.
            let inicio = chrono::Local::now().to_rfc3339();
            anotar(&d, |x| x.inicio = Some(inicio.clone()));
            // Para la ventana y los avisos del escritorio: sin la carpeta (es una ruta).
            let (tipo, nombre) = match d.nube.as_deref().filter(|_| d.tipo == "nube") {
                Some(n) => ("nube", n.to_string()),
                None => ("espejo", "Disco o carpeta del equipo".to_string()),
            };
            let guarda = crate::escritorio::en_marcha::empezar(tipo, &i.to_string(), &nombre);
            let por = if motivo == Motivo::TrasCopia { " (después de una copia nueva)" } else { "" };
            let texto = match copiar_a(&origen, &d, limite_kib, &guarda) {
                Ok(t) => {
                    guarda.terminar("ok");
                    format!("Espejo hecho en {}{por}: {t}", d.texto())
                }
                Err(m) => {
                    drop(guarda);
                    format!("ERROR: espejo del Servidor de copias en {}: {m}", d.texto())
                }
            };
            crate::agent::log(&texto);
            // v1.31: el espacio de la nube tras cada vuelta (para «¿Cuándo se llena?»).
            let cuota = if d.tipo == "nube" { d.nube.as_deref().and_then(crate::nube::buscar).and_then(|n| crate::nube::cuota(&n)) } else { None };
            let fin = chrono::Local::now().to_rfc3339();
            let mut c = crate::server::load();
            if let Some(esp) = c.espejo.as_mut() {
                esp.normalizar();
                if let Some(x) = esp.destinos.iter_mut().find(|x| x.mismo(&d)) {
                    x.ultima = Some(fin.clone());
                    x.resultado = Some(texto.clone());
                    if let Some(q) = cuota {
                        x.cuota = Some((q, fin.clone()));
                    }
                }
                esp.ultima = Some(fin.clone());
                esp.resultado = resultado_global(&esp.destinos);
                // También en la bitácora del equipo (para una consola nueva), sin rutas.
                crate::bitacora::espejo(&fin, &texto);
            }
            let _ = crate::server::save(&c);
        }
        EN_MARCHA.store(false, std::sync::atomic::Ordering::SeqCst);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copia_sin_locks_ni_recientes_y_sin_borrar() {
        let base = std::env::temp_dir().join(format!("resguardo-espejo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (o, d) = (base.join("origen"), base.join("destino"));
        std::fs::create_dir_all(o.join("ana/repo/data/ab")).unwrap();
        std::fs::create_dir_all(o.join("ana/repo/locks")).unwrap();
        std::fs::write(o.join("ana/repo/config"), b"config").unwrap();
        std::fs::write(o.join("ana/repo/data/ab/abcdef"), b"datos").unwrap();
        std::fs::write(o.join("ana/repo/locks/l1"), b"lock").unwrap();
        // Todo «reciente»: nada se copia todavía.
        let r = copiar(&o, &d).unwrap();
        assert_eq!((r.copiados, r.recientes), (0, 2));
        // Con fecha antigua, sí (menos el lock).
        let viejo = SystemTime::now() - Duration::from_secs(3600);
        for f in ["ana/repo/config", "ana/repo/data/ab/abcdef"] {
            std::fs::File::options().write(true).open(o.join(f)).unwrap().set_modified(viejo).unwrap();
        }
        let mut avances = Vec::new();
        let r = copiar_con(&o, &d, &mut |b| avances.push(b)).unwrap();
        assert_eq!((r.copiados, r.bytes), (2, 11));
        // Los bytes copiados, tras cada archivo (de ahí sale el ritmo en la ventana).
        assert_eq!(avances.len(), 2);
        assert_eq!(avances.last(), Some(&11));
        assert!(!d.join("ana/repo/locks").exists());
        assert_eq!(std::fs::read(d.join("ana/repo/data/ab/abcdef")).unwrap(), b"datos");
        // Otra vuelta: nada nuevo. Y lo borrado en el origen sigue en el espejo.
        std::fs::remove_file(o.join("ana/repo/config")).unwrap();
        let r = copiar(&o, &d).unwrap();
        assert_eq!((r.copiados, r.iguales), (0, 1));
        assert!(d.join("ana/repo/config").is_file());
        // Un archivo cambiado en el origen (otro tamaño) no reemplaza al del espejo.
        let f = o.join("ana/repo/data/ab/abcdef");
        std::fs::write(&f, b"datos cambiados").unwrap();
        std::fs::File::options().write(true).open(&f).unwrap().set_modified(viejo).unwrap();
        let r = copiar(&o, &d).unwrap();
        assert_eq!((r.copiados, r.distintos), (0, 1));
        assert_eq!(std::fs::read(d.join("ana/repo/data/ab/abcdef")).unwrap(), b"datos");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Un enlace puesto en el destino (por un usuario) no se sigue: el espejo se para con error.
    #[cfg(windows)]
    #[test]
    fn no_escribe_a_traves_de_enlaces() {
        let base = std::env::temp_dir().join(format!("resguardo-espejo-enlace-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (o, d, otra) = (base.join("origen"), base.join("destino"), base.join("otra"));
        std::fs::create_dir_all(o.join("ana/repo")).unwrap();
        std::fs::create_dir_all(&d).unwrap();
        std::fs::create_dir_all(&otra).unwrap();
        let f = o.join("ana/repo/config");
        std::fs::write(&f, b"config").unwrap();
        std::fs::File::options().write(true).open(&f).unwrap().set_modified(SystemTime::now() - Duration::from_secs(3600)).unwrap();
        let ok = std::process::Command::new(crate::platform::system_tool("cmd.exe"))
            .args(["/c", "mklink", "/J"])
            .arg(d.join("ana"))
            .arg(&otra)
            .output()
            .is_ok_and(|o| o.status.success());
        if ok {
            assert!(copiar(&o, &d).unwrap_err().contains("enlace"));
            assert!(std::fs::read_dir(&otra).unwrap().next().is_none(), "nada escrito a través del enlace");
        }
        let _ = std::fs::remove_dir(d.join("ana"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn hora_del_espejo() {
        use chrono::TimeZone;
        let e = Espejo { carpeta: "x".into(), hora: "02:00".into(), ..Default::default() };
        assert_eq!(e.destinos().len(), 1, "forma de 0.7.0");
        let d = |ultima: Option<&str>| Destino { ultima: ultima.map(String::from), ..e.destinos()[0].clone() };
        let t = chrono::Local.with_ymd_and_hms(2026, 10, 2, 2, 0, 0).unwrap();
        // Nunca hecho: toca ya (con el hueco de las 02:00 que ya pasó).
        assert_eq!(toca(&e, &d(None), t, None), Some(Motivo::Horario));
        // Hecho ayer (forma antigua: solo la fecha, o RFC 3339): a las 02:00 de hoy, sí; antes, no.
        assert_eq!(toca(&e, &d(Some("2026-10-01")), t, None), Some(Motivo::Horario));
        let ayer = chrono::Local.with_ymd_and_hms(2026, 10, 1, 2, 3, 0).unwrap().to_rfc3339();
        assert_eq!(toca(&e, &d(Some(&ayer)), t, None), Some(Motivo::Horario));
        let antes = chrono::Local.with_ymd_and_hms(2026, 10, 2, 1, 59, 0).unwrap();
        assert_eq!(toca(&e, &d(Some(&ayer)), antes, None), None);
        // Hecho hoy después de las 02:00: no vuelve a tocar hasta mañana.
        let hoy = chrono::Local.with_ymd_and_hms(2026, 10, 2, 2, 3, 0).unwrap().to_rfc3339();
        assert_eq!(toca(&e, &d(Some(&hoy)), t + chrono::Duration::hours(20), None), None);
        assert_eq!(toca(&e, &d(Some(&hoy)), t + chrono::Duration::hours(24), None), Some(Motivo::Horario));
    }

    /// §3a: el horario de las copias en cada destino, y «después de cada copia».
    #[test]
    fn horario_por_destino_y_tras_copia() {
        use chrono::TimeZone;
        use serde_json::json;
        let e = pedido(&json!({ "hora": "02:00", "destinos": [
            { "tipo": "carpeta", "carpeta": "E:\\espejo", "horario": { "dias": [], "horas": [], "reglas": [
                { "tipo": "intervalo", "dias": [1, 2, 3, 4, 5], "cada_min": 60, "desde": "08:00", "hasta": "18:00" } ] } },
            { "tipo": "nube", "nube": "Dropbox Altamar", "carpeta": "Sur", "tras_copia": true },
        ] }))
        .unwrap()
        .unwrap();
        let (disco, nube) = (e.destinos[0].clone(), e.destinos[1].clone());
        assert!(disco.horario.is_some() && !disco.tras_copia && nube.horario.is_none() && nube.tras_copia);
        // Disco: cada hora de 8 a 18 de lunes a viernes. Viernes 2 de octubre de 2026.
        let hecho = |h: u32, m: u32| Destino { inicio: Some(chrono::Local.with_ymd_and_hms(2026, 10, 2, h, m, 0).unwrap().to_rfc3339()), ..disco.clone() };
        let a = |h: u32, m: u32| chrono::Local.with_ymd_and_hms(2026, 10, 2, h, m, 0).unwrap();
        assert_eq!(toca(&e, &hecho(9, 0), a(9, 30), None), None);
        assert_eq!(toca(&e, &hecho(9, 0), a(10, 0), None), Some(Motivo::Horario));
        assert_eq!(toca(&e, &hecho(18, 0), a(23, 0), None), None, "de noche, no");
        // Nube: cada día a las 02:00 (la `hora` de siempre) y después de cada copia.
        let n = Destino { inicio: Some(a(2, 0).to_rfc3339()), ..nube.clone() };
        assert_eq!(toca(&e, &n, a(12, 0), None), None);
        let ts = |dt: chrono::DateTime<chrono::Local>| SystemTime::from(dt);
        // Una versión a las 11:55: a las 12:00 aún no (agrupa); a las 12:07, sí.
        assert_eq!(toca(&e, &n, a(12, 0), Some((ts(a(11, 55)), ts(a(11, 55))))), None);
        assert_eq!(toca(&e, &n, a(12, 7), Some((ts(a(11, 55)), ts(a(11, 55))))), Some(Motivo::TrasCopia));
        // Siguen llegando: a la hora de la primera, aunque la última sea de hace 2 min.
        assert_eq!(toca(&e, &n, a(12, 50), Some((ts(a(11, 55)), ts(a(12, 48))))), None);
        assert_eq!(toca(&e, &n, a(12, 56), Some((ts(a(11, 55)), ts(a(12, 54))))), Some(Motivo::TrasCopia));
        // Sin «tras_copia», las versiones nuevas no cuentan.
        assert_eq!(toca(&e, &Destino { tras_copia: false, ..n.clone() }, a(12, 7), Some((ts(a(11, 55)), ts(a(11, 55))))), None);
        // El resumen lleva el horario, «tras_copia» y la próxima vuelta.
        let r = e.resumen();
        assert_eq!(r["destinos"][0]["horario"]["reglas"][0]["cada_min"], 60);
        assert_eq!((r["destinos"][1]["tras_copia"].as_bool(), r["destinos"][1].get("horario")), (Some(true), None));
        assert!(r["destinos"][1]["proxima"].as_str().is_some_and(|p| p.contains("T02:00:00")));
        // Un horario que no vale se rechaza.
        for mal in [json!({ "dias": [9], "horas": ["02:00"] }), json!({ "reglas": [{ "tipo": "mensual", "dia": 31, "hora": "02:00" }] }), json!("cada día")] {
            assert!(pedido(&json!({ "destinos": [{ "tipo": "carpeta", "carpeta": "E:\\x", "horario": mal }] })).is_err(), "{mal}");
        }
        assert!(pedido(&json!({ "destinos": [{ "tipo": "carpeta", "carpeta": "E:\\x", "tras_copia": "sí" }] })).is_err());
    }

    /// Las versiones nuevas de `snapshots/`, en los repositorios del almacén.
    #[test]
    fn versiones_nuevas_del_almacen() {
        let base = std::env::temp_dir().join(format!("resguardo-espejo-nuevas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        for r in ["ana/contabilidad", "ana/fotos", "srv"] {
            std::fs::create_dir_all(base.join(r).join("snapshots")).unwrap();
            std::fs::write(base.join(r).join("config"), b"c").unwrap();
        }
        std::fs::create_dir_all(base.join("ana/no-es-repo")).unwrap();
        assert_eq!(repos_en(&base), vec!["ana/contabilidad", "ana/fotos", "srv"]);
        let hace = |s: u64| SystemTime::now() - Duration::from_secs(s);
        for (f, t) in [("ana/contabilidad/snapshots/a1", 3600), ("ana/fotos/snapshots/b2", 600), ("srv/snapshots/c3", 60)] {
            std::fs::write(base.join(f), b"s").unwrap();
            std::fs::File::options().write(true).open(base.join(f)).unwrap().set_modified(hace(t)).unwrap();
        }
        let todos = repos_en(&base);
        let (primera, ultima) = novedades(&base, &todos, None).unwrap();
        assert!(primera <= hace(3500) && ultima >= hace(120));
        // Solo lo posterior a la última vuelta, y solo en los repositorios pedidos.
        let (p, _) = novedades(&base, &todos, Some(hace(1800))).unwrap();
        assert!(p >= hace(700));
        assert_eq!(novedades(&base, &["ana/contabilidad".to_string()], Some(hace(1800))), None);
        assert_eq!(novedades(&base, &todos, Some(SystemTime::now())), None);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn resultado_de_todo_el_espejo() {
        let d = |r: Option<&str>| Destino { resultado: r.map(String::from), ..Default::default() };
        assert_eq!(resultado_global(&[d(None), d(None)]), None);
        assert_eq!(resultado_global(&[d(Some("Espejo hecho en E: 3 archivos"))]).as_deref(), Some("Espejo hecho en E: 3 archivos"));
        assert_eq!(resultado_global(&[d(Some("bien")), d(None)]).as_deref(), Some("Espejo hecho en los 2 destinos."));
        assert!(resultado_global(&[d(Some("bien")), d(Some("ERROR: x"))]).unwrap().starts_with("ERROR: espejo del Servidor de copias: 1 de 2"));
    }
    #[test]
    fn pedidos_y_destinos_quitados() {
        use serde_json::json;
        assert_eq!(pedido(&json!(null)).unwrap(), None);
        assert_eq!(pedido(&json!({"carpeta": null})).unwrap(), None);
        let viejo = pedido(&json!({"carpeta": "E:\\espejo", "hora": "03:00"})).unwrap().unwrap();
        assert_eq!((viejo.hora.as_str(), viejo.destinos[0].tipo.as_str()), ("03:00", "carpeta"));
        let dos = pedido(&json!({"destinos": [{"tipo": "carpeta", "carpeta": "E:\\espejo"}, {"tipo": "nube", "nube": "Dropbox Altamar", "carpeta": "Resguardo/Sur"}], "limite_kib": 512}))
            .unwrap()
            .unwrap();
        assert_eq!((dos.destinos.len(), dos.limite_kib, dos.hora.as_str()), (2, Some(512), "02:00"));
        assert!(pedido(&json!({"destinos": [{"tipo": "ftp", "carpeta": "x"}]})).is_err());
        assert!(pedido(&json!({"destinos": [{"tipo": "nube", "carpeta": "x"}]})).is_err());
        // Añadir un destino no quita nada; quitar uno, o el espejo entero, sí.
        let antiguo = Espejo { carpeta: "E:\\espejo".into(), hora: "02:00".into(), ..Default::default() };
        assert!(!antiguo.quita_destinos(Some(&dos)));
        assert!(dos.quita_destinos(Some(&viejo)));
        assert!(antiguo.quita_destinos(None));
        let mut n = antiguo.clone();
        n.normalizar();
        assert_eq!((n.carpeta.as_str(), n.destinos.len()), ("", 1));
        let r = dos.resumen();
        assert_eq!(r["destinos"][1]["nube"], "Dropbox Altamar");
        assert_eq!(r["limite_kib"], 512);
    }
}
