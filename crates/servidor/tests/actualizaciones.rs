//! Actualización automática de los agentes (docs/actualizaciones.md): el
//! servidor como espejo solo acepta publicaciones firmadas, las sirve tal cual
//! a sus equipos y guarda la política de cada cliente y el anillo de cada equipo.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use base64::Engine;
use http_body_util::BodyExt;
use resguardo_protocolo::publicacion::{self as p, pruebas as pp};
use resguardo_protocolo::{claves, mensajes};
use resguardo_servidor::estado::{Opciones, St};
use resguardo_servidor::{api, auth, preparar};
use serde_json::{json, Value};
use tower::ServiceExt;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

struct Prueba {
    _dir: tempfile::TempDir,
    st: St,
    app: Router,
}

fn servidor(con_llave_de_pruebas: bool) -> Prueba {
    let dir = tempfile::tempdir().unwrap();
    let llaves_pruebas = con_llave_de_pruebas.then(|| pp::LLAVE_A_PUB.to_string());
    let st = preparar(dir.path(), Opciones { https: false, llaves_pruebas, ..Default::default() }).unwrap();
    let app = api::router(st.clone());
    Prueba { _dir: dir, st, app }
}

struct Resp {
    estado: StatusCode,
    json: Value,
    bytes: Vec<u8>,
    cookie: Option<String>,
}

async fn pedir_cuerpo(app: &Router, metodo: &str, ruta: &str, cuerpo: Body, json: bool, cookie: Option<&str>, extra: &[(&str, &str)]) -> Resp {
    let mut r = Request::builder().method(metodo).uri(ruta).header("host", "localhost");
    if metodo != "GET" {
        r = r.header("x-resguardo", "1");
    }
    if json {
        r = r.header("content-type", "application/json");
    }
    if let Some(c) = cookie {
        r = r.header("cookie", c);
    }
    for (k, v) in extra {
        r = r.header(*k, *v);
    }
    let res = app.clone().oneshot(r.body(cuerpo).unwrap()).await.unwrap();
    let estado = res.status();
    let cookie = res.headers().get("set-cookie").and_then(|v| v.to_str().ok()).and_then(|v| v.split(';').next()).map(str::to_string);
    let bytes = res.into_body().collect().await.unwrap().to_bytes().to_vec();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    Resp { estado, json, bytes, cookie }
}

async fn pedir(app: &Router, metodo: &str, ruta: &str, cuerpo: Option<Value>, cookie: Option<&str>, extra: &[(&str, &str)]) -> Resp {
    let (b, j) = match cuerpo {
        Some(v) => (Body::from(v.to_string()), true),
        None => (Body::empty(), false),
    };
    pedir_cuerpo(app, metodo, ruta, b, j, cookie, extra).await
}

fn codigo_totp(secreto: &str) -> String {
    let paso = chrono::Utc::now().timestamp() / 30;
    format!("{:06}", auth::hotp(&auth::de_base32(secreto).unwrap(), paso as u64))
}

async fn propietario(p: &Prueba) -> String {
    let codigo = api::preparar_codigo_arranque(&p.st).unwrap().unwrap();
    let cuerpo = json!({ "codigo_arranque": codigo, "correo": "ana@ejemplo.com", "nombre": "Ana", "contrasena": "una contraseña bien larga" });
    let r = pedir(&p.app, "POST", "/api/inicio", Some(cuerpo), None, &[]).await;
    let (cookie, secreto) = (r.cookie.unwrap(), r.json["totp"]["secreto"].as_str().unwrap().to_string());
    let r = pedir(&p.app, "POST", "/api/sesion/totp", Some(json!({ "codigo": codigo_totp(&secreto) })), Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    r.cookie.unwrap()
}

/// Un cliente con un equipo unido y confirmado: (cliente, equipo, «Authorization» del equipo).
async fn cliente_con_equipo(p: &Prueba, cookie: &str) -> (String, String, String) {
    let r = pedir(&p.app, "POST", "/api/clientes", Some(json!({ "nombre": "Ferretería Altamar", "espera_min_horas": 1 })), Some(cookie), &[]).await;
    let c = r.json["id"].as_str().unwrap().to_string();
    let r = pedir(&p.app, "POST", &format!("/api/clientes/{c}/emparejamientos"), None, Some(cookie), &[]).await;
    let (emp, codigo) = (r.json["id"].as_str().unwrap().to_string(), r.json["codigo"].as_str().unwrap().to_string());
    let box_pub = claves::public_of(&claves::new_key()).unwrap();
    let firma = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
    let cuerpo = json!({ "codigo_hash": mensajes::code_hash(&codigo), "nombre": "PC-01", "so": "windows", "version": "0.7.24", "box_pub": box_pub,
        "sign_pub": B64.encode(firma.verifying_key().to_bytes()), "sal_equipo": B64.encode([3u8; 16]), "sas_version": 3 });
    let r = pedir(&p.app, "POST", "/api/agente/unirse", Some(cuerpo), None, &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let (e, secreto) = (r.json["equipo_id"].as_str().unwrap().to_string(), r.json["secreto"].as_str().unwrap().to_string());
    let r = pedir(&p.app, "POST", &format!("/api/clientes/{c}/emparejamientos/{emp}/confirmar"), Some(json!({ "etiqueta": "E" })), Some(cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    (c, e.clone(), format!("Equipo {e}:{secreto}"))
}

fn publicacion(version: &str) -> (String, String, Vec<u8>, Vec<u8>) {
    let exe = format!("MZ instalador {version}").repeat(300).into_bytes();
    let tgz = format!("paquete {version}").repeat(200).into_bytes();
    let m = pp::manifiesto(version, &[("windows-x86_64", "Resguardo-Agente-setup.exe", &exe), ("linux-x86_64", "agente.tar.gz", &tgz)]);
    let texto = serde_json::to_string_pretty(&m).unwrap();
    let firma = pp::firmar_a(texto.as_bytes());
    (texto, firma, exe, tgz)
}

#[tokio::test]
async fn el_espejo_solo_acepta_lo_firmado_y_lo_sirve_tal_cual() {
    let p = servidor(true);
    let cookie = propietario(&p).await;
    let (c, e, auth_equipo) = cliente_con_equipo(&p, &cookie).await;
    let agente = [("authorization", auth_equipo.as_str())];
    let (texto, firma, exe, tgz) = publicacion("0.7.25");

    // Sin nada: la política por defecto y sin publicación.
    let r = pedir(&p.app, "GET", "/api/agente/actualizacion", None, None, &agente).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["publicacion"], Value::Null);
    assert_eq!((r.json["politica"]["modo"].as_str(), r.json["politica"]["anillo"].as_str()), (Some("auto"), Some("general")));
    // Sin equipo, nada.
    assert_eq!(pedir(&p.app, "GET", "/api/agente/actualizacion", None, None, &[]).await.estado, StatusCode::UNAUTHORIZED);

    // Sin firma buena, nada: de otra llave, manifiesto cambiado, basura.
    let mal = [
        json!({ "manifiesto": texto, "firma": pp::firmar_b(texto.as_bytes()) }),
        json!({ "manifiesto": texto.replace("0.7.25", "0.7.99"), "firma": firma }),
        json!({ "manifiesto": texto, "firma": "untrusted comment: x\nAAAA\n" }),
    ];
    for m in mal {
        let r = pedir(&p.app, "PUT", "/api/servidor/publicacion", Some(m), Some(&cookie), &[]).await;
        assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.json);
    }
    // Un archivo antes de su manifiesto: no.
    let r = pedir_cuerpo(&p.app, "PUT", "/api/servidor/publicacion/0.7.25/agente.tar.gz", Body::from(tgz.clone()), false, Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY);

    // El manifiesto firmado: se guarda, pero sin archivos no se sirve.
    let r = pedir(&p.app, "PUT", "/api/servidor/publicacion", Some(json!({ "manifiesto": texto, "firma": firma })), Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["vigente"], Value::Null);
    assert_eq!(r.json["guardadas"][0]["completa"], false);
    assert_eq!(pedir(&p.app, "GET", "/api/agente/actualizacion", None, None, &agente).await.json["publicacion"], Value::Null);

    // Archivos que no coinciden con el manifiesto: cambiados, más grandes o con otro nombre.
    let mut tocado = exe.clone();
    tocado[10] ^= 1;
    let r = pedir_cuerpo(&p.app, "PUT", "/api/servidor/publicacion/0.7.25/Resguardo-Agente-setup.exe", Body::from(tocado), false, Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.json);
    let mut grande = exe.clone();
    grande.push(0);
    let r = pedir_cuerpo(&p.app, "PUT", "/api/servidor/publicacion/0.7.25/Resguardo-Agente-setup.exe", Body::from(grande), false, Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY);
    let r = pedir_cuerpo(&p.app, "PUT", "/api/servidor/publicacion/0.7.25/otro.exe", Body::from(exe.clone()), false, Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY);

    // Los buenos: ya se sirve.
    for (n, b) in [("Resguardo-Agente-setup.exe", &exe), ("agente.tar.gz", &tgz)] {
        let r = pedir_cuerpo(&p.app, "PUT", &format!("/api/servidor/publicacion/0.7.25/{n}"), Body::from(b.clone()), false, Some(&cookie), &[]).await;
        assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    }
    let r = pedir(&p.app, "GET", "/api/servidor/publicacion", None, Some(&cookie), &[]).await;
    assert_eq!(r.json["vigente"]["version"], "0.7.25");
    let r = pedir(&p.app, "GET", "/api/agente/actualizacion", None, None, &agente).await;
    let publicacion = &r.json["publicacion"];
    assert_eq!(publicacion["manifiesto"].as_str(), Some(texto.as_str()), "el manifiesto, byte a byte");
    assert_eq!(publicacion["firma"].as_str(), Some(firma.as_str()));
    // Y el agente lo comprueba con su llave.
    let v = p::verificar(texto.as_bytes(), &firma, &pp::llaves_a(), &[], p::PRODUCTO_AGENTE).unwrap();
    assert_eq!(v.manifiesto.version, "0.7.25");
    let base = publicacion["archivos"].as_str().unwrap();
    let r = pedir(&p.app, "GET", &format!("{base}agente.tar.gz"), None, None, &agente).await;
    assert_eq!(r.estado, StatusCode::OK);
    assert_eq!(r.bytes, tgz);
    // Sin credencial de equipo, o con un nombre raro, nada.
    assert_eq!(pedir(&p.app, "GET", &format!("{base}agente.tar.gz"), None, None, &[]).await.estado, StatusCode::UNAUTHORIZED);
    assert_eq!(pedir(&p.app, "GET", &format!("{base}..%2Fdatos.db"), None, None, &agente).await.estado, StatusCode::NOT_FOUND);
    assert_eq!(pedir(&p.app, "GET", "/api/agente/actualizacion/archivos/0.7.24/agente.tar.gz", None, None, &agente).await.estado, StatusCode::NOT_FOUND);

    // La consola del cliente ve la versión disponible.
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/actualizaciones"), None, Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["disponible"]["version"], "0.7.25");
    assert_eq!(r.json["equipos"][&e]["anillo"], "general");
}

#[tokio::test]
async fn sin_llave_no_se_acepta_nada() {
    // El servidor compilado con el marcador de posición (sin llave fijada) y sin la de pruebas.
    if !p::es_marcador(resguardo_servidor::publicaciones::LLAVES_FIJADAS) {
        return; // ya hay llave de verdad: esta prueba no aplica
    }
    let p = servidor(false);
    let cookie = propietario(&p).await;
    let (texto, firma, _, _) = publicacion("0.7.25");
    let r = pedir(&p.app, "GET", "/api/servidor/publicacion", None, Some(&cookie), &[]).await;
    assert_eq!(r.json["sin_llave"], true);
    let r = pedir(&p.app, "PUT", "/api/servidor/publicacion", Some(json!({ "manifiesto": texto, "firma": firma })), Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(r.json["mensaje"].as_str().unwrap().contains("llave"));
}

#[tokio::test]
async fn politica_anillo_actualizar_ahora_y_retenidas() {
    let p = servidor(true);
    let cookie = propietario(&p).await;
    let (c, e, auth_equipo) = cliente_con_equipo(&p, &cookie).await;
    let agente = [("authorization", auth_equipo.as_str())];
    let politica = |p: &Prueba| {
        let app = p.app.clone();
        let a = auth_equipo.clone();
        async move { pedir(&app, "GET", "/api/agente/actualizacion", None, None, &[("authorization", a.as_str())]).await.json["politica"].clone() }
    };

    // Política del cliente: se valida.
    for mala in [
        json!({ "modo": "siempre" }),
        json!({ "modo": "auto", "dias_general": 31 }),
        json!({ "modo": "auto", "ventana": { "desde": "25:00", "hasta": "06:00" } }),
    ] {
        assert_eq!(
            pedir(&p.app, "PUT", &format!("/api/clientes/{c}/actualizaciones"), Some(mala), Some(&cookie), &[]).await.estado,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let r = pedir(
        &p.app,
        "PUT",
        &format!("/api/clientes/{c}/actualizaciones"),
        Some(json!({ "modo": "manual", "dias_general": 5, "ventana": { "desde": "22:00", "hasta": "06:00" } })),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let pol = politica(&p).await;
    assert_eq!((pol["modo"].as_str(), pol["dias_general"].as_u64(), pol["ventana"]["desde"].as_str()), (Some("manual"), Some(5), Some("22:00")));

    // Anillo del equipo.
    assert_eq!(
        pedir(&p.app, "PUT", &format!("/api/clientes/{c}/equipos/{e}/anillo"), Some(json!({ "anillo": "beta" })), Some(&cookie), &[]).await.estado,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        pedir(&p.app, "PUT", &format!("/api/clientes/{c}/equipos/no-existe/anillo"), Some(json!({ "anillo": "prueba" })), Some(&cookie), &[]).await.estado,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        pedir(&p.app, "PUT", &format!("/api/clientes/{c}/equipos/{e}/anillo"), Some(json!({ "anillo": "prueba" })), Some(&cookie), &[]).await.estado,
        StatusCode::OK
    );
    assert_eq!(politica(&p).await["anillo"], "prueba");

    // «Actualizar ahora» sin nada que dar: conflicto. Con una publicación: aprueba esa versión.
    assert_eq!(
        pedir(&p.app, "POST", &format!("/api/clientes/{c}/actualizaciones/ahora"), Some(json!({})), Some(&cookie), &[]).await.estado,
        StatusCode::CONFLICT
    );
    let carpeta = p._dir.path().join("origen");
    std::fs::create_dir_all(&carpeta).unwrap();
    let (texto, firma, exe, tgz) = publicacion("0.7.25");
    for (n, b) in [(p::NOMBRE_MANIFIESTO, texto.as_bytes()), (p::NOMBRE_FIRMA, firma.as_bytes()), ("Resguardo-Agente-setup.exe", &exe), ("agente.tar.gz", &tgz)]
    {
        std::fs::write(carpeta.join(n), b).unwrap();
    }
    resguardo_servidor::publicaciones::poner_carpeta(&p.st.datos, &resguardo_servidor::publicaciones::llaves(&p.st.opciones), &carpeta).unwrap();

    // Un equipo dice que la 0.7.25 falló (vuelta atrás): queda retenida para el cliente.
    let informe = json!({ "datos": { "version": "0.7.24", "actualizacion": { "estado": "vuelta_atras", "version_fallida": "0.7.25", "motivo": "No estuvo sana en 10 minutos." } } });
    assert_eq!(pedir(&p.app, "POST", "/api/agente/informe", Some(informe), None, &agente).await.estado, StatusCode::NO_CONTENT);
    assert_eq!(politica(&p).await["retenidas"], json!(["0.7.25"]));
    // Y el aviso para las personas.
    let r = pedir(
        &p.app,
        "POST",
        "/api/agente/aviso",
        Some(json!({ "tipo": "actualizacion_fallida", "mensaje": "La 0.7.25 no estuvo sana: volvió a la 0.7.24." })),
        None,
        &agente,
    )
    .await;
    assert_eq!(r.estado, StatusCode::NO_CONTENT, "{}", r.json);
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/avisos"), None, Some(&cookie), &[]).await;
    assert!(r.json.as_array().unwrap().iter().any(|a| a["tipo"] == "actualizacion_fallida"), "{}", r.json);

    // «Actualizar ahora» para el cliente: aprobada y ya no retenida.
    let r = pedir(&p.app, "POST", &format!("/api/clientes/{c}/actualizaciones/ahora"), Some(json!({})), Some(&cookie), &[]).await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    assert_eq!(r.json["version"], "0.7.25");
    let pol = politica(&p).await;
    assert_eq!((pol["aprobada"].as_str(), pol["retenidas"].clone()), (Some("0.7.25"), json!([])));
    // Para un equipo que no es del cliente: no existe.
    assert_eq!(
        pedir(&p.app, "POST", &format!("/api/clientes/{c}/actualizaciones/ahora"), Some(json!({ "equipo": "otro" })), Some(&cookie), &[]).await.estado,
        StatusCode::NOT_FOUND
    );

    // Todo queda en la auditoría del cliente.
    let r = pedir(&p.app, "GET", &format!("/api/clientes/{c}/auditoria"), None, Some(&cookie), &[]).await;
    let acciones: Vec<String> = r.json.as_array().map(|l| l.iter().filter_map(|x| x["accion"].as_str().map(str::to_string)).collect()).unwrap_or_default();
    for a in ["politica_actualizaciones", "anillo_equipo", "retener_version", "actualizar_ahora"] {
        assert!(acciones.iter().any(|x| x == a), "falta {a} en {acciones:?}");
    }
}

#[tokio::test]
async fn solo_el_propietario_del_servidor_pone_publicaciones() {
    let p = servidor(true);
    let cookie = propietario(&p).await;
    let (c, _, _) = cliente_con_equipo(&p, &cookie).await;
    // Una persona con rol de lectura en el cliente no puede subir nada ni cambiar la política.
    let r = pedir(&p.app, "POST", &format!("/api/clientes/{c}/invitaciones"), Some(json!({ "rol": "lectura" })), Some(&cookie), &[]).await;
    let token = r.json["enlace"].as_str().unwrap().split('#').nth(1).unwrap().to_string();
    let r = pedir(
        &p.app,
        "POST",
        "/api/invitaciones/aceptar",
        Some(json!({ "token": token, "correo": "lectura@ejemplo.com", "nombre": "Luis", "contrasena": "otra contraseña bien larga" })),
        None,
        &[],
    )
    .await;
    assert_eq!(r.estado, StatusCode::OK, "{}", r.json);
    let (c2, secreto) = (r.cookie.unwrap(), r.json["totp"]["secreto"].as_str().unwrap().to_string());
    let r = pedir(&p.app, "POST", "/api/sesion/totp", Some(json!({ "codigo": codigo_totp(&secreto) })), Some(&c2), &[]).await;
    let lectura = r.cookie.unwrap();
    let (texto, firma, _, _) = publicacion("0.7.25");
    assert_eq!(
        pedir(&p.app, "PUT", "/api/servidor/publicacion", Some(json!({ "manifiesto": texto, "firma": firma })), Some(&lectura), &[]).await.estado,
        StatusCode::FORBIDDEN
    );
    assert_eq!(pedir(&p.app, "GET", "/api/servidor/publicacion", None, Some(&lectura), &[]).await.estado, StatusCode::FORBIDDEN);
    assert_eq!(
        pedir(&p.app, "PUT", &format!("/api/clientes/{c}/actualizaciones"), Some(json!({ "modo": "pausada" })), Some(&lectura), &[]).await.estado,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        pedir(&p.app, "POST", &format!("/api/clientes/{c}/actualizaciones/ahora"), Some(json!({})), Some(&lectura), &[]).await.estado,
        StatusCode::FORBIDDEN
    );
    // Pero ve la versión y la política.
    assert_eq!(pedir(&p.app, "GET", &format!("/api/clientes/{c}/actualizaciones"), None, Some(&lectura), &[]).await.estado, StatusCode::OK);
}
