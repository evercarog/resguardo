//! Desconectar una nube del equipo (plan 0.7.26, 1.1; orden `quitar_nube`,
//! `admite: "nube_revocar"`).
//!
//! - **No deja** si la usa algo de este equipo: un repositorio que está en ella,
//!   una copia externa o derivada que sube a ella o el espejo del almacén. Lo
//!   dice (cuál) y no toca nada.
//! - **Borra al momento** las credenciales selladas (`nubes.bin`) y los restos:
//!   los archivos de configuración de rclone de sus vueltas (los suyos llevan
//!   una huella de su nombre), el compartido de versiones anteriores y el de
//!   `nube conectar`. Si ya no queda ninguna nube, la carpeta de vueltas entera.
//! - **Anula el permiso en el proveedor** cuando se puede:
//!   - Dropbox: `POST /2/auth/token/revoke` con su access token (renovado antes
//!     si caducó). Dropbox anula ese token y su refresh token: solo este equipo,
//!     no los demás que tengan conectada la misma cuenta.
//!   - Si no se puede ahora (sin internet, Dropbox caído, un error suyo), las
//!     credenciales se borran igual del equipo y la anulación queda **apuntada**
//!     (sellada, solo lo necesario para anular) y se reintenta con espera
//!     creciente durante 7 días ([`reintentar`], en cada vuelta del servicio).
//!     Si vence, se olvida y se manda el aviso `nube_sin_anular`.
//!   - Si Dropbox ya no aceptaba el permiso: «el permiso ya no era válido».
//!   - Google Drive (conectada con `rclone authorize`, la app de rclone): no se
//!     anula desde aquí. Google retira el permiso de la app entera para esa
//!     cuenta, así que dejaría sin acceso a todos los equipos (y a cualquier
//!     rclone) que la usen. Se dice dónde quitarlo.
//!   - B2, S3, SFTP, SMB, WebDAV: no hay un permiso que anular; la clave o la
//!     contraseña sigue valiendo en el proveedor y se dice.
//! - Nunca se registra ni se devuelve un token (ni la app key).

use crate::nube::{FalloRenovar, Nube};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Lo apuntado para reintentar (sellado como `nubes.bin`).
const ARCHIVO: &str = "nubes-por-anular.bin";
/// Cuánto se reintenta una anulación que no se pudo hacer.
pub const PLAZO_S: i64 = 7 * 24 * 3600;
/// Espera antes del primer reintento; se dobla en cada uno, hasta [`ESPERA_MAX_S`].
const ESPERA_S: i64 = 5 * 60;
const ESPERA_MAX_S: i64 = 6 * 3600;

/// Dónde se anula y se renueva (en las pruebas, un servidor de mentira en 127.0.0.1).
pub struct Urls {
    pub revocar: String,
    pub token: String,
}

impl Urls {
    /// Las de Dropbox. En las pruebas, un puerto cerrado de 127.0.0.1: una
    /// prueba nunca habla con Dropbox de verdad, ni por descuido.
    pub fn dropbox() -> Self {
        if cfg!(test) {
            return Urls { revocar: "http://127.0.0.1:9/2/auth/token/revoke".into(), token: "http://127.0.0.1:9/oauth2/token".into() };
        }
        Urls { revocar: "https://api.dropboxapi.com/2/auth/token/revoke".into(), token: "https://api.dropboxapi.com/oauth2/token".into() }
    }
}

/// Qué pasó con el permiso en el proveedor.
#[derive(Debug, PartialEq)]
pub enum Anulacion {
    /// Anulado.
    Anulado,
    /// El proveedor ya no lo aceptaba (se quitó antes, caducó).
    YaNoValia,
    /// No se pudo ahora: se reintenta (el motivo, sin datos del proveedor).
    Reintentar(String),
    /// Desde aquí no se puede: qué hacer (en la web del proveedor).
    SigueVivo(String),
}

/// Una anulación pendiente: solo lo necesario para anular. Sin `Debug` con datos.
#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct PorAnular {
    pub nombre: String,
    pub tipo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    app_key: Option<String>,
    #[serde(default)]
    refresh: String,
    #[serde(default)]
    access: String,
    /// Cuándo caduca `access` (segundos Unix).
    #[serde(default)]
    expira: i64,
    /// Desde cuándo se intenta (segundos Unix).
    pub desde: i64,
    pub intentos: u32,
    /// Cuándo toca el próximo intento.
    pub proximo: i64,
}

impl std::fmt::Debug for PorAnular {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PorAnular").field("nombre", &self.nombre).field("tipo", &self.tipo).field("intentos", &self.intentos).finish_non_exhaustive()
    }
}

fn ruta() -> std::path::PathBuf {
    crate::agent::private_dir().join(ARCHIVO)
}

pub fn pendientes() -> Vec<PorAnular> {
    std::fs::read(ruta()).ok().and_then(|b| crate::platform::unprotect(&b).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn guardar_pendientes(l: &[PorAnular]) -> Result<(), String> {
    if l.is_empty() {
        return match std::fs::remove_file(ruta()) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        };
    }
    crate::agent::prepare_dir()?;
    let datos = crate::platform::protect(&serde_json::to_vec(l).map_err(|e| e.to_string())?)?;
    let tmp = crate::agent::private_dir().join(format!("{ARCHIVO}.tmp"));
    let _ = std::fs::remove_file(&tmp);
    std::fs::write(&tmp, datos).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, ruta()).map_err(|e| e.to_string())
}

/// Para el resumen (`resumen.nubes_por_anular`): sin tokens.
pub fn resumen() -> Vec<Value> {
    pendientes()
        .iter()
        .map(|p| {
            let fecha = |s: i64| chrono::DateTime::from_timestamp(s, 0).map(|d| d.to_rfc3339());
            json!({ "nombre": p.nombre, "tipo": p.tipo, "desde": fecha(p.desde), "hasta": fecha(p.desde + PLAZO_S), "intentos": p.intentos })
        })
        .collect()
}

// ---------- qué la usa ----------

/// Qué de este equipo usa la nube `nombre` (para decirlo al no dejar quitarla).
pub fn quien_la_usa(nombre: &str) -> Option<String> {
    let mut usos: Vec<String> = Vec::new();
    if let Some(v) = crate::servidor_v2::cargar() {
        for d in v.destinos.iter().filter(|d| d.tipo == "nube" && d.nube.as_deref() == Some(nombre)) {
            for r in v.repos_v2.iter().filter(|r| r.destino == d.id) {
                usos.push(format!("el repositorio «{}»", r.nombre));
            }
        }
    }
    for r in crate::agent::load_config().repos.iter() {
        if r.offsite.as_ref().is_some_and(|o| o.dest.nube.as_deref() == Some(nombre)) {
            usos.push(format!("la copia externa de «{}»", r.name));
        }
        if r.derived.iter().any(|d| d.offsite.dest.nube.as_deref() == Some(nombre)) {
            usos.push(format!("una copia derivada de «{}»", r.name));
        }
    }
    if crate::nube::usa_espejo(nombre) {
        usos.push("el espejo de este almacén".into());
    }
    usos.dedup();
    match usos.len() {
        0 => None,
        1..=3 => Some(usos.join(", ")),
        n => Some(format!("{} y {} más", usos[..3].join(", "), n - 3)),
    }
}

// ---------- Dropbox ----------

fn agente_http() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(30)))
        .http_status_as_error(false)
        .tls_config(crate::web::tls_sistema())
        .build()
        .new_agent()
}

/// Lo que respondió Dropbox al anular.
#[derive(Debug, PartialEq)]
enum Respuesta {
    Anulado,
    /// 401 con `expired_access_token`: hay que renovar antes.
    Caducado,
    /// 401 por otra cosa: el token ya no vale.
    NoVale,
    Fallo(String),
}

/// `POST /2/auth/token/revoke` con el access token (sin argumentos: cuerpo `null`).
fn revocar_en(url: &str, access: &str) -> Respuesta {
    let r = agente_http().post(url).header("Authorization", &format!("Bearer {access}")).header("Content-Type", "application/json").send("null");
    let mut r = match r {
        Ok(r) => r,
        Err(_) => return Respuesta::Fallo("No se pudo hablar con Dropbox desde este equipo (¿sin internet?).".into()),
    };
    let status = r.status().as_u16();
    let v: Value = r.body_mut().read_json().unwrap_or(Value::Null);
    interpretar_revocacion(status, &v)
}

fn interpretar_revocacion(status: u16, v: &Value) -> Respuesta {
    match status {
        200 => Respuesta::Anulado,
        401 if v["error"][".tag"] == "expired_access_token" => Respuesta::Caducado,
        401 => Respuesta::NoVale,
        429 => Respuesta::Fallo("Dropbox pide esperar (demasiadas peticiones).".into()),
        s if s >= 500 => Respuesta::Fallo("Dropbox no responde ahora.".into()),
        _ => Respuesta::Fallo("Dropbox no aceptó la petición para anular el permiso.".into()),
    }
}

/// Un intento de anular en Dropbox: renueva el access token si caducó (y se
/// puede) y lo anula. Nunca devuelve ni registra el token.
fn intentar(p: &PorAnular, urls: &Urls) -> Anulacion {
    let ahora = chrono::Utc::now().timestamp();
    let renovar = || -> Result<String, Anulacion> {
        match p.app_key.as_deref().filter(|_| !p.refresh.is_empty()) {
            Some(k) => match crate::nube::renovar_dropbox_en(&urls.token, k, &p.refresh) {
                Ok((a, _)) => Ok(a),
                // `invalid_grant`: el permiso ya no existe en Dropbox.
                Err(FalloRenovar::Rechazado(_)) => Err(Anulacion::YaNoValia),
                Err(FalloRenovar::SinConexion(m)) => Err(Anulacion::Reintentar(m)),
            },
            // Conectada con `rclone authorize` (la app de rclone): el agente no puede renovarla.
            None => Err(Anulacion::SigueVivo(SIGUE_VIVO_DROPBOX.into())),
        }
    };
    let mut access = p.access.clone();
    let mut renovado = false;
    if access.is_empty() || p.expira <= ahora + 60 {
        match renovar() {
            Ok(a) => (access, renovado) = (a, true),
            Err(e) => return e,
        }
    }
    loop {
        match revocar_en(&urls.revocar, &access) {
            Respuesta::Anulado => return Anulacion::Anulado,
            Respuesta::NoVale => return Anulacion::YaNoValia,
            Respuesta::Caducado if !renovado => match renovar() {
                Ok(a) => (access, renovado) = (a, true),
                Err(e) => return e,
            },
            Respuesta::Caducado => return Anulacion::Reintentar("Dropbox dio por caducado el permiso recién renovado.".into()),
            Respuesta::Fallo(m) => return Anulacion::Reintentar(m),
        }
    }
}

const SIGUE_VIVO_DROPBOX: &str = "El permiso sigue vivo en Dropbox: quítalo desde la web de Dropbox → Configuración → Aplicaciones conectadas (ojo: allí se quita a todos los equipos que la usan).";

/// Lo que hace falta para anular, sacado de la nube guardada.
fn por_anular(n: &Nube, ahora: i64) -> PorAnular {
    let t: Value = serde_json::from_str(&n.token).unwrap_or_default();
    let texto = |k: &str| t[k].as_str().filter(|x| crate::nube::token_plausible(x)).unwrap_or("").to_string();
    let expira = t["expiry"].as_str().and_then(|e| chrono::DateTime::parse_from_rfc3339(e).ok()).map_or(0, |d| d.timestamp());
    PorAnular {
        nombre: n.nombre.clone(),
        tipo: n.tipo.clone(),
        app_key: n.app_key.clone(),
        refresh: texto("refresh_token"),
        access: texto("access_token"),
        expira,
        desde: ahora,
        intentos: 0,
        proximo: ahora,
    }
}

/// Qué hacer con el permiso de una nube que se desconecta.
fn anular(n: &Nube, urls: &Urls) -> (Anulacion, Option<PorAnular>) {
    match n.tipo.as_str() {
        "dropbox" => {
            let p = por_anular(n, chrono::Utc::now().timestamp());
            if p.access.is_empty() && p.refresh.is_empty() {
                return (Anulacion::SigueVivo(SIGUE_VIVO_DROPBOX.into()), None);
            }
            let a = intentar(&p, urls);
            let apuntar = matches!(a, Anulacion::Reintentar(_)).then_some(p);
            (a, apuntar)
        }
        "drive" => (
            Anulacion::SigueVivo(
                "El permiso sigue vivo en Google: quítalo en tu cuenta de Google → Seguridad → Aplicaciones de terceros («rclone»; ojo: afecta a todos los equipos que la usan con esa cuenta).".into(),
            ),
            None,
        ),
        "sftp" => (Anulacion::SigueVivo("La contraseña o la llave siguen valiendo en ese servidor: cámbialas o quita ese usuario si ya no lo usa nadie.".into()), None),
        "smb" | "webdav" => (Anulacion::SigueVivo("El usuario y la contraseña siguen valiendo en ese servidor: cámbialos si ya no los usa nadie.".into()), None),
        // B2, S3 y lo demás: una clave de acceso.
        _ => (Anulacion::SigueVivo("La clave sigue valiendo en el proveedor: bórrala o anúlala en su web si ya no la usa nadie.".into()), None),
    }
}

// ---------- desconectar ----------

/// Desconecta la nube `nombre`: no deja si algo la usa; si no, borra sus
/// credenciales y sus restos al momento y anula el permiso (o lo apunta para
/// reintentar). El mensaje nunca lleva un token.
pub fn desconectar(nombre: &str, urls: &Urls) -> Result<String, String> {
    let nubes = crate::nube::cargar();
    let Some(n) = nubes.iter().find(|n| n.nombre == nombre).cloned() else {
        return Err(format!("No hay ninguna nube «{nombre}» en este equipo."));
    };
    if let Some(uso) = quien_la_usa(nombre) {
        return Err(format!("«{nombre}» no se desconecta: la usa {uso}. Quítala antes de ahí (o elige otro destino) y vuelve a intentarlo."));
    }
    let (anulacion, apuntar) = anular(&n, urls);
    // Las credenciales y los restos se borran pase lo que pase con el proveedor.
    let quedan: Vec<Nube> = nubes.into_iter().filter(|x| x.nombre != nombre).collect();
    crate::nube::guardar(&quedan)?;
    borrar_restos(nombre, quedan.is_empty());
    let que = match n.tipo.as_str() {
        "dropbox" => "Dropbox",
        _ => "el proveedor",
    };
    let mut m = match &anulacion {
        Anulacion::Anulado => format!("«{nombre}» desconectada: credenciales borradas de este equipo y permiso anulado en {que}."),
        Anulacion::YaNoValia => format!("«{nombre}» desconectada: credenciales borradas de este equipo; el permiso ya no era válido en {que}."),
        Anulacion::Reintentar(_) => {
            format!(
                "«{nombre}» desconectada. Credenciales borradas de este equipo; no se pudo anular el permiso en {que} todavía: se reintentará (hasta 7 días)."
            )
        }
        Anulacion::SigueVivo(t) => format!("«{nombre}» desconectada: credenciales borradas de este equipo. {t}"),
    };
    if let (Anulacion::Reintentar(_), Some(p)) = (&anulacion, apuntar) {
        let mut l = pendientes();
        l.retain(|x| x.nombre != p.nombre);
        l.push(PorAnular { proximo: p.desde + ESPERA_S, ..p });
        if guardar_pendientes(&l).is_err() {
            m = format!("«{nombre}» desconectada. Credenciales borradas de este equipo; no se pudo anular el permiso en {que} ni apuntarlo para reintentar. {SIGUE_VIVO_DROPBOX}");
        }
    }
    m += " Lo ya subido sigue en la nube.";
    let registro = match &anulacion {
        Anulacion::Anulado => "permiso anulado",
        Anulacion::YaNoValia => "el permiso ya no era válido",
        Anulacion::Reintentar(_) => "anulación pendiente (se reintenta)",
        Anulacion::SigueVivo(_) => "el permiso sigue en el proveedor",
    };
    crate::agent::log(&format!("Nube «{nombre}» ({}) desconectada: credenciales borradas; {registro}.", n.tipo));
    Ok(m)
}

/// Borra lo que pudo quedar de la nube en la carpeta privada: los archivos de
/// sus vueltas (y su known_hosts), el compartido de versiones anteriores y el
/// de `nube conectar`. Sin nubes, toda la carpeta de vueltas.
fn borrar_restos(nombre: &str, ninguna: bool) {
    let privada = crate::agent::private_dir();
    for f in ["rclone-restic.conf", "rclone-vacio.conf"] {
        let _ = std::fs::remove_file(privada.join(f));
    }
    let dir = crate::nube::carpeta_de_vueltas();
    if ninguna {
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    let prefijo = crate::nube::prefijo_de_vuelta(nombre);
    for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        if e.file_name().to_string_lossy().starts_with(&prefijo) && e.file_type().is_ok_and(|t| t.is_file()) {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

// ---------- reintentos ----------

/// En cada vuelta del servicio: reintenta las anulaciones a las que les toca.
pub fn reintentar() {
    if ruta().exists() {
        reintentar_con(&Urls::dropbox(), chrono::Utc::now().timestamp());
    }
}

/// Reintenta las que tocan a la hora `ahora`. Devuelve lo que pasó con cada una
/// (para las pruebas): su nombre y si se quitó de la lista.
pub fn reintentar_con(urls: &Urls, ahora: i64) -> Vec<(String, Anulacion)> {
    let mut l = pendientes();
    if l.is_empty() {
        return Vec::new();
    }
    let mut hecho = Vec::new();
    let mut quedan = Vec::new();
    for mut p in std::mem::take(&mut l) {
        if p.proximo > ahora {
            quedan.push(p);
            continue;
        }
        let a = intentar(&p, urls);
        match &a {
            Anulacion::Anulado | Anulacion::YaNoValia => {
                let que = if a == Anulacion::Anulado { "anulado" } else { "ya no era válido" };
                crate::agent::log(&format!("Nube «{}»: el permiso pendiente de anular, {que}.", p.nombre));
            }
            Anulacion::Reintentar(_) if ahora < p.desde + PLAZO_S => {
                p.intentos += 1;
                let espera = ESPERA_S.saturating_mul(1 << p.intentos.min(12)).min(ESPERA_MAX_S);
                p.proximo = ahora + espera;
                quedan.push(p.clone());
            }
            Anulacion::Reintentar(_) | Anulacion::SigueVivo(_) => {
                let m = format!(
                    "No se pudo anular el permiso de «{}» en Dropbox en 7 días (las credenciales ya se borraron del equipo). Comprueba en la web de Dropbox → Configuración → Aplicaciones conectadas si sigue «Resguardo» y quítalo si no lo usa otro equipo.",
                    p.nombre
                );
                crate::agent::log(&m);
                crate::bitacora::aviso("nube_sin_anular", &m);
            }
        }
        hecho.push((p.nombre.clone(), a));
    }
    if guardar_pendientes(&quedan).is_err() {
        crate::agent::log("No se pudo guardar la lista de permisos de nubes por anular.");
    }
    hecho
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "abc123def456ghi";
    const RT: &str = "rt-secreto-NO-SALE-0123456789";
    const AT: &str = "at-secreto-NO-SALE-9876543210";

    /// Un servidor de mentira en 127.0.0.1 con las dos direcciones de Dropbox
    /// (`/oauth2/token` y `/2/auth/token/revoke`). Cada petición recibe la
    /// siguiente respuesta de `revocar` (`200`, `401`, `401c` = caducado, `500`)
    /// o, para el token, `200`. Guarda lo que llegó (para comprobar la cabecera).
    struct Falso {
        urls: Urls,
        recibido: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }

    fn falso(revocar: &[&str], token: &[&str]) -> Falso {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        let recibido = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let r2 = recibido.clone();
        let mut revocar: std::collections::VecDeque<String> = revocar.iter().map(|s| s.to_string()).collect();
        let mut token: std::collections::VecDeque<String> = token.iter().map(|s| s.to_string()).collect();
        std::thread::spawn(move || {
            for mut s in l.incoming().flatten() {
                let _ = s.set_read_timeout(Some(std::time::Duration::from_millis(500)));
                let mut buf = vec![0u8; 16384];
                let mut n = 0;
                // Lee la cabecera y el cuerpo (puede llegar aparte).
                while let Ok(k) = s.read(&mut buf[n..]) {
                    if k == 0 {
                        break;
                    }
                    n += k;
                    let t = String::from_utf8_lossy(&buf[..n]);
                    if let Some(fin) = t.find("\r\n\r\n") {
                        let largo = t
                            .lines()
                            .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|x| x.trim().parse::<usize>().unwrap_or(0)))
                            .unwrap_or(0);
                        if n >= fin + 4 + largo {
                            break;
                        }
                    }
                }
                let pet = String::from_utf8_lossy(&buf[..n]).into_owned();
                let (status, cuerpo) = if pet.starts_with("POST /oauth2/token") {
                    match token.pop_front().as_deref().unwrap_or("200") {
                        "400" => ("400 Bad Request", r#"{"error":"invalid_grant"}"#.to_string()),
                        "500" => ("500 Internal Server Error", "{}".to_string()),
                        _ => ("200 OK", r#"{"access_token":"renovado-0123456789","token_type":"bearer","expires_in":14400}"#.to_string()),
                    }
                } else {
                    match revocar.pop_front().as_deref().unwrap_or("200") {
                        "401" => ("401 Unauthorized", r#"{"error_summary":"invalid_access_token/","error":{".tag":"invalid_access_token"}}"#.to_string()),
                        "401c" => ("401 Unauthorized", r#"{"error_summary":"expired_access_token/","error":{".tag":"expired_access_token"}}"#.to_string()),
                        "500" => ("500 Internal Server Error", "{}".to_string()),
                        _ => ("200 OK", "null".to_string()),
                    }
                };
                r2.lock().unwrap().push(pet);
                let _ =
                    write!(s, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cuerpo}", cuerpo.len());
            }
        });
        Falso { urls: Urls { revocar: format!("{base}/2/auth/token/revoke"), token: format!("{base}/oauth2/token") }, recibido }
    }

    /// Un puerto de 127.0.0.1 en el que no escucha nadie (sin conexión).
    fn sin_red() -> Urls {
        let puerto = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        Urls { revocar: format!("http://127.0.0.1:{puerto}/2/auth/token/revoke"), token: format!("http://127.0.0.1:{puerto}/oauth2/token") }
    }

    fn dropbox(nombre: &str, horas: i64) -> Nube {
        let expira = chrono::Utc::now() + chrono::Duration::hours(horas);
        let token = json!({ "access_token": AT, "token_type": "bearer", "refresh_token": RT, "expiry": expira.to_rfc3339() }).to_string();
        Nube { nombre: nombre.into(), tipo: "dropbox".into(), token, app_key: Some(KEY.into()), ..Default::default() }
    }

    /// La carpeta del agente en un temporal, para esta prueba.
    struct Carpeta(std::path::PathBuf);
    impl Drop for Carpeta {
        fn drop(&mut self) {
            std::env::remove_var("RESGUARDO_AGENT_DIR");
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn carpeta(que: &str) -> Carpeta {
        let dir = std::env::temp_dir().join(format!("resguardo-anular-{que}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("RESGUARDO_AGENT_DIR", &dir);
        Carpeta(dir)
    }

    fn sin_secretos(m: &str) {
        assert!(!m.contains(RT) && !m.contains(AT) && !m.contains(KEY) && !m.contains("renovado-0123"), "{m}");
    }

    /// Lo que no debe aparecer en el registro del agente.
    fn registro_sin_secretos() {
        let mut texto = String::new();
        for d in [crate::agent::agent_dir(), crate::agent::private_dir()] {
            for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
                let n = e.file_name().to_string_lossy().into_owned();
                if n.ends_with(".log") || n.ends_with(".txt") || n.ends_with(".jsonl") {
                    texto += &std::fs::read_to_string(e.path()).unwrap_or_default();
                }
            }
        }
        sin_secretos(&texto);
    }

    #[test]
    fn respuestas_de_dropbox_al_anular() {
        assert_eq!(interpretar_revocacion(200, &Value::Null), Respuesta::Anulado);
        assert_eq!(interpretar_revocacion(401, &json!({ "error": { ".tag": "expired_access_token" } })), Respuesta::Caducado);
        assert_eq!(interpretar_revocacion(401, &json!({ "error": { ".tag": "invalid_access_token" } })), Respuesta::NoVale);
        assert_eq!(interpretar_revocacion(401, &Value::Null), Respuesta::NoVale);
        for s in [400, 409, 429, 500, 503] {
            assert!(matches!(interpretar_revocacion(s, &Value::Null), Respuesta::Fallo(_)), "{s}");
        }
    }

    #[test]
    fn por_anular_sin_datos_al_depurar() {
        let p = por_anular(&dropbox("Caja", 4), 0);
        assert_eq!((p.refresh.as_str(), p.access.as_str(), p.app_key.as_deref()), (RT, AT, Some(KEY)));
        sin_secretos(&format!("{p:?}"));
    }

    /// Anular de verdad (contra el servidor de mentira): la cabecera lleva el
    /// access token; con el token caducado, se renueva antes; 401 = ya no valía.
    #[test]
    fn desconectar_anula_en_dropbox() {
        let _l = crate::restic::tests::real_repo_lock();
        let _c = carpeta("anula");
        // 1. Token vigente: se anula con él, se borra todo y lo dice.
        let f = falso(&["200"], &[]);
        crate::nube::guardar(&[dropbox("Caja", 4), dropbox("Otra", 4)]).unwrap();
        // Restos: un archivo de una vuelta suya, uno de otra nube y el compartido antiguo.
        let vueltas = crate::nube::carpeta_de_vueltas();
        std::fs::create_dir_all(&vueltas).unwrap();
        let suya = vueltas.join(format!("{}1-0-0.conf", crate::nube::prefijo_de_vuelta("Caja")));
        let suya_hosts = suya.with_extension("hosts");
        let de_otra = vueltas.join(format!("{}1-0-0.conf", crate::nube::prefijo_de_vuelta("Otra")));
        for a in [&suya, &suya_hosts, &de_otra] {
            std::fs::write(a, format!("[rnube]\ntoken = {RT}\n")).unwrap();
        }
        std::fs::write(crate::agent::private_dir().join("rclone-restic.conf"), "x").unwrap();
        let m = desconectar("Caja", &f.urls).unwrap();
        assert!(m.contains("permiso anulado en Dropbox") && m.contains("credenciales borradas"), "{m}");
        sin_secretos(&m);
        let pet = f.recibido.lock().unwrap().join("\n");
        assert!(pet.contains("POST /2/auth/token/revoke") && pet.contains(&format!("Bearer {AT}")), "{pet}");
        assert!(!pet.contains("/oauth2/token"), "vigente: no se renueva");
        assert!(crate::nube::buscar("Caja").is_none() && crate::nube::buscar("Otra").is_some());
        assert!(!suya.exists() && !suya_hosts.exists() && de_otra.exists(), "solo los restos suyos");
        assert!(!crate::agent::private_dir().join("rclone-restic.conf").exists());
        assert!(pendientes().is_empty());

        // 2. Token caducado: se renueva con la app key y se anula el nuevo.
        let f = falso(&["200"], &["200"]);
        crate::nube::guardar(&[dropbox("Caja", -1), dropbox("Otra", 4)]).unwrap();
        let m = desconectar("Caja", &f.urls).unwrap();
        assert!(m.contains("permiso anulado"), "{m}");
        let pet = f.recibido.lock().unwrap().join("\n");
        assert!(pet.contains("POST /oauth2/token") && pet.contains("Bearer renovado-0123456789"), "{pet}");

        // 3. Dropbox dice que estaba caducado aunque no lo pareciera: se renueva y se repite.
        let f = falso(&["401c", "200"], &["200"]);
        crate::nube::guardar(&[dropbox("Caja", 4), dropbox("Otra", 4)]).unwrap();
        assert!(desconectar("Caja", &f.urls).unwrap().contains("permiso anulado"));
        assert_eq!(f.recibido.lock().unwrap().len(), 3);

        // 4. 401: el permiso ya no valía (se quitó desde la web).
        let f = falso(&["401"], &[]);
        crate::nube::guardar(&[dropbox("Caja", 4), dropbox("Otra", 4)]).unwrap();
        let m = desconectar("Caja", &f.urls).unwrap();
        assert!(m.contains("el permiso ya no era válido"), "{m}");
        assert!(crate::nube::buscar("Caja").is_none() && pendientes().is_empty());

        // 5. Caducado y Dropbox ya no deja renovarlo (invalid_grant): tampoco valía.
        let f = falso(&[], &["400"]);
        crate::nube::guardar(&[dropbox("Caja", -1), dropbox("Otra", 4)]).unwrap();
        assert!(desconectar("Caja", &f.urls).unwrap().contains("ya no era válido"));

        // 6. La última: también la carpeta de vueltas entera.
        let f = falso(&["200"], &[]);
        assert!(desconectar("Otra", &f.urls).unwrap().contains("anulado"));
        assert!(crate::nube::cargar().is_empty() && !vueltas.exists());
        registro_sin_secretos();
    }

    /// Sin conexión, con un error de Dropbox o sin poder renovar: las
    /// credenciales se borran igual y la anulación se apunta (sellada) y se
    /// reintenta con espera creciente; al lograrlo o al vencer, se olvida.
    #[test]
    fn sin_conexion_se_apunta_y_se_reintenta() {
        let _l = crate::restic::tests::real_repo_lock();
        let _c = carpeta("reintenta");
        // Sin red.
        crate::nube::guardar(&[dropbox("Caja", 4)]).unwrap();
        let m = desconectar("Caja", &sin_red()).unwrap();
        assert!(m.contains("Credenciales borradas de este equipo; no se pudo anular el permiso en Dropbox todavía: se reintentará"), "{m}");
        sin_secretos(&m);
        assert!(crate::nube::cargar().is_empty(), "las credenciales se borran ya");
        let p = pendientes();
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].nombre.as_str(), p[0].refresh.as_str(), p[0].access.as_str()), ("Caja", RT, AT));
        if cfg!(windows) {
            let crudo = String::from_utf8_lossy(&std::fs::read(ruta()).unwrap()).into_owned();
            assert!(!crudo.contains(RT) && !crudo.contains(AT) && !crudo.contains(KEY), "sellado");
        }
        let r = serde_json::to_string(&resumen()).unwrap();
        assert!(r.contains("\"Caja\"") && r.contains("hasta"), "{r}");
        sin_secretos(&r);

        // Aún no toca: nada.
        let desde = p[0].desde;
        assert!(reintentar_con(&sin_red(), desde).is_empty());
        // Toca y sigue sin red: se espera más (5 min · 2^n).
        let h = reintentar_con(&sin_red(), desde + ESPERA_S);
        assert!(matches!(h[0].1, Anulacion::Reintentar(_)));
        let p = pendientes();
        assert_eq!(p[0].intentos, 1);
        assert_eq!(p[0].proximo, desde + ESPERA_S + 2 * ESPERA_S);
        // Dropbox falla (500): igual.
        let f = falso(&["500"], &[]);
        let h = reintentar_con(&f.urls, p[0].proximo);
        assert!(matches!(h[0].1, Anulacion::Reintentar(_)));
        assert_eq!(pendientes()[0].intentos, 2);
        // Ya hay conexión: se anula y se quita de la lista.
        let f = falso(&["200"], &[]);
        let h = reintentar_con(&f.urls, pendientes()[0].proximo);
        assert_eq!(h, vec![("Caja".to_string(), Anulacion::Anulado)]);
        assert!(pendientes().is_empty() && !ruta().exists());

        // Dropbox da 500 al desconectar: se apunta. Luego 401: ya no valía, se olvida.
        crate::nube::guardar(&[dropbox("Caja", 4)]).unwrap();
        let f = falso(&["500"], &[]);
        assert!(desconectar("Caja", &f.urls).unwrap().contains("se reintentará"));
        let f = falso(&["401"], &[]);
        let h = reintentar_con(&f.urls, pendientes()[0].proximo);
        assert_eq!(h[0].1, Anulacion::YaNoValia);
        assert!(pendientes().is_empty());

        // Caducado y sin poder renovar (Dropbox caído al renovar): se apunta.
        crate::nube::guardar(&[dropbox("Caja", -1)]).unwrap();
        let f = falso(&[], &["500"]);
        assert!(desconectar("Caja", &f.urls).unwrap().contains("se reintentará"));
        // Vencen los 7 días sin lograrlo: se olvida y se avisa (bitácora → consola).
        let p = pendientes();
        let h = reintentar_con(&sin_red(), p[0].desde + PLAZO_S + 1);
        assert!(matches!(h[0].1, Anulacion::Reintentar(_)));
        assert!(pendientes().is_empty());
        let avisos: Vec<Value> = crate::bitacora::leer().into_iter().filter(|e| e["aviso"] == "nube_sin_anular").collect();
        assert_eq!(avisos.len(), 1);
        let texto = avisos[0]["mensaje"].as_str().unwrap();
        assert!(texto.contains("Aplicaciones conectadas") && texto.contains("Caja"), "{texto}");
        registro_sin_secretos();
    }

    /// No deja si algo la usa (y dice qué); las que no se pueden anular, lo dicen.
    #[test]
    fn no_deja_si_la_usan_y_avisa_si_sigue_viva() {
        let _l = crate::restic::tests::real_repo_lock();
        let _c = carpeta("usada");
        assert!(desconectar("No existe", &sin_red()).unwrap_err().contains("No hay ninguna nube"));
        // El espejo la usa: no se toca nada (ni se llama a Dropbox).
        let f = falso(&[], &[]);
        crate::nube::guardar(&[dropbox("Dropbox Sur", 4)]).unwrap();
        let mut conf = crate::server::load();
        conf.enabled = true;
        conf.espejo = Some(crate::espejo::Espejo {
            hora: "02:00".into(),
            destinos: vec![crate::espejo::Destino { tipo: "nube".into(), carpeta: "Sur".into(), nube: Some("Dropbox Sur".into()), ..Default::default() }],
            ..Default::default()
        });
        crate::server::save(&conf).unwrap();
        let e = desconectar("Dropbox Sur", &f.urls).unwrap_err();
        assert!(e.contains("la usa el espejo de este almacén"), "{e}");
        assert!(crate::nube::buscar("Dropbox Sur").is_some() && f.recibido.lock().unwrap().is_empty());
        conf.espejo = None;
        crate::server::save(&conf).unwrap();

        // B2, SFTP y Drive: se borra y se dice que sigue valiendo allí.
        let otra = |nombre: &str, tipo: &str| Nube { nombre: nombre.into(), tipo: tipo.into(), token: String::new(), ..Default::default() };
        crate::nube::guardar(&[otra("B2 Norte", "b2"), otra("SFTP Casa", "sftp"), otra("Drive", "drive")]).unwrap();
        let m = desconectar("B2 Norte", &f.urls).unwrap();
        assert!(m.contains("credenciales borradas") && m.contains("La clave sigue valiendo en el proveedor"), "{m}");
        assert!(desconectar("SFTP Casa", &f.urls).unwrap().contains("siguen valiendo en ese servidor"));
        assert!(desconectar("Drive", &f.urls).unwrap().contains("sigue vivo en Google"));
        assert!(crate::nube::cargar().is_empty() && f.recibido.lock().unwrap().is_empty() && pendientes().is_empty());

        // Dropbox conectada con la app de rclone (sin app key) y caducada: no se puede renovar desde aquí.
        crate::nube::guardar(&[Nube { app_key: None, ..dropbox("Rc", -1) }]).unwrap();
        let m = desconectar("Rc", &f.urls).unwrap();
        assert!(m.contains("Aplicaciones conectadas") && pendientes().is_empty(), "{m}");
    }
}
