//! Rutas HTTP de la consola y de los agentes (docs/api-servidor.md).

mod clientes;
mod cuentas;
mod equipos;
pub(crate) mod etiquetas;
pub(crate) mod instaladores;
pub(crate) mod marca;
mod notas;
mod notificaciones;
mod ordenes;
mod panel;
mod plantillas;
mod respaldo;
mod servidor_clientes;
mod sesiones;

pub use cuentas::preparar_codigo_arranque;
pub(crate) use etiquetas::normalizar_etiqueta;
pub use sesiones::{dir_relevo, esperar_mensajes, MAX_MENSAJE as MAX_MENSAJE_SESION};

use crate::almacen::{Equipo, Orden, Ts};
use crate::estado::St;
use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post, put};
use axum::Router;
use serde_json::{json, Value};

/// Fecha RFC 3339 (hora local del servidor, con su zona).
pub fn fecha(ts: Ts) -> String {
    chrono::DateTime::from_timestamp(ts, 0).map(|d| d.with_timezone(&chrono::Local).to_rfc3339()).unwrap_or_default()
}

pub fn fecha_opt(ts: Option<Ts>) -> Value {
    ts.map(|t| Value::String(fecha(t))).unwrap_or(Value::Null)
}

pub fn de_fecha(s: &str) -> Option<Ts> {
    chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.timestamp())
}

pub fn equipo_json(e: &Equipo, conectado: bool) -> Value {
    json!({
        "id": e.id, "nombre": e.nombre, "so": e.so, "version_agente": e.version_agente,
        "box_pub": e.box_pub, "sign_pub": e.sign_pub, "sal_equipo": e.sal_equipo, "etiqueta": e.etiqueta,
        "rol": e.rol, "modo": e.modo, "confirmado": e.confirmado, "conectado": conectado,
        "ultimo_contacto": fecha_opt(e.ultimo_contacto), "estado_servicio": e.estado_servicio,
        "siguiente_seq": e.siguiente_seq, "resumen": e.resumen,
        // La que confirmó el equipo (resultado firmado de cambiar_espera), si la hay.
        "espera_min_horas": e.espera_min_horas,
        // v1.18: etiquetas libres para agrupar (en claro; no es la `etiqueta` HMAC).
        "etiquetas": e.etiquetas,
    })
}

pub fn orden_json(o: &Orden, nombre_emisor: &str) -> Value {
    json!({
        "id": o.id, "equipo": o.equipo_id, "tipo": o.tipo, "seq": o.seq,
        "emitida": fecha(o.emitida), "emitida_por": { "id": o.emitida_por, "nombre": nombre_emisor },
        "not_before": fecha_opt(o.not_before), "caduca": fecha(o.caduca),
        "estado": o.estado, "mensaje": o.mensaje, "detalle": o.detalle, "firma_agente": o.firma_agente,
        "actualizada": fecha(o.actualizada),
    })
}

/// v1.4x (consolas-multiples.md §5): las órdenes que los equipos tienen en espera y que
/// mandó **otra** consola (las de esta ya cuentan en `ordenes_con_espera`). Del resumen
/// que sube cada equipo; las que ya pasaron su hora no cuentan.
pub fn en_espera_de_otras(equipos: &[crate::almacen::Equipo], ahora: Ts) -> usize {
    equipos
        .iter()
        .filter_map(|e| e.resumen.as_ref()?.get("en_espera")?.as_array())
        .flatten()
        .filter(|x| x["consola"]["esta"] != true && x["caduca"].as_str().and_then(de_fecha).is_some_and(|c| c > ahora))
        .count()
}

/// La orden como la recibe el agente.
pub fn orden_agente(o: &Orden) -> Value {
    json!({ "id": o.id, "tipo": o.tipo, "seq": o.seq, "sellado": o.sellado, "not_before": fecha_opt(o.not_before), "caduca": fecha(o.caduca) })
}

/// ¿Cabe otro equipo en el cliente (su cuota, v1.34)? Antes de dar un código o un instalador.
pub(crate) async fn cabe_otro_equipo(st: &St, ctx: &crate::almacen::ClienteCtx) -> crate::error::Res<()> {
    let (ctx, publico) = (ctx.clone(), st.opciones.publico);
    let r = st.db(move |db| crate::cuotas::cabe_otro_equipo(db, publico, ctx.id(), db.equipos(&ctx)?.len())).await?;
    r.map_err(crate::agentes::error_cuota)
}

/// Cabeceras de seguridad en todas las respuestas.
async fn cabeceras(State(st): State<St>, req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self' https://api.dropboxapi.com; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'",
        ),
    );
    h.insert(header::HeaderName::from_static("cross-origin-opener-policy"), HeaderValue::from_static("same-origin"));
    // Nada de otros sitios puede incrustar lo que sirve este servidor (ni leerlo con <img>, <script>…).
    h.insert(header::HeaderName::from_static("cross-origin-resource-policy"), HeaderValue::from_static("same-origin"));
    // La consola no usa ninguna de estas capacidades del navegador.
    h.insert(
        header::HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=(), usb=(), serial=(), hid=(), bluetooth=(), midi=(), interest-cohort=()"),
    );
    h.insert(header::HeaderName::from_static("x-permitted-cross-domain-policies"), HeaderValue::from_static("none"));
    if st.opciones.https {
        h.insert(header::STRICT_TRANSPORT_SECURITY, HeaderValue::from_static("max-age=31536000"));
    }
    if !h.contains_key(header::CACHE_CONTROL) && res.status() != StatusCode::NOT_MODIFIED {
        res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    res
}

/// Defensa CSRF de la consola: lo que cambia algo lleva `X-Resguardo: 1` (una
/// cabecera propia: otro sitio no la puede poner sin CORS, que el servidor no
/// da), si hay `Origin` es el propio servidor y, si el navegador manda
/// `Sec-Fetch-Site`, la petición sale de la misma página (`same-origin`).
async fn csrf(req: Request, next: Next) -> Response {
    let cambia = !matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    if cambia {
        let h = req.headers();
        let sitio_ok = h.get("sec-fetch-site").map(|v| v.to_str().ok()).is_none_or(|s| matches!(s, Some("same-origin" | "none")));
        let cabecera = sitio_ok && h.get("x-resguardo").and_then(|v| v.to_str().ok()) == Some("1");
        let origen_ok = match (h.get(header::ORIGIN).and_then(|v| v.to_str().ok()), h.get(header::HOST).and_then(|v| v.to_str().ok())) {
            (Some(origen), Some(host)) => origen == format!("https://{host}") || origen == format!("http://{host}"),
            (Some(_), None) => false,
            (None, _) => true,
        };
        if !cabecera || !origen_ok {
            return crate::error::ErrorApi::nuevo(
                StatusCode::FORBIDDEN,
                "csrf",
                "Petición rechazada (falta la cabecera X-Resguardo o el origen no es este servidor).",
            )
            .into_response();
        }
    }
    next.run(req).await
}

/// El cliente de una ruta `/api/clientes/{c}/…` (solo si `{c}` es un id: las rutas
/// fijas como `/api/clientes/recibir` no lo son).
pub fn cliente_de_ruta(ruta: &str) -> Option<&str> {
    let c = ruta.strip_prefix("/api/clientes/")?.split('/').next()?;
    uuid::Uuid::parse_str(c).is_ok().then_some(c)
}

/// Aislamiento entre clientes, también por si una ruta se olvidara de comprobarlo
/// (v1.34): todo lo que cuelga de `/api/clientes/{c}` exige, antes de leer el cuerpo
/// y de llegar a la ruta, una sesión completa de alguien que es miembro de ese
/// cliente. Para quien no lo es, el cliente «no existe» (como en cada ruta, que
/// además comprueba el rol). Ni el propietario del servidor entra sin ser miembro.
async fn guardia_cliente(State(st): State<St>, req: Request, next: Next) -> Response {
    if let Some(c) = cliente_de_ruta(req.uri().path()) {
        let s = match crate::auth::sesion_de_cabeceras(req.headers(), &st).await {
            Ok(s) if s.aal >= 2 => s,
            Ok(_) => return crate::error::ErrorApi::necesita_totp().into_response(),
            Err(e) => return e.into_response(),
        };
        let (cuenta, c) = (s.cuenta.id, c.to_string());
        match st.db(move |db| db.rol(&cuenta, &c)).await {
            Ok(Some(_)) => {}
            Ok(None) => return crate::error::ErrorApi::no_existe().into_response(),
            Err(e) => return e.into_response(),
        }
    }
    next.run(req).await
}

/// Un cuerpo que deja de llegar (slowloris) corta la petición: tanto tiempo sin
/// recibir nada. Las cabeceras tienen su propio límite (`servir`).
pub const ESPERA_CUERPO: std::time::Duration = std::time::Duration::from_secs(30);

async fn cuerpo_con_espera(req: Request, next: Next) -> Response {
    let (partes, cuerpo) = req.into_parts();
    let cuerpo = axum::body::Body::new(tower_http::timeout::TimeoutBody::new(ESPERA_CUERPO, cuerpo));
    next.run(Request::from_parts(partes, cuerpo)).await
}

pub fn router(st: St) -> Router {
    let consola = Router::new()
        // Servidor y cuentas
        .route("/api/servidor", get(cuentas::servidor))
        .route("/api/servidor/ca", get(cuentas::servidor_ca))
        .route("/api/servidor/respaldo", get(respaldo::ver).put(respaldo::cambiar))
        .route("/api/servidor/respaldo/ahora", post(respaldo::ahora))
        // Notificaciones (canales del servidor, registro)
        .route("/api/servidor/notificaciones", get(notificaciones::ver).put(notificaciones::cambiar))
        .route("/api/servidor/notificaciones/canales", post(notificaciones::crear_canal))
        .route("/api/servidor/notificaciones/canales/{k}", patch(notificaciones::cambiar_canal).delete(notificaciones::borrar_canal))
        .route("/api/servidor/notificaciones/canales/{k}/prueba", post(notificaciones::probar_canal))
        .route("/api/servidor/notificaciones/registro", get(notificaciones::registro_servidor))
        // Clientes del servidor (v1.34: solo cifras; sin entrar en los clientes)
        .route("/api/servidor/clientes", get(servidor_clientes::listar).post(servidor_clientes::crear_para_otro))
        .route("/api/servidor/clientes/{c}/cuotas", put(servidor_clientes::poner_cuotas))
        .route("/api/servidor/clientes/{c}/invitacion", post(servidor_clientes::invitar_propietario))
        .route("/api/servidor/cuotas", put(servidor_clientes::poner_predeterminadas))
        .route("/api/inicio", post(cuentas::inicio))
        .route("/api/sesion", post(cuentas::entrar).delete(cuentas::salir))
        .route("/api/sesion/totp", post(cuentas::totp))
        .route("/api/sesion/restablecimiento", post(cuentas::usar_restablecimiento))
        .route("/api/cuenta", get(cuentas::cuenta).patch(cuentas::renombrar))
        .route("/api/cuenta/totp", post(cuentas::totp_nuevo))
        .route("/api/cuenta/totp/confirmar", post(cuentas::totp_confirmar))
        .route("/api/cuenta/recuperacion", post(cuentas::codigos_nuevos))
        .route("/api/cuenta/notificaciones", get(notificaciones::mias).put(notificaciones::cambiar_mias))
        .route("/api/cuenta/contrasena", put(cuentas::cambiar_contrasena))
        .route("/api/invitaciones/aceptar", post(cuentas::aceptar_invitacion))
        // Clientes
        .route("/api/clientes", get(clientes::listar).post(clientes::crear))
        .route("/api/clientes/recibir", post(clientes::recibir))
        // v1.38: «Todos los clientes» (solo los clientes de los que la cuenta es miembro).
        .route("/api/panel", get(panel::ver))
        .route("/api/panel/progreso", get(panel::progreso))
        .route("/api/clientes/{c}/fichas", post(clientes::ficha))
        .route(
            "/api/clientes/{c}/paquete",
            get(clientes::ver_paquete).put(clientes::poner_paquete).delete(clientes::borrar_paquete).layer(DefaultBodyLimit::max(clientes::MAX_PAQUETE)),
        )
        .route("/api/clientes/{c}/importar", post(clientes::importar).layer(DefaultBodyLimit::max(64 * 1024 * 1024)))
        .route("/api/clientes/{c}/auditoria/importada", get(clientes::auditoria_importada))
        .route("/api/clientes/{c}", get(clientes::ver).patch(clientes::renombrar))
        // v1.32: marca del cliente (logo PNG y acento).
        .route("/api/clientes/{c}/marca", get(marca::ver).put(marca::cambiar))
        .route("/api/clientes/{c}/marca/logo", get(marca::logo))
        .route("/api/clientes/{c}/miembros", get(clientes::miembros))
        .route("/api/clientes/{c}/miembros/{cuenta}", put(clientes::poner_rol).delete(clientes::quitar))
        .route("/api/clientes/{c}/miembros/{cuenta}/restablecer-totp", post(cuentas::restablecer_totp))
        .route("/api/clientes/{c}/invitaciones", post(clientes::invitar))
        .route("/api/clientes/{c}/notificaciones", get(notificaciones::ver_cliente))
        .route("/api/clientes/{c}/notificaciones/canales", post(notificaciones::crear_canal_cliente))
        .route("/api/clientes/{c}/notificaciones/canales/{k}", patch(notificaciones::cambiar_canal_cliente).delete(notificaciones::borrar_canal_cliente))
        .route("/api/clientes/{c}/notificaciones/canales/{k}/prueba", post(notificaciones::probar_canal_cliente))
        .route("/api/clientes/{c}/notificaciones/registro", get(notificaciones::registro_cliente))
        .route("/api/clientes/{c}/notificaciones/personas", get(notificaciones::personas))
        .route("/api/clientes/{c}/notificaciones/personas/{cuenta}", put(notificaciones::poner_prefs))
        // Equipos y emparejamiento
        .route("/api/clientes/{c}/resumen", get(equipos::resumen))
        .route("/api/clientes/{c}/equipos", get(equipos::listar))
        .route("/api/clientes/{c}/equipos/{e}", get(equipos::ver).patch(equipos::renombrar))
        .route("/api/clientes/{c}/equipos/{e}/atencion", post(equipos::atencion))
        .route("/api/clientes/{c}/equipos/{e}/etiquetas", put(equipos::poner_etiquetas))
        .route("/api/clientes/{c}/equipos/{e}/config", get(equipos::config))
        .route("/api/clientes/{c}/equipos/{e}/informes", get(equipos::informes))
        .route("/api/clientes/{c}/equipos/{e}/historial", get(equipos::historial))
        .route("/api/clientes/{c}/informes", get(equipos::ultimos_informes))
        .route("/api/clientes/{c}/progreso", get(crate::progreso::ver))
        // v1.39: canal en vivo de la consola (WebSocket con pistas de lo que cambió).
        .route("/api/clientes/{c}/vivo", get(crate::vivo::canal))
        .route("/api/clientes/{c}/emparejamientos", post(equipos::abrir_emparejamiento).get(instaladores::listar))
        .route("/api/clientes/{c}/instaladores", post(instaladores::preparar))
        .route("/api/clientes/{c}/instalador-agente", get(instaladores::instalador_generico))
        .route("/api/clientes/{c}/preparados", get(instaladores::contar))
        .route("/api/clientes/{c}/codigo-abierto", get(instaladores::codigo_abierto))
        .route("/api/clientes/{c}/a-medias", get(instaladores::a_medias))
        .route("/api/clientes/{c}/equipo-local", post(instaladores::vincular_local))
        .route("/api/clientes/{c}/plantillas", get(plantillas::listar))
        // v1.4x: color, plantilla por defecto y avisos de cada etiqueta de los equipos.
        .route("/api/clientes/{c}/etiquetas", get(etiquetas::listar).put(etiquetas::poner))
        .route("/api/clientes/{c}/plantillas/{p}", put(plantillas::guardar).delete(plantillas::borrar))
        .route("/api/clientes/{c}/emparejamientos/{p}", get(equipos::ver_emparejamiento).delete(equipos::cancelar_emparejamiento))
        .route("/api/clientes/{c}/emparejamientos/{p}/confirmar", post(equipos::confirmar_emparejamiento))
        .route("/api/clientes/{c}/avisos", get(equipos::avisos))
        .route("/api/clientes/{c}/avisos/{a}/visto", post(equipos::aviso_visto))
        .route("/api/clientes/{c}/auditoria", get(equipos::auditoria))
        .route("/api/clientes/{c}/auditoria/verificar", get(equipos::verificar_auditoria))
        // v1.40: observaciones y comentarios
        .route("/api/clientes/{c}/notas", get(notas::indice))
        .route("/api/clientes/{c}/notas/objeto", get(notas::ver))
        .route("/api/clientes/{c}/notas/todas", get(notas::todas))
        .route("/api/clientes/{c}/notas/observacion", put(notas::poner_observacion))
        .route("/api/clientes/{c}/notas/comentarios", post(notas::comentar))
        .route("/api/clientes/{c}/notas/comentarios/{k}", patch(notas::editar).delete(notas::borrar))
        // Órdenes
        .route("/api/clientes/{c}/equipos/{e}/ordenes", get(ordenes::listar).post(ordenes::enviar))
        .route("/api/clientes/{c}/ordenes", get(ordenes::listar_cliente))
        .route("/api/clientes/{c}/ordenes/{o}/cancelar", post(ordenes::cancelar))
        // Sesiones interactivas y relé
        .route("/api/clientes/{c}/sesiones/{s}/mensajes", get(sesiones::leer).post(sesiones::escribir))
        .route("/api/clientes/{c}/sesiones/{s}", delete(sesiones::cerrar))
        .route("/api/clientes/{c}/relevos/{r}", get(sesiones::relevo).delete(sesiones::borrar_relevo))
        .route("/api/clientes/{c}/relevos/{r}/trozos/{n}", get(sesiones::trozo))
        .layer(middleware::from_fn(csrf))
        .layer(middleware::from_fn_with_state(st.clone(), guardia_cliente))
        .layer(DefaultBodyLimit::max(1024 * 1024));

    let agentes = crate::agentes::router();

    let web = match &st.opciones.consola {
        Some(dir) => {
            Router::new().fallback_service(tower_http::services::ServeDir::new(dir).fallback(tower_http::services::ServeFile::new(dir.join("index.html"))))
        }
        #[cfg(feature = "consola-integrada")]
        None if crate::consola_integrada::disponible() => Router::new().fallback(crate::consola_integrada::servir),
        None => Router::new().route("/", get(pagina_minima)),
    };

    Router::new()
        .merge(consola)
        .merge(agentes)
        .merge(web)
        .route("/api/{*resto}", patch(no_existe).get(no_existe).post(no_existe).put(no_existe).delete(no_existe))
        .layer(middleware::from_fn(cuerpo_con_espera))
        .layer(middleware::from_fn_with_state(st.clone(), limite_y_registro))
        .layer(middleware::from_fn_with_state(st.clone(), cabeceras))
        .with_state(st)
}

/// Peticiones a la API por IP y minuto, de todo tipo (además de los límites de
/// cada ruta: entrar, códigos, órdenes…). Una oficina con muchos equipos tras
/// una sola IP cabe de sobra; un barrido, no.
pub const MAX_PETICIONES_IP_MIN: u32 = 1_800;

/// Límite general por IP y registro de los accesos fallidos (para fail2ban,
/// `crate::registro`). Las peticiones sin IP (pruebas) o de este mismo equipo no
/// tienen límite. Un límite superado se anota como mucho una vez por minuto e IP.
async fn limite_y_registro(State(st): State<St>, req: Request, next: Next) -> Response {
    use std::time::Duration;
    let ip = req.extensions().get::<crate::estado::IpCliente>().and_then(|i| i.0);
    let (metodo, ruta) = (req.method().to_string(), req.uri().path().to_string());
    // Un límite superado: una línea por minuto, IP y límite, en la salida (fail2ban, journald) y
    // en servidor.log de la carpeta de datos (el servicio de Windows no tiene otra salida).
    let anotar_limite = |ip: std::net::IpAddr, limite: &str| {
        if st.limites.intento(&format!("registro-limite:{limite}:{}", crate::estado::clave_ip(Some(ip))), 1, Duration::from_secs(60)) {
            let linea = crate::registro::linea_limite(ip, limite, &metodo, &ruta);
            println!("{linea}");
            crate::registro::al_archivo(&st.datos, &linea);
        }
    };
    if let Some(ip) = ip.filter(|ip| !ip.is_loopback()) {
        let clave = format!("api-ip:{}", crate::estado::clave_ip(Some(ip)));
        if ruta.starts_with("/api/") {
            if let Err(espera) = st.limites.intento_o_espera(&clave, MAX_PETICIONES_IP_MIN, Duration::from_secs(60)) {
                anotar_limite(ip, "ip");
                return crate::error::ErrorApi::demasiados_esperar(
                    "ip",
                    "Demasiadas peticiones desde tu red en el último minuto. Vuelve a intentarlo en un momento.",
                    espera,
                )
                .into_response();
            }
        }
    }
    let res = next.run(req).await;
    if let Some(ip) = ip {
        if let Some(crate::error::AccesoFallido(que)) = res.extensions().get::<crate::error::AccesoFallido>() {
            println!("{}", crate::registro::linea_acceso_fallido(ip, que, &metodo, &ruta));
        } else if res.status() == StatusCode::TOO_MANY_REQUESTS {
            anotar_limite(ip, res.extensions().get::<crate::error::LimiteSuperado>().map_or("intentos", |l| l.0));
        }
    }
    res
}

async fn no_existe() -> crate::error::ErrorApi {
    crate::error::ErrorApi::no_existe()
}

async fn pagina_minima() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        "<!doctype html><html lang=\"es\"><meta charset=\"utf-8\"><title>Resguardo Server</title>\
         <body style=\"font-family:system-ui,sans-serif;max-width:40rem;margin:4rem auto;padding:0 1rem;color:#222\">\
         <h1>Resguardo Server</h1><p>El servidor está en marcha. La consola web aún no está instalada en este servidor \
         (opción <code>--consola</code>).</p></body></html>",
    )
}
