//! 14f / 11g (plan 0.8.0, B6): el servidor baja solo de GitHub las versiones firmadas
//! del agente (docs/actualizaciones.md §8).
//!
//! Lo mismo que «Subir una versión» o `poner-publicacion`, sin pasar por las manos de
//! nadie: el manifiesto (`manifiesto-agente.json`) y su firma (`.minisig`) de la última
//! publicación y los archivos que lista. Se comprueban **con el mismo código**
//! ([`crate::publicaciones::poner_manifiesto`] y
//! [`crate::publicaciones::poner_archivo_leyendo`]): la firma con una de las llaves
//! fijadas al compilar y el tamaño y el SHA-256 de cada archivo; lo que no cuadra no se
//! guarda y se dice. El servidor nunca firma nada.
//!
//! Red: solo HTTPS y solo a GitHub ([`HOSTS_GITHUB`]: la página de publicaciones y los
//! sitios de descarga a los que redirige), siguiendo a mano como mucho
//! [`MAX_REDIRECCIONES`] redirecciones y solo hacia esos mismos sitios; con tiempo máximo
//! y con el tamaño máximo del manifiesto, de la firma y de cada archivo (el del
//! manifiesto firmado).
//!
//! Cada [`HORAS`] h por defecto (lo cambia o lo apaga el propietario del servidor) y con
//! «Buscar ahora». En una compilación de desarrollo solo busca sola con
//! `RESGUARDO_GITHUB_AUTO=1` (las pruebas nunca hablan con GitHub).

use crate::almacen::{Almacen, Ts, R};
use crate::estado::St;
use crate::publicaciones as pb;
use resguardo_protocolo::publicacion::{self as p, Llaves};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// La última publicación del proyecto.
pub const BASE_GITHUB: &str = "https://github.com/evercarog/resguardo/releases/latest/download/";
/// Los únicos sitios a los que se conecta: GitHub y sus sitios de descarga.
pub const HOSTS_GITHUB: &[&str] =
    &["github.com", "api.github.com", "objects.githubusercontent.com", "release-assets.githubusercontent.com", "github-releases.githubusercontent.com"];
/// Redirecciones que se siguen, como mucho (GitHub da una: a su sitio de descarga).
pub const MAX_REDIRECCIONES: usize = 5;
/// Tiempo máximo para el manifiesto o la firma.
pub const ESPERA_TEXTO: Duration = Duration::from_secs(60);
/// Tiempo máximo para cada archivo (hasta 512 MB en una conexión lenta).
pub const ESPERA_ARCHIVO: Duration = Duration::from_secs(45 * 60);
/// Cada cuántas horas se busca, por defecto.
pub const HORAS: u32 = 6;
pub const MAX_HORAS: u32 = 7 * 24;

/// De dónde se baja y adónde se deja ir: GitHub ([`Fuente::github`]) o, en las pruebas,
/// un servidor local de mentira.
#[derive(Clone, Debug)]
pub struct Fuente {
    /// Donde están `manifiesto-agente.json` y su firma (acaba en `/`).
    pub base: String,
    pub hosts: Vec<String>,
    /// Solo `https://` (siempre, salvo en las pruebas con un servidor local).
    pub solo_https: bool,
}

impl Fuente {
    pub fn github() -> Self {
        Fuente { base: BASE_GITHUB.into(), hosts: HOSTS_GITHUB.iter().map(|h| h.to_string()).collect(), solo_https: true }
    }

    /// ¿Se puede pedir esta dirección? Esquema y nombre exactos (sin usuario ni nada raro).
    pub fn permitida(&self, url: &str) -> bool {
        let Ok(u) = url.parse::<ureq::http::Uri>() else { return false };
        let esquema_ok = match u.scheme_str() {
            Some("https") => true,
            Some("http") => !self.solo_https,
            _ => false,
        };
        let autoridad = u.authority().map(|a| a.as_str()).unwrap_or("");
        esquema_ok && !autoridad.contains('@') && u.host().is_some_and(|h| self.hosts.iter().any(|x| x.eq_ignore_ascii_case(h)))
    }

    /// De dónde se baja un archivo: su `url` del manifiesto firmado si es de un sitio
    /// permitido; sin ella, junto al manifiesto.
    fn url_de(&self, a: &p::Archivo) -> Result<String, String> {
        match &a.url {
            Some(u) if self.permitida(u) => Ok(u.clone()),
            Some(_) => Err(format!("{}: el manifiesto lo da en otro sitio que no es GitHub; no se baja.", a.nombre)),
            None => Ok(format!("{}{}", self.base, a.nombre)),
        }
    }
}

fn agente(espera: Duration) -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
    let tls = TlsConfig::builder().provider(TlsProvider::Rustls).root_certs(RootCerts::WebPki).build();
    ureq::Agent::config_builder()
        .timeout_global(Some(espera))
        .http_status_as_error(false)
        // Las redirecciones, a mano: solo a los sitios permitidos.
        .max_redirects(0)
        .tls_config(tls)
        .user_agent(format!("Resguardo-Server/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .new_agent()
}

/// Un error de red en palabras (nunca el texto de ureq tal cual).
fn explicar(e: &ureq::Error) -> &'static str {
    use ureq::Error as E;
    match e {
        E::HostNotFound => "no se encuentra GitHub (DNS): ¿tiene salida a Internet este servidor?",
        E::Timeout(_) => "GitHub no respondió a tiempo",
        E::ConnectionFailed | E::Io(_) => "no se pudo conectar con GitHub: ¿tiene salida a Internet este servidor?",
        E::Tls(_) | E::Rustls(_) => "problema con el certificado de GitHub (TLS)",
        _ => "error de red al hablar con GitHub",
    }
}

/// La dirección de una redirección (`Location`), absoluta.
fn siguiente(actual: &str, location: &str) -> Option<String> {
    if location.starts_with("https://") || location.starts_with("http://") {
        return Some(location.to_string());
    }
    let u = actual.parse::<ureq::http::Uri>().ok()?;
    if !location.starts_with('/') || location.starts_with("//") {
        return None;
    }
    Some(format!("{}://{}{location}", u.scheme_str()?, u.authority()?.as_str()))
}

impl Fuente {
    /// GET siguiendo solo redirecciones a sitios permitidos. Devuelve la respuesta 200.
    fn pedir(&self, url: &str, espera: Duration) -> Result<ureq::http::Response<ureq::Body>, String> {
        let a = agente(espera);
        let mut url = url.to_string();
        for _ in 0..=MAX_REDIRECCIONES {
            if !self.permitida(&url) {
                return Err("GitHub redirigió a un sitio que no es suyo: no se sigue.".into());
            }
            let r = a.get(&url).call().map_err(|e| explicar(&e).to_string())?;
            let s = r.status().as_u16();
            match s {
                200 => return Ok(r),
                301 | 302 | 303 | 307 | 308 => {
                    let loc = r.headers().get("location").and_then(|h| h.to_str().ok()).unwrap_or("");
                    url = siguiente(&url, loc).ok_or("GitHub dio una redirección que no se entiende.")?;
                }
                404 => return Err("no está en GitHub (404): ¿aún no hay ninguna publicación firmada?".into()),
                _ => return Err(format!("GitHub respondió {s}")),
            }
        }
        Err("demasiadas redirecciones".into())
    }

    fn texto(&self, nombre: &str, limite: usize) -> Result<String, String> {
        let mut r = self.pedir(&format!("{}{nombre}", self.base), ESPERA_TEXTO).map_err(|e| format!("{nombre}: {e}"))?;
        let t = r.body_mut().with_config().limit(limite as u64 + 1).read_to_string().map_err(|e| match e {
            ureq::Error::BodyExceedsLimit(_) => format!("{nombre} es demasiado grande."),
            _ => format!("{nombre}: no se pudo leer"),
        })?;
        if t.len() > limite {
            return Err(format!("{nombre} es demasiado grande."));
        }
        Ok(t)
    }
}

/// Lo que dejó una búsqueda.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Informe {
    pub version: String,
    /// Es una versión que el servidor no tenía (o le faltaban archivos).
    pub nueva: bool,
    pub completa: bool,
}

/// Busca la última versión firmada en `fuente` y, si es nueva, la guarda como
/// «Subir una versión»: firma y cada SHA-256 comprobados antes de quedarse con nada.
/// Bloquea (red y disco): en un hilo aparte.
pub fn buscar(datos: &Path, llaves: &Llaves, fuente: &Fuente) -> Result<Informe, Rechazo> {
    if llaves.vacia() {
        return Err(Rechazo::Red("Este servidor se compiló sin llave de publicación: no acepta versiones del agente.".into()));
    }
    let texto = fuente.texto(p::NOMBRE_MANIFIESTO, p::MAX_MANIFIESTO).map_err(Rechazo::Red)?;
    let firma = fuente.texto(p::NOMBRE_FIRMA, p::MAX_FIRMA).map_err(Rechazo::Red)?;
    // La misma comprobación que al subirla (firma, reglas, producto), antes de escribir nada.
    let v =
        p::verificar(texto.as_bytes(), &firma, llaves, &[], p::PRODUCTO_AGENTE).map_err(|e| Rechazo::NoCuadra(format!("La versión de GitHub no vale: {e}")))?;
    let version = v.manifiesto.version.clone();
    let guardadas = pb::guardadas(datos, llaves);
    // Ya la tiene entera (la misma), o tiene una más nueva: nada que hacer.
    if guardadas.iter().any(|g| g.manifiesto.version == version && g.texto == texto && g.completa()) {
        return Ok(Informe { version, nueva: false, completa: true });
    }
    if let Some(g) = guardadas.iter().find(|g| g.completa()) {
        if p::comparar(&g.manifiesto.version, &version).is_some_and(|o| o.is_gt()) {
            return Ok(Informe { version: g.manifiesto.version.clone(), nueva: false, completa: true });
        }
    }
    let g = pb::poner_manifiesto(datos, llaves, &texto, &firma).map_err(Rechazo::NoCuadra)?;
    let mut errores = Vec::new();
    for a in g.manifiesto.archivos.iter().filter(|a| g.faltan.contains(&a.nombre)) {
        let r = fuente.url_de(a).map_err(Rechazo::NoCuadra).and_then(|url| {
            let mut resp = fuente.pedir(&url, ESPERA_ARCHIVO).map_err(|e| Rechazo::Red(format!("{}: {e}", a.nombre)))?;
            let mut lector = resp.body_mut().with_config().limit(a.tamano + 1).reader();
            pb::poner_archivo_leyendo(&g, a, &mut lector).map_err(|e| {
                if e.contains("SHA-256") || e.contains("más grande") {
                    Rechazo::NoCuadra(e)
                } else {
                    Rechazo::Red(e)
                }
            })
        });
        if let Err(e) = r {
            errores.push(e);
        }
    }
    let completa = pb::guardadas(datos, llaves).iter().any(|x| x.manifiesto.version == version && x.completa());
    // Lo que no cuadra pesa más que un corte de red.
    if let Some(e) = errores.iter().find(|e| matches!(e, Rechazo::NoCuadra(_))).or(errores.first()) {
        return Err(e.clone());
    }
    Ok(Informe { version, nueva: true, completa })
}

/// Por qué no se usó lo de GitHub.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rechazo {
    /// Sin red, GitHub no responde, no hay publicación…: se reintenta en la próxima vuelta.
    Red(String),
    /// La firma o un SHA-256 no cuadran, o un archivo viene de otro sitio: no se usa.
    NoCuadra(String),
}

impl Rechazo {
    pub fn texto(&self) -> &str {
        match self {
            Rechazo::Red(t) | Rechazo::NoCuadra(t) => t,
        }
    }
}

// ---------------------------------------------------------------------------
// Ajustes y lo último (tabla de valores del servidor)
// ---------------------------------------------------------------------------

const CLAVE_AJUSTES: &str = "github:ajustes";
const CLAVE_ULTIMA: &str = "github:ultima";

/// Lo que decide el propietario del servidor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ajustes {
    #[serde(default = "si")]
    pub activo: bool,
    #[serde(default = "horas")]
    pub horas: u32,
}

impl Default for Ajustes {
    fn default() -> Self {
        Ajustes { activo: true, horas: HORAS }
    }
}

fn si() -> bool {
    true
}

fn horas() -> u32 {
    HORAS
}

/// La última búsqueda.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ultima {
    pub cuando: Ts,
    /// `nueva` (bajó una versión), `al_dia`, `sin_red` o `no_cuadra`.
    pub resultado: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub mensaje: Option<String>,
    /// 14dd: qué pasó con el instalador del agente («Puesto el de la 0.8.0»…).
    #[serde(default)]
    pub instalador: Option<String>,
}

pub fn ajustes(db: &dyn Almacen) -> R<Ajustes> {
    let mut a: Ajustes = db.valor(CLAVE_AJUSTES)?.and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    a.horas = a.horas.clamp(1, MAX_HORAS);
    Ok(a)
}

pub fn guardar_ajustes(db: &dyn Almacen, a: &Ajustes) -> R<()> {
    db.poner_valor(CLAVE_AJUSTES, &serde_json::to_string(a).map_err(|e| e.to_string())?)
}

pub fn ultima(db: &dyn Almacen) -> R<Option<Ultima>> {
    Ok(db.valor(CLAVE_ULTIMA)?.and_then(|t| serde_json::from_str(&t).ok()))
}

fn anotar(db: &dyn Almacen, u: &Ultima) -> R<()> {
    db.poner_valor(CLAVE_ULTIMA, &serde_json::to_string(u).map_err(|e| e.to_string())?)
}

/// ¿Busca sola? Siempre en una compilación de publicación; en una de desarrollo, solo con
/// `RESGUARDO_GITHUB_AUTO=1` (las pruebas y el e2e nunca hablan con GitHub).
pub fn automatica_permitida() -> bool {
    !cfg!(debug_assertions) || std::env::var("RESGUARDO_GITHUB_AUTO").is_ok_and(|v| v == "1")
}

/// ¿Toca buscar ya?
pub fn toca(a: &Ajustes, u: Option<&Ultima>, ahora: Ts) -> bool {
    a.activo && u.is_none_or(|u| ahora - u.cuando >= i64::from(a.horas.clamp(1, MAX_HORAS)) * 3600)
}

static BUSCANDO: AtomicBool = AtomicBool::new(false);

/// ¿Hay una búsqueda en marcha?
pub fn buscando() -> bool {
    BUSCANDO.load(Ordering::SeqCst)
}

/// Busca, lo anota y, si quedó una versión completa, pone el instalador del agente
/// (14dd, si no hay ninguno o es más nuevo). Bloquea: en un hilo aparte. `None` si ya
/// había otra búsqueda en marcha.
pub fn buscar_y_anotar(st: &St, fuente: &Fuente) -> Option<Ultima> {
    if BUSCANDO.swap(true, Ordering::SeqCst) {
        return None;
    }
    struct Suelta;
    impl Drop for Suelta {
        fn drop(&mut self) {
            BUSCANDO.store(false, Ordering::SeqCst);
        }
    }
    let _suelta = Suelta;
    let llaves = pb::llaves(&st.opciones);
    let mut u = Ultima { cuando: crate::almacen::ahora(), ..Default::default() };
    match buscar(&st.datos, &llaves, fuente) {
        Ok(i) => {
            u.resultado = if i.nueva { "nueva" } else { "al_dia" }.into();
            u.mensaje = Some(match (i.nueva, i.completa) {
                (true, true) => format!("Versión {} bajada de GitHub, con su firma y cada archivo comprobados.", i.version),
                (true, false) => format!("Versión {} bajada de GitHub, pero aún le faltan archivos.", i.version),
                _ => format!("Al día: la última versión firmada es la {}.", i.version),
            });
            u.version = Some(i.version);
            u.instalador = instalador_tras_bajar(st);
            if u.resultado == "nueva" {
                let detalle = serde_json::json!({ "version": u.version }).to_string();
                let _ = st.db.auditar_servidor("servidor", "github_publicacion", u.version.as_deref().unwrap_or(""), &detalle);
            }
        }
        Err(r) => {
            u.resultado = if matches!(r, Rechazo::NoCuadra(_)) { "no_cuadra" } else { "sin_red" }.into();
            u.mensaje = Some(r.texto().to_string());
            if matches!(r, Rechazo::NoCuadra(_)) {
                // No se usa nada y queda a la vista del propietario (y en la auditoría del servidor).
                let detalle = serde_json::json!({ "motivo": r.texto() }).to_string();
                let _ = st.db.auditar_servidor("servidor", "github_rechazada", "", &detalle);
            }
            crate::registro::al_archivo(&st.datos, &format!("Actualizaciones del agente (GitHub): {}", r.texto()));
        }
    }
    let _ = anotar(st.db.as_ref(), &u);
    Some(u)
}

/// 14dd: tras bajar una versión completa, el instalador del agente (si el servidor usa el
/// de su carpeta de datos y no hay ninguno o es de una versión anterior). 15x: uno puesto a
/// mano cuenta con la versión de la publicación que tenga su mismo SHA-256; si no es de
/// ninguna conocida, no se pisa (la consola ofrece «Cambiarlo por el de la X»).
fn instalador_tras_bajar(st: &St) -> Option<String> {
    use crate::instalador_agente as ia;
    if st.opciones.instalador_agente.as_deref() != Some(ia::en_datos(&st.datos).as_path()) {
        return None;
    }
    let l = pb::llaves(&st.opciones);
    let g = pb::vigente(&st.datos, &l)?;
    let conocidas = pb::guardadas(&st.datos, &l);
    match ia::de_publicacion(&st.datos, &g, &conocidas, false) {
        Ok(ia::DePublicacion::Puesto { version, .. }) => Some(format!("Instalador del agente de Windows puesto: el de la {version}.")),
        Ok(_) => None,
        Err(e) => Some(format!("No se pudo poner el instalador del agente: {e}")),
    }
}

/// La tarea de fondo: cada 10 min mira si toca buscar (la primera, a los 10 min de arrancar).
pub fn arrancar(st: St) {
    if !automatica_permitida() {
        return;
    }
    tokio::spawn(async move {
        let mut t = tokio::time::interval(Duration::from_secs(600));
        t.tick().await;
        loop {
            t.tick().await;
            let st2 = st.clone();
            let _ = tokio::task::spawn_blocking(move || {
                let (Ok(a), Ok(u)) = (ajustes(st2.db.as_ref()), ultima(st2.db.as_ref())) else { return };
                if toca(&a, u.as_ref(), crate::almacen::ahora()) {
                    buscar_y_anotar(&st2, &Fuente::github());
                }
            })
            .await;
        }
    });
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use resguardo_protocolo::publicacion::pruebas as pp;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// Un servidor HTTP de mentira: `rutas` (ruta → (estado, cabeceras, cuerpo)); anota lo pedido.
    struct Mentira {
        puerto: u16,
        pedidas: Arc<Mutex<Vec<String>>>,
    }

    type Rutas = Vec<(String, (u16, Vec<(String, String)>, Vec<u8>))>;

    fn mentira(rutas: Rutas) -> Mentira {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let puerto = l.local_addr().unwrap().port();
        let pedidas = Arc::new(Mutex::new(Vec::new()));
        let p2 = pedidas.clone();
        std::thread::spawn(move || {
            for s in l.incoming().flatten() {
                let mut lector = BufReader::new(s.try_clone().unwrap());
                let mut linea = String::new();
                if lector.read_line(&mut linea).is_err() {
                    continue;
                }
                let ruta = linea.split_whitespace().nth(1).unwrap_or("").to_string();
                loop {
                    let mut h = String::new();
                    if lector.read_line(&mut h).is_err() || h.trim().is_empty() {
                        break;
                    }
                }
                p2.lock().unwrap().push(ruta.clone());
                let (estado, cab, cuerpo) = rutas.iter().find(|(r, _)| *r == ruta).map(|(_, x)| x.clone()).unwrap_or((404, vec![], b"no".to_vec()));
                let mut s = s;
                let mut r = format!("HTTP/1.1 {estado} X\r\nContent-Length: {}\r\nConnection: close\r\n", cuerpo.len());
                for (k, v) in cab {
                    r.push_str(&format!("{k}: {v}\r\n"));
                }
                r.push_str("\r\n");
                let _ = s.write_all(r.as_bytes());
                let _ = s.write_all(&cuerpo);
            }
        });
        Mentira { puerto, pedidas }
    }

    fn fuente(m: &Mentira) -> Fuente {
        Fuente { base: format!("http://127.0.0.1:{}/ultima/", m.puerto), hosts: vec!["127.0.0.1".into()], solo_https: false }
    }

    /// Una publicación firmada con la llave de pruebas: (manifiesto, firma, exe, tgz).
    fn publicacion(version: &str) -> (String, String, Vec<u8>, Vec<u8>) {
        let mut exe = vec![0u8; 100_000];
        exe[..2].copy_from_slice(b"MZ");
        exe[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        exe[0x80..0x84].copy_from_slice(b"PE\0\0");
        exe[1000..1000 + version.len()].copy_from_slice(version.as_bytes());
        let tgz = format!("paquete {version}").repeat(50).into_bytes();
        let nombre = format!("Resguardo-Agente_{version}_x64-setup.exe");
        let m = pp::manifiesto(version, &[("windows-x86_64", &nombre, &exe), ("linux-x86_64", "agente.tar.gz", &tgz)]);
        let texto = serde_json::to_string_pretty(&m).unwrap();
        let firma = pp::firmar_a(texto.as_bytes());
        (texto, firma, exe, tgz)
    }

    fn rutas_de(version: &str, texto: &str, firma: &str, exe: &[u8], tgz: &[u8]) -> Rutas {
        vec![
            (format!("/ultima/{}", p::NOMBRE_MANIFIESTO), (200, vec![], texto.as_bytes().to_vec())),
            (format!("/ultima/{}", p::NOMBRE_FIRMA), (200, vec![], firma.as_bytes().to_vec())),
            // El instalador, con una redirección a «su sitio de descarga» (el mismo, otra ruta).
            (format!("/ultima/Resguardo-Agente_{version}_x64-setup.exe"), (302, vec![("Location".into(), "/descargas/setup".into())], vec![])),
            ("/descargas/setup".into(), (200, vec![], exe.to_vec())),
            ("/ultima/agente.tar.gz".into(), (200, vec![], tgz.to_vec())),
        ]
    }

    fn datos() -> (tempfile::TempDir, std::path::PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("datos");
        std::fs::create_dir(&d).unwrap();
        (t, d)
    }

    #[test]
    fn solo_https_y_solo_github() {
        let f = Fuente::github();
        assert!(f.permitida("https://github.com/evercarog/resguardo/releases/latest/download/x"));
        assert!(f.permitida("https://objects.githubusercontent.com/github-production-release-asset/1?x=y"));
        assert!(f.permitida("https://release-assets.githubusercontent.com/a"));
        assert!(!f.permitida("http://github.com/x"), "sin TLS no");
        assert!(!f.permitida("https://github.com.ejemplo.net/x"));
        assert!(!f.permitida("https://ejemplo.net/github.com"));
        assert!(!f.permitida("https://usuario@github.com/x"));
        assert!(!f.permitida("file:///etc/passwd"));
        assert!(!f.permitida("no es una dirección"));
        assert_eq!(siguiente("https://github.com/a/b", "/c").as_deref(), Some("https://github.com/c"));
        assert_eq!(siguiente("https://github.com/a/b", "https://objects.githubusercontent.com/x").as_deref(), Some("https://objects.githubusercontent.com/x"));
        assert_eq!(siguiente("https://github.com/a/b", "c"), None);
    }

    #[test]
    fn baja_comprueba_y_guarda_como_al_subirla() {
        let (_t, d) = datos();
        let (texto, firma, exe, tgz) = publicacion("0.8.0");
        let m = mentira(rutas_de("0.8.0", &texto, &firma, &exe, &tgz));
        let l = pp::llaves_a();
        let i = buscar(&d, &l, &fuente(&m)).unwrap();
        assert_eq!(i, Informe { version: "0.8.0".into(), nueva: true, completa: true });
        let v = pb::vigente(&d, &l).unwrap();
        assert_eq!((v.texto.as_str(), v.firma.as_str()), (texto.as_str(), firma.as_str()), "tal cual");
        assert_eq!(std::fs::read(v.dir.join("agente.tar.gz")).unwrap(), tgz);
        // Otra vez: ya la tiene, no baja nada más.
        let antes = m.pedidas.lock().unwrap().len();
        assert_eq!(buscar(&d, &l, &fuente(&m)).unwrap(), Informe { version: "0.8.0".into(), nueva: false, completa: true });
        assert_eq!(m.pedidas.lock().unwrap().len(), antes + 2, "solo el manifiesto y la firma");
        // Sin llave fijada (el marcador de posición): nada.
        assert!(matches!(buscar(&d, &Llaves::default(), &fuente(&m)), Err(Rechazo::Red(_))));
    }

    #[test]
    fn lo_que_no_cuadra_no_se_usa() {
        let l = pp::llaves_a();
        let (texto, firma, exe, tgz) = publicacion("0.8.0");
        // Firma de otra llave.
        let (_t, d) = datos();
        let otra = pp::firmar_b(texto.as_bytes());
        let m = mentira(rutas_de("0.8.0", &texto, &otra, &exe, &tgz));
        assert!(matches!(buscar(&d, &l, &fuente(&m)), Err(Rechazo::NoCuadra(_))));
        assert!(pb::guardadas(&d, &l).is_empty());
        // Un archivo cambiado (otro SHA-256): no se queda y la versión no se sirve.
        let (_t, d) = datos();
        let mut cambiado = exe.clone();
        cambiado[5000] ^= 1;
        let m = mentira(rutas_de("0.8.0", &texto, &firma, &cambiado, &tgz));
        assert!(matches!(buscar(&d, &l, &fuente(&m)), Err(Rechazo::NoCuadra(e)) if e.contains("SHA-256")));
        assert!(pb::vigente(&d, &l).is_none());
        // Más grande que en el manifiesto: tampoco.
        let (_t, d) = datos();
        let mut grande = exe.clone();
        grande.extend_from_slice(b"de mas");
        let m = mentira(rutas_de("0.8.0", &texto, &firma, &grande, &tgz));
        assert!(matches!(buscar(&d, &l, &fuente(&m)), Err(Rechazo::NoCuadra(_))));
        assert!(pb::vigente(&d, &l).is_none());
    }

    #[test]
    fn no_sigue_redirecciones_a_otros_sitios() {
        let (_t, d) = datos();
        let l = pp::llaves_a();
        let (texto, firma, exe, tgz) = publicacion("0.8.0");
        let mut rutas = rutas_de("0.8.0", &texto, &firma, &exe, &tgz);
        // El instalador redirige a otro sitio (127.0.0.2 no está permitido: ni se conecta).
        rutas[2].1 .1 = vec![("Location".into(), "http://127.0.0.2:9/setup".into())];
        let m = mentira(rutas);
        let e = buscar(&d, &l, &fuente(&m)).unwrap_err();
        assert!(e.texto().contains("no es suyo"), "{e:?}");
        assert!(pb::vigente(&d, &l).is_none());
        // El manifiesto también: sin red de por medio.
        let (_t, d) = datos();
        let m = mentira(vec![(format!("/ultima/{}", p::NOMBRE_MANIFIESTO), (302, vec![("Location".into(), "https://ejemplo.net/m.json".into())], vec![]))]);
        assert!(matches!(buscar(&d, &l, &fuente(&m)), Err(Rechazo::Red(e)) if e.contains("no es suyo")));
    }

    #[test]
    fn tamanos_maximos_del_manifiesto() {
        let (_t, d) = datos();
        let l = pp::llaves_a();
        let enorme = vec![b'{'; p::MAX_MANIFIESTO + 10];
        let m = mentira(vec![(format!("/ultima/{}", p::NOMBRE_MANIFIESTO), (200, vec![], enorme))]);
        assert!(matches!(buscar(&d, &l, &fuente(&m)), Err(Rechazo::Red(e)) if e.contains("demasiado grande")));
    }

    #[test]
    fn una_mas_vieja_no_desplaza_la_que_hay() {
        let (_t, d) = datos();
        let l = pp::llaves_a();
        let (texto, firma, exe, tgz) = publicacion("0.8.1");
        buscar(&d, &l, &fuente(&mentira(rutas_de("0.8.1", &texto, &firma, &exe, &tgz)))).unwrap();
        let (texto, firma, exe, tgz) = publicacion("0.8.0");
        let i = buscar(&d, &l, &fuente(&mentira(rutas_de("0.8.0", &texto, &firma, &exe, &tgz)))).unwrap();
        assert_eq!(i, Informe { version: "0.8.1".into(), nueva: false, completa: true });
        assert_eq!(pb::guardadas(&d, &l).len(), 1);
    }

    #[test]
    fn el_instalador_de_la_publicacion() {
        use crate::instalador_agente as ia;
        let (_t, d) = datos();
        let l = pp::llaves_a();
        let (texto, firma, exe, tgz) = publicacion("0.8.0");
        buscar(&d, &l, &fuente(&mentira(rutas_de("0.8.0", &texto, &firma, &exe, &tgz)))).unwrap();
        let g = pb::vigente(&d, &l).unwrap();
        let conocidas = pb::guardadas(&d, &l);
        // Sin ninguno: se pone, con su versión.
        assert_eq!(ia::de_publicacion(&d, &g, &conocidas, false).unwrap(), ia::DePublicacion::Puesto { version: "0.8.0".into(), bytes: exe.len() as u64 });
        assert_eq!(std::fs::read(ia::en_datos(&d)).unwrap(), exe);
        assert_eq!(ia::version_puesta(&d).as_deref(), Some("0.8.0"));
        assert_eq!(ia::de_publicacion(&d, &g, &conocidas, false).unwrap(), ia::DePublicacion::AlDia { version: "0.8.0".into() });
        // Una más nueva: se cambia sola.
        let (texto, firma, exe2, tgz) = publicacion("0.8.1");
        buscar(&d, &l, &fuente(&mentira(rutas_de("0.8.1", &texto, &firma, &exe2, &tgz)))).unwrap();
        let g2 = pb::vigente(&d, &l).unwrap();
        let conocidas = pb::guardadas(&d, &l);
        assert!(matches!(ia::de_publicacion(&d, &g2, &conocidas, false).unwrap(), ia::DePublicacion::Puesto { .. }));
        assert_eq!(std::fs::read(ia::en_datos(&d)).unwrap(), exe2);
        let sha = |b: &[u8]| ia::sha256_hex(b);
        assert_eq!(ia::servido(&d, &ia::en_datos(&d), &sha(&exe2), &conocidas), ia::Servido { version: Some("0.8.1".into()), origen: "publicacion" });
        // 15x: uno puesto a mano que es el de una publicación conocida (el de la 0.8.0): cuenta con
        // esa versión y se cambia solo por el de la 0.8.1.
        let a_mano = d.parent().unwrap().join("a-mano.exe");
        std::fs::write(&a_mano, &exe).unwrap();
        ia::poner(&d, &a_mano, None).unwrap();
        assert_eq!(ia::version_puesta(&d), None);
        assert_eq!(ia::servido(&d, &ia::en_datos(&d), &sha(&exe), &conocidas), ia::Servido { version: Some("0.8.0".into()), origen: "huella" });
        assert_eq!(ia::de_publicacion(&d, &g2, &conocidas, false).unwrap(), ia::DePublicacion::Puesto { version: "0.8.1".into(), bytes: exe2.len() as u64 });
        assert_eq!(std::fs::read(ia::en_datos(&d)).unwrap(), exe2);
        // Uno puesto a mano que no es de ninguna publicación conocida: no se pisa solo; sí con
        // «Cambiarlo por el de la 0.8.1» (forzar).
        let mut raro = exe.clone();
        raro[3000] = 7;
        std::fs::write(&a_mano, &raro).unwrap();
        ia::poner(&d, &a_mano, None).unwrap();
        assert_eq!(ia::servido(&d, &ia::en_datos(&d), &sha(&raro), &conocidas), ia::Servido { version: None, origen: "desconocida" });
        assert_eq!(ia::de_publicacion(&d, &g2, &conocidas, false).unwrap(), ia::DePublicacion::OtroAMano);
        assert_eq!(std::fs::read(ia::en_datos(&d)).unwrap(), raro);
        assert!(matches!(ia::de_publicacion(&d, &g2, &conocidas, true).unwrap(), ia::DePublicacion::Puesto { .. }));
        assert_eq!(std::fs::read(ia::en_datos(&d)).unwrap(), exe2);
        // La marca de la versión no vale si el archivo cambió después (mismo nombre, otro SHA-256).
        std::fs::write(ia::en_datos(&d), &raro).unwrap();
        assert_eq!(ia::servido(&d, &ia::en_datos(&d), &sha(&raro), &conocidas).origen, "desconocida");
    }

    #[test]
    fn cuando_toca() {
        let a = Ajustes::default();
        assert!(toca(&a, None, 1000));
        let u = Ultima { cuando: 1000, resultado: "al_dia".into(), ..Default::default() };
        assert!(!toca(&a, Some(&u), 1000 + 6 * 3600 - 1));
        assert!(toca(&a, Some(&u), 1000 + 6 * 3600));
        assert!(!toca(&Ajustes { activo: false, horas: 6 }, None, 0));
    }
}
