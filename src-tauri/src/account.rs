//! «Todos mis equipos»: iniciar sesión en la cuenta de Resguardo Web desde la
//! app (correo, contraseña y el segundo factor TOTP: sesión aal2) para ver los
//! demás equipos y pedirles «Copiar ahora».
//!
//! Todo ocurre en el backend: la interfaz nunca ve los tokens. La contraseña
//! no se guarda en ningún sitio; el token de renovación se guarda en el
//! almacén de credenciales del usuario de Windows (Credential Manager) y el de
//! acceso solo en memoria. Las consultas van por PostgREST con el JWT del
//! usuario, así que la seguridad a nivel de fila (RLS, que exige aal2) aplica.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Mutex;

const KEYRING_SERVICE: &str = "io.github.resguardo.cuenta";
const KEYRING_USER: &str = "sesion";

/// Adónde se conecta: la web vinculada o, si no hay, la de la compilación.
#[derive(Clone, Debug, PartialEq)]
pub struct Endpoint {
    pub url: String,
    pub key: String,
}

impl Endpoint {
    pub fn current() -> Result<Self, String> {
        let (url, key) = match crate::web::load_link() {
            Some(l) => (l.url, l.key),
            None => crate::web::defaults(),
        };
        if url.is_empty() || key.is_empty() {
            return Err("No sé a qué web conectarme: vincula antes este equipo con Resguardo Web.".into());
        }
        Ok(Endpoint { url: url.trim_end_matches('/').to_string(), key })
    }
}

/// Una petición HTTP ya preparada (para probarlas sin red).
#[derive(Debug, PartialEq)]
pub struct Req {
    pub method: &'static str,
    pub url: String,
    pub bearer: Option<String>,
    pub body: Option<Value>,
}

// ---------- Construcción de peticiones (GoTrue y PostgREST) ----------

pub fn password_login(ep: &Endpoint, email: &str, password: &str) -> Req {
    Req {
        method: "POST",
        url: format!("{}/auth/v1/token?grant_type=password", ep.url),
        bearer: None,
        body: Some(json!({ "email": email.trim(), "password": password })),
    }
}

pub fn refresh(ep: &Endpoint, refresh_token: &str) -> Req {
    Req {
        method: "POST",
        url: format!("{}/auth/v1/token?grant_type=refresh_token", ep.url),
        bearer: None,
        body: Some(json!({ "refresh_token": refresh_token })),
    }
}

pub fn challenge(ep: &Endpoint, access: &str, factor: &str) -> Req {
    Req { method: "POST", url: format!("{}/auth/v1/factors/{}/challenge", ep.url, enc(factor)), bearer: Some(access.into()), body: Some(json!({})) }
}

pub fn verify(ep: &Endpoint, access: &str, factor: &str, challenge_id: &str, code: &str) -> Req {
    Req {
        method: "POST",
        url: format!("{}/auth/v1/factors/{}/verify", ep.url, enc(factor)),
        bearer: Some(access.into()),
        body: Some(json!({ "challenge_id": challenge_id, "code": code })),
    }
}

pub fn logout(ep: &Endpoint, access: &str) -> Req {
    Req { method: "POST", url: format!("{}/auth/v1/logout?scope=local", ep.url), bearer: Some(access.into()), body: None }
}

/// Lectura de una tabla con RLS (`select=*`: tolera columnas nuevas).
pub fn table(ep: &Endpoint, access: &str, table: &str, query: &str) -> Req {
    let q = if query.is_empty() { String::new() } else { format!("&{query}") };
    Req { method: "GET", url: format!("{}/rest/v1/{table}?select=*{q}", ep.url), bearer: Some(access.into()), body: None }
}

pub fn rpc(ep: &Endpoint, access: &str, function: &str, body: Value) -> Req {
    Req { method: "POST", url: format!("{}/rest/v1/rpc/{function}", ep.url), bearer: Some(access.into()), body: Some(body) }
}

/// Identificadores en la URL: solo letras, números y guiones (son UUID).
fn enc(id: &str) -> String {
    id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect()
}

fn send(ep: &Endpoint, req: &Req) -> Result<Value, String> {
    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(30)))
        .http_status_as_error(false)
        .tls_config(TlsConfig::builder().provider(TlsProvider::NativeTls).root_certs(RootCerts::PlatformVerifier).build())
        .build()
        .new_agent();
    let auth = req.bearer.as_ref().map(|b| format!("Bearer {b}")).unwrap_or_else(|| format!("Bearer {}", ep.key));
    let result = match (req.method, &req.body) {
        ("GET", _) => agent.get(&req.url).header("apikey", &ep.key).header("Authorization", &auth).call(),
        (_, Some(body)) => agent.post(&req.url).header("apikey", &ep.key).header("Authorization", &auth).send_json(body),
        (_, None) => agent.post(&req.url).header("apikey", &ep.key).header("Authorization", &auth).send_empty(),
    };
    let mut resp = result.map_err(|e| format!("No se pudo conectar con la web: {e}"))?;
    let status = resp.status().as_u16();
    let value: Value = resp.body_mut().read_json().unwrap_or(Value::Null);
    if status >= 400 {
        return Err(explain(status, &value));
    }
    Ok(value)
}

/// Mensaje claro para los errores de GoTrue y PostgREST (nunca con tokens).
pub fn explain(status: u16, v: &Value) -> String {
    let code = v["error_code"].as_str().or(v["code"].as_str()).unwrap_or("");
    let msg = v["msg"].as_str().or(v["message"].as_str()).or(v["error_description"].as_str()).unwrap_or("");
    match code {
        "invalid_credentials" => "Correo o contraseña incorrectos.".into(),
        "mfa_verification_failed" => "El código no es correcto o ya caducó.".into(),
        "refresh_token_not_found" | "refresh_token_already_used" | "session_not_found" => "La sesión caducó: vuelve a iniciarla.".into(),
        "over_request_rate_limit" => "Demasiados intentos seguidos. Espera un momento.".into(),
        // Las funciones de la web lanzan excepciones con su texto en español
        // («Este equipo no permite copias a distancia…»): se muestran tal cual.
        _ if !msg.is_empty() => msg.chars().take(300).collect(),
        _ if status == 429 => "Demasiados intentos seguidos. Espera un momento.".into(),
        _ if status == 401 || status == 403 => "La web no permite esta acción con esta sesión.".into(),
        _ => format!("La web respondió con un error ({status})."),
    }
}

// ---------- Sesión ----------

#[derive(Serialize, Deserialize, Clone)]
struct Saved {
    url: String,
    email: String,
    refresh_token: String,
}

struct Session {
    access: String,
    expires: DateTime<Utc>,
}

/// Inicio de sesión a medias: falta el código del segundo factor.
struct Pending {
    ep: Endpoint,
    email: String,
    access: String,
    refresh: String,
    factor: String,
}

#[derive(Default)]
pub struct Account {
    pending: Mutex<Option<Pending>>,
    session: Mutex<Option<Session>>,
}

#[derive(Serialize)]
pub struct Status {
    pub signed_in: bool,
    pub email: Option<String>,
    /// Falta el código del autenticador.
    pub needs_code: bool,
    /// Nombre de este equipo (el que verán los otros como origen).
    pub this_device: String,
}

fn saved() -> Option<Saved> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).ok()?;
    serde_json::from_str(&entry.get_password().ok()?).ok()
}

fn save(s: &Saved) -> Result<(), String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .and_then(|e| e.set_password(&serde_json::to_string(s).unwrap()))
        .map_err(|e| format!("No se pudo guardar la sesión: {e}"))
}

fn forget() {
    if let Ok(e) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
        let _ = e.delete_credential();
    }
}

/// Factor TOTP verificado del usuario (de la respuesta de inicio de sesión).
pub fn totp_factor(user: &Value) -> Option<String> {
    user["factors"].as_array()?.iter().find(|f| f["factor_type"] == "totp" && f["status"] == "verified").and_then(|f| f["id"].as_str()).map(String::from)
}

pub fn this_device() -> String {
    crate::web::load_link().map(|l| l.device_name).unwrap_or_else(crate::web::default_device_name)
}

impl Account {
    pub fn status(&self) -> Status {
        let pending = self.pending.lock().unwrap();
        let s = saved();
        Status {
            signed_in: s.is_some() && pending.is_none(),
            email: pending.as_ref().map(|p| p.email.clone()).or(s.map(|s| s.email)),
            needs_code: pending.is_some(),
            this_device: this_device(),
        }
    }

    /// Paso 1: correo y contraseña. Devuelve true si falta el código (siempre,
    /// salvo error: la web exige el segundo factor para ver los equipos).
    pub fn sign_in(&self, email: &str, password: &str) -> Result<bool, String> {
        let ep = Endpoint::current()?;
        if email.trim().is_empty() || password.is_empty() {
            return Err("Escribe el correo y la contraseña de tu cuenta de Resguardo Web.".into());
        }
        let v = send(&ep, &password_login(&ep, email, password))?;
        let (Some(access), Some(refresh)) = (v["access_token"].as_str(), v["refresh_token"].as_str()) else {
            return Err("La web no devolvió una sesión.".into());
        };
        let Some(factor) = totp_factor(&v["user"]) else {
            return Err("Tu cuenta no tiene activado el segundo factor (autenticador). Actívalo en la web: es obligatorio para ver los equipos.".into());
        };
        *self.pending.lock().unwrap() = Some(Pending { ep, email: email.trim().into(), access: access.into(), refresh: refresh.into(), factor });
        *self.session.lock().unwrap() = None;
        Ok(true)
    }

    /// Paso 2: el código de 6 cifras del autenticador.
    pub fn verify_code(&self, code: &str) -> Result<(), String> {
        let code: String = code.chars().filter(|c| c.is_ascii_digit()).collect();
        if code.len() != 6 {
            return Err("El código tiene 6 cifras.".into());
        }
        let pending = self.pending.lock().unwrap().take().ok_or("Empieza de nuevo: inicia sesión con tu correo.")?;
        let ch = send(&pending.ep, &challenge(&pending.ep, &pending.access, &pending.factor));
        let result = ch.and_then(|ch| {
            let id = ch["id"].as_str().ok_or("La web no devolvió el desafío.")?.to_string();
            send(&pending.ep, &verify(&pending.ep, &pending.access, &pending.factor, &id, &code))
        });
        let v = match result {
            Ok(v) => v,
            Err(e) => {
                // Se puede volver a intentar el código con la misma sesión.
                *self.pending.lock().unwrap() = Some(pending);
                return Err(e);
            }
        };
        let (Some(access), Some(refresh)) = (v["access_token"].as_str(), v["refresh_token"].as_str()) else {
            return Err("La web no devolvió la sesión.".into());
        };
        let _ = &pending.refresh;
        save(&Saved { url: pending.ep.url.clone(), email: pending.email.clone(), refresh_token: refresh.into() })?;
        *self.session.lock().unwrap() = Some(Session { access: access.into(), expires: expiry(&v) });
        Ok(())
    }

    pub fn sign_out(&self) {
        *self.pending.lock().unwrap() = None;
        if let (Some(s), Ok(ep)) = (self.session.lock().unwrap().take(), Endpoint::current()) {
            let _ = send(&ep, &logout(&ep, &s.access));
        }
        forget();
    }

    /// Token de acceso vigente (renovándolo si hace falta).
    fn access(&self) -> Result<(Endpoint, String), String> {
        let ep = Endpoint::current()?;
        {
            let s = self.session.lock().unwrap();
            if let Some(s) = s.as_ref().filter(|s| s.expires > Utc::now() + Duration::seconds(60)) {
                return Ok((ep, s.access.clone()));
            }
        }
        let saved = saved().ok_or("Inicia sesión con tu cuenta de Resguardo Web.")?;
        if saved.url != ep.url {
            forget();
            return Err("La web vinculada cambió: vuelve a iniciar sesión.".into());
        }
        let v = match send(&ep, &refresh(&ep, &saved.refresh_token)) {
            Ok(v) => v,
            Err(e) => {
                if e.contains("caducó") {
                    forget();
                }
                return Err(e);
            }
        };
        let (Some(access), Some(refresh)) = (v["access_token"].as_str(), v["refresh_token"].as_str()) else {
            return Err("La web no devolvió la sesión.".into());
        };
        // El token de renovación cambia en cada uso: se guarda el nuevo.
        save(&Saved { refresh_token: refresh.into(), ..saved })?;
        *self.session.lock().unwrap() = Some(Session { access: access.into(), expires: expiry(&v) });
        Ok((ep, access.into()))
    }

    /// Clientes, equipos, destinos y las últimas peticiones a distancia (con RLS).
    pub fn overview(&self) -> Result<Value, String> {
        let (ep, access) = self.access()?;
        let get = |t: &str, q: &str| send(&ep, &table(&ep, &access, t, q));
        let clients = get("clients", "order=name.asc")?;
        let devices = get("devices", "revoked_at=is.null&order=name.asc")?;
        let repos = get("repos", "order=name.asc")?;
        // Las peticiones aún pueden no existir en una web antigua: no es un error.
        let commands = get("device_commands", "order=requested_at.desc&limit=50").unwrap_or(Value::Array(vec![]));
        Ok(json!({ "clients": clients, "devices": devices, "repos": repos, "commands": commands, "this_device": this_device() }))
    }

    /// Pide «Copiar ahora» de un plan en otro equipo. Devuelve el id de la petición.
    pub fn request_backup(&self, device: &str, repo: &str, plan: &str) -> Result<String, String> {
        let (ep, access) = self.access()?;
        let body = json!({ "p_device": device, "p_repo": repo, "p_plan": plan, "p_from": this_device() });
        let v = send(&ep, &rpc(&ep, &access, "request_remote_backup", body))?;
        v.as_str().map(String::from).or_else(|| v["id"].as_str().map(String::from)).ok_or_else(|| "La web no devolvió la petición.".into())
    }

    /// Destinos que comparten los equipos del usuario y el estado de mis
    /// peticiones (fase 3, docs/compartir.md).
    pub fn shares(&self) -> Result<Value, String> {
        let (ep, access) = self.access()?;
        let list = send(&ep, &rpc(&ep, &access, "shares_list", json!({})))?;
        let mine = send(&ep, &rpc(&ep, &access, "share_requests_mine", json!({}))).unwrap_or(Value::Array(vec![]));
        Ok(json!({ "shares": list, "requests": mine }))
    }

    /// Pide un destino compartido para el equipo `device` (este equipo, vinculado a la web).
    pub fn request_share(&self, share: &str, device: &str) -> Result<Value, String> {
        let (ep, access) = self.access()?;
        send(&ep, &rpc(&ep, &access, "share_request", json!({ "p_share": share, "p_device": device })))
    }

    /// Cancela una petición mía aún pendiente (antes de que se entregue).
    pub fn cancel_share(&self, request: &str) -> Result<Value, String> {
        let (ep, access) = self.access()?;
        send(&ep, &rpc(&ep, &access, "share_cancel", json!({ "p_request": request })))
    }

    /// Estado de una petición (`device_commands`).
    pub fn command(&self, id: &str) -> Result<Value, String> {
        let (ep, access) = self.access()?;
        let v = send(&ep, &table(&ep, &access, "device_commands", &format!("id=eq.{}", enc(id))))?;
        Ok(v.as_array().and_then(|a| a.first().cloned()).unwrap_or(Value::Null))
    }
}

fn expiry(v: &Value) -> DateTime<Utc> {
    let secs = v["expires_in"].as_i64().unwrap_or(3600).clamp(60, 24 * 3600);
    Utc::now() + Duration::seconds(secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep() -> Endpoint {
        Endpoint { url: "https://abc.supabase.co".into(), key: "pub".into() }
    }

    #[test]
    fn peticiones_de_gotrue() {
        let r = password_login(&ep(), " ana@x.com ", "secreta");
        assert_eq!(r.url, "https://abc.supabase.co/auth/v1/token?grant_type=password");
        assert_eq!(r.body.unwrap(), json!({ "email": "ana@x.com", "password": "secreta" }));
        assert!(r.bearer.is_none());
        let r = refresh(&ep(), "rt");
        assert_eq!(r.url, "https://abc.supabase.co/auth/v1/token?grant_type=refresh_token");
        assert_eq!(r.body.unwrap()["refresh_token"], "rt");
        let r = challenge(&ep(), "jwt", "f-1/../x?y");
        assert_eq!(r.url, "https://abc.supabase.co/auth/v1/factors/f-1xy/challenge", "el id no puede cambiar la ruta");
        assert_eq!(r.bearer.as_deref(), Some("jwt"));
        let r = verify(&ep(), "jwt", "f1", "c1", "123456");
        assert_eq!(r.body.unwrap(), json!({ "challenge_id": "c1", "code": "123456" }));
        assert!(logout(&ep(), "jwt").url.ends_with("/auth/v1/logout?scope=local"));
    }

    #[test]
    fn peticiones_de_postgrest() {
        let r = table(&ep(), "jwt", "devices", "revoked_at=is.null");
        assert_eq!(r.url, "https://abc.supabase.co/rest/v1/devices?select=*&revoked_at=is.null");
        assert_eq!(r.method, "GET");
        let r = rpc(&ep(), "jwt", "request_remote_backup", json!({ "p_device": "d" }));
        assert_eq!(r.url, "https://abc.supabase.co/rest/v1/rpc/request_remote_backup");
        assert_eq!(r.bearer.as_deref(), Some("jwt"));
    }

    #[test]
    fn factor_y_errores() {
        let user = json!({ "factors": [
            { "id": "a", "factor_type": "totp", "status": "unverified" },
            { "id": "b", "factor_type": "totp", "status": "verified" }
        ]});
        assert_eq!(totp_factor(&user).as_deref(), Some("b"));
        assert!(totp_factor(&json!({ "factors": [] })).is_none());
        assert!(totp_factor(&json!({})).is_none());
        assert_eq!(explain(400, &json!({ "error_code": "invalid_credentials", "msg": "Invalid login credentials" })), "Correo o contraseña incorrectos.");
        assert!(explain(422, &json!({ "error_code": "mfa_verification_failed" })).contains("código"));
        assert!(explain(429, &json!({})).contains("Demasiados"));
        let rpc_err = json!({ "code": "P0001", "message": "Este equipo no permite copias a distancia. Actívalo en Resguardo, en ese equipo." });
        assert_eq!(explain(400, &rpc_err), "Este equipo no permite copias a distancia. Actívalo en Resguardo, en ese equipo.");
        assert!(explain(400, &json!({ "error_code": "refresh_token_not_found" })).contains("caducó"));
    }
}
