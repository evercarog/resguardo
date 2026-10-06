//! Publicaciones firmadas del agente (docs/actualizaciones.md): el manifiesto
//! de cada versión, su firma minisign (Ed25519, llave fuera de línea), las
//! llaves fijadas al compilar, la comparación de versiones, la política de
//! cada consola (anillo, modo, días, ventana) y la decisión «¿toca
//! actualizar?».
//!
//! Lo usan el agente (que solo instala lo firmado) y el servidor (que solo
//! sirve lo firmado). La comprobación de la firma la hace el crate
//! `minisign-verify` (MIT, sin dependencias, del autor de minisign).
//!
//! Todo lo que llega de fuera (manifiesto, firma, política) se comprueba aquí
//! y nunca provoca un `panic`.

use base64::Engine;
use chrono::{DateTime, Duration, FixedOffset, NaiveTime, TimeZone, Timelike};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// Versión del formato del manifiesto.
pub const FORMATO: u32 = 1;
/// El producto de los manifiestos del agente (un manifiesto de otro producto no vale aquí).
pub const PRODUCTO_AGENTE: &str = "resguardo-agente";
/// Tamaño máximo del manifiesto y de su firma.
pub const MAX_MANIFIESTO: usize = 64 * 1024;
pub const MAX_FIRMA: usize = 4096;
/// Tamaño máximo de cada archivo de una publicación.
pub const MAX_TAMANO: u64 = 512 * 1024 * 1024;
pub const MAX_NOMBRE: usize = 128;
const MAX_URL: usize = 2048;
const MAX_NOTAS: usize = 4000;
/// Marca del archivo de llaves sin ninguna llave (`packaging/llave-publicacion.pub`).
pub const MARCA_SIN_LLAVE: &str = "PENDIENTE-SIN-LLAVE";
/// Nombres de los archivos del manifiesto y su firma (en GitHub y en el servidor).
pub const NOMBRE_MANIFIESTO: &str = "manifiesto-agente.json";
pub const NOMBRE_FIRMA: &str = "manifiesto-agente.json.minisig";
/// Las plataformas que se publican.
pub const PLATAFORMAS: &[&str] = &["windows-x86_64", "linux-x86_64", "linux-aarch64"];
/// Días que espera el anillo «general» si la consola no dice otra cosa, y el máximo.
pub const DIAS_GENERAL: u32 = 2;
pub const MAX_DIAS_GENERAL: u32 = 30;

/// El tipo de archivo de cada plataforma.
pub fn tipo_de(plataforma: &str) -> Option<&'static str> {
    match plataforma {
        "windows-x86_64" => Some("instalador-nsis"),
        "linux-x86_64" | "linux-aarch64" => Some("tar.gz"),
        _ => None,
    }
}

/// La plataforma de este programa (`None` si no se publica para ella).
pub fn plataforma_actual() -> Option<&'static str> {
    if cfg!(all(windows, target_arch = "x86_64")) {
        Some("windows-x86_64")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("linux-x86_64")
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        Some("linux-aarch64")
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Llaves
// ---------------------------------------------------------------------------

/// El identificador de una llave de minisign («key id»), como lo escribe
/// minisign: los 8 bytes como un `u64` en little-endian, en hexadecimal.
fn id_hex(bytes: &[u8]) -> String {
    let mut b = [0u8; 8];
    b.copy_from_slice(&bytes[..8]);
    format!("{:016X}", u64::from_le_bytes(b))
}

/// Una llave pública de publicación.
#[derive(Clone)]
pub struct Llave {
    pub id: String,
    clave: minisign_verify::PublicKey,
}

/// Las llaves en las que se confía (las de `packaging/llave-publicacion.pub`).
#[derive(Clone, Default)]
pub struct Llaves {
    lista: Vec<Llave>,
}

impl Llaves {
    /// Lee un archivo de llaves: una o varias llaves públicas de minisign. Se
    /// ignoran las líneas vacías, las que empiezan por `#` y las
    /// `untrusted comment:`. Cualquier otra línea tiene que ser una llave.
    pub fn leer(texto: &str) -> Result<Self, String> {
        let mut lista: Vec<Llave> = Vec::new();
        for (n, linea) in texto.lines().enumerate() {
            let l = linea.trim();
            if l.is_empty() || l.starts_with('#') || l.starts_with("untrusted comment:") {
                continue;
            }
            let mala = || format!("Línea {} del archivo de llaves: no es una llave pública de minisign.", n + 1);
            let bin = B64.decode(l).map_err(|_| mala())?;
            if bin.len() != 42 || &bin[..2] != b"Ed" {
                return Err(mala());
            }
            let clave = minisign_verify::PublicKey::from_base64(l).map_err(|_| mala())?;
            let id = id_hex(&bin[2..10]);
            if !lista.iter().any(|k| k.id == id) {
                lista.push(Llave { id, clave });
            }
        }
        Ok(Llaves { lista })
    }

    pub fn vacia(&self) -> bool {
        self.lista.is_empty()
    }

    pub fn len(&self) -> usize {
        self.lista.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lista.is_empty()
    }

    pub fn ids(&self) -> Vec<String> {
        self.lista.iter().map(|k| k.id.clone()).collect()
    }

    /// Añade las de `otras` (sin repetir).
    pub fn anadir(&mut self, otras: Llaves) {
        for k in otras.lista {
            if !self.lista.iter().any(|x| x.id == k.id) {
                self.lista.push(k);
            }
        }
    }
}

/// ¿Es el archivo de llaves el marcador de posición (sin ninguna llave)?
pub fn es_marcador(texto: &str) -> bool {
    texto.contains(MARCA_SIN_LLAVE) && Llaves::leer(texto).is_ok_and(|l| l.vacia())
}

// ---------------------------------------------------------------------------
// Firma
// ---------------------------------------------------------------------------

/// El identificador de la llave con la que se hizo una firma de minisign.
pub fn id_de_firma(firma: &str) -> Option<String> {
    let linea = firma.lines().nth(1)?.trim();
    let bin = B64.decode(linea).ok()?;
    (bin.len() == 74).then(|| id_hex(&bin[2..10]))
}

/// Comprueba la firma minisign de `datos` con una de las `llaves` que no
/// esté en `revocadas`. Devuelve el identificador de la llave que firmó.
/// Solo firmas «prehash» (minisign ≥ 0.10); las heredadas no valen.
pub fn comprobar_firma(datos: &[u8], firma: &str, llaves: &Llaves, revocadas: &[String]) -> Result<String, String> {
    if firma.len() > MAX_FIRMA {
        return Err("La firma es demasiado grande.".into());
    }
    let sig = minisign_verify::Signature::decode(firma).map_err(|_| "La firma no tiene el formato de minisign.".to_string())?;
    let id = id_de_firma(firma).ok_or("La firma no tiene el formato de minisign.")?;
    if revocadas.iter().any(|r| r.eq_ignore_ascii_case(&id)) {
        return Err(format!("La firma es de una llave revocada ({id})."));
    }
    let Some(llave) = llaves.lista.iter().find(|k| k.id == id) else {
        return Err(format!("La firma es de una llave que este programa no conoce ({id})."));
    };
    llave.clave.verify(datos, &sig, false).map_err(|e| match e {
        minisign_verify::Error::UnexpectedAlgorithm => "La firma es del formato antiguo de minisign (sin prehash): no se acepta.".to_string(),
        _ => "La firma no es válida: el manifiesto no es el que se firmó.".to_string(),
    })?;
    Ok(id)
}

// ---------------------------------------------------------------------------
// Versiones
// ---------------------------------------------------------------------------

/// Una versión `MAYOR.MENOR.PARCHE[-preversión]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub numeros: [u64; 3],
    pub pre: Option<String>,
}

impl Version {
    pub fn leer(s: &str) -> Option<Self> {
        if s.is_empty() || s.len() > 64 {
            return None;
        }
        let (nums, pre) = match s.split_once('-') {
            Some((n, p)) => (n, Some(p)),
            None => (s, None),
        };
        let partes: Vec<&str> = nums.split('.').collect();
        if partes.len() != 3 {
            return None;
        }
        let mut numeros = [0u64; 3];
        for (i, p) in partes.iter().enumerate() {
            if p.is_empty() || p.len() > 9 || !p.bytes().all(|b| b.is_ascii_digit()) || (p.len() > 1 && p.starts_with('0')) {
                return None;
            }
            numeros[i] = p.parse().ok()?;
        }
        if let Some(p) = pre {
            if p.is_empty() || p.len() > 32 || !p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-') || p.split('.').any(str::is_empty) {
                return None;
            }
        }
        Some(Version { numeros, pre: pre.map(str::to_string) })
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [a, b, c] = self.numeros;
        match &self.pre {
            Some(p) => write!(f, "{a}.{b}.{c}-{p}"),
            None => write!(f, "{a}.{b}.{c}"),
        }
    }
}

fn comparar_pre(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.split('.'), b.split('.'));
    loop {
        match (x.next(), y.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(p), Some(q)) => {
                let o = match (p.parse::<u64>(), q.parse::<u64>()) {
                    (Ok(n), Ok(m)) => n.cmp(&m),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => p.cmp(q),
                };
                if o != Ordering::Equal {
                    return o;
                }
            }
        }
    }
}

impl Ord for Version {
    fn cmp(&self, otra: &Self) -> Ordering {
        self.numeros.cmp(&otra.numeros).then_with(|| match (&self.pre, &otra.pre) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(a), Some(b)) => comparar_pre(a, b),
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, otra: &Self) -> Option<Ordering> {
        Some(self.cmp(otra))
    }
}

/// Compara dos versiones escritas (`None` si alguna no es válida).
pub fn comparar(a: &str, b: &str) -> Option<Ordering> {
    Some(Version::leer(a)?.cmp(&Version::leer(b)?))
}

// ---------------------------------------------------------------------------
// Manifiesto
// ---------------------------------------------------------------------------

/// Un archivo de una publicación.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Archivo {
    pub plataforma: String,
    pub tipo: String,
    pub nombre: String,
    pub sha256: String,
    pub tamano: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// El manifiesto de una versión (docs/actualizaciones.md §2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifiesto {
    pub formato: u32,
    pub producto: String,
    pub version: String,
    pub fecha: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimo_desde: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notas: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub revocadas: Vec<String>,
    pub archivos: Vec<Archivo>,
}

/// ¿Es un nombre de archivo seguro (sin carpetas, sin empezar por punto)?
pub fn nombre_valido(n: &str) -> bool {
    !n.is_empty() && n.len() <= MAX_NOMBRE && !n.starts_with('.') && n.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn id_llave_valido(s: &str) -> bool {
    s.len() == 16 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

impl Manifiesto {
    /// Lee y comprueba un manifiesto (sin la firma: eso es [`verificar`]).
    pub fn leer(datos: &[u8]) -> Result<Self, String> {
        if datos.len() > MAX_MANIFIESTO {
            return Err("El manifiesto es demasiado grande.".into());
        }
        let m: Manifiesto = serde_json::from_slice(datos).map_err(|e| format!("El manifiesto no es válido: {e}"))?;
        m.validar()?;
        Ok(m)
    }

    /// Las reglas de docs/actualizaciones.md §2.
    pub fn validar(&self) -> Result<(), String> {
        if self.formato != FORMATO {
            return Err(format!("Formato de manifiesto desconocido ({}).", self.formato));
        }
        if self.producto.is_empty() || self.producto.len() > 64 {
            return Err("Producto no válido.".into());
        }
        if Version::leer(&self.version).is_none() {
            return Err("La versión del manifiesto no es válida.".into());
        }
        if DateTime::parse_from_rfc3339(&self.fecha).is_err() {
            return Err("La fecha del manifiesto no es válida.".into());
        }
        if self.canal.as_deref().is_some_and(|c| c != "estable") {
            return Err("Canal desconocido.".into());
        }
        if self.minimo_desde.as_deref().is_some_and(|v| Version::leer(v).is_none()) {
            return Err("«minimo_desde» no es una versión válida.".into());
        }
        if self.notas.as_ref().is_some_and(|n| n.len() > MAX_NOTAS) {
            return Err("Las notas son demasiado largas.".into());
        }
        if self.revocadas.len() > 64 || !self.revocadas.iter().all(|r| id_llave_valido(r)) {
            return Err("Lista de llaves revocadas no válida.".into());
        }
        if self.archivos.is_empty() || self.archivos.len() > PLATAFORMAS.len() {
            return Err("El manifiesto no tiene archivos (o tiene demasiados).".into());
        }
        for (i, a) in self.archivos.iter().enumerate() {
            if !PLATAFORMAS.contains(&a.plataforma.as_str()) {
                return Err(format!("Plataforma desconocida: {}.", a.plataforma.chars().take(40).collect::<String>()));
            }
            if self.archivos[..i].iter().any(|b| b.plataforma == a.plataforma) {
                return Err(format!("Dos archivos para {}.", a.plataforma));
            }
            if tipo_de(&a.plataforma) != Some(a.tipo.as_str()) {
                return Err(format!("Tipo de archivo no válido para {}.", a.plataforma));
            }
            if !nombre_valido(&a.nombre) {
                return Err("Nombre de archivo no válido.".into());
            }
            if !hex64(&a.sha256) {
                return Err(format!("SHA-256 no válido para {}.", a.nombre));
            }
            if a.tamano == 0 || a.tamano > MAX_TAMANO {
                return Err(format!("Tamaño no válido para {}.", a.nombre));
            }
            if let Some(u) = &a.url {
                if !u.starts_with("https://") || u.len() > MAX_URL || u.chars().any(|c| c.is_whitespace() || c.is_control()) {
                    return Err(format!("La dirección de {} no es https://.", a.nombre));
                }
            }
        }
        Ok(())
    }

    /// El archivo de una plataforma.
    pub fn archivo(&self, plataforma: &str) -> Option<&Archivo> {
        self.archivos.iter().find(|a| a.plataforma == plataforma)
    }

    /// El archivo con ese nombre.
    pub fn archivo_por_nombre(&self, nombre: &str) -> Option<&Archivo> {
        self.archivos.iter().find(|a| a.nombre == nombre)
    }

    /// La versión (ya comprobada al leer).
    pub fn version_leida(&self) -> Option<Version> {
        Version::leer(&self.version)
    }
}

/// Un manifiesto con la firma comprobada.
#[derive(Clone, Debug)]
pub struct Verificado {
    pub manifiesto: Manifiesto,
    /// La llave que lo firmó.
    pub llave: String,
}

/// Comprueba la firma y el contenido de un manifiesto de `producto`.
pub fn verificar(datos: &[u8], firma: &str, llaves: &Llaves, revocadas: &[String], producto: &str) -> Result<Verificado, String> {
    if datos.len() > MAX_MANIFIESTO {
        return Err("El manifiesto es demasiado grande.".into());
    }
    if llaves.vacia() {
        return Err("Este programa no tiene llave de publicación: no acepta actualizaciones.".into());
    }
    let llave = comprobar_firma(datos, firma, llaves, revocadas)?;
    let manifiesto = Manifiesto::leer(datos)?;
    if manifiesto.producto != producto {
        return Err(format!("El manifiesto es de otro producto ({}).", manifiesto.producto.chars().take(40).collect::<String>()));
    }
    if manifiesto.revocadas.iter().any(|r| r.eq_ignore_ascii_case(&llave)) {
        return Err("El manifiesto revoca la llave con la que está firmado.".into());
    }
    Ok(Verificado { manifiesto, llave })
}

// ---------------------------------------------------------------------------
// Política de cada consola y su combinación
// ---------------------------------------------------------------------------

/// Una ventana de mantenimiento en la hora del equipo (`HH:MM`–`HH:MM`; si
/// `hasta` es antes que `desde`, cruza la medianoche; iguales, todo el día).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ventana {
    pub desde: String,
    pub hasta: String,
}

fn hora_min(s: &str) -> Option<u32> {
    let (h, m) = s.split_once(':')?;
    if h.len() != 2 || m.len() != 2 {
        return None;
    }
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

impl Ventana {
    fn tramo(&self) -> Option<(u32, u32)> {
        Some((hora_min(&self.desde)?, hora_min(&self.hasta)?))
    }

    pub fn valida(&self) -> bool {
        self.tramo().is_some()
    }

    /// ¿Está abierta a esa hora? Una ventana no válida nunca lo está.
    pub fn abierta(&self, t: NaiveTime) -> bool {
        let Some((d, h)) = self.tramo() else { return false };
        let m = t.hour() * 60 + t.minute();
        match d.cmp(&h) {
            Ordering::Equal => true,
            Ordering::Less => d <= m && m < h,
            Ordering::Greater => m >= d || m < h,
        }
    }
}

fn modo_auto() -> String {
    "auto".into()
}

fn anillo_general() -> String {
    "general".into()
}

fn dias_por_defecto() -> u32 {
    DIAS_GENERAL
}

/// Lo que dice una consola para un equipo (`GET /api/agente/actualizacion`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Politica {
    /// `auto`, `manual` o `pausada`.
    #[serde(default = "modo_auto")]
    pub modo: String,
    /// `prueba` o `general`.
    #[serde(default = "anillo_general")]
    pub anillo: String,
    #[serde(default = "dias_por_defecto")]
    pub dias_general: u32,
    #[serde(default)]
    pub ventana: Option<Ventana>,
    /// La versión aprobada con «Actualizar ahora».
    #[serde(default)]
    pub aprobada: Option<String>,
    /// Versiones que fallaron en algún equipo del cliente.
    #[serde(default)]
    pub retenidas: Vec<String>,
}

impl Default for Politica {
    fn default() -> Self {
        Politica { modo: modo_auto(), anillo: anillo_general(), dias_general: DIAS_GENERAL, ventana: None, aprobada: None, retenidas: Vec::new() }
    }
}

pub const MODOS: &[&str] = &["auto", "manual", "pausada"];
pub const ANILLOS: &[&str] = &["prueba", "general"];

impl Politica {
    /// Lo que llega de fuera, a valores conocidos y prudentes: un modo
    /// desconocido cuenta como `manual`, un anillo desconocido como `general`.
    pub fn normalizada(mut self) -> Self {
        if !MODOS.contains(&self.modo.as_str()) {
            self.modo = "manual".into();
        }
        if !ANILLOS.contains(&self.anillo.as_str()) {
            self.anillo = "general".into();
        }
        self.dias_general = self.dias_general.min(MAX_DIAS_GENERAL);
        self.aprobada = self.aprobada.filter(|v| Version::leer(v).is_some());
        self.retenidas.retain(|v| Version::leer(v).is_some());
        self.retenidas.truncate(64);
        self
    }
}

/// La política que vale para el equipo, combinando la de todas sus consolas
/// (docs/actualizaciones.md §5.2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Efectiva {
    pub modo: String,
    pub anillo: String,
    pub dias_general: u32,
    pub ventanas: Vec<Ventana>,
    pub aprobadas: Vec<String>,
    pub retenidas: Vec<String>,
}

/// Lo más prudente de todas: pausada > manual > auto; general si alguna lo
/// dice; los días, los más; todas las ventanas a la vez; las aprobaciones y
/// las retenidas, todas. Sin ninguna: automática, anillo general, 2 días.
pub fn combinar(politicas: &[Politica]) -> Efectiva {
    let ps: Vec<Politica> = politicas.iter().cloned().map(Politica::normalizada).collect();
    let peso = |m: &str| match m {
        "pausada" => 2,
        "manual" => 1,
        _ => 0,
    };
    let modo = ps.iter().map(|p| p.modo.as_str()).max_by_key(|m| peso(m)).unwrap_or("auto").to_string();
    let anillo = if ps.is_empty() || ps.iter().any(|p| p.anillo == "general") { "general" } else { "prueba" }.to_string();
    let dias_general = ps.iter().map(|p| p.dias_general).max().unwrap_or(DIAS_GENERAL);
    let mut ventanas: Vec<Ventana> = Vec::new();
    let mut aprobadas: Vec<String> = Vec::new();
    let mut retenidas: Vec<String> = Vec::new();
    for p in &ps {
        if let Some(v) = &p.ventana {
            if !ventanas.contains(v) {
                ventanas.push(v.clone());
            }
        }
        if let Some(a) = &p.aprobada {
            if !aprobadas.contains(a) {
                aprobadas.push(a.clone());
            }
        }
        for r in &p.retenidas {
            if !retenidas.contains(r) {
                retenidas.push(r.clone());
            }
        }
    }
    Efectiva { modo, anillo, dias_general, ventanas, aprobadas, retenidas }
}

// ---------------------------------------------------------------------------
// La decisión
// ---------------------------------------------------------------------------

/// Lo que hace falta para decidir.
pub struct Entrada<'a> {
    /// La versión instalada.
    pub actual: &'a str,
    pub manifiesto: &'a Manifiesto,
    pub plataforma: &'a str,
    pub politica: &'a Efectiva,
    /// La más tardía entre la fecha del manifiesto y la primera vez que este equipo lo vio.
    pub disponible_desde: DateTime<FixedOffset>,
    /// Ahora, con la zona horaria del equipo (para la ventana).
    pub ahora: DateTime<FixedOffset>,
    /// ¿Hay algo en marcha (copia, restauración…)?
    pub en_marcha: bool,
    /// Versiones que ya fallaron en este equipo.
    pub fallidas: &'a [String],
    /// ¿Es un almacén («Este equipo guarda copias»)?
    pub almacen: bool,
}

/// El resultado: `al_dia`, `instalar`, `pendiente` (con su motivo) o `pausada`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Decision {
    pub estado: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub motivo: Option<&'static str>,
    /// Hasta cuándo espera (si se sabe), RFC 3339.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hasta: Option<String>,
    /// Se instala porque una consola la aprobó.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub aprobada: bool,
}

impl Decision {
    fn de(estado: &'static str) -> Self {
        Decision { estado, motivo: None, hasta: None, aprobada: false }
    }

    fn pendiente(motivo: &'static str, hasta: Option<DateTime<FixedOffset>>) -> Self {
        Decision { estado: "pendiente", motivo: Some(motivo), hasta: hasta.map(|h| h.to_rfc3339()), aprobada: false }
    }

    pub fn instalar(&self) -> bool {
        self.estado == "instalar"
    }
}

/// ¿Están abiertas todas las ventanas a esa hora?
fn en_ventana(ventanas: &[Ventana], t: NaiveTime) -> bool {
    ventanas.iter().all(|v| v.abierta(t))
}

/// Cuándo se abren todas las ventanas a la vez (en las próximas 48 h).
fn proxima_apertura(ventanas: &[Ventana], ahora: DateTime<FixedOffset>) -> Option<DateTime<FixedOffset>> {
    let mut candidatos: Vec<DateTime<FixedOffset>> = Vec::new();
    for d in 0..=2 {
        let dia = ahora.date_naive() + Duration::days(d);
        for v in ventanas {
            let Some((desde, _)) = v.tramo() else { continue };
            let Some(hora) = NaiveTime::from_hms_opt(desde / 60, desde % 60, 0) else { continue };
            if let Some(t) = ahora.offset().from_local_datetime(&dia.and_time(hora)).single() {
                if t > ahora && t <= ahora + Duration::hours(48) {
                    candidatos.push(t);
                }
            }
        }
    }
    candidatos.sort();
    candidatos.into_iter().find(|t| en_ventana(ventanas, t.time()))
}

/// ¿Toca instalar la versión del manifiesto? (docs/actualizaciones.md §5.3)
pub fn decidir(e: &Entrada) -> Decision {
    let (Some(actual), Some(nueva)) = (Version::leer(e.actual), Version::leer(&e.manifiesto.version)) else {
        return Decision::pendiente("version_desconocida", None);
    };
    if nueva <= actual {
        return Decision::de("al_dia");
    }
    if e.manifiesto.archivo(e.plataforma).is_none() {
        return Decision::pendiente("sin_paquete", None);
    }
    if e.manifiesto.minimo_desde.as_deref().and_then(Version::leer).is_some_and(|m| actual < m) {
        return Decision::pendiente("necesita_intermedia", None);
    }
    let p = e.politica;
    if p.modo == "pausada" {
        return Decision::de("pausada");
    }
    let aprobada = p.aprobadas.iter().any(|a| Version::leer(a).as_ref() == Some(&nueva));
    if aprobada {
        if e.en_marcha {
            return Decision::pendiente("en_marcha", None);
        }
        return Decision { estado: "instalar", motivo: None, hasta: None, aprobada: true };
    }
    if p.modo != "auto" {
        return Decision::pendiente("espera_aprobacion", None);
    }
    let es = |v: &String| Version::leer(v).as_ref() == Some(&nueva);
    if p.retenidas.iter().any(es) || e.fallidas.iter().any(es) {
        return Decision::pendiente("retenida", None);
    }
    if p.anillo != "prueba" {
        let desde = e.disponible_desde + Duration::days(i64::from(p.dias_general.min(MAX_DIAS_GENERAL)));
        if e.ahora < desde {
            return Decision::pendiente("espera_anillo", Some(desde));
        }
    }
    if !en_ventana(&p.ventanas, e.ahora.time()) {
        return match proxima_apertura(&p.ventanas, e.ahora) {
            Some(t) => Decision::pendiente("espera_ventana", Some(t)),
            None => Decision::pendiente("ventanas_sin_coincidir", None),
        };
    }
    if e.en_marcha {
        return Decision::pendiente("en_marcha", None);
    }
    if e.almacen && p.ventanas.is_empty() {
        return Decision::pendiente("almacen_sin_ventana", None);
    }
    Decision::de("instalar")
}

// ---------------------------------------------------------------------------
// Firmar con las llaves de pruebas (solo pruebas)
// ---------------------------------------------------------------------------

/// Firmar manifiestos con las llaves de **pruebas** (`tests/fixtures`). Solo
/// existe en las pruebas y con la feature `pruebas` (las pruebas del agente y
/// del servidor): nada de esto entra en un programa publicado.
#[cfg(any(test, feature = "pruebas"))]
pub mod pruebas {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    /// Las llaves públicas de pruebas, en formato minisign.
    pub const LLAVE_A_PUB: &str = include_str!("../tests/fixtures/llave-pruebas-a.pub");
    pub const LLAVE_B_PUB: &str = include_str!("../tests/fixtures/llave-pruebas-b.pub");
    pub const ID_A: &str = "C57E2BA1129956D5";
    pub const ID_B: &str = "851E4472FB75946F";
    const PRIVADAS: &str = include_str!("../tests/fixtures/LLAVES-DE-PRUEBAS-LEEME.txt");

    fn semilla(id: &str) -> [u8; 32] {
        let hex = PRIVADAS
            .lines()
            .find_map(|l| l.strip_prefix(id).and_then(|r| r.strip_prefix('=')))
            .unwrap_or_else(|| panic!("falta la llave de pruebas {id}"))
            .trim();
        let mut s = [0u8; 32];
        for (i, b) in s.iter_mut().enumerate() {
            *b = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("semilla en hex");
        }
        s
    }

    /// Firma `datos` como `minisign -S` (prehash) con la llave de pruebas `id`.
    pub fn firmar(datos: &[u8], id: &str, comentario: &str) -> String {
        use blake2::Digest;
        let llave = SigningKey::from_bytes(&semilla(id));
        let numero = u64::from_str_radix(id, 16).expect("id en hex").to_le_bytes();
        let hash = blake2::Blake2b512::digest(datos);
        let firma = llave.sign(&hash).to_bytes();
        let mut global = firma.to_vec();
        global.extend_from_slice(comentario.as_bytes());
        let global = llave.sign(&global).to_bytes();
        let mut linea = b"ED".to_vec();
        linea.extend_from_slice(&numero);
        linea.extend_from_slice(&firma);
        format!("untrusted comment: signature from minisign secret key\n{}\ntrusted comment: {comentario}\n{}\n", B64.encode(linea), B64.encode(global))
    }

    pub fn firmar_a(datos: &[u8]) -> String {
        firmar(datos, ID_A, "timestamp:1760000000\tfile:manifiesto-agente.json")
    }

    pub fn firmar_b(datos: &[u8]) -> String {
        firmar(datos, ID_B, "timestamp:1760000000\tfile:manifiesto-agente.json")
    }

    pub fn llaves_a() -> Llaves {
        Llaves::leer(LLAVE_A_PUB).expect("llave A")
    }

    /// Un manifiesto de prueba con un archivo por plataforma (`nombre`, contenido).
    pub fn manifiesto(version: &str, archivos: &[(&str, &str, &[u8])]) -> Manifiesto {
        use sha2::Digest;
        Manifiesto {
            formato: FORMATO,
            producto: PRODUCTO_AGENTE.into(),
            version: version.into(),
            fecha: "2026-10-20T10:00:00Z".into(),
            canal: Some("estable".into()),
            minimo_desde: None,
            notas: None,
            revocadas: Vec::new(),
            archivos: archivos
                .iter()
                .map(|(plataforma, nombre, datos)| Archivo {
                    plataforma: plataforma.to_string(),
                    tipo: tipo_de(plataforma).unwrap_or("tar.gz").into(),
                    nombre: nombre.to_string(),
                    sha256: sha2::Sha256::digest(datos).iter().map(|b| format!("{b:02x}")).collect(),
                    tamano: datos.len() as u64,
                    url: None,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::pruebas::*;
    use super::*;
    use serde_json::Value;

    fn vectores() -> Value {
        serde_json::from_str(include_str!("../vectors/publicacion.json")).expect("vectors/publicacion.json")
    }

    fn manifiesto_bytes(v: &str) -> Vec<u8> {
        serde_json::to_vec_pretty(&manifiesto(v, &[("windows-x86_64", "Resguardo-Agente_setup.exe", b"exe"), ("linux-x86_64", "agente.tar.gz", b"tgz")]))
            .unwrap()
    }

    #[test]
    fn firma_buena_de_la_llave_fijada() {
        let datos = manifiesto_bytes("0.7.25");
        let v = verificar(&datos, &firmar_a(&datos), &llaves_a(), &[], PRODUCTO_AGENTE).unwrap();
        assert_eq!((v.manifiesto.version.as_str(), v.llave.as_str()), ("0.7.25", ID_A));
    }

    #[test]
    fn firma_mala_llave_equivocada_y_manifiesto_tocado() {
        let datos = manifiesto_bytes("0.7.25");
        let llaves = llaves_a();
        // Otra llave (la B, que este programa no tiene).
        assert!(verificar(&datos, &firmar_b(&datos), &llaves, &[], PRODUCTO_AGENTE).unwrap_err().contains("no conoce"));
        // Manifiesto cambiado después de firmar.
        let firma = firmar_a(&datos);
        let mut tocado = datos.clone();
        let i = tocado.windows(6).position(|w| w == b"0.7.25").unwrap();
        tocado[i + 5] = b'9';
        assert!(verificar(&tocado, &firma, &llaves, &[], PRODUCTO_AGENTE).unwrap_err().contains("no es válida"));
        // Firma cambiada (un bit de la firma del archivo, y del comentario de confianza).
        let mut lineas: Vec<String> = firma.lines().map(str::to_string).collect();
        let mut bin = B64.decode(&lineas[1]).unwrap();
        bin[20] ^= 1;
        lineas[1] = B64.encode(bin);
        assert!(verificar(&datos, &lineas.join("\n"), &llaves, &[], PRODUCTO_AGENTE).is_err());
        let mut lineas: Vec<String> = firma.lines().map(str::to_string).collect();
        lineas[2] = "trusted comment: otra cosa".into();
        assert!(verificar(&datos, &lineas.join("\n"), &llaves, &[], PRODUCTO_AGENTE).is_err());
        // Basura y vacía.
        assert!(verificar(&datos, "", &llaves, &[], PRODUCTO_AGENTE).is_err());
        assert!(verificar(&datos, "untrusted comment: x\nAAAA\n", &llaves, &[], PRODUCTO_AGENTE).is_err());
        assert!(verificar(&datos, &"A".repeat(MAX_FIRMA + 1), &llaves, &[], PRODUCTO_AGENTE).is_err());
        // Sin llaves: nada vale.
        assert!(verificar(&datos, &firma, &Llaves::default(), &[], PRODUCTO_AGENTE).unwrap_err().contains("no tiene llave"));
    }

    #[test]
    fn revocadas_otro_producto_y_varias_llaves() {
        let datos = manifiesto_bytes("0.7.25");
        let mut llaves = llaves_a();
        llaves.anadir(Llaves::leer(LLAVE_B_PUB).unwrap());
        assert_eq!(llaves.len(), 2);
        // Rotación: con las dos fijadas, valen las dos.
        assert!(verificar(&datos, &firmar_b(&datos), &llaves, &[], PRODUCTO_AGENTE).is_ok());
        // Revocada: la A ya no vale.
        assert!(verificar(&datos, &firmar_a(&datos), &llaves, &[ID_A.to_lowercase()], PRODUCTO_AGENTE).unwrap_err().contains("revocada"));
        // Un manifiesto que revoca su propia llave no vale.
        let mut m = manifiesto("0.7.25", &[("linux-x86_64", "a.tar.gz", b"x")]);
        m.revocadas = vec![ID_A.into()];
        let d = serde_json::to_vec(&m).unwrap();
        assert!(verificar(&d, &firmar_a(&d), &llaves, &[], PRODUCTO_AGENTE).is_err());
        // Otro producto, bien firmado: no vale aquí.
        let mut m = manifiesto("0.7.25", &[("linux-x86_64", "a.tar.gz", b"x")]);
        m.producto = "resguardo-servidor".into();
        let d = serde_json::to_vec(&m).unwrap();
        assert!(verificar(&d, &firmar_a(&d), &llaves, &[], PRODUCTO_AGENTE).unwrap_err().contains("otro producto"));
    }

    /// Una firma hecha por minisign de verdad (la de las pruebas de `minisign-verify`,
    /// de su autor): el formato que comprobamos es el del programa, no solo el nuestro.
    #[test]
    fn firma_de_minisign_real() {
        let llaves =
            Llaves::leer("untrusted comment: minisign public key E7620F1842B4E81F\nRWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3\n").unwrap();
        assert_eq!(llaves.ids(), ["E7620F1842B4E81F"]);
        let firma = "untrusted comment: signature from minisign secret key\nRUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=\ntrusted comment: timestamp:1556193335\tfile:test\ny/rUw2y8/hOUYjZU71eHp/Wo1KZ40fGy2VJEDl34XMJM+TX48Ss/17u3IvIfbVR1FkZZSNCisQbuQY+bHwhEBg==\n";
        assert_eq!(comprobar_firma(b"test", firma, &llaves, &[]).unwrap(), "E7620F1842B4E81F");
        assert!(comprobar_firma(b"Test", firma, &llaves, &[]).is_err());
        // La heredada (sin prehash) del mismo programa: no se acepta.
        let heredada = "untrusted comment: signature from minisign secret key\nRWQf6LRCGA9i59SLOFxz6NxvASXDJeRtuZykwQepbDEGt87ig1BNpWaVWuNrm73YiIiJbq71Wi+dP9eKL8OC351vwIasSSbXxwA=\ntrusted comment: timestamp:1555779966\tfile:test\nQtKMXWyYcwdpZAlPF7tE2ENJkRd1ujvKjlj1m9RtHTBnZPa5WKU5uWRs5GoP5M/VqE81QFuMKI5k/SfNQUaOAA==\n";
        assert!(comprobar_firma(b"test", heredada, &llaves, &[]).unwrap_err().contains("antiguo"));
    }

    #[test]
    fn archivo_de_llaves_y_marcador() {
        let marcador = include_str!("../../../packaging/llave-publicacion.pub");
        if marcador.contains(MARCA_SIN_LLAVE) {
            assert!(es_marcador(marcador));
        }
        assert!(Llaves::leer(marcador).is_ok(), "el archivo de llaves del repositorio se lee");
        assert!(!es_marcador(LLAVE_A_PUB));
        assert!(Llaves::leer("hola").is_err());
        assert!(Llaves::leer("# solo comentarios\n\n").unwrap().vacia());
        let dos = format!("{LLAVE_A_PUB}\n{LLAVE_B_PUB}\n{LLAVE_A_PUB}");
        assert_eq!(Llaves::leer(&dos).unwrap().ids(), [ID_A, ID_B]);
        // Con CRLF (Windows) también.
        assert_eq!(Llaves::leer(&LLAVE_A_PUB.replace('\n', "\r\n")).unwrap().ids(), [ID_A]);
    }

    #[test]
    fn manifiestos_no_validos() {
        let base = manifiesto("0.7.25", &[("windows-x86_64", "setup.exe", b"x")]);
        let malo = |f: &dyn Fn(&mut Manifiesto)| {
            let mut m = base.clone();
            f(&mut m);
            m.validar().is_err()
        };
        assert!(base.validar().is_ok());
        assert!(malo(&|m| m.formato = 2));
        assert!(malo(&|m| m.version = "0.7".into()));
        assert!(malo(&|m| m.fecha = "ayer".into()));
        assert!(malo(&|m| m.canal = Some("nocturno".into())));
        assert!(malo(&|m| m.minimo_desde = Some("x".into())));
        assert!(malo(&|m| m.archivos.clear()));
        assert!(malo(&|m| m.archivos[0].plataforma = "macos-arm64".into()));
        assert!(malo(&|m| m.archivos[0].tipo = "tar.gz".into()));
        assert!(malo(&|m| m.archivos[0].nombre = "../setup.exe".into()));
        assert!(malo(&|m| m.archivos[0].nombre = "carpeta/setup.exe".into()));
        assert!(malo(&|m| m.archivos[0].nombre = ".oculto".into()));
        assert!(malo(&|m| m.archivos[0].sha256 = "zz".into()));
        assert!(malo(&|m| m.archivos[0].tamano = 0));
        assert!(malo(&|m| m.archivos[0].tamano = MAX_TAMANO + 1));
        assert!(malo(&|m| m.archivos[0].url = Some("http://ejemplo.com/x".into())));
        assert!(malo(&|m| m.archivos[0].url = Some("https://ejemplo.com/a b".into())));
        assert!(malo(&|m| m.revocadas = vec!["xyz".into()]));
        assert!(malo(&|m| m.archivos.push(m.archivos[0].clone())));
        assert!(Manifiesto::leer(b"{}").is_err());
        assert!(Manifiesto::leer(&vec![b' '; MAX_MANIFIESTO + 1]).is_err());
        // Campos desconocidos: se ignoran.
        let mut v = serde_json::to_value(&base).unwrap();
        v["algo_nuevo"] = Value::from(1);
        assert!(Manifiesto::leer(v.to_string().as_bytes()).is_ok());
    }

    /// Las firmas que hizo la implementación de JavaScript (scripts/lib/minisign.mjs) con las
    /// llaves de pruebas: las dos implementaciones dicen lo mismo.
    #[test]
    fn vectores_de_firmas_de_otra_implementacion() {
        let v = vectores();
        let llaves = llaves_a();
        for c in v["firmas"].as_array().unwrap() {
            let nombre = c["nombre"].as_str().unwrap();
            let revocadas: Vec<String> = serde_json::from_value(c.get("revocadas").cloned().unwrap_or(Value::Array(vec![]))).unwrap();
            let datos = c["manifiesto"].as_str().unwrap().as_bytes();
            let r = comprobar_firma(datos, c["firma"].as_str().unwrap(), &llaves, &revocadas);
            assert_eq!(r.is_ok(), c["valida"].as_bool().unwrap(), "{nombre}: {r:?}");
            if let Some(id) = c["llave"].as_str() {
                assert_eq!(r.as_deref(), Ok(id), "{nombre}");
            }
            if c["valida"] == true {
                let producto_ok = c["producto_valido"].as_bool().unwrap_or(true);
                assert_eq!(verificar(datos, c["firma"].as_str().unwrap(), &llaves, &revocadas, PRODUCTO_AGENTE).is_ok(), producto_ok, "{nombre}");
            }
        }
        let real = &v["firma_minisign_real"];
        let l = Llaves::leer(real["llave_pub"].as_str().unwrap()).unwrap();
        assert_eq!(
            comprobar_firma(real["datos"].as_str().unwrap().as_bytes(), real["firma"].as_str().unwrap(), &l, &[]).as_deref(),
            Ok(real["llave"].as_str().unwrap())
        );
    }

    #[test]
    fn vectores_de_versiones() {
        let v = vectores();
        for c in v["versiones"].as_array().unwrap() {
            let (a, b, esperado) = (c[0].as_str().unwrap(), c[1].as_str().unwrap(), c[2].as_i64().unwrap());
            let o = comparar(a, b).unwrap_or_else(|| panic!("{a} {b}"));
            assert_eq!(o as i64, esperado, "{a} frente a {b}");
            assert_eq!(Version::leer(a).unwrap().to_string(), a);
        }
        for m in v["versiones_no_validas"].as_array().unwrap() {
            assert!(Version::leer(m.as_str().unwrap()).is_none(), "{m}");
        }
    }

    #[test]
    fn vectores_de_combinar() {
        for c in vectores()["combinar"].as_array().unwrap() {
            let ps: Vec<Politica> = serde_json::from_value(c["politicas"].clone()).unwrap();
            let e: Efectiva = serde_json::from_value(c["efectiva"].clone()).unwrap();
            assert_eq!(combinar(&ps), e, "{}", c["nombre"]);
        }
    }

    #[test]
    fn vectores_de_decidir() {
        for c in vectores()["decidir"].as_array().unwrap() {
            let nombre = c["nombre"].as_str().unwrap();
            let plataformas: Vec<String> = serde_json::from_value(c["plataformas"].clone()).unwrap();
            let archivos: Vec<(&str, &str, &[u8])> = plataformas.iter().map(|p| (p.as_str(), "archivo", b"x".as_slice())).collect();
            let mut m = manifiesto(c["nueva"].as_str().unwrap(), &archivos);
            m.minimo_desde = c["minimo_desde"].as_str().map(str::to_string);
            let politica: Efectiva = serde_json::from_value(c["politica"].clone()).unwrap();
            let fallidas: Vec<String> = serde_json::from_value(c.get("fallidas").cloned().unwrap_or(Value::Array(vec![]))).unwrap();
            let fecha = |k: &str| DateTime::parse_from_rfc3339(c[k].as_str().unwrap()).unwrap();
            let d = decidir(&Entrada {
                actual: c["actual"].as_str().unwrap(),
                manifiesto: &m,
                plataforma: c["plataforma"].as_str().unwrap(),
                politica: &politica,
                disponible_desde: fecha("disponible_desde"),
                ahora: fecha("ahora"),
                en_marcha: c["en_marcha"].as_bool().unwrap_or(false),
                fallidas: &fallidas,
                almacen: c["almacen"].as_bool().unwrap_or(false),
            });
            let esperado = &c["decision"];
            assert_eq!(d.estado, esperado["estado"].as_str().unwrap(), "{nombre}: {d:?}");
            assert_eq!(d.motivo, esperado["motivo"].as_str(), "{nombre}: {d:?}");
            if let Some(h) = esperado["hasta"].as_str() {
                assert_eq!(d.hasta.as_deref().map(|x| DateTime::parse_from_rfc3339(x).unwrap()), Some(DateTime::parse_from_rfc3339(h).unwrap()), "{nombre}");
            }
        }
    }

    #[test]
    fn politica_rara_se_vuelve_prudente() {
        let p: Politica = serde_json::from_str(r#"{"modo":"loco","anillo":"x","dias_general":900,"aprobada":"no","retenidas":["0.7.1","z"]}"#).unwrap();
        let p = p.normalizada();
        assert_eq!((p.modo.as_str(), p.anillo.as_str(), p.dias_general, p.aprobada, p.retenidas), ("manual", "general", 30, None, vec!["0.7.1".to_string()]));
        let vacia: Politica = serde_json::from_str("{}").unwrap();
        assert_eq!(vacia, Politica::default());
    }

    #[test]
    fn plataforma_de_este_programa() {
        if cfg!(all(windows, target_arch = "x86_64")) {
            assert_eq!(plataforma_actual(), Some("windows-x86_64"));
        }
        for p in PLATAFORMAS {
            assert!(tipo_de(p).is_some());
        }
    }
}
