//! Actualización automática del agente (docs/actualizaciones.md).
//!
//! El servicio busca cada 6 horas (y cuando una consola le da un toque) una
//! versión nueva **firmada** en sus consolas y, si se permite, en las
//! publicaciones de GitHub; la comprueba (firma minisign con una llave fijada
//! al compilar, producto, plataforma, versión mayor que la instalada), decide
//! con la política de sus consolas si toca (anillo, días, ventana, nada en
//! marcha…), la baja comprobando tamaño y SHA-256 y lanza el **actualizador**:
//! una copia del programa que ya funciona, fuera del servicio, que instala,
//! espera a que la versión nueva diga que está sana y, si no lo dice en 10
//! minutos, vuelve a la anterior.
//!
//! El paso que de verdad cambia el programa está detrás de [`Plataforma`]
//! (Windows: el instalador NSIS en modo actualización; Linux: reemplazo atómico
//! de los binarios y `systemctl restart`), para probar el resto con una falsa.

use resguardo_protocolo::publicacion::{self as p, Llaves, Verificado};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::time::Duration;

/// Las llaves de publicación fijadas al compilar.
pub const LLAVES_FIJADAS: &str = include_str!("../../../packaging/llave-publicacion.pub");
/// `RESGUARDO_SIN_ACTUALIZACIONES=1` al compilar: agente sin actualización automática.
const SIN_ACTUALIZACIONES: Option<&str> = option_env!("RESGUARDO_SIN_ACTUALIZACIONES");
/// Las publicaciones de GitHub (la última).
pub const BASE_GITHUB: &str = "https://github.com/evercarog/resguardo/releases/latest/download/";
/// Cada cuánto se busca, y la primera vez tras arrancar.
pub const CADA: Duration = Duration::from_secs(6 * 3600);
pub const AL_ARRANCAR: Duration = Duration::from_secs(10 * 60);
/// Lo que tiene la versión nueva para decir que está sana.
pub const PLAZO_SALUD: Duration = Duration::from_secs(10 * 60);
/// Un toque de una consola, como mucho uno por minuto.
const ENTRE_TOQUES: i64 = 60;
/// Una actualización lanzada que no termina en este tiempo se da por perdida (p. ej. se apagó el equipo).
const ACTUALIZANDO_MAX: i64 = 45 * 60;
const ARCHIVO_ESTADO: &str = "actualizacion.json";
const ARCHIVO_AJUSTES: &str = "actualizaciones-ajustes.json";
const RUTA_ARCHIVOS: &str = "/api/agente/actualizacion/archivos/";

/// ¿Se compiló sin actualización automática?
pub fn desactivada_al_compilar() -> bool {
    SIN_ACTUALIZACIONES == Some("1")
}

/// Las llaves en las que confía este programa: las fijadas y, **solo en una
/// compilación de desarrollo**, las de pruebas de `RESGUARDO_LLAVES_PRUEBAS`.
pub fn llaves() -> Llaves {
    if desactivada_al_compilar() {
        return Llaves::default();
    }
    let mut l = Llaves::leer(LLAVES_FIJADAS).unwrap_or_default();
    if cfg!(any(test, debug_assertions)) {
        if let Some(extra) = std::env::var("RESGUARDO_LLAVES_PRUEBAS").ok().and_then(|r| std::fs::read_to_string(r).ok()).and_then(|t| Llaves::leer(&t).ok()) {
            l.anadir(extra);
        }
    }
    l
}

/// ¿Puede actualizarse solo? (hay alguna llave fijada y una plataforma publicada)
pub fn activada() -> bool {
    !llaves().vacia() && p::plataforma_actual().is_some()
}

// ---------------------------------------------------------------------------
// Estado (lo que se enseña en la consola y en `resguardo-agente actualizaciones`)
// ---------------------------------------------------------------------------

/// `actualizacion.json` en la carpeta del agente (sin secretos).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Estado {
    /// `al_dia`, `pendiente`, `pausada`, `descargando`, `actualizando`, `actualizada`,
    /// `vuelta_atras`, `fallida` o `desactivada`.
    #[serde(default)]
    pub estado: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motivo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mensaje: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_disponible: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hasta: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_objetivo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_fallida: Option<String>,
    /// Versiones que fallaron aquí (no se vuelven a intentar solas).
    #[serde(default)]
    pub fallidas: Vec<String>,
    /// La primera vez que se vio cada versión (para los días del anillo general).
    #[serde(default)]
    pub vistas: std::collections::BTreeMap<String, String>,
    /// Llaves revocadas por algún manifiesto válido (nunca se quitan).
    #[serde(default)]
    pub revocadas: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origen: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anillo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ultima_busqueda: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cuando: Option<String>,
    /// Consolas (ids de vínculo) a las que falta mandar el aviso `actualizacion_fallida`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aviso_para: Vec<String>,
}

pub fn leer_estado() -> Estado {
    std::fs::read(crate::agent::agent_dir().join(ARCHIVO_ESTADO)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn guardar_estado(e: &Estado) {
    let _ = crate::agent::write_json(ARCHIVO_ESTADO, e);
}

/// Cambia el estado bajo un cerrojo (el hilo y el canal no se pisan).
fn cambiar<R>(f: impl FnOnce(&mut Estado) -> R) -> R {
    static CERROJO: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _g = CERROJO.lock().unwrap_or_else(|e| e.into_inner());
    let mut e = leer_estado();
    let r = f(&mut e);
    guardar_estado(&e);
    r
}

fn ahora_rfc() -> String {
    chrono::Local::now().to_rfc3339()
}

/// `informe.actualizacion` para las consolas (v1.57).
pub fn informe() -> Value {
    if !activada() {
        return json!({ "estado": "desactivada", "motivo": if desactivada_al_compilar() { "sin_actualizaciones" } else { "sin_llave" } });
    }
    let e = leer_estado();
    if e.estado.is_empty() {
        return json!({ "estado": "al_dia" });
    }
    json!({
        "estado": e.estado, "motivo": e.motivo, "mensaje": e.mensaje, "version_disponible": e.version_disponible, "hasta": e.hasta,
        "version_objetivo": e.version_objetivo, "version_fallida": e.version_fallida, "anillo": e.anillo, "modo": e.modo,
        "origen": e.origen, "ultima_busqueda": e.ultima_busqueda, "cuando": e.cuando,
    })
}

/// Ajustes locales (`actualizaciones-ajustes.json`, solo administradores).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ajustes {
    /// Buscar también en GitHub (si no lo tiene ninguna consola).
    #[serde(default = "si")]
    pub github: bool,
}

fn si() -> bool {
    true
}

impl Default for Ajustes {
    fn default() -> Self {
        Ajustes { github: true }
    }
}

pub fn ajustes() -> Ajustes {
    std::fs::read(crate::agent::agent_dir().join(ARCHIVO_AJUSTES)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn poner_ajustes(a: &Ajustes) -> Result<(), String> {
    crate::agent::write_json(ARCHIVO_AJUSTES, a)
}

/// ¿Se mira GitHub? En las pruebas (`RESGUARDO_AGENT_DIR`), nunca, salvo que se pida.
fn github_permitido() -> bool {
    if crate::agent::test_mode() && std::env::var("RESGUARDO_ACTUALIZACIONES_GITHUB").ok().as_deref() != Some("si") {
        return false;
    }
    ajustes().github
}

// ---------------------------------------------------------------------------
// El hilo del servicio
// ---------------------------------------------------------------------------

static TOQUE: AtomicBool = AtomicBool::new(false);
static ULTIMO_TOQUE: AtomicI64 = AtomicI64::new(0);
static PROXIMA: AtomicI64 = AtomicI64::new(0);

/// Una consola pide que se busque ya (`{"t":"actualizacion"}` por el canal).
pub fn toque() {
    let ahora = chrono::Utc::now().timestamp();
    if ahora - ULTIMO_TOQUE.load(Ordering::SeqCst) >= ENTRE_TOQUES {
        ULTIMO_TOQUE.store(ahora, Ordering::SeqCst);
        TOQUE.store(true, Ordering::SeqCst);
    }
}

/// El hilo de las actualizaciones (lo arranca el servicio, también en `--primer-plano`).
pub fn hilo() {
    PROXIMA.store(chrono::Utc::now().timestamp() + AL_ARRANCAR.as_secs() as i64, Ordering::SeqCst);
    std::thread::spawn(|| loop {
        tick();
        std::thread::sleep(Duration::from_secs(if crate::agent::test_mode() { 2 } else { 30 }));
    });
}

/// Una vuelta del hilo (también para las pruebas).
pub fn tick() {
    if !activada() {
        cambiar(|e| {
            if e.estado != "desactivada" {
                e.estado = "desactivada".into();
                e.cuando = Some(ahora_rfc());
            }
        });
        return;
    }
    procesar_resultado();
    salud_sin_consolas();
    mandar_avisos();
    if en_curso() {
        return;
    }
    let ahora = chrono::Utc::now().timestamp();
    let toca = TOQUE.swap(false, Ordering::SeqCst) || ahora >= PROXIMA.load(Ordering::SeqCst);
    if !toca {
        return;
    }
    // Al azar hasta 30 min más, para no llegar todos a la vez.
    let azar = i64::from(rand_u8()) * 30 * 60 / 255;
    PROXIMA.store(ahora + CADA.as_secs() as i64 + azar, Ordering::SeqCst);
    if let Err(m) = buscar_e_instalar() {
        crate::agent::log(&format!("Actualizaciones: {m}"));
    }
}

fn rand_u8() -> u8 {
    uuid::Uuid::new_v4().as_bytes()[0]
}

// ---------------------------------------------------------------------------
// Buscar
// ---------------------------------------------------------------------------

/// Una versión firmada que ofrece una fuente.
#[derive(Clone, Debug)]
pub struct Oferta {
    pub verificado: Verificado,
    pub origen: Origen,
}

#[derive(Clone, Debug)]
pub enum Origen {
    /// Una consola: su vínculo y la ruta de sus archivos.
    Consola {
        enlace: String,
        nombre: String,
        ruta: String,
    },
    GitHub,
}

impl Origen {
    fn texto(&self) -> String {
        match self {
            Origen::Consola { nombre, .. } => format!("consola «{nombre}»"),
            Origen::GitHub => "GitHub".into(),
        }
    }
}

/// Lo que dicen las consolas: sus políticas, sus ofertas y si alguna respondió.
#[derive(Default)]
pub struct DeConsolas {
    pub politicas: Vec<p::Politica>,
    pub ofertas: Vec<Oferta>,
    pub respondieron: usize,
    pub errores: Vec<String>,
}

/// La ruta de los archivos que da una consola: la suya y nada más.
fn ruta_valida(r: &str) -> bool {
    r.starts_with(RUTA_ARCHIVOS)
        && r.ends_with('/')
        && r.len() < 200
        && r.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'.' | b'-' | b'_'))
        && !r.contains("..")
}

/// Lee la respuesta de `GET /api/agente/actualizacion` de una consola.
pub fn leer_respuesta(r: &Value, llaves: &Llaves, revocadas: &[String], enlace: &str, nombre: &str) -> (Option<p::Politica>, Result<Option<Oferta>, String>) {
    let politica = r.get("politica").and_then(|x| serde_json::from_value::<p::Politica>(x.clone()).ok()).map(p::Politica::normalizada);
    let Some(pb) = r.get("publicacion").filter(|x| x.is_object()) else { return (politica, Ok(None)) };
    let (Some(texto), Some(firma), Some(ruta)) = (pb["manifiesto"].as_str(), pb["firma"].as_str(), pb["archivos"].as_str()) else {
        return (politica, Err(format!("La consola «{nombre}» dio una publicación incompleta.")));
    };
    if !ruta_valida(ruta) {
        return (politica, Err(format!("La consola «{nombre}» dio una ruta de archivos no válida.")));
    }
    let oferta = p::verificar(texto.as_bytes(), firma, llaves, revocadas, p::PRODUCTO_AGENTE)
        .map(|v| Some(Oferta { verificado: v, origen: Origen::Consola { enlace: enlace.into(), nombre: nombre.into(), ruta: ruta.into() } }))
        .map_err(|e| format!("La versión que da la consola «{nombre}» no vale: {e}"));
    (politica, oferta)
}

fn de_consolas(llaves: &Llaves, revocadas: &[String]) -> DeConsolas {
    let mut d = DeConsolas::default();
    let Some(v) = crate::servidor_v2::cargar().filter(|v| v.modo == "gestionado") else { return d };
    for w in v.vistas() {
        let nombre = crate::consolas_v2::nombre_de(&w.nombre_consola, &w.url);
        match crate::servidor_v2::llamar(&w, "GET", "/api/agente/actualizacion", None) {
            Ok((200, r)) => {
                d.respondieron += 1;
                let (pol, oferta) = leer_respuesta(&r, llaves, revocadas, &w.id_enlace(), &nombre);
                // Una consola que responde sin política (no debería): la de por defecto.
                d.politicas.push(pol.unwrap_or_default());
                match oferta {
                    Ok(Some(o)) => d.ofertas.push(o),
                    Ok(None) => {}
                    Err(e) => d.errores.push(e),
                }
            }
            // Una consola anterior a esta función: no cuenta.
            Ok((404, _)) => {}
            Ok((s, _)) => d.errores.push(format!("La consola «{nombre}» respondió {s}.")),
            Err(e) => d.errores.push(format!("Consola «{nombre}»: {e}")),
        }
    }
    d
}

fn agente_publico(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(timeout)).http_status_as_error(false).tls_config(crate::web::tls_sistema()).build().new_agent()
}

fn leer_texto(agente: &ureq::Agent, url: &str, limite: u64) -> Result<String, String> {
    let mut r = agente.get(url).call().map_err(|e| format!("No se pudo leer {url}: {e}"))?;
    if r.status().as_u16() != 200 {
        return Err(format!("{url} respondió {}.", r.status().as_u16()));
    }
    r.body_mut().with_config().limit(limite).read_to_string().map_err(|e| format!("No se pudo leer {url}: {e}"))
}

fn de_github(llaves: &Llaves, revocadas: &[String]) -> Result<Option<Oferta>, String> {
    let a = agente_publico(Duration::from_secs(60));
    let texto = leer_texto(&a, &format!("{BASE_GITHUB}{}", p::NOMBRE_MANIFIESTO), p::MAX_MANIFIESTO as u64)?;
    let firma = leer_texto(&a, &format!("{BASE_GITHUB}{}", p::NOMBRE_FIRMA), p::MAX_FIRMA as u64)?;
    let v = p::verificar(texto.as_bytes(), &firma, llaves, revocadas, p::PRODUCTO_AGENTE).map_err(|e| format!("La versión de GitHub no vale: {e}"))?;
    Ok(Some(Oferta { verificado: v, origen: Origen::GitHub }))
}

/// La mejor oferta: la versión más alta; a igualdad, la de una consola.
pub fn elegir(ofertas: Vec<Oferta>) -> Option<Oferta> {
    let mut mejor: Option<Oferta> = None;
    for o in ofertas {
        let mejor_v = mejor.as_ref().and_then(|m| m.verificado.manifiesto.version_leida());
        let esta = o.verificado.manifiesto.version_leida();
        let gana = match (&mejor, mejor_v, esta) {
            (None, _, Some(_)) => true,
            (Some(m), Some(a), Some(b)) => b > a || (b == a && matches!(m.origen, Origen::GitHub) && matches!(o.origen, Origen::Consola { .. })),
            _ => false,
        };
        if gana {
            mejor = Some(o);
        }
    }
    mejor
}

/// ¿Hay algo en marcha (copia, verificación, restauración, espejo…)?
fn en_marcha() -> bool {
    !crate::progreso_v2::tareas(None).is_empty() || !crate::escritorio::en_marcha::actividades().is_empty()
}

/// Busca, decide y, si toca, baja e instala.
pub fn buscar_e_instalar() -> Result<(), String> {
    let llaves = llaves();
    let plataforma = p::plataforma_actual().ok_or("Esta plataforma no se publica.")?;
    let revocadas = leer_estado().revocadas;
    let consolas = de_consolas(&llaves, &revocadas);
    let mut ofertas = consolas.ofertas.clone();
    let mut errores = consolas.errores.clone();
    if github_permitido() {
        match de_github(&llaves, &revocadas) {
            Ok(Some(o)) => ofertas.push(o),
            Ok(None) => {}
            Err(e) => errores.push(e),
        }
    }
    for e in &errores {
        crate::agent::log(&format!("Actualizaciones: {e}"));
    }
    let politica = p::combinar(&consolas.politicas);
    let actual = crate::version_programa().to_string();
    let Some(oferta) = elegir(ofertas) else {
        cambiar(|e| {
            e.estado = "al_dia".into();
            e.motivo = None;
            e.mensaje = None;
            e.version_disponible = None;
            e.hasta = None;
            e.anillo = Some(politica.anillo.clone());
            e.modo = Some(politica.modo.clone());
            e.ultima_busqueda = Some(ahora_rfc());
        });
        return Ok(());
    };
    let m = oferta.verificado.manifiesto.clone();
    // Simulada (pruebas): la versión «instalada» sigue siendo la de antes; no se repite.
    if simulacion().is_some() {
        let e = leer_estado();
        if e.estado == "actualizada" && e.version_objetivo.as_deref() == Some(m.version.as_str()) {
            return Ok(());
        }
    }
    let ahora = chrono::Local::now().fixed_offset();
    let (vista, fallidas) = cambiar(|e| {
        // Lo revocado por un manifiesto válido, para siempre.
        for r in &m.revocadas {
            if !e.revocadas.iter().any(|x| x.eq_ignore_ascii_case(r)) {
                e.revocadas.push(r.to_ascii_uppercase());
            }
        }
        let vista = e.vistas.entry(m.version.clone()).or_insert_with(|| ahora.to_rfc3339()).clone();
        // Solo las últimas (no crece sin fin).
        while e.vistas.len() > 20 {
            let primera = e.vistas.keys().next().cloned().unwrap_or_default();
            e.vistas.remove(&primera);
        }
        (vista, e.fallidas.clone())
    });
    let fecha = chrono::DateTime::parse_from_rfc3339(&m.fecha).unwrap_or(ahora);
    let vista = chrono::DateTime::parse_from_rfc3339(&vista).unwrap_or(ahora);
    let decision = p::decidir(&p::Entrada {
        actual: &actual,
        manifiesto: &m,
        plataforma,
        politica: &politica,
        disponible_desde: fecha.max(vista),
        ahora,
        en_marcha: en_marcha(),
        fallidas: &fallidas,
        almacen: crate::server::load().enabled,
    });
    let origen = oferta.origen.texto();
    cambiar(|e| {
        // Si esa versión acaba de fallar aquí, la consola sigue viendo «volvió a la anterior».
        let sigue_vuelta = e.estado == "vuelta_atras" && decision.motivo == Some("retenida") && e.version_fallida.as_deref() == Some(m.version.as_str());
        if !sigue_vuelta {
            e.estado = decision.estado.to_string();
            e.motivo = decision.motivo.map(str::to_string);
            e.mensaje = None;
        }
        e.version_disponible = (decision.estado != "al_dia").then(|| m.version.clone());
        e.hasta = decision.hasta.clone();
        e.anillo = Some(politica.anillo.clone());
        e.modo = Some(politica.modo.clone());
        e.origen = Some(origen.clone());
        e.ultima_busqueda = Some(ahora_rfc());
    });
    if !decision.instalar() {
        return Ok(());
    }
    // Con consolas, solo si alguna respondió ahora (si no, la salud no se podría comprobar).
    let con_consolas = crate::servidor_v2::cargar().is_some_and(|v| v.modo == "gestionado" && !v.vistas().is_empty());
    if con_consolas && consolas.respondieron == 0 {
        cambiar(|e| {
            e.estado = "pendiente".into();
            e.motivo = Some("sin_consola".into());
        });
        return Ok(());
    }
    crate::agent::log(&format!(
        "Actualizaciones: se instala la versión {} (de {origen}){}.",
        m.version,
        if decision.aprobada { ", aprobada en la consola" } else { "" }
    ));
    if let Err(err) = instalar(&oferta, plataforma) {
        crate::agent::log(&format!("Actualizaciones: no se pudo actualizar a la {}: {err}", m.version));
        cambiar(|e| {
            e.estado = "fallida".into();
            e.motivo = Some("preparar".into());
            e.mensaje = Some(err.chars().take(300).collect());
            e.cuando = Some(ahora_rfc());
        });
        return Err(err);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Bajar y preparar
// ---------------------------------------------------------------------------

/// La carpeta privada de las actualizaciones (solo SYSTEM y Administradores, o root).
pub fn carpeta() -> PathBuf {
    crate::agent::private_dir().join("actualizacion")
}

fn preparar_carpeta(d: &Path) -> Result<(), String> {
    if crate::agent::test_mode() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        return Ok(());
    }
    let _ = crate::agent::prepare_dir();
    crate::platform::carpeta_privada(d)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Copia `entrada` en `destino` (archivo nuevo) sin pasar de `tamano` y calculando el
/// SHA-256 al vuelo. Si no coincide en tamaño o en hash, lo borra.
pub fn guardar_comprobado(mut entrada: impl std::io::Read, destino: &Path, a: &p::Archivo) -> Result<(), String> {
    use std::io::Write;
    let _ = std::fs::remove_file(destino);
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    let mut f = o.open(destino).map_err(|e| format!("No se pudo crear {}: {e}", destino.display()))?;
    let mut h = Sha256::new();
    let mut n: u64 = 0;
    let mut buf = vec![0u8; 256 * 1024];
    let r = (|| {
        loop {
            let leidos = entrada.read(&mut buf).map_err(|e| format!("La descarga se cortó: {e}"))?;
            if leidos == 0 {
                break;
            }
            n += leidos as u64;
            if n > a.tamano {
                return Err(format!("{} es más grande que en el manifiesto firmado.", a.nombre));
            }
            h.update(&buf[..leidos]);
            f.write_all(&buf[..leidos]).map_err(|e| e.to_string())?;
        }
        f.sync_all().map_err(|e| e.to_string())?;
        if n != a.tamano {
            return Err(format!("{} no tiene el tamaño del manifiesto firmado ({n} de {} bytes).", a.nombre, a.tamano));
        }
        if !hex(&h.finalize()).eq_ignore_ascii_case(&a.sha256) {
            return Err(format!("{} no coincide con el SHA-256 del manifiesto firmado: se descarta.", a.nombre));
        }
        Ok(())
    })();
    if r.is_err() {
        drop(f);
        let _ = std::fs::remove_file(destino);
    }
    r
}

/// Baja el archivo de su origen (y, si falla y se permite, de su `url` en GitHub).
fn bajar(oferta: &Oferta, a: &p::Archivo, destino: &Path) -> Result<(), String> {
    let de_consola = || -> Result<(), String> {
        let Origen::Consola { enlace, ruta, .. } = &oferta.origen else { return Err("sin consola".into()) };
        let v = crate::consolas_v2::vista(enlace).ok_or("Esa consola ya no gestiona este equipo.")?;
        let agente = crate::servidor_v2::agente_descarga(&v)?;
        let url = format!("{}{ruta}{}", v.url.trim_end_matches('/'), a.nombre);
        let mut r = agente
            .get(&url)
            .header("authorization", &format!("Equipo {}:{}", v.equipo_id, v.secreto))
            .call()
            .map_err(|e| crate::servidor_v2::explicar_error_conexion(&e))?;
        if r.status().as_u16() != 200 {
            return Err(format!("La consola respondió {} al pedir {}.", r.status().as_u16(), a.nombre));
        }
        guardar_comprobado(r.body_mut().as_reader(), destino, a)
    };
    let de_url = || -> Result<(), String> {
        let url = a.url.as_deref().filter(|u| u.starts_with("https://")).ok_or("El manifiesto no da una dirección para este archivo.")?;
        let mut r = agente_publico(Duration::from_secs(2 * 3600)).get(url).call().map_err(|e| format!("No se pudo bajar {}: {e}", a.nombre))?;
        if r.status().as_u16() != 200 {
            return Err(format!("{url} respondió {}.", r.status().as_u16()));
        }
        guardar_comprobado(r.body_mut().as_reader(), destino, a)
    };
    match &oferta.origen {
        Origen::Consola { .. } => de_consola().or_else(|e| if github_permitido() { de_url().map_err(|e2| format!("{e} · {e2}")) } else { Err(e) }),
        Origen::GitHub => de_url(),
    }
}

/// Lo que el actualizador necesita (`plan.json`): todo con rutas absolutas, sin depender
/// de la carpeta del agente (el actualizador se llama de otra forma).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Plan {
    pub version_anterior: String,
    pub version_objetivo: String,
    pub plataforma: String,
    /// El instalador (Windows) o el paquete (Linux) ya comprobado.
    pub archivo: PathBuf,
    pub sha256: String,
    pub tamano: u64,
    /// `privado/actualizacion`.
    pub carpeta: PathBuf,
    /// La carpeta del programa instalado.
    pub instalacion: PathBuf,
    pub plazo_s: u64,
    pub creado: String,
}

impl Plan {
    pub fn salud(&self) -> PathBuf {
        self.carpeta.join("salud.json")
    }
    pub fn resultado(&self) -> PathBuf {
        self.carpeta.join("resultado.json")
    }
    pub fn anterior(&self) -> PathBuf {
        self.carpeta.join("anterior")
    }
}

/// Los programas instalados que se guardan para volver atrás.
pub fn programas(plataforma: &str) -> &'static [&'static str] {
    if plataforma.starts_with("windows") {
        &["resguardo-agente.exe", "restic.exe", "rest-server.exe", "rclone.exe"]
    } else {
        &["resguardo-agente", "restic", "rest-server", "rclone", "VERSION"]
    }
}

/// Guarda lo instalado en `anterior/` (copia completa, para volver atrás).
pub fn guardar_anterior(plan: &Plan) -> Result<(), String> {
    let dir = plan.anterior();
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("No se pudo crear {}: {e}", dir.display()))?;
    for n in programas(&plan.plataforma) {
        let de = plan.instalacion.join(n);
        if std::fs::symlink_metadata(&de).is_ok_and(|m| m.is_file()) {
            std::fs::copy(&de, dir.join(n)).map_err(|e| format!("No se pudo guardar {n} para volver atrás: {e}"))?;
        }
    }
    if !dir.join(programas(&plan.plataforma)[0]).is_file() {
        return Err("No se encontró el programa instalado para poder volver atrás.".into());
    }
    Ok(())
}

fn leer_plan(carpeta: &Path) -> Option<Plan> {
    std::fs::read(carpeta.join("plan.json")).ok().and_then(|b| serde_json::from_slice(&b).ok())
}

fn escribir_json(ruta: &Path, v: &impl Serialize) -> Result<(), String> {
    let tmp = ruta.with_extension("tmp");
    let _ = std::fs::remove_file(&tmp);
    std::fs::write(&tmp, serde_json::to_vec_pretty(v).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, ruta).map_err(|e| e.to_string())
}

fn instalar(oferta: &Oferta, plataforma: &str) -> Result<(), String> {
    let m = &oferta.verificado.manifiesto;
    let a = m.archivo(plataforma).ok_or("La versión no trae archivo para este equipo.")?.clone();
    let dir = carpeta();
    preparar_carpeta(&dir)?;
    for viejo in ["plan.json", "salud.json", "resultado.json"] {
        let _ = std::fs::remove_file(dir.join(viejo));
    }
    cambiar(|e| {
        e.estado = "descargando".into();
        e.motivo = None;
        e.version_objetivo = Some(m.version.clone());
        e.cuando = Some(ahora_rfc());
    });
    let parte = dir.join("descarga.part");
    bajar(oferta, &a, &parte)?;
    let archivo = dir.join(&a.nombre);
    let _ = std::fs::remove_file(&archivo);
    std::fs::rename(&parte, &archivo).map_err(|e| e.to_string())?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let instalacion = exe.parent().ok_or("No se sabe dónde está instalado el programa.")?.to_path_buf();
    let plan = Plan {
        version_anterior: crate::version_programa().to_string(),
        version_objetivo: m.version.clone(),
        plataforma: plataforma.to_string(),
        archivo,
        sha256: a.sha256.clone(),
        tamano: a.tamano,
        carpeta: dir.clone(),
        instalacion,
        plazo_s: plazo_salud().as_secs(),
        creado: ahora_rfc(),
    };
    let simulada = simulacion();
    if simulada.is_none() {
        guardar_anterior(&plan)?;
    }
    escribir_json(&dir.join("plan.json"), &plan)?;
    cambiar(|e| {
        e.estado = "actualizando".into();
        e.cuando = Some(ahora_rfc());
    });
    match simulada {
        Some(modo) => {
            // Solo en compilaciones de desarrollo: el resto del camino, sin tocar el programa.
            std::thread::spawn(move || {
                let r = ejecutar(&plan, &Simulada { modo });
                let _ = escribir_json(&plan.resultado(), &r);
            });
            Ok(())
        }
        None => lanzar_actualizador(&plan, &exe),
    }
}

fn plazo_salud() -> Duration {
    if cfg!(debug_assertions) {
        if let Some(s) = std::env::var("RESGUARDO_ACTUALIZACION_PLAZO_S").ok().and_then(|s| s.parse::<u64>().ok()) {
            return Duration::from_secs(s.max(1));
        }
    }
    PLAZO_SALUD
}

/// En compilaciones de desarrollo y en la carpeta de pruebas: `simular-actualizacion.txt`
/// («ok» o «falla») hace la instalación de mentira (e2e). Nunca en una publicada.
fn simulacion() -> Option<String> {
    if !cfg!(debug_assertions) || !crate::agent::test_mode() {
        return None;
    }
    std::fs::read_to_string(crate::agent::agent_dir().join("simular-actualizacion.txt"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| s == "ok" || s == "falla")
}

/// Copia el programa actual como actualizador y lo lanza fuera del servicio.
fn lanzar_actualizador(plan: &Plan, exe: &Path) -> Result<(), String> {
    let nombre = if cfg!(windows) { "actualizador.exe" } else { "actualizador" };
    let actualizador = plan.carpeta.join(nombre);
    let _ = std::fs::remove_file(&actualizador);
    std::fs::copy(exe, &actualizador).map_err(|e| format!("No se pudo preparar el actualizador: {e}"))?;
    let plan_json = plan.carpeta.join("plan.json");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        let lanzar = |extra: u32| {
            std::process::Command::new(&actualizador)
                .arg("--actualizar-agente")
                .arg(&plan_json)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | extra)
                .spawn()
        };
        // Fuera del trabajo (job) del servicio si se puede; si el sistema no lo deja, sin eso.
        lanzar(CREATE_BREAKAWAY_FROM_JOB).or_else(|_| lanzar(0)).map(|_| ()).map_err(|e| format!("No se pudo lanzar el actualizador: {e}"))
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&actualizador, std::fs::Permissions::from_mode(0o700));
        let systemd_run = ["/usr/bin/systemd-run", "/bin/systemd-run"]
            .into_iter()
            .find(|r| Path::new(r).is_file())
            .ok_or("Falta systemd-run: este equipo no se actualiza solo.")?;
        // Una unidad propia: `systemctl restart resguardo-agente` no la mata con el resto del servicio.
        let unidad = format!("resguardo-agente-actualizacion-{}", chrono::Utc::now().timestamp());
        let ok = std::process::Command::new(systemd_run)
            .args(["--unit", &unidad, "--collect", "--quiet", "--description=Resguardo Agente: actualización"])
            .arg(&actualizador)
            .arg("--actualizar-agente")
            .arg(&plan_json)
            .stdin(std::process::Stdio::null())
            .status()
            .map_err(|e| format!("No se pudo lanzar el actualizador: {e}"))?
            .success();
        if ok {
            Ok(())
        } else {
            Err("systemd-run no pudo lanzar el actualizador.".into())
        }
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = (actualizador, plan_json);
        Err("Este sistema no se actualiza solo.".into())
    }
}

// ---------------------------------------------------------------------------
// La salud de la versión nueva
// ---------------------------------------------------------------------------

/// `salud.json`: la versión nueva está en marcha y sana.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Salud {
    pub version: String,
    pub sano: bool,
    pub cuando: String,
    #[serde(default)]
    pub como: String,
}

/// ¿Esta versión es la que espera una actualización en curso?
fn plan_para_mi() -> Option<Plan> {
    leer_plan(&carpeta()).filter(|p| p.version_objetivo == crate::version_programa() && !p.resultado().exists())
}

fn anotar_sana(como: &str) {
    let Some(plan) = plan_para_mi() else { return };
    if plan.salud().exists() {
        return;
    }
    let s = Salud { version: crate::version_programa().to_string(), sano: true, cuando: ahora_rfc(), como: como.into() };
    if escribir_json(&plan.salud(), &s).is_ok() {
        crate::agent::log(&format!("Actualizaciones: la versión {} está sana ({como}).", s.version));
    }
}

/// El canal con una consola se abrió y la consola demostró quién es.
pub fn canal_abierto() {
    anotar_sana("canal con una consola");
}

/// Sin consolas (modo local): sana si lee su configuración y el restic que lleva responde.
fn salud_sin_consolas() {
    if plan_para_mi().is_none() || crate::servidor_v2::cargar().is_some_and(|v| v.modo == "gestionado" && !v.vistas().is_empty()) {
        return;
    }
    let _ = crate::agent::load_config();
    if resguardo_motor::restic::version().is_ok() {
        anotar_sana("comprobación local");
    }
}

/// ¿Hay una actualización en marcha (lanzada hace poco y sin resultado)?
fn en_curso() -> bool {
    let Some(plan) = leer_plan(&carpeta()) else { return false };
    if plan.resultado().exists() {
        return false;
    }
    let reciente = chrono::DateTime::parse_from_rfc3339(&plan.creado).is_ok_and(|t| chrono::Utc::now().timestamp() - t.timestamp() < ACTUALIZANDO_MAX);
    if !reciente {
        // Se perdió (p. ej. se apagó el equipo a mitad): lo que corre es lo que quedó.
        let ahora_es = crate::version_programa();
        let estado = if ahora_es == plan.version_objetivo { "actualizada" } else { "fallida" };
        crate::agent::log(&format!("Actualizaciones: la actualización a la {} no terminó; queda la {ahora_es}.", plan.version_objetivo));
        cambiar(|e| {
            e.estado = estado.into();
            e.motivo = Some("interrumpida".into());
            e.cuando = Some(ahora_rfc());
        });
        let _ = std::fs::remove_file(carpeta().join("plan.json"));
        return false;
    }
    true
}

// ---------------------------------------------------------------------------
// El resultado del actualizador
// ---------------------------------------------------------------------------

/// `resultado.json`, lo que deja el actualizador.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Resultado {
    /// `actualizada`, `vuelta_atras` o `vuelta_atras_fallida`.
    pub estado: String,
    pub version_anterior: String,
    pub version_objetivo: String,
    #[serde(default)]
    pub motivo: Option<String>,
    pub cuando: String,
}

/// Lo que dejó el actualizador: se anota y, si volvió atrás, se avisa a todas las consolas.
fn procesar_resultado() {
    let dir = carpeta();
    let Some(r) = std::fs::read(dir.join("resultado.json")).ok().and_then(|b| serde_json::from_slice::<Resultado>(&b).ok()) else { return };
    let enlaces = crate::servidor_v2::cargar().map(|v| v.ids_enlaces()).unwrap_or_default();
    crate::agent::log(&format!(
        "Actualizaciones: {} → {}: {}{}",
        r.version_anterior,
        r.version_objetivo,
        r.estado,
        r.motivo.as_deref().map(|m| format!(" ({m})")).unwrap_or_default()
    ));
    cambiar(|e| {
        e.cuando = Some(r.cuando.clone());
        e.version_objetivo = Some(r.version_objetivo.clone());
        if r.estado == "actualizada" {
            e.estado = "actualizada".into();
            e.motivo = None;
            e.mensaje = None;
            e.version_disponible = None;
            e.version_fallida = None;
        } else {
            e.estado = "vuelta_atras".into();
            e.motivo = Some(r.estado.clone());
            e.mensaje = r.motivo.clone();
            e.version_fallida = Some(r.version_objetivo.clone());
            if !e.fallidas.contains(&r.version_objetivo) {
                e.fallidas.push(r.version_objetivo.clone());
            }
            e.aviso_para = enlaces.clone();
        }
    });
    if r.estado != "actualizada" {
        let nombre = crate::web::default_device_name();
        crate::bitacora::aviso(
            "actualizacion_fallida",
            &format!("{nombre}: la versión {} no estuvo sana y volvió a la {}. {}", r.version_objetivo, r.version_anterior, r.motivo.as_deref().unwrap_or("")),
        );
    }
    for f in ["resultado.json", "plan.json", "salud.json", "descarga.part"] {
        let _ = std::fs::remove_file(dir.join(f));
    }
    // El instalador o el paquete ya no hacen falta (anterior/ se queda hasta la próxima).
    if let Ok(it) = std::fs::read_dir(&dir) {
        for e in it.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.ends_with(".exe") && n != "actualizador.exe" || n.ends_with(".tar.gz") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    // Las consolas lo ven ya.
    if let Some(v) = crate::servidor_v2::cargar() {
        for id in v.ids_enlaces() {
            let _ = crate::servidor_v2::enviar_informe(&id);
        }
    }
}

/// El aviso `actualizacion_fallida` a cada consola que falte (una vez a cada una).
fn mandar_avisos() {
    let e = leer_estado();
    if e.aviso_para.is_empty() {
        return;
    }
    let Some(v) = crate::servidor_v2::cargar() else { return };
    let mensaje = format!(
        "La versión {} no estuvo sana y el equipo volvió a la {}. {}",
        e.version_fallida.as_deref().unwrap_or("?"),
        crate::version_programa(),
        e.mensaje.as_deref().unwrap_or("")
    );
    let mut hechos: Vec<String> = Vec::new();
    for w in v.vistas() {
        let id = w.id_enlace();
        if !e.aviso_para.contains(&id) {
            continue;
        }
        let cuerpo = json!({ "tipo": "actualizacion_fallida", "mensaje": mensaje.trim() });
        match crate::servidor_v2::llamar(&w, "POST", "/api/agente/aviso", Some(&cuerpo)) {
            Ok((s, _)) if s < 300 => hechos.push(id),
            // Una consola anterior no conoce el tipo: como cambio inusual.
            Ok((400 | 422, _)) => {
                let _ = crate::servidor_v2::llamar(&w, "POST", "/api/agente/aviso", Some(&json!({ "tipo": "cambio_inusual", "mensaje": mensaje.trim() })));
                hechos.push(id);
            }
            _ => {}
        }
    }
    let quedan: Vec<String> = v.ids_enlaces();
    cambiar(|e| e.aviso_para.retain(|x| !hechos.contains(x) && quedan.contains(x)));
}

// ---------------------------------------------------------------------------
// El actualizador (`--actualizar-agente <plan.json>`)
// ---------------------------------------------------------------------------

/// Lo que cambia de verdad el programa (Windows, Linux o una falsa en las pruebas).
pub trait Plataforma {
    fn instalar(&self, plan: &Plan) -> Result<(), String>;
    fn volver_atras(&self, plan: &Plan) -> Result<(), String>;
    /// ¿La versión nueva dijo que está sana?
    fn sana(&self, plan: &Plan) -> bool {
        std::fs::read(plan.salud()).ok().and_then(|b| serde_json::from_slice::<Salud>(&b).ok()).is_some_and(|s| s.sano && s.version == plan.version_objetivo)
    }
    fn dormir(&self, d: Duration) {
        std::thread::sleep(d);
    }
}

/// Cada cuánto mira la salud.
const PASO_SALUD: Duration = Duration::from_secs(5);

/// Instala, espera la salud y, si no llega, vuelve atrás.
pub fn ejecutar(plan: &Plan, pl: &dyn Plataforma) -> Resultado {
    let resultado = |estado: &str, motivo: Option<String>| Resultado {
        estado: estado.into(),
        version_anterior: plan.version_anterior.clone(),
        version_objetivo: plan.version_objetivo.clone(),
        motivo,
        cuando: ahora_rfc(),
    };
    let volver = |motivo: String| match pl.volver_atras(plan) {
        Ok(()) => resultado("vuelta_atras", Some(motivo)),
        Err(e) => resultado("vuelta_atras_fallida", Some(format!("{motivo} Y no se pudo volver a la anterior: {e}"))),
    };
    if let Err(e) = pl.instalar(plan) {
        return volver(format!("No se pudo instalar: {e}"));
    }
    let mut esperado = Duration::ZERO;
    while esperado < Duration::from_secs(plan.plazo_s) {
        if pl.sana(plan) {
            return resultado("actualizada", None);
        }
        let paso = PASO_SALUD.min(Duration::from_secs(plan.plazo_s));
        pl.dormir(paso);
        esperado += paso;
    }
    if pl.sana(plan) {
        return resultado("actualizada", None);
    }
    volver(format!("La versión {} no dijo que estaba sana en {} minutos.", plan.version_objetivo, plan.plazo_s.div_ceil(60)))
}

/// Registro del actualizador (en su carpeta: no usa la del agente).
fn log_actualizador(plan_dir: &Path, linea: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(plan_dir.join("actualizador.log")) {
        let _ = writeln!(f, "{} {linea}", ahora_rfc());
    }
}

/// `resguardo-agente --actualizar-agente <plan.json>` (lo lanza el servicio).
pub fn actualizador_main(plan_json: &str) -> i32 {
    let ruta = PathBuf::from(plan_json);
    let Some(plan) = ruta.parent().and_then(leer_plan) else { return 2 };
    // El plan tiene que estar en la carpeta privada que dice (nunca en otra).
    if ruta.parent() != Some(plan.carpeta.as_path()) || !plan.archivo.starts_with(&plan.carpeta) {
        return 2;
    }
    log_actualizador(&plan.carpeta, &format!("Actualizando de {} a {}.", plan.version_anterior, plan.version_objetivo));
    let r = ejecutar(&plan, &Real);
    log_actualizador(&plan.carpeta, &format!("Resultado: {}{}", r.estado, r.motivo.as_deref().map(|m| format!(" ({m})")).unwrap_or_default()));
    if escribir_json(&plan.resultado(), &r).is_err() {
        return 1;
    }
    i32::from(r.estado != "actualizada")
}

/// La de mentira de las pruebas e2e (compilaciones de desarrollo): no toca el programa.
struct Simulada {
    modo: String,
}

impl Plataforma for Simulada {
    fn instalar(&self, plan: &Plan) -> Result<(), String> {
        let datos = std::fs::read(&plan.archivo).map_err(|e| e.to_string())?;
        if hex(&Sha256::digest(&datos)) != plan.sha256 {
            return Err("SHA-256 distinto".into());
        }
        let _ = escribir_json(&plan.carpeta.join("simulada.json"), &json!({ "version": plan.version_objetivo, "sha256": plan.sha256, "modo": self.modo }));
        if self.modo == "ok" {
            let s = Salud { version: plan.version_objetivo.clone(), sano: true, cuando: ahora_rfc(), como: "simulada".into() };
            escribir_json(&plan.salud(), &s)?;
        }
        Ok(())
    }
    fn volver_atras(&self, _plan: &Plan) -> Result<(), String> {
        Ok(())
    }
    fn dormir(&self, d: Duration) {
        std::thread::sleep(d.min(Duration::from_millis(200)));
    }
}

/// La de verdad.
struct Real;

/// Abre el archivo sin dejar que nadie lo cambie mientras esté abierto y comprueba su SHA-256.
fn abrir_comprobado(plan: &Plan) -> Result<std::fs::File, String> {
    use std::io::{Read, Seek};
    let mut o = std::fs::OpenOptions::new();
    o.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_SHARE_READ: se puede leer y ejecutar, nadie puede escribirlo ni borrarlo.
        o.share_mode(0x0000_0001);
    }
    let mut f = o.open(&plan.archivo).map_err(|e| format!("No se pudo abrir {}: {e}", plan.archivo.display()))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut n: u64 = 0;
    loop {
        let l = f.read(&mut buf).map_err(|e| e.to_string())?;
        if l == 0 {
            break;
        }
        n += l as u64;
        h.update(&buf[..l]);
    }
    if n != plan.tamano || !hex(&h.finalize()).eq_ignore_ascii_case(&plan.sha256) {
        return Err("El archivo descargado cambió: no coincide con el manifiesto firmado.".into());
    }
    f.rewind().map_err(|e| e.to_string())?;
    Ok(f)
}

#[cfg(windows)]
impl Plataforma for Real {
    fn instalar(&self, plan: &Plan) -> Result<(), String> {
        use std::os::windows::process::CommandExt;
        // Abierto (sin permitir escribir) hasta que termina el instalador.
        let _abierto = abrir_comprobado(plan)?;
        let tmp = plan.carpeta.join("tmp");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
        let estado = std::process::Command::new(&plan.archivo)
            .args(["/S", "/ACTUALIZACION=1"])
            .env("TEMP", &tmp)
            .env("TMP", &tmp)
            .current_dir(&plan.carpeta)
            .stdin(std::process::Stdio::null())
            .creation_flags(0x0800_0000)
            .status()
            .map_err(|e| format!("No se pudo ejecutar el instalador: {e}"))?;
        let _ = std::fs::remove_dir_all(&tmp);
        if !estado.success() {
            return Err(format!("El instalador terminó con el código {}.", estado.code().unwrap_or(-1)));
        }
        Ok(())
    }

    fn volver_atras(&self, plan: &Plan) -> Result<(), String> {
        use std::os::windows::process::CommandExt;
        let sistema = |exe: &str, args: &[&str]| {
            std::process::Command::new(PathBuf::from(crate::platform::system_dir()).join(exe))
                .args(args)
                .stdin(std::process::Stdio::null())
                .creation_flags(0x0800_0000)
                .status()
        };
        let _ = sistema("sc.exe", &["stop", crate::agente::SERVICE_NAME]);
        std::thread::sleep(Duration::from_secs(5));
        // El actualizador se llama de otra forma: esto no lo termina a él.
        let _ = sistema("taskkill.exe", &["/F", "/IM", "resguardo-agente.exe"]);
        std::thread::sleep(Duration::from_secs(2));
        devolver_programas(plan)?;
        let _ = sistema(
            "reg.exe",
            &[
                "add",
                r"HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall\ResguardoAgente",
                "/v",
                "DisplayVersion",
                "/d",
                &plan.version_anterior,
                "/f",
                "/reg:64",
            ],
        );
        let r = sistema("sc.exe", &["start", crate::agente::SERVICE_NAME]).map_err(|e| e.to_string())?;
        // 1056: ya estaba en marcha.
        if !r.success() && r.code() != Some(1056) {
            return Err(format!("No se pudo arrancar el servicio (código {}).", r.code().unwrap_or(-1)));
        }
        Ok(())
    }
}

#[cfg(unix)]
impl Plataforma for Real {
    fn instalar(&self, plan: &Plan) -> Result<(), String> {
        let f = abrir_comprobado(plan)?;
        drop(f);
        let nueva = plan.carpeta.join("nueva");
        let _ = std::fs::remove_dir_all(&nueva);
        std::fs::create_dir_all(&nueva).map_err(|e| e.to_string())?;
        let tar = ["/usr/bin/tar", "/bin/tar"].into_iter().find(|r| Path::new(r).is_file()).ok_or("Falta tar.")?;
        let ok = std::process::Command::new(tar)
            .args(["--no-same-owner", "--no-same-permissions", "-xzf"])
            .arg(&plan.archivo)
            .arg("-C")
            .arg(&nueva)
            .stdin(std::process::Stdio::null())
            .status()
            .map_err(|e| format!("No se pudo extraer el paquete: {e}"))?
            .success();
        // Se comprobó antes de extraer; por si acaso, otra vez (nadie lo cambió entretanto).
        abrir_comprobado(plan)?;
        if !ok || !nueva.join("resguardo-agente").is_file() {
            return Err("El paquete no se pudo extraer o no trae resguardo-agente.".into());
        }
        comprobar_carpeta_root(&plan.instalacion)?;
        for n in programas(&plan.plataforma) {
            let de = nueva.join(n);
            if std::fs::symlink_metadata(&de).is_ok_and(|m| m.is_file()) {
                poner_archivo(&de, &plan.instalacion.join(n), *n != "VERSION")?;
            }
        }
        let _ = std::fs::remove_dir_all(&nueva);
        systemctl(&["restart", "resguardo-agente.service"])
    }

    fn volver_atras(&self, plan: &Plan) -> Result<(), String> {
        devolver_programas(plan)?;
        systemctl(&["restart", "resguardo-agente.service"])
    }
}

#[cfg(not(any(windows, unix)))]
impl Plataforma for Real {
    fn instalar(&self, _plan: &Plan) -> Result<(), String> {
        Err("Sistema no compatible.".into())
    }
    fn volver_atras(&self, _plan: &Plan) -> Result<(), String> {
        Err("Sistema no compatible.".into())
    }
}

#[cfg(unix)]
fn systemctl(args: &[&str]) -> Result<(), String> {
    let bin = ["/usr/bin/systemctl", "/bin/systemctl"].into_iter().find(|r| Path::new(r).is_file()).ok_or("Falta systemctl.")?;
    let ok = std::process::Command::new(bin).args(args).stdin(std::process::Stdio::null()).status().map_err(|e| e.to_string())?.success();
    if ok {
        Ok(())
    } else {
        Err(format!("systemctl {} ha fallado.", args.join(" ")))
    }
}

/// La carpeta del programa tiene que ser de root y no escribible por otros.
#[cfg(unix)]
fn comprobar_carpeta_root(d: &Path) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;
    let m = std::fs::symlink_metadata(d).map_err(|e| e.to_string())?;
    if !m.is_dir() || m.uid() != 0 || m.mode() & 0o022 != 0 {
        return Err(format!("{} no es una carpeta de root sin permiso de escritura para otros: no se toca.", d.display()));
    }
    Ok(())
}

/// Pone `de` en `a` de una vez: un archivo nuevo al lado (`.<nombre>.nuevo`) y renombrar.
pub fn poner_archivo(de: &Path, a: &Path, ejecutable: bool) -> Result<(), String> {
    use std::io::Write;
    let nombre = a.file_name().and_then(|n| n.to_str()).ok_or("Nombre no válido.")?;
    let tmp = a.with_file_name(format!(".{nombre}.nuevo"));
    let _ = std::fs::remove_file(&tmp);
    let r = (|| {
        let datos = std::fs::read(de).map_err(|e| e.to_string())?;
        let mut o = std::fs::OpenOptions::new();
        o.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut o, if ejecutable { 0o755 } else { 0o644 });
        let mut f = o.open(&tmp).map_err(|e| e.to_string())?;
        f.write_all(&datos).and_then(|()| f.sync_all()).map_err(|e| e.to_string())?;
        drop(f);
        #[cfg(windows)]
        {
            // En Windows no se puede renombrar encima de un programa en uso: se aparta antes.
            let fuera = a.with_file_name(format!("{nombre}.fallida"));
            let _ = std::fs::remove_file(&fuera);
            if a.exists() {
                std::fs::rename(a, &fuera).map_err(|e| e.to_string())?;
            }
        }
        let _ = ejecutable;
        std::fs::rename(&tmp, a).map_err(|e| e.to_string())
    })();
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    r.map_err(|e| format!("No se pudo poner {}: {e}", a.display()))
}

/// Devuelve los programas de `anterior/` a su sitio.
pub fn devolver_programas(plan: &Plan) -> Result<(), String> {
    let mut errores = Vec::new();
    for n in programas(&plan.plataforma) {
        let de = plan.anterior().join(n);
        if de.is_file() {
            if let Err(e) = poner_archivo(&de, &plan.instalacion.join(n), *n != "VERSION") {
                errores.push(e);
            }
        }
    }
    if errores.is_empty() {
        Ok(())
    } else {
        Err(errores.join(" · "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use resguardo_protocolo::publicacion::pruebas as pp;
    use std::cell::{Cell, RefCell};

    fn plan(dir: &Path, plazo_s: u64) -> Plan {
        Plan {
            version_anterior: "0.7.24".into(),
            version_objetivo: "0.7.25".into(),
            plataforma: "linux-x86_64".into(),
            archivo: dir.join("agente.tar.gz"),
            sha256: "0".repeat(64),
            tamano: 1,
            carpeta: dir.to_path_buf(),
            instalacion: dir.join("opt"),
            plazo_s,
            creado: ahora_rfc(),
        }
    }

    /// Una plataforma de mentira: cuenta lo que se le pide y dice «sana» cuando toca.
    struct Falsa {
        instala: Result<(), String>,
        vuelve: Result<(), String>,
        /// Sana tras tantas comprobaciones (None: nunca).
        sana_en: Option<u32>,
        miradas: Cell<u32>,
        dormido: Cell<u64>,
        pasos: RefCell<Vec<&'static str>>,
    }

    impl Falsa {
        fn nueva(instala: Result<(), String>, vuelve: Result<(), String>, sana_en: Option<u32>) -> Self {
            Falsa { instala, vuelve, sana_en, miradas: Cell::new(0), dormido: Cell::new(0), pasos: RefCell::new(Vec::new()) }
        }
    }

    impl Plataforma for Falsa {
        fn instalar(&self, _: &Plan) -> Result<(), String> {
            self.pasos.borrow_mut().push("instalar");
            self.instala.clone()
        }
        fn volver_atras(&self, _: &Plan) -> Result<(), String> {
            self.pasos.borrow_mut().push("volver");
            self.vuelve.clone()
        }
        fn sana(&self, _: &Plan) -> bool {
            self.miradas.set(self.miradas.get() + 1);
            self.sana_en.is_some_and(|n| self.miradas.get() >= n)
        }
        fn dormir(&self, d: Duration) {
            self.dormido.set(self.dormido.get() + d.as_secs());
        }
    }

    #[test]
    fn instala_y_espera_la_salud() {
        let d = tempfile::tempdir().unwrap();
        let f = Falsa::nueva(Ok(()), Ok(()), Some(3));
        let r = ejecutar(&plan(d.path(), 600), &f);
        assert_eq!(r.estado, "actualizada");
        assert_eq!(*f.pasos.borrow(), ["instalar"]);
        assert_eq!(f.dormido.get(), 10, "dos esperas de 5 s");
    }

    #[test]
    fn sin_salud_en_10_minutos_vuelve_atras() {
        let d = tempfile::tempdir().unwrap();
        let f = Falsa::nueva(Ok(()), Ok(()), None);
        let r = ejecutar(&plan(d.path(), 600), &f);
        assert_eq!(r.estado, "vuelta_atras");
        assert!(r.motivo.unwrap().contains("10 minutos"));
        assert_eq!(*f.pasos.borrow(), ["instalar", "volver"]);
        assert_eq!(f.dormido.get(), 600, "esperó el plazo entero");
    }

    #[test]
    fn si_falla_el_instalador_vuelve_atras_sin_esperar() {
        let d = tempfile::tempdir().unwrap();
        let f = Falsa::nueva(Err("código 4".into()), Ok(()), Some(1));
        let r = ejecutar(&plan(d.path(), 600), &f);
        assert_eq!((r.estado.as_str(), f.dormido.get()), ("vuelta_atras", 0));
        assert!(r.motivo.unwrap().contains("código 4"));
        // Y si tampoco puede volver, lo dice.
        let f = Falsa::nueva(Ok(()), Err("sin permiso".into()), None);
        let r = ejecutar(&plan(d.path(), 30), &f);
        assert_eq!(r.estado, "vuelta_atras_fallida");
        assert!(r.motivo.unwrap().contains("sin permiso"));
    }

    #[test]
    fn la_salud_es_la_de_la_version_nueva() {
        let d = tempfile::tempdir().unwrap();
        let pl = plan(d.path(), 10);
        assert!(!Real.sana(&pl));
        escribir_json(&pl.salud(), &Salud { version: "0.7.24".into(), sano: true, cuando: ahora_rfc(), como: String::new() }).unwrap();
        assert!(!Real.sana(&pl), "la anterior no cuenta");
        escribir_json(&pl.salud(), &Salud { version: "0.7.25".into(), sano: false, cuando: ahora_rfc(), como: String::new() }).unwrap();
        assert!(!Real.sana(&pl));
        escribir_json(&pl.salud(), &Salud { version: "0.7.25".into(), sano: true, cuando: ahora_rfc(), como: String::new() }).unwrap();
        assert!(Real.sana(&pl));
    }

    #[test]
    fn descarga_con_tamano_y_hash() {
        let d = tempfile::tempdir().unwrap();
        let datos = b"paquete de la version nueva".to_vec();
        let m = pp::manifiesto("0.7.25", &[("linux-x86_64", "agente.tar.gz", &datos)]);
        let a = m.archivos[0].clone();
        let destino = d.path().join("descarga.part");
        guardar_comprobado(&datos[..], &destino, &a).unwrap();
        assert_eq!(std::fs::read(&destino).unwrap(), datos);
        // Cambiado (mismo tamaño), más corto o más largo: fuera, sin dejar nada.
        let mut tocado = datos.clone();
        tocado[0] ^= 1;
        assert!(guardar_comprobado(&tocado[..], &destino, &a).unwrap_err().contains("SHA-256"));
        assert!(!destino.exists());
        assert!(guardar_comprobado(&datos[..5], &destino, &a).unwrap_err().contains("tamaño"));
        let mut largo = datos.clone();
        largo.extend_from_slice(b"de mas");
        assert!(guardar_comprobado(&largo[..], &destino, &a).unwrap_err().contains("más grande"));
        assert!(!destino.exists());
    }

    #[test]
    fn respuesta_de_una_consola() {
        let llaves = pp::llaves_a();
        let m = pp::manifiesto("0.7.25", &[("linux-x86_64", "agente.tar.gz", b"x")]);
        let texto = serde_json::to_string(&m).unwrap();
        let firma = pp::firmar_a(texto.as_bytes());
        let r = json!({ "politica": { "modo": "auto", "anillo": "prueba" }, "publicacion": { "manifiesto": texto, "firma": firma, "archivos": "/api/agente/actualizacion/archivos/0.7.25/" } });
        let (pol, o) = leer_respuesta(&r, &llaves, &[], "principal", "Oficina");
        assert_eq!(pol.unwrap().anillo, "prueba");
        let o = o.unwrap().unwrap();
        assert_eq!(o.verificado.manifiesto.version, "0.7.25");
        // Firma de otra llave, o revocada: no vale (pero la política sí).
        let r2 = json!({ "politica": {}, "publicacion": { "manifiesto": texto, "firma": pp::firmar_b(texto.as_bytes()), "archivos": "/api/agente/actualizacion/archivos/0.7.25/" } });
        let (pol, o) = leer_respuesta(&r2, &llaves, &[], "principal", "Oficina");
        assert!(pol.is_some() && o.is_err());
        assert!(leer_respuesta(&r, &llaves, &[pp::ID_A.into()], "principal", "Oficina").1.is_err());
        // Una ruta de archivos que no es la de la consola: no.
        let r3 = json!({ "publicacion": { "manifiesto": texto, "firma": firma, "archivos": "https://otro.ejemplo.com/" } });
        assert!(leer_respuesta(&r3, &llaves, &[], "principal", "Oficina").1.is_err());
        let r4 = json!({ "publicacion": { "manifiesto": texto, "firma": firma, "archivos": "/api/agente/actualizacion/archivos/../../datos/" } });
        assert!(leer_respuesta(&r4, &llaves, &[], "principal", "Oficina").1.is_err());
        // Sin publicación: solo la política.
        assert!(leer_respuesta(&json!({ "politica": {} }), &llaves, &[], "p", "x").1.unwrap().is_none());
    }

    #[test]
    fn elige_la_mas_nueva_y_a_igualdad_la_consola() {
        let o = |v: &str, consola: bool| Oferta {
            verificado: Verificado { manifiesto: pp::manifiesto(v, &[("linux-x86_64", "a.tar.gz", b"x")]), llave: pp::ID_A.into() },
            origen: if consola { Origen::Consola { enlace: "p".into(), nombre: "Oficina".into(), ruta: RUTA_ARCHIVOS.into() } } else { Origen::GitHub },
        };
        let e = elegir(vec![o("0.7.25", false), o("0.7.26", true), o("0.7.24", true)]).unwrap();
        assert_eq!(e.verificado.manifiesto.version, "0.7.26");
        let e = elegir(vec![o("0.7.26", false), o("0.7.26", true)]).unwrap();
        assert!(matches!(e.origen, Origen::Consola { .. }));
        assert!(elegir(vec![]).is_none());
    }

    #[test]
    fn guardar_y_devolver_los_programas() {
        let d = tempfile::tempdir().unwrap();
        let mut pl = plan(d.path(), 10);
        if cfg!(windows) {
            pl.plataforma = "windows-x86_64".into();
        }
        std::fs::create_dir_all(&pl.instalacion).unwrap();
        // Sin el programa instalado no se puede guardar la anterior.
        assert!(guardar_anterior(&pl).is_err());
        let principal = programas(&pl.plataforma)[0];
        std::fs::write(pl.instalacion.join(principal), b"version vieja").unwrap();
        std::fs::write(pl.instalacion.join(programas(&pl.plataforma)[1]), b"restic viejo").unwrap();
        guardar_anterior(&pl).unwrap();
        // La «nueva» se instala encima…
        std::fs::write(pl.instalacion.join(principal), b"version nueva").unwrap();
        // …y se vuelve atrás.
        devolver_programas(&pl).unwrap();
        assert_eq!(std::fs::read(pl.instalacion.join(principal)).unwrap(), b"version vieja");
        assert_eq!(std::fs::read(pl.instalacion.join(programas(&pl.plataforma)[1])).unwrap(), b"restic viejo");
        let restos: Vec<String> = std::fs::read_dir(&pl.instalacion).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert!(!restos.iter().any(|n| n.ends_with(".nuevo")), "{restos:?}");
    }

    #[test]
    fn el_actualizador_solo_acepta_su_plan() {
        let d = tempfile::tempdir().unwrap();
        // Un plan que apunta a un archivo fuera de su carpeta: no se ejecuta.
        let mut pl = plan(d.path(), 1);
        pl.archivo = std::env::temp_dir().join("otro.exe");
        escribir_json(&d.path().join("plan.json"), &pl).unwrap();
        assert_eq!(actualizador_main(&d.path().join("plan.json").to_string_lossy()), 2);
        assert!(!pl.resultado().exists());
        // Sin plan, tampoco.
        assert_eq!(actualizador_main(&d.path().join("no.json").to_string_lossy()), 2);
    }
}
