//! Agente v2: lo que gestiona Resguardo Server en el equipo (docs/api-servidor.md,
//! §6 «Cuerpos de las órdenes» y «Configuración del equipo»).
//!
//! - **Destinos** (dónde se guardan las copias) con sus credenciales, y
//!   **repositorios** con su contraseña: llegan una vez, con `crear_repositorio`
//!   (nivel administración), y se quedan en el equipo (en `servidor.bin`,
//!   protegido como los demás secretos del agente). Nunca vuelven al servidor.
//! - **Configuración** (`config`): el documento `Configuracion` v1 de la
//!   consola, **sin secretos** (copias, carpetas, horarios). El equipo la
//!   aplica, la sube cifrada con `K_cfg` (simetrico::cifrar_config, atada al
//!   equipo y a su número) y deja en claro un resumen sin rutas.
//! - La retención no se cambia con `config` (reduce la protección): solo con
//!   `cambiar_retencion` (nivel repositorio, con espera) y se aplica con
//!   `aplicar_retencion`.

use crate::servidor_v2::Vinculo;
use base64::Engine;
use resguardo_protocolo::simetrico;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// Un destino con sus credenciales (solo en el equipo).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Destino {
    pub id: String,
    pub nombre: String,
    /// "local", "rest", "s3", "b2" o "sftp".
    pub tipo: String,
    /// Carpeta, URL del servidor o bucket (sin credenciales).
    pub donde: String,
    #[serde(default)]
    pub usuario: Option<String>,
    #[serde(default)]
    pub secreto: Option<String>,
    /// Autoridad TLS propia de un rest-server (PEM).
    #[serde(default)]
    pub ca_pem: Option<String>,
    /// v1.30: el equipo que guarda copias de este destino («Copiar en <almacén>»),
    /// si lo dijo la consola al crearlo. Solo un id: va en el resumen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equipo_almacen: Option<String>,
}

/// v1.30: `destino.equipo_almacen` de una orden (el id del equipo que guarda
/// copias), solo en un destino rest y si tiene forma de id.
pub fn equipo_almacen_de(dest: &Value) -> Option<String> {
    (dest["tipo"] == "rest").then(|| dest["equipo_almacen"].as_str().filter(|s| id_valido(s)).map(str::to_string)).flatten()
}

/// Cantidad «sin límite» (`unlimited` en restic): «mensuales siempre».
pub const SIEMPRE: i64 = resguardo_motor::retention::UNLIMITED;

/// Un plazo de restic («15d», «1y», «6m», «48h», «1y6m»): años, meses, días y horas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Plazo {
    pub anos: i32,
    pub meses: i32,
    pub dias: i32,
    pub horas: i32,
}

impl Plazo {
    /// Como `restic` (`ParseDuration`): uno o más pares número+unidad (y, m, d, h).
    /// `None` si no vale, si es cero o si es desmesurado (más de 200 años).
    pub fn leer(s: &str) -> Option<Plazo> {
        if s.is_empty() || s.len() > 16 {
            return None;
        }
        let mut p = Plazo::default();
        let mut n: Option<i32> = None;
        for c in s.chars() {
            match c {
                '0'..='9' => n = Some(n.unwrap_or(0).checked_mul(10)?.checked_add(c.to_digit(10)? as i32)?),
                'y' | 'm' | 'd' | 'h' => {
                    let v = n.take()?;
                    let campo = match c {
                        'y' => &mut p.anos,
                        'm' => &mut p.meses,
                        'd' => &mut p.dias,
                        _ => &mut p.horas,
                    };
                    *campo = campo.checked_add(v)?;
                }
                _ => return None,
            }
        }
        let horas_aprox = i64::from(p.anos) * 8766 + i64::from(p.meses) * 730 + i64::from(p.dias) * 24 + i64::from(p.horas);
        (n.is_none() && horas_aprox > 0 && horas_aprox <= 200 * 8766).then_some(p)
    }

    /// «15 días», «1 año», «1 año y 6 meses».
    pub fn texto(&self) -> String {
        let partes: Vec<String> = [(self.anos, "año", "años"), (self.meses, "mes", "meses"), (self.dias, "día", "días"), (self.horas, "hora", "horas")]
            .iter()
            .filter(|(n, _, _)| *n > 0)
            .map(|(n, uno, varios)| format!("{n} {}", if *n == 1 { uno } else { varios }))
            .collect();
        match partes.len() {
            0 => String::new(),
            1 => partes[0].clone(),
            _ => format!("{} y {}", partes[..partes.len() - 1].join(", "), partes[partes.len() - 1]),
        }
    }
}

/// Plazos de la retención (v1.28, como `restic forget --keep-within-hourly 15d`…):
/// la última versión de cada hora, día, semana, mes o año **dentro** de ese
/// tiempo, contado hacia atrás desde la versión más reciente (no desde ahora:
/// un equipo que deja de copiar no pierde versiones por esperar).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Plazos {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horarias: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diarias: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semanales: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mensuales: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anuales: Option<String>,
}

impl Plazos {
    pub fn vacio(&self) -> bool {
        self.lista().iter().all(|p| p.is_none())
    }
    /// De la hora al año.
    pub fn lista(&self) -> [Option<&str>; 5] {
        fn f(o: &Option<String>) -> Option<&str> {
            o.as_deref().filter(|s| !s.is_empty())
        }
        [f(&self.horarias), f(&self.diarias), f(&self.semanales), f(&self.mensuales), f(&self.anuales)]
    }
}

fn es_cero(n: &i64) -> bool {
    *n == 0
}

/// Retención de un repositorio: cuántas versiones horarias, diarias, semanales…
/// se guardan (`SIEMPRE`: todas las de ese tipo) y, desde v1.28, plazos
/// (`plazos`). Sin los campos nuevos se serializa como siempre, así que un
/// agente o una consola anteriores la leen igual.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Retencion {
    #[serde(default, skip_serializing_if = "es_cero")]
    pub horarias: i64,
    #[serde(default)]
    pub diarias: i64,
    #[serde(default)]
    pub semanales: i64,
    #[serde(default)]
    pub mensuales: i64,
    #[serde(default)]
    pub anuales: i64,
    #[serde(default, skip_serializing_if = "Plazos::vacio")]
    pub plazos: Plazos,
}

const PALABRAS: [&str; 5] = ["horarias", "diarias", "semanales", "mensuales", "anuales"];

impl Retencion {
    /// Las cantidades, de la hora al año.
    pub fn cantidades(&self) -> [i64; 5] {
        [self.horarias, self.diarias, self.semanales, self.mensuales, self.anuales]
    }

    /// ¿Usa algo de v1.28 (horarias, plazos o «siempre»)?
    pub fn nueva(&self) -> bool {
        self.horarias != 0 || !self.plazos.vacio() || self.cantidades().contains(&SIEMPRE)
    }

    pub fn politica(&self) -> resguardo_motor::retention::Policy {
        let p = |o: &Option<String>| o.clone().filter(|s| !s.is_empty());
        resguardo_motor::retention::Policy {
            keep_hourly: self.horarias,
            keep_daily: self.diarias,
            keep_weekly: self.semanales,
            keep_monthly: self.mensuales,
            keep_yearly: self.anuales,
            keep_within_hourly: p(&self.plazos.horarias),
            keep_within_daily: p(&self.plazos.diarias),
            keep_within_weekly: p(&self.plazos.semanales),
            keep_within_monthly: p(&self.plazos.mensuales),
            keep_within_yearly: p(&self.plazos.anuales),
            ..Default::default()
        }
    }

    /// Al menos una regla; cantidades de 0 a 1000 (o `SIEMPRE`) y plazos de restic.
    pub fn valida(&self) -> Result<(), String> {
        if self.cantidades().iter().any(|n| *n != SIEMPRE && !(0..=1000).contains(n)) {
            return Err("La retención guarda de 0 a 1000 versiones de cada tipo (o todas: «siempre»).".into());
        }
        for (p, palabra) in self.plazos.lista().iter().zip(PALABRAS) {
            if let Some(p) = p {
                if Plazo::leer(p).is_none() {
                    return Err(format!("Plazo de las {palabra} no válido: «{p}» (por ejemplo 15d, 6m o 1y)."));
                }
            }
        }
        if self.cantidades().iter().all(|n| *n == 0) && self.plazos.vacio() {
            return Err("La retención tiene que guardar al menos una versión (y como mucho 1000 de cada tipo).".into());
        }
        Ok(())
    }

    /// En palabras. Sin nada de v1.28, como siempre («7 diarias · 4 semanales ·
    /// 12 mensuales · 2 anuales»: las consolas anteriores la leen así); si no,
    /// solo lo que guarda: «horarias 15 días · diarias 1 año · mensuales siempre».
    pub fn texto(&self) -> String {
        if !self.nueva() {
            return format!("{} diarias · {} semanales · {} mensuales · {} anuales", self.diarias, self.semanales, self.mensuales, self.anuales);
        }
        let mut partes = Vec::new();
        for ((n, p), palabra) in self.cantidades().iter().zip(self.plazos.lista()).zip(PALABRAS) {
            if let Some(p) = p.and_then(Plazo::leer) {
                partes.push(format!("{palabra} {}", p.texto()));
            }
            match *n {
                SIEMPRE => partes.push(format!("{palabra} siempre")),
                0 => {}
                n => partes.push(format!("{n} {palabra}")),
            }
        }
        partes.join(" · ")
    }
}

/// Un repositorio gestionado, con su contraseña (solo en el equipo).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct RepoV2 {
    pub id: String,
    pub nombre: String,
    pub destino: String,
    pub contrasena: String,
    #[serde(default)]
    pub retencion: Option<Retencion>,
    /// Importado de otro equipo (`importar_repositorio`): solo para explorar y restaurar.
    #[serde(default)]
    pub solo_lectura: bool,
    /// Su nombre (id) dentro del destino, si no es el mismo `id` (importados).
    #[serde(default)]
    pub ubicacion_origen: Option<String>,
    /// Copia externa: `{destino, hora}` (las credenciales, en los secretos del agente).
    #[serde(default)]
    pub externa: Option<Value>,
    /// Adoptado (`adoptar_repositorio`) en un rest-server de solo añadir: la
    /// retención no se aplica desde aquí, sino en el propio servidor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solo_anadir: Option<bool>,
}

/// ¿Su servidor es de solo añadir? Lo que vio la comprobación diaria del
/// agente o, si aún no hay, la de cuando se adoptó.
pub fn solo_anadir(r: &RepoV2) -> Option<bool> {
    crate::agent::load_state().append_only.get(&r.id).and_then(|c| c.append_only).or(r.solo_anadir)
}

pub fn id_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn texto_valido(t: &str, max: usize) -> bool {
    !t.trim().is_empty() && t.chars().count() <= max && !t.chars().any(char::is_control)
}

fn texto(c: &Value, k: &str) -> String {
    c[k].as_str().unwrap_or("").to_string()
}

/// Ubicación restic del repositorio `repo` en el destino `d`.
pub fn ubicacion(d: &Destino, repo: &str) -> Result<String, String> {
    let donde = d.donde.trim();
    if !texto_valido(donde, 500) || resguardo_motor::restic::has_embedded_password(donde) || donde.starts_with('-') {
        return Err("Destino no válido (escribe la carpeta, servidor o bucket, sin contraseñas).".into());
    }
    let mut base = donde.trim_end_matches(['/', '\\']);
    // La raíz de un disco («D:\», «/») sigue siendo una carpeta completa.
    if d.tipo == "local" && (base.is_empty() || (base.len() == 2 && base.ends_with(':'))) {
        base = &donde[..(base.len() + 1).min(donde.len())];
    }
    // Sin ruta: el repositorio es el propio destino (p. ej. un rest-server con
    // un solo repositorio en su raíz, de la app de escritorio; `adoptar_repositorio`).
    if repo.is_empty() {
        return Ok(match d.tipo.as_str() {
            "local" if std::path::Path::new(base).is_absolute() => base.to_string(),
            "local" => return Err("El destino local tiene que ser una carpeta completa.".into()),
            "rest" if base.starts_with("https://") || base.starts_with("http://") => format!("rest:{base}/"),
            "rest" => return Err("El destino rest tiene que ser una dirección https://.".into()),
            "s3" => format!("s3:{base}"),
            "b2" if base.contains(':') => format!("b2:{base}"),
            "b2" => format!("b2:{base}:"),
            "sftp" if d.secreto.is_none() => format!("sftp:{base}"),
            "sftp" => return Err("SFTP usa la llave SSH del equipo: no admite contraseña.".into()),
            otro => return Err(format!("Tipo de destino no admitido: «{otro}».")),
        });
    }
    Ok(match d.tipo.as_str() {
        "local" => {
            let p = std::path::Path::new(base);
            if !p.is_absolute() {
                return Err("El destino local tiene que ser una carpeta completa.".into());
            }
            p.join(repo).display().to_string()
        }
        "rest" => {
            if !base.starts_with("https://") && !base.starts_with("http://") {
                return Err("El destino rest tiene que ser una dirección https://.".into());
            }
            format!("rest:{base}/{repo}")
        }
        "s3" => format!("s3:{base}/{repo}"),
        "b2" => {
            if base.contains(':') {
                format!("b2:{base}/{repo}")
            } else {
                format!("b2:{base}:{repo}")
            }
        }
        "sftp" => {
            if d.secreto.is_some() {
                return Err("SFTP usa la llave SSH del equipo: no admite contraseña.".into());
            }
            format!("sftp:{base}/{repo}")
        }
        otro => return Err(format!("Tipo de destino no admitido: «{otro}».")),
    })
}

/// Datos de acceso de un repositorio gestionado.
pub fn acceso(v: &Vinculo, repo_id: &str) -> Result<resguardo_motor::restic::Access, String> {
    let r = v.repos_v2.iter().find(|r| r.id == repo_id).ok_or("Ese repositorio no lo gestiona este servidor.")?;
    let d = v.destinos.iter().find(|d| d.id == r.destino).ok_or("Falta el destino de ese repositorio.")?;
    acceso_destino(d, r.ubicacion_origen.as_deref().unwrap_or(&r.id), &r.contrasena)
}

/// Datos de acceso del repositorio `ruta` del destino `d` (gestionado o no:
/// también el origen de `copiar_historial` o el que se prueba antes de adoptarlo).
pub fn acceso_destino(d: &Destino, ruta: &str, contrasena: &str) -> Result<resguardo_motor::restic::Access, String> {
    let location = ubicacion(d, ruta)?;
    let (rest_auth, env) = match d.tipo.as_str() {
        "rest" => (d.usuario.clone().map(|u| (u, d.secreto.clone().unwrap_or_default())), Vec::new()),
        "s3" | "b2" => (None, crate::tasks::cloud_env(&location, None, d.usuario.as_deref(), d.secreto.as_deref())),
        _ => (None, Vec::new()),
    };
    let cacert = match &d.ca_pem {
        Some(pem) => {
            let path = crate::agent::private_dir().join(format!("ca-destino-{}.pem", d.id));
            if std::fs::read_to_string(&path).ok().as_deref() != Some(pem.as_str()) {
                crate::agent::prepare_dir()?;
                std::fs::write(&path, pem).map_err(|e| format!("No se pudo guardar la autoridad del destino: {e}"))?;
            }
            Some(path.display().to_string())
        }
        None => None,
    };
    Ok(resguardo_motor::restic::Access { location, password: contrasena.to_string(), rest_auth, cacert, env })
}

/// `crear_repositorio {id, nombre, destino: {id} | {id, nombre, tipo, donde, usuario?, secreto?, ca_pem?}, contrasena, retencion?, parametros_de?}`.
/// Lo inicializa (o, si ya existe, comprueba que la contraseña lo abre).
pub fn crear_repositorio(v: &mut Vinculo, c: &Value) -> Result<String, String> {
    let (id, nombre, contrasena) = (texto(c, "id"), texto(c, "nombre"), texto(c, "contrasena"));
    if !id_valido(&id) {
        return Err(format!("Id de repositorio no válido: «{id}»."));
    }
    if !texto_valido(&nombre, 80) {
        return Err("Escribe un nombre para el repositorio (hasta 80 caracteres).".into());
    }
    if contrasena.chars().count() < 8 {
        return Err("La contraseña del repositorio debe tener al menos 8 caracteres.".into());
    }
    if v.repos_v2.iter().any(|r| r.id == id) {
        return Err(format!("Ya hay un repositorio «{id}» en este equipo."));
    }
    let dest = &c["destino"];
    let destino_id = texto(dest, "id");
    if !id_valido(&destino_id) {
        return Err("Id de destino no válido.".into());
    }
    let nuevo = if dest.get("tipo").is_some() {
        if v.destinos.iter().any(|d| d.id == destino_id) {
            return Err(format!("Ya hay un destino «{destino_id}»."));
        }
        let d = Destino {
            id: destino_id.clone(),
            nombre: texto(dest, "nombre"),
            tipo: texto(dest, "tipo"),
            donde: texto(dest, "donde"),
            usuario: dest["usuario"].as_str().filter(|s| !s.is_empty()).map(str::to_string),
            secreto: dest["secreto"].as_str().filter(|s| !s.is_empty()).map(str::to_string),
            ca_pem: dest["ca_pem"].as_str().filter(|s| s.contains("BEGIN CERTIFICATE")).map(str::to_string),
            equipo_almacen: equipo_almacen_de(dest),
        };
        if !texto_valido(&d.nombre, 80) {
            return Err("Escribe un nombre para el destino.".into());
        }
        ubicacion(&d, &id)?;
        Some(d)
    } else {
        if !v.destinos.iter().any(|d| d.id == destino_id) {
            return Err(format!("No hay ningún destino «{destino_id}» en este equipo."));
        }
        None
    };
    let retencion: Option<Retencion> =
        c.get("retencion").filter(|r| r.is_object()).map(|r| serde_json::from_value(r.clone())).transpose().map_err(|e| format!("Retención no válida: {e}"))?;
    // Se prueba en una copia del vínculo: si restic falla, no queda nada a medias.
    let mut prueba = v.clone();
    if let Some(d) = nuevo.clone() {
        prueba.destinos.push(d);
    }
    prueba.repos_v2.push(RepoV2 { id: id.clone(), nombre: nombre.clone(), destino: destino_id, contrasena, retencion, ..Default::default() });
    let acc = acceso(&prueba, &id)?;
    // v1.14: `parametros_de` (un origen como el de `copiar_historial`): nace con
    // los parámetros de troceado de ese repositorio, para traer su historial
    // después sin duplicar el espacio.
    let init = match c.get("parametros_de").filter(|o| o.is_object()) {
        Some(o) => crate::adoptar_v2::init_como(&prueba, &acc, o),
        None => resguardo_motor::restic::run_raw(&acc, &["init"], std::time::Duration::from_secs(300)),
    };
    let mensaje = match init {
        Ok(out) if out.code == Some(0) => format!("Repositorio «{nombre}» creado."),
        Ok(out) if out.stderr.contains("already") || out.stderr.contains("config file already exists") => {
            resguardo_motor::restic::snapshots(&acc).map_err(|e| format!("Ya hay un repositorio en ese destino y no se pudo abrir con esa contraseña: {e}"))?;
            format!("Conectado al repositorio «{nombre}» que ya existía en ese destino.")
        }
        Ok(out) => return Err(resguardo_motor::restic::exit_error(out.code, &out.stderr)),
        Err(e) => return Err(e),
    };
    *v = prueba;
    let _ = subir_config(v);
    Ok(mensaje)
}

// ---------- Configuración v1 ----------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Horario {
    /// 1 = lunes … 7 = domingo.
    #[serde(default)]
    pub dias: Vec<u8>,
    /// «HH:MM», hora local del equipo.
    #[serde(default)]
    pub horas: Vec<String>,
    /// Agente ≥ 0.7.9: reglas que se suman («cada 10 minutos de 8:00 a
    /// 18:00…», «el día 1 de cada mes…»). Si hay alguna, mandan ellas y
    /// `dias`/`horas` se ignoran (la consola los rellena, si se puede, para
    /// las consolas anteriores).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reglas: Vec<Regla>,
}

/// Una regla del horario (v1.24, agente ≥ 0.7.9). Los días, 1 = lunes … 7 = domingo.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Regla {
    /// A estas horas, los días elegidos.
    Horas { dias: Vec<u8>, horas: Vec<String> },
    /// Cada `cada_min` minutos (5, 10, 15, 20, 30 o horas enteras: 60, 120…) de `desde` a `hasta`.
    Intervalo { dias: Vec<u8>, cada_min: u32, desde: String, hasta: String },
    /// Cada `cada` días (1 a 365) desde `inicio` («AAAA-MM-DD», la primera vez), a la `hora`.
    CadaDias { cada: u32, inicio: String, hora: String },
    /// El día `dia` de cada mes (1 a 28; -1 = el último), a la `hora`.
    Mensual { dia: i8, hora: String },
}

/// Días de la consola (1 = lunes … 7 = domingo) a los del motor (0 = lunes).
fn dias_motor(dias: &[u8]) -> Result<Vec<u8>, String> {
    if dias.iter().any(|d| !(1..=7).contains(d)) {
        return Err("Día no válido (1 = lunes … 7 = domingo).".into());
    }
    Ok(dias.iter().map(|d| d - 1).collect())
}

impl Horario {
    /// El horario del plan que ejecuta el agente.
    pub(crate) fn plan_schedule(&self) -> Result<crate::plans::PlanSchedule, String> {
        use crate::plans::{PlanSchedule, ScheduleRule};
        if self.reglas.is_empty() {
            let days = dias_motor(&self.dias)?;
            return Ok(PlanSchedule {
                days,
                mode: "at".into(),
                times: self.horas.clone(),
                every_hours: 1,
                from: String::new(),
                to: String::new(),
                rules: vec![],
            });
        }
        let rules = self
            .reglas
            .iter()
            .map(|r| {
                Ok(match r {
                    Regla::Horas { dias, horas } => ScheduleRule::At { days: dias_motor(dias)?, times: horas.clone() },
                    Regla::Intervalo { dias, cada_min, desde, hasta } => {
                        ScheduleRule::Every { days: dias_motor(dias)?, every_min: *cada_min, from: desde.clone(), to: hasta.clone() }
                    }
                    Regla::CadaDias { cada, inicio, hora } => ScheduleRule::EveryDays { every: *cada, start: inicio.clone(), time: hora.clone() },
                    Regla::Mensual { dia, hora } => ScheduleRule::Monthly { day: *dia, time: hora.clone() },
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(PlanSchedule::from_rules(rules))
    }
}

fn si() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Copia {
    pub id: String,
    pub nombre: String,
    pub repo: String,
    pub carpetas: Vec<String>,
    #[serde(default)]
    pub exclusiones: Vec<String>,
    pub horario: Horario,
    #[serde(default = "si")]
    pub activa: bool,
    #[serde(default)]
    pub gancho: Option<Value>,
    /// «Solo guardar si hay cambios» (v1.16, agente ≥ 0.7.7): sin el campo,
    /// encendido (como siempre en el modo gestionado).
    #[serde(default = "si")]
    pub solo_si_cambios: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Configuracion {
    pub v: u32,
    #[serde(default)]
    pub copias: Vec<Copia>,
    /// Sin uso (lo guarda tal cual): la verificación va por repositorio en `verificaciones`.
    #[serde(default)]
    pub verificacion: Option<Value>,
    #[serde(default)]
    pub bandeja: Option<Value>,
    /// v1.28: verificación automática de cada repositorio, `{ "<repo>": { cada_dias, porcentaje } }`.
    /// Sin el campo (una consola anterior) no se toca la que haya; con él, los
    /// repositorios que no están se quedan sin verificación automática.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verificaciones: Option<std::collections::BTreeMap<String, VerificacionAuto>>,
    /// v1.36: la ventana y los avisos del escritorio (docs/agente-ventana.md §2):
    /// `{ ventana: off|siempre_disponible|al_trabajar, avisos: off|errores|todo }`.
    /// Sin el campo (una consola anterior), se queda el que hubiera.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escritorio: Option<Value>,
    /// v1.36: cuándo se cambió por última vez en el propio equipo (con la clave de
    /// administración); la consola lo enseña como «cambiado en el equipo». Lo pone
    /// el equipo: una `config` de la consola lo quita.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cambiado_en_equipo: Option<String>,
}

/// «Verificar automáticamente cada `cada_dias` días, `porcentaje` % de los datos»:
/// `restic check` y, con porcentaje, `--read-data-subset` **rotativo** (cada vez
/// la parte siguiente, así en 100/porcentaje verificaciones se lee todo).
/// 0 %: solo la estructura; 100 %: todo cada vez.
///
/// v1.40 (`admite: "verificacion_horario"`): con `horario` (el mismo que el de
/// las copias, con sus reglas), se verifica cuando toca cualquiera de ellas y
/// `cada_dias` no cuenta (la consola lo manda igual, para un agente anterior).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct VerificacionAuto {
    #[serde(default)]
    pub cada_dias: u32,
    pub porcentaje: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horario: Option<Horario>,
}

/// La primera verificación automática, de madrugada (fuera de las horas de copia).
const HORA_VERIFICACION: u32 = 3;

impl VerificacionAuto {
    /// Las reglas del horario, si lo tiene (y no está vacío).
    fn reglas(&self) -> Result<Option<Vec<crate::plans::ScheduleRule>>, String> {
        let Some(h) = self.horario.as_ref().filter(|h| !h.reglas.is_empty() || !h.horas.is_empty()) else { return Ok(None) };
        let plan = h.plan_schedule().map_err(|e| format!("Horario de la verificación: {e}"))?;
        plan.validate().map_err(|e| format!("Horario de la verificación: {e}"))?;
        Ok(Some(plan.effective_rules()))
    }

    pub fn valida(&self) -> Result<(), String> {
        if self.porcentaje > 100 {
            return Err("El porcentaje de la verificación va de 0 a 100.".into());
        }
        if self.reglas()?.is_some() {
            return Ok(());
        }
        if !(1..=31).contains(&self.cada_dias) {
            return Err("La verificación automática va de cada día a cada 31 días.".into());
        }
        if self.porcentaje > 100 {
            return Err("El porcentaje de la verificación va de 0 a 100.".into());
        }
        Ok(())
    }

    /// `(subset_percent, rotate_parts)` del agente: 0 % → solo la estructura;
    /// 100 % → todo; si no, la rotativa con 100/porcentaje partes (de 2 a 52).
    pub fn partes(&self) -> (u8, u32) {
        match self.porcentaje {
            0 => (0, 0),
            100.. => (100, 0),
            p => (0, ((100.0 / f64::from(p)).round() as u32).clamp(2, 52)),
        }
    }

    /// La de una verificación del agente (lo que enseña el resumen).
    pub fn de(v: &crate::tasks::Verify) -> Option<VerificacionAuto> {
        let porcentaje = if v.rotate_parts > 0 { (100.0 / f64::from(v.rotate_parts)).round() as u8 } else { v.subset_percent };
        match &v.schedule {
            crate::agent::Schedule::Hours { every } => Some(VerificacionAuto { cada_dias: (every / 24).max(1), porcentaje, horario: None }),
            crate::agent::Schedule::Rules { rules } => {
                // `cada_dias`: lo más que pasa entre dos (para una consola anterior).
                let hueco = crate::plans::PlanSchedule::from_rules(rules.clone()).max_gap_hours();
                let horario = Horario { dias: vec![], horas: vec![], reglas: rules.iter().map(regla_de_motor).collect() };
                Some(VerificacionAuto { cada_dias: hueco.div_ceil(24).clamp(1, 31), porcentaje, horario: Some(horario) })
            }
            _ => None,
        }
    }

    /// El horario de la verificación en el agente.
    fn schedule(&self) -> crate::agent::Schedule {
        match self.reglas() {
            Ok(Some(rules)) => crate::agent::Schedule::Rules { rules },
            _ => crate::agent::Schedule::Hours { every: self.cada_dias.clamp(1, 31) * 24 },
        }
    }

    /// La verificación del agente. Con horario, la primera cuando toque; si
    /// no, a las 03:00 siguientes (y después, cada `cada_dias` días desde la anterior).
    pub fn verify(&self, ahora: chrono::DateTime<chrono::Local>) -> crate::tasks::Verify {
        use chrono::TimeZone;
        let (subset_percent, rotate_parts) = self.partes();
        let schedule = self.schedule();
        if matches!(schedule, crate::agent::Schedule::Rules { .. }) {
            return crate::tasks::Verify { schedule, subset_percent, rotate_parts, enabled_at: ahora.to_rfc3339() };
        }
        let every = self.cada_dias.clamp(1, 31) * 24;
        let hoy = ahora.date_naive().and_hms_opt(HORA_VERIFICACION, 0, 0).and_then(|t| chrono::Local.from_local_datetime(&t).earliest());
        let primera = match hoy {
            Some(t) if t > ahora + chrono::Duration::hours(1) => t,
            Some(t) => t + chrono::Duration::days(1),
            None => ahora + chrono::Duration::hours(1),
        };
        crate::tasks::Verify { schedule, subset_percent, rotate_parts, enabled_at: (primera - chrono::Duration::hours(i64::from(every))).to_rfc3339() }
    }

    /// ¿Hace lo mismo que `v`? (Entonces se deja como está: no vuelve a empezar.)
    pub fn igual_que(&self, v: &crate::tasks::Verify) -> bool {
        let (subset, rotate) = self.partes();
        v.schedule == self.schedule() && v.subset_percent == subset && v.rotate_parts == rotate
    }
}

/// Una regla del motor (días 0 = lunes) como la de la consola (1 = lunes).
fn regla_de_motor(r: &crate::plans::ScheduleRule) -> Regla {
    use crate::plans::ScheduleRule as S;
    let dias = |d: &[u8]| d.iter().map(|x| x + 1).collect();
    match r {
        S::At { days, times } => Regla::Horas { dias: dias(days), horas: times.clone() },
        S::Every { days, every_min, from, to } => Regla::Intervalo { dias: dias(days), cada_min: *every_min, desde: from.clone(), hasta: to.clone() },
        S::EveryDays { every, start, time } => Regla::CadaDias { cada: *every, inicio: start.clone(), hora: time.clone() },
        S::Monthly { day, time } => Regla::Mensual { dia: *day, hora: time.clone() },
    }
}

/// Aplica `verificaciones` (si la configuración las trae) a los repositorios
/// con copias en el agente. Devuelve los que la tienen.
pub(crate) fn aplicar_verificaciones(v: &Vinculo, cfg: &Configuracion) -> Result<usize, String> {
    let Some(mapa) = &cfg.verificaciones else { return Ok(0) };
    let ahora = chrono::Local::now();
    let mut n = 0;
    for r in crate::agent::load_config().repos.iter().filter(|r| v.repos_v2.iter().any(|x| x.id == r.id)) {
        let quiere = mapa.get(&r.id);
        n += usize::from(quiere.is_some());
        match (quiere, &r.verify) {
            (Some(q), Some(actual)) if q.igual_que(actual) => {}
            (None, None) => {}
            (q, _) => crate::agent::set_verify(&r.id, q.map(|q| q.verify(ahora)))?,
        }
    }
    Ok(n)
}

fn plan_de(c: &Copia) -> Result<crate::plans::Plan, String> {
    let schedule = c.horario.plan_schedule().map_err(|e| format!("Copia «{}»: {e}", c.nombre))?;
    let plan: crate::plans::Plan = serde_json::from_value(json!({
        "id": c.id, "name": c.nombre, "paths": c.carpetas, "excludes": c.exclusiones,
        "schedule": schedule,
        "skip_unchanged": c.solo_si_cambios,
    }))
    .map_err(|e| e.to_string())?;
    // Ganchos: solo plantillas cerradas, validadas (ganchos.rs).
    let plan = crate::plans::Plan {
        ganchos: crate::ganchos::de_config(c.gancho.as_ref().unwrap_or(&Value::Null)).map_err(|e| format!("Copia «{}»: {e}", c.nombre))?,
        ..plan
    };
    plan.validate()?;
    Ok(plan)
}

/// `config {config}`: aplica la configuración (sin secretos) y la sube cifrada.
pub fn aplicar_config(v: &mut Vinculo, c: &Value) -> Result<String, String> {
    aplicar_config_desde(v, c, false)
}

/// `aplicar_config`, desde la consola (`en_equipo: false`) o desde el propio
/// equipo con la clave de administración (la ventana, en modo local).
pub fn aplicar_config_desde(v: &mut Vinculo, c: &Value, en_equipo: bool) -> Result<String, String> {
    let mut cfg: Configuracion = serde_json::from_value(c["config"].clone()).map_err(|e| format!("Configuración no válida: {e}"))?;
    if cfg.v != 1 {
        return Err("Versión de la configuración no admitida (se espera v = 1).".into());
    }
    match &cfg.escritorio {
        Some(e) => {
            crate::escritorio::Escritorio::validar(e)?;
        }
        // Una consola que no sabe de la ventana no deshace lo que haya.
        None => cfg.escritorio = v.config_v1.as_ref().and_then(|c| c.get("escritorio")).filter(|e| e.is_object()).cloned(),
    }
    cfg.cambiado_en_equipo = en_equipo.then(|| chrono::Local::now().to_rfc3339());
    let mut ids = std::collections::HashSet::new();
    for k in &cfg.copias {
        if !id_valido(&k.id) || !ids.insert(k.id.clone()) {
            return Err(format!("Id de copia no válido o repetido: «{}».", k.id));
        }
        if !v.repos_v2.iter().any(|r| r.id == k.repo) {
            return Err(format!("La copia «{}» usa un repositorio que este equipo no tiene: créalo antes.", k.nombre));
        }
        if v.repos_v2.iter().any(|r| r.id == k.repo && r.solo_lectura) {
            return Err(format!("La copia «{}» usa un repositorio importado (solo de lectura).", k.nombre));
        }
        if k.horario.dias.iter().any(|d| !(1..=7).contains(d)) {
            return Err(format!("Día no válido en la copia «{}» (1 = lunes … 7 = domingo).", k.nombre));
        }
        plan_de(k)?;
    }
    for (repo, va) in cfg.verificaciones.iter().flatten() {
        if !v.repos_v2.iter().any(|r| r.id == *repo && !r.solo_lectura) {
            return Err(format!("La verificación automática es de un repositorio que este equipo no tiene: «{repo}»."));
        }
        va.valida()?;
    }
    for r in v.repos_v2.clone() {
        let planes: Vec<crate::plans::Plan> = cfg.copias.iter().filter(|k| k.repo == r.id && k.activa).map(plan_de).collect::<Result<_, _>>()?;
        if planes.is_empty() {
            let _ = crate::agent::set_schedule_by_id(&r.id, None);
            continue;
        }
        let acc = acceso(v, &r.id)?;
        let repo: crate::store::Repo = serde_json::from_value(json!({
            "id": r.id, "name": r.nombre, "location": acc.location,
            "rest_username": acc.rest_auth.as_ref().map(|a| a.0.clone()), "cacert": acc.cacert, "plans": planes,
        }))
        .map_err(|e| e.to_string())?;
        crate::agent::set_schedule(&repo, Some(&acc), Some(crate::agent::Schedule::Plans))?;
    }
    let verificados = aplicar_verificaciones(v, &cfg)?;
    v.config_v1 = Some(serde_json::to_value(&cfg).map_err(|e| e.to_string())?);
    let _ = subir_config(v);
    let activas = cfg.copias.iter().filter(|k| k.activa).count();
    let n = v.repos_v2.len();
    let mut m = match activas {
        0 => "Configuración aplicada: ninguna copia activa.".to_string(),
        1 => format!("Configuración aplicada: 1 copia activa en {}.", if n == 1 { "1 repositorio".to_string() } else { format!("{n} repositorios") }),
        k => format!("Configuración aplicada: {k} copias activas en {}.", if n == 1 { "1 repositorio".to_string() } else { format!("{n} repositorios") }),
    };
    if let Some(mapa) = &cfg.verificaciones {
        let sin_copias = mapa.len().saturating_sub(verificados);
        match verificados {
            0 => {}
            1 => m.push_str(" Verificación automática en 1 repositorio."),
            k => m.push_str(&format!(" Verificación automática en {k} repositorios.")),
        }
        if sin_copias > 0 {
            m.push_str(" La verificación automática necesita alguna copia activa en el repositorio: se pondrá cuando la tenga.");
        }
    }
    Ok(m)
}

/// El documento completo que ve la consola (sin secretos): las copias de la
/// última `config` y los repositorios y destinos que tiene el equipo.
pub fn documento(v: &Vinculo) -> Value {
    let mut doc = v.config_v1.clone().unwrap_or_else(|| json!({ "v": 1, "copias": [] }));
    doc["repositorios"] =
        json!(v.repos_v2.iter().map(|r| json!({ "id": r.id, "nombre": r.nombre, "destino": r.destino, "retencion": r.retencion })).collect::<Vec<_>>());
    doc["destinos"] = json!(v.destinos.iter().map(|d| json!({ "id": d.id, "nombre": d.nombre, "tipo": d.tipo, "donde": d.donde })).collect::<Vec<_>>());
    doc
}

fn estado_de(result: &str) -> &'static str {
    match result {
        "ok" => "ok",
        "warning" => "aviso",
        _ => "fallo",
    }
}

/// v1.28: lo que entiende este agente, en `resumen.admite`: plazos y horarias
/// en la retención (también la del almacén), `config.verificaciones` y
/// `guarda_copias { anadir, local: true }` (un repositorio en su propio almacén).
/// v1.36: `consolas_multiples` (`anadir_consola`, `quitar_consola`, `resumen.consolas`) y `escritorio` (la ventana del agente).
/// v1.40: `verificacion_horario` (la verificación automática con un horario de reglas) y
/// `retencion_almacen_horario` (la retención del almacén, también con reglas).
/// (pendiente de numerar) `espejo_flexible`: el espejo del almacén con horario, selección,
/// retención y verificación por destino (docs/espejo.md).
pub const ADMITE: [&str; 10] = [
    "retencion_plazos",
    "verificacion_auto",
    "almacen_propio",
    "consolas_multiples",
    "escritorio",
    "verificacion_horario",
    "retencion_almacen_horario",
    // v1.46: copia externa a un repositorio que ya existe, con bloqueo de objetos y «Probar».
    "externa_existente",
    "espejo_flexible",
    // (pendiente de numerar) "conectar_nube" también con B2, S3, SFTP, SMB y WebDAV (docs/espejo.md §3c).
    "espejo_destinos",
];

/// Puertos que se proponen para el Servidor de copias, en orden.
const PUERTOS_PROPUESTOS: [u16; 6] = [8000, 8002, 8004, 8080, 8888, 9000];

/// El primero de los propuestos que nadie usa ahora en este equipo (si el
/// Servidor de copias ya está en marcha, su propio puerto cuenta como ocupado).
pub fn puerto_libre() -> Option<u16> {
    PUERTOS_PROPUESTOS.into_iter().find(|p| std::net::TcpListener::bind(("0.0.0.0", *p)).is_ok())
}

/// Resumen en claro (lo ve el servidor): sin rutas ni secretos.
pub fn resumen(v: &Vinculo) -> Value {
    let estado = crate::agent::load_state();
    let config = crate::agent::load_config();
    let copias: Vec<Copia> = v.config_v1.as_ref().and_then(|c| serde_json::from_value::<Configuracion>(c.clone()).ok()).map(|c| c.copias).unwrap_or_default();
    let now = chrono::Local::now();
    let pausa = config.repos.iter().filter(|r| v.repos_v2.iter().any(|x| x.id == r.id)).filter_map(|r| r.active_pause(now)).map(|p| p.until.clone()).next();
    let proximas = proximas(&copias, now);
    let cache_repos = Value::Object(v.repos_v2.iter().map(|r| (r.id.clone(), crate::informe_v2::resumen_repo(&r.id))).collect());
    let tareas = crate::tasks::load_state();
    // v1.28: la verificación automática de cada repositorio (la del agente) y cuándo toca.
    let verificacion_auto = |id: &str| {
        let r = config.repos.iter().find(|r| r.id == id)?;
        let va = VerificacionAuto::de(r.verify.as_ref()?)?;
        Some(json!({
            "cada_dias": va.cada_dias, "porcentaje": va.porcentaje,
            // v1.40: con horario, sus reglas (como las de las copias).
            "horario": va.horario,
            "proxima": crate::tasks::next_verify(r, &tareas).map(|t| t.to_rfc3339()),
            "todo_leido": tareas.rotation.get(&crate::tasks::rotation_key("verify", id)).and_then(|x| x.last_full_at.clone()),
        }))
    };
    json!({
        // v1.28: lo que este agente sabe hacer de lo nuevo (la consola no ofrece lo que no).
        "admite": ADMITE,
        // v1.36: la ventana y los avisos del escritorio (no es secreto) y si se cambiaron en el equipo.
        "escritorio": v.config_v1.as_ref().map(|c| crate::escritorio::Escritorio::de_config(c).a_json()),
        "escritorio_cambiado_en_equipo": v.config_v1.as_ref().and_then(|c| c.get("cambiado_en_equipo")).filter(|x| x.is_string()),
        "copias": copias.iter().map(|k| {
            let run = estado.runs.get(&crate::plans::plan_key(&k.repo, &k.id));
            json!({
                "id": k.id, "nombre": k.nombre, "repo": k.repo, "horario": k.horario, "carpetas": k.carpetas.len(), "activa": k.activa, "solo_si_cambios": k.solo_si_cambios,
                "ultima": run.map(|r| json!({ "cuando": r.finished, "estado": estado_de(&r.result), "mensaje": crate::web::public_message(&r.message), "bytes": r.data_added })),
                "proxima": proximas.get(&k.id),
            })
        }).collect::<Vec<_>>(),
        "repositorios": v.repos_v2.iter().map(|r| json!({
            "id": r.id, "nombre": r.nombre,
            "versiones": cache_repos[&r.id]["versiones"], "bytes": cache_repos[&r.id]["bytes"], "ultima_version": cache_repos[&r.id]["ultima_version"],
            "destino": v.destinos.iter().find(|d| d.id == r.destino).map(|d| d.nombre.clone()).unwrap_or_default(),
            "retencion": r.retencion.as_ref().map(Retencion::texto),
            // v1.28: la regla tal cual (para editarla), además del texto.
            "retencion_regla": r.retencion,
            "verificacion_auto": verificacion_auto(&r.id),
            // v1.14: solo si se sabe (rest-server de solo añadir: la retención la aplica el servidor).
            "solo_anadir": solo_anadir(r),
            // v1.22: su carpeta en un servidor rest si no es su id (adoptado): la
            // consola la busca en el almacén para su retención. Solo un nombre.
            "ruta": r.ubicacion_origen.as_ref().filter(|_| v.destinos.iter().any(|d| d.id == r.destino && d.tipo == "rest")),
            "externa": r.externa.as_ref().map(|e| json!({
                "destino": v.destinos.iter().find(|d| Some(d.id.as_str()) == e["destino"].as_str()).map(|d| d.nombre.clone()),
                "destino_id": e["destino"],
                "hora": e["hora"],
                // v1.46: a un repositorio que ya existía, con bloqueo de objetos o de solo añadir.
                "existente": e.get("existente"),
                "bloqueo_dias": e.get("bloqueo_dias"),
                "solo_anadir": e.get("solo_anadir"),
                "con_retencion": config.repos.iter().find(|x| x.id == r.id).and_then(|x| x.offsite.as_ref()).map(|o| o.retention.as_ref().is_some_and(|p| !p.is_empty())),
            })),
        })).collect::<Vec<_>>(),
        // Sin la carpeta de un destino local (es una ruta del equipo).
        // v1.30: `equipo_almacen`, el equipo que guarda copias del destino (si se sabe).
        // v1.41: de un destino local, solo qué disco es (`unidad` «D:», `extraible`, `red`): la
        // consola avisa si las copias se quedan en el mismo equipo que protegen.
        "destinos": v.destinos.iter().map(|d| {
            let mut x = json!({ "id": d.id, "nombre": d.nombre, "tipo": d.tipo, "donde": (d.tipo != "local").then(|| d.donde.clone()), "equipo_almacen": d.equipo_almacen });
            if d.tipo == "local" {
                let disco = crate::espacio::disco_de(&d.donde).json();
                for k in ["unidad", "extraible", "red"] {
                    x[k] = disco[k].clone();
                }
            }
            x
        }).collect::<Vec<_>>(),
        "pausado_hasta": pausa.map(|u| json!(u.unwrap_or_else(|| "indefinido".into()))),
        "guarda_copias": resumen_guarda_copias(),
        // v1.19: un puerto libre para «Este equipo guarda copias» (la consola lo propone).
        "puerto_libre": puerto_libre(),
        "servidores_respaldo": v.respaldo.iter().map(|r| json!({ "url": r.url, "identidad_corta": r.identidad.chars().take(8).collect::<String>() })).collect::<Vec<_>>(),
        "respaldo_dias": (!v.respaldo.is_empty()).then_some(v.respaldo_dias),
        "traslado": v.cambio.as_ref().map(|c| json!({
            "estado": "en_marcha",
            "hacia": c.destino.url,
            "hasta": chrono::DateTime::from_timestamp(c.hasta, 0).map(|d| d.to_rfc3339()),
        })),
        // v1.36: las consolas que gestionan el equipo (esta, `esta: true`) y de cuál vino el último cambio.
        "consolas": crate::consolas_v2::resumen(v),
        "cambio_config": crate::consolas_v2::resumen_cambio(v),
    })
}

/// El último resumen subido a cada consola (en memoria: al arrancar se sube una vez).
static RESUMEN_SUBIDO: std::sync::Mutex<Option<std::collections::HashMap<String, String>>> = std::sync::Mutex::new(None);

/// Con cada informe: si el resumen en claro cambió (una copia terminó, cambió
/// la próxima hora, hay versiones nuevas…), se vuelve a subir con la
/// configuración, para que la consola no muestre datos de cuando se configuró.
/// A la consola activa (`v`); cada una lleva su cuenta.
pub fn subir_resumen_si_cambio(v: &mut Vinculo) {
    let r = resumen(v).to_string();
    let clave = format!("{}|{}", v.id_enlace(), v.url);
    if RESUMEN_SUBIDO.lock().map(|g| g.as_ref().and_then(|m| m.get(&clave)) == Some(&r)).unwrap_or(true) {
        return;
    }
    if subir_config_enlace(v).is_ok() && crate::servidor_v2::guardar(v).is_ok() {
        if let Ok(mut g) = RESUMEN_SUBIDO.lock() {
            g.get_or_insert_with(Default::default).insert(clave, r);
        }
    }
}

/// Sube la configuración cifrada con `K_cfg` y el resumen en claro a la
/// consola activa y deja pendiente la de las demás (v1.36: la sube el canal de
/// cada una enseguida, cifrada con su `K_cfg`; una consola apagada no frena).
pub fn subir_config(v: &mut Vinculo) -> Result<(), String> {
    // Sin consola (modo local, docs/agente-ventana.md §5): no hay a quién subirla.
    if v.url.is_empty() {
        return Ok(());
    }
    for e in v.otras.iter_mut() {
        e.config_pendiente = true;
    }
    subir_config_enlace(v)
}

/// Sube la configuración cifrada y el resumen solo a la consola activa. Con
/// ellos van (v1.36) la etiqueta del equipo calculada con la `K_cfg` de esa
/// consola y la espera mínima: tras cambiar la clave o la espera desde otra
/// consola, esta queda al día sola. Un servidor anterior los ignora.
pub fn subir_config_enlace(v: &mut Vinculo) -> Result<(), String> {
    let Some(k) = v.k_cfg.as_deref().and_then(|k| B64.decode(k).ok()).and_then(|k| <[u8; 32]>::try_from(k).ok()) else { return Ok(()) };
    v.config_seq += 1;
    let doc = documento(v).to_string();
    let cifrado = simetrico::cifrar_config(&k, &v.equipo_id, v.config_seq, doc.as_bytes(), &simetrico::nonce_aleatorio());
    let mut cuerpo = json!({ "seq": v.config_seq, "cifrado": B64.encode(cifrado), "resumen": resumen(v), "espera_min_horas": v.espera_min_horas });
    let box_pub = resguardo_protocolo::claves::public_of(&v.box_secret).ok();
    let sign_pub = B64
        .decode(&v.sign_seed)
        .ok()
        .and_then(|s| <[u8; 32]>::try_from(s).ok())
        .map(|s| B64.encode(ed25519_dalek::SigningKey::from_bytes(&s).verifying_key().to_bytes()));
    if let (Some(b), Some(s)) = (box_pub, sign_pub) {
        cuerpo["etiqueta"] = json!(resguardo_protocolo::derivaciones::etiqueta_equipo(&k, &v.equipo_id, &b, &s));
    }
    crate::servidor_v2::llamar_ok(v, "/api/agente/config", &cuerpo)?;
    Ok(())
}

/// Informe para el servidor: estado del servicio y de las copias, sin rutas.
pub fn informe(v: Option<&Vinculo>) -> Value {
    let estado = crate::agent::load_state();
    let copias: Vec<Copia> =
        v.and_then(|v| v.config_v1.as_ref()).and_then(|c| serde_json::from_value::<Configuracion>(c.clone()).ok()).map(|c| c.copias).unwrap_or_default();
    let filas: Vec<Value> = estado
        .runs
        .iter()
        .map(|(clave, r)| {
            let (repo, copia) = clave.split_once('#').unwrap_or((clave.as_str(), ""));
            let nombre = copias.iter().find(|k| k.repo == repo && k.id == copia).map(|k| k.nombre.clone());
            json!({
                "id": copia, "repo": repo, "nombre": nombre, "estado": estado_de(&r.result), "cuando": r.finished, "mensaje": crate::web::public_message(&r.message),
                "ganchos": r.ganchos.iter().map(|g| json!({ "tipo": g.tipo, "estado": g.estado, "mensaje": crate::web::public_message(&g.mensaje) })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let mut inf = json!({ "version": crate::version_programa(), "servicio": "en_marcha", "so": crate::web::os_label(), "copias": filas, "proximas": proximas(&copias, chrono::Local::now()) });
    // El último número de orden aceptado: el servidor no vuelve por debajo (p. ej. tras restaurar la copia de la consola).
    if let Some(v) = v.filter(|v| v.ultimo_seq > 0) {
        inf["ultimo_seq"] = json!(v.ultimo_seq);
    }
    // Lo que está en marcha (v1.25): también aquí, por si el canal no pasa.
    let progreso = crate::progreso_v2::tareas(v);
    if !progreso.is_empty() {
        inf["progreso"] = json!(progreso);
    }
    // Detalle por repositorio (versiones, ejecuciones, espacio, protección), acotado de tamaño.
    if let Some(v) = v {
        inf["repos"] = json!(crate::informe_v2::repos(v));
        crate::informe_v2::acotar(&mut inf);
    }
    inf
}

/// v1.12: la próxima vez que toca cada copia, `{id: RFC 3339 | null}`. Null si
/// está desactivada, sin horario o su repositorio está en pausa sin fecha; con
/// una pausa con fecha, la primera hora después de que termine.
fn proximas(copias: &[Copia], ahora: chrono::DateTime<chrono::Local>) -> Value {
    let config = crate::agent::load_config();
    let mut out = serde_json::Map::new();
    for k in copias {
        let pausa = config.repos.iter().find(|r| r.id == k.repo).and_then(|r| r.active_pause(ahora));
        let desde = match pausa {
            None => Some(ahora),
            Some(p) => p.until.as_deref().and_then(|u| chrono::DateTime::parse_from_rfc3339(u).ok()).map(|u| u.with_timezone(&chrono::Local).max(ahora)),
        };
        out.insert(k.id.clone(), json!(proxima_de(k, desde)));
    }
    Value::Object(out)
}

fn proxima_de(k: &Copia, desde: Option<chrono::DateTime<chrono::Local>>) -> Option<String> {
    if !k.activa {
        return None;
    }
    let plan = plan_de(k).ok()?;
    plan.schedule?.next_slot(desde?).map(|t| t.to_rfc3339())
}

/// `copiar_ahora {copia}` (o `{repo, copia}`): la copia de la configuración.
pub fn copiar_ahora(v: &Vinculo, c: &Value) -> Result<String, String> {
    let copia = texto(c, "copia");
    let repo = match c["repo"].as_str() {
        Some(r) => r.to_string(),
        None => v
            .config_v1
            .as_ref()
            .and_then(|cfg| serde_json::from_value::<Configuracion>(cfg.clone()).ok())
            .and_then(|cfg| cfg.copias.into_iter().find(|k| k.id == copia).map(|k| k.repo))
            .ok_or("Esa copia no está en la configuración del equipo.")?,
    };
    if !v.repos_v2.iter().any(|r| r.id == repo) {
        return Err("Ese repositorio no lo gestiona este servidor.".into());
    }
    crate::agent::request_backup(&repo, &copia)?;
    Ok("Copia pedida: empieza en unos segundos.".into())
}

/// Repositorios a los que afecta una orden: el del cuerpo o, sin él, todos.
fn repos_de(v: &Vinculo, c: &Value) -> Result<Vec<String>, String> {
    match c["repo"].as_str() {
        Some(r) if v.repos_v2.iter().any(|x| x.id == r) => Ok(vec![r.to_string()]),
        Some(_) => Err("Ese repositorio no lo gestiona este servidor.".into()),
        None => Ok(v.repos_v2.iter().map(|r| r.id.clone()).collect()),
    }
}

/// `pausar {horas, repo?}` (0 = hasta reanudar).
pub fn pausar(v: &Vinculo, c: &Value) -> Result<String, String> {
    let horas = c["horas"].as_i64().unwrap_or(0);
    if !(0..=crate::agent::MAX_PAUSE_DAYS * 24).contains(&horas) {
        return Err(format!("La pausa puede durar como mucho {} días.", crate::agent::MAX_PAUSE_DAYS));
    }
    let hasta = (horas > 0).then(|| (chrono::Local::now() + chrono::Duration::hours(horas)).to_rfc3339());
    let config = crate::agent::load_config();
    let mut n = 0;
    for r in repos_de(v, c)? {
        // Solo los que tienen copias programadas en el agente.
        if config.repos.iter().any(|x| x.id == r) {
            crate::agent::set_pause(&r, hasta.as_deref())?;
            n += 1;
        }
    }
    Ok(match hasta {
        Some(_) => format!("Copias automáticas en pausa {horas} h ({n} repositorios)."),
        None => format!("Copias automáticas en pausa hasta reanudarlas ({n} repositorios)."),
    })
}

/// `reanudar {repo?}`.
pub fn reanudar(v: &Vinculo, c: &Value) -> Result<String, String> {
    let config = crate::agent::load_config();
    for r in repos_de(v, c)? {
        if config.repos.iter().any(|x| x.id == r) {
            crate::agent::resume(&r)?;
        }
    }
    Ok("Copias automáticas reanudadas.".into())
}

/// Un repositorio importado de otro equipo es solo para explorar y restaurar:
/// su retención la decide el equipo que lo copia (aquí se podría borrar lo suyo).
const SOLO_LECTURA: &str = "Ese repositorio es de otro equipo (importado): aquí solo se explora y se restaura.";

/// En un servidor de solo añadir nadie puede borrar desde los equipos (es lo que lo protege).
pub const SOLO_ANADIR: &str =
    "El servidor de este repositorio es de solo añadir: desde este equipo no se puede borrar nada. La retención se aplica en el propio servidor \
     (si es un almacén de Resguardo, con «Retención en el almacén» en la página del repositorio).";

/// `cambiar_retencion {repo, diarias, semanales, mensuales, anuales}` (la guarda; se aplica con `aplicar_retencion`).
pub fn cambiar_retencion(v: &mut Vinculo, c: &Value, repo: &str) -> Result<String, String> {
    let r: Retencion = serde_json::from_value(c.clone()).map_err(|e| format!("Retención no válida: {e}"))?;
    r.valida()?;
    let entrada = v.repos_v2.iter_mut().find(|x| x.id == repo).ok_or("Ese repositorio no lo gestiona este servidor.")?;
    if entrada.solo_lectura {
        return Err(SOLO_LECTURA.into());
    }
    entrada.retencion = Some(r.clone());
    let _ = subir_config(v);
    Ok(format!("Retención guardada: {}.", r.texto()))
}

/// `aplicar_retencion {repo}`: `restic forget --prune` con la retención guardada.
pub fn aplicar_retencion(v: &Vinculo, repo: &str) -> Result<String, String> {
    aplicar_retencion_por(v, repo, "orden")
}

/// Lo mismo, diciendo quién la pidió en la bitácora («Retención en detalle»):
/// `"orden"` (la consola) o `"ventana"` (la ventana del equipo).
pub fn aplicar_retencion_por(v: &Vinculo, repo: &str, por: &'static str) -> Result<String, String> {
    let r = v.repos_v2.iter().find(|x| x.id == repo).ok_or("Ese repositorio no lo gestiona este servidor.")?;
    if r.solo_lectura {
        return Err(SOLO_LECTURA.into());
    }
    if solo_anadir(r) == Some(true) {
        return Err(SOLO_ANADIR.into());
    }
    let regla = r.retencion.as_ref().ok_or("Ese repositorio no tiene retención: cámbiala antes.")?;
    let politica = regla.politica();
    let acc = acceso(v, repo)?;
    let inicio = chrono::Local::now();
    // Lo que había, para anotar qué se quitó (si no se puede leer, se aplica igual).
    let antes = resguardo_motor::restic::snapshots(&acc).ok();
    let mut args: Vec<String> = vec!["forget".into(), "--prune".into(), "--retry-lock".into(), "30m".into()];
    args.extend(politica.args());
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = resguardo_motor::restic::run_raw(&acc, &refs, std::time::Duration::from_secs(6 * 3600));
    let r = match &out {
        Ok(o) if o.code == Some(0) => Ok("Retención aplicada.".to_string()),
        Ok(o) => Err(resguardo_motor::restic::exit_error(o.code, &o.stderr)),
        Err(e) => Err(e.clone()),
    };
    if let Some(antes) = &antes {
        let despues = resguardo_motor::restic::snapshots(&acc).ok();
        let motivos = crate::retencion_registro::motivos(&crate::retencion_registro::candidatas(antes), regla);
        let salida = out.as_ref().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
        crate::retencion_registro::anotar(&crate::retencion_registro::Vuelta {
            origen: "equipo",
            por,
            repo,
            usuario: None,
            regla: Some(regla),
            inicio,
            antes,
            despues: despues.as_deref(),
            motivos: &motivos,
            copias: &crate::informe_v2::copias_por_version(),
            liberado: crate::retencion_registro::liberado(&salida).or(despues.as_ref().filter(|d| d.len() == antes.len()).map(|_| 0)),
            sospechosas: None,
            resultado: match &r {
                Ok(m) => Ok(m.as_str()),
                Err(m) => Err(m.as_str()),
            },
        });
    }
    r
}

/// `dejar_de_copiar {repo}` y `quitar_repositorio {repo}` (este, además, lo olvida con su contraseña).
pub fn dejar_de_copiar(v: &mut Vinculo, repo: &str, olvidar: bool) -> Result<String, String> {
    if !v.repos_v2.iter().any(|r| r.id == repo) {
        return Err("Ese repositorio no lo gestiona este servidor.".into());
    }
    let _ = crate::agent::set_schedule_by_id(repo, None);
    if let Some(cfg) = v.config_v1.as_mut() {
        if let Some(copias) = cfg["copias"].as_array_mut() {
            for k in copias.iter_mut().filter(|k| k["repo"] == repo) {
                k["activa"] = json!(false);
            }
        }
    }
    if olvidar {
        v.repos_v2.retain(|r| r.id != repo);
        if let Some(cfg) = v.config_v1.as_mut() {
            if let Some(copias) = cfg["copias"].as_array_mut() {
                copias.retain(|k| k["repo"] != repo);
            }
        }
    }
    let _ = subir_config(v);
    Ok(if olvidar {
        "Repositorio quitado del equipo (lo guardado sigue en su destino)."
    } else {
        "Ya no se copia en ese repositorio (lo guardado sigue ahí)."
    }
    .into())
}

/// `desvincular {modo: "dejar_de_copiar"}` y `baja_equipo`: deja de copiar en todos.
pub fn dejar_todo(v: &mut Vinculo) {
    for r in v.repos_v2.iter().map(|r| r.id.clone()).collect::<Vec<_>>() {
        let _ = crate::agent::set_schedule_by_id(&r, None);
    }
}

/// `cambiar_destino {destino, donde?, usuario?, secreto?, ca_pem?}`: nuevas
/// credenciales (o dirección) de un destino. Antes de guardarlas se comprueba
/// que cada repositorio del destino se abre con ellas; si alguno no, no cambia nada.
pub fn cambiar_destino(v: &mut Vinculo, c: &Value) -> Result<String, String> {
    let id = texto(c, "destino");
    let pos = v.destinos.iter().position(|d| d.id == id).ok_or_else(|| format!("No hay ningún destino «{id}» en este equipo."))?;
    let mut d = v.destinos[pos].clone();
    if let Some(donde) = c["donde"].as_str() {
        d.donde = donde.to_string();
    }
    for (campo, valor) in [("usuario", &mut d.usuario), ("secreto", &mut d.secreto)] {
        match c.get(campo) {
            Some(Value::Null) => *valor = None,
            Some(Value::String(s)) if !s.is_empty() => *valor = Some(s.clone()),
            Some(Value::String(_)) => *valor = None,
            _ => {}
        }
    }
    match c.get("ca_pem") {
        Some(Value::Null) => d.ca_pem = None,
        Some(Value::String(pem)) if pem.contains("BEGIN CERTIFICATE") => d.ca_pem = Some(pem.clone()),
        Some(_) => return Err("ca_pem no es un certificado.".into()),
        None => {}
    }
    let mut prueba = v.clone();
    prueba.destinos[pos] = d;
    let repos: Vec<String> = prueba.repos_v2.iter().filter(|r| r.destino == id).map(|r| r.id.clone()).collect();
    for r in &repos {
        let acc = acceso(&prueba, r)?;
        resguardo_motor::restic::snapshots(&acc).map_err(|e| format!("Con los datos nuevos no se abre el repositorio «{r}»: {e}"))?;
    }
    *v = prueba;
    // Las copias programadas, con el acceso nuevo.
    if let Some(cfg) = v.config_v1.clone() {
        aplicar_config(v, &json!({ "config": cfg }))?;
    } else {
        let _ = subir_config(v);
    }
    Ok(format!("Destino actualizado ({} repositorios comprobados).", repos.len()))
}

/// `cambiar_copia_externa {repo, destino: {id} | {id, nombre, tipo, donde, usuario?, secreto?, ca_pem?}, hora: "HH:MM" | null, retencion?, contrasena_destino?,
///  existente?, ruta?, bloqueo_dias?, solo_probar?}` (contraseña del repositorio): cada día a `hora`, `restic copy` del
/// repositorio a otro destino (otro disco, otro servidor o la nube). `hora: null` la quita (destructiva:
/// el servidor se fía del `not_before`, el agente exige la espera).
///
/// - Sin `existente`, el repositorio va en el destino en una carpeta con su id
///   (o en `ruta`) y el agente lo crea allí al guardarla (con los parámetros de
///   troceado del origen), o usa el que ya haya si la contraseña lo abre.
/// - `existente: true` (v1.46): un repositorio que **ya existe** en `ruta`
///   dentro del destino (p. ej. la subida a la nube de la app de escritorio):
///   se abre (`restic cat config`), se cuentan sus versiones y se compara su
///   troceado con el del origen. Nunca se crea: si no se abre, no se guarda nada.
///   La primera subida solo sube lo que falta.
/// - `bloqueo_dias` (v1.46, 1–3650): el destino tiene bloqueo de objetos: la
///   retención de allí solo quita versiones (`forget`, sin `prune`) y nunca de
///   los últimos días bloqueados.
/// - `solo_probar` (v1.46): solo se comprueba (el «Probar» de la consola).
pub fn cambiar_copia_externa(v: &mut Vinculo, c: &Value, repo: &str) -> Result<String, String> {
    if !v.repos_v2.iter().any(|r| r.id == repo && !r.solo_lectura) {
        return Err("Ese repositorio no lo gestiona este servidor.".into());
    }
    let solo_probar = c["solo_probar"] == true;
    if !solo_probar && !crate::agent::load_config().repos.iter().any(|r| r.id == repo) {
        return Err("Ese repositorio aún no tiene copias activas en este equipo: aplica antes una configuración con alguna copia.".into());
    }
    let hora = match c["hora"].as_str() {
        Some(h) => h.to_string(),
        None if solo_probar => String::new(),
        None => {
            crate::agent::set_offsite(repo, None, None)?;
            return Ok("Copia externa quitada (lo ya copiado sigue en su destino).".into());
        }
    };
    let dest = &c["destino"];
    let destino_id = texto(dest, "id");
    if !id_valido(&destino_id) {
        return Err("Id de destino no válido.".into());
    }
    // Cambiar la hora, la retención o el bloqueo de la misma copia externa
    // (`destino: {id}` al que ya va, sin `existente` ni `ruta`): se queda su
    // carpeta (la de uno que ya existía), su contraseña y, si no se dice, su bloqueo.
    let actual = v.repos_v2.iter().find(|r| r.id == repo).and_then(|r| r.externa.clone()).filter(|e| e["destino"] == destino_id.as_str());
    let misma = actual.is_some() && dest.get("tipo").is_none() && c.get("existente").is_none() && c.get("ruta").is_none();
    let actual = actual.filter(|_| misma).unwrap_or(Value::Null);
    let existente = c["existente"] == true || actual["existente"] == true;
    // La carpeta del repositorio en el destino: la suya (si ya existe) o su id.
    let ruta = match c["ruta"].as_str().or(actual["ruta"].as_str()).map(|r| r.trim().trim_matches('/').to_string()) {
        Some(r) if existente || !r.is_empty() => r,
        _ if existente => return Err("Falta la carpeta del repositorio que ya existe.".into()),
        _ => repo.to_string(),
    };
    if !crate::adoptar_v2::ruta_valida(&ruta) {
        return Err("La carpeta del repositorio no es válida (sin «..», «\\» ni «:»).".into());
    }
    let bloqueo = match c.get("bloqueo_dias").or(actual.get("bloqueo_dias")) {
        None | Some(Value::Null) => None,
        Some(x) => match x.as_u64() {
            Some(0) => None,
            Some(n) if n <= u64::from(crate::tasks::MAX_LOCK_DAYS) => Some(n as u32),
            _ => return Err(format!("Días de bloqueo no válidos (de 1 a {}).", crate::tasks::MAX_LOCK_DAYS)),
        },
    };
    let nuevo = if dest.get("tipo").is_some() {
        if v.destinos.iter().any(|d| d.id == destino_id) && !solo_probar {
            return Err(format!("Ya hay un destino «{destino_id}»."));
        }
        let mut d = Destino {
            id: destino_id.clone(),
            nombre: texto(dest, "nombre"),
            tipo: texto(dest, "tipo"),
            donde: texto(dest, "donde"),
            usuario: dest["usuario"].as_str().filter(|s| !s.is_empty()).map(str::to_string),
            secreto: dest["secreto"].as_str().filter(|s| !s.is_empty()).map(str::to_string),
            ca_pem: None,
            equipo_almacen: equipo_almacen_de(dest),
        };
        if existente {
            // Como al adoptar: con un nombre por defecto si no trae (B2, S3…).
            d.nombre = crate::adoptar_v2::destino_de(dest, &destino_id)?.nombre;
        }
        if !texto_valido(&d.nombre, 80) {
            return Err("Escribe un nombre para el destino.".into());
        }
        ubicacion(&d, &ruta)?;
        Some(d)
    } else {
        None
    };
    let d = match &nuevo {
        Some(d) => d.clone(),
        None => v.destinos.iter().find(|d| d.id == destino_id).ok_or_else(|| format!("No hay ningún destino «{destino_id}» en este equipo."))?.clone(),
    };
    let origen = v.repos_v2.iter().find(|r| r.id == repo).map(|r| r.destino.clone()).unwrap_or_default();
    if origen == d.id {
        return Err("La copia externa tiene que ir a otro destino (otro disco, otro servidor o la nube).".into());
    }
    let retention = c
        .get("retencion")
        .filter(|r| r.is_object())
        .map(|r| serde_json::from_value::<Retencion>(r.clone()))
        .transpose()
        .map_err(|e| format!("Retención no válida: {e}"))?;
    if let Some(r) = &retention {
        r.valida()?;
    }
    // Lo que se va a usar, comprobado antes de guardar nada.
    let src = acceso(v, repo)?;
    let contrasena_destino = c["contrasena_destino"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| misma.then(|| crate::agent::load_secrets().ok().and_then(|s| s.get(repo).and_then(|x| x.offsite_password.clone()))).flatten());
    // Sin certificado propio: la subida usa el del origen (`tasks::dest_access`).
    let d_sin_ca = Destino { ca_pem: None, ..d.clone() };
    let dest_acc = acceso_destino(&d_sin_ca, &ruta, contrasena_destino.as_deref().unwrap_or(&src.password))?;
    if dest_acc.location.trim_end_matches(['/', '\\']).eq_ignore_ascii_case(src.location.trim_end_matches(['/', '\\'])) {
        return Err("Ese es el mismo repositorio de origen: la copia externa tiene que ir a otro.".into());
    }
    // Como lo abrirá cada subida: el usuario del servidor dentro de la dirección.
    let con_auth = crate::tasks::location_with_auth(&dest_acc.location, dest_acc.rest_auth.as_ref());
    let dest_run = resguardo_motor::restic::Access {
        location: con_auth.clone(),
        password: dest_acc.password.clone(),
        rest_auth: None,
        cacert: None,
        env: dest_acc.env.clone(),
    };
    let probado = probar_externa(&src, &dest_run, existente, solo_probar)?;
    let solo_anadir = (d.tipo == "rest")
        .then(|| crate::protection::probe_append_only(&dest_acc.location, dest_acc.rest_auth.as_ref().map(|(u, p)| (u.as_str(), p.as_str())), None))
        .flatten()
        == Some(true);
    let efecto = efecto_retencion(retention.is_some(), bloqueo, solo_anadir);
    if solo_probar {
        return Ok(format!("{probado}{efecto}"));
    }
    if !(hora.len() == 5 && chrono::NaiveTime::parse_from_str(&hora, "%H:%M").is_ok()) {
        return Err("La hora no es válida (HH:MM).".into());
    }
    let mut offsite: crate::tasks::Offsite = serde_json::from_value(json!({
        "location": dest_acc.location, "provider": format!("destino:{}", d.id), "schedule": { "kind": "daily", "time": hora },
        "retention": retention.as_ref().map(Retencion::politica), "target_name": d.nombre, "enabled_at": chrono::Local::now().to_rfc3339(),
    }))
    .map_err(|e| e.to_string())?;
    offsite.dest = crate::tasks::DestinoExterno { existing: existente, object_lock_days: bloqueo, append_only: solo_anadir };
    let creds =
        crate::agent::OffsiteSecrets { password: contrasena_destino, key_id: None, key_secret: None, location: Some(con_auth), env: dest_acc.env.clone() };
    crate::agent::set_offsite(repo, Some(offsite), Some(creds))?;
    if let Some(d) = nuevo {
        v.destinos.push(d.clone());
    }
    if let Some(r) = v.repos_v2.iter_mut().find(|r| r.id == repo) {
        let mut e = json!({ "destino": d.id, "hora": hora });
        if existente {
            e["existente"] = json!(true);
        }
        // Su carpeta (solo en el equipo: el resumen no la lleva), para cambiar luego la hora sin repetirla.
        if existente || ruta != repo {
            e["ruta"] = json!(ruta);
        }
        if let Some(b) = bloqueo {
            e["bloqueo_dias"] = json!(b);
        }
        if solo_anadir {
            e["solo_anadir"] = json!(true);
        }
        r.externa = Some(e);
    }
    let _ = subir_config(v);
    Ok(format!("Copia externa a «{}» cada día a las {hora}. {probado}{efecto}", d.nombre))
}

/// Mensaje de [`probar_externa`] si el destino trocea distinto que el origen.
pub const TROCEA_DISTINTO: &str = "Atención: este repositorio trocea distinto: la primera subida ocupará como una copia completa.";

/// Abre el destino de una copia externa y dice qué pasará (sin rutas ni secretos):
/// - uno que ya existe: se abre con su contraseña, cuántas versiones tiene y
///   si trocea como el origen (si no, la primera subida ocupa como una completa);
/// - uno nuevo: si ya hay un repositorio allí se usa (si la contraseña lo
///   abre); si no, se crea ahora (salvo al solo probar).
pub fn probar_externa(
    src: &resguardo_motor::restic::Access,
    dest: &resguardo_motor::restic::Access,
    existente: bool,
    solo_probar: bool,
) -> Result<String, String> {
    use resguardo_motor::restic;
    if !existente {
        let probe = restic::run_raw(dest, &["cat", "config", "--no-lock"], restic::CHECK_TIMEOUT)?;
        return match probe.code {
            Some(0) => Ok(format!("Allí ya hay un repositorio y la contraseña lo abre: se seguirá con él. {}", troceado(src, dest)?)),
            Some(10) if solo_probar => {
                Ok("El destino responde y allí aún no hay ningún repositorio: se creará al guardar, con el mismo troceado que el origen.".into())
            }
            Some(10) => {
                crate::tasks::prepare_destination(src, dest).map_err(|e| format!("No se pudo crear el repositorio en el destino: {e}"))?;
                Ok("Repositorio creado en el destino, con el mismo troceado que el origen.".into())
            }
            code => Err(restic::exit_error(code, &probe.stderr)),
        };
    }
    let info = crate::adoptar_v2::abrir(dest)?;
    let versiones = match info.versiones {
        0 => "todavía sin versiones".to_string(),
        1 => "1 versión".to_string(),
        n => format!("{n} versiones"),
    };
    Ok(format!("El repositorio que ya existe se abre con esa contraseña ({versiones}). {}", troceado(src, dest)?))
}

/// «Trocea igual…» o el aviso de que no.
fn troceado(src: &resguardo_motor::restic::Access, dest: &resguardo_motor::restic::Access) -> Result<String, String> {
    let a = crate::tasks::chunker_polynomial(src).map_err(|e| format!("Repositorio de origen: {e}"))?;
    let b = crate::tasks::chunker_polynomial(dest)?;
    Ok(if a == b { "Trocea igual que el origen: cada subida solo sube lo que falte allí.".into() } else { TROCEA_DISTINTO.into() })
}

/// Qué pasa con la retención en el destino (para el mensaje y la consola).
pub fn efecto_retencion(con_retencion: bool, bloqueo: Option<u32>, solo_anadir: bool) -> String {
    match (con_retencion, bloqueo, solo_anadir) {
        (_, _, true) => " El destino es de solo añadir: desde aquí no se borra nada allí (la retención la aplica el propio servidor).".into(),
        (true, Some(d), _) => {
            format!(" Con bloqueo de {d} días: la retención de allí solo quita versiones de más de {d} días (forget, sin prune) y no libera espacio.")
        }
        (false, Some(d), _) => format!(" Con bloqueo de {d} días: allí no se borra nada (sin retención propia)."),
        _ => String::new(),
    }
}

/// `desbloquear {repo?}`: quita los bloqueos antiguos (`restic unlock`, que
/// nunca quita los de un proceso en marcha).
pub fn desbloquear(v: &Vinculo, c: &Value) -> Result<String, String> {
    let repos = repos_de(v, c)?;
    for r in &repos {
        let acc = acceso(v, r)?;
        let out = resguardo_motor::restic::run_raw(&acc, &["unlock"], std::time::Duration::from_secs(300))?;
        if out.code != Some(0) {
            return Err(format!("«{r}»: {}", resguardo_motor::restic::exit_error(out.code, &out.stderr)));
        }
    }
    Ok(format!("Bloqueos antiguos quitados ({} repositorios).", repos.len()))
}

/// `guarda_copias`: el Servidor de copias de este equipo (rest-server,
/// siempre solo añadir y repos privados; server.rs).
/// - `{activo: true, carpeta, puerto?, solo_red_local?}` lo activa (o cambia);
/// - `{activo: false}` lo desactiva (destructiva: los clientes dejan de poder copiar);
/// - `{anadir: "<equipo>"}` crea el usuario de un equipo cliente: su
///   contraseña va **sellada para `responder_a`** (obligatorio) con todo lo
///   que necesita `crear_repositorio` en el cliente. Con `local: true` (v1.28)
///   el cliente es **este mismo equipo** (un repositorio en su propio almacén):
///   la dirección es `https://localhost:<puerto>/<usuario>/` (el certificado
///   ya cubre `localhost`), que no depende de la IP ni del cortafuegos. Ese
///   usuario es uno más: solo añadir, solo su carpeta;
/// - `{quitar: "<usuario>"}` le quita el acceso (destructiva);
/// - `{espejo: {destinos: [{tipo:"carpeta", carpeta} | {tipo:"nube", nube, carpeta}], hora, limite_kib?}}`
///   pone el espejo nocturno (también la forma antigua `{espejo: {carpeta, hora}}`);
///   `{espejo: null}` lo quita. Quitar un destino es destructiva.
///
/// Devuelve el mensaje y, si hay, el detalle privado (para sellar).
pub fn guarda_copias(c: &Value, responder_a: bool) -> Result<(String, Option<Value>), String> {
    use crate::server;
    if let Some(nombre) = c["anadir"].as_str() {
        if !responder_a {
            return Err("Para añadir un equipo cliente, la orden tiene que llevar responder_a (la contraseña solo la ve la consola).".into());
        }
        let (usuario, contrasena, ubicacion) = server::anadir_equipo(nombre)?;
        // Que el rest-server ya lo acepte cuando el equipo cliente cree su repositorio.
        server::esperar_usuario(&usuario, &contrasena);
        let cfg = server::load();
        let ubicacion = if c["local"] == true { server::ubicacion_local(cfg.port, &usuario) } else { ubicacion };
        let privado = json!({
            "usuario": usuario, "contrasena": contrasena,
            "destino": { "tipo": "rest", "donde": ubicacion.trim_start_matches("rest:"), "usuario": usuario, "secreto": contrasena,
                         "ca_pem": std::fs::read_to_string(server::cert_file()).ok() },
            "huella_tls": cfg.tls_sha256,
        });
        return Ok((format!("Equipo cliente «{usuario}» añadido."), Some(privado)));
    }
    // §3b (docs/espejo.md): confirmar lo que falta de golpe en el almacén (espera).
    if let Some(d) = c.get("espejo_freno") {
        return Ok((crate::espejo::aceptar_freno(d)?, None));
    }
    if let Some(espejo) = c.get("espejo") {
        let m = server::poner_espejo(crate::espejo::pedido(espejo)?)?;
        return Ok((m, None));
    }
    if let Some(usuario) = c["quitar"].as_str() {
        server::quitar_equipo(usuario)?;
        return Ok((format!("«{usuario}» ya no puede guardar copias aquí (lo guardado se queda)."), None));
    }
    match c["activo"].as_bool() {
        Some(true) => {
            let carpeta = c["carpeta"].as_str().ok_or("Falta la carpeta donde guardar las copias de los demás.")?;
            let puerto = c["puerto"].as_u64().unwrap_or(8000);
            let puerto = u16::try_from(puerto).map_err(|_| "Puerto no válido.".to_string())?;
            let cfg = server::activar(carpeta, puerto, c["solo_red_local"].as_bool().unwrap_or(true))?;
            Ok((format!("Servidor de copias activado en el puerto {}.", cfg.port), None))
        }
        Some(false) => {
            server::desactivar()?;
            Ok(("Servidor de copias desactivado (las copias guardadas se quedan).".into(), None))
        }
        None => Err("Falta «activo», «anadir» o «quitar».".into()),
    }
}

/// Lo que se ve del Servidor de copias en el resumen.
pub fn resumen_guarda_copias() -> Value {
    let c = crate::server::load();
    if !c.enabled {
        return Value::Null;
    }
    json!({
        "activo": true, "puerto": c.port, "solo_red_local": c.local_subnet_only, "usuarios": c.users.len(),
        // v1.21: dónde guarda (la carpeta del almacén, como la del espejo) y los
        // repositorios de cada equipo cliente, solo nombres (`/<usuario>/<repo>/`):
        // la consola dice dónde dejar un repositorio antiguo para adoptarlo sin
        // ocupar más (docs/agente-gestionado.md, «Venir de la app de escritorio»).
        "carpeta": c.path,
        // v1.31 («¿Cuándo se llena?»): libre y total del volumen de la carpeta
        // (solo los números y cuándo se leyeron; `null` si no se puede leer).
        "espacio": crate::espacio::json_de(&c.path),
        "repositorios": crate::server::repos_by_user(&c).into_iter().map(|(usuario, repos)| json!({ "usuario": usuario, "repos": repos })).collect::<Vec<_>>(),
        "espejo": c.espejo.as_ref().map(crate::espejo::Espejo::resumen),
        // v1.22: la retención que aplica este almacén (regla, horario y
        // resultados por `usuario/repo`; nunca su clave).
        "retenciones": crate::retencion_almacen::resumen(),
        // Nubes conectadas en este equipo para el espejo (nunca sus tokens).
        "nubes": crate::nube::lista(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_destino_recuerda_su_almacen() {
        let id = "0b5c1f8e-1d2a-4c3b-9e8f-7a6b5c4d3e2f";
        assert_eq!(equipo_almacen_de(&json!({ "tipo": "rest", "equipo_almacen": id })).as_deref(), Some(id));
        assert_eq!(equipo_almacen_de(&json!({ "tipo": "rest" })), None, "una consola anterior no lo manda");
        assert_eq!(equipo_almacen_de(&json!({ "tipo": "b2", "equipo_almacen": id })), None, "solo en un rest-server");
        assert_eq!(equipo_almacen_de(&json!({ "tipo": "rest", "equipo_almacen": "../x" })), None);
        // Un vínculo guardado antes no lo tiene y se sigue leyendo; sin él, no se escribe.
        let d: Destino = serde_json::from_value(json!({ "id": "d", "nombre": "D", "tipo": "rest", "donde": "https://x" })).unwrap();
        assert!(d.equipo_almacen.is_none() && !serde_json::to_string(&d).unwrap().contains("equipo_almacen"));
        let v = Vinculo { destinos: vec![Destino { equipo_almacen: Some(id.into()), ..d }], ..Default::default() };
        assert_eq!(resumen(&v)["destinos"][0]["equipo_almacen"], id);
    }

    #[test]
    fn un_destino_local_dice_que_disco_es_sin_su_ruta() {
        let local = Destino { id: "nas".into(), nombre: "NAS".into(), tipo: "local".into(), donde: r"\\nas\carpeta-reservada".into(), ..Default::default() };
        let rest = Destino { id: "srv".into(), nombre: "Servidor".into(), tipo: "rest".into(), donde: "https://x:8000/a/".into(), ..Default::default() };
        let v = Vinculo { destinos: vec![local, rest], ..Default::default() };
        let r = resumen(&v);
        let d = &r["destinos"][0];
        assert_eq!((d["red"].as_bool(), d["extraible"].as_bool(), d["donde"].is_null()), (Some(true), Some(false), true));
        assert!(!r.to_string().contains("carpeta-reservada"), "nunca la ruta: {r}");
        assert!(r["destinos"][1].get("unidad").is_none(), "solo en los locales");
    }

    #[test]
    fn el_informe_lleva_el_ultimo_numero_de_orden() {
        let v = Vinculo { ultimo_seq: 12, ..Default::default() };
        assert_eq!(informe(Some(&v))["ultimo_seq"], 12);
        assert!(informe(Some(&Vinculo::default())).get("ultimo_seq").is_none(), "sin órdenes, nada");
        assert!(informe(None).get("ultimo_seq").is_none());
    }

    fn destino(tipo: &str, donde: &str) -> Destino {
        Destino { id: "d".into(), nombre: "D".into(), tipo: tipo.into(), donde: donde.into(), ..Default::default() }
    }

    #[test]
    fn ubicaciones_por_tipo() {
        assert_eq!(ubicacion(&destino("rest", "https://nas:8000/ana/"), "r1").unwrap(), "rest:https://nas:8000/ana/r1");
        assert_eq!(ubicacion(&destino("s3", "s3.amazonaws.com/cubo"), "r1").unwrap(), "s3:s3.amazonaws.com/cubo/r1");
        assert_eq!(ubicacion(&destino("b2", "cubo"), "r1").unwrap(), "b2:cubo:r1");
        assert_eq!(ubicacion(&destino("b2", "cubo:copias"), "r1").unwrap(), "b2:cubo:copias/r1");
        assert!(ubicacion(&destino("rest", "https://ana:clave@nas:8000/"), "r1").is_err(), "sin contraseñas en la ubicación");
        assert!(ubicacion(&destino("local", "relativa"), "r1").is_err());
        assert!(ubicacion(&destino("ftp", "x"), "r1").is_err());
        let local = if cfg!(windows) { r"D:\Copias" } else { "/srv/copias" };
        assert!(ubicacion(&destino("local", local), "r1").unwrap().ends_with("r1"));
    }

    /// v1.28: horarias, plazos y «siempre»; lo de antes se lee y se escribe igual.
    #[test]
    fn retencion_con_plazos() {
        let siigo: Retencion = serde_json::from_value(json!({
            "repo": "r", "diarias": 0, "semanales": 0, "mensuales": -1, "anuales": 0, "plazos": { "horarias": "15d", "diarias": "1y" }
        }))
        .unwrap();
        assert!(siigo.valida().is_ok());
        assert_eq!(siigo.texto(), "horarias 15 días · diarias 1 año · mensuales siempre");
        assert_eq!(siigo.politica().args(), ["--keep-within-hourly", "15d", "--keep-within-daily", "1y", "--keep-monthly", "unlimited"]);
        // Solo plazos, sin cantidades: vale (antes, «al menos una versión»).
        let p: Retencion = serde_json::from_value(json!({ "plazos": { "diarias": "30d", "semanales": "6m" } })).unwrap();
        assert!(p.valida().is_ok());
        assert_eq!(p.texto(), "diarias 30 días · semanales 6 meses");
        assert_eq!(
            serde_json::to_value(&p).unwrap(),
            json!({ "diarias": 0, "semanales": 0, "mensuales": 0, "anuales": 0, "plazos": { "diarias": "30d", "semanales": "6m" } })
        );
        // Lo de siempre, igual que antes (un agente o una consola anteriores lo leen).
        let viejo = Retencion { diarias: 7, semanales: 4, mensuales: 12, anuales: 2, ..Default::default() };
        assert_eq!(serde_json::to_value(&viejo).unwrap(), json!({ "diarias": 7, "semanales": 4, "mensuales": 12, "anuales": 2 }));
        assert_eq!(viejo.texto(), "7 diarias · 4 semanales · 12 mensuales · 2 anuales");
        assert_eq!(Retencion { horarias: 24, diarias: 7, ..Default::default() }.texto(), "24 horarias · 7 diarias");
        // Lo que no vale.
        for malo in [
            json!({ "diarias": 0 }),
            json!({ "diarias": 1001 }),
            json!({ "diarias": -2 }),
            json!({ "plazos": { "horarias": "15 días" } }),
            json!({ "plazos": { "diarias": "0d" } }),
            json!({ "diarias": 3, "plazos": { "mensuales": "1w" } }),
        ] {
            let r: Retencion = serde_json::from_value(malo.clone()).unwrap();
            assert!(r.valida().is_err(), "{malo}");
        }
        // cambiar_retencion la guarda tal cual.
        let mut v = Vinculo::default();
        v.repos_v2.push(RepoV2 { id: "r".into(), ..Default::default() });
        let m = cambiar_retencion(&mut v, &json!({ "repo": "r", "mensuales": -1, "plazos": { "horarias": "15d", "diarias": "1y" } }), "r").unwrap();
        assert_eq!(m, "Retención guardada: horarias 15 días · diarias 1 año · mensuales siempre.");
        assert_eq!(v.repos_v2[0].retencion.as_ref(), Some(&siigo));
    }

    /// v1.28: «Verificar automáticamente cada N días, X % de los datos (rotativa)».
    #[test]
    fn verificacion_automatica() {
        use chrono::TimeZone;
        let va = |cada_dias, porcentaje| VerificacionAuto { cada_dias, porcentaje, horario: None };
        assert_eq!(va(7, 0).partes(), (0, 0), "solo la estructura");
        assert_eq!(va(7, 100).partes(), (100, 0), "todo cada vez");
        assert_eq!(va(7, 10).partes(), (0, 10), "rotativa: en 10 vueltas, todo");
        assert_eq!(va(7, 5).partes(), (0, 20));
        assert_eq!(va(7, 1).partes(), (0, 52), "como mucho 52 partes");
        assert_eq!(va(7, 60).partes(), (0, 2));
        assert!(va(0, 5).valida().is_err() && va(32, 5).valida().is_err() && va(7, 101).valida().is_err());
        assert!(va(31, 100).valida().is_ok());
        // La primera, a las 03:00 siguientes; después, cada 7 días desde la anterior.
        let ahora = chrono::Local.with_ymd_and_hms(2026, 10, 4, 15, 0, 0).unwrap();
        let v = va(7, 10).verify(ahora);
        assert!(v.validate().is_ok());
        assert_eq!(v.schedule, crate::agent::Schedule::Hours { every: 168 });
        assert_eq!(v.rotate_parts, 10);
        let primera = chrono::DateTime::parse_from_rfc3339(&v.enabled_at).unwrap() + chrono::Duration::hours(168);
        assert_eq!(primera, chrono::Local.with_ymd_and_hms(2026, 10, 5, 3, 0, 0).unwrap());
        // A las 02:30, la de las 03:00 queda muy cerca: al día siguiente.
        let v = va(1, 0).verify(chrono::Local.with_ymd_and_hms(2026, 10, 4, 2, 30, 0).unwrap());
        let primera = chrono::DateTime::parse_from_rfc3339(&v.enabled_at).unwrap() + chrono::Duration::hours(24);
        assert_eq!(primera, chrono::Local.with_ymd_and_hms(2026, 10, 5, 3, 0, 0).unwrap());
        // La misma regla no vuelve a empezar; otra, sí. Y se lee de vuelta para el resumen.
        assert!(va(7, 10).igual_que(&v_de(7, 10)) && !va(7, 5).igual_que(&v_de(7, 10)) && !va(3, 10).igual_que(&v_de(7, 10)));
        assert_eq!(VerificacionAuto::de(&va(7, 10).verify(ahora)), Some(va(7, 10)));
        assert_eq!(VerificacionAuto::de(&va(3, 100).verify(ahora)), Some(va(3, 100)));
        // En la configuración: sin el campo no se toca; con él, solo repositorios del equipo.
        let cfg: Configuracion = serde_json::from_value(json!({ "v": 1, "copias": [] })).unwrap();
        assert!(cfg.verificaciones.is_none());
        assert!(!serde_json::to_string(&cfg).unwrap().contains("verificaciones"));
        let mut v = Vinculo::default();
        let c = json!({ "config": { "v": 1, "copias": [], "verificaciones": { "otro": { "cada_dias": 7, "porcentaje": 5 } } } });
        assert!(aplicar_config(&mut v, &c).unwrap_err().contains("no tiene"));
        v.repos_v2.push(RepoV2 { id: "r".into(), ..Default::default() });
        let c = json!({ "config": { "v": 1, "copias": [], "verificaciones": { "r": { "cada_dias": 40, "porcentaje": 5 } } } });
        assert!(aplicar_config(&mut v, &c).unwrap_err().contains("31 días"));
    }

    fn v_de(cada_dias: u32, porcentaje: u8) -> crate::tasks::Verify {
        VerificacionAuto { cada_dias, porcentaje, horario: None }.verify(chrono::Local::now())
    }

    /// v1.40: la verificación con un horario de reglas (como el de las copias).
    #[test]
    fn verificacion_con_horario() {
        use chrono::TimeZone;
        let h = |reglas: Value| -> Horario { serde_json::from_value(json!({ "reglas": reglas })).unwrap() };
        // Los sábados y domingos a las 02:00, y el día 1 de cada mes a las 04:00; un 20 %.
        let va = VerificacionAuto {
            cada_dias: 7,
            porcentaje: 20,
            horario: Some(h(json!([{ "tipo": "horas", "dias": [6, 7], "horas": ["02:00"] }, { "tipo": "mensual", "dia": 1, "hora": "04:00" }]))),
        };
        assert!(va.valida().is_ok());
        let ahora = chrono::Local.with_ymd_and_hms(2026, 10, 7, 15, 0, 0).unwrap(); // miércoles
        let v = va.verify(ahora);
        assert!(v.validate().is_ok());
        assert!(matches!(&v.schedule, crate::agent::Schedule::Rules { rules } if rules.len() == 2));
        assert_eq!(v.rotate_parts, 5);
        let since = chrono::DateTime::parse_from_rfc3339(&v.enabled_at).unwrap().with_timezone(&chrono::Local);
        // No toca hasta el sábado a las 02:00; entonces sí (una vez).
        assert!(!v.schedule.is_due(since, chrono::Local.with_ymd_and_hms(2026, 10, 10, 1, 59, 0).unwrap()));
        let sabado = chrono::Local.with_ymd_and_hms(2026, 10, 10, 2, 1, 0).unwrap();
        assert!(v.schedule.is_due(since, sabado));
        assert!(!v.schedule.is_due(sabado, sabado + chrono::Duration::hours(3)), "ya hecha");
        // Se lee de vuelta (con las reglas) y no vuelve a empezar si llega la misma.
        let de = VerificacionAuto::de(&v).unwrap();
        assert_eq!(de.porcentaje, 20);
        assert!((1..=31).contains(&de.cada_dias), "un número para una consola anterior");
        assert_eq!(serde_json::to_value(&de.horario).unwrap()["reglas"][0], json!({ "tipo": "horas", "dias": [6, 7], "horas": ["02:00"] }));
        assert!(va.igual_que(&v) && de.igual_que(&v));
        assert!(!VerificacionAuto { horario: None, ..va.clone() }.igual_que(&v));
        // Un horario que no vale (cada 0 días) se rechaza; sin reglas ni horas, manda `cada_dias`.
        let malo = VerificacionAuto { horario: Some(h(json!([{ "tipo": "cada_dias", "cada": 0, "inicio": "2026-10-01", "hora": "03:00" }]))), ..va.clone() };
        assert!(malo.valida().is_err());
        let vacio = VerificacionAuto { cada_dias: 3, porcentaje: 0, horario: Some(Horario { dias: vec![], horas: vec![], reglas: vec![] }) };
        assert!(vacio.valida().is_ok());
        assert_eq!(vacio.verify(ahora).schedule, crate::agent::Schedule::Hours { every: 72 });
        // Lo que la consola manda sin reglas cuando cabe en una lista de horas (`dias`/`horas`).
        let simple = VerificacionAuto { horario: Some(serde_json::from_value(json!({ "dias": [7], "horas": ["03:00"] })).unwrap()), ..va.clone() };
        assert!(simple.valida().is_ok());
        assert_eq!(
            simple.verify(ahora).schedule,
            crate::agent::Schedule::Rules { rules: vec![crate::plans::ScheduleRule::At { days: vec![6], times: vec!["03:00".into()] }] }
        );
        // Lo que manda una consola anterior (sin `horario`) sigue igual.
        let viejo: VerificacionAuto = serde_json::from_value(json!({ "cada_dias": 7, "porcentaje": 10 })).unwrap();
        assert_eq!(viejo.horario, None);
        assert!(!serde_json::to_string(&viejo).unwrap().contains("horario"));
        assert!(ADMITE.contains(&"verificacion_horario"));
    }

    #[test]
    fn retencion_no_en_importados() {
        let mut v = Vinculo::default();
        v.repos_v2.push(RepoV2 { id: "imp".into(), solo_lectura: true, retencion: Some(Retencion { diarias: 7, ..Default::default() }), ..Default::default() });
        let c = json!({"diarias": 1, "semanales": 0, "mensuales": 0, "anuales": 0});
        assert_eq!(cambiar_retencion(&mut v, &c, "imp").unwrap_err(), SOLO_LECTURA);
        assert_eq!(v.repos_v2[0].retencion.as_ref().unwrap().diarias, 7, "sin cambios");
        assert_eq!(aplicar_retencion(&v, "imp").unwrap_err(), SOLO_LECTURA);
        // Adoptado en un rest-server de solo añadir: la retención la aplica el servidor.
        let id = format!("adoptado-{}", uuid::Uuid::new_v4().simple());
        v.repos_v2.push(RepoV2 {
            id: id.clone(),
            solo_anadir: Some(true),
            retencion: Some(Retencion { diarias: 7, ..Default::default() }),
            ..Default::default()
        });
        assert_eq!(aplicar_retencion(&v, &id).unwrap_err(), SOLO_ANADIR);
    }

    #[test]
    fn ubicacion_sin_ruta() {
        assert_eq!(ubicacion(&destino("rest", "http://192.168.1.30:8001/"), "").unwrap(), "rest:http://192.168.1.30:8001/");
        assert_eq!(ubicacion(&destino("b2", "cubo"), "").unwrap(), "b2:cubo:");
        assert_eq!(ubicacion(&destino("b2", "cubo:siigo"), "").unwrap(), "b2:cubo:siigo");
        assert!(ubicacion(&destino("local", "relativa"), "").is_err());
        let raiz = if cfg!(windows) { r"D:\" } else { "/" };
        assert_eq!(ubicacion(&destino("local", raiz), "Siigo").unwrap(), std::path::Path::new(raiz).join("Siigo").display().to_string());
    }

    #[test]
    fn copia_a_plan() {
        let k = Copia {
            id: "docs".into(),
            nombre: "Documentos".into(),
            repo: "r1".into(),
            carpetas: vec!["C:\\Users\\Ana\\Documents".into()],
            exclusiones: vec![],
            horario: Horario { dias: vec![1, 7], horas: vec!["13:00".into()], reglas: vec![] },
            activa: true,
            gancho: None,
            solo_si_cambios: true,
        };
        let p = plan_de(&k).unwrap();
        assert!(p.skip_unchanged, "encendido por defecto");
        assert_eq!(p.schedule.unwrap().days, vec![0, 6], "1 = lunes → 0; 7 = domingo → 6");
        // «Solo guardar si hay cambios»: sin el campo, encendido; apagado, cada vuelta guarda versión.
        let sin: Copia =
            serde_json::from_value(json!({"id": "d", "nombre": "D", "repo": "r1", "carpetas": [], "horario": {"dias": [1], "horas": ["13:00"]}})).unwrap();
        assert!(sin.solo_si_cambios);
        assert!(!plan_de(&Copia { solo_si_cambios: false, ..k.clone() }).unwrap().skip_unchanged);
        // Ganchos: una plantilla válida pasa al plan; lo demás se rechaza.
        let con = Copia { gancho: Some(json!({"tipo": "sqlserver", "bases": ["WO"], "carpeta": "C:\\ResguardoVolcados"})), ..k.clone() };
        assert_eq!(plan_de(&con).unwrap().ganchos.len(), 1);
        let libre = Copia { gancho: Some(json!({"tipo": "orden", "linea": "cmd /c del"})), ..k.clone() };
        assert!(plan_de(&libre).is_err());
        let mala = Copia { horario: Horario { dias: vec![1], horas: vec!["25:00".into()], reglas: vec![] }, ..k };
        assert!(plan_de(&mala).is_err());
    }

    #[test]
    fn proxima_vez_de_una_copia() {
        use chrono::TimeZone;
        let k = Copia {
            id: "docs".into(),
            nombre: "Documentos".into(),
            repo: "r1".into(),
            carpetas: vec![r"C:\Datos".into()],
            exclusiones: vec![],
            horario: Horario { dias: vec![1, 2, 3, 4, 5], horas: vec!["13:00".into(), "19:00".into()], reglas: vec![] },
            activa: true,
            gancho: None,
            solo_si_cambios: true,
        };
        // Viernes 2 de octubre de 2026, 14:00: la siguiente, a las 19:00; después, el lunes.
        let vie = chrono::Local.with_ymd_and_hms(2026, 10, 2, 14, 0, 0).unwrap();
        assert!(proxima_de(&k, Some(vie)).unwrap().starts_with("2026-10-02T19:00:00"));
        let noche = chrono::Local.with_ymd_and_hms(2026, 10, 2, 20, 0, 0).unwrap();
        assert!(proxima_de(&k, Some(noche)).unwrap().starts_with("2026-10-05T13:00:00"));
        // Desactivada o en pausa sin fecha: ninguna.
        assert_eq!(proxima_de(&Copia { activa: false, ..k.clone() }, Some(vie)), None);
        assert_eq!(proxima_de(&k, None), None);
    }

    /// v1.24 (agente ≥ 0.7.9): horario con reglas.
    #[test]
    fn horario_con_reglas() {
        use chrono::TimeZone;
        let copia = |horario: Value| -> Copia {
            serde_json::from_value(json!({"id": "d", "nombre": "Docs", "repo": "r1", "carpetas": [r"C:\Datos"], "horario": horario})).unwrap()
        };
        // Cada 10 minutos de 8:00 a 18:00 de lunes a viernes, y el día 1 de cada mes a las 23:00.
        // `dias`/`horas` van vacíos (no se pueden desplegar): mandan las reglas.
        let k = copia(json!({"dias": [], "horas": [], "reglas": [
            {"tipo": "intervalo", "dias": [1, 2, 3, 4, 5], "cada_min": 10, "desde": "08:00", "hasta": "18:00"},
            {"tipo": "mensual", "dia": 1, "hora": "23:00"},
        ]}));
        let s = plan_de(&k).unwrap().schedule.unwrap();
        assert_eq!(s.mode, "rules");
        assert_eq!(s.rules.len(), 2);
        assert_eq!(s.times_of_day().len(), 61);
        // Viernes 30 de octubre, 18:05: la próxima es el domingo 1 a las 23:00.
        let vie = chrono::Local.with_ymd_and_hms(2026, 10, 30, 18, 5, 0).unwrap();
        assert!(proxima_de(&k, Some(vie)).unwrap().starts_with("2026-11-01T23:00:00"));
        let lun = chrono::Local.with_ymd_and_hms(2026, 10, 5, 10, 1, 0).unwrap();
        assert!(proxima_de(&k, Some(lun)).unwrap().starts_with("2026-10-05T10:10:00"));
        // Sin `dias` ni `horas` (solo reglas) también vale; y vuelve igual en el documento.
        let solo = copia(json!({"reglas": [{"tipo": "cada_dias", "cada": 3, "inicio": "2026-10-01", "hora": "23:00"}]}));
        assert!(plan_de(&solo).is_ok());
        assert_eq!(serde_json::to_value(&solo.horario).unwrap()["reglas"][0]["inicio"], "2026-10-01");
        // Sin reglas, el documento queda como siempre (sin el campo).
        let viejo = copia(json!({"dias": [1], "horas": ["13:00"]}));
        assert!(serde_json::to_value(&viejo.horario).unwrap().get("reglas").is_none());
        assert_eq!(plan_de(&viejo).unwrap().schedule.unwrap().mode, "at");
        // Reglas que no valen: la copia (y la configuración entera) se rechaza.
        for mala in [
            json!({"tipo": "intervalo", "dias": [1], "cada_min": 7, "desde": "08:00", "hasta": "18:00"}),
            json!({"tipo": "intervalo", "dias": [0], "cada_min": 10, "desde": "08:00", "hasta": "18:00"}),
            json!({"tipo": "intervalo", "dias": [8], "cada_min": 10, "desde": "08:00", "hasta": "18:00"}),
            json!({"tipo": "mensual", "dia": 31, "hora": "23:00"}),
            json!({"tipo": "cada_dias", "cada": 0, "inicio": "2026-10-01", "hora": "23:00"}),
            json!({"tipo": "horas", "dias": [1], "horas": []}),
        ] {
            assert!(plan_de(&copia(json!({"dias": [1], "horas": ["13:00"], "reglas": [mala.clone()]}))).is_err(), "{mala}");
        }
        // Un tipo desconocido no se entiende: «Configuración no válida».
        let raro: Result<Copia, _> = serde_json::from_value(
            json!({"id": "d", "nombre": "D", "repo": "r1", "carpetas": [], "horario": {"reglas": [{"tipo": "cada_luna", "hora": "23:00"}]}}),
        );
        assert!(raro.is_err());
    }

    /// La subida a la nube de la app de escritorio, en B2 o por su S3: la
    /// dirección que escribe la persona (partida en destino + carpeta por la
    /// consola) llega a la misma ubicación de restic, con las credenciales
    /// solo en variables del proceso.
    #[test]
    fn repositorio_que_ya_existe_en_b2_o_s3() {
        let b2 = |donde: &str| Destino {
            id: "externa-1".into(),
            nombre: "Backblaze B2".into(),
            tipo: "b2".into(),
            donde: donde.into(),
            usuario: Some("0041a2b3c4d5".into()),
            secreto: Some("K004secreto".into()),
            ..Default::default()
        };
        assert_eq!(ubicacion(&b2("cubo-copias"), "siigo").unwrap(), "b2:cubo-copias:siigo");
        assert_eq!(ubicacion(&b2("cubo-copias:copias"), "siigo").unwrap(), "b2:cubo-copias:copias/siigo");
        assert_eq!(ubicacion(&b2("cubo-copias"), "").unwrap(), "b2:cubo-copias:");
        let acc = acceso_destino(&b2("cubo-copias"), "siigo", "clave de la nube").unwrap();
        assert!(acc.env.contains(&("B2_ACCOUNT_ID".into(), "0041a2b3c4d5".into())) && acc.env.contains(&("B2_ACCOUNT_KEY".into(), "K004secreto".into())));
        assert!(!acc.location.contains("K004") && acc.rest_auth.is_none());
        let s3 = Destino { tipo: "s3".into(), donde: "https://s3.us-west-004.backblazeb2.com/cubo-copias".into(), ..b2("") };
        let acc = acceso_destino(&s3, "siigo", "clave de la nube").unwrap();
        assert_eq!(acc.location, "s3:https://s3.us-west-004.backblazeb2.com/cubo-copias/siigo");
        assert!(acc.env.contains(&("AWS_ACCESS_KEY_ID".into(), "0041a2b3c4d5".into())));
        // Una contraseña dentro de la dirección no se acepta.
        assert!(ubicacion(&Destino { tipo: "rest".into(), donde: "https://ana:clave@nas:8000".into(), ..b2("") }, "siigo").is_err());
    }

    /// «Copia externa → Usar uno que ya existe» y su «Probar»: se abre con su
    /// contraseña, se cuentan sus versiones y se compara su troceado con el del
    /// origen. Nada cambia en el equipo y los mensajes no llevan rutas ni claves.
    #[test]
    fn probar_copia_externa_a_un_repositorio_que_ya_existe() {
        if resguardo_motor::restic::version().is_err() {
            return; // sin restic
        }
        use resguardo_motor::restic::{run_raw, Access};
        let b = std::env::temp_dir().join(format!("resguardo-externa-probar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&b);
        std::fs::create_dir_all(b.join("datos")).unwrap();
        std::fs::write(b.join("datos").join("factura.txt"), "factura 1").unwrap();
        let ok = |acc: &Access, args: &[&str]| {
            let out = run_raw(acc, args, std::time::Duration::from_secs(300)).unwrap();
            assert_eq!(out.code, Some(0), "{args:?}: {}", out.stderr);
        };
        // El origen, gestionado («siigo» en un disco).
        let origen = Access::new(b.join("almacen").join("siigo").display().to_string(), "clave del origen");
        ok(&origen, &["init"]);
        ok(&origen, &["backup", "--host", "PC-CONTABLE", &b.join("datos").display().to_string()]);
        // La «nube»: uno con el troceado del origen y una versión, y otro que trocea distinto.
        let nube = Access::new(b.join("nube").join("copias").join("siigo").display().to_string(), "clave de la nube");
        let mut desde = nube.clone();
        desde.env.push(("RESTIC_FROM_PASSWORD".into(), origen.password.clone()));
        ok(&desde, &["init", "--copy-chunker-params", "--from-repo", &origen.location]);
        ok(&desde, &["copy", "--from-repo", &origen.location]);
        ok(&Access::new(b.join("nube").join("otro").display().to_string(), "clave de la nube"), &["init"]);

        let mut v = Vinculo { equipo_id: uuid::Uuid::new_v4().to_string(), modo: "local".into(), ..Default::default() };
        v.destinos.push(Destino {
            id: "almacen".into(),
            nombre: "Disco".into(),
            tipo: "local".into(),
            donde: b.join("almacen").display().to_string(),
            ..Default::default()
        });
        v.repos_v2.push(RepoV2 {
            id: "siigo".into(),
            nombre: "Siigo".into(),
            destino: "almacen".into(),
            contrasena: origen.password.clone(),
            ..Default::default()
        });
        let antes = serde_json::to_string(&(&v.destinos, &v.repos_v2.iter().map(|r| &r.externa).collect::<Vec<_>>())).unwrap();
        let pedir = |ruta: Option<&str>, clave: &str| {
            let mut c = json!({ "repo": "siigo", "hora": "21:00", "solo_probar": true, "existente": true, "contrasena_destino": clave, "bloqueo_dias": 30,
                                "retencion": { "diarias": 7, "semanales": 4, "mensuales": 12, "anuales": 2 },
                                "destino": { "id": "externa-1a2b", "tipo": "local", "donde": b.join("nube").display().to_string() } });
            if let Some(r) = ruta {
                c["ruta"] = json!(r);
            }
            c
        };
        let sin_secretos = |m: &str| {
            assert!(!m.contains("clave") && !m.contains(&b.display().to_string()), "sin rutas ni claves: {m}");
        };
        let m = cambiar_copia_externa(&mut v, &pedir(Some("copias/siigo"), "clave de la nube"), "siigo").unwrap();
        assert!(m.contains("se abre con esa contraseña (1 versión)") && m.contains("Trocea igual"), "{m}");
        assert!(m.contains("Con bloqueo de 30 días") && m.contains("sin prune"), "{m}");
        sin_secretos(&m);
        // Trocea distinto: se avisa, pero se puede usar.
        let m = cambiar_copia_externa(&mut v, &pedir(Some("otro"), "clave de la nube"), "siigo").unwrap();
        assert!(m.contains(TROCEA_DISTINTO) && m.contains("todavía sin versiones"), "{m}");
        // Lo que no vale.
        let mut mucho_bloqueo = pedir(Some("copias/siigo"), "clave de la nube");
        mucho_bloqueo["bloqueo_dias"] = json!(9999);
        for (c, error) in [
            (pedir(Some("copias/siigo"), "otra clave"), "La contraseña no abre"),
            (pedir(Some("copias/no-esta"), "clave de la nube"), "No hay ningún repositorio"),
            (pedir(None, "clave de la nube"), "Falta la carpeta"),
            (pedir(Some("../siigo"), "clave de la nube"), "no es válida"),
            (mucho_bloqueo, "Días de bloqueo"),
        ] {
            let e = cambiar_copia_externa(&mut v, &c, "siigo").unwrap_err();
            assert!(e.contains(error), "{error}: {e}");
            sin_secretos(&e);
        }
        // El mismo repositorio de origen, no.
        let mut mismo = pedir(Some("siigo"), "clave del origen");
        mismo["destino"]["donde"] = json!(b.join("almacen").display().to_string());
        assert!(cambiar_copia_externa(&mut v, &mismo, "siigo").unwrap_err().contains("mismo repositorio"));
        // Uno nuevo donde aún no hay nada: se creará al guardar (al probar, no).
        let mut nuevo = pedir(None, "clave de la nube");
        nuevo["existente"] = json!(false);
        nuevo["destino"]["nombre"] = json!("Nube");
        let m = cambiar_copia_externa(&mut v, &nuevo, "siigo").unwrap();
        assert!(m.contains("se creará al guardar"), "{m}");
        assert!(!b.join("nube").join("siigo").exists(), "probar no crea nada");
        // Nada cambió en el equipo.
        assert_eq!(serde_json::to_string(&(&v.destinos, &v.repos_v2.iter().map(|r| &r.externa).collect::<Vec<_>>())).unwrap(), antes);
        assert_eq!(
            efecto_retencion(true, None, true),
            " El destino es de solo añadir: desde aquí no se borra nada allí (la retención la aplica el propio servidor)."
        );
        assert!(efecto_retencion(false, Some(30), false).contains("allí no se borra nada"));
        assert_eq!(efecto_retencion(true, None, false), "");
        // Cambiar solo la hora de una que va a uno que ya existía (`destino: {id}`):
        // sigue en su carpeta y con su bloqueo, sin crear nada en la carpeta del id.
        v.destinos.push(Destino {
            id: "nube-ex".into(),
            nombre: "Nube".into(),
            tipo: "local".into(),
            donde: b.join("nube").display().to_string(),
            ..Default::default()
        });
        v.repos_v2[0].externa = Some(json!({ "destino": "nube-ex", "hora": "21:00", "existente": true, "ruta": "copias/siigo", "bloqueo_dias": 30 }));
        let hora = json!({ "repo": "siigo", "hora": "22:00", "solo_probar": true, "contrasena_destino": "clave de la nube", "destino": { "id": "nube-ex" },
                           "retencion": { "diarias": 7, "semanales": 4, "mensuales": 12, "anuales": 2 } });
        let m = cambiar_copia_externa(&mut v, &hora, "siigo").unwrap();
        assert!(m.contains("que ya existe se abre") && m.contains("(1 versión)") && m.contains("Con bloqueo de 30 días"), "{m}");
        let _ = std::fs::remove_dir_all(&b);
    }
}
