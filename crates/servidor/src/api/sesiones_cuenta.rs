//! Las sesiones de la consola de cada cuenta (16k): la lista con «Cerrar» y
//! «Cerrar en todos», los plazos que ajusta el propietario del servidor y el
//! cierre que hace el propietario por quien perdió su único dispositivo.
//!
//! Una sesión de menos de 24 h no puede cerrar otras (`Usuario::sesion_asentada`);
//! cerrar otras siempre pide repetir la contraseña y el segundo paso.

use super::cuentas::{reautenticar, Reautenticar};
use super::fecha;
use crate::almacen::{ahora, Sesion};
use crate::auth::{self, Plazos, Usuario};
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

/// Una sesión como la ve su dueño (sin la ficha ni su hash).
pub fn vista(s: &Sesion, actual: &str, t: i64) -> Value {
    json!({
        "id": s.id,
        "actual": s.id == actual,
        "navegador": s.agente,
        "ip": s.ip,
        "creada": fecha(s.creada),
        "usada": fecha(s.usada),
        "caduca": fecha(s.expira),
        "maximo": fecha(s.maximo),
        "recordar": s.recordar,
        "nueva": t - s.creada < auth::SESION_NUEVA_S,
    })
}

/// Lo que la consola necesita de la sesión actual y de las nuevas (para el aviso
/// «Nueva sesión en … ¿eras tú?»): las otras sesiones abiertas después de esta en
/// las últimas 24 h.
pub async fn resumen_para_cuenta(st: &St, u: &Usuario) -> Res<Value> {
    let id = u.id().to_string();
    let (sesiones, creada_cuenta) = st.db(move |db| Ok((db.sesiones_de(&id)?, db.cuenta_creada(&id)?))).await?;
    let t = ahora();
    let actual = &u.0.sesion;
    let nuevas: Vec<Value> = sesiones
        .iter()
        .filter(|s| s.id != actual.id && t - s.creada < auth::SESION_NUEVA_S && s.creada > actual.creada)
        .map(|s| json!({ "id": s.id, "navegador": s.agente, "creada": fecha(s.creada) }))
        .collect();
    let hasta = auth::sesion_nueva_hasta(actual.creada, creada_cuenta, t);
    Ok(json!({
        "sesion": {
            "id": actual.id, "creada": fecha(actual.creada), "recordar": actual.recordar,
            "caduca": fecha(actual.expira), "maximo": fecha(actual.maximo),
            "nueva_hasta": hasta.map(fecha),
        },
        "sesiones_nuevas": nuevas,
    }))
}

/// `GET /api/cuenta/sesiones`.
pub async fn listar(State(st): State<St>, u: Usuario) -> Res<Json<Value>> {
    let id = u.id().to_string();
    let (sesiones, creada_cuenta) = st.db(move |db| Ok((db.sesiones_de(&id)?, db.cuenta_creada(&id)?))).await?;
    let t = ahora();
    let actual = u.0.sesion.id.clone();
    let hasta = auth::sesion_nueva_hasta(u.0.sesion.creada, creada_cuenta, t);
    Ok(Json(json!({
        "sesiones": sesiones.iter().map(|s| vista(s, &actual, t)).collect::<Vec<_>>(),
        "puede_cerrar": hasta.is_none(),
        "puede_desde": hasta.map(fecha),
    })))
}

#[derive(Deserialize)]
pub struct Cerrar {
    /// Las sesiones que cerrar (por su identificador público).
    #[serde(default)]
    ids: Vec<String>,
    /// Todas menos esta.
    #[serde(default)]
    otras: bool,
    #[serde(flatten)]
    reautenticar: Reautenticar,
}

const MAX_IDS: usize = 100;

/// `POST /api/cuenta/sesiones/cerrar`: cierra otras sesiones de la cuenta. Desde una
/// sesión de menos de 24 h, no; siempre con la contraseña y el segundo paso. Para
/// cerrar la propia está `DELETE /api/sesion`.
pub async fn cerrar(State(st): State<St>, u: Usuario, Json(p): Json<Cerrar>) -> Res<Json<Value>> {
    if p.ids.len() > MAX_IDS || p.ids.iter().any(|i| i.is_empty() || i.len() > 64 || !i.chars().all(|c| c.is_ascii_hexdigit())) {
        return Err(ErrorApi::datos("Sesiones no válidas."));
    }
    if !p.otras && p.ids.is_empty() {
        return Err(ErrorApi::datos("Elige qué sesiones cerrar."));
    }
    let actual = u.0.sesion.id.clone();
    if !p.otras && p.ids.contains(&actual) {
        return Err(ErrorApi::datos("Esta es la sesión en la que estás: para cerrarla, usa «Salir»."));
    }
    u.sesion_asentada(&st).await?;
    reautenticar(&st, &u, &p.reautenticar).await?;
    let (id, correo, th, otras, ids) = (u.id().to_string(), u.0.cuenta.correo.clone(), u.0.token_hash.clone(), p.otras, p.ids);
    let cerradas = st
        .db(move |db| {
            let antes = db.sesiones_de(&id)?;
            let cerradas: Vec<Sesion> = if otras {
                db.borrar_sesiones_de(&id, Some(&th))?;
                antes.into_iter().filter(|s| s.id != actual).collect()
            } else {
                let mut v = Vec::new();
                for s in antes.into_iter().filter(|s| ids.contains(&s.id)) {
                    if db.borrar_sesion_por_id(&id, &s.id)? {
                        v.push(s);
                    }
                }
                v
            };
            let navegadores: Vec<&str> = cerradas.iter().map(|s| s.agente.as_str()).collect();
            db.auditar_servidor(
                &format!("cuenta:{correo}"),
                if otras { "cerrar_otras_sesiones" } else { "cerrar_sesion" },
                &id,
                &json!({ "cerradas": cerradas.len(), "navegadores": navegadores }).to_string(),
            )?;
            Ok(cerradas.len())
        })
        .await?;
    Ok(Json(json!({ "cerradas": cerradas })))
}

/// `POST /api/servidor/cuentas/{id}/sesiones/cerrar`: el propietario del servidor cierra
/// todas las sesiones de otra cuenta (para quien perdió su único dispositivo y no
/// puede esperar las 24 h). Con su contraseña y su segundo paso, y en la auditoría.
pub async fn cerrar_de_otra(State(st): State<St>, u: Usuario, Path(otra): Path<String>, Json(p): Json<Reautenticar>) -> Res<Json<Value>> {
    if !u.0.cuenta.superusuario {
        return Err(ErrorApi::prohibido());
    }
    if otra == u.id() {
        return Err(ErrorApi::datos("Para tu propia cuenta, usa «Cerrar en todos» en «Mi cuenta»."));
    }
    u.sesion_asentada(&st).await?;
    reautenticar(&st, &u, &p).await?;
    let o2 = otra.clone();
    let cuenta = st.db(move |db| db.cuenta(&o2)).await?.ok_or_else(ErrorApi::no_existe)?;
    let (actor, id) = (format!("cuenta:{}", u.0.cuenta.correo), cuenta.id.clone());
    let n = st
        .db(move |db| {
            let n = db.sesiones_de(&id)?.len();
            db.borrar_sesiones_de(&id, None)?;
            db.auditar_servidor(&actor, "cerrar_sesiones_de_otra", &id, &json!({ "correo": cuenta.correo, "cerradas": n }).to_string())?;
            Ok(n)
        })
        .await?;
    Ok(Json(json!({ "cerradas": n })))
}

/// `GET /api/servidor/sesiones`: los plazos de las sesiones (propietario del servidor).
pub async fn ver_plazos(State(st): State<St>, u: Usuario) -> Res<Json<Value>> {
    if !u.0.cuenta.superusuario {
        return Err(ErrorApi::prohibido());
    }
    let p = st.db(|db| Ok(auth::plazos(db))).await?;
    Ok(Json(json!({ "plazos": p, "predeterminados": Plazos::default() })))
}

/// `PUT /api/servidor/sesiones`: cambia los plazos (con topes). Valen para las sesiones
/// nuevas y para la caducidad por inactividad de las abiertas; nunca alargan el máximo
/// de una ya abierta.
pub async fn poner_plazos(State(st): State<St>, u: Usuario, Json(p): Json<Plazos>) -> Res<(StatusCode, Json<Value>)> {
    if !u.0.cuenta.superusuario {
        return Err(ErrorApi::prohibido());
    }
    let p = p.con_topes();
    let valor = serde_json::to_string(&p).map_err(ErrorApi::interno)?;
    let (actor, id) = (format!("cuenta:{}", u.0.cuenta.correo), u.id().to_string());
    st.db(move |db| {
        db.poner_valor(auth::CLAVE_PLAZOS, &valor)?;
        db.auditar_servidor(&actor, "plazos_sesiones", &id, &valor)
    })
    .await?;
    Ok((StatusCode::OK, Json(json!({ "plazos": p }))))
}

#[cfg(test)]
mod pruebas {
    //! El camino entero por HTTP: entrar con «Recordar», la lista de sesiones, una
    //! sesión nueva que no puede cerrar las demás ni cambiar la contraseña, y una
    //! antigua que sí la cierra (con la contraseña y el código).
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::Router;
    use http_body_util::BodyExt;
    use serde_json::{json, Value};
    use tower::ServiceExt;

    struct Resp {
        estado: StatusCode,
        json: Value,
        cookie: Option<String>,
        set_cookie: String,
    }

    async fn pedir(app: &Router, metodo: &str, ruta: &str, cuerpo: Option<Value>, cookie: Option<&str>) -> Resp {
        let mut r = Request::builder()
            .method(metodo)
            .uri(ruta)
            .header("host", "consola.ejemplo.com:8443")
            .header("user-agent", "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0 Mobile Safari/537.36");
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
        let set_cookie = res.headers().get("set-cookie").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
        let cookie = (!set_cookie.is_empty()).then(|| set_cookie.split(';').next().unwrap_or("").to_string());
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        Resp { estado, json: serde_json::from_slice(&bytes).unwrap_or(Value::Null), cookie, set_cookie }
    }

    fn totp(secreto: &str, mas: i64) -> String {
        let paso = chrono::Utc::now().timestamp() / 30 + mas;
        format!("{:06}", crate::auth::hotp(&crate::auth::de_base32(secreto).unwrap(), paso as u64))
    }

    fn control(dir: &std::path::Path) -> rusqlite::Connection {
        let ruta = [dir.join("control.db"), dir.join("datos").join("control.db")].into_iter().find(|r| r.is_file()).expect("control.db");
        rusqlite::Connection::open(ruta).unwrap()
    }

    fn max_age(set_cookie: &str) -> i64 {
        set_cookie.split(';').find_map(|p| p.trim().strip_prefix("Max-Age=")).and_then(|v| v.parse().ok()).unwrap_or(-1)
    }

    #[tokio::test]
    async fn sesiones_recordar_lista_y_24_horas() {
        let dir = tempfile::tempdir().unwrap();
        let st = crate::preparar(dir.path(), crate::estado::Opciones { https: false, ..Default::default() }).unwrap();
        let app = crate::api::router(st.clone());
        let codigo = crate::api::preparar_codigo_arranque(&st).unwrap().unwrap();
        let contrasena = "una contraseña bien larga";
        let inicio = json!({ "codigo_arranque": codigo, "correo": "ana@ejemplo.com", "nombre": "Ana", "contrasena": contrasena });
        let r = pedir(&app, "POST", "/api/inicio", Some(inicio), None).await;
        let secreto = r.json["totp"]["secreto"].as_str().unwrap().to_string();
        let r = pedir(&app, "POST", "/api/sesion/totp", Some(json!({ "codigo": totp(&secreto, -1) })), r.cookie.as_deref()).await;
        assert_eq!(r.estado, StatusCode::OK);
        // Sin «Recordar»: la cookie, como mucho 24 h.
        assert!((max_age(&r.set_cookie) - 24 * 3600).abs() < 120, "{}", r.set_cookie);
        let a = r.cookie.unwrap();

        // Cuenta recién creada: su primera sesión puede todo (no hay sesiones antiguas que proteger).
        let r = pedir(&app, "GET", "/api/cuenta/sesiones", None, Some(&a)).await;
        assert_eq!(r.json["puede_cerrar"], json!(true));
        assert_eq!(r.json["sesiones"][0]["navegador"], json!("Chrome · Android"));
        assert_eq!(r.json["sesiones"][0]["actual"], json!(true));
        assert!(r.json["sesiones"][0].get("token_hash").is_none());

        // La cuenta y la sesión A pasan a ser antiguas.
        {
            let db = control(dir.path());
            db.execute("UPDATE cuentas SET creada = creada - 30 * 86400", []).unwrap();
            db.execute("UPDATE sesiones SET creada = creada - 25 * 3600", []).unwrap();
        }

        // Sesión B, con «Recordar este dispositivo»: 14 días.
        let r = pedir(&app, "POST", "/api/sesion", Some(json!({ "correo": "ana@ejemplo.com", "contrasena": contrasena, "recordar": true })), None).await;
        assert_eq!(r.json["necesita"], json!("totp"));
        let r = pedir(&app, "POST", "/api/sesion/totp", Some(json!({ "codigo": totp(&secreto, 0) })), r.cookie.as_deref()).await;
        assert_eq!(r.estado, StatusCode::OK);
        assert!((max_age(&r.set_cookie) - 14 * 86_400).abs() < 120, "{}", r.set_cookie);
        let b = r.cookie.unwrap();

        // B es nueva: ve las dos, pero no puede cerrar A ni cambiar la contraseña.
        let r = pedir(&app, "GET", "/api/cuenta/sesiones", None, Some(&b)).await;
        let sesiones = r.json["sesiones"].as_array().unwrap().clone();
        assert_eq!(sesiones.len(), 2);
        assert_eq!(r.json["puede_cerrar"], json!(false));
        assert!(r.json["puede_desde"].is_string());
        let id_a = sesiones.iter().find(|s| s["actual"] == json!(false)).unwrap()["id"].as_str().unwrap().to_string();
        let id_b = sesiones.iter().find(|s| s["actual"] == json!(true)).unwrap()["id"].as_str().unwrap().to_string();
        assert_eq!(sesiones.iter().find(|s| s["id"] == json!(id_b)).unwrap()["recordar"], json!(true));
        let r =
            pedir(&app, "POST", "/api/cuenta/sesiones/cerrar", Some(json!({ "ids": [id_a], "contrasena": contrasena, "codigo": totp(&secreto, 1) })), Some(&b))
                .await;
        assert_eq!(r.estado, StatusCode::FORBIDDEN);
        assert_eq!(r.json["error"], json!("sesion_nueva"), "{}", r.json);
        let r = pedir(&app, "PUT", "/api/cuenta/contrasena", Some(json!({ "actual": contrasena, "nueva": "otra contraseña muy larga" })), Some(&b)).await;
        assert_eq!(r.estado, StatusCode::FORBIDDEN);
        let r =
            pedir(&app, "POST", "/api/cuenta/sesiones/cerrar", Some(json!({ "otras": true, "contrasena": contrasena, "codigo": totp(&secreto, 1) })), Some(&b))
                .await;
        assert_eq!(r.estado, StatusCode::FORBIDDEN);

        // A ve el aviso de la sesión nueva.
        let r = pedir(&app, "GET", "/api/cuenta", None, Some(&a)).await;
        assert_eq!(r.json["sesiones_nuevas"][0]["id"], json!(id_b));
        assert_eq!(r.json["sesion"]["nueva_hasta"], Value::Null);
        // B no se ve a sí misma como nueva de otra.
        let r = pedir(&app, "GET", "/api/cuenta", None, Some(&b)).await;
        assert_eq!(r.json["sesiones_nuevas"], json!([]));
        assert!(r.json["sesion"]["nueva_hasta"].is_string());

        // A cierra B: sin la contraseña buena, no; con ella y el código, sí.
        let r = pedir(
            &app,
            "POST",
            "/api/cuenta/sesiones/cerrar",
            Some(json!({ "ids": [id_b], "contrasena": "no es esta", "codigo": totp(&secreto, 1) })),
            Some(&a),
        )
        .await;
        assert_eq!(r.estado, StatusCode::UNAUTHORIZED);
        // La propia, con «Salir».
        let r =
            pedir(&app, "POST", "/api/cuenta/sesiones/cerrar", Some(json!({ "ids": [id_a], "contrasena": contrasena, "codigo": totp(&secreto, 1) })), Some(&a))
                .await;
        assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY);
        let r =
            pedir(&app, "POST", "/api/cuenta/sesiones/cerrar", Some(json!({ "ids": [id_b], "contrasena": contrasena, "codigo": totp(&secreto, 1) })), Some(&a))
                .await;
        assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
        assert_eq!(r.json["cerradas"], json!(1));
        let r = pedir(&app, "GET", "/api/cuenta", None, Some(&b)).await;
        assert_eq!(r.estado, StatusCode::UNAUTHORIZED);

        // Plazos: solo el propietario, con topes.
        let r = pedir(&app, "PUT", "/api/servidor/sesiones", Some(json!({ "inactividad_horas": 0, "maximo_horas": 9999, "recordar_dias": 0 })), Some(&a)).await;
        assert_eq!(r.estado, StatusCode::OK);
        assert_eq!(r.json["plazos"], json!({ "inactividad_horas": 1, "maximo_horas": 168, "recordar_dias": 0 }));
        let r = pedir(&app, "GET", "/api/servidor", None, None).await;
        assert_eq!(r.json["sesion"]["recordar_dias"], json!(0));

        // Una sesión que pasó su máximo no vale, aunque se use.
        {
            let db = control(dir.path());
            db.execute("UPDATE sesiones SET maximo = strftime('%s','now') - 1", []).unwrap();
        }
        let r = pedir(&app, "GET", "/api/cuenta", None, Some(&a)).await;
        assert_eq!(r.estado, StatusCode::UNAUTHORIZED);

        // En la auditoría, el cierre (con el navegador, sin fichas).
        let db = control(dir.path());
        let datos: String = db.query_row("SELECT datos FROM auditoria WHERE accion = 'cerrar_sesion'", [], |r| r.get(0)).unwrap();
        assert!(datos.contains("Chrome · Android"), "{datos}");
    }
}
