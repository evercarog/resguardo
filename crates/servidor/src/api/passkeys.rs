//! 0.8.1 (H3, plan-mejoras 12c): llaves de acceso (passkeys) para entrar en la consola,
//! como segundo factor en vez del código TOTP (docs/passkeys.md). La contraseña sigue
//! haciendo falta: una llave no sustituye a la contraseña (decisión en docs/passkeys.md).
//!
//! - `GET /api/cuenta/passkeys`: las llaves de la cuenta (nombre, alta, último uso).
//! - `POST /api/cuenta/passkeys/opciones` (contraseña y código TOTP): las opciones para
//!   `navigator.credentials.create`, con un reto de un solo uso que caduca en 5 minutos.
//! - `POST /api/cuenta/passkeys`: guarda la llave nueva (con su nombre), comprobada.
//! - `DELETE /api/cuenta/passkeys/{ref}` (contraseña y código TOTP): la quita.
//! - `POST /api/sesion/passkey/opciones` (sesión a medias, tras la contraseña): las
//!   opciones para `navigator.credentials.get`, con un reto de un solo uso de 2 minutos.
//! - `POST /api/sesion/passkey`: la firma; si vale, sesión completa (como con el TOTP).
//!
//! Las llaves se guardan por cuenta (`passkeys:<cuenta>`, JSON: id, nombre, clave
//! pública COSE, algoritmo, contador, el rpId con el que se dio de alta y fechas). Los
//! retos, en `passkey_reto:<para>` con su caducidad, y se gastan al primer intento. Los
//! intentos de entrar cuentan junto con los del TOTP (`totp:<cuenta>`). Altas y bajas
//! quedan en la auditoría del servidor con el nombre de la llave, nunca su id ni su
//! clave.

use super::cuentas::{con_cookie, cuenta_json, reautenticar, Reautenticar, MAX_INTENTOS, VENTANA};
use crate::almacen::{ahora, Almacen};
use crate::auth::{self, Pendiente, Usuario};
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use crate::webauthn::{self as wa, b64url, de_b64url};
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// Como mucho, llaves por cuenta.
pub const MAX_PASSKEYS: usize = 10;
/// Lo que dura un reto: para dar de alta una llave y para entrar.
const RETO_ALTA_S: i64 = 5 * 60;
const RETO_ENTRAR_S: i64 = 2 * 60;

/// Una llave guardada.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Passkey {
    /// El id de la credencial (base64url).
    pub id: String,
    pub nombre: String,
    /// La clave pública COSE (base64url).
    pub cose: String,
    pub alg: i64,
    pub contador: u32,
    /// El rpId con el que se dio de alta (el nombre de la consola).
    pub rp: String,
    pub creada: i64,
    #[serde(default)]
    pub usada: Option<i64>,
}

impl Passkey {
    /// Una referencia corta para quitarla (no es el id de la credencial).
    fn referencia(&self) -> String {
        Sha256::digest(self.id.as_bytes()).iter().take(8).map(|b| format!("{b:02x}")).collect()
    }
    fn vista(&self) -> Value {
        json!({ "ref": self.referencia(), "nombre": self.nombre, "creada": crate::api::fecha(self.creada), "usada": self.usada.map(crate::api::fecha), "dominio": self.rp })
    }
}

fn clave(cuenta: &str) -> String {
    format!("passkeys:{cuenta}")
}

/// Las llaves de una cuenta.
pub fn de_cuenta(db: &dyn Almacen, cuenta: &str) -> Result<Vec<Passkey>, String> {
    Ok(db.valor(&clave(cuenta))?.and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default())
}

fn guardar(db: &dyn Almacen, cuenta: &str, ps: &[Passkey]) -> Result<(), String> {
    db.poner_valor(&clave(cuenta), &serde_json::to_string(ps).map_err(|e| e.to_string())?)
}

/// Quita todas las llaves de una cuenta (al restablecer su verificación en dos pasos).
pub fn quitar_todas(db: &dyn Almacen, cuenta: &str) -> Result<(), String> {
    db.poner_valor(&clave(cuenta), "")
}

/// Guarda un reto nuevo (32 bytes al azar) para `para`, con su caducidad.
fn poner_reto(db: &dyn Almacen, para: &str, segundos: i64) -> Result<Vec<u8>, String> {
    let reto: [u8; 32] = rand::random();
    db.poner_valor(&format!("passkey_reto:{para}"), &format!("{}:{}", ahora() + segundos, b64url(&reto)))?;
    Ok(reto.to_vec())
}

/// Gasta el reto de `para` (vale una sola vez, se acierte o no) y lo devuelve si no caducó.
fn gastar_reto(db: &dyn Almacen, para: &str) -> Result<Option<Vec<u8>>, String> {
    let k = format!("passkey_reto:{para}");
    let v = db.valor(&k)?.unwrap_or_default();
    db.poner_valor(&k, "")?;
    Ok(v.split_once(':').filter(|(c, _)| c.parse::<i64>().is_ok_and(|c| c > ahora())).and_then(|(_, r)| de_b64url(r)))
}

/// El origen y el rpId de esta petición (por la cabecera `Host`).
fn origen(headers: &HeaderMap) -> Res<(String, String)> {
    let host = headers.get(header::HOST).and_then(|h| h.to_str().ok()).unwrap_or("");
    wa::origen_y_rp(host).map_err(ErrorApi::datos)
}

// ---------- En la cuenta ----------

/// `GET /api/cuenta/passkeys`.
pub async fn listar(State(st): State<St>, u: Usuario) -> Res<Json<Value>> {
    let id = u.id().to_string();
    let ps = st.db(move |db| de_cuenta(db, &id)).await?;
    Ok(Json(json!({ "passkeys": ps.iter().map(Passkey::vista).collect::<Vec<_>>(), "max": MAX_PASSKEYS })))
}

/// `POST /api/cuenta/passkeys/opciones` (contraseña y código TOTP).
pub async fn opciones_alta(State(st): State<St>, u: Usuario, headers: HeaderMap, Json(p): Json<Reautenticar>) -> Res<Json<Value>> {
    let (_, rp) = origen(&headers)?;
    reautenticar(&st, &u, &p).await?;
    let id = u.id().to_string();
    let ps = st
        .db({
            let id = id.clone();
            move |db| de_cuenta(db, &id)
        })
        .await?;
    if ps.len() >= MAX_PASSKEYS {
        return Err(ErrorApi::datos(format!("Como mucho {MAX_PASSKEYS} llaves de acceso por cuenta: quita alguna.")));
    }
    let id2 = id.clone();
    let reto = st.db(move |db| poner_reto(db, &format!("alta:{id2}"), RETO_ALTA_S)).await?;
    Ok(Json(json!({
        "challenge": b64url(&reto),
        "rp": { "id": rp, "name": "Resguardo Server" },
        "user": { "id": b64url(id.as_bytes()), "name": u.0.cuenta.correo, "displayName": u.0.cuenta.nombre },
        "pubKeyCredParams": wa::ALGORITMOS.iter().map(|a| json!({ "type": "public-key", "alg": a })).collect::<Vec<_>>(),
        "excludeCredentials": ps.iter().map(|p| json!({ "type": "public-key", "id": p.id })).collect::<Vec<_>>(),
        "authenticatorSelection": { "residentKey": "discouraged", "userVerification": "preferred" },
        "attestation": "none",
        "timeout": RETO_ALTA_S * 1000,
    })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Alta {
    nombre: String,
    /// El id de la credencial (base64url).
    id: String,
    client_data: String,
    attestation: String,
}

/// `POST /api/cuenta/passkeys`: guarda la llave nueva.
pub async fn alta(State(st): State<St>, u: Usuario, headers: HeaderMap, Json(a): Json<Alta>) -> Res<Json<Value>> {
    if !st.limites.intento(&format!("passkey-alta:{}", u.id()), MAX_INTENTOS, VENTANA) {
        return Err(ErrorApi::demasiados());
    }
    let (origen, rp) = origen(&headers)?;
    let nombre = a.nombre.trim().to_string();
    if nombre.is_empty() || nombre.chars().count() > 60 || nombre.chars().any(char::is_control) {
        return Err(ErrorApi::datos("Ponle un nombre a la llave (hasta 60 caracteres): «Portátil», «Llave USB»…"));
    }
    let (cd, ao) = (de_b64url(&a.client_data), de_b64url(&a.attestation));
    let (Some(cd), Some(ao)) = (cd, ao) else { return Err(ErrorApi::datos("Respuesta de la llave no válida.")) };
    let (id, correo) = (u.id().to_string(), u.0.cuenta.correo.clone());
    let id_cred = a.id;
    let r = st
        .db_crudo(move |db| {
            let Some(reto) = gastar_reto(db, &format!("alta:{id}"))? else {
                return Err("El alta caducó o ya se usó: empieza otra vez.".to_string());
            };
            let reg = wa::verificar_registro(&cd, &ao, &reto, &origen, &rp).map_err(|e| format!("No se pudo dar de alta la llave: {e}."))?;
            if b64url(&reg.id) != id_cred.trim_end_matches('=') {
                return Err("La respuesta de la llave no cuadra.".into());
            }
            let mut ps = de_cuenta(db, &id)?;
            if ps.len() >= MAX_PASSKEYS {
                return Err(format!("Como mucho {MAX_PASSKEYS} llaves de acceso por cuenta: quita alguna."));
            }
            let nueva = Passkey {
                id: b64url(&reg.id),
                nombre: nombre.clone(),
                cose: b64url(&reg.cose),
                alg: reg.alg,
                contador: reg.contador,
                rp,
                creada: ahora(),
                usada: None,
            };
            if ps.iter().any(|p| p.id == nueva.id) {
                return Err("Esa llave ya está dada de alta.".into());
            }
            ps.push(nueva);
            guardar(db, &id, &ps)?;
            db.auditar_servidor(&format!("cuenta:{correo}"), "alta_passkey", &id, &json!({ "nombre": nombre }).to_string())?;
            Ok(ps.iter().map(Passkey::vista).collect::<Vec<_>>())
        })
        .await?
        .map_err(ErrorApi::datos)?;
    st.limites.olvidar(&format!("passkey-alta:{}", u.id()));
    Ok(Json(json!({ "passkeys": r, "max": MAX_PASSKEYS })))
}

/// `DELETE /api/cuenta/passkeys/{ref}` (contraseña y código TOTP).
pub async fn baja(State(st): State<St>, u: Usuario, Path(r): Path<String>, Json(p): Json<Reautenticar>) -> Res<Json<Value>> {
    reautenticar(&st, &u, &p).await?;
    let (id, correo) = (u.id().to_string(), u.0.cuenta.correo.clone());
    let lista = st
        .db_crudo(move |db| {
            let mut ps = de_cuenta(db, &id)?;
            let Some(i) = ps.iter().position(|x| x.referencia() == r) else { return Ok(None) };
            let quitada = ps.remove(i);
            guardar(db, &id, &ps)?;
            db.auditar_servidor(&format!("cuenta:{correo}"), "baja_passkey", &id, &json!({ "nombre": quitada.nombre }).to_string())?;
            Ok(Some(ps.iter().map(Passkey::vista).collect::<Vec<_>>()))
        })
        .await?
        .map_err(ErrorApi::interno)?
        .ok_or_else(ErrorApi::no_existe)?;
    Ok(Json(json!({ "passkeys": lista, "max": MAX_PASSKEYS })))
}

// ---------- Al entrar ----------

/// `POST /api/sesion/passkey/opciones`: tras la contraseña, si la cuenta tiene llaves de este sitio.
pub async fn opciones_entrar(State(st): State<St>, Pendiente(s): Pendiente, headers: HeaderMap) -> Res<Json<Value>> {
    let (_, rp) = origen(&headers)?;
    if !s.cuenta.totp_activo {
        return Err(ErrorApi::datos("Termina antes de dar de alta tu verificación en dos pasos."));
    }
    if !st.limites.intento(&format!("passkey-opciones:{}", s.cuenta.id), MAX_INTENTOS * 3, VENTANA) {
        return Err(ErrorApi::demasiados());
    }
    let (id, th) = (s.cuenta.id.clone(), s.token_hash.clone());
    let r = st
        .db(move |db| {
            let ps: Vec<Passkey> = de_cuenta(db, &id)?.into_iter().filter(|p| p.rp == rp).collect();
            if ps.is_empty() {
                return Ok(None);
            }
            let reto = poner_reto(db, &format!("entrar:{th}"), RETO_ENTRAR_S)?;
            Ok(Some(json!({
                "challenge": b64url(&reto),
                "rpId": rp,
                "allowCredentials": ps.iter().map(|p| json!({ "type": "public-key", "id": p.id })).collect::<Vec<_>>(),
                "userVerification": "preferred",
                "timeout": RETO_ENTRAR_S * 1000,
            })))
        })
        .await?;
    r.map(Json).ok_or_else(|| ErrorApi::datos("No tienes llaves de acceso para esta dirección: usa el código de tu aplicación."))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Firma {
    id: String,
    client_data: String,
    authenticator_data: String,
    firma: String,
}

/// `POST /api/sesion/passkey`: entra con una llave (en vez del código TOTP).
pub async fn entrar(State(st): State<St>, Pendiente(s): Pendiente, headers: HeaderMap, Json(f): Json<Firma>) -> Res<Response> {
    // Los mismos intentos que con el código TOTP: juntos, no se suman.
    if !st.limites.intento(&format!("totp:{}", s.cuenta.id), MAX_INTENTOS, VENTANA) {
        return Err(ErrorApi::demasiados());
    }
    let (origen, rp) = origen(&headers)?;
    if !s.cuenta.totp_activo {
        return Err(ErrorApi::datos("Termina antes de dar de alta tu verificación en dos pasos."));
    }
    let (cd, ad, firma) = (de_b64url(&f.client_data), de_b64url(&f.authenticator_data), de_b64url(&f.firma));
    let (Some(cd), Some(ad), Some(firma)) = (cd, ad, firma) else { return Err(ErrorApi::datos("Respuesta de la llave no válida.")) };
    let cuenta = s.cuenta.clone();
    let (id, th, correo) = (cuenta.id.clone(), s.token_hash.clone(), cuenta.correo.clone());
    let id_cred = f.id.trim_end_matches('=').to_string();
    let (ficha, th_nueva) = auth::ficha_nueva();
    let ok = st
        .db_crudo(move |db| {
            let reto = gastar_reto(db, &format!("entrar:{th}"))?;
            let mut ps = de_cuenta(db, &id)?;
            let pk = ps.iter_mut().find(|p| p.id == id_cred && p.rp == rp);
            let (Some(reto), Some(pk)) = (reto, pk) else { return Ok(false) };
            let Some(cose) = de_b64url(&pk.cose) else { return Ok(false) };
            match wa::verificar_asercion(&cd, &ad, &firma, &reto, &origen, &rp, &cose, pk.contador) {
                Ok(n) => {
                    pk.contador = n;
                    pk.usada = Some(ahora());
                    let nombre = pk.nombre.clone();
                    guardar(db, &id, &ps)?;
                    // Sesión completa = ficha nueva (como con el TOTP).
                    db.borrar_sesion(&th)?;
                    db.crear_sesion(&th_nueva, &id, 2, ahora() + auth::SESION_HORAS * 3600)?;
                    db.auditar_servidor(&format!("cuenta:{correo}"), "entrar_passkey", &id, &json!({ "nombre": nombre }).to_string())?;
                    Ok(true)
                }
                Err(e) => {
                    // Un contador que no sube puede ser una llave copiada: queda anotado (sin datos de la llave).
                    if e.contains("contador") {
                        db.auditar_servidor(&format!("cuenta:{correo}"), "passkey_contador", &id, &json!({ "nombre": pk.nombre }).to_string())?;
                    }
                    Ok(false)
                }
            }
        })
        .await?
        .map_err(ErrorApi::interno)?;
    if !ok {
        return Err(ErrorApi::nuevo(StatusCode::UNAUTHORIZED, "codigo", "La llave de acceso no sirve para entrar aquí. Usa el código de tu aplicación.")
            .acceso("passkey"));
    }
    st.limites.olvidar(&format!("totp:{}", cuenta.id));
    st.limites.olvidar(&format!("entrar:{}", cuenta.correo));
    let cuerpo = json!({ "cuenta": cuenta_json(&st, &cuenta.id).await? });
    Ok(con_cookie(auth::cookie_sesion(&st, &ficha, auth::SESION_HORAS * 3600), cuerpo))
}

/// Cuántas llaves de esta dirección tiene la cuenta (para decirlo al entrar).
pub fn cuantas_para(db: &dyn Almacen, cuenta: &str, host: Option<&str>) -> usize {
    let rp = host.and_then(|h| wa::origen_y_rp(h).ok()).map(|x| x.1);
    de_cuenta(db, cuenta).map(|ps| ps.iter().filter(|p| Some(&p.rp) == rp.as_ref()).count()).unwrap_or(0)
}

#[cfg(test)]
mod pruebas {
    //! El camino entero por HTTP con un autenticador simulado: dar de alta una llave (con la
    //! contraseña y el código), entrar con ella en vez del código, lo que se rechaza y quitarla.
    use crate::webauthn::pruebas::Autenticador;
    use crate::webauthn::{b64url, de_b64url};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::Router;
    use http_body_util::BodyExt;
    use serde_json::{json, Value};
    use tower::ServiceExt;

    async fn pedir(app: &Router, metodo: &str, ruta: &str, cuerpo: Option<Value>, cookie: Option<&str>) -> (StatusCode, Value, Option<String>) {
        let mut r = Request::builder().method(metodo).uri(ruta).header("host", "consola.ejemplo.com:8443");
        if metodo != "GET" {
            r = r.header("x-resguardo", "1");
        }
        if let Some(c) = cookie {
            r = r.header("cookie", c);
        }
        let body = match cuerpo {
            Some(v) => {
                r = r.header("content-type", "application/json");
                Body::from(v.to_string())
            }
            None => Body::empty(),
        };
        let res = app.clone().oneshot(r.body(body).unwrap()).await.unwrap();
        let estado = res.status();
        let cookie = res.headers().get("set-cookie").and_then(|v| v.to_str().ok()).and_then(|v| v.split(';').next()).map(str::to_string);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        (estado, serde_json::from_slice(&bytes).unwrap_or(Value::Null), cookie)
    }

    fn totp(secreto: &str, mas: i64) -> String {
        let paso = chrono::Utc::now().timestamp() / 30 + mas;
        format!("{:06}", crate::auth::hotp(&crate::auth::de_base32(secreto).unwrap(), paso as u64))
    }

    struct Linea {
        accion: String,
        datos: String,
    }

    /// La auditoría del servidor (`control.db`), leída tal cual.
    fn auditoria(dir: &std::path::Path) -> Vec<Linea> {
        let ruta = [dir.join("control.db"), dir.join("datos").join("control.db")].into_iter().find(|r| r.is_file()).expect("control.db");
        let db = rusqlite::Connection::open(ruta).unwrap();
        let mut st = db.prepare("SELECT accion, datos FROM auditoria").unwrap();
        st.query_map([], |r| Ok(Linea { accion: r.get(0)?, datos: r.get(1)? })).unwrap().map(Result::unwrap).collect()
    }

    fn firma_json(a: &mut Autenticador, reto: &[u8]) -> Value {
        let (cd, ad, firma) = a.firmar(reto);
        json!({ "id": b64url(&a.id), "client_data": b64url(&cd), "authenticator_data": b64url(&ad), "firma": b64url(&firma) })
    }

    #[tokio::test]
    async fn alta_entrada_y_baja_de_una_llave() {
        let dir = tempfile::tempdir().unwrap();
        let st = crate::preparar(dir.path(), crate::estado::Opciones { https: false, ..Default::default() }).unwrap();
        let app = crate::api::router(st.clone());
        let codigo = crate::api::preparar_codigo_arranque(&st).unwrap().unwrap();
        let contrasena = "una contraseña bien larga";
        let inicio = json!({ "codigo_arranque": codigo, "correo": "ana@ejemplo.com", "nombre": "Ana", "contrasena": contrasena });
        let (_, j, c) = pedir(&app, "POST", "/api/inicio", Some(inicio), None).await;
        let secreto = j["totp"]["secreto"].as_str().unwrap().to_string();
        let (e, _, c) = pedir(&app, "POST", "/api/sesion/totp", Some(json!({ "codigo": totp(&secreto, -1) })), c.as_deref()).await;
        assert_eq!(e, StatusCode::OK);
        let ana = c.unwrap();

        // Alta: sin la contraseña y el código, nada.
        let (e, _, _) =
            pedir(&app, "POST", "/api/cuenta/passkeys/opciones", Some(json!({ "contrasena": "otra", "codigo": totp(&secreto, 0) })), Some(&ana)).await;
        assert_eq!(e, StatusCode::UNAUTHORIZED);
        let (e, op, _) =
            pedir(&app, "POST", "/api/cuenta/passkeys/opciones", Some(json!({ "contrasena": contrasena, "codigo": totp(&secreto, 0) })), Some(&ana)).await;
        assert_eq!(e, StatusCode::OK, "{op}");
        assert_eq!(op["rp"]["id"], "consola.ejemplo.com");
        assert_eq!(op["attestation"], "none");
        let reto = de_b64url(op["challenge"].as_str().unwrap()).unwrap();
        let mut a = Autenticador::ed25519("consola.ejemplo.com", "https://consola.ejemplo.com:8443");
        let (cd, ao) = a.crear(&reto);
        let alta = json!({ "nombre": "Llave USB", "id": b64url(&a.id), "client_data": b64url(&cd), "attestation": b64url(&ao) });
        let (e, j, _) = pedir(&app, "POST", "/api/cuenta/passkeys", Some(alta.clone()), Some(&ana)).await;
        assert_eq!(e, StatusCode::OK, "{j}");
        assert_eq!(j["passkeys"][0]["nombre"], "Llave USB");
        let referencia = j["passkeys"][0]["ref"].as_str().unwrap().to_string();
        assert!(!j.to_string().contains(&b64url(&a.id)), "la lista no lleva el id de la credencial");
        // El mismo reto otra vez: ya se gastó.
        let (e, _, _) = pedir(&app, "POST", "/api/cuenta/passkeys", Some(alta), Some(&ana)).await;
        assert_eq!(e, StatusCode::UNPROCESSABLE_ENTITY);

        // Entrar: contraseña y, en vez del código, la llave.
        let (e, j, c) = pedir(&app, "POST", "/api/sesion", Some(json!({ "correo": "ana@ejemplo.com", "contrasena": contrasena })), None).await;
        assert_eq!((e, j["necesita"].as_str(), j["passkeys"].as_u64()), (StatusCode::OK, Some("totp"), Some(1)));
        let media = c.unwrap();
        let (e, _, _) = pedir(&app, "GET", "/api/cuenta", None, Some(&media)).await;
        assert_ne!(e, StatusCode::OK, "a medias no se entra en la API");
        let (e, op, _) = pedir(&app, "POST", "/api/sesion/passkey/opciones", None, Some(&media)).await;
        assert_eq!(e, StatusCode::OK, "{op}");
        let reto = de_b64url(op["challenge"].as_str().unwrap()).unwrap();
        // Una firma hecha en otra dirección (una página falsa que reenvía): no. Y gasta el reto.
        a.origen = "https://consola-falsa.ejemplo.net".into();
        let f = firma_json(&mut a, &reto);
        let (e, _, _) = pedir(&app, "POST", "/api/sesion/passkey", Some(f), Some(&media)).await;
        assert_eq!(e, StatusCode::UNAUTHORIZED);
        a.origen = "https://consola.ejemplo.com:8443".into();
        let f = firma_json(&mut a, &reto);
        let (e, _, _) = pedir(&app, "POST", "/api/sesion/passkey", Some(f), Some(&media)).await;
        assert_eq!(e, StatusCode::UNAUTHORIZED, "reto de un solo uso");
        let (_, op, _) = pedir(&app, "POST", "/api/sesion/passkey/opciones", None, Some(&media)).await;
        let reto = de_b64url(op["challenge"].as_str().unwrap()).unwrap();
        let f = firma_json(&mut a, &reto);
        let (e, j, c) = pedir(&app, "POST", "/api/sesion/passkey", Some(f.clone()), Some(&media)).await;
        assert_eq!(e, StatusCode::OK, "{j}");
        let completa = c.unwrap();
        let (e, _, _) = pedir(&app, "GET", "/api/cuenta", None, Some(&completa)).await;
        assert_eq!(e, StatusCode::OK);
        // La misma firma otra vez: no (la sesión a medias ya no existe y el reto se gastó).
        let (e, _, _) = pedir(&app, "POST", "/api/sesion/passkey", Some(f), Some(&media)).await;
        assert_ne!(e, StatusCode::OK);
        // Auditoría: alta y entrada, con el nombre y sin el id.
        let aud = auditoria(dir.path());
        assert!(aud.iter().any(|x| x.accion == "alta_passkey" && x.datos.contains("Llave USB")));
        assert!(aud.iter().any(|x| x.accion == "entrar_passkey"));
        assert!(!aud.iter().any(|x| x.datos.contains(&b64url(&a.id))), "nada de la llave en la auditoría");

        // Quitarla: con la contraseña y el código.
        let ruta = format!("/api/cuenta/passkeys/{referencia}");
        let (e, _, _) = pedir(&app, "DELETE", &ruta, Some(json!({ "contrasena": "otra", "codigo": "000000" })), Some(&completa)).await;
        assert_eq!(e, StatusCode::UNAUTHORIZED);
        let (e, j, _) = pedir(&app, "DELETE", &ruta, Some(json!({ "contrasena": contrasena, "codigo": totp(&secreto, 1) })), Some(&completa)).await;
        assert_eq!(e, StatusCode::OK, "{j}");
        assert_eq!(j["passkeys"].as_array().map(Vec::len), Some(0));
        assert!(auditoria(dir.path()).iter().any(|x| x.accion == "baja_passkey"));
    }
}
