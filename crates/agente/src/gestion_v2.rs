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
    /// "local", "rest", "s3", "b2", "sftp" o (tarea 4a) "nube": una nube conectada en
    /// este equipo (`nube.rs`), con `nube` su nombre y `donde` la carpeta dentro de ella.
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
    /// Tarea 4a: en un destino "nube", el nombre de la nube conectada en este equipo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nube: Option<String>,
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
    /// Plan 0.8.0, F3 (14u, `admite: "retencion_por_dia"`): «N versiones por día».
    /// Solo la aplica el planificador propio (el del almacén y, con esto, el del
    /// equipo): restic no tiene esa regla. Un agente anterior la ignora (la consola
    /// no se la ofrece).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub por_dia: Option<PorDia>,
}

/// F3 (14u): de cada día (en la hora de cada versión), las versiones más cercanas a
/// estas horas o, sin horas, la primera y la última del día. Dentro de `plazo`
/// (contado desde la más reciente, como los demás plazos); sin él, todos los días.
/// Se suma a lo demás de la regla (lo que guarda cualquiera de las dos, se queda) y
/// vale también para la historia que ya existe: no depende de etiquetas al copiar.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct PorDia {
    /// «HH:MM» (1 a 12, sin repetir). Vacío: la primera y la última del día.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub horas: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plazo: Option<String>,
}

/// F3: como mucho, tantas horas en «N versiones por día».
pub const MAX_HORAS_POR_DIA: usize = 12;

impl PorDia {
    /// Cuántas se quedan de cada día.
    pub fn cuantas(&self) -> usize {
        if self.horas.is_empty() {
            2
        } else {
            self.horas.len()
        }
    }

    pub fn valida(&self) -> Result<(), String> {
        if self.horas.len() > MAX_HORAS_POR_DIA {
            return Err(format!("«Versiones por día» admite hasta {MAX_HORAS_POR_DIA} horas."));
        }
        let mut vistas = std::collections::HashSet::new();
        for h in &self.horas {
            if h.len() != 5 || chrono::NaiveTime::parse_from_str(h, "%H:%M").is_err() || !vistas.insert(h.as_str()) {
                return Err(format!("Hora no válida en «Versiones por día»: «{h}» (HH:MM, sin repetir)."));
            }
        }
        if let Some(p) = self.plazo.as_deref().filter(|p| !p.is_empty()) {
            if Plazo::leer(p).is_none() {
                return Err(format!("Plazo de «Versiones por día» no válido: «{p}» (por ejemplo 30d o 6m)."));
            }
        }
        Ok(())
    }

    /// «2 por día (la primera y la última)», «3 por día, las más cercanas a las 09:00, 13:00 y 18:00, 30 días».
    pub fn texto(&self) -> String {
        let cuales = if self.horas.is_empty() {
            "la primera y la última".to_string()
        } else {
            let mut hs = self.horas.clone();
            hs.sort();
            let lista = match hs.len() {
                1 => hs[0].clone(),
                n => format!("{} y {}", hs[..n - 1].join(", "), hs[n - 1]),
            };
            format!("las más cercanas a {} {lista}", if hs.len() == 1 { "la" } else { "las" })
        };
        let plazo = self.plazo.as_deref().and_then(Plazo::leer).map(|p| format!(" durante {}", p.texto())).unwrap_or_default();
        format!("{} por día ({cuales}){plazo}", self.cuantas())
    }
}

const PALABRAS: [&str; 5] = ["horarias", "diarias", "semanales", "mensuales", "anuales"];

impl Retencion {
    /// Las cantidades, de la hora al año.
    pub fn cantidades(&self) -> [i64; 5] {
        [self.horarias, self.diarias, self.semanales, self.mensuales, self.anuales]
    }

    /// ¿Usa algo de v1.28 (horarias, plazos o «siempre»), o «por día»?
    pub fn nueva(&self) -> bool {
        self.horarias != 0 || !self.plazos.vacio() || self.cantidades().contains(&SIEMPRE) || self.por_dia.is_some()
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
        if let Some(d) = &self.por_dia {
            d.valida()?;
        }
        if self.cantidades().iter().all(|n| *n == 0) && self.plazos.vacio() && self.por_dia.is_none() {
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
        if let Some(d) = &self.por_dia {
            partes.push(d.texto());
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
    /// Tarea 4b: las demás copias derivadas, `[{ id, destino, cuando, existente?, ruta?,
    /// bloqueo_dias?, solo_anadir?, filtro?, verificacion? }]` (las credenciales, en los
    /// secretos del agente; ver [`cambiar_derivada`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derivadas: Vec<Value>,
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
    // Tarea 4a: una nube conectada en este equipo, por rclone (`rclone:rnube:<carpeta>/<repo>`).
    if d.tipo == "nube" {
        let carpeta = d.donde.trim().trim_matches('/');
        if (!carpeta.is_empty() && !crate::nube::carpeta_remota_valida(carpeta)) || d.nube.as_deref().is_none_or(|n| n.trim().is_empty()) {
            return Err("Destino en la nube no válido (la nube conectada y una carpeta dentro de ella).".into());
        }
        if repo.is_empty() {
            return Err("En una nube, el repositorio va en una carpeta: di cuál.".into());
        }
        return Ok(crate::nube::ubicacion_restic(carpeta, repo));
    }
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
        // Tarea 4a: las credenciales de la nube, de ahora (con el token al día).
        "nube" => (None, crate::nube::entorno_restic(d.nube.as_deref().unwrap_or_default())?),
        _ => (None, Vec::new()),
    };
    let cacert = archivo_ca(d)?;
    Ok(resguardo_motor::restic::Access { location, password: contrasena.to_string(), rest_auth, cacert, env })
}

/// La autoridad TLS propia de un destino (un almacén) en un archivo de la carpeta privada
/// (lo que pide `--cacert`), o `None` si no tiene.
fn archivo_ca(d: &Destino) -> Result<Option<String>, String> {
    let Some(pem) = &d.ca_pem else { return Ok(None) };
    let path = crate::agent::private_dir().join(format!("ca-destino-{}.pem", d.id));
    if std::fs::read_to_string(&path).ok().as_deref() != Some(pem.as_str()) {
        crate::agent::prepare_dir()?;
        std::fs::write(&path, pem).map_err(|e| format!("No se pudo guardar la autoridad del destino: {e}"))?;
    }
    Ok(Some(path.display().to_string()))
}

/// 14jj: la autoridad TLS del destino `id` del equipo (para las copias externas o derivadas
/// guardadas antes de llevarla consigo), si la tiene.
pub fn ca_del_destino(id: &str) -> Option<String> {
    let v = crate::servidor_v2::cargar()?;
    let d = v.destinos.iter().find(|d| d.id == id)?;
    archivo_ca(d).ok().flatten()
}

/// Tarea 4a: la nube de un destino `nube`, conectada en este equipo.
fn nube_conectada(d: &Destino) -> Result<(), String> {
    let n = d.nube.as_deref().unwrap_or_default();
    if crate::nube::buscar(n).is_none() {
        return Err(format!("La nube «{n}» no está conectada en este equipo: conéctala antes."));
    }
    Ok(())
}

/// `crear_repositorio {id, nombre, destino: {id} | {id, nombre, tipo, donde, usuario?, secreto?, ca_pem?, nube?}, contrasena, retencion?, parametros_de?}`.
/// Con `tipo: "nube"` (`repo_en_nube`), `nube` es el nombre de una nube conectada en este equipo y `donde` su carpeta.
/// Lo inicializa (o, si ya existe, comprueba que la contraseña lo abre).
pub fn crear_repositorio(v: &mut Vinculo, c: &Value) -> Result<String, String> {
    // Plan 0.8.0 (B3, 14hh): solo el acceso a una zona de un almacén, sin repositorio.
    if c["solo_destino"] == true {
        return poner_solo_destino(v, &c["destino"]);
    }
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
    // Tarea 4a: también en una nube conectada en este equipo (`repo_en_nube`): cada
    // proceso de restic recibe la nube al día (`nube::preparar_vuelta`).
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
            // Tarea 4a: el nombre de la nube conectada en este equipo.
            nube: (dest["tipo"] == "nube").then(|| texto(dest, "nube").trim().to_string()),
        };
        if !texto_valido(&d.nombre, 80) {
            return Err("Escribe un nombre para el destino.".into());
        }
        if d.tipo == "nube" {
            nube_conectada(&d)?;
        }
        ubicacion(&d, &id)?;
        Some(d)
    } else {
        let d = v.destinos.iter().find(|d| d.id == destino_id).ok_or_else(|| format!("No hay ningún destino «{destino_id}» en este equipo."))?;
        if d.tipo == "nube" {
            nube_conectada(d)?;
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

/// Plan 0.8.0 (B3, 14hh): `crear_repositorio { solo_destino: true, destino }` (clave de
/// administración, `admite: "solo_destino"`): guarda el acceso que dio un almacén
/// (`guarda_copias { anadir }`) como destino de este equipo **sin crear ningún
/// repositorio**, para que una copia derivada (o un repositorio después) pueda ir a
/// esa zona. Solo un servidor de copias de un almacén (`rest` con `equipo_almacen`,
/// https y su autoridad). Si ya estaba, se ponen al día su dirección y sus credenciales.
fn poner_solo_destino(v: &mut Vinculo, dest: &Value) -> Result<String, String> {
    let id = texto(dest, "id");
    if !id_valido(&id) {
        return Err("Id de destino no válido.".into());
    }
    let Some(almacen) = equipo_almacen_de(dest) else {
        return Err("Solo se da acceso así a una zona de un almacén (un servidor de copias con su equipo).".into());
    };
    let d = Destino {
        id: id.clone(),
        nombre: texto(dest, "nombre").trim().to_string(),
        tipo: "rest".into(),
        donde: texto(dest, "donde").trim().to_string(),
        usuario: dest["usuario"].as_str().filter(|s| !s.is_empty()).map(str::to_string),
        secreto: dest["secreto"].as_str().filter(|s| !s.is_empty()).map(str::to_string),
        ca_pem: dest["ca_pem"].as_str().filter(|s| s.contains("BEGIN CERTIFICATE")).map(str::to_string),
        equipo_almacen: Some(almacen.clone()),
        nube: None,
    };
    if !texto_valido(&d.nombre, 80) {
        return Err("Escribe un nombre para el destino.".into());
    }
    if !d.donde.starts_with("https://") {
        return Err("La dirección del almacén tiene que ser https://.".into());
    }
    if d.usuario.is_none() || d.secreto.is_none() || d.ca_pem.is_none() {
        return Err("Faltan el usuario, la contraseña o la autoridad del almacén.".into());
    }
    ubicacion(&d, "")?;
    let mensaje = match v.destinos.iter_mut().find(|x| x.id == id) {
        Some(x) if x.tipo != "rest" || x.equipo_almacen.as_deref() != Some(almacen.as_str()) => {
            return Err(format!("Ya hay un destino «{id}» que no es de ese almacén."));
        }
        Some(x) => {
            *x = d;
            format!("Acceso a «{}» puesto al día (sin crear ningún repositorio).", x.nombre)
        }
        None => {
            let m = format!("Este equipo ya puede guardar en «{}» (sin crear ningún repositorio).", d.nombre);
            v.destinos.push(d);
            m
        }
    };
    let _ = subir_config(v);
    Ok(mensaje)
}

// ---------- Configuración v1 ----------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
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

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
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
    /// Tarea 7c (`admite: "cadenas"`): «después de la anterior», el id de otra
    /// copia de esta configuración. Empieza cuando aquella termina bien; si
    /// falla, esta no se hace y se avisa (`cadena_parada`). Puede tener además
    /// su horario, o ninguno (`horario` vacío). Un agente anterior lo ignora.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tras: Option<String>,
    /// Plan 0.7.26 (`admite: "inicio_despues"`): cómo empieza. `horario` (sin
    /// `tras`), `cadena` (tras la anterior, solo si sale bien: lo de siempre con
    /// `tras`) o `despues` (tras la anterior, aunque falle). Sin el campo y con
    /// `tras`, `cadena`. Un agente anterior lo ignora.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inicio: Option<String>,
    /// Plan 0.7.26: minutos de espera tras terminar la anterior (0 a 1440; sin
    /// el campo, 0: enseguida). Solo con `tras`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retraso_min: Option<u32>,
    /// F1 (14pp, `admite: "termina"`): «Solo a mano»: sin horario, solo «Copiar
    /// ahora». No cuenta como atrasada. Un agente anterior lo ignora (y rechaza la
    /// copia sin horario: la consola no se lo ofrece).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub a_mano: bool,
    /// F1: cuándo termina («Una sola vez», tras N copias o el día …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub termina: Option<Termina>,
    /// 15hh (5, `admite: "parar"`): la velocidad de subida según la hora, `{ kib?, franjas? }`
    /// como en las copias derivadas (14t). Un agente anterior lo ignora.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limite: Option<Value>,
    /// 15ll (`admite: "sin_suspension"`): no pedir al sistema que no se suspenda mientras
    /// copia (equipos con batería). Sin el campo, sí se pide.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub permitir_suspension: bool,
    /// 0.8.1 (15v, `admite: "etiquetas_copia"`): las etiquetas de sus versiones (hasta 10,
    /// sin espacios ni comas; sin ninguna, `resguardo-agente`). Cambiarlas solo afecta a las
    /// versiones nuevas. Un agente anterior las ignora (y sigue con `resguardo-agente`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub etiquetas: Vec<String>,
    /// 0.8.1 (15y, `admite: "grupos_en_copias"`): de qué grupos de exclusiones del cliente
    /// vienen reglas de `exclusiones`, con las reglas que tenía cada uno al aplicarlo (para
    /// que la consola sepa qué copias usan un grupo y si ya tienen su última versión). Las
    /// reglas que se aplican son siempre las de `exclusiones`: el equipo no depende de la consola.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grupos_exclusiones: Vec<GrupoUsado>,
    /// 15y: «No copiar archivos de más de N MB» (1 a 1 000 000).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mayores_mb: Option<u32>,
    /// 15y: tipos que se copian aunque pasen del límite (`*.pst`, `*.bak`; hasta 50).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excepto_tamano: Vec<String>,
    /// 0.8.1 (15ii, `admite: "equipos_moviles"`): además del horario, una vez al volver a
    /// conectarse si la última copia buena es antigua.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub al_reconectar: bool,
    /// 0.8.1 (15kk, `admite: "apagar_al_terminar"`): al terminar bien, `apagar` o `hibernar`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub al_terminar: Option<String>,
    /// 15kk: confirmado expresamente en un equipo que guarda copias de otros (almacén).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub al_terminar_servidor: bool,
}

/// 15y: un grupo de exclusiones aplicado a una copia (sus reglas, tal como estaban).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct GrupoUsado {
    pub id: String,
    #[serde(default)]
    pub nombre: String,
    #[serde(default)]
    pub reglas: Vec<String>,
}

/// 15y: como mucho, grupos aplicados a una copia.
pub const MAX_GRUPOS_COPIA: usize = 50;

/// F1 (14pp): cuándo termina una copia. Al terminar queda **completada**: no se
/// hace sola ni avisa, y conserva su repositorio, sus versiones y su retención.
/// La cuenta la lleva el agente (sin consola) y vuelve a empezar si esto cambia.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Termina {
    /// Después de N copias correctas (1 a 10 000).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub veces: Option<u32>,
    /// El último día en que se copia (AAAA-MM-DD, hora del equipo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hasta: Option<String>,
    /// «Una sola vez», a esta hora (AAAA-MM-DDTHH:MM, hora del equipo). Sin horario.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub una_vez: Option<String>,
    /// Desde cuándo cuenta (RFC 3339; lo pone la consola al poner el fin o al reanudar).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desde: Option<String>,
}

/// F1: como mucho, tantas copias antes de terminar.
pub const MAX_VECES: u32 = 10_000;

impl Termina {
    /// ¿Dice algo? (Uno vacío es «nunca».)
    pub fn algo(&self) -> bool {
        self.veces.is_some() || self.hasta.is_some() || self.una_vez.is_some()
    }

    pub fn valida(&self, nombre: &str) -> Result<(), String> {
        if let Some(n) = self.veces {
            if !(1..=MAX_VECES).contains(&n) {
                return Err(format!("La copia «{nombre}» termina después de 1 a {MAX_VECES} copias."));
            }
        }
        if let Some(h) = &self.hasta {
            if chrono::NaiveDate::parse_from_str(h, "%Y-%m-%d").is_err() {
                return Err(format!("El día en que termina la copia «{nombre}» no es válido (AAAA-MM-DD)."));
            }
        }
        if let Some(u) = &self.una_vez {
            if u.len() != 16 || chrono::NaiveDateTime::parse_from_str(u, "%Y-%m-%dT%H:%M").is_err() {
                return Err(format!("La hora de «Una sola vez» de la copia «{nombre}» no es válida (AAAA-MM-DDTHH:MM)."));
            }
            if self.veces.is_some() || self.hasta.is_some() {
                return Err(format!("La copia «{nombre}» se hace una sola vez: no lleva además un número de copias ni un último día."));
            }
        }
        if self.desde.as_deref().is_some_and(|d| d.len() > 40 || chrono::DateTime::parse_from_rfc3339(d).is_err()) {
            return Err(format!("La fecha desde la que cuenta la copia «{nombre}» no es válida."));
        }
        Ok(())
    }
}

/// Plan 0.7.26: como mucho, un día de retraso tras la anterior.
pub const MAX_RETRASO_MIN: u32 = 1440;

impl Copia {
    /// 15y: los grupos de exclusiones, el límite de tamaño y sus excepciones, válidos.
    pub fn valida_exclusiones(&self) -> Result<(), String> {
        let n = &self.nombre;
        if self.grupos_exclusiones.len() > MAX_GRUPOS_COPIA {
            return Err(format!("La copia «{n}» lleva demasiados grupos de exclusiones (como mucho {MAX_GRUPOS_COPIA})."));
        }
        for g in &self.grupos_exclusiones {
            if !id_valido(&g.id) || g.nombre.chars().count() > 60 || g.nombre.chars().any(char::is_control) {
                return Err(format!("Un grupo de exclusiones de la copia «{n}» no es válido."));
            }
            crate::excluir::reglas_validas(&g.reglas, 100, &format!("Las reglas del grupo «{}»", g.nombre))?;
        }
        if self.mayores_mb.is_some_and(|m| !(1..=crate::excluir::MAX_MB).contains(&m)) {
            return Err(format!("El tamaño máximo de la copia «{n}» va de 1 MB a {} MB.", crate::excluir::MAX_MB));
        }
        crate::excluir::reglas_validas(&self.excepto_tamano, crate::excluir::MAX_EXCEPCIONES, &format!("Las excepciones al tamaño de «{n}»"))?;
        Ok(())
    }

    /// Lo nuevo del plan en el agente (0.8.1, bloque D).
    pub fn extra(&self) -> crate::agent::PlanExtra {
        crate::agent::PlanExtra {
            larger_mb: self.mayores_mb,
            larger_except: if self.mayores_mb.is_some() { self.excepto_tamano.clone() } else { vec![] },
            on_reconnect: self.al_reconectar,
            on_finish: self.al_terminar.clone(),
        }
    }

    /// ¿Tiene horario propio? (Uno vacío solo vale con «después de la anterior»,
    /// «solo a mano» o «una sola vez».)
    pub fn con_horario(&self) -> bool {
        !self.horario.reglas.is_empty() || !self.horario.horas.is_empty()
    }

    /// F1: sin horario a propósito («solo a mano» o «una sola vez»).
    pub fn sin_horario(&self) -> bool {
        self.a_mano || self.termina.as_ref().is_some_and(|t| t.una_vez.is_some())
    }

    /// F1: el fin del plan en el agente (`None`: como siempre).
    pub fn fin(&self) -> Option<crate::agent::PlanEnd> {
        let t = self.termina.as_ref().filter(|t| t.algo());
        if !self.a_mano && t.is_none() {
            return None;
        }
        Some(crate::agent::PlanEnd {
            manual: self.a_mano,
            count: t.and_then(|t| t.veces),
            until: t.and_then(|t| t.hasta.clone()),
            once_at: t.and_then(|t| t.una_vez.clone()),
            since: t.and_then(|t| t.desde.clone()),
        })
    }

    /// F1: «solo a mano» y «una sola vez» no tienen horario ni van tras otra.
    pub fn valida_termina(&self) -> Result<(), String> {
        if let Some(t) = &self.termina {
            t.valida(&self.nombre)?;
        }
        if self.a_mano && self.termina.as_ref().is_some_and(Termina::algo) {
            return Err(format!("La copia «{}» es solo a mano: no termina.", self.nombre));
        }
        if self.sin_horario() && (self.con_horario() || self.tras.is_some()) {
            return Err(format!("La copia «{}» no tiene horario (solo a mano o una sola vez): ni horario ni «después de» otra.", self.nombre));
        }
        Ok(())
    }

    /// Plan 0.7.26: cómo empieza «después de» la anterior (siempre o solo si
    /// sale bien, y con cuánto retraso). `None` si no va después de ninguna.
    pub fn modo_cadena(&self) -> Option<crate::agent::ChainMode> {
        self.tras.as_ref()?;
        Some(crate::agent::ChainMode { always: self.inicio.as_deref() == Some("despues"), delay_min: self.retraso_min.unwrap_or(0) })
    }

    /// Plan 0.7.26: `inicio` y `retraso_min` que tienen sentido con `tras`.
    pub fn valida_inicio(&self) -> Result<(), String> {
        match (self.inicio.as_deref(), &self.tras) {
            (None, _) | (Some("cadena" | "despues"), Some(_)) | (Some("horario"), None) => {}
            (Some("horario"), Some(_)) => return Err(format!("La copia «{}» empieza con su horario: no puede ir además después de otra.", self.nombre)),
            (Some("cadena" | "despues"), None) => return Err(format!("La copia «{}» va después de otra, pero no dice de cuál.", self.nombre)),
            (Some(x), _) => return Err(format!("Cómo empieza la copia «{}» no es válido: «{x}».", self.nombre)),
        }
        match self.retraso_min {
            Some(n) if n > MAX_RETRASO_MIN => Err(format!("El retraso de la copia «{}» es de 0 a {MAX_RETRASO_MIN} minutos.", self.nombre)),
            Some(n) if n > 0 && self.tras.is_none() => Err(format!("La copia «{}» tiene retraso, pero no va después de otra.", self.nombre)),
            _ => Ok(()),
        }
    }
}

/// Tarea 7c: comprueba las cadenas de una configuración: cada `tras` es otra
/// copia de la lista (no ella misma) y no hay vueltas (A tras B tras A).
pub fn validar_cadenas(copias: &[Copia]) -> Result<(), String> {
    for k in copias {
        let Some(t) = &k.tras else { continue };
        if t == &k.id {
            return Err(format!("La copia «{}» no puede ir después de sí misma.", k.nombre));
        }
        if !copias.iter().any(|x| &x.id == t) {
            return Err(format!("La copia «{}» va después de otra que no está en la configuración.", k.nombre));
        }
        // Siguiendo la cadena hacia atrás no se puede volver a ella.
        let mut actual = t.clone();
        for _ in 0..=copias.len() {
            match copias.iter().find(|x| x.id == actual).and_then(|x| x.tras.clone()) {
                Some(sig) if sig == k.id => {
                    return Err(format!("La cadena de «{}» se cierra en un círculo: alguna copia tiene que empezar con su horario.", k.nombre))
                }
                Some(sig) => actual = sig,
                None => break,
            }
        }
    }
    Ok(())
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
    /// Tarea 8 (docs/regla-3-2-1.md, el «0» de la regla 3-2-1-1-0): prueba de
    /// restauración automática de cada repositorio, `{ "<repo>": { cada_dias } }`.
    /// Como `verificaciones`: sin el campo (una consola anterior) no se toca la que
    /// haya; con él, los repositorios que no están se quedan sin prueba automática.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pruebas_restauracion: Option<std::collections::BTreeMap<String, PruebaAuto>>,
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
    /// 0.8.1 (15ii, `admite: "equipos_moviles"`): un equipo que entra y sale (portátil): si
    /// «sale de la oficina» y con qué margen, y la batería y la conexión medida. Sin el campo
    /// (una consola anterior) se queda lo que hubiera.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub movil: Option<crate::movil::Movil>,
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

/// Tarea 8: «probar a restaurar cada `cada_dias` días»: unos archivos al azar de
/// la última versión a una carpeta temporal, comparados con lo guardado (la
/// prueba de siempre, `restore_test.rs`, con sus límites por defecto).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PruebaAuto {
    pub cada_dias: u32,
}

/// La primera prueba automática, de madrugada (después de la verificación).
const HORA_PRUEBA: u32 = 4;

impl PruebaAuto {
    pub fn valida(&self) -> Result<(), String> {
        if !(1..=31).contains(&self.cada_dias) {
            return Err("La prueba de restauración automática va de cada día a cada 31 días.".into());
        }
        Ok(())
    }

    fn schedule(&self) -> crate::agent::Schedule {
        crate::agent::Schedule::Hours { every: self.cada_dias.clamp(1, 31) * 24 }
    }

    /// La de una prueba del agente (lo que enseña el resumen); con otro horario (el de la app), nada.
    pub fn de(t: &crate::restore_test::RestoreTest) -> Option<PruebaAuto> {
        match &t.schedule {
            crate::agent::Schedule::Hours { every } => Some(PruebaAuto { cada_dias: (every / 24).max(1) }),
            _ => None,
        }
    }

    /// La prueba del agente: la primera a las 04:00 siguientes; después, cada `cada_dias` días desde la anterior.
    pub fn prueba(&self, ahora: chrono::DateTime<chrono::Local>) -> crate::restore_test::RestoreTest {
        use chrono::TimeZone;
        let every = self.cada_dias.clamp(1, 31) * 24;
        let hoy = ahora.date_naive().and_hms_opt(HORA_PRUEBA, 0, 0).and_then(|t| chrono::Local.from_local_datetime(&t).earliest());
        let primera = match hoy {
            Some(t) if t > ahora + chrono::Duration::hours(1) => t,
            Some(t) => t + chrono::Duration::days(1),
            None => ahora + chrono::Duration::hours(1),
        };
        crate::restore_test::RestoreTest {
            schedule: self.schedule(),
            files: 20,
            max_mb: 200,
            enabled_at: (primera - chrono::Duration::hours(i64::from(every))).to_rfc3339(),
        }
    }

    /// ¿Hace lo mismo que `t`? (Entonces se deja como está: no vuelve a empezar.)
    pub fn igual_que(&self, t: &crate::restore_test::RestoreTest) -> bool {
        t.schedule == self.schedule()
    }
}

/// Aplica `pruebas_restauracion` (si la configuración las trae) a los repositorios
/// con copias en el agente. Devuelve los que la tienen.
pub(crate) fn aplicar_pruebas(v: &Vinculo, cfg: &Configuracion) -> Result<usize, String> {
    let Some(mapa) = &cfg.pruebas_restauracion else { return Ok(0) };
    let ahora = chrono::Local::now();
    let mut n = 0;
    for r in crate::agent::load_config().repos.iter().filter(|r| v.repos_v2.iter().any(|x| x.id == r.id)) {
        let quiere = mapa.get(&r.id);
        n += usize::from(quiere.is_some());
        match (quiere, &r.restore_test) {
            (Some(q), Some(actual)) if q.igual_que(actual) => {}
            (None, None) => {}
            // Una prueba con otro horario (la puso la app de escritorio, no la consola): se deja.
            (None, Some(actual)) if PruebaAuto::de(actual).is_none() => {}
            (q, _) => crate::agent::set_restore_test(&r.id, q.map(|q| q.prueba(ahora)))?,
        }
    }
    Ok(n)
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
    // Tarea 7c: «después de la anterior» sin horario propio: el plan va sin horario.
    let schedule = match (&c.tras, c.con_horario()) {
        (Some(_), false) => None,
        // F1: «solo a mano» o «una sola vez»: sin horario.
        (None, false) if c.sin_horario() => None,
        _ => Some(c.horario.plan_schedule().map_err(|e| format!("Copia «{}»: {e}", c.nombre))?),
    };
    let plan: crate::plans::Plan = serde_json::from_value(json!({
        "id": c.id, "name": c.nombre, "paths": c.carpetas, "excludes": c.exclusiones,
        // 15v: las etiquetas de sus versiones (el plan las valida: hasta 10, sin espacios ni comas).
        "tags": c.etiquetas,
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

/// El plan de una copia con su cadena: `after` es la clave (`<repo>#<copia>`)
/// de la copia tras la que va, según la configuración entera.
fn plan_en(c: &Copia, copias: &[Copia]) -> Result<crate::plans::Plan, String> {
    let mut plan = plan_de(c)?;
    plan.after = c.tras.as_ref().and_then(|t| copias.iter().find(|x| &x.id == t)).map(|x| crate::plans::plan_key(&x.repo, &x.id));
    Ok(plan)
}

/// `config {config}`: aplica la configuración (sin secretos) y la sube cifrada.
pub fn aplicar_config(v: &mut Vinculo, c: &Value) -> Result<String, String> {
    aplicar_config_desde(v, c, false)
}

/// `aplicar_config`, desde la consola (`en_equipo: false`) o desde el propio
/// equipo con la clave de administración (la ventana, en modo local).
pub fn aplicar_config_desde(v: &mut Vinculo, c: &Value, en_equipo: bool) -> Result<String, String> {
    aplicar_config_marca(v, c, en_equipo.then(|| chrono::Local::now().to_rfc3339()))
}

/// `aplicar_config` con la marca «cambiado en el equipo» que se quiere dejar (`None`: ninguna).
fn aplicar_config_marca(v: &mut Vinculo, c: &Value, cambiado_en_equipo: Option<String>) -> Result<String, String> {
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
    cfg.cambiado_en_equipo = cambiado_en_equipo;
    // 15ii: sin el campo (una consola anterior), el de antes.
    match &cfg.movil {
        Some(m) => m.valida()?,
        None => cfg.movil = v.config_v1.as_ref().and_then(|c| c.get("movil")).and_then(|m| serde_json::from_value(m.clone()).ok()),
    }
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
        k.valida_termina()?;
        k.valida_exclusiones()?;
        if let Some(m) = &k.al_terminar {
            valida_al_terminar(m, k.al_terminar_servidor)?;
        }
        plan_de(k)?;
        k.valida_inicio()?;
        // 15hh (5): sus franjas de velocidad, como las de una derivada.
        if let Some(l) = &k.limite {
            limite_de(&json!({ "limite": l })).map_err(|e| format!("Copia «{}»: {e}", k.nombre))?;
        }
    }
    validar_cadenas(&cfg.copias)?;
    for (repo, va) in cfg.verificaciones.iter().flatten() {
        if !v.repos_v2.iter().any(|r| r.id == *repo && !r.solo_lectura) {
            return Err(format!("La verificación automática es de un repositorio que este equipo no tiene: «{repo}»."));
        }
        va.valida()?;
    }
    for (repo, p) in cfg.pruebas_restauracion.iter().flatten() {
        if !v.repos_v2.iter().any(|r| r.id == *repo && !r.solo_lectura) {
            return Err(format!("La prueba de restauración automática es de un repositorio que este equipo no tiene: «{repo}»."));
        }
        p.valida()?;
    }
    for r in v.repos_v2.clone() {
        let planes: Vec<crate::plans::Plan> =
            cfg.copias.iter().filter(|k| k.repo == r.id && k.activa).map(|k| plan_en(k, &cfg.copias)).collect::<Result<_, _>>()?;
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
        // Plan 0.7.26: «después de la anterior» aunque falle y el retraso, por copia.
        let modos = cfg.copias.iter().filter(|k| k.repo == r.id && k.activa).filter_map(|k| Some((k.id.clone(), k.modo_cadena()?))).collect();
        // F1: solo a mano, una sola vez o con fin.
        let fines = cfg.copias.iter().filter(|k| k.repo == r.id && k.activa).filter_map(|k| Some((k.id.clone(), k.fin()?))).collect();
        // 0.8.1 (bloque D): tamaño máximo y lo demás nuevo de cada copia.
        let extras = cfg.copias.iter().filter(|k| k.repo == r.id && k.activa).map(|k| (k.id.clone(), k.extra())).filter(|(_, x)| !x.vacio()).collect();
        crate::agent::set_schedule_full(&repo, Some(&acc), Some(crate::agent::Schedule::Plans), &modos, &fines, &extras)?;
    }
    // 15ii: la batería y la conexión medida, en el agente (agent.json).
    crate::agent::set_energia(cfg.movil.as_ref().and_then(crate::movil::Movil::energia))?;
    let verificados = aplicar_verificaciones(v, &cfg)?;
    aplicar_pruebas(v, &cfg)?;
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
    // Tarea 4b: con sus copias derivadas enteras (este documento va cifrado: el filtro con sus carpetas, para editarlo).
    doc["repositorios"] = json!(v
        .repos_v2
        .iter()
        .map(|r| {
            let mut x = json!({ "id": r.id, "nombre": r.nombre, "destino": r.destino, "retencion": r.retencion });
            if !r.derivadas.is_empty() {
                x["derivadas"] = json!(r.derivadas.iter().map(|d| json!({ "id": d["id"], "nombre": d.get("nombre"), "destino": d["destino"], "cuando": d["cuando"], "filtro": d.get("filtro"), "verificacion": d.get("verificacion"), "limite": d.get("limite") })).collect::<Vec<_>>());
            }
            x
        })
        .collect::<Vec<_>>());
    doc["destinos"] = json!(v.destinos.iter().map(|d| json!({ "id": d.id, "nombre": d.nombre, "tipo": d.tipo, "donde": d.donde })).collect::<Vec<_>>());
    doc
}

/// 15ii: el estado y el mensaje de la última vuelta de una copia, con lo de «sale de la
/// oficina»: (estado, mensaje, fuera de la oficina dentro del margen).
pub fn estado_movil(
    movil: Option<&crate::movil::Movil>,
    estado: &crate::agent::AgentState,
    key: &str,
    r: &crate::agent::RunRecord,
) -> (&'static str, String, bool) {
    if r.result == "error" {
        if let Some(m) = movil {
            let tecnico = format!("{} {}", r.message, r.detail.as_deref().unwrap_or_default());
            if let Some((e, msg)) = crate::movil::estado_fuera(m, &tecnico, crate::agent::ultima_buena(estado, key), chrono::Local::now()) {
                return (e, msg, e == "aviso");
            }
        }
    }
    (estado_de(&r.result), r.message.clone(), false)
}

/// 15ii: lo de «sale de la oficina» de la configuración.
pub fn movil_de(v: Option<&Vinculo>) -> Option<crate::movil::Movil> {
    v.and_then(|v| v.config_v1.as_ref()).and_then(|c| c.get("movil")).and_then(|m| serde_json::from_value(m.clone()).ok())
}

fn estado_de(result: &str) -> &'static str {
    match result {
        "ok" => "ok",
        "warning" => "aviso",
        // 15hh: parada o pausada a petición (con la clave): ni bien ni fallo.
        "stopped" => "parada",
        _ => "fallo",
    }
}

/// v1.28: lo que entiende este agente, en `resumen.admite`: plazos y horarias
/// en la retención (también la del almacén), `config.verificaciones` y
/// `guarda_copias { anadir, local: true }` (un repositorio en su propio almacén).
/// v1.36: `consolas_multiples` (`anadir_consola`, `quitar_consola`, `resumen.consolas`) y `escritorio` (la ventana del agente).
/// v1.40: `verificacion_horario` (la verificación automática con un horario de reglas) y
/// `retencion_almacen_horario` (la retención del almacén, también con reglas).
/// v1.49: `ordenes_en_espera` (guarda las órdenes con espera y las aplica a su hora,
/// `resumen.en_espera`, `cancelar_espera`; docs/consolas-multiples.md §5).
/// (0.8.0) `espejo_flexible`: el espejo del almacén con horario, selección,
/// retención y verificación por destino (docs/espejo.md).
pub const ADMITE: &[&str] = &[
    "retencion_plazos",
    "verificacion_auto",
    "almacen_propio",
    "consolas_multiples",
    "escritorio",
    "verificacion_horario",
    "retencion_almacen_horario",
    // v1.46: copia externa a un repositorio que ya existe, con bloqueo de objetos y «Probar».
    "externa_existente",
    "ordenes_en_espera",
    "espejo_flexible",
    // (0.8.0) "conectar_nube" también con B2, S3, SFTP, SMB y WebDAV (docs/espejo.md §3c).
    "espejo_destinos",
    // (0.8.0) varias zonas en el almacén: `guarda_copias { zona }`, `{ quitar_zona }`,
    // `{ anadir, zona }`, `{ quitar, zona }` y `guarda_copias.zonas` (docs/copias-en-cadena.md, 7b).
    "zonas_almacen",
    // (0.8.0) tarea 7c: `config.copias[].tras` («después de la anterior») e
    // `informe.cadenas[]` (aviso `cadena_parada`) (docs/copias-en-cadena.md).
    "cadenas",
    // (0.8.0) tarea 4b: `cambiar_derivada` / `quitar_derivada`, `repositorios[].derivadas`,
    // `informe.repos[].derivadas` y `subir_ahora { derivada }`.
    "derivadas",
    // (0.8.0) tarea 4c: `filtro` con `carpetas`, `desde` y `ultimos_dias` en `copiar_historial`
    // y en las copias derivadas (un agente anterior solo entiende `equipos` y `etiquetas`).
    "filtros",
    // (0.8.0) tarea 4a: `conectar_nube` también fuera de un almacén y destinos
    // `{ tipo: "nube", nube, donde }` en las copias derivadas (por rclone).
    "nube_equipo",
    // (0.8.0) tarea 4a completa: `crear_repositorio` con un destino `{ tipo: "nube", nube, donde }`
    // (copiar las carpetas directamente a una nube conectada en el equipo; el token se pone al día en cada proceso).
    "repo_en_nube",
    // (0.8.0) tarea 7d.2: destinos del espejo con `zona` (de qué zona copia) y
    // `{ tipo: "zona", carpeta: "<id>" | "principal" }` (a otra zona del almacén).
    "espejo_zonas",
    // (0.8.0) tarea 8: `config.pruebas_restauracion` y `repositorios[].prueba_auto`.
    "prueba_auto",
    // (0.8.0) el nombre, las etiquetas y la observación del equipo, iguales en todas
    // sus consolas: `nombre_equipo`, `etiquetas_equipo`, `observacion_equipo` y `resumen.datos_equipo`
    // (docs/consolas-multiples.md §6).
    "datos_equipo",
    // (0.8.0) `quitar_destino` (un destino sin uso) y `quitar_repositorio { quitar_destino }`.
    "quitar_destino",
    // (v1.66; 0.7.26, bloque 8) los datos comunes del cliente: `datos_cliente`,
    // `datos_cliente_admin`, `resumen.datos_cliente` y `datos_cliente` al subir la configuración.
    "datos_cliente",
    // (0.8.0) actualización automática: `informe.actualizacion`, `{"t":"actualizacion"}`
    // por el canal y `GET /api/agente/actualizacion` (docs/actualizaciones.md).
    "actualizaciones",
    // (0.8.0) plan 0.7.26, 1.1: `quitar_nube` anula el permiso en el proveedor
    // (Dropbox; si no puede ahora, lo reintenta 7 días), borra los restos y no deja si algo la usa;
    // `resumen.nubes_por_anular` (docs/destinos.md «Desconectar una nube»).
    "nube_revocar",
    // (0.8.0) plan 0.7.26, bloque 4: trabajos de espejo en el almacén
    // (`guarda_copias { espejo: { trabajos } }`, `guarda_copias.espejo.trabajos[]`, `espejo_freno { trabajo }`).
    "espejo_trabajos",
    // (0.8.0) plan 0.7.26, bloque 4: espejos que hace el propio equipo con los
    // repositorios de sus discos (`guarda_copias { espejo_equipo }`, `resumen.espejo_equipo`).
    "espejo_equipo",
    // (0.8.0) plan 0.7.26, bloque 3: `config.copias[].inicio` («despues»: tras la
    // anterior aunque falle) y `retraso_min` (minutos tras la anterior), también en el resumen.
    "inicio_despues",
    // (0.8.0) plan 0.8.0 (B3, 14hh): `crear_repositorio { solo_destino: true, destino }`
    // guarda el acceso a una zona de un almacén sin crear ningún repositorio (para una copia derivada).
    "solo_destino",
    // 0.8.0 plan 0.8.0, E1: `restaurar { destino: "nueva" | "zip" }`
    // (con la clave de administración), `contrasena_zip` (AES-256, también en `descargar`),
    // `motivo` y la entrada `restauracion` de la bitácora (13a, 13b, 13d).
    "restaurar_carpeta",
    // 0.8.0 14m: `desde` (`externa` o una copia derivada) en
    // `explorar`, `restaurar` y `descargar`.
    "restaurar_desde",
    // 0.8.0 13e: `informe.repos[].restauracion_real` (una restauración
    // real correcta cuenta como prueba de restauración).
    "restauracion_real",
    // (0.8.0) plan 0.8.0, B1: carpetas legibles en el almacén (`guarda_copias { anadir, nombre }`,
    // `{ mover }`, `guarda_copias.renombres`), `cambiar_destino { carpetas }` y `nombre_repositorio`.
    crate::nombres::ADMITE,
    // (0.8.0) 14bbb: la orden `comprobar_contrasena` y `repositorios[].contrasena_comprobada`.
    "comprobar_contrasena",
    // 0.8.0 14a: aprobar una orden en espera en el propio equipo
    // (ventana o `resguardo-agente ordenes aprobar`), `en_espera[].aprobable`,
    // `resumen.aprobar_en_equipo`, `aprobar_en_equipo` en `hola`/`tomar`/`{"t":"ajustes"}`,
    // `aprobada_en_equipo` en el historial y el aviso `orden_aprobada_en_equipo`.
    "aprobar_en_equipo",
    // (0.8.0) plan 0.8.0, F1 (14pp): `config.copias[].a_mano` y `termina`
    // (`veces`, `hasta`, `una_vez`, `desde`); en el resumen, `hechas` y `completada`.
    "termina",
    // (0.8.0) plan 0.8.0, F2 (14ww-2): `comentario_version` y `resumen.comentarios`.
    "comentarios_version",
    // (0.8.0) plan 0.8.0, F2 (14cc): `conservar_version` (en el equipo y en el almacén),
    // `resumen.conservadas`, `guarda_copias.conservadas`; la retención nunca las quita.
    "conservar_version",
    // (0.8.0) plan 0.8.0, F3 (14u): `retencion.por_dia` (en el equipo y en el almacén).
    "retencion_por_dia",
    // (0.8.0) plan 0.8.0, F4 (14t): límite de velocidad según la hora en los espejos
    // (`trabajos[].limite_franjas`, por rclone) y en las copias derivadas (`cambiar_derivada { limite }`).
    "limite_franjas",
    // 0.8.0 plan 0.8.0, D1 y D2 (14ll, 14ss): los datos comunes
    // `sede:`, `sede.equipo:` y `sede.destino:` (con la clave), `etiqueta.categoria:` y
    // `orden.equipos:` en `datos_cliente` (docs/sedes.md).
    resguardo_protocolo::datos_cliente::ADMITE_SEDES,
    // 0.8.0 14s: `cuando.primera_desde` en los trabajos de espejo (la
    // primera subida, no antes de esa hora). 14i, 14c y 14p (campos opcionales, sin token): `subidas`,
    // `ocupa`, `bien`, `nunca_bien` y `en_cola` en cada trabajo y el progreso `espejo`.
    "espejo_primera",
    // 0.8.0 14ccc: `verificar` (opcional) en el gancho `sqlserver`
    // (`RESTORE VERIFYONLY … WITH CHECKSUM` de cada volcado, por defecto sí) y `verificado` en sus resultados.
    "verificar_volcado",
    // 0.8.1 (15u, 15f): «todas las bases» en el gancho `sqlserver` (`todas`, `excluir`, `sistema`),
    // la orden `bases_sqlserver` («Ver las bases de la instancia») y el aviso (no fallo) cuando
    // solo falta el permiso para verificar un volcado.
    "sqlserver_todas",
    // 0.8.1 (C3, 15c): la orden `tipo_equipo` (el tipo de equipo corregido a mano, en
    // `datos_equipo.tipo`). (C4, 15d; C1, 15n; C2, 15cc; campos opcionales, sin token):
    // `datos_basicos.almacenamiento` y `discos[].volumen`, `destinos[].carpeta` de un destino
    // local y `derivada` en el progreso de una copia derivada en marcha.
    "tipo_equipo",
    // 0.8.1 (15hh): la orden `parar { clave, pausa? }`, `parar` y `pausable` en el progreso,
    // `copias[].pausa` en el resumen y `config.copias[].limite` (franjas de velocidad).
    crate::parar::ADMITE,
    // 0.8.1 (15q): `quitar_repositorio { repo, vacio: true }` solo con la clave de administración
    // (el equipo comprueba que no tiene versiones).
    "quitar_vacio",
    // 0.8.1 (15g): `clave_almacen { derivada }`, la clave del almacén en el repositorio de una
    // copia derivada (para su «Retención en el almacén»), y `repositorios[].derivadas[].ruta`.
    "retencion_derivada",
    // 0.8.1 (15ll): `config.copias[].permitir_suspension` y `evitando_suspension` en el progreso;
    // `datos_basicos.red_ahorro` (las tarjetas de red que el sistema puede apagar).
    crate::suspension::ADMITE,
    // 0.8.0 plan 0.8.0, E4 (14j): `probar_restauracion { espejo }`,
    // `{ programar_espejo }` y `{ quitar_espejo }` (prueba de restauración desde un espejo, la hace
    // el equipo dueño) e `informe.repos[].pruebas_espejo[]`.
    crate::prueba_espejo::ADMITE,
    // 0.8.1 (B2, 15y): el dato común `exclusiones:<id>` (grupos de «No copiar» del cliente)
    // en `datos_cliente`. Solo se guarda y se reparte: las copias llevan sus propias reglas.
    resguardo_protocolo::datos_cliente::ADMITE_EXCLUSIONES,
    // 0.8.1 (B5, 15pp): guarda observaciones y comentarios comunes del cliente.
    resguardo_protocolo::datos_cliente::ADMITE_NOTAS,
    // 0.8.1 (15t): la orden `nombre_copia` (el nombre de una copia o de una copia derivada,
    // con la clave de administración y sin espera) y `derivadas[].nombre`.
    "nombre_copia",
    // 0.8.1 (15v): `config.copias[].etiquetas` (las etiquetas de sus versiones) y en el resumen.
    "etiquetas_copia",
    // 0.8.1 (15gg): `guarda_copias.sueltos` (repositorios sueltos del almacén), `guarda_copias
    // { dar_suelto }` (moverlo a la carpeta de un equipo, sin copiar), `{ sueltos_en }` y, en
    // `adoptar_repositorio` sellado, `ultima_carpetas` y `ultimo_equipo`.
    "repos_sueltos",
    // 0.8.1 (15kk): `config.copias[].al_terminar` y `copiar_ahora { al_terminar }` (con la
    // clave de administración): apagar o hibernar al terminar bien, con 2 minutos de aviso.
    "apagar_al_terminar",
    // 0.8.1 (15ii): `config.movil` (sale de la oficina, margen, batería y conexión medida),
    // `config.copias[].al_reconectar` y, en un equipo que sale de la oficina, las copias sin
    // conexión dentro del margen como «aviso» con `fuera: true` (no «fallo»).
    "equipos_moviles",
    // 0.8.1 (15y): `config.copias[].grupos_exclusiones`, `mayores_mb` y `excepto_tamano`
    // («no copiar más de N MB» con excepciones; lo que quedó fuera, en la versión) y la orden
    // `vista_exclusiones` (cuánto deja fuera cada grupo, sin copiar nada).
    "grupos_en_copias",
    // 0.8.1 (E1, 15w): `excluir` (rutas exactas quitadas de lo elegido) en `restaurar` y
    // `descargar`, y la operación `medir` de la sesión `explorar` (lo que de verdad se restaura).
    crate::restaurar_v2::ADMITE_EXCLUIR,
    // 0.8.1 (E2, 15ee; H1, 15mm): los ajustes comunes `ajuste:restauraciones` (sin clave) y
    // `ajuste:recuperar` (con la clave) en `datos_cliente`.
    resguardo_protocolo::datos_cliente::ADMITE_AJUSTES,
    // 0.8.1 (E4, 14m): `desde: "espejo:<id>"` (un espejo que hace el propio equipo) en
    // `explorar`, `restaurar` y `descargar`.
    crate::restaurar_v2::ADMITE_DESDE_ESPEJO,
    // 0.8.1 (H1, 15mm): la orden `recuperar_en_equipo` (con la clave), `resumen.recuperar` y
    // lo que la persona del equipo recupera por la ventana o la terminal (con `cuenta` en la bitácora).
    crate::recuperar::ADMITE,
    // 0.8.1 (H4, 15ss): guarda las pruebas funcionales y su política como datos comunes.
    resguardo_protocolo::datos_cliente::ADMITE_PRUEBAS,
    // 0.8.1 (H2, 15qq): guarda el inventario de lo protegido (responsable, criticidad…).
    resguardo_protocolo::datos_cliente::ADMITE_INVENTARIO,
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
    let movil = movil_de(Some(v));
    let now = chrono::Local::now();
    let pausa = config.repos.iter().filter(|r| v.repos_v2.iter().any(|x| x.id == r.id)).filter_map(|r| r.active_pause(now)).map(|p| p.until.clone()).next();
    let proximas = proximas(&copias, now);
    let cache_repos = Value::Object(v.repos_v2.iter().map(|r| (r.id.clone(), crate::informe_v2::resumen_repo(&r.id))).collect());
    let tareas = crate::tasks::load_state();
    let comprobadas = crate::comprobar_contrasena::leer();
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
    // Tarea 8: la prueba de restauración automática y cuándo toca.
    let prueba_auto = |id: &str| {
        let r = config.repos.iter().find(|r| r.id == id)?;
        let p = PruebaAuto::de(r.restore_test.as_ref()?)?;
        Some(json!({ "cada_dias": p.cada_dias, "proxima": crate::tasks::next_restore_test(r, &tareas).map(|t| t.to_rfc3339()) }))
    };
    let mut r = json!({
        // v1.28: lo que este agente sabe hacer de lo nuevo (la consola no ofrece lo que no).
        "admite": ADMITE,
        // v1.36: la ventana y los avisos del escritorio (no es secreto) y si se cambiaron en el equipo.
        "escritorio": v.config_v1.as_ref().map(|c| crate::escritorio::Escritorio::de_config(c).a_json()),
        "escritorio_cambiado_en_equipo": v.config_v1.as_ref().and_then(|c| c.get("cambiado_en_equipo")).filter(|x| x.is_string()),
        // 15ii: si sale de la oficina y su margen (y la batería y la conexión medida): no es secreto.
        "movil": movil.as_ref(),
        "copias": copias.iter().map(|k| {
            let run = estado.runs.get(&crate::plans::plan_key(&k.repo, &k.id));
            json!({
                "id": k.id, "nombre": k.nombre, "repo": k.repo, "horario": k.horario, "carpetas": k.carpetas.len(), "activa": k.activa, "solo_si_cambios": k.solo_si_cambios,
                // 15v: las etiquetas de sus versiones (no son secretas: van en cada versión).
                "etiquetas": (!k.etiquetas.is_empty()).then_some(&k.etiquetas),
                // 15y: qué grupos de exclusiones usa (con sus reglas: son datos comunes del cliente,
                // sin rutas) y el tamaño máximo. Las reglas sueltas no salen (pueden llevar rutas).
                "grupos_exclusiones": (!k.grupos_exclusiones.is_empty()).then_some(&k.grupos_exclusiones),
                "mayores_mb": k.mayores_mb,
                // Tarea 7c: «después de la anterior» (el id de la otra copia).
                "tras": k.tras,
                // Plan 0.7.26: cómo empieza tras la anterior y con cuánto retraso.
                "inicio": k.inicio, "retraso_min": k.retraso_min,
                // F1 (14pp): solo a mano, cuándo termina, cuántas lleva y si ya terminó.
                "a_mano": k.a_mano.then_some(true),
                // 15hh (4): en pausa con «Pausar»: hasta cuándo (sin hora: a mano) y quién la pausó.
                "pausa": estado.pausas.get(&crate::plans::plan_key(&k.repo, &k.id)).map(|p| json!({ "hasta": p.hasta, "desde": p.desde, "quien": p.quien })),
                // 15hh (5) y 15ll: su límite de velocidad y si deja suspender el equipo.
                "limite": k.limite,
                "permitir_suspension": k.permitir_suspension.then_some(true),
                "termina": k.termina.as_ref().filter(|t| t.algo()),
                "hechas": fin_de(&config, &estado, k).map(|f| f.0),
                "completada": fin_de(&config, &estado, k).and_then(|f| f.1),
                "ultima": run.map(|r| {
                    let (e, m, fuera) = estado_movil(movil.as_ref(), &estado, &crate::plans::plan_key(&k.repo, &k.id), r);
                    json!({ "cuando": r.finished, "estado": e, "mensaje": crate::web::public_message(&m), "bytes": r.data_added,
                        // 15jj: quedó «en curso» y no terminó (`estado` sigue en «fallo» para las consolas anteriores).
                        "interrumpida": r.interrumpida.then_some(true),
                        // 15ii: sin conexión, en un equipo que sale de la oficina y dentro de su margen.
                        "fuera": fuera.then_some(true) })
                }),
                // 15ii: además, al volver a conectarse.
                "al_reconectar": k.al_reconectar.then_some(true),
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
            "prueba_auto": prueba_auto(&r.id),
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
            // Tarea 4b: las demás copias derivadas (sin rutas ni secretos).
            "derivadas": r.derivadas.iter().map(|e| derivada_resumen(v, &r.id, e, config.repos.iter().find(|x| x.id == r.id))).collect::<Vec<_>>(),
            // 14bbb: cuándo se comprobó por última vez su contraseña (y cómo fue).
            "contrasena_comprobada": crate::comprobar_contrasena::resumen(&r.id, &comprobadas),
        })).collect::<Vec<_>>(),
        // Sin la carpeta de un destino local (es una ruta del equipo).
        // v1.30: `equipo_almacen`, el equipo que guarda copias del destino (si se sabe).
        // v1.41: de un destino local, solo qué disco es (`unidad` «D:», `extraible`, `red`): la
        // consola avisa si las copias se quedan en el mismo equipo que protegen.
        "destinos": v.destinos.iter().map(|d| {
            let mut x = json!({ "id": d.id, "nombre": d.nombre, "tipo": d.tipo, "donde": (d.tipo != "local").then(|| d.donde.clone()), "equipo_almacen": d.equipo_almacen });
            // Tarea 4a: la nube conectada en el equipo a la que va (su nombre).
            if let Some(n) = d.nube.as_ref().filter(|_| d.tipo == "nube") {
                x["nube"] = json!(n);
            }
            if d.tipo == "local" {
                let disco = crate::espacio::disco_de(&d.donde).json();
                for k in ["unidad", "extraible", "red"] {
                    x[k] = disco[k].clone();
                }
                // Plan 0.8.1 (C1, 15n): su carpeta, para «Dónde está» (como la del almacén), sin
                // el nombre de la cuenta si está dentro de un perfil de usuario.
                // Una carpeta de la red no: su ruta dice el nombre del servidor.
                if let Some(c) = crate::espacio::ruta_publica(&d.donde).filter(|_| disco["red"] != true) {
                    x["carpeta"] = json!(c);
                }
                // Tarea 8e: el sistema de archivos (solo su nombre; nunca resta en la regla 3-2-1).
                x["sistema_archivos"] = crate::espacio::json_fs(&d.donde);
                // 0.7.26: cuándo se vio conectado su disco (para la marca «Aislado»): solo
                // un resumen del volumen y la hora, nunca la ruta. Sin datos, no va.
                if disco["red"] != true {
                    let a = crate::volumenes::resumen(&format!("destino:{}", d.id), &d.donde);
                    if !a.is_null() {
                        x["aislado"] = a;
                    }
                }
            }
            x
        }).collect::<Vec<_>>(),
        "pausado_hasta": pausa.map(|u| json!(u.unwrap_or_else(|| "indefinido".into()))),
        "guarda_copias": resumen_guarda_copias(),
        // Plan 0.7.26 (bloque 4): los espejos que hace el propio equipo (`null` si no tiene).
        "espejo_equipo": crate::espejo_trabajos::resumen_equipo(),
        // Tarea 4a: las nubes conectadas en este equipo (solo nombre y tipo), también si no guarda copias.
        "nubes": crate::nube::lista(),
        // Plan 0.7.26 (1.1): nubes desconectadas cuyo permiso aún no se pudo anular (sin tokens).
        "nubes_por_anular": crate::nube_anular::resumen(),
        // Tarea 8e: si corre en una máquina virtual o un contenedor (solo un dato para la consola).
        "entorno": crate::espacio::entorno(),
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
        // v1.49: las órdenes con espera que tiene el equipo (de cualquiera de sus consolas).
        "en_espera": crate::espera_v2::resumen(v),
        // 14a: si el cliente de esta consola deja aprobar sus órdenes en espera en el propio equipo
        // (lo que el equipo recibió de ella; por defecto, sí).
        "aprobar_en_equipo": crate::espera_v2::aprobable(v, &v.id_enlace()),
    });
    // v1.56: el nombre, las etiquetas y la observación que tiene el equipo (solo lo puesto con
    // sus órdenes; cada consola enseña esto en vez de lo suyo). Sin nada puesto, no va.
    let datos = crate::datos_equipo::resumen(v);
    if !datos.is_null() {
        r["datos_equipo"] = datos;
    }
    // 0.7.26 (bloque 8): cuántos datos comunes del cliente tiene y su huella (el documento
    // entero va con la configuración: el resumen no pasa de 64 KiB).
    let comunes = crate::datos_cliente::resumen(v);
    if !comunes.is_null() {
        r["datos_cliente"] = comunes;
    }
    // Plan 0.8.0, F2: comentarios en las versiones y versiones conservadas para siempre
    // (iguales en todas sus consolas; sin nada, no van).
    let comentarios = crate::versiones_v2::resumen_comentarios(v);
    if !comentarios.is_null() {
        r["comentarios"] = comentarios;
    }
    let conservadas = crate::versiones_v2::resumen_conservadas(v);
    if !conservadas.is_null() {
        r["conservadas"] = conservadas;
    }
    // 0.8.1 (H1, 15mm): «Recuperar en el equipo»: lo del cliente que llegó y lo de cada copia.
    r["recuperar"] = crate::recuperar::resumen(v);
    // 14uu (plan 0.8.0, D3): sistema exacto, procesador, memoria, discos, modelo y último
    // arranque (leídos al arrancar y una vez al día). Las rutas solo marcan qué discos
    // guardan algo de las copias (`en_uso`): no salen del equipo.
    let mut rutas: Vec<String> = copias.iter().flat_map(|k| k.carpetas.iter().cloned()).collect();
    rutas.extend(v.destinos.iter().filter(|d| d.tipo == "local").map(|d| d.donde.clone()));
    if let Some(c) = r["guarda_copias"]["carpeta"].as_str() {
        rutas.push(c.to_string());
    }
    r["datos_basicos"] = crate::datos_basicos::resumen(&rutas);
    r
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
    // 0.7.26 (bloque 8): los datos comunes del cliente (un servidor anterior los ignora).
    if let Some(d) = crate::datos_cliente::documento(v) {
        cuerpo["datos_cliente"] = d;
    }
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
    let movil = movil_de(v);
    let copias: Vec<Copia> =
        v.and_then(|v| v.config_v1.as_ref()).and_then(|c| serde_json::from_value::<Configuracion>(c.clone()).ok()).map(|c| c.copias).unwrap_or_default();
    let filas: Vec<Value> = estado
        .runs
        .iter()
        .map(|(clave, r)| {
            let (repo, copia) = clave.split_once('#').unwrap_or((clave.as_str(), ""));
            let nombre = copias.iter().find(|k| k.repo == repo && k.id == copia).map(|k| k.nombre.clone());
            // 15ii: en un equipo que sale de la oficina, sin conexión dentro del margen no es un fallo.
            let (e, m, fuera) = estado_movil(movil.as_ref(), &estado, clave, r);
            json!({
                "id": copia, "repo": repo, "nombre": nombre, "estado": e, "cuando": r.finished, "mensaje": crate::web::public_message(&m), "fuera": fuera.then_some(true),
                // 15f: con `verificado`, para que la tarjeta «Antes de copiar» diga «volcadas y verificadas».
                "ganchos": r.ganchos.iter().map(|g| json!({ "tipo": g.tipo, "estado": g.estado, "mensaje": crate::web::public_message(&g.mensaje), "verificado": g.verificado })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let mut inf = json!({ "version": crate::version_programa(), "servicio": "en_marcha", "so": crate::web::os_label(), "copias": filas, "proximas": proximas(&copias, chrono::Local::now()) });
    // Tarea 7c: cómo quedó cada copia «después de la anterior» (de ahí el aviso `cadena_parada`).
    let cadenas = cadenas_informe(&estado, &copias);
    if !cadenas.is_empty() {
        inf["cadenas"] = json!(cadenas);
    }
    // El último número de orden aceptado: el servidor no vuelve por debajo (p. ej. tras restaurar la copia de la consola).
    if let Some(v) = v.filter(|v| v.ultimo_seq > 0) {
        inf["ultimo_seq"] = json!(v.ultimo_seq);
    }
    // (0.8.0) la actualización automática: versión disponible, estado y motivo.
    inf["actualizacion"] = crate::actualizacion::informe();
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

/// Tarea 7c: `informe.cadenas[]`: `{ id, repo, nombre, estado: "parada" | "ok", cuando, anterior, mensaje }`
/// de las copias de la configuración que van «después de la anterior».
pub fn cadenas_informe(estado: &crate::agent::AgentState, copias: &[Copia]) -> Vec<Value> {
    copias
        .iter()
        .filter(|k| k.tras.is_some())
        .filter_map(|k| {
            let c = estado.chains.get(&crate::plans::plan_key(&k.repo, &k.id))?;
            let anterior = c.previous.split_once('#').map(|(_, id)| id.to_string()).unwrap_or_default();
            Some(json!({
                "id": k.id, "repo": k.repo, "nombre": k.nombre, "estado": c.state, "cuando": c.at, "anterior": anterior,
                "anterior_nombre": copias.iter().find(|x| x.id == anterior).map(|x| x.nombre.clone()),
                "mensaje": c.message,
            }))
        })
        .collect()
}

/// v1.12: la próxima vez que toca cada copia, `{id: RFC 3339 | null}`. Null si
/// está desactivada, sin horario o su repositorio está en pausa sin fecha; con
/// una pausa con fecha, la primera hora después de que termine.
/// F1: ¿hay alguna copia «una sola vez» cuya hora ya llegó y que aún no empezó? (Entonces
/// se hace una vuelta enseguida, como con «Copiar ahora».)
pub fn una_vez_pendiente() -> bool {
    let config = crate::agent::load_config();
    let estado = crate::agent::load_state();
    let ahora = chrono::Local::now();
    config.repos.iter().flat_map(|r| r.plans.iter().map(move |p| (r, p))).any(|(r, p)| {
        let clave = crate::plans::plan_key(&r.id, &p.id);
        p.end.as_ref().and_then(crate::agent::PlanEnd::once_local).is_some_and(|at| at <= ahora)
            && !p.end.as_ref().is_some_and(|e| e.once_started(estado.runs.get(&clave)))
            && !crate::agent::plan_finished(p, &clave, &estado, ahora)
    })
}

/// F1: copias hechas y cuándo terminó una copia con fin (según el agente).
fn fin_de(config: &crate::agent::AgentConfig, estado: &crate::agent::AgentState, k: &Copia) -> Option<(u32, Option<String>)> {
    k.termina.as_ref().filter(|t| t.algo())?;
    let plan = config.repos.iter().find(|r| r.id == k.repo)?.plans.iter().find(|p| p.id == k.id)?;
    Some(crate::agent::end_status(plan, &crate::plans::plan_key(&k.repo, &k.id), estado))
}

fn proximas(copias: &[Copia], ahora: chrono::DateTime<chrono::Local>) -> Value {
    let config = crate::agent::load_config();
    let estado = crate::agent::load_state();
    let mut out = serde_json::Map::new();
    for k in copias {
        // F1: una copia completada no vuelve a tocar; «una sola vez», a su hora si aún no se hizo.
        let plan = config.repos.iter().find(|r| r.id == k.repo).and_then(|r| r.plans.iter().find(|p| p.id == k.id));
        let clave = crate::plans::plan_key(&k.repo, &k.id);
        if plan.is_some_and(|p| crate::agent::plan_finished(p, &clave, &estado, ahora)) {
            out.insert(k.id.clone(), Value::Null);
            continue;
        }
        if let Some(at) = plan.and_then(|p| p.end.as_ref()).and_then(crate::agent::PlanEnd::once_local) {
            let empezada = plan.and_then(|p| p.end.as_ref()).is_some_and(|e| e.once_started(estado.runs.get(&clave)));
            out.insert(k.id.clone(), if k.activa && !empezada { json!(at.max(ahora).to_rfc3339()) } else { Value::Null });
            continue;
        }
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
/// 15kk: `apagar` o `hibernar`, y en un almacén solo con confirmación expresa (sus equipos se
/// quedarían sin destino).
pub fn valida_al_terminar(modo: &str, confirmado_servidor: bool) -> Result<(), String> {
    if !crate::apagar::modo_valido(modo) {
        return Err("Al terminar: «apagar» o «hibernar».".into());
    }
    if crate::server::load().enabled && !confirmado_servidor {
        return Err("Este equipo guarda copias de otros equipos: apagarlo los dejaría sin destino. Hay que confirmarlo expresamente.".into());
    }
    Ok(())
}

pub fn copiar_ahora(v: &Vinculo, c: &Value) -> Result<String, String> {
    copiar_ahora_por(v, c, None)
}

/// `copiar_ahora { copia, al_terminar?, confirmar_servidor? }` (15kk: con `al_terminar`, la
/// orden lleva la clave de administración) y quién la pide.
pub fn copiar_ahora_por(v: &Vinculo, c: &Value, por: Option<&str>) -> Result<String, String> {
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
    // 15kk: apagar o hibernar al terminar (solo esta vez).
    let al_terminar = c["al_terminar"].as_str().filter(|m| !m.is_empty());
    if let Some(m) = al_terminar {
        valida_al_terminar(m, c["confirmar_servidor"] == true)?;
    }
    crate::agent::request_backup(&repo, &copia)?;
    if let Some(m) = al_terminar {
        crate::apagar::pedir(crate::apagar::Pedido {
            modo: m.to_string(),
            clave: crate::plans::plan_key(&repo, &copia),
            por: por.map(|p| p.chars().take(80).collect()),
            pedido: chrono::Local::now().to_rfc3339(),
        })?;
        return Ok(format!(
            "Copia pedida: empieza en unos segundos. Al terminar bien, el equipo {} (2 minutos de aviso a quien lo use).",
            crate::apagar::verbo(m)
        ));
    }
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
    // Plan 0.8.0: con versiones conservadas para siempre (F2) o «N por día» (F3), decide el
    // planificador propio (las reglas de `restic forget`, probadas contra él) y se quitan
    // por id; si no, `restic forget` con la regla, como siempre.
    let conservadas = crate::versiones_v2::conservadas_de(v, repo);
    let (out, r) = if !conservadas.is_empty() || regla.por_dia.is_some() {
        retencion_por_ids(&acc, antes.as_deref(), regla, &conservadas)
    } else {
        let mut args: Vec<String> = vec!["forget".into(), "--prune".into(), "--retry-lock".into(), "30m".into()];
        args.extend(politica.args());
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = resguardo_motor::restic::run_raw(&acc, &refs, std::time::Duration::from_secs(6 * 3600));
        let r = match &out {
            Ok(o) if o.code == Some(0) => Ok("Retención aplicada.".to_string()),
            Ok(o) => Err(resguardo_motor::restic::exit_error(o.code, &o.stderr)),
            Err(e) => Err(e.clone()),
        };
        (out, r)
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

/// Plan 0.8.0 (F2, F3): la retención del equipo decidida por el planificador propio
/// (`retencion_almacen::planear`, con las horas de las versiones como de fiar: son
/// las del propio equipo), sin las conservadas para siempre, y quitada por ids.
/// Devuelve la salida del `prune` (para lo liberado) y el mensaje.
fn retencion_por_ids(
    acc: &resguardo_motor::restic::Access,
    antes: Option<&[resguardo_motor::restic::Snapshot]>,
    regla: &Retencion,
    conservadas: &[String],
) -> (Result<resguardo_motor::restic::RawOutput, String>, Result<String, String>) {
    let Some(antes) = antes else {
        let e = "No se pudieron leer las versiones del repositorio: no se ha quitado nada.".to_string();
        return (Err(e.clone()), Err(e));
    };
    let versiones: Vec<crate::retencion_almacen::Version> = antes.iter().map(crate::retencion_almacen::Version::de_confianza).collect();
    let plan = crate::retencion_almacen::planear(&versiones, regla);
    let (quitar, salvadas) = crate::versiones_v2::sin_conservadas(plan.quitar, conservadas);
    let limite = std::time::Duration::from_secs(6 * 3600);
    for tanda in quitar.chunks(100) {
        let mut args: Vec<&str> = vec!["forget", "--retry-lock", "30m", "--"];
        args.extend(tanda.iter().map(String::as_str));
        match resguardo_motor::restic::run_raw(acc, &args, limite) {
            Ok(o) if o.code == Some(0) => {}
            Ok(o) => {
                let e = resguardo_motor::restic::exit_error(o.code, &o.stderr);
                return (Ok(o), Err(e));
            }
            Err(e) => return (Err(e.clone()), Err(e)),
        }
    }
    let out = if quitar.is_empty() {
        Ok(resguardo_motor::restic::RawOutput { code: Some(0), stdout: Vec::new(), stderr: String::new() })
    } else {
        resguardo_motor::restic::run_raw(acc, &["prune", "--retry-lock", "30m"], limite)
    };
    let mut m = "Retención aplicada.".to_string();
    match salvadas {
        0 => {}
        1 => m.push_str(" 1 versión conservada para siempre no se toca."),
        n => m.push_str(&format!(" {n} versiones conservadas para siempre no se tocan.")),
    }
    let r = match &out {
        Ok(o) if o.code == Some(0) => Ok(m),
        Ok(o) => Err(resguardo_motor::restic::exit_error(o.code, &o.stderr)),
        Err(e) => Err(e.clone()),
    };
    (out, r)
}

/// `dejar_de_copiar {repo}` y `quitar_repositorio {repo}` (este, además, lo olvida con su contraseña).
/// Lo que usa un destino del equipo: sus repositorios, las copias externas y las
/// derivadas que van a él (los nombres de los repositorios, para decirlo).
pub fn usos_destino(v: &Vinculo, id: &str) -> Vec<String> {
    let mut usos = Vec::new();
    for r in &v.repos_v2 {
        if r.destino == id {
            usos.push(format!("el repositorio «{}»", r.nombre));
        }
        if r.externa.as_ref().is_some_and(|e| e["destino"] == id) {
            usos.push(format!("la copia derivada de cada día de «{}»", r.nombre));
        }
        if r.derivadas.iter().any(|e| e["destino"] == id) {
            usos.push(format!("una copia derivada de «{}»", r.nombre));
        }
    }
    usos
}

/// ¿Cuántos repositorios de restic hay en una carpeta (ella misma o una carpeta suya,
/// o dos niveles más abajo, como `<usuario>/<repo>`)? Solo mira: nunca toca nada.
pub fn repositorios_en_carpeta(carpeta: &std::path::Path) -> usize {
    fn es_repo(p: &std::path::Path) -> bool {
        p.join("config").is_file() && p.join("data").is_dir() && p.join("keys").is_dir()
    }
    if es_repo(carpeta) {
        return 1;
    }
    let hijos = |p: &std::path::Path| -> Vec<std::path::PathBuf> {
        std::fs::read_dir(p).map(|l| l.flatten().map(|e| e.path()).filter(|p| p.is_dir()).take(2000).collect()).unwrap_or_default()
    };
    hijos(carpeta).iter().map(|h| if es_repo(h) { 1 } else { hijos(h).iter().filter(|n| es_repo(n)).count() }).sum()
}

/// `quitar_destino { destino }` (v1.56; desde la v1.59, con la clave de administración y solo
/// administradores): olvida un destino
/// que ya no usa nada (ni repositorios, ni copia externa, ni derivadas), con sus credenciales.
/// **Nunca borra nada de lo que hay en él**; si es una carpeta del equipo y aún tiene copias
/// guardadas, lo dice.
pub fn quitar_destino(v: &mut Vinculo, c: &Value) -> Result<String, String> {
    let id = texto(c, "destino");
    let pos = v.destinos.iter().position(|d| d.id == id).ok_or_else(|| "Ese destino ya no está en este equipo.".to_string())?;
    let usos = usos_destino(v, &id);
    if !usos.is_empty() {
        return Err(format!("No se puede quitar: lo usa {}. Quita eso antes.", usos.join(", ")));
    }
    let d = v.destinos.remove(pos);
    Ok(texto_destino_quitado(&d))
}

/// Lo que se dice al quitar un destino (también tras `quitar_repositorio { quitar_destino }`).
fn texto_destino_quitado(d: &Destino) -> String {
    let nombre = crate::web::public_message(&d.nombre);
    if d.tipo == "local" {
        return match repositorios_en_carpeta(std::path::Path::new(d.donde.trim())) {
            0 => format!("Destino «{nombre}» quitado del equipo. No se ha borrado nada de su carpeta."),
            n => format!(
                "Destino «{nombre}» quitado del equipo. Su carpeta aún tiene copias guardadas ({n} {}): no se ha borrado nada; si ya no las quieres, bórralas a mano.",
                if n == 1 { "repositorio" } else { "repositorios" }
            ),
        };
    }
    format!("Destino «{nombre}» quitado del equipo, con sus credenciales. Lo guardado allí se queda.")
}

/// 15q: ¿de verdad no tiene ninguna versión? (Para quitarlo solo con la clave de
/// administración.) Si en el destino no hay repositorio, tampoco hay nada que perder.
pub fn comprobar_vacio(v: &Vinculo, repo: &str) -> Result<(), String> {
    use resguardo_motor::restic;
    let acc = acceso(v, repo)?;
    let out = restic::run_raw(&acc, &["snapshots", "--json", "--no-lock"], restic::QUERY_TIMEOUT)
        .map_err(|e| format!("No se pudo comprobar que esté vacío ({e}). Quítalo con su contraseña."))?;
    match out.code {
        Some(0) => match restic::parse_snapshots(&out.stdout)?.len() {
            0 => Ok(()),
            1 => Err("Tiene 1 versión: para quitarlo hace falta también su contraseña.".into()),
            n => Err(format!("Tiene {n} versiones: para quitarlo hace falta también su contraseña.")),
        },
        // Sin repositorio en el destino (nunca se llegó a crear o ya no está).
        Some(10) => Ok(()),
        code => Err(format!("No se pudo comprobar que esté vacío ({}). Quítalo con su contraseña.", restic::exit_error(code, &out.stderr))),
    }
}

pub fn dejar_de_copiar(v: &mut Vinculo, repo: &str, olvidar: bool, quitar_destino_vacio: bool) -> Result<String, String> {
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
    let destino = v.repos_v2.iter().find(|r| r.id == repo).map(|r| r.destino.clone()).unwrap_or_default();
    if olvidar {
        v.repos_v2.retain(|r| r.id != repo);
        // Plan 0.8.0, F2: sus comentarios y marcas se van con él.
        crate::versiones_v2::olvidar_repo(v, repo);
        if let Some(cfg) = v.config_v1.as_mut() {
            if let Some(copias) = cfg["copias"].as_array_mut() {
                copias.retain(|k| k["repo"] != repo);
            }
        }
    }
    // v1.56: `quitar_repositorio { quitar_destino: true }`: si su destino se queda sin uso,
    // también se olvida (nunca se borra nada de lo que hay en él). Un agente anterior lo ignora.
    let mut m: String = if olvidar {
        "Repositorio quitado del equipo (lo guardado sigue en su destino)."
    } else {
        "Ya no se copia en ese repositorio (lo guardado sigue ahí)."
    }
    .into();
    if olvidar && quitar_destino_vacio && usos_destino(v, &destino).is_empty() {
        if let Some(pos) = v.destinos.iter().position(|d| d.id == destino) {
            let d = v.destinos.remove(pos);
            m = format!("{m} {}", texto_destino_quitado(&d));
        }
    }
    let _ = subir_config(v);
    Ok(m)
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
    // B1 (docs/nombres-legibles.md): la carpeta nueva de algunos de sus repositorios
    // (tras «Ordenar nombres de carpeta» en el almacén). El id no cambia.
    if let Some(cs) = c.get("carpetas").filter(|x| !x.is_null()) {
        let cs = cs.as_object().ok_or("«carpetas» tiene que ser un objeto (repositorio → carpeta).")?;
        for (repo, carpeta) in cs {
            let carpeta = carpeta.as_str().filter(|x| crate::nombres::carpeta_valida(x)).ok_or_else(|| format!("Carpeta no válida para «{repo}»."))?;
            let r = prueba
                .repos_v2
                .iter_mut()
                .find(|r| r.id == *repo && r.destino == id)
                .ok_or_else(|| format!("«{repo}» no es un repositorio de ese destino."))?;
            r.ubicacion_origen = (carpeta != r.id).then(|| carpeta.to_string());
        }
    }
    let repos: Vec<String> = prueba.repos_v2.iter().filter(|r| r.destino == id).map(|r| r.id.clone()).collect();
    for r in &repos {
        let acc = acceso(&prueba, r)?;
        resguardo_motor::restic::snapshots(&acc).map_err(|e| format!("Con los datos nuevos no se abre el repositorio «{r}»: {e}"))?;
    }
    *v = prueba;
    // 15i: las copias derivadas (y la externa) que van a este destino guardan su propia
    // dirección: con la nueva (su carpeta se movió con la del usuario en el almacén).
    let derivadas = rehacer_derivadas_hacia(v, &id);
    // Las copias programadas, con el acceso nuevo.
    if let Some(cfg) = v.config_v1.clone() {
        aplicar_config(v, &json!({ "config": cfg }))?;
    } else {
        let _ = subir_config(v);
    }
    let mut m = format!("Destino actualizado ({} repositorios comprobados).", repos.len());
    match derivadas {
        Ok(0) => {}
        Ok(1) => m.push_str(" También la dirección de 1 copia derivada que va a él."),
        Ok(n) => m.push_str(&format!(" También la dirección de {n} copias derivadas que van a él.")),
        Err(e) => m.push_str(&format!(" No se pudo poner al día la dirección de sus copias derivadas: {e}. Ábrelas y guárdalas sin cambios para rehacerla.")),
    }
    Ok(m)
}

/// 15i: rehace la dirección de la copia externa y de las copias derivadas de cualquier
/// repositorio del equipo que van al destino `destino`, con lo que ese destino dice ahora (su
/// carpeta de siempre dentro de él: `ruta`, o la de por defecto). Devuelve cuántas cambió.
pub(crate) fn rehacer_derivadas_hacia(v: &Vinculo, destino: &str) -> Result<usize, String> {
    let Some(d) = v.destinos.iter().find(|d| d.id == destino) else { return Ok(0) };
    let mut n = 0;
    for r in &v.repos_v2 {
        let mut pasos: Vec<(Option<String>, String)> = Vec::new();
        if let Some(e) = r.externa.as_ref().filter(|e| e["destino"] == destino) {
            pasos.push((None, e["ruta"].as_str().map(str::to_string).unwrap_or_else(|| r.id.clone())));
        }
        for e in r.derivadas.iter().filter(|e| e["destino"] == destino) {
            let Some(id) = e["id"].as_str() else { continue };
            pasos.push((Some(id.to_string()), e["ruta"].as_str().map(str::to_string).unwrap_or_else(|| format!("{}-{id}", r.id))));
        }
        for (id, ruta) in pasos {
            // Como `acceso_destino`, sin tocar nada más (la autoridad TLS y las nubes, igual que antes).
            let location = ubicacion(d, &ruta)?;
            let auth = if d.tipo == "rest" { d.usuario.clone().map(|u| (u, d.secreto.clone().unwrap_or_default())) } else { None };
            let con_auth = crate::tasks::location_with_auth(&location, auth.as_ref());
            if crate::agent::cambiar_ubicacion_derivada(&r.id, id.as_deref(), &location, &con_auth)? {
                n += 1;
            }
        }
    }
    Ok(n)
}

/// `nombre_repositorio { repo, nombre }` (B1, inofensiva): el nombre que se ve de un
/// repositorio, igual en todas las consolas. No cambia su id ni su carpeta.
pub fn nombre_repositorio(v: &mut Vinculo, c: &Value) -> Result<String, String> {
    let id = texto(c, "repo");
    let nombre = texto(c, "nombre").trim().to_string();
    if !texto_valido(&nombre, 80) {
        return Err("Escribe un nombre para el repositorio (hasta 80 caracteres).".into());
    }
    let r = v.repos_v2.iter_mut().find(|r| r.id == id).ok_or("Ese repositorio no lo gestiona este servidor.")?;
    if r.nombre == nombre {
        return Ok(format!("El repositorio ya se llama «{nombre}»."));
    }
    let antes = std::mem::replace(&mut r.nombre, nombre.clone());
    Ok(format!("«{antes}» ahora se llama «{nombre}». Lo verán así todas sus consolas."))
}

/// Como mucho, caracteres del nombre de una copia (el motor no admite más: `Plan::validate`).
pub const MAX_NOMBRE_COPIA: usize = 60;

/// 0.8.1 (15t, `admite: "nombre_copia"`): `nombre_copia { copia, nombre }` o
/// `{ derivada: { repo, id }, nombre }` (clave de administración, sin espera: no reduce la
/// protección). Cambia el nombre que se ve de una copia o de una copia derivada, igual en
/// todas las consolas y en el historial («"X" ahora se llama "Y"»). La copia sigue igual
/// (mismo id, mismas versiones): solo se vuelve a aplicar su configuración con el nombre nuevo.
pub fn nombre_copia(v: &mut Vinculo, c: &Value) -> Result<String, String> {
    let nombre = texto(c, "nombre").trim().to_string();
    if !texto_valido(&nombre, MAX_NOMBRE_COPIA) {
        return Err(format!("Escribe un nombre (hasta {MAX_NOMBRE_COPIA} caracteres)."));
    }
    if let Some(d) = c.get("derivada").filter(|d| d.is_object()) {
        let (repo, id) = (texto(d, "repo"), texto(d, "id"));
        let r = v.repos_v2.iter_mut().find(|r| r.id == repo && !r.solo_lectura).ok_or("Ese repositorio no lo gestiona este servidor.")?;
        let de = r.nombre.clone();
        let e = r.derivadas.iter_mut().find(|e| e["id"] == id.as_str()).ok_or("Esa copia derivada ya no está.")?;
        let antes = e["nombre"].as_str().map(str::to_string).unwrap_or_else(|| format!("{de} (copia derivada)"));
        if antes == nombre {
            return Ok(format!("La copia derivada ya se llama «{nombre}»."));
        }
        e["nombre"] = json!(nombre);
        let _ = subir_config(v);
        return Ok(format!("«{antes}» ahora se llama «{nombre}». Lo verán así todas sus consolas."));
    }
    let id = texto(c, "copia");
    let mut cfg: Configuracion = v
        .config_v1
        .clone()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| format!("Configuración guardada no válida: {e}"))?
        .ok_or("Este equipo aún no tiene copias.")?;
    let k = cfg.copias.iter_mut().find(|k| k.id == id).ok_or("Esa copia ya no está en este equipo.")?;
    if k.nombre == nombre {
        return Ok(format!("La copia ya se llama «{nombre}»."));
    }
    let antes = std::mem::replace(&mut k.nombre, nombre.clone());
    // La marca «cambiado en el equipo» se queda como estaba: el nombre no cambia lo que se hace.
    let marca = cfg.cambiado_en_equipo.clone();
    let cuerpo = json!({ "config": serde_json::to_value(&cfg).map_err(|e| e.to_string())? });
    aplicar_config_marca(v, &cuerpo, marca)?;
    Ok(format!("«{antes}» ahora se llama «{nombre}». Lo verán así todas sus consolas."))
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
    // 15p: quitarla también si el repositorio ya no tiene copias activas (antes, ese caso
    // fallaba y la consola la seguía enseñando).
    if !solo_probar && !c["hora"].is_string() {
        return quitar_copia_externa(v, repo);
    }
    if !solo_probar && !crate::agent::load_config().repos.iter().any(|r| r.id == repo) {
        return Err("Ese repositorio aún no tiene copias activas en este equipo: aplica antes una configuración con alguna copia.".into());
    }
    let hora = c["hora"].as_str().map(str::to_string).unwrap_or_default();
    let actual = v.repos_v2.iter().find(|r| r.id == repo).and_then(|r| r.externa.clone()).unwrap_or(Value::Null);
    let guardada = || crate::agent::load_secrets().ok().and_then(|s| s.get(repo).and_then(|x| x.offsite_password.clone()));
    let p = preparar_derivada(v, c, repo, &actual, repo, &guardada, solo_probar)?;
    if solo_probar {
        return Ok(p.texto);
    }
    if !(hora.len() == 5 && chrono::NaiveTime::parse_from_str(&hora, "%H:%M").is_ok()) {
        return Err("La hora no es válida (HH:MM).".into());
    }
    let offsite = p.offsite(crate::agent::Schedule::Daily { time: hora.clone() }, None, None)?;
    crate::agent::set_offsite(repo, Some(offsite), Some(p.creds))?;
    if let Some(d) = p.nuevo.clone() {
        v.destinos.push(d);
    }
    if let Some(r) = v.repos_v2.iter_mut().find(|r| r.id == repo) {
        let mut e = p.entrada.clone();
        e["hora"] = json!(hora);
        // 15p: el destino se creó para ella (al quitarla, se olvida si nadie más lo usa).
        if p.nuevo.is_some() || r.externa.as_ref().is_some_and(|x| x["destino_propio"] == true && x["destino"] == e["destino"]) {
            e["destino_propio"] = json!(true);
        }
        r.externa = Some(e);
    }
    let _ = subir_config(v);
    Ok(format!("Copia derivada a «{}» cada día a las {hora}. {}", p.destino.nombre, p.texto))
}

/// 15p: quita la copia derivada «de cada día» (la copia externa de antes): su tarea en el
/// agente y `externa` en `repos_v2`, que es lo que el resumen manda a las consolas (si no, la
/// seguían enseñando aunque ya no se hacía). Y el destino que se creó solo para ella, si ya
/// no lo usa nada (nunca se borra nada de lo guardado allí).
fn quitar_copia_externa(v: &mut Vinculo, repo: &str) -> Result<String, String> {
    let config = crate::agent::load_config();
    if config.repos.iter().any(|r| r.id == repo && r.offsite.is_some()) {
        crate::agent::set_offsite(repo, None, None)?;
    }
    let antes = v.repos_v2.iter_mut().find(|r| r.id == repo).and_then(|r| r.externa.take());
    let mut m = match &antes {
        Some(_) => "Copia derivada de cada día quitada (lo ya copiado sigue en su destino).".to_string(),
        None => "Este repositorio ya no tenía copia derivada de cada día.".to_string(),
    };
    if let Some(t) = antes.as_ref().and_then(|e| quitar_destino_propio(v, e)) {
        m = format!("{m} {t}");
    }
    let _ = subir_config(v);
    Ok(m)
}

/// 15p: si la copia `e` (externa o derivada) creó su destino (`destino_propio`) y ya no lo
/// usa nada en el equipo, se olvida con sus credenciales. Lo guardado allí se queda.
fn quitar_destino_propio(v: &mut Vinculo, e: &Value) -> Option<String> {
    if e["destino_propio"] != true {
        return None;
    }
    let id = e["destino"].as_str()?;
    if !usos_destino(v, id).is_empty() {
        return None;
    }
    let pos = v.destinos.iter().position(|d| d.id == id)?;
    let d = v.destinos.remove(pos);
    Some(texto_destino_quitado(&d))
}

/// 15p: al arrancar, las copias derivadas «de cada día» que siguen en `repos_v2` (y por eso en
/// el resumen de las consolas) pero cuyo repositorio ya no tiene la tarea en la configuración
/// del agente: se quitan (las dejaba así quitarla en un agente 0.8.0). Solo las de repositorios
/// que están en la configuración (si no, no se toca nada: podría ser una configuración que no
/// se pudo leer). Devuelve los repositorios arreglados.
pub fn limpiar_externas_sin_tarea(v: &mut Vinculo, config: &crate::agent::AgentConfig) -> Vec<String> {
    let mut arreglados = Vec::new();
    let huerfanas: Vec<(String, Value)> = v
        .repos_v2
        .iter_mut()
        .filter(|r| r.externa.is_some() && config.repos.iter().any(|x| x.id == r.id && x.offsite.is_none()))
        .filter_map(|r| r.externa.take().map(|e| (r.id.clone(), e)))
        .collect();
    for (repo, e) in huerfanas {
        quitar_destino_propio(v, &e);
        arreglados.push(repo);
    }
    arreglados
}

/// Lo que se comprueba y prepara antes de guardar una copia externa o
/// derivada (nada se guarda hasta tenerlo todo).
pub(crate) struct Preparada {
    /// La ubicación de restic del repositorio de destino (sin credenciales).
    location: String,
    retencion: Option<Retencion>,
    /// El destino (el que ya tenía el equipo o `nuevo`).
    destino: Destino,
    /// Un destino que aún no tenía el equipo (se guarda al final).
    nuevo: Option<Destino>,
    dest: crate::tasks::DestinoExterno,
    creds: crate::agent::OffsiteSecrets,
    /// Lo que se guarda en `RepoV2` (sin secretos): `{ destino, existente?, ruta?, bloqueo_dias?, solo_anadir? }`.
    entrada: Value,
    /// Lo que dijo la comprobación y lo que pasa con la retención.
    texto: String,
}

impl Preparada {
    /// La tarea del agente con este destino y este horario.
    fn offsite(
        &self,
        schedule: crate::agent::Schedule,
        filtro: Option<crate::adoptar_v2::Filtro>,
        verify: Option<crate::tasks::Verify>,
    ) -> Result<crate::tasks::Offsite, String> {
        let mut o: crate::tasks::Offsite = serde_json::from_value(json!({
            "location": self.location, "provider": format!("destino:{}", self.destino.id), "schedule": schedule,
            "retention": self.retencion.as_ref().map(Retencion::politica), "target_name": self.destino.nombre, "enabled_at": chrono::Local::now().to_rfc3339(),
        }))
        .map_err(|e| e.to_string())?;
        o.dest = self.dest.clone();
        o.filtro = filtro;
        o.verify = verify;
        Ok(o)
    }
}

/// Lo común de `cambiar_copia_externa` y `cambiar_derivada`: el destino (uno del
/// equipo o uno nuevo), la carpeta del repositorio allí (`ruta`, o `ruta_defecto`),
/// el bloqueo, la retención y la contraseña (la escrita, la guardada si es el
/// mismo destino, o la del origen); abre o crea el repositorio de destino.
/// `actual`: lo guardado de esa misma copia (o `null`).
fn preparar_derivada(
    v: &Vinculo,
    c: &Value,
    repo: &str,
    actual: &Value,
    ruta_defecto: &str,
    guardada: &dyn Fn() -> Option<String>,
    solo_probar: bool,
) -> Result<Preparada, String> {
    let dest = &c["destino"];
    let destino_id = texto(dest, "id");
    if !id_valido(&destino_id) {
        return Err("Id de destino no válido.".into());
    }
    // Cambiar la hora, la retención o el bloqueo de la misma copia (`destino: {id}` al
    // que ya va, sin `existente` ni `ruta`): se queda su carpeta (la de uno que ya
    // existía), su contraseña y, si no se dice, su bloqueo.
    let misma = actual["destino"] == destino_id.as_str() && dest.get("tipo").is_none() && c.get("existente").is_none() && c.get("ruta").is_none();
    let actual = if misma { actual.clone() } else { Value::Null };
    let existente = c["existente"] == true || actual["existente"] == true;
    // La carpeta del repositorio en el destino: la suya (si ya existe) o la de por defecto.
    let ruta = match c["ruta"].as_str().or(actual["ruta"].as_str()).map(|r| r.trim().trim_matches('/').to_string()) {
        Some(r) if existente || !r.is_empty() => r,
        _ if existente => return Err("Falta la carpeta del repositorio que ya existe.".into()),
        _ => ruta_defecto.to_string(),
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
            // Tarea 4a: el nombre de la nube conectada en este equipo.
            nube: (dest["tipo"] == "nube").then(|| texto(dest, "nube").trim().to_string()),
        };
        if d.tipo == "nube" && crate::nube::buscar(d.nube.as_deref().unwrap_or_default()).is_none() {
            return Err(format!("La nube «{}» no está conectada en este equipo: conéctala antes.", d.nube.as_deref().unwrap_or_default()));
        }
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
        return Err("La copia tiene que ir a otro destino (otro disco, otro servidor o la nube).".into());
    }
    let retencion = c
        .get("retencion")
        .filter(|r| r.is_object())
        .map(|r| serde_json::from_value::<Retencion>(r.clone()))
        .transpose()
        .map_err(|e| format!("Retención no válida: {e}"))?;
    if let Some(r) = &retencion {
        r.valida()?;
    }
    // Lo que se va a usar, comprobado antes de guardar nada.
    let src = acceso(v, repo)?;
    let contrasena_destino = c["contrasena_destino"].as_str().filter(|s| !s.is_empty()).map(str::to_string).or_else(|| misma.then(guardada).flatten());
    // 14jj: con la autoridad TLS propia del destino (un almacén), si la tiene: cada subida
    // lleva las dos, la del origen y la del destino (`tasks::copy_access`).
    let dest_acc = acceso_destino(&d, &ruta, contrasena_destino.as_deref().unwrap_or(&src.password))?;
    if dest_acc.location.trim_end_matches(['/', '\\']).eq_ignore_ascii_case(src.location.trim_end_matches(['/', '\\'])) {
        return Err("Ese es el mismo repositorio de origen: la copia tiene que ir a otro.".into());
    }
    // Como lo abrirá cada subida: el usuario del servidor dentro de la dirección.
    let con_auth = crate::tasks::location_with_auth(&dest_acc.location, dest_acc.rest_auth.as_ref());
    let dest_run = resguardo_motor::restic::Access {
        location: con_auth.clone(),
        password: dest_acc.password.clone(),
        rest_auth: None,
        cacert: dest_acc.cacert.clone(),
        env: dest_acc.env.clone(),
    };
    let probado = probar_externa(&src, &dest_run, existente, solo_probar)?;
    let solo_anadir = (d.tipo == "rest")
        .then(|| {
            crate::protection::probe_append_only(
                &dest_acc.location,
                dest_acc.rest_auth.as_ref().map(|(u, p)| (u.as_str(), p.as_str())),
                dest_acc.cacert.as_deref(),
            )
        })
        .flatten()
        == Some(true);
    let efecto = efecto_retencion(retencion.is_some(), bloqueo, solo_anadir);
    let mut entrada = json!({ "destino": d.id });
    if existente {
        entrada["existente"] = json!(true);
    }
    // Su carpeta (solo en el equipo: el resumen no la lleva), para cambiar luego la hora sin repetirla.
    if existente || ruta != ruta_defecto {
        entrada["ruta"] = json!(ruta);
    }
    if let Some(b) = bloqueo {
        entrada["bloqueo_dias"] = json!(b);
    }
    if solo_anadir {
        entrada["solo_anadir"] = json!(true);
    }
    let nube = d.nube.clone().filter(|_| d.tipo == "nube");
    Ok(Preparada {
        location: dest_acc.location.clone(),
        retencion,
        destino: d,
        nuevo,
        dest: crate::tasks::DestinoExterno {
            existing: existente,
            object_lock_days: bloqueo,
            append_only: solo_anadir,
            nube: nube.clone(),
            cacert: dest_acc.cacert.clone(),
            limit_windows: vec![],
        },
        // Las credenciales de una nube no se guardan con la copia: se ponen al día en cada uso.
        creds: crate::agent::OffsiteSecrets {
            password: contrasena_destino,
            key_id: None,
            key_secret: None,
            location: Some(con_auth),
            env: if nube.is_some() { Vec::new() } else { dest_acc.env.clone() },
        },
        entrada,
        texto: format!("{probado}{efecto}"),
    })
}

// ---------- Copias derivadas (tarea 4b, docs/copias-en-cadena.md) ----------

/// Como mucho, copias derivadas por repositorio (además de la externa de siempre).
pub const MAX_DERIVADAS: usize = 8;

/// Id de una copia derivada: `[a-z0-9_-]`, de 1 a 40, y no «externa» (que es la de siempre).
pub fn id_derivada_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id != "externa" && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Cuándo se hace una copia derivada: `tras_copia: true` («después de cada
/// copia» del repositorio, como mucho cada `min_minutos`), `horario` (el de las
/// copias, con sus reglas) o `hora` («HH:MM», cada día). Devuelve el horario
/// del agente y lo que se guarda (`cuando`).
pub fn cuando_derivada(c: &Value) -> Result<(crate::agent::Schedule, Value), String> {
    if c["tras_copia"] == true {
        let min = match &c["min_minutos"] {
            Value::Null => 0,
            m => m.as_u64().filter(|m| *m <= 1440).ok_or("Los minutos entre dos subidas van de 0 a 1440.")? as u32,
        };
        return Ok((crate::agent::Schedule::AfterBackup { min_minutes: min }, json!({ "tras_copia": true, "min_minutos": min })));
    }
    if let Some(h) = c.get("horario").filter(|h| h.is_object()) {
        let horario: Horario = serde_json::from_value(h.clone()).map_err(|_| "Horario no válido.".to_string())?;
        let plan = horario.plan_schedule()?;
        plan.validate()?;
        return Ok((crate::agent::Schedule::Rules { rules: plan.effective_rules() }, json!({ "horario": horario })));
    }
    match c["hora"].as_str() {
        Some(h) if h.len() == 5 && chrono::NaiveTime::parse_from_str(h, "%H:%M").is_ok() => {
            Ok((crate::agent::Schedule::Daily { time: h.to_string() }, json!({ "hora": h })))
        }
        Some(_) => Err("La hora no es válida (HH:MM).".into()),
        None => Err("Di cuándo: a una hora, con un horario o después de cada copia.".into()),
    }
}

/// ¿Reduce la protección este `cambiar_derivada`? Si cambia una que ya existe
/// y le pone (o cambia) la retención, o la lleva a otro destino (lo de allí deja
/// de recibir copias). Crear una nueva o cambiar solo su horario, no.
pub fn derivada_reduce(v: &Vinculo, c: &Value, repo: &str) -> bool {
    let id = c["id"].as_str().unwrap_or_default();
    let Some(actual) = v.repos_v2.iter().find(|r| r.id == repo).and_then(|r| r.derivadas.iter().find(|d| d["id"] == id)) else { return false };
    c.get("retencion").is_some_and(Value::is_object) || c["destino"]["id"] != actual["destino"]
}

/// `cambiar_derivada { repo, id, destino: {id} | {id, nombre, tipo, donde, usuario?, secreto?}, hora? | horario? | tras_copia? (min_minutos?),
///  retencion?, contrasena_destino?, existente?, ruta?, bloqueo_dias?, filtro?, verificacion?, solo_probar? }` (contraseña del
/// repositorio; tarea 4b): otra copia derivada del repositorio, además de la externa de siempre. Lo mismo que
/// `cambiar_copia_externa` (crear el repositorio en el destino con el troceado del origen, uno que ya existe,
/// bloqueo de objetos, «Probar»), con:
/// - `id`: el de la derivada (`[a-z0-9_-]`, no «externa»); si ya existe, se cambia;
/// - cuándo: `hora`, `horario` (el de las copias) o `tras_copia` («después de cada copia», [`cuando_derivada`]);
/// - `contrasena_destino`: otra contraseña (sin ella, la del origen o la que ya tenía); la consola la lleva al kit;
/// - `filtro` (tarea 4c): `{ equipos?, etiquetas?, carpetas?, desde?, ultimos_dias? }`, qué versiones se suben;
/// - `verificacion`: `{ cada_dias, porcentaje, horario? }`, la del repositorio de destino.
///
/// Por defecto el repositorio va en `<destino>/<repo>-<id>`. Reduce la protección
/// (espera) solo si cambia la retención o el destino de una que ya existía ([`derivada_reduce`]).
pub fn cambiar_derivada(v: &mut Vinculo, c: &Value, repo: &str) -> Result<String, String> {
    if !v.repos_v2.iter().any(|r| r.id == repo && !r.solo_lectura) {
        return Err("Ese repositorio no lo gestiona este servidor.".into());
    }
    let id = texto(c, "id");
    if !id_derivada_valido(&id) {
        return Err("Id de copia derivada no válido.".into());
    }
    let solo_probar = c["solo_probar"] == true;
    if !solo_probar && !crate::agent::load_config().repos.iter().any(|r| r.id == repo) {
        return Err("Ese repositorio aún no tiene copias activas en este equipo: aplica antes una configuración con alguna copia.".into());
    }
    let r = v.repos_v2.iter().find(|r| r.id == repo).ok_or("Ese repositorio no lo gestiona este servidor.")?;
    let actual = r.derivadas.iter().find(|d| d["id"] == id.as_str()).cloned().unwrap_or(Value::Null);
    if actual.is_null() && r.derivadas.len() >= MAX_DERIVADAS {
        return Err(format!("Como mucho {MAX_DERIVADAS} copias derivadas por repositorio (además de la de cada día)."));
    }
    let guardada = || crate::agent::load_secrets().ok().and_then(|s| s.get(repo).and_then(|x| x.derived.get(&id).and_then(|d| d.password.clone())));
    let ruta_defecto = format!("{repo}-{id}");
    let p = preparar_derivada(v, c, repo, &actual, &ruta_defecto, &guardada, solo_probar)?;
    // Dos copias del mismo repositorio no pueden ir al mismo repositorio de destino.
    let misma_carpeta = |e: &Value, defecto: &str| {
        e["destino"] == p.destino.id.as_str() && e["ruta"].as_str().unwrap_or(defecto) == p.entrada["ruta"].as_str().unwrap_or(&ruta_defecto)
    };
    let otras_derivadas =
        r.derivadas.iter().filter(|e| e["id"] != id.as_str()).any(|e| misma_carpeta(e, &format!("{repo}-{}", e["id"].as_str().unwrap_or_default())));
    if otras_derivadas || r.externa.as_ref().is_some_and(|e| misma_carpeta(e, repo)) {
        return Err("Otra copia de este repositorio ya va a esa carpeta de ese destino: elige otra carpeta u otro destino.".into());
    }
    if solo_probar {
        return Ok(p.texto);
    }
    let (schedule, cuando) = cuando_derivada(c)?;
    let filtro = match c.get("filtro") {
        None | Some(Value::Null) => None,
        Some(f) => Some(crate::adoptar_v2::Filtro::de(f)?).filter(|f| !f.todas()),
    };
    let verificacion: Option<VerificacionAuto> = c
        .get("verificacion")
        .filter(|x| x.is_object())
        .map(|x| serde_json::from_value(x.clone()))
        .transpose()
        .map_err(|e| format!("Verificación no válida: {e}"))?;
    if let Some(va) = &verificacion {
        va.valida()?;
    }
    let mut offsite = p.offsite(schedule, filtro.clone(), verificacion.as_ref().map(|va| va.verify(chrono::Local::now())))?;
    // Plan 0.8.0, F4 (14t): `limite { kib?, franjas? }`, la velocidad de subida según la hora.
    let limite = limite_de(c)?;
    if let Some((kib, franjas)) = &limite {
        offsite.limit_upload_kib = *kib;
        offsite.dest.limit_windows = franjas.clone();
    }
    crate::agent::set_derived(repo, &id, Some(crate::tasks::Derived { id: id.clone(), offsite }), Some(p.creds))?;
    if let Some(d) = p.nuevo.clone() {
        v.destinos.push(d);
    }
    let mut e = p.entrada.clone();
    e["id"] = json!(id);
    e["cuando"] = cuando;
    // 15t: su nombre (el que llega o el que ya tenía).
    match c["nombre"].as_str().map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) if texto_valido(n, MAX_NOMBRE_COPIA) => e["nombre"] = json!(n),
        Some(_) => return Err(format!("El nombre de la copia derivada: hasta {MAX_NOMBRE_COPIA} caracteres.")),
        None if actual["nombre"].is_string() => e["nombre"] = actual["nombre"].clone(),
        None => {}
    }
    // 15p: el destino se creó para ella (al quitarla, se olvida si nadie más lo usa).
    if p.nuevo.is_some() || (actual["destino_propio"] == true && actual["destino"] == e["destino"]) {
        e["destino_propio"] = json!(true);
    }
    if let Some(f) = &filtro {
        e["filtro"] = json!(f);
    }
    if let Some(va) = &verificacion {
        e["verificacion"] = json!(va);
    }
    if let Some((kib, franjas)) = &limite {
        e["limite"] = json!({ "kib": kib, "franjas": franjas });
    }
    if let Some(r) = v.repos_v2.iter_mut().find(|r| r.id == repo) {
        r.derivadas.retain(|x| x["id"] != id.as_str());
        r.derivadas.push(e);
    }
    let _ = subir_config(v);
    let que = match &filtro {
        Some(f) => format!(" (solo las versiones {})", f.texto()),
        None => String::new(),
    };
    Ok(format!("Copia derivada a «{}»{que} guardada. {}", p.destino.nombre, p.texto))
}

/// Plan 0.8.0, F4 (14t): `limite { kib?, franjas? }` de una copia derivada (`None` sin
/// el campo o sin nada: sin límite). `kib`: fuera de las franjas.
/// F4: el límite fuera de las franjas (KiB/s) y las franjas.
pub type Limite = (Option<u32>, Vec<crate::franjas::Franja>);

pub fn limite_de(c: &Value) -> Result<Option<Limite>, String> {
    let Some(l) = c.get("limite").filter(|l| l.is_object()) else { return Ok(None) };
    let kib = match &l["kib"] {
        Value::Null => None,
        x => Some(x.as_u64().and_then(|k| u32::try_from(k).ok()).ok_or("Límite de velocidad no válido.")?).filter(|k| *k > 0),
    };
    if kib.is_some_and(|k| !(crate::franjas::MIN_KIB..=crate::franjas::MAX_KIB).contains(&k)) {
        return Err(format!("El límite de velocidad va de {} KiB/s a {} KiB/s.", crate::franjas::MIN_KIB, crate::franjas::MAX_KIB));
    }
    let franjas: Vec<crate::franjas::Franja> = match l.get("franjas") {
        None | Some(Value::Null) => vec![],
        Some(f) => serde_json::from_value(f.clone()).map_err(|_| "Franjas de velocidad no válidas.".to_string())?,
    };
    crate::franjas::validar(&franjas)?;
    Ok((kib.is_some() || !franjas.is_empty()).then_some((kib, franjas)))
}

/// `quitar_derivada { repo, id }` (contraseña del repositorio; reduce la
/// protección: espera): deja de hacerla. Lo ya copiado sigue en su destino.
pub fn quitar_derivada(v: &mut Vinculo, c: &Value, repo: &str) -> Result<String, String> {
    let id = texto(c, "id");
    let r = v.repos_v2.iter().find(|r| r.id == repo && !r.solo_lectura).ok_or("Ese repositorio no lo gestiona este servidor.")?;
    if !r.derivadas.iter().any(|d| d["id"] == id.as_str()) {
        return Err("Esa copia derivada ya no está.".into());
    }
    // Si el agente ya no la tiene (el repositorio no tiene copias activas), basta con olvidarla.
    if crate::agent::load_config().repos.iter().any(|x| x.id == repo) {
        crate::agent::set_derived(repo, &id, None, None)?;
    }
    let quitada = v.repos_v2.iter_mut().find(|r| r.id == repo).and_then(|r| {
        let pos = r.derivadas.iter().position(|x| x["id"] == id.as_str())?;
        Some(r.derivadas.remove(pos))
    });
    let mut m = "Copia derivada quitada (lo ya copiado sigue en su destino).".to_string();
    // 15p: y el destino que se creó solo para ella, si ya no lo usa nada.
    if let Some(t) = quitada.as_ref().and_then(|e| quitar_destino_propio(v, e)) {
        m = format!("{m} {t}");
    }
    let _ = subir_config(v);
    Ok(m)
}

/// Lo que va al resumen de una derivada (sin rutas: del filtro, solo cuántas carpetas).
fn derivada_resumen(v: &Vinculo, repo: &str, e: &Value, agente: Option<&crate::agent::AgentRepo>) -> Value {
    let id = e["id"].as_str().unwrap_or_default();
    let tarea = agente.and_then(|a| a.derived.iter().find(|d| d.id == id));
    let mut filtro = e["filtro"].clone();
    if let Some(n) = filtro.get("carpetas").and_then(Value::as_array).map(Vec::len) {
        filtro["carpetas"] = json!(n);
    }
    json!({
        "id": id,
        // 15t: su nombre, si se le puso (sin él, la consola dice «<repositorio> (copia derivada)»).
        "nombre": e.get("nombre").filter(|n| n.is_string()),
        "destino": v.destinos.iter().find(|d| Some(d.id.as_str()) == e["destino"].as_str()).map(|d| d.nombre.clone()),
        "destino_id": e["destino"],
        "cuando": e["cuando"],
        "existente": e.get("existente"),
        "bloqueo_dias": e.get("bloqueo_dias"),
        "solo_anadir": e.get("solo_anadir"),
        "filtro": Some(filtro).filter(|f| f.is_object()),
        "verificacion": e.get("verificacion"),
        // Plan 0.8.0, F4: la velocidad de subida según la hora (sin nada, no va).
        "limite": e.get("limite"),
        // 15g: el nombre de su carpeta en el destino (sin la ruta del destino), si no es el de
        // siempre (`<repo>-<id>`): la consola la busca en el almacén para su retención.
        "ruta": e.get("ruta").filter(|r| r.as_str().is_some_and(|r| !r.contains(['/', '\\']))),
        // Si el agente la tiene (un repositorio sin copias activas la pierde) y si aplica retención allí.
        "activa": tarea.is_some(),
        "con_retencion": tarea.map(|t| t.offsite.retention.as_ref().is_some_and(|p| !p.is_empty())),
        "repo": repo,
    })
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
/// - tarea 7b (docs/copias-en-cadena.md): `{zona: {nombre?, carpeta, puerto}}`
///   crea otra zona (otro disco, su propio rest-server en su puerto),
///   `{zona: {id, nombre}}` la renombra, `{quitar_zona: "<id>"}` la quita
///   (destructiva) y `anadir` / `quitar` con `zona: "<id>"` son de esa zona;
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
        // Tarea 7b: en una zona (otro disco del almacén, con su puerto) o en la principal.
        let zona = zona_de(c)?;
        // B1 (docs/nombres-legibles.md): con `nombre`, la carpeta del equipo es legible (`pc-01-3f2a`).
        let legible = crate::nombres::texto_legible(c, "nombre")?;
        let (usuario, contrasena, ubicacion, puerto) = server::anadir_equipo_legible(nombre, zona.as_deref(), legible.as_deref())?;
        // Que el rest-server ya lo acepte cuando el equipo cliente cree su repositorio.
        server::esperar_usuario_de(zona.as_deref(), &usuario, &contrasena);
        let cfg = server::load();
        let ubicacion = if c["local"] == true { server::ubicacion_local(puerto, &usuario) } else { ubicacion };
        let mut privado = json!({
            "usuario": usuario, "contrasena": contrasena,
            "destino": { "tipo": "rest", "donde": ubicacion.trim_start_matches("rest:"), "usuario": usuario, "secreto": contrasena,
                         "ca_pem": std::fs::read_to_string(server::cert_file()).ok() },
            "huella_tls": cfg.tls_sha256,
        });
        // La consola comprueba que la respuesta es de la zona que pidió (un agente
        // anterior ignoraría `zona` y daría un usuario de la principal).
        if let Some(z) = &zona {
            privado["zona"] = json!(z);
        }
        let donde = zona.and_then(|z| cfg.zonas.iter().find(|x| x.id == z).map(|x| format!(" en la zona «{}»", x.nombre))).unwrap_or_default();
        return Ok((format!("Equipo cliente «{usuario}» añadido{donde}."), Some(privado)));
    }
    // 0.8.1 (15gg): un repositorio suelto del almacén, a la carpeta de un equipo (sin copiar).
    if let Some(d) = c.get("dar_suelto").filter(|d| d.is_object()) {
        crate::agent::require_admin()?;
        let cfg = server::load();
        if !cfg.enabled {
            return Err("Este equipo no guarda copias de otros.".into());
        }
        let (m, _) = crate::sueltos::dar(&cfg, d)?;
        return Ok((m, None));
    }
    // 15gg: dónde buscar además repositorios sueltos (fuera de la carpeta del almacén).
    if let Some(l) = c.get("sueltos_en") {
        crate::agent::require_admin()?;
        let mut cfg = server::load();
        if !cfg.enabled {
            return Err("Este equipo no guarda copias de otros.".into());
        }
        let m = crate::sueltos::poner_busqueda(&mut cfg, l)?;
        server::save(&cfg)?;
        return Ok((m, None));
    }
    // B1 (docs/nombres-legibles.md): «Ordenar nombres de carpeta» de un equipo.
    if let Some(m) = c.get("mover").filter(|m| m.is_object()) {
        let (mensaje, detalle) = crate::nombres::mover(m)?;
        return Ok((mensaje, Some(detalle)));
    }
    // Tarea 7b: crear una zona (`{ zona: { nombre?, carpeta, puerto } }`) o
    // cambiar su nombre (`{ zona: { id, nombre } }`).
    if let Some(z) = c.get("zona").filter(|z| z.is_object()) {
        if let Some(id) = z["id"].as_str() {
            let nombre = z["nombre"].as_str().ok_or("Falta el nombre nuevo de la zona.")?;
            server::renombrar_zona(id, nombre)?;
            return Ok((format!("Zona renombrada: «{}».", nombre.trim()), None));
        }
        let carpeta = z["carpeta"].as_str().ok_or("Falta la carpeta de la zona.")?;
        let puerto = z["puerto"].as_u64().and_then(|p| u16::try_from(p).ok()).ok_or("Puerto no válido.")?;
        let zona = server::crear_zona(z["nombre"].as_str(), carpeta, puerto)?;
        // La pone en marcha la tarea del Servidor de copias: se espera un poco a
        // que responda, para que el primer equipo que copie allí ya entre.
        let lista = server::esperar_escucha(zona.port, std::time::Duration::from_secs(20));
        let m = if lista {
            format!("Zona «{}» lista en el puerto {} (solo añadir).", zona.nombre, zona.port)
        } else {
            format!("Zona «{}» creada en el puerto {}: su servidor se pone en marcha en unos segundos.", zona.nombre, zona.port)
        };
        return Ok((m, Some(json!({ "zona": zona.id }))));
    }
    if let Some(id) = c["quitar_zona"].as_str() {
        let z = server::quitar_zona(id)?;
        return Ok((format!("Zona «{}» quitada: sus equipos ya no pueden copiar allí (lo guardado se queda en su carpeta).", z.nombre), None));
    }
    // §3b (docs/espejo.md): confirmar lo que falta de golpe en el almacén (espera).
    if let Some(d) = c.get("espejo_freno") {
        return Ok((crate::espejo::aceptar_freno(d)?, None));
    }
    // Plan 0.7.26 (bloque 4): «Hacer ahora» un espejo (del almacén o del propio equipo).
    if let Some(a) = c.get("espejo_ahora") {
        return Ok((crate::espejo_trabajos::pedir_ahora(a, None)?, None));
    }
    // Plan 0.7.26 (bloque 4): los espejos que hace el propio equipo (no hace falta ser almacén).
    if let Some(e) = c.get("espejo_equipo") {
        let t = if e.is_null() { None } else { Some(crate::espejo_trabajos::leer_pedido(e, crate::espejo_trabajos::QUIEN_EQUIPO)?) };
        return Ok((crate::espejo_trabajos::poner_equipo(t)?, None));
    }
    if let Some(espejo) = c.get("espejo") {
        // Plan 0.7.26: `{ trabajos }`; la forma de antes (`destinos`) sigue valiendo.
        let m = if espejo.get("trabajos").is_some_and(|t| !t.is_null()) {
            server::poner_trabajos(Some(crate::espejo_trabajos::leer_pedido(espejo, crate::espejo_trabajos::QUIEN_ALMACEN)?))?
        } else {
            server::poner_espejo(crate::espejo::pedido(espejo)?)?
        };
        return Ok((m, None));
    }
    if let Some(usuario) = c["quitar"].as_str() {
        server::quitar_equipo_en(usuario, zona_de(c)?.as_deref())?;
        return Ok((format!("«{usuario}» ya no puede guardar copias aquí (lo guardado se queda)."), None));
    }
    match c["activo"].as_bool() {
        Some(true) => {
            let carpeta = c["carpeta"].as_str().ok_or("Falta la carpeta donde guardar las copias de los demás.")?;
            let puerto = c["puerto"].as_u64().unwrap_or(8000);
            let puerto = u16::try_from(puerto).map_err(|_| "Puerto no válido.".to_string())?;
            let cfg = server::activar(carpeta, puerto, c["solo_red_local"].as_bool().unwrap_or(true))?;
            Ok((format!("Almacén activado en el puerto {}: este equipo ya guarda copias de otros.", cfg.port), None))
        }
        Some(false) => {
            server::desactivar()?;
            Ok(("Almacén desactivado (las copias guardadas se quedan).".into(), None))
        }
        None => Err("Falta «activo», «anadir» o «quitar».".into()),
    }
}

/// Tarea 7b: la zona de `anadir` o `quitar` (`"zona": "<id>"`); sin ella, la principal.
fn zona_de(c: &Value) -> Result<Option<String>, String> {
    match c.get("zona") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(id)) if crate::server::zona_id_valido(id) => Ok(Some(id.clone())),
        Some(_) => Err("Zona no válida.".into()),
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
        // Tarea 8e: el sistema de archivos de la carpeta (NTFS, ReFS, ext4, zfs…).
        "sistema_archivos": crate::espacio::json_fs(&c.path),
        "repositorios": crate::server::repos_by_user(&c).into_iter().map(|(usuario, repos)| json!({ "usuario": usuario, "repos": repos })).collect::<Vec<_>>(),
        "espejo": c.espejo.as_ref().map(crate::espejo::Espejo::resumen),
        // v1.22: la retención que aplica este almacén (regla, horario y
        // resultados por `usuario/repo`; nunca su clave).
        "retenciones": crate::retencion_almacen::resumen(),
        // Nubes conectadas en este equipo para el espejo (nunca sus tokens).
        "nubes": crate::nube::lista(),
        // Tarea 7b: las otras zonas (otros discos), cada una con su puerto; solo si hay.
        "zonas": (!c.zonas.is_empty()).then(|| crate::server::resumen_zonas(&c)),
        // Plan 0.8.0, F2 (14cc): las versiones que su retención nunca quita (`null` si ninguna).
        "conservadas": crate::versiones_v2::resumen_almacen(),
        // B1: las carpetas movidas («Ordenar nombres de carpeta»), las 50 últimas: la
        // consola ve qué equipo sigue con la ruta de antes. Solo nombres de carpeta.
        "renombres": (!c.renombres.is_empty()).then(|| c.renombres.iter().rev().take(50).collect::<Vec<_>>()),
        // 0.8.1 (15gg): repositorios sueltos en su disco (fuera de la carpeta de ningún
        // equipo), sin abrirlos: carpeta relativa, cuántas versiones y la más reciente.
        "sueltos": crate::sueltos::resumen(&c),
        "sueltos_en": (!c.sueltos_en.is_empty()).then_some(&c.sueltos_en),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 15p: al arrancar se quitan las copias derivadas «de cada día» cuyo repositorio ya no tiene
    /// la tarea (las dejaba así quitarlas en un agente 0.8.0), con el destino creado solo para
    /// ellas si nadie más lo usa; las que tienen su tarea y las de repositorios que no están en la
    /// configuración no se tocan.
    #[test]
    fn externas_sin_tarea_al_arrancar() {
        let repo = |id: &str, con_tarea: bool| -> crate::agent::AgentRepo {
            let mut r: crate::agent::AgentRepo = serde_json::from_value(json!({
                "id": id, "name": id, "location": "x", "paths": [], "schedule": crate::agent::Schedule::Hours { every: 1 },
                "enabled_at": "2026-10-01T00:00:00Z",
            }))
            .unwrap();
            if con_tarea {
                r.offsite = Some(
                    serde_json::from_value(json!({
                        "location": "x", "provider": "destino:d-b", "schedule": crate::agent::Schedule::Daily { time: "21:00".into() },
                        "target_name": "Nube", "enabled_at": "2026-10-01T00:00:00Z",
                    }))
                    .unwrap(),
                );
            }
            r
        };
        let config = crate::agent::AgentConfig { repos: vec![repo("a", false), repo("b", true), repo("d", false)], ..Default::default() };
        let mut v = Vinculo::default();
        let destino = |id: &str| Destino {
            id: id.into(),
            nombre: format!("Destino {id}"),
            tipo: "rest".into(),
            donde: "https://almacen.ejemplo:8000/".into(),
            ..Default::default()
        };
        v.destinos.push(destino("disco"));
        for (id, dest, propio) in [("a", "d-a", true), ("b", "d-b", true), ("c", "d-c", true), ("d", "d-d", false)] {
            v.destinos.push(destino(dest));
            v.repos_v2.push(RepoV2 {
                id: id.into(),
                nombre: id.to_uppercase(),
                destino: "disco".into(),
                externa: Some(json!({ "destino": dest, "hora": "21:00", "destino_propio": propio })),
                ..Default::default()
            });
        }
        assert_eq!(limpiar_externas_sin_tarea(&mut v, &config), vec!["a".to_string(), "d".to_string()]);
        let externa = |id: &str| v.repos_v2.iter().find(|r| r.id == id).unwrap().externa.is_some();
        assert_eq!((externa("a"), externa("b"), externa("c"), externa("d")), (false, true, true, false));
        // El destino creado para «a» se olvida; el de «d» no se sabe si era solo suyo: se queda.
        let ids: Vec<&str> = v.destinos.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["disco", "d-b", "d-c", "d-d"]);
        // Otra vez: nada que hacer.
        assert!(limpiar_externas_sin_tarea(&mut v, &config).is_empty());
        // Un destino propio que aún usa otra cosa (aquí, otro repositorio) no se olvida.
        v.repos_v2.push(RepoV2 { id: "e".into(), destino: "d-b".into(), ..Default::default() });
        assert_eq!(quitar_destino_propio(&mut v, &json!({ "destino": "d-b", "destino_propio": true })), None);
        assert!(v.destinos.iter().any(|d| d.id == "d-b"));
    }

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
        // Tarea 8e: una carpeta de la red no se consulta; el entorno, solo nombres (o nada).
        assert!(d["sistema_archivos"].is_null());
        assert!(r["destinos"][1].get("sistema_archivos").is_none(), "solo en los locales");
        assert!(r["entorno"].is_null() || r["entorno"].is_object());
        let tmp = std::env::temp_dir().to_string_lossy().to_string();
        let v = Vinculo {
            destinos: vec![Destino { id: "d".into(), nombre: "D".into(), tipo: "local".into(), donde: tmp.clone(), ..Default::default() }],
            ..Default::default()
        };
        let r = resumen(&v);
        if cfg!(any(windows, target_os = "linux")) {
            assert!(r["destinos"][0]["sistema_archivos"].is_string(), "{r}");
        }
        // Plan 0.8.1 (C1): su carpeta sí (para «Dónde está»), pero nunca con el nombre de la cuenta.
        assert_eq!(r["destinos"][0]["carpeta"].as_str(), crate::espacio::ruta_publica(&tmp).as_deref());
        if tmp.to_lowercase().contains("users") || tmp.starts_with("/home/") {
            assert!(!r["destinos"][0].to_string().contains(&tmp), "sin el nombre de la cuenta");
        }
        // Una carpeta de la red, nunca (su ruta dice el servidor).
        assert!(v_red_sin_carpeta());
    }

    fn v_red_sin_carpeta() -> bool {
        let local = Destino { id: "nas".into(), nombre: "NAS".into(), tipo: "local".into(), donde: r"\\nas\copias".into(), ..Default::default() };
        let v = Vinculo { destinos: vec![local], ..Default::default() };
        resumen(&v)["destinos"][0].get("carpeta").is_none()
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

    /// Tarea 7c: `tras` (con o sin horario propio) y las cadenas que no valen.
    #[test]
    fn copias_en_cadena() {
        let copia = |id: &str, repo: &str, tras: Option<&str>, horas: &[&str]| -> Copia {
            serde_json::from_value(json!({"id": id, "nombre": id.to_uppercase(), "repo": repo, "carpetas": [r"C:\Datos"],
                "horario": {"dias": if horas.is_empty() { json!([]) } else { json!([1, 2, 3, 4, 5]) }, "horas": horas}, "tras": tras}))
            .unwrap()
        };
        let lista = vec![copia("docs", "r1", None, &["13:00"]), copia("disco-e", "r2", Some("docs"), &[]), copia("nube", "r2", Some("disco-e"), &["23:00"])];
        validar_cadenas(&lista).unwrap();
        // Sin horario propio: el plan va sin horario y con la clave de la anterior (de otro repositorio).
        let p = plan_en(&lista[1], &lista).unwrap();
        assert!(p.schedule.is_none());
        assert_eq!(p.after.as_deref(), Some("r1#docs"));
        // Con horario y además «después de la anterior».
        let p = plan_en(&lista[2], &lista).unwrap();
        assert!(p.schedule.is_some());
        assert_eq!(p.after.as_deref(), Some("r2#disco-e"));
        assert_eq!(plan_en(&lista[0], &lista).unwrap().after, None);
        // Sin horario y sin anterior no vale (como siempre).
        assert!(plan_de(&copia("suelta", "r1", None, &[])).is_err());
        // Ella misma, una que no está o una vuelta: no.
        assert!(validar_cadenas(&[copia("a", "r1", Some("a"), &["10:00"])]).is_err());
        assert!(validar_cadenas(&[copia("a", "r1", Some("zz"), &["10:00"])]).is_err());
        assert!(validar_cadenas(&[copia("a", "r1", Some("b"), &[]), copia("b", "r1", Some("c"), &[]), copia("c", "r1", Some("a"), &[])]).is_err());
        // El resumen y el documento llevan `tras`; sin él, ni aparece.
        assert_eq!(serde_json::to_value(&lista[1]).unwrap()["tras"], "docs");
        assert!(serde_json::to_value(&lista[0]).unwrap().get("tras").is_none());
        assert!(ADMITE.contains(&"cadenas"));
    }

    /// Plan 0.7.26: `inicio` (`horario` | `cadena` | `despues`) y `retraso_min`.
    /// Sin `inicio`, `tras` es «en cadena» (lo de siempre).
    #[test]
    fn inicio_de_las_copias() {
        let copia = |tras: Option<&str>, inicio: Option<&str>, retraso: Option<u32>| -> Copia {
            serde_json::from_value(json!({"id": "k", "nombre": "K", "repo": "r1", "carpetas": [r"C:\Datos"],
                "horario": {"dias": [], "horas": []}, "tras": tras, "inicio": inicio, "retraso_min": retraso}))
            .unwrap()
        };
        use crate::agent::ChainMode;
        // Lo de siempre: `tras` sin `inicio` va en cadena, sin retraso.
        let k = copia(Some("a"), None, None);
        k.valida_inicio().unwrap();
        assert_eq!(k.modo_cadena(), Some(ChainMode { always: false, delay_min: 0 }));
        assert!(serde_json::to_value(&k).unwrap().get("inicio").is_none(), "una consola anterior no lo ve");
        assert_eq!(copia(Some("a"), Some("cadena"), Some(10)).modo_cadena(), Some(ChainMode { always: false, delay_min: 10 }));
        assert_eq!(copia(Some("a"), Some("despues"), None).modo_cadena(), Some(ChainMode { always: true, delay_min: 0 }));
        assert_eq!(copia(None, Some("horario"), None).modo_cadena(), None);
        for (tras, inicio, retraso) in
            [(Some("a"), Some("despues"), Some(0)), (Some("a"), Some("cadena"), Some(MAX_RETRASO_MIN)), (None, Some("horario"), None), (None, None, Some(0))]
        {
            copia(tras, inicio, retraso).valida_inicio().unwrap();
        }
        // Lo que no tiene sentido.
        for (tras, inicio, retraso) in [
            (Some("a"), Some("horario"), None),
            (None, Some("despues"), None),
            (None, Some("cadena"), None),
            (Some("a"), Some("luego"), None),
            (Some("a"), Some("despues"), Some(MAX_RETRASO_MIN + 1)),
            (None, None, Some(5)),
        ] {
            assert!(copia(tras, inicio, retraso).valida_inicio().is_err(), "{tras:?} {inicio:?} {retraso:?}");
        }
        assert!(ADMITE.contains(&"inicio_despues"));
    }

    /// F4 (14t): `limite { kib?, franjas? }` de una copia derivada.
    #[test]
    fn limite_de_velocidad_de_una_derivada() {
        assert_eq!(limite_de(&json!({})).unwrap(), None);
        assert_eq!(limite_de(&json!({ "limite": { "kib": 0, "franjas": [] } })).unwrap(), None, "nada: sin límite");
        let (kib, franjas) =
            limite_de(&json!({ "limite": { "kib": 4096, "franjas": [{ "dias": [1, 2, 3, 4, 5], "desde": "08:00", "hasta": "18:00", "kib": 2048 }] } }))
                .unwrap()
                .unwrap();
        assert_eq!((kib, franjas.len(), franjas[0].kib), (Some(4096), 1, 2048));
        for malo in [
            json!({ "limite": { "kib": 10 } }),
            json!({ "limite": { "kib": "mucho" } }),
            json!({ "limite": { "franjas": [{ "dias": [1], "desde": "18:00", "hasta": "08:00", "kib": 2048 }] } }),
            json!({ "limite": { "franjas": "lunes" } }),
        ] {
            assert!(limite_de(&malo).is_err(), "{malo}");
        }
        assert!(ADMITE.contains(&"limite_franjas"));
    }

    /// F1 (14pp): «solo a mano», «una sola vez» y «termina después de N copias o el día …».
    #[test]
    fn cuando_termina_una_copia() {
        let copia = |extra: Value| -> Copia {
            let mut v = json!({"id": "k", "nombre": "K", "repo": "r1", "carpetas": [r"C:\Datos"], "horario": {"dias": [], "horas": []}});
            for (k, x) in extra.as_object().unwrap() {
                v[k] = x.clone();
            }
            serde_json::from_value(v).unwrap()
        };
        // Solo a mano: sin horario, sin fin, plan sin horario (solo «Copiar ahora»).
        let a_mano = copia(json!({"a_mano": true}));
        a_mano.valida_termina().unwrap();
        assert!(plan_de(&a_mano).unwrap().schedule.is_none());
        assert_eq!(a_mano.fin(), Some(crate::agent::PlanEnd { manual: true, ..Default::default() }));
        // Una sola vez: a una hora, sin horario.
        let una = copia(json!({"termina": {"una_vez": "2026-10-20T21:30"}}));
        una.valida_termina().unwrap();
        assert!(plan_de(&una).unwrap().schedule.is_none());
        let fin = una.fin().unwrap();
        assert!(fin.without_schedule() && fin.ends() && fin.once_local().is_some());
        // Con horario y fin: tras 10 copias o el día …
        let diez = copia(json!({"horario": {"dias": [1], "horas": ["13:00"]}, "termina": {"veces": 10, "hasta": "2027-01-31"}}));
        diez.valida_termina().unwrap();
        assert!(plan_de(&diez).unwrap().schedule.is_some());
        let f = diez.fin().unwrap();
        assert_eq!((f.count, f.until.as_deref(), f.manual), (Some(10), Some("2027-01-31"), false));
        // «Nunca» (vacío) es como sin el campo.
        assert_eq!(copia(json!({"horario": {"dias": [1], "horas": ["13:00"]}, "termina": {}})).fin(), None);
        // Lo que no vale.
        for mala in [
            json!({"a_mano": true, "horario": {"dias": [1], "horas": ["13:00"]}}),
            json!({"a_mano": true, "tras": "otra"}),
            json!({"a_mano": true, "termina": {"veces": 3}}),
            json!({"termina": {"una_vez": "2026-10-20T21:30", "veces": 2}}),
            json!({"termina": {"una_vez": "mañana"}}),
            json!({"horario": {"dias": [1], "horas": ["13:00"]}, "termina": {"veces": 0}}),
            json!({"horario": {"dias": [1], "horas": ["13:00"]}, "termina": {"veces": MAX_VECES + 1}}),
            json!({"horario": {"dias": [1], "horas": ["13:00"]}, "termina": {"hasta": "31/01/2027"}}),
            json!({"horario": {"dias": [1], "horas": ["13:00"]}, "termina": {"veces": 2, "desde": "ayer"}}),
        ] {
            assert!(copia(mala.clone()).valida_termina().is_err(), "{mala}");
        }
        // Sin horario y sin «a mano» ni «una vez», como siempre: no vale.
        assert!(plan_de(&copia(json!({}))).is_err());
        // Un agente anterior no ve nada nuevo si no se usa.
        let v = serde_json::to_value(copia(json!({"horario": {"dias": [1], "horas": ["13:00"]}}))).unwrap();
        assert!(v.get("a_mano").is_none() && v.get("termina").is_none());
        assert!(ADMITE.contains(&"termina"));
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
            tras: None,
            inicio: None,
            retraso_min: None,
            a_mano: false,
            termina: None,
            ..Default::default()
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
        // 15v: las etiquetas de sus versiones pasan al plan; las que no valen, no.
        let con = Copia { etiquetas: vec!["bases".into(), "binarios".into()], ..k.clone() };
        assert_eq!(plan_de(&con).unwrap().tags, vec!["bases", "binarios"]);
        assert!(plan_de(&Copia { etiquetas: vec!["con espacio".into()], ..k.clone() }).is_err());
        assert!(plan_de(&Copia { etiquetas: (0..11).map(|i| format!("e{i}")).collect(), ..k.clone() }).is_err());
        // Sin etiquetas, el campo no va (un agente anterior no ve nada nuevo).
        assert!(serde_json::to_value(&k).unwrap().get("etiquetas").is_none());
        let mala = Copia { horario: Horario { dias: vec![1], horas: vec!["25:00".into()], reglas: vec![] }, ..k };
        assert!(plan_de(&mala).is_err());
    }

    /// 0.8.1 (15y): grupos de exclusiones, tamaño máximo y sus excepciones.
    #[test]
    fn exclusiones_de_una_copia() {
        let k: Copia = serde_json::from_value(json!({"id": "k", "nombre": "K", "repo": "r1", "carpetas": ["/d"], "horario": {"dias": [1], "horas": ["13:00"]},
            "grupos_exclusiones": [{"id": "musica", "nombre": "Música", "reglas": ["*.mp3"]}], "mayores_mb": 500, "excepto_tamano": ["*.pst"]}))
        .unwrap();
        assert!(k.valida_exclusiones().is_ok());
        assert_eq!(k.extra(), crate::agent::PlanExtra { larger_mb: Some(500), larger_except: vec!["*.pst".into()], on_reconnect: false, on_finish: None });
        // Sin límite, las excepciones no cuentan; y un plan sin nada nuevo no lleva `extra`.
        assert!(Copia { mayores_mb: None, ..k.clone() }.extra().vacio());
        assert!(Copia { mayores_mb: Some(0), ..k.clone() }.valida_exclusiones().is_err());
        assert!(Copia { excepto_tamano: vec!["".into()], ..k.clone() }.valida_exclusiones().is_err());
        let mal = GrupoUsado { id: "no vale".into(), ..Default::default() };
        assert!(Copia { grupos_exclusiones: vec![mal], ..k.clone() }.valida_exclusiones().is_err());
        // Un agente anterior no ve nada nuevo si no se usa.
        let sin = serde_json::to_value(Copia { grupos_exclusiones: vec![], mayores_mb: None, excepto_tamano: vec![], ..k }).unwrap();
        assert!(sin.get("grupos_exclusiones").is_none() && sin.get("mayores_mb").is_none() && sin.get("excepto_tamano").is_none());
        assert!(ADMITE.contains(&"grupos_en_copias"));
    }

    /// 0.8.1 (15kk): al terminar, solo «apagar» o «hibernar»; en el plan, `on_finish`.
    #[test]
    fn apagar_al_terminar() {
        assert!(valida_al_terminar("reiniciar", true).is_err());
        assert!(valida_al_terminar("", true).is_err());
        let k: Copia = serde_json::from_value(
            json!({"id": "k", "nombre": "K", "repo": "r1", "carpetas": ["/d"], "horario": {"dias": [1], "horas": ["13:00"]}, "al_terminar": "hibernar"}),
        )
        .unwrap();
        assert_eq!(k.extra().on_finish.as_deref(), Some("hibernar"));
        assert!(ADMITE.contains(&"apagar_al_terminar"));
    }

    /// 0.8.1 (15t): el nombre de una copia derivada y lo que no vale.
    #[test]
    fn nombre_de_una_copia() {
        let mut v = Vinculo::default();
        v.repos_v2.push(RepoV2 { id: "r1".into(), nombre: "Docs".into(), derivadas: vec![json!({ "id": "d1", "destino": "x" })], ..Default::default() });
        assert!(nombre_copia(&mut v, &json!({ "copia": "k", "nombre": "  " })).unwrap_err().contains("Escribe"));
        assert!(nombre_copia(&mut v, &json!({ "copia": "k", "nombre": "x".repeat(61) })).is_err());
        assert!(nombre_copia(&mut v, &json!({ "copia": "k", "nombre": "Nueva" })).unwrap_err().contains("aún no tiene copias"));
        let m = nombre_copia(&mut v, &json!({ "derivada": { "repo": "r1", "id": "d1" }, "nombre": " Docs a la nube " })).unwrap();
        assert!(m.contains("«Docs (copia derivada)» ahora se llama «Docs a la nube»"), "{m}");
        assert_eq!(v.repos_v2[0].derivadas[0]["nombre"], "Docs a la nube");
        assert!(nombre_copia(&mut v, &json!({ "derivada": { "repo": "r1", "id": "d1" }, "nombre": "Docs a la nube" })).unwrap().contains("ya se llama"));
        assert!(nombre_copia(&mut v, &json!({ "derivada": { "repo": "r1", "id": "otra" }, "nombre": "X" })).is_err());
        // El resumen de la derivada lleva su nombre.
        assert_eq!(derivada_resumen(&v, "r1", &v.repos_v2[0].derivadas[0], None)["nombre"], "Docs a la nube");
        // Una copia que no está.
        v.config_v1 = Some(json!({ "v": 1, "copias": [] }));
        assert!(nombre_copia(&mut v, &json!({ "copia": "k", "nombre": "Nueva" })).unwrap_err().contains("ya no está"));
        assert!(ADMITE.contains(&"nombre_copia") && ADMITE.contains(&"etiquetas_copia"));
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
            tras: None,
            inicio: None,
            retraso_min: None,
            a_mano: false,
            termina: None,
            ..Default::default()
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

        // Tarea 4b: «Probar» una copia derivada: lo mismo, con su id y su carpeta por defecto
        // (`<repo>-<id>`, para no chocar con la externa en el mismo destino).
        let derivada = |id: &str, destino: &str| json!({ "repo": "siigo", "id": id, "solo_probar": true, "tras_copia": true, "destino": { "id": destino } });
        let m = cambiar_derivada(&mut v, &derivada("nube-2", "nube-ex"), "siigo").unwrap();
        assert!(m.contains("se creará al guardar"), "{m}");
        assert!(!b.join("nube").join("siigo-nube-2").exists(), "probar no crea nada");
        for (c, error) in
            [(derivada("externa", "nube-ex"), "no válido"), (derivada("Mal Id", "nube-ex"), "no válido"), (derivada("d1", "almacen"), "otro destino")]
        {
            let e = cambiar_derivada(&mut v, &c, "siigo").unwrap_err();
            assert!(e.contains(error), "{error}: {e}");
        }
        // A la misma carpeta que la externa (o que otra derivada) del mismo destino, no.
        let mut choca = derivada("d1", "nube-ex");
        choca["ruta"] = json!("copias/siigo");
        choca["existente"] = json!(true);
        choca["contrasena_destino"] = json!("clave de la nube");
        assert!(cambiar_derivada(&mut v, &choca, "siigo").unwrap_err().contains("ya va a esa carpeta"));
        v.repos_v2[0].derivadas = vec![json!({ "id": "d2", "destino": "nube-ex" })];
        assert!(cambiar_derivada(&mut v, &derivada("d3", "nube-ex"), "siigo").is_ok(), "otra carpeta por defecto");
        let mut misma = derivada("d3", "nube-ex");
        misma["ruta"] = json!("siigo-d2");
        assert!(cambiar_derivada(&mut v, &misma, "siigo").unwrap_err().contains("ya va a esa carpeta"));
        // Qué reduce la protección: cambiar la retención o el destino de una que ya existe.
        let con = |extra: Value| {
            let mut c = json!({ "id": "d2", "destino": { "id": "nube-ex" }, "hora": "21:00" });
            c.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            c
        };
        assert!(!derivada_reduce(&v, &con(json!({})), "siigo"), "cambiar la hora no");
        assert!(derivada_reduce(&v, &con(json!({ "retencion": { "diarias": 7 } })), "siigo"));
        assert!(derivada_reduce(&v, &con(json!({ "destino": { "id": "otro" } })), "siigo"));
        assert!(!derivada_reduce(&v, &json!({ "id": "nueva", "destino": { "id": "x" }, "retencion": { "diarias": 7 } }), "siigo"), "una nueva no");
        let _ = std::fs::remove_dir_all(&b);
    }

    /// Tarea 4b: cuándo se hace una copia derivada y lo que va al resumen (sin rutas).
    #[test]
    fn cuando_y_resumen_de_una_derivada() {
        use crate::agent::Schedule;
        assert_eq!(cuando_derivada(&json!({ "tras_copia": true })).unwrap().0, Schedule::AfterBackup { min_minutes: 0 });
        assert_eq!(cuando_derivada(&json!({ "tras_copia": true, "min_minutos": 30 })).unwrap().1, json!({ "tras_copia": true, "min_minutos": 30 }));
        assert_eq!(cuando_derivada(&json!({ "hora": "23:00" })).unwrap().0, Schedule::Daily { time: "23:00".into() });
        let (s, cuando) = cuando_derivada(&json!({ "horario": { "reglas": [{ "tipo": "mensual", "dia": 1, "hora": "03:00" }] } })).unwrap();
        assert!(matches!(s, Schedule::Rules { ref rules } if rules.len() == 1));
        assert_eq!(cuando["horario"]["reglas"][0]["dia"], 1);
        for mal in [
            json!({}),
            json!({ "hora": "25:00" }),
            json!({ "tras_copia": true, "min_minutos": 9999 }),
            json!({ "horario": { "dias": [9], "horas": ["01:00"] } }),
        ] {
            assert!(cuando_derivada(&mal).is_err(), "{mal}");
        }
        let mut v = Vinculo::default();
        v.destinos.push(Destino { id: "b2".into(), nombre: "B2 Sur".into(), tipo: "b2".into(), donde: "copias-sur".into(), ..Default::default() });
        let e = json!({ "id": "d1", "destino": "b2", "ruta": "privada/siigo", "cuando": { "tras_copia": true },
                        "filtro": { "carpetas": [r"C:\Datos\Contabilidad"], "etiquetas": ["diaria"] } });
        let r = derivada_resumen(&v, "siigo", &e, None);
        assert_eq!((r["destino"].as_str(), r["filtro"]["carpetas"].as_u64(), r["activa"].as_bool()), (Some("B2 Sur"), Some(1), Some(false)));
        assert!(!r.to_string().contains("Contabilidad") && !r.to_string().contains("privada"), "sin rutas: {r}");
        assert!(id_derivada_valido("nube-2") && !id_derivada_valido("externa") && !id_derivada_valido("") && !id_derivada_valido("a/b"));
        assert!(ADMITE.contains(&"derivadas") && ADMITE.contains(&"filtros"));
    }

    /// Tarea 4a: un destino «nube» (conectada en el equipo) va por rclone.
    #[test]
    fn ubicacion_en_una_nube_del_equipo() {
        let d = |donde: &str, nube: Option<&str>| Destino {
            id: "n".into(),
            nombre: "Dropbox".into(),
            tipo: "nube".into(),
            donde: donde.into(),
            nube: nube.map(String::from),
            ..Default::default()
        };
        assert_eq!(ubicacion(&d("Resguardo/Sur", Some("Dropbox Sur")), "siigo-d1").unwrap(), "rclone:rnube:Resguardo/Sur/siigo-d1");
        assert_eq!(ubicacion(&d("", Some("Dropbox Sur")), "siigo-d1").unwrap(), "rclone:rnube:siigo-d1");
        for (donde, nube) in [("../x", Some("D")), ("a:b", Some("D")), ("Resguardo", None), ("Resguardo", Some(" "))] {
            assert!(ubicacion(&d(donde, nube), "r").is_err(), "{donde:?} {nube:?}");
        }
        assert!(ubicacion(&d("Resguardo", Some("D")), "").is_err());
        // Copiar las carpetas directamente a una nube: solo si está conectada en el equipo.
        let mut v = Vinculo::default();
        let c = json!({ "id": "r1", "nombre": "Docs", "contrasena": "una clave larga", "destino": { "id": "n1", "tipo": "nube", "nube": "Dropbox que no hay", "nombre": "Dropbox" } });
        assert!(crear_repositorio(&mut v, &c).unwrap_err().contains("no está conectada en este equipo"));
        assert!(v.destinos.is_empty() && v.repos_v2.is_empty());
        assert!(ADMITE.contains(&"nube_equipo") && ADMITE.contains(&"repo_en_nube"));
    }

    /// Plan 0.8.0 (B3, 14hh): dar acceso a una zona de un almacén sin crear repositorio.
    #[test]
    fn solo_el_acceso_a_un_almacen() {
        let mut v = Vinculo::default();
        let ca = "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----\n";
        let pedir = |dest: serde_json::Value| json!({ "solo_destino": true, "destino": dest });
        let bien = json!({ "id": "almacen-0b5c1f8e", "nombre": "ALMACEN-01", "tipo": "rest", "donde": "https://192.0.2.10:8000/pc-01/",
            "usuario": "pc-01", "secreto": "una-clave", "ca_pem": ca, "equipo_almacen": "0b5c1f8e" });
        let m = crear_repositorio(&mut v, &pedir(bien.clone())).unwrap();
        assert!(m.contains("sin crear ningún repositorio"), "{m}");
        assert!(v.repos_v2.is_empty(), "ningún repositorio");
        let d = &v.destinos[0];
        assert_eq!((d.id.as_str(), d.usuario.as_deref(), d.equipo_almacen.as_deref()), ("almacen-0b5c1f8e", Some("pc-01"), Some("0b5c1f8e")));
        assert!(d.ca_pem.is_some());
        // Otra vez (el almacén dio otra contraseña): se pone al día, no se duplica.
        let mut otra = bien.clone();
        otra["secreto"] = json!("otra-clave");
        assert!(crear_repositorio(&mut v, &pedir(otra)).unwrap().contains("al día"));
        assert_eq!((v.destinos.len(), v.destinos[0].secreto.as_deref()), (1, Some("otra-clave")));
        // Solo un almacén (rest con su equipo, https, credenciales y autoridad).
        for (campo, valor) in [
            ("equipo_almacen", json!(null)),
            ("tipo", json!("b2")),
            ("donde", json!("http://192.0.2.10:8000/")),
            ("ca_pem", json!(null)),
            ("secreto", json!("")),
        ] {
            let mut mala = bien.clone();
            mala["id"] = json!("almacen-otro");
            mala[campo] = valor;
            assert!(crear_repositorio(&mut v, &pedir(mala)).is_err(), "{campo}");
        }
        // Un destino con ese id que no es de ese almacén: no se toca.
        v.destinos.push(Destino { id: "d-disco".into(), nombre: "Disco".into(), tipo: "local".into(), donde: r"E:\Copias".into(), ..Default::default() });
        let mut choca = bien.clone();
        choca["id"] = json!("d-disco");
        assert!(crear_repositorio(&mut v, &pedir(choca)).unwrap_err().contains("no es de ese almacén"));
        assert_eq!(v.destinos[1].tipo, "local");
        assert!(ADMITE.contains(&"solo_destino"));
    }

    /// Tarea 4a completa: un repositorio directamente en una nube conectada en el
    /// equipo, con el restic y el rclone de verdad (la «nube» es una carpeta por
    /// rclone, tipo `alias`, solo en las pruebas): crear, copiar y restaurar.
    #[test]
    fn repositorio_directo_en_una_nube_conectada() {
        use resguardo_motor::restic::{run_raw, snapshots};
        let _l = crate::restic::tests::real_repo_lock();
        let Ok(rclone) = crate::nube::comprobar_binario() else {
            eprintln!("Sin rclone en src-tauri/binaries: se salta la prueba.");
            return;
        };
        if resguardo_motor::restic::version().is_err() {
            eprintln!("Sin restic: se salta la prueba.");
            return;
        }
        let b = std::env::temp_dir().join(format!("resguardo-repo-nube-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&b);
        std::env::set_var("RESGUARDO_AGENT_DIR", b.join("agente"));
        // restic busca «rclone» en el PATH (en las pruebas no hay uno junto al ejecutable).
        std::fs::create_dir_all(b.join("bin")).unwrap();
        std::fs::copy(&rclone, b.join("bin").join(if cfg!(windows) { "rclone.exe" } else { "rclone" })).unwrap();
        let path_antes = std::env::var_os("PATH").unwrap_or_default();
        std::env::set_var("PATH", std::env::join_paths(std::iter::once(b.join("bin")).chain(std::env::split_paths(&path_antes))).unwrap());
        crate::nube::registrar();
        let nube = b.join("nube");
        std::fs::create_dir_all(&nube).unwrap();
        crate::nube::conectar_rclone(&json!({ "tipo": "alias", "nombre": "Nube de prueba", "parametros": { "carpeta": nube.display().to_string() } })).unwrap();

        let mut v = Vinculo { equipo_id: uuid::Uuid::new_v4().to_string(), modo: "local".into(), ..Default::default() };
        let pedir = |n: &str| {
            json!({ "id": "docs", "nombre": "Documentos", "contrasena": "clave-de-prueba-larga",
                    "destino": { "id": "nube-prueba", "nombre": "Dropbox Oficina", "tipo": "nube", "nube": n, "donde": "Resguardo/Sur" } })
        };
        assert!(crear_repositorio(&mut v, &pedir("Otra")).unwrap_err().contains("no está conectada"));
        let m = crear_repositorio(&mut v, &pedir("Nube de prueba")).unwrap();
        assert!(m.contains("creado"), "{m}");
        assert!(nube.join("Resguardo").join("Sur").join("docs").join("config").is_file(), "el repositorio está en la nube");
        assert_eq!((v.destinos[0].tipo.as_str(), v.destinos[0].nube.as_deref()), ("nube", Some("Nube de prueba")));
        // Con la copia se guarda solo la marca de la nube (nada de credenciales).
        let acc = acceso(&v, "docs").unwrap();
        assert_eq!(acc.location, "rclone:rnube:Resguardo/Sur/docs");
        assert_eq!(acc.env, vec![(crate::nube::MARCA.to_string(), "Nube de prueba".to_string())]);
        // Copiar y restaurar, como lo hará cada vuelta del agente.
        std::fs::create_dir_all(b.join("datos")).unwrap();
        std::fs::write(b.join("datos").join("factura.txt"), "factura de prueba").unwrap();
        let out = run_raw(&acc, &["backup", "--json", &b.join("datos").display().to_string()], std::time::Duration::from_secs(300)).unwrap();
        assert_eq!(out.code, Some(0), "{}", out.stderr);
        assert_eq!(snapshots(&acc).unwrap().len(), 1);
        let out = run_raw(&acc, &["restore", "latest", "--target", &b.join("restaurado").display().to_string()], std::time::Duration::from_secs(300)).unwrap();
        assert_eq!(out.code, Some(0), "{}", out.stderr);
        fn buscar(d: &std::path::Path) -> Option<std::path::PathBuf> {
            std::fs::read_dir(d).ok()?.flatten().find_map(|e| {
                if e.path().is_dir() {
                    buscar(&e.path())
                } else {
                    (e.file_name() == "factura.txt").then(|| e.path())
                }
            })
        }
        assert_eq!(std::fs::read_to_string(buscar(&b.join("restaurado")).unwrap()).unwrap(), "factura de prueba");
        // Ni un archivo de rclone se queda en la carpeta privada.
        let vueltas = crate::agent::private_dir().join("rclone-vueltas");
        assert_eq!(std::fs::read_dir(&vueltas).map(|d| d.count()).unwrap_or(0), 0);
        // Mientras un repositorio está en ella, la nube no se puede desconectar.
        crate::servidor_v2::guardar(&v).unwrap();
        assert!(crate::nube::quitar_desde_orden(&json!({ "nombre": "Nube de prueba" })).unwrap_err().contains("la usa el repositorio"));
        std::env::set_var("PATH", path_antes);
        std::env::remove_var("RESGUARDO_AGENT_DIR");
        let _ = std::fs::remove_dir_all(&b);
    }
}
