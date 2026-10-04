//! Resguardo Server: consola web, API y canal de los agentes.
//! Diseño: docs/plataforma.md (§2 y §3). Contrato: docs/api-servidor.md.
//!
//! El servidor no tiene autoridad sobre los equipos: guarda y reenvía órdenes
//! selladas que solo el equipo destinatario puede abrir y que solo acepta con
//! la contraseña del repositorio o la clave de administración del cliente.

pub mod acme;
pub mod agentes;
pub mod almacen;
pub mod api;
pub mod auth;
#[cfg(feature = "consola-integrada")]
pub mod consola_integrada;
pub mod cuotas;
pub mod error;
pub mod estado;
pub mod identidad;
pub mod instalador_agente;
pub mod notificaciones;
pub mod pistas;
pub mod progreso;
#[cfg(test)]
mod propiedades;
pub mod registro;
pub mod respaldo;

use estado::{Estado, IpCliente, Limites, Opciones, St};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Abre (o crea) los datos del servidor en `datos` y prepara el estado.
pub fn preparar(datos: &Path, opciones: Opciones) -> Result<St, String> {
    std::fs::create_dir_all(datos).map_err(|e| format!("No se pudo crear {}: {e}", datos.display()))?;
    let db: Arc<dyn almacen::Almacen> = Arc::new(almacen::sqlite::Sqlite::abrir(datos)?);
    let identidad = identidad::identidad(datos)?;
    let huella_ca = std::fs::read_to_string(datos.join("tls").join("ca.huella")).map(|s| s.trim().to_string()).unwrap_or_default();
    Ok(Arc::new(Estado {
        db,
        identidad_pub: identidad::publica(&identidad),
        identidad: identidad.clone(),
        huella_ca,
        datos: datos.to_path_buf(),
        opciones,
        conectados: Mutex::new(Default::default()),
        cambios: tokio::sync::Notify::new(),
        limites: Limites::default(),
        progreso: Default::default(),
        notif: notificaciones::Motor::nuevo(notificaciones::cifrado::Clave::de_identidad(&identidad)),
    }))
}

/// Tareas de fondo: entregar las órdenes con espera cuando llega su hora a
/// los agentes conectados, y limpiar lo caducado.
pub fn tareas(st: St) {
    notificaciones::arrancar(st.clone());
    let st1 = st.clone();
    tokio::spawn(async move {
        let mut t = tokio::time::interval(Duration::from_secs(30));
        loop {
            t.tick().await;
            let conectados: Vec<String> = st1.conectados.lock().unwrap_or_else(|e| e.into_inner()).keys().cloned().collect();
            for equipo in conectados {
                let e2 = equipo.clone();
                if let Ok(Some(ctx)) = st1.db(move |db| db.cliente_de_equipo(&e2)).await {
                    agentes::empujar(&st1, &ctx, &equipo).await;
                }
            }
        }
    });
    // La copia de la consola (cada noche, si está puesta).
    let st3 = st.clone();
    tokio::spawn(async move {
        let mut t = tokio::time::interval(Duration::from_secs(300));
        loop {
            t.tick().await;
            let (datos, identidad) = (st3.datos.clone(), st3.identidad.clone());
            let _ = tokio::task::spawn_blocking(move || respaldo::si_toca(&datos, &identidad)).await;
        }
    });
    tokio::spawn(async move {
        let mut t = tokio::time::interval(Duration::from_secs(600));
        loop {
            t.tick().await;
            let st2 = st.clone();
            let datos = st.datos.clone();
            let _ = tokio::task::spawn_blocking(move || {
                let ahora = almacen::ahora();
                let _ = st2.db.limpiar_servidor(ahora);
                for ctx in st2.db.todos_los_clientes().unwrap_or_default() {
                    if let Ok(caducados) = st2.db.limpiar(&ctx, ahora) {
                        for r in caducados {
                            if let Some(dir) = api::dir_relevo(&st2, ctx.id(), &r) {
                                let _ = std::fs::remove_dir_all(dir);
                            }
                        }
                    }
                    avisar_sin_contacto(&st2, &ctx, ahora);
                }
                drop(datos);
            })
            .await;
        }
    });
}

/// Aviso si un equipo lleva más de 24 h sin conectar (uno por equipo mientras siga abierto).
fn avisar_sin_contacto(st: &St, ctx: &almacen::ClienteCtx, ahora: almacen::Ts) {
    let (Ok(equipos), Ok(avisos)) = (st.db.equipos(ctx), st.db.avisos(ctx, true)) else { return };
    for e in equipos.iter().filter(|e| e.confirmado && e.modo == "gestionado") {
        let callado = e.ultimo_contacto.is_some_and(|t| ahora - t > 24 * 3600);
        let ya = avisos.iter().any(|a| a.tipo == "equipo_sin_contacto" && a.equipo.as_deref() == Some(&e.id));
        if callado && !ya {
            let _ = notificaciones::aviso(
                st.db.as_ref(),
                ctx,
                Some(&e.id),
                "equipo_sin_contacto",
                &format!("«{}» lleva más de 24 h sin conectar con el servidor.", e.nombre),
            );
        }
    }
}

/// Dónde escuchar y con qué TLS.
pub enum Tls {
    /// Certificado propio (de la carpeta de datos) o los indicados.
    Propio { cert: PathBuf, clave: PathBuf },
    /// Consola en internet: certificado público automático (ACME) para el
    /// dominio y el propio (`cert`, `clave`) para todo lo demás, que es lo que
    /// fijan los agentes. Los retos HTTP-01 y la redirección, en `http`.
    Publico { cert: PathBuf, clave: PathBuf, acme: acme::ConfigAcme, http: SocketAddr },
    /// Sin TLS (solo detrás de un proxy con HTTPS, o en pruebas).
    Ninguno,
}

/// Conexiones abiertas a la vez, como mucho (cada agente conectado tiene la
/// suya, con su WebSocket). Las de más se cierran nada más aceptarlas.
pub const MAX_CONEXIONES: usize = 10_000;
/// Y desde una misma IP (salvo este mismo equipo, p. ej. un proxy): una oficina
/// con muchos equipos tras una sola IP cabe; una sola IP no se queda con todas.
pub const MAX_CONEXIONES_IP: usize = 1_000;
/// Tiempo para el saludo TLS y para recibir las cabeceras de cada petición
/// (también la siguiente de una conexión que se queda abierta): contra quien
/// abre conexiones y no termina de hablar (slowloris). El cuerpo tiene el suyo
/// (`api::ESPERA_CUERPO`).
pub const ESPERA_CABECERAS: Duration = Duration::from_secs(20);
const ESPERA_TLS: Duration = Duration::from_secs(10);

/// Sirve hasta que se pare el proceso. Con TLS o sin él, el mismo bucle: cada
/// conexión con su límite de tiempo para las cabeceras y un tope de conexiones.
pub async fn servir(st: St, direccion: SocketAddr, tls: Tls) -> Result<(), String> {
    let app = api::router(st.clone());
    let proxy = st.opciones.proxy;
    let datos = st.datos.clone();
    tareas(st);
    let listener = tokio::net::TcpListener::bind(direccion).await.map_err(|e| format!("No se pudo escuchar en {direccion}: {e}"))?;
    let acceptor = match tls {
        Tls::Propio { cert, clave } => Some(tokio_rustls::TlsAcceptor::from(identidad::config_rustls(&cert, &clave)?)),
        Tls::Publico { cert, clave, acme: cfg, http } => {
            let leer = |p: &Path| std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()));
            let propio = acme::certificado_rustls(&leer(&cert)?, &leer(&clave)?)?;
            let certs = Arc::new(acme::Certificados::nuevos(&cfg.dominio, propio));
            let retos = acme::Retos::default();
            let oyente80 = tokio::net::TcpListener::bind(http)
                .await
                .map_err(|e| format!("No se pudo escuchar en {http} (lo necesita el certificado público, reto HTTP-01): {e}"))?;
            tokio::spawn(bucle(oyente80, None, acme::app_http(retos.clone(), cfg.dominio.clone(), direccion.port()), false));
            acme::arrancar(cfg, acme::carpeta(&datos), retos, certs.clone());
            Some(tokio_rustls::TlsAcceptor::from(acme::config_rustls(certs)?))
        }
        Tls::Ninguno => None,
    };
    bucle(listener, acceptor, app, proxy).await;
    Ok(())
}

/// Cuenta las conexiones abiertas de una IP mientras vive.
struct ConexionIp(Arc<Mutex<HashMap<IpAddr, usize>>>, IpAddr);

impl Drop for ConexionIp {
    fn drop(&mut self) {
        let mut m = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(n) = m.get_mut(&self.1) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                m.remove(&self.1);
            }
        }
    }
}

/// Una conexión más de esa IP, si cabe (las de este mismo equipo no cuentan).
fn contar_conexion(por_ip: &Arc<Mutex<HashMap<IpAddr, usize>>>, ip: IpAddr, maximo: usize) -> Result<Option<ConexionIp>, ()> {
    if ip.is_loopback() {
        return Ok(None);
    }
    let mut m = por_ip.lock().unwrap_or_else(|e| e.into_inner());
    let n = m.entry(ip).or_insert(0);
    if *n >= maximo {
        return Err(());
    }
    *n += 1;
    Ok(Some(ConexionIp(por_ip.clone(), ip)))
}

/// El bucle que acepta conexiones (con TLS o sin él) y las atiende.
async fn bucle(listener: tokio::net::TcpListener, acceptor: Option<tokio_rustls::TlsAcceptor>, app: axum::Router, proxy: bool) {
    let cupo = Arc::new(tokio::sync::Semaphore::new(MAX_CONEXIONES));
    let por_ip: Arc<Mutex<HashMap<IpAddr, usize>>> = Default::default();
    loop {
        let (tcp, ip) = match listener.accept().await {
            Ok(c) => c,
            Err(_) => {
                // Sin descriptores libres (o parecido): no dar vueltas en vacío.
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }
        };
        let Ok(permiso) = cupo.clone().try_acquire_owned() else { continue };
        let Ok(de_ip) = contar_conexion(&por_ip, ip.ip(), MAX_CONEXIONES_IP) else { continue };
        let (acceptor, app) = (acceptor.clone(), app.clone());
        tokio::spawn(async move {
            let (_permiso, _de_ip) = (permiso, de_ip);
            match acceptor {
                Some(a) => {
                    let Ok(Ok(tls)) = tokio::time::timeout(ESPERA_TLS, a.accept(tcp)).await else { return };
                    atender_conexion(hyper_util::rt::TokioIo::new(tls), app, ip, proxy).await;
                }
                None => atender_conexion(hyper_util::rt::TokioIo::new(tcp), app, ip, proxy).await,
            }
        });
    }
}

/// La IP de quien pide: la del otro lado de la conexión o, detrás de un proxy en
/// este mismo equipo (`--detras-de-proxy`), la última de `X-Forwarded-For` (la
/// que añadió el proxy; las anteriores las pudo poner cualquiera).
pub fn ip_real(conexion: IpAddr, proxy: bool, xff: Option<&str>) -> IpAddr {
    if !proxy || !conexion.is_loopback() {
        return conexion;
    }
    xff.and_then(|v| v.rsplit(',').next()).and_then(|s| s.trim().parse::<IpAddr>().ok()).unwrap_or(conexion)
}

/// HTTP/1.1 en una conexión ya aceptada, con la IP de quien pide para los límites.
async fn atender_conexion<I>(io: I, app: axum::Router, ip: SocketAddr, proxy: bool)
where
    I: hyper::rt::Read + hyper::rt::Write + Unpin + Send + 'static,
{
    let servicio = hyper::service::service_fn(move |mut req: hyper::Request<hyper::body::Incoming>| {
        let xff = req.headers().get("x-forwarded-for").and_then(|v| v.to_str().ok());
        let real = ip_real(ip.ip(), proxy, xff);
        req.extensions_mut().insert(IpCliente(Some(real)));
        let mut app = app.clone();
        async move { tower::Service::call(&mut app, req.map(axum::body::Body::new)).await }
    });
    let _ = hyper::server::conn::http1::Builder::new()
        .timer(hyper_util::rt::TokioTimer::new())
        .header_read_timeout(ESPERA_CABECERAS)
        .serve_connection(io, servicio)
        .with_upgrades()
        .await;
}

#[cfg(test)]
mod pruebas_red {
    use super::*;

    #[test]
    fn la_ip_de_detras_del_proxy_solo_si_viene_del_proxy() {
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        // Sin --detras-de-proxy, X-Forwarded-For no cuenta.
        assert_eq!(ip_real(ip("127.0.0.1"), false, Some("203.0.113.5")), ip("127.0.0.1"));
        // Con él: la última (la que puso el proxy), no las que pudo inventar el cliente.
        assert_eq!(ip_real(ip("127.0.0.1"), true, Some("10.0.0.1, 203.0.113.5")), ip("203.0.113.5"));
        assert_eq!(ip_real(ip("::1"), true, Some("2001:db8::7")), ip("2001:db8::7"));
        // Una conexión que no viene de este equipo no puede fingir su IP.
        assert_eq!(ip_real(ip("198.51.100.9"), true, Some("203.0.113.5")), ip("198.51.100.9"));
        assert_eq!(ip_real(ip("127.0.0.1"), true, Some("basura")), ip("127.0.0.1"));
        assert_eq!(ip_real(ip("127.0.0.1"), true, None), ip("127.0.0.1"));
    }

    #[test]
    fn tope_de_conexiones_por_ip() {
        let por_ip: Arc<Mutex<HashMap<IpAddr, usize>>> = Default::default();
        let ip: IpAddr = "203.0.113.5".parse().unwrap();
        let a = contar_conexion(&por_ip, ip, 2).unwrap();
        let b = contar_conexion(&por_ip, ip, 2).unwrap();
        assert!(contar_conexion(&por_ip, ip, 2).is_err(), "la tercera no cabe");
        assert!(contar_conexion(&por_ip, "203.0.113.6".parse().unwrap(), 2).is_ok(), "otra IP, sí");
        drop(a);
        assert!(contar_conexion(&por_ip, ip, 2).is_ok(), "al cerrarse una, cabe otra");
        drop(b);
        // Este mismo equipo (el proxy) no tiene tope.
        for _ in 0..5 {
            assert!(contar_conexion(&por_ip, "127.0.0.1".parse().unwrap(), 2).unwrap().is_none());
        }
    }
}
