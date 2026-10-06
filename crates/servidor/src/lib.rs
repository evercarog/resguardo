//! Resguardo Server: consola web, API y canal de los agentes.
//! Diseño: docs/plataforma.md (§2 y §3). Contrato: docs/api-servidor.md.
//!
//! El servidor no tiene autoridad sobre los equipos: guarda y reenvía órdenes
//! selladas que solo el equipo destinatario puede abrir y que solo acepta con
//! la contraseña del repositorio o la clave de administración del cliente.

pub mod acme;
pub mod agentes;
pub mod almacen;
pub mod ancla;
pub mod api;
pub mod auth;
#[cfg(feature = "consola-integrada")]
pub mod consola_integrada;
pub mod cuotas;
pub mod datos_equipo;
pub mod error;
pub mod estado;
pub mod identidad;
pub mod instalador_agente;
pub mod notificaciones;
pub mod pistas;
pub mod progreso;
#[cfg(test)]
mod propiedades;
pub mod publicaciones;
pub mod registro;
pub mod respaldo;
pub mod vivo;

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
        vivo: Default::default(),
        notif: notificaciones::Motor::nuevo(notificaciones::cifrado::Clave::de_identidad(&identidad)),
        uso_relevos: {
            let u = estado::UsoRelevos::default();
            u.poner(estado::medir_relevos(&datos.join("relevos")));
            u
        },
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
            // Lo que estaba en marcha y dejó de contarse: terminó (para las consolas en vivo).
            for (cliente, equipo) in st1.progreso.purgar() {
                st1.vivo.avisar(&cliente, vivo::Cambio::Progreso(&equipo, vivo::Paso::Termina));
            }
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
                    quitar_sin_alta(&st2, &ctx);
                    avisar_sin_contacto(&st2, &ctx, ahora);
                }
                // Lo que ocupan los relés, medido de nuevo (corrige cualquier desvío de la cuenta).
                st2.uso_relevos.poner(estado::medir_relevos(&datos.join("relevos")));
            })
            .await;
        }
    });
}

/// Quita los equipos que se unieron con un código que caducó o se anuló sin
/// comparar el número de comprobación (un intento fallido de «Añadir equipo»):
/// nunca recibieron la clave de administración, no se pierde nada.
fn quitar_sin_alta(st: &St, ctx: &almacen::ClienteCtx) {
    for eq in st.db.equipos_sin_alta(ctx).unwrap_or_default() {
        if st.db.borrar_equipo(ctx, &eq).and_then(|_| st.db.desindexar_equipo(&eq)).is_ok() {
            let _ = st.db.auditar(ctx, "servidor", "quitar_equipo_sin_alta", &eq, "{}");
            st.vivo.avisar(ctx.id(), crate::vivo::Cambio::Equipo(&eq));
        }
    }
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
            st.vivo.avisar(ctx.id(), vivo::Cambio::Avisos(Some(&e.id)));
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

/// Lo que se espera, como mucho, a que quede libre el puerto al arrancar.
pub const ESPERA_PUERTO: Duration = Duration::from_secs(10);

/// Escucha en `direccion`. Si el puerto está ocupado (al reiniciar el servicio, el
/// proceso anterior puede tardar un momento en soltarlo), lo reintenta con esperas
/// crecientes (0,1 s… 1 s) hasta `plazo`; `aviso` dice que espera y cuando lo logra.
pub async fn escuchar(direccion: SocketAddr, plazo: Duration, aviso: &(dyn Fn(&str) + Send + Sync)) -> Result<tokio::net::TcpListener, String> {
    let inicio = std::time::Instant::now();
    let mut espera = Duration::from_millis(100);
    let mut avisado = false;
    loop {
        match tokio::net::TcpListener::bind(direccion).await {
            Ok(l) => {
                if avisado {
                    aviso(&format!("El puerto {} ya está libre: escuchando (tras {:.1} s).", direccion.port(), inicio.elapsed().as_secs_f32()));
                }
                return Ok(l);
            }
            Err(e) if matches!(e.kind(), std::io::ErrorKind::AddrInUse | std::io::ErrorKind::PermissionDenied) && inicio.elapsed() + espera <= plazo => {
                if !avisado {
                    aviso(&format!(
                        "El puerto {} está ocupado ({e}); se reintenta durante {} s por si es el proceso anterior, que aún termina.",
                        direccion.port(),
                        plazo.as_secs()
                    ));
                    avisado = true;
                }
                tokio::time::sleep(espera).await;
                espera = (espera * 2).min(Duration::from_secs(1));
            }
            Err(e) => return Err(format!("No se pudo escuchar en {direccion}: {e}")),
        }
    }
}

static PARADA: std::sync::OnceLock<tokio::sync::Notify> = std::sync::OnceLock::new();

/// Pide que `servir` termine (el servicio de Windows al pararlo): deja de aceptar
/// conexiones y vuelve. Si aún no estaba sirviendo, termina en cuanto empiece.
pub fn pedir_parada() {
    PARADA.get_or_init(tokio::sync::Notify::new).notify_one();
}

/// Sirve hasta que se pare el proceso (o `pedir_parada`). Con TLS o sin él, el mismo
/// bucle: cada conexión con su límite de tiempo para las cabeceras y un tope de conexiones.
pub async fn servir(st: St, direccion: SocketAddr, tls: Tls) -> Result<(), String> {
    let app = api::router(st.clone());
    let proxies = Arc::new(Proxies { activo: st.opciones.proxy, redes: st.opciones.proxy_redes.clone() });
    let datos = st.datos.clone();
    tareas(st);
    let d2 = datos.clone();
    let aviso = move |l: &str| {
        println!("{l}");
        registro::al_archivo(&d2, l);
    };
    let listener = escuchar(direccion, ESPERA_PUERTO, &aviso).await?;
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
            tokio::spawn(bucle(oyente80, None, acme::app_http(retos.clone(), cfg.dominio.clone(), direccion.port()), Default::default()));
            acme::arrancar(cfg, acme::carpeta(&datos), retos, certs.clone());
            Some(tokio_rustls::TlsAcceptor::from(acme::config_rustls(certs)?))
        }
        Tls::Ninguno => None,
    };
    tokio::select! {
        _ = bucle(listener, acceptor, app, proxies) => {}
        _ = PARADA.get_or_init(tokio::sync::Notify::new).notified() => {}
    }
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
/// `exenta`: un proxy de confianza (`--proxy-red`), por el que llega todo: como este equipo.
fn contar_conexion(por_ip: &Arc<Mutex<HashMap<IpAddr, usize>>>, ip: IpAddr, maximo: usize, exenta: bool) -> Result<Option<ConexionIp>, ()> {
    if ip.is_loopback() || exenta {
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
async fn bucle(listener: tokio::net::TcpListener, acceptor: Option<tokio_rustls::TlsAcceptor>, app: axum::Router, proxies: Arc<Proxies>) {
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
        let Ok(de_ip) = contar_conexion(&por_ip, ip.ip(), MAX_CONEXIONES_IP, proxies.de_confianza(ip.ip())) else { continue };
        let (acceptor, app, proxies) = (acceptor.clone(), app.clone(), proxies.clone());
        tokio::spawn(async move {
            let (_permiso, _de_ip) = (permiso, de_ip);
            match acceptor {
                Some(a) => {
                    let Ok(Ok(tls)) = tokio::time::timeout(ESPERA_TLS, a.accept(tcp)).await else { return };
                    atender_conexion(hyper_util::rt::TokioIo::new(tls), app, ip, proxies).await;
                }
                None => atender_conexion(hyper_util::rt::TokioIo::new(tcp), app, ip, proxies).await,
            }
        });
    }
}

/// Una red en notación CIDR (`10.0.0.0/8`, `fd00::/8`; una IP sola vale por `/32` o `/128`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RedIp {
    base: IpAddr,
    prefijo: u8,
}

impl RedIp {
    /// Lee `IP/PREFIJO` (o una IP sola). No admite `/0`: sería fiarse de cualquiera.
    pub fn leer(texto: &str) -> Result<Self, String> {
        let t = texto.trim();
        let mal = || format!("Red no válida: «{t}» (p. ej. 172.18.0.0/16 o fd00::/8).");
        let (ip, prefijo) = match t.split_once('/') {
            Some((ip, p)) => (ip, Some(p.parse::<u8>().map_err(|_| mal())?)),
            None => (t, None),
        };
        let ip = ip.parse::<IpAddr>().map_err(|_| mal())?.to_canonical();
        let maximo = if ip.is_ipv4() { 32 } else { 128 };
        let prefijo = prefijo.unwrap_or(maximo);
        if prefijo > maximo {
            return Err(mal());
        }
        if prefijo == 0 {
            return Err(format!("«{t}» son todas las direcciones: cualquiera podría poner su IP en X-Forwarded-For. Pon solo la red de los proxies."));
        }
        Ok(Self { base: Self::cortar(ip, prefijo), prefijo })
    }

    fn cortar(ip: IpAddr, prefijo: u8) -> IpAddr {
        match ip {
            IpAddr::V4(v) => IpAddr::V4((u32::from(v) & (u32::MAX.checked_shl(32 - u32::from(prefijo)).unwrap_or(0))).into()),
            IpAddr::V6(v) => IpAddr::V6((u128::from(v) & (u128::MAX.checked_shl(128 - u32::from(prefijo)).unwrap_or(0))).into()),
        }
    }

    pub fn contiene(&self, ip: IpAddr) -> bool {
        let ip = ip.to_canonical();
        ip.is_ipv4() == self.base.is_ipv4() && Self::cortar(ip, self.prefijo) == self.base
    }
}

impl std::fmt::Display for RedIp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.base, self.prefijo)
    }
}

/// Los proxies de los que se cree `X-Forwarded-For`: con `--detras-de-proxy`, este
/// mismo equipo y las redes de `--proxy-red` (un proxy en otro contenedor o máquina).
#[derive(Clone, Debug, Default)]
pub struct Proxies {
    pub activo: bool,
    pub redes: Vec<RedIp>,
}

impl Proxies {
    pub fn de_confianza(&self, ip: IpAddr) -> bool {
        self.activo && (ip.to_canonical().is_loopback() || self.redes.iter().any(|r| r.contiene(ip)))
    }
}

/// Un salto de `X-Forwarded-For`: la IP, con o sin puerto (algunos proxies lo ponen).
fn ip_de_salto(s: &str) -> Option<IpAddr> {
    let s = s.trim();
    s.parse::<IpAddr>().ok().or_else(|| s.parse::<SocketAddr>().ok().map(|d| d.ip())).map(|ip| ip.to_canonical())
}

/// La IP de quien pide: la del otro lado de la conexión o, si la conexión viene de un
/// proxy de confianza ([`Proxies`]), la de `X-Forwarded-For` empezando por la derecha
/// (lo que añadió cada proxy): la primera que no es de un proxy de confianza. Lo de su
/// izquierda lo pudo poner cualquiera. Si un salto no es una IP, la del último proxy.
pub fn ip_real(conexion: IpAddr, proxies: &Proxies, xff: Option<&str>) -> IpAddr {
    if !proxies.de_confianza(conexion) {
        return conexion;
    }
    let mut ultima = conexion;
    for salto in xff.unwrap_or_default().rsplit(',') {
        let Some(ip) = ip_de_salto(salto) else { return ultima };
        if !proxies.de_confianza(ip) {
            return ip;
        }
        ultima = ip;
    }
    ultima
}

/// Todas las cabeceras `X-Forwarded-For`, juntas y en orden (un proxy puede añadir la
/// suya en otra línea en vez de al final de la que trajo el cliente). Si alguna no es
/// texto, ninguna.
fn xff_de(cabeceras: &hyper::HeaderMap) -> Option<String> {
    let mut partes = Vec::new();
    for v in cabeceras.get_all("x-forwarded-for") {
        partes.push(v.to_str().ok()?);
    }
    (!partes.is_empty()).then(|| partes.join(","))
}

/// HTTP/1.1 en una conexión ya aceptada, con la IP de quien pide para los límites.
async fn atender_conexion<I>(io: I, app: axum::Router, ip: SocketAddr, proxies: Arc<Proxies>)
where
    I: hyper::rt::Read + hyper::rt::Write + Unpin + Send + 'static,
{
    let servicio = hyper::service::service_fn(move |mut req: hyper::Request<hyper::body::Incoming>| {
        let real = ip_real(ip.ip(), &proxies, xff_de(req.headers()).as_deref());
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

    /// Al reiniciar el servicio, el proceso anterior aún tiene el puerto un momento:
    /// el nuevo espera a que lo suelte en vez de fallar (y lo dice).
    #[tokio::test]
    async fn espera_a_que_el_puerto_quede_libre() {
        let ocupado = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let dir = ocupado.local_addr().unwrap();
        let soltar = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(600));
            drop(ocupado);
        });
        let avisos = Arc::new(Mutex::new(Vec::<String>::new()));
        let a2 = avisos.clone();
        let aviso = move |l: &str| a2.lock().unwrap().push(l.to_string());
        let l = escuchar(dir, Duration::from_secs(10), &aviso).await.expect("tenía que poder escuchar en cuanto quedó libre");
        assert_eq!(l.local_addr().unwrap(), dir);
        soltar.join().unwrap();
        let avisos = avisos.lock().unwrap().clone();
        assert_eq!(avisos.len(), 2, "{avisos:?}");
        assert!(avisos[0].contains("ocupado") && avisos[1].contains("libre"), "{avisos:?}");
        // Ocupado de verdad (otro programa): se rinde pasado el plazo, con el motivo.
        let otro = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let inicio = std::time::Instant::now();
        let e = escuchar(otro.local_addr().unwrap(), Duration::from_millis(500), &|_: &str| {}).await.unwrap_err();
        assert!(e.contains("No se pudo escuchar"), "{e}");
        assert!(inicio.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn la_ip_de_detras_del_proxy_solo_si_viene_del_proxy() {
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        let sin = Proxies::default();
        let local = Proxies { activo: true, redes: vec![] };
        // Sin --detras-de-proxy, X-Forwarded-For no cuenta.
        assert_eq!(ip_real(ip("127.0.0.1"), &sin, Some("203.0.113.5")), ip("127.0.0.1"));
        // Con él: la última (la que puso el proxy), no las que pudo inventar el cliente.
        assert_eq!(ip_real(ip("127.0.0.1"), &local, Some("10.0.0.1, 203.0.113.5")), ip("203.0.113.5"));
        assert_eq!(ip_real(ip("::1"), &local, Some("2001:db8::7")), ip("2001:db8::7"));
        // Una conexión que no viene de este equipo no puede fingir su IP.
        assert_eq!(ip_real(ip("198.51.100.9"), &local, Some("203.0.113.5")), ip("198.51.100.9"));
        assert_eq!(ip_real(ip("127.0.0.1"), &local, Some("basura")), ip("127.0.0.1"));
        assert_eq!(ip_real(ip("127.0.0.1"), &local, None), ip("127.0.0.1"));
    }

    /// `--proxy-red` (docs/plan-mejoras.md, 9e): el proxy en otro contenedor o máquina.
    #[test]
    fn proxies_de_confianza_en_otras_redes() {
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        let red = |s: &str| RedIp::leer(s).unwrap();
        let p = Proxies { activo: true, redes: vec![red("172.18.0.0/16"), red("fd00:1::/32"), red("10.9.8.7")] };
        // Sin --proxy-red, una conexión del contenedor del proxy no cuenta (como antes).
        assert_eq!(ip_real(ip("172.18.0.2"), &Proxies { activo: true, redes: vec![] }, Some("203.0.113.5")), ip("172.18.0.2"));
        // Con ella, sí: la IP del cliente, no la del proxy.
        assert_eq!(ip_real(ip("172.18.0.2"), &p, Some("203.0.113.5")), ip("203.0.113.5"));
        assert_eq!(ip_real(ip("::ffff:172.18.0.2"), &p, Some("203.0.113.5")), ip("203.0.113.5"));
        assert_eq!(ip_real(ip("fd00:1::5"), &p, Some("2001:db8::9")), ip("2001:db8::9"));
        // Varios proxies seguidos: la de más a la derecha que no es de confianza.
        assert_eq!(ip_real(ip("172.18.0.2"), &p, Some("198.51.100.1, 203.0.113.5, 10.9.8.7, 172.18.0.3")), ip("203.0.113.5"));
        // Lo que inventa el cliente a la izquierda no cuenta, aunque sea de una red de confianza.
        assert_eq!(ip_real(ip("172.18.0.2"), &p, Some("172.18.0.9, 203.0.113.5")), ip("203.0.113.5"));
        // Con puerto o IPv6 entre corchetes.
        assert_eq!(ip_real(ip("172.18.0.2"), &p, Some("203.0.113.5:4711")), ip("203.0.113.5"));
        assert_eq!(ip_real(ip("172.18.0.2"), &p, Some("[2001:db8::9]:4711")), ip("2001:db8::9"));
        // Todo de confianza: la de más a la izquierda. Basura: la del último proxy.
        assert_eq!(ip_real(ip("172.18.0.2"), &p, Some("10.9.8.7")), ip("10.9.8.7"));
        assert_eq!(ip_real(ip("172.18.0.2"), &p, Some("basura, 10.9.8.7")), ip("10.9.8.7"));
        assert_eq!(ip_real(ip("172.18.0.2"), &p, None), ip("172.18.0.2"));
        // Fuera de las redes, X-Forwarded-For no cuenta.
        assert_eq!(ip_real(ip("172.19.0.2"), &p, Some("203.0.113.5")), ip("172.19.0.2"));
        assert_eq!(ip_real(ip("10.9.8.6"), &p, Some("203.0.113.5")), ip("10.9.8.6"));
        // Inactivo (sin --detras-de-proxy), nada.
        assert_eq!(ip_real(ip("172.18.0.2"), &Proxies { activo: false, ..p.clone() }, Some("203.0.113.5")), ip("172.18.0.2"));
        // Las redes.
        assert_eq!(red("172.18.5.4/16").to_string(), "172.18.0.0/16");
        assert_eq!(red("::ffff:10.0.0.1").to_string(), "10.0.0.1/32");
        assert!(red("0.0.0.0/1").contiene(ip("127.0.0.1")) && !red("0.0.0.0/1").contiene(ip("128.0.0.1")));
        assert!(red("fd00::/8").contiene(ip("fd12::1")) && !red("fd00::/8").contiene(ip("10.0.0.1")));
        for malo in ["", "10.0.0.0/33", "fd00::/129", "10.0.0/8", "0.0.0.0/0", "::/0", "10.0.0.0/x", "nombre.ejemplo.com"] {
            assert!(RedIp::leer(malo).is_err(), "{malo}");
        }
    }

    #[test]
    fn varias_cabeceras_x_forwarded_for_en_orden() {
        let mut h = hyper::HeaderMap::new();
        assert_eq!(xff_de(&h), None);
        h.append("x-forwarded-for", "198.51.100.1".parse().unwrap());
        h.append("x-forwarded-for", "203.0.113.5".parse().unwrap());
        assert_eq!(xff_de(&h).as_deref(), Some("198.51.100.1,203.0.113.5"));
        let local = Proxies { activo: true, redes: vec![] };
        assert_eq!(ip_real("127.0.0.1".parse().unwrap(), &local, xff_de(&h).as_deref()), "203.0.113.5".parse::<IpAddr>().unwrap());
        h.append("x-forwarded-for", hyper::header::HeaderValue::from_bytes(b"\xff").unwrap());
        assert_eq!(xff_de(&h), None);
    }

    #[test]
    fn tope_de_conexiones_por_ip() {
        let por_ip: Arc<Mutex<HashMap<IpAddr, usize>>> = Default::default();
        let ip: IpAddr = "203.0.113.5".parse().unwrap();
        let a = contar_conexion(&por_ip, ip, 2, false).unwrap();
        let b = contar_conexion(&por_ip, ip, 2, false).unwrap();
        assert!(contar_conexion(&por_ip, ip, 2, false).is_err(), "la tercera no cabe");
        assert!(contar_conexion(&por_ip, ip, 2, true).unwrap().is_none(), "un proxy de confianza, sin tope");
        assert!(contar_conexion(&por_ip, "203.0.113.6".parse().unwrap(), 2, false).is_ok(), "otra IP, sí");
        drop(a);
        assert!(contar_conexion(&por_ip, ip, 2, false).is_ok(), "al cerrarse una, cabe otra");
        drop(b);
        // Este mismo equipo (el proxy) no tiene tope.
        for _ in 0..5 {
            assert!(contar_conexion(&por_ip, "127.0.0.1".parse().unwrap(), 2, false).unwrap().is_none());
        }
    }
}
