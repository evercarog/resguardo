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

pub use crate::espejo_motor::RECIENTE;

/// Un destino del espejo.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Destino {
    /// "carpeta" (ruta local completa), "nube" (carpeta dentro de la nube `nube`) o,
    /// tarea 7d.2, "zona" (otra zona de este almacén: `carpeta` es su id o "principal").
    pub tipo: String,
    pub carpeta: String,
    /// Tarea 7d.2 (docs/copias-en-cadena.md): de qué zona se copia (su id); sin él, la principal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zona: Option<String>,
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
    /// §3f: solo estos repositorios (`<usuario>` o `<usuario>/<repo>`); sin
    /// ellos, todo lo que guarda el almacén (también lo que llegue después).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repos: Option<Vec<String>>,
    /// §3f: los repositorios del almacén que había cuando se eligió la
    /// selección: los demás son nuevos y la consola pregunta si entran.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vistos: Vec<String>,
    /// §3d: % de los archivos del destino que se comprueban cada día. Sin él,
    /// 5 en una carpeta y 0 en una nube (comprobar allí es descargar).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verificar_pct: Option<u8>,
    /// §3d: la última comprobación del destino.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verificacion: Option<Verificacion>,
    /// §3d: archivos dañados del almacén que no se copiaron en la última vuelta.
    #[serde(default, skip_serializing_if = "is_cero")]
    pub danados_origen: u64,
    /// §3b: borrar del destino lo que ya no está en el almacén pasados estos días
    /// (7 a 3650). Sin ello, como siempre: nunca se borra nada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retencion_dias: Option<u32>,
    /// §3b: destino con bloqueo de objetos (Object Lock) o que no se debe tocar: nunca se borra.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bloqueo: bool,
    /// §3b: lo que espera para borrarse, tras la última vuelta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub por_borrar: Option<PorBorrar>,
    /// §3b: el freno de la última vuelta (no se anotó ni borró nada), si saltó.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freno: Option<String>,
}

/// §3b: lo que espera para borrarse del destino.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct PorBorrar {
    pub archivos: u64,
    pub bytes: u64,
    /// El día en que se borra el primero (AAAA-MM-DD).
    pub primero: Option<String>,
}

fn is_cero(n: &u64) -> bool {
    *n == 0
}

/// §3d: lo que se comprobó del destino la última vez.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Verificacion {
    pub ultima: String,
    pub archivos: u64,
    /// Los que no cuadraban (reparados o no).
    pub mal: u64,
}

/// % de verificación por defecto en una carpeta del equipo.
pub const VERIFICAR_CARPETA: u8 = 5;

/// Lo que se espera desde la última versión nueva antes de empezar el espejo
/// «después de cada copia» (agrupa varias copias seguidas; más que [`RECIENTE`],
/// para que la última ya se copie).
pub const ESPERA_TRAS_COPIA: Duration = Duration::from_secs(12 * 60);
/// Como mucho, lo que se espera desde la primera versión sin copiar aunque sigan llegando.
pub const ESPERA_MAXIMA_TRAS_COPIA: Duration = Duration::from_secs(60 * 60);

impl Destino {
    /// §3b: los días de retención que cuentan (con bloqueo, ninguno).
    pub fn retencion(&self) -> Option<u32> {
        self.retencion_dias.filter(|_| !self.bloqueo)
    }

    /// §3d: el % que se comprueba cada día.
    pub fn pct_verificar(&self) -> u8 {
        self.verificar_pct.unwrap_or(if self.tipo == "carpeta" || self.tipo == "zona" { VERIFICAR_CARPETA } else { 0 }).min(100)
    }

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
        self.tipo == o.tipo && self.carpeta == o.carpeta && self.nube == o.nube && self.zona == o.zona
    }
    pub fn texto(&self) -> String {
        let t = match &self.nube {
            Some(n) => format!("{n}:{}", self.carpeta),
            None if self.tipo == "zona" => format!("zona {}", self.carpeta),
            None => self.carpeta.clone(),
        };
        match &self.zona {
            Some(z) => format!("{t} (desde la zona {z})"),
            None => t,
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
    /// Plan 0.7.26, bloque 4: los trabajos de espejo (espejo_trabajos.rs). Con ellos,
    /// `destinos` es solo una vista compatible para un agente o una consola anteriores.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trabajos: Vec<crate::espejo_trabajos::Trabajo>,
}

impl Espejo {
    /// Los trabajos: los guardados o, de un espejo de antes, uno por destino (equivalentes).
    pub fn trabajos_efectivos(&self) -> Vec<crate::espejo_trabajos::Trabajo> {
        if !self.trabajos.is_empty() {
            return self.trabajos.clone();
        }
        self.destinos().iter().enumerate().map(|(i, d)| crate::espejo_trabajos::de_destino(d, &self.hora, self.limite_kib, i as u32)).collect()
    }

    /// El espejo con estos trabajos y la vista compatible para un agente o una consola
    /// anteriores (`destinos`, `hora`, `limite_kib`; solo los activos, sin repetir destino).
    pub fn de_trabajos(trabajos: Vec<crate::espejo_trabajos::Trabajo>) -> Espejo {
        let mut destinos: Vec<Destino> = Vec::new();
        for t in trabajos.iter().filter(|t| t.activo && !t.es_equipo()) {
            let d = crate::espejo_trabajos::a_destino(t);
            if !destinos.iter().any(|x| x.mismo(&d)) {
                destinos.push(d);
            }
        }
        let hora = trabajos
            .iter()
            .filter_map(|t| t.cuando.horario.as_ref())
            .find_map(|h| {
                h.horas.first().cloned().or_else(|| {
                    h.reglas.iter().find_map(|r| match r {
                        crate::gestion_v2::Regla::Horas { horas, .. } => horas.first().cloned(),
                        crate::gestion_v2::Regla::Intervalo { desde, .. } => Some(desde.clone()),
                        crate::gestion_v2::Regla::CadaDias { hora, .. } | crate::gestion_v2::Regla::Mensual { hora, .. } => Some(hora.clone()),
                    })
                })
            })
            .filter(|h| chrono::NaiveTime::parse_from_str(h, "%H:%M").is_ok())
            .unwrap_or_else(|| "02:00".into());
        let ultima = trabajos.iter().filter_map(|t| t.estado.ultima.clone()).max();
        let limite_kib = trabajos.iter().filter(|t| t.adonde.tipo == "nube").find_map(|t| t.limite_kib);
        let resultado = resultado_global(&destinos);
        Espejo { carpeta: String::new(), hora, ultima, resultado, destinos, limite_kib, trabajos }
    }

    /// Los destinos, contando la forma antigua de una sola carpeta. Con trabajos, el de
    /// cada trabajo del almacén (también los pausados: para saber qué se usa).
    pub fn destinos(&self) -> Vec<Destino> {
        if !self.trabajos.is_empty() {
            return self.trabajos.iter().filter(|t| !t.es_equipo()).map(crate::espejo_trabajos::a_destino).collect();
        }
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

    /// ¿Reduce `nuevo` la protección? (orden destructiva, docs/espejo.md): quita
    /// un destino (o el espejo entero), deja fuera repositorios que iban a uno,
    /// borra antes, quita el bloqueo o afloja el freno.
    pub fn quita_destinos(&self, nuevo: Option<&Espejo>) -> bool {
        crate::espejo_trabajos::reduce(&self.trabajos_efectivos(), &nuevo.map(Espejo::trabajos_efectivos).unwrap_or_default())
    }

    /// Lo que se ve en el resumen de la consola (sin secretos): los trabajos y,
    /// para una consola anterior, cada destino como antes.
    pub fn resumen(&self) -> serde_json::Value {
        let trabajos = self.trabajos_efectivos();
        // La vista compatible: la de los trabajos activos (sin repetir destino).
        let compat = if self.trabajos.is_empty() { self.destinos() } else { Espejo::de_trabajos(trabajos.clone()).destinos };
        let destinos: Vec<_> = compat
            .iter()
            .map(|d| {
                let espacio = espacio_de(&d.tipo, &d.carpeta, &d.cuota);
                let mut v = serde_json::json!({ "tipo": d.tipo, "carpeta": d.carpeta, "nube": d.nube, "ultima": d.ultima, "resultado": d.resultado, "espacio": espacio });
                // Tarea 7d.2: la zona de origen (sin ella, la principal).
                if let Some(z) = &d.zona {
                    v["zona"] = z.clone().into();
                }
                // Tarea 8e: el sistema de archivos de una carpeta (solo un dato).
                if d.tipo == "carpeta" {
                    v["sistema_archivos"] = crate::espacio::json_fs(&d.carpeta);
                    // 0.7.26: cuándo se vio conectado su disco (marca «Aislado»; solo un resumen del volumen).
                    let a = crate::volumenes::resumen(&format!("espejo:{}", crate::volumenes::resumen_id(&d.carpeta)), &d.carpeta);
                    if !a.is_null() {
                        v["aislado"] = a;
                    }
                }
                // §3a: su horario (si tiene uno propio), «después de cada copia» y la próxima vuelta por horario.
                if let Some(h) = &d.horario {
                    v["horario"] = serde_json::to_value(h).unwrap_or_default();
                }
                v["tras_copia"] = d.tras_copia.into();
                // §3d: cuánto se comprueba y la última comprobación; lo dañado en el almacén.
                v["verificar_pct"] = d.pct_verificar().into();
                v["verificacion"] = serde_json::to_value(&d.verificacion).unwrap_or_default();
                v["danados_origen"] = d.danados_origen.into();
                // §3b: la retención (o el bloqueo), lo que espera para borrarse y el freno.
                v["retencion_dias"] = d.retencion_dias.into();
                v["bloqueo"] = d.bloqueo.into();
                v["por_borrar"] = serde_json::to_value(&d.por_borrar).unwrap_or_default();
                v["freno"] = d.freno.clone().into();
                // §3f: la selección (sin ella, todos) y lo que había al elegirla.
                if let Some(r) = &d.repos {
                    v["repos"] = r.clone().into();
                    v["vistos"] = d.vistos.clone().into();
                }
                v["proxima"] = d.plan(&self.hora).and_then(|p| p.next_slot(chrono::Local::now())).map(|t| t.to_rfc3339()).into();
                // Plan 0.7.26: de qué trabajo es esta vista (una consola anterior no lo mira).
                if !self.trabajos.is_empty() {
                    if let Some(t) = trabajos.iter().find(|t| t.activo && crate::espejo_trabajos::a_destino(t).mismo(d)) {
                        v["trabajo"] = t.id.clone().into();
                    }
                }
                v
            })
            .collect();
        // Plan 0.7.26: los trabajos, con su configuración y cómo fue su última vuelta.
        let trabajos: Vec<_> =
            trabajos.iter().map(|t| crate::espejo_trabajos::resumen(t, espacio_de(&t.adonde.tipo, &t.adonde.carpeta, &t.estado.cuota))).collect();
        let ultima = if self.trabajos.is_empty() { self.ultima.clone() } else { self.trabajos.iter().filter_map(|t| t.estado.ultima.clone()).max() };
        let resultado = if self.trabajos.is_empty() { self.resultado.clone() } else { resultado_global(&compat) };
        serde_json::json!({
            "hora": self.hora, "ultima": ultima, "resultado": resultado, "limite_kib": self.limite_kib, "destinos": destinos, "trabajos": trabajos,
        })
    }
}

/// v1.31: libre y total de un destino (carpeta: su volumen, ahora; zona: su carpeta;
/// nube: la cuenta, tras el último espejo).
fn espacio_de(tipo: &str, carpeta: &str, cuota: &Option<(crate::espacio::Espacio, String)>) -> serde_json::Value {
    match (tipo, cuota) {
        ("carpeta", _) => crate::espacio::json_de(carpeta),
        ("zona", _) => crate::server::load().carpeta_zona(Some(carpeta)).map(crate::espacio::json_de).unwrap_or_default(),
        (_, Some((e, leido))) => e.json(leido),
        _ => serde_json::Value::Null,
    }
}

/// Los trabajos que pide una orden `guarda_copias { espejo }`: `{ trabajos: […] }`
/// (plan 0.7.26) o la forma de antes (`destinos`, `carpeta`), convertida. `None`: quitarlo.
pub fn trabajos_del_pedido(v: &serde_json::Value) -> Result<Option<Vec<crate::espejo_trabajos::Trabajo>>, String> {
    if v.get("trabajos").is_some_and(|t| !t.is_null()) {
        let t = crate::espejo_trabajos::leer_pedido(v, crate::espejo_trabajos::QUIEN_ALMACEN)?;
        return Ok((!t.is_empty()).then_some(t));
    }
    let actuales = crate::server::load().espejo.map(|e| e.trabajos_efectivos()).unwrap_or_default();
    Ok(pedido(v)?.map(|mut e| {
        e.normalizar();
        let mut t = e.trabajos_efectivos();
        crate::espejo_trabajos::mapear_ids(&mut t, &actuales);
        t
    }))
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
                // Tarea 7d.2: otra zona del almacén (su id, o «principal»).
                Some("zona") if carpeta == "principal" || crate::server::zona_id_valido(&carpeta) => {
                    Destino { tipo: "zona".into(), carpeta, ..Default::default() }
                }
                Some("zona") => return Err("Zona del espejo no válida.".into()),
                _ => return Err("Tipo de destino del espejo no válido (carpeta, nube o zona).".into()),
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

/// §3f: lo que había en el almacén al elegir la selección de cada destino.
/// Lo dice la consola (lo que enseñó); si no, el de antes con la misma
/// selección o, si cambió, lo que hay ahora (`hay`). Los elegidos, siempre.
pub fn fijar_vistos(nuevo: &mut Espejo, anterior: Option<&Espejo>, hay: &[String]) {
    for d in nuevo.destinos.iter_mut() {
        let Some(repos) = d.repos.clone() else {
            d.vistos.clear();
            continue;
        };
        if d.vistos.is_empty() {
            d.vistos = match anterior.and_then(|a| a.destinos.iter().find(|x| x.mismo(d) && x.repos.as_ref() == Some(&repos))) {
                Some(x) => x.vistos.clone(),
                None => hay.to_vec(),
            };
        }
        for r in repos {
            if !d.vistos.contains(&r) {
                d.vistos.push(r);
            }
        }
        d.vistos.sort();
    }
}

/// Las opciones de un destino (docs/espejo.md), ya comprobadas.
fn leer_opciones(d: &serde_json::Value, nuevo: &mut Destino) -> Result<(), String> {
    // Tarea 7d.2: la zona de origen («principal» o nada: la principal).
    nuevo.zona = match &d["zona"] {
        serde_json::Value::Null => None,
        serde_json::Value::String(z) if z == "principal" => None,
        serde_json::Value::String(z) if crate::server::zona_id_valido(z) => Some(z.clone()),
        _ => return Err("Zona de origen del espejo no válida.".into()),
    };
    if let Some(h) = d.get("horario").filter(|h| !h.is_null()) {
        let h: crate::gestion_v2::Horario = serde_json::from_value(h.clone()).map_err(|_| "Horario del espejo no válido.".to_string())?;
        h.plan_schedule()?.validate()?;
        nuevo.horario = Some(h);
    }
    nuevo.tras_copia = match &d["tras_copia"] {
        serde_json::Value::Null => false,
        t => t.as_bool().ok_or("«tras_copia» tiene que ser verdadero o falso.")?,
    };
    let lista = |k: &str| -> Result<Option<Vec<String>>, String> {
        match &d[k] {
            serde_json::Value::Null => Ok(None),
            serde_json::Value::Array(l) => {
                let mut v = Vec::new();
                for r in l {
                    let r =
                        r.as_str().filter(|r| crate::espejo_motor::repo_valido(r)).ok_or("Nombre de repositorio del espejo no válido (<usuario>/<repo>).")?;
                    if !v.iter().any(|x| x == r) {
                        v.push(r.to_string());
                    }
                }
                if v.len() > 500 {
                    return Err("Demasiados repositorios en un destino del espejo.".into());
                }
                Ok(Some(v))
            }
            _ => Err(format!("«{k}» tiene que ser una lista de repositorios.")),
        }
    };
    nuevo.repos = lista("repos")?;
    if nuevo.repos.as_ref().is_some_and(Vec::is_empty) {
        return Err("Elige al menos un repositorio para ese destino del espejo (o todos).".into());
    }
    nuevo.vistos = lista("vistos")?.unwrap_or_default();
    nuevo.retencion_dias = match &d["retencion_dias"] {
        serde_json::Value::Null => None,
        n => Some(
            n.as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .filter(|n| (crate::espejo_motor::RETENCION_MIN..=crate::espejo_motor::RETENCION_MAX).contains(n))
                .ok_or(format!(
                    "Los días de retención del espejo tienen que ir de {} a {}.",
                    crate::espejo_motor::RETENCION_MIN,
                    crate::espejo_motor::RETENCION_MAX
                ))?,
        ),
    };
    nuevo.bloqueo = match &d["bloqueo"] {
        serde_json::Value::Null => false,
        b => b.as_bool().ok_or("«bloqueo» tiene que ser verdadero o falso.")?,
    };
    if nuevo.bloqueo && nuevo.retencion_dias.is_some() {
        return Err("Un destino con bloqueo de objetos no puede tener retención: el espejo nunca borra allí.".into());
    }
    nuevo.verificar_pct = match &d["verificar_pct"] {
        serde_json::Value::Null => None,
        p => Some(p.as_u64().filter(|p| *p <= 100).ok_or("El % de verificación del espejo tiene que ir de 0 a 100.")? as u8),
    };
    Ok(())
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

/// §3b: `guarda_copias { espejo_freno: … }` (espera, como lo que reduce la protección):
/// lo que falta en el almacén se anota en la próxima vuelta sin freno, y se borrará a su
/// tiempo. Plan 0.7.26: `{ trabajo: <id>, quien?: "equipo" }` confirma ese trabajo; la forma
/// de antes (`{ tipo, carpeta, nube?, zona? }`), todos los trabajos del almacén a ese destino.
pub fn aceptar_freno(v: &serde_json::Value) -> Result<String, String> {
    use crate::espejo_trabajos as et;
    if let Some(id) = v["trabajo"].as_str() {
        let lista = if v["quien"] == et::QUIEN_EQUIPO {
            et::cargar_equipo().trabajos
        } else {
            crate::server::load().espejo.map(|e| e.trabajos_efectivos()).unwrap_or_default()
        };
        let t = lista.iter().find(|t| t.id == id).ok_or("Ese espejo ya no está.")?;
        return et::aceptar_freno(t);
    }
    let que = Destino {
        tipo: v["tipo"].as_str().unwrap_or_default().to_string(),
        carpeta: v["carpeta"].as_str().unwrap_or_default().trim().to_string(),
        nube: v["nube"].as_str().map(|n| n.trim().to_string()),
        zona: v["zona"].as_str().filter(|z| *z != "principal").map(str::to_string),
        ..Default::default()
    };
    let trabajos: Vec<et::Trabajo> =
        crate::server::load().espejo.map(|e| e.trabajos_efectivos()).unwrap_or_default().into_iter().filter(|t| et::a_destino(t).mismo(&que)).collect();
    if trabajos.is_empty() {
        return Err("Ese destino ya no está en el espejo.".into());
    }
    let mut hecho = None;
    for t in &trabajos {
        if let Ok(m) = et::aceptar_freno(t) {
            hecho = Some(m);
        }
    }
    hecho.ok_or_else(|| "Ese destino del espejo no tiene retención: nunca borra nada.".into())
}

/// El archivo de estado de un destino de antes (el mismo que el de su trabajo equivalente).
#[cfg(test)]
fn archivo_estado(d: &Destino) -> PathBuf {
    crate::espejo_trabajos::archivo_estado(&crate::espejo_trabajos::de_destino(d, "02:00", None, 0), None)
}

/// El resultado de una vuelta en una frase (error si algo no cuadra).
pub fn texto_de(r: &crate::espejo_motor::Resumen) -> Result<String, String> {
    let mut texto =
        format!("{} archivos nuevos ({} MB), {} ya estaban, {} se dejan para la próxima vez.", r.copiados, r.bytes / (1024 * 1024), r.iguales, r.recientes);
    if !r.faltan_repos.is_empty() {
        texto += &format!(" Ya no están en el almacén: {}.", r.faltan_repos.join(", "));
    }
    if r.verificados > 0 {
        texto += &format!(" Comprobados {} archivos del espejo.", r.verificados);
    }
    if r.borrados > 0 {
        texto += &format!(" Borrados {} archivos que ya no estaban en el almacén.", r.borrados);
    }
    // §3d: lo que no cuadra con su nombre. Todo es un error (aviso `espejo_fallido`).
    let mut problemas = Vec::new();
    if !r.danados_origen.is_empty() {
        problemas.push(format!(
            "{} archivos dañados en el almacén no se han copiado (su contenido no cuadra con su nombre; p. ej. {}): revisa el disco del almacén y comprueba esos repositorios",
            r.danados_origen.len(),
            r.danados_origen[0]
        ));
    }
    if !r.mal_destino.is_empty() {
        problemas.push(format!(
            "{} archivos del espejo están dañados y no se han podido reparar (p. ej. {}): revisa ese destino",
            r.mal_destino.len(),
            r.mal_destino[0]
        ));
    }
    if r.reparados > 0 {
        problemas.push(format!("{} archivos dañados del espejo se han vuelto a copiar bien del almacén: revisa el disco del espejo", r.reparados));
    }
    if let Some(f) = &r.freno {
        problemas.push(f.clone());
    }
    if r.distintos > 0 {
        problemas.push(format!(
            "{} ya estaban en el espejo con otro tamaño y no se han reemplazado: alguien ha cambiado copias ya escritas (en el Servidor de copias o en el espejo). Revísalo",
            r.distintos
        ));
    }
    if !problemas.is_empty() {
        return Err(format!("{texto} Pero {}.", problemas.join(". Además, ")));
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

/// Anota en la configuración algo de un trabajo del almacén (se vuelve a leer: pudo
/// cambiar mientras se copiaba). Un espejo de antes pasa aquí a guardarse como trabajos.
fn anotar(id: &str, f: impl FnOnce(&mut crate::espejo_trabajos::Trabajo)) {
    let mut c = crate::server::load();
    if let Some(esp) = c.espejo.as_ref() {
        let mut ts = esp.trabajos_efectivos();
        if let Some(x) = ts.iter_mut().find(|x| x.id == id) {
            f(x);
            c.espejo = Some(Espejo::de_trabajos(ts));
            let _ = crate::server::save(&c);
        }
    }
}

/// Los repositorios del almacén que mira «después de cada copia nueva» de un trabajo.
fn repos_del_trabajo(t: &crate::espejo_trabajos::Trabajo, origen: &Path) -> Vec<String> {
    use crate::espejo_trabajos::Que;
    match &t.que {
        Que::Todos => repos_en(origen),
        Que::Equipos { equipos } => repos_en(origen).into_iter().filter(|r| equipos.iter().any(|u| r == u || r.starts_with(&format!("{u}/")))).collect(),
        Que::Repos { repos } => repos.clone(),
    }
}

/// Lo llama el servicio en cada vuelta: los trabajos del almacén a los que toca, uno
/// detrás de otro (por su orden) en otro hilo, anotando el resultado de cada uno.
pub fn si_toca() {
    use crate::espejo_trabajos::{self as et, Motivo};
    static EN_MARCHA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    static ULTIMA_MIRADA: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
    if EN_MARCHA.load(std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let c = crate::server::load();
    let Some(e) = c.espejo.clone().filter(|_| c.enabled) else { return };
    let trabajos = e.trabajos_efectivos();
    let ahora = chrono::Local::now();
    // «Después de cada copia»: mirar `snapshots/` como mucho una vez por minuto.
    let mirar = trabajos.iter().any(|t| t.activo && t.cuando.tras_copia) && {
        let mut u = ULTIMA_MIRADA.lock().unwrap_or_else(|p| p.into_inner());
        let ya = u.is_some_and(|t| t.elapsed() < Duration::from_secs(60));
        if !ya {
            *u = Some(std::time::Instant::now());
        }
        !ya
    };
    let mut toca_ya: Vec<(et::Trabajo, Motivo, PathBuf)> = trabajos
        .iter()
        .filter_map(|t| {
            // Tarea 7d.2: cada trabajo copia desde su zona (sin ella, la principal).
            let origen = c.carpeta_zona(t.zona.as_deref()).map(PathBuf::from)?;
            let nuevas = if mirar && t.activo && t.cuando.tras_copia {
                novedades(&origen, &repos_del_trabajo(t, &origen), t.desde().map(SystemTime::from))
            } else {
                None
            };
            et::toca(t, &trabajos, ahora, nuevas).map(|m| (t.clone(), m, origen))
        })
        .collect();
    toca_ya.sort_by_key(|(t, _, _)| t.orden);
    if toca_ya.is_empty() || EN_MARCHA.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        for (t, motivo, origen) in toca_ya {
            hacer(&t, motivo, &origen);
        }
        EN_MARCHA.store(false, std::sync::atomic::Ordering::SeqCst);
    });
}

/// Una vuelta de un trabajo del almacén: su origen, su alcance y su destino.
pub fn hacer(t: &crate::espejo_trabajos::Trabajo, motivo: crate::espejo_trabajos::Motivo, origen: &Path) -> String {
    use crate::espejo_trabajos as et;
    // El comienzo, antes de empezar: si el servicio se para a medias, no se repite en bucle.
    let inicio = chrono::Local::now().to_rfc3339();
    anotar(&t.id, |x| x.estado.inicio = Some(inicio.clone()));
    // Para la ventana y los avisos del escritorio: sin la carpeta (es una ruta).
    let (tipo, nombre) = match t.adonde.nube.as_deref().filter(|_| t.adonde.tipo == "nube") {
        Some(n) => ("nube", n.to_string()),
        None => ("espejo", "Disco o carpeta del equipo".to_string()),
    };
    let guarda = crate::escritorio::en_marcha::empezar(tipo, &t.id, &nombre);
    let carpeta = t.adonde.tipo != "nube";
    let zona = |z: &str| crate::server::load().carpeta_zona(Some(z)).map(str::to_string);
    let hecho = et::vuelta(t, origen, &t.alcance(), None, &zona, &et::archivo_estado(t, None), &mut |l, s| {
        if carpeta {
            guarda.progreso(l, None)
        } else {
            guarda.ritmos(l, s)
        }
    });
    let mut resumenes = Vec::new();
    let texto = hecho.and_then(|r| {
        let x = texto_de(&r);
        resumenes.push(r);
        x
    });
    let texto = et::texto_de(t, motivo, &[(None, texto)]);
    if texto.starts_with("ERROR") {
        drop(guarda);
    } else {
        guarda.terminar("ok");
    }
    crate::agent::log(&texto);
    // v1.31: el espacio de la nube tras cada vuelta (para «¿Cuándo se llena?»).
    let cuota = if t.adonde.tipo == "nube" { t.adonde.nube.as_deref().and_then(crate::nube::buscar).and_then(|n| crate::nube::cuota(&n)) } else { None };
    let fin = chrono::Local::now().to_rfc3339();
    anotar(&t.id, |x| {
        et::anotar_resultado(x, &texto, &fin, &resumenes);
        if let Some(q) = cuota {
            x.estado.cuota = Some((q, fin.clone()));
        }
    });
    // También en la bitácora del equipo (para una consola nueva).
    crate::bitacora::espejo(&fin, &texto);
    texto
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::espejo_motor::{vuelta, Alcance, Estado, Lado, Opciones, Resumen};

    /// Una vuelta de todo el almacén a una carpeta.
    fn copiar(origen: &Path, destino: &Path) -> Result<Resumen, String> {
        vuelta(origen, &Lado::Carpeta(destino), &Alcance::Todos, &Opciones::default(), &mut Estado::default(), &mut |_, _| {})
    }
    fn copiar_con(origen: &Path, destino: &Path, avance: &mut dyn FnMut(u64)) -> Result<Resumen, String> {
        vuelta(origen, &Lado::Carpeta(destino), &Alcance::Todos, &Opciones::default(), &mut Estado::default(), &mut |l, _| avance(l.unwrap_or(0)))
    }

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

    /// §3f: la selección de repositorios de cada destino.
    #[test]
    fn seleccion_de_repositorios() {
        use serde_json::json;
        let pide = |d: serde_json::Value| pedido(&json!({ "destinos": [d] }));
        let e = pide(json!({ "tipo": "carpeta", "carpeta": "E:\\x", "repos": ["ana/conta", "srv", "ana/conta"] })).unwrap().unwrap();
        assert_eq!(e.destinos[0].repos.as_deref(), Some(&["ana/conta".to_string(), "srv".to_string()][..]), "sin repetir");
        assert!(pide(json!({ "tipo": "carpeta", "carpeta": "E:\\x", "repos": null })).unwrap().unwrap().destinos[0].repos.is_none(), "null: todos");
        for mal in [json!([]), json!(["../x"]), json!(["a/b/c"]), json!([5]), json!("ana")] {
            assert!(pide(json!({ "tipo": "carpeta", "carpeta": "E:\\x", "repos": mal })).is_err(), "{mal}");
        }
        // Qué reduce la protección: quitar un repositorio de la selección, o pasar de todos a algunos.
        let con = |r: Option<Vec<&str>>| Espejo {
            hora: "02:00".into(),
            destinos: vec![Destino {
                tipo: "carpeta".into(),
                carpeta: "E:\\x".into(),
                repos: r.map(|l| l.into_iter().map(String::from).collect()),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(con(None).quita_destinos(Some(&con(Some(vec!["a"])))));
        assert!(con(Some(vec!["a", "b"])).quita_destinos(Some(&con(Some(vec!["a"])))));
        assert!(!con(Some(vec!["a"])).quita_destinos(Some(&con(Some(vec!["a", "b"])))), "añadir no");
        assert!(!con(Some(vec!["a"])).quita_destinos(Some(&con(None))), "pasar a todos no");
        assert!(!con(None).quita_destinos(Some(&con(None))));
        // Lo que había al elegir: lo que dice la consola, lo de antes si la selección no cambia, o lo de ahora.
        let hay = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let mut n = con(Some(vec!["a"]));
        fijar_vistos(&mut n, None, &hay);
        assert_eq!(n.destinos[0].vistos, hay);
        let mut antes = n.clone();
        antes.destinos[0].vistos = vec!["a".into(), "b".into()];
        let mut otra = con(Some(vec!["a"]));
        fijar_vistos(&mut otra, Some(&antes), &hay);
        assert_eq!(otra.destinos[0].vistos, vec!["a", "b"], "misma selección: lo de antes (c sigue siendo nuevo)");
        let mut cambia = con(Some(vec!["a", "b"]));
        fijar_vistos(&mut cambia, Some(&antes), &hay);
        assert_eq!(cambia.destinos[0].vistos, hay, "otra selección: lo de ahora");
        let mut dicho = con(Some(vec!["a"]));
        dicho.destinos[0].vistos = vec!["c".into()];
        fijar_vistos(&mut dicho, Some(&antes), &hay);
        assert_eq!(dicho.destinos[0].vistos, vec!["a", "c"], "lo que dice la consola, y los elegidos");
        let mut todos = con(None);
        todos.destinos[0].vistos = vec!["x".into()];
        fijar_vistos(&mut todos, None, &hay);
        assert!(todos.destinos[0].vistos.is_empty());
        // En el resumen, solo con selección.
        let r = cambia.resumen();
        assert_eq!((r["destinos"][0]["repos"][1].as_str(), r["destinos"][0]["vistos"][2].as_str()), (Some("b"), Some("c")));
        assert!(con(None).resumen()["destinos"][0].get("repos").is_none());
    }

    /// §3b: la retención de cada destino, el bloqueo y qué reduce la protección.
    #[test]
    fn retencion_y_bloqueo_de_un_destino() {
        use serde_json::json;
        let pide = |extra: serde_json::Value| {
            let mut d = json!({ "tipo": "carpeta", "carpeta": "E:\\x" });
            d.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            pedido(&json!({ "destinos": [d] }))
        };
        let e = pide(json!({ "retencion_dias": 30 })).unwrap().unwrap();
        assert_eq!((e.destinos[0].retencion(), e.destinos[0].bloqueo), (Some(30), false));
        for mal in [
            json!({ "retencion_dias": 6 }),
            json!({ "retencion_dias": 4000 }),
            json!({ "retencion_dias": "30" }),
            json!({ "retencion_dias": 30, "bloqueo": true }),
            json!({ "bloqueo": "sí" }),
        ] {
            assert!(pide(mal.clone()).is_err(), "{mal}");
        }
        assert!(pide(json!({ "bloqueo": true })).unwrap().unwrap().destinos[0].bloqueo);
        let con = |ret: Option<u32>, bloqueo: bool| Espejo {
            hora: "02:00".into(),
            destinos: vec![Destino { tipo: "carpeta".into(), carpeta: "E:\\x".into(), retencion_dias: ret, bloqueo, ..Default::default() }],
            ..Default::default()
        };
        assert!(con(None, false).quita_destinos(Some(&con(Some(30), false))), "poner retención espera");
        assert!(con(Some(60), false).quita_destinos(Some(&con(Some(30), false))), "acortarla también");
        assert!(!con(Some(30), false).quita_destinos(Some(&con(Some(60), false))), "alargarla no");
        assert!(!con(Some(30), false).quita_destinos(Some(&con(None, false))), "quitarla no (deja de borrar)");
        assert!(con(None, true).quita_destinos(Some(&con(None, false))), "quitar el bloqueo espera");
        assert!(!con(None, false).quita_destinos(Some(&con(None, true))));
        let mut x = con(Some(30), false);
        x.destinos[0].por_borrar = Some(PorBorrar { archivos: 3, bytes: 10, primero: Some("2026-11-01".into()) });
        x.destinos[0].freno = Some("falta de golpe…".into());
        let r = x.resumen();
        assert_eq!((r["destinos"][0]["retencion_dias"].as_u64(), r["destinos"][0]["por_borrar"]["archivos"].as_u64()), (Some(30), Some(3)));
        assert_eq!(r["destinos"][0]["freno"], "falta de golpe…");
    }

    /// Tarea 7d.2: un paso «espejo» de un repositorio desde una zona (la E) a otra
    /// zona (la principal, D) y a una carpeta: el pedido, el origen por zona y la vuelta.
    #[test]
    fn espejo_de_un_repositorio_entre_zonas() {
        use serde_json::json;
        let e = pedido(&json!({ "hora": "02:00", "destinos": [
            { "tipo": "zona", "carpeta": "principal", "zona": "z0e0e0e", "repos": ["ana-2/conta"], "tras_copia": true },
            { "tipo": "carpeta", "carpeta": "F:\\espejo", "zona": "z0e0e0e", "repos": ["ana-2/conta"], "retencion_dias": 30 },
            { "tipo": "carpeta", "carpeta": "F:\\espejo", "repos": ["ana/conta"] },
        ] }))
        .unwrap()
        .unwrap();
        let (a_d, a_f, de_d) = (&e.destinos[0], &e.destinos[1], &e.destinos[2]);
        assert_eq!((a_d.tipo.as_str(), a_d.carpeta.as_str(), a_d.zona.as_deref()), ("zona", "principal", Some("z0e0e0e")));
        assert!(!a_f.mismo(de_d), "la misma carpeta desde otra zona es otro destino");
        assert_eq!(a_d.texto(), "zona principal (desde la zona z0e0e0e)");
        assert_eq!(a_d.pct_verificar(), VERIFICAR_CARPETA, "otra zona es una carpeta del equipo");
        assert_ne!(archivo_estado(a_f), archivo_estado(de_d), "cada origen lleva su cuenta");
        assert_eq!(e.resumen()["destinos"][0]["zona"], "z0e0e0e");
        assert!(e.resumen()["destinos"][2].get("zona").is_none());
        // «principal» como zona de origen es lo mismo que no decir nada.
        let p = pedido(&json!({ "destinos": [{ "tipo": "carpeta", "carpeta": "F:\\x", "zona": "principal" }] })).unwrap().unwrap();
        assert_eq!(p.destinos[0].zona, None);
        for mal in [json!({ "tipo": "zona", "carpeta": "../x" }), json!({ "tipo": "carpeta", "carpeta": "F:\\x", "zona": "Z1" })] {
            assert!(pedido(&json!({ "destinos": [mal] })).is_err(), "{mal}");
        }
        // La carpeta de cada zona en la configuración del almacén.
        let base = std::env::temp_dir().join(format!("resguardo-espejo-zonas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (zona_d, zona_e) = (base.join("D"), base.join("E"));
        let mut c = crate::server::ServerConfig { path: zona_d.display().to_string(), ..Default::default() };
        c.zonas.push(crate::server::Zona {
            id: "z0e0e0e".into(),
            nombre: "Disco E".into(),
            path: zona_e.display().to_string(),
            port: 8002,
            users: vec![],
            creada: String::new(),
        });
        assert_eq!(c.carpeta_zona(None), Some(c.path.as_str()));
        assert_eq!(c.carpeta_zona(Some("principal")), Some(c.path.as_str()));
        assert_eq!(c.carpeta_zona(Some("z0e0e0e")).map(PathBuf::from), Some(zona_e.clone()));
        assert_eq!(c.carpeta_zona(Some("z999999")), None);
        // La vuelta: solo ese repositorio de la zona E, a la D; sin retención, nunca borra.
        let viejo = SystemTime::now() - Duration::from_secs(3600);
        for f in ["ana-2/conta/config", "ana-2/conta/data/ab/abcd", "ana-2/otro/config"] {
            let p = zona_e.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, b"x").unwrap();
            std::fs::File::options().write(true).open(&p).unwrap().set_modified(viejo).unwrap();
        }
        let origen = PathBuf::from(c.carpeta_zona(a_d.zona.as_deref()).unwrap());
        let alcance = crate::espejo_motor::Alcance::de(a_d.repos.as_deref());
        let r = vuelta(&origen, &Lado::Carpeta(&zona_d), &alcance, &Opciones::default(), &mut Estado::default(), &mut |_, _| {}).unwrap();
        assert_eq!(r.copiados, 2);
        assert!(zona_d.join("ana-2/conta/data/ab/abcd").is_file() && !zona_d.join("ana-2/otro").exists());
        std::fs::remove_file(zona_e.join("ana-2/conta/data/ab/abcd")).unwrap();
        let r = vuelta(&origen, &Lado::Carpeta(&zona_d), &alcance, &Opciones::default(), &mut Estado::default(), &mut |_, _| {}).unwrap();
        assert_eq!(r.borrados, 0);
        assert!(zona_d.join("ana-2/conta/data/ab/abcd").is_file(), "sin retención no se borra nada");
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
