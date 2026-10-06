//! Estado compartido del servidor.

use crate::almacen::{Almacen, R};
use crate::error::{ErrorApi, Res};
use ed25519_dalek::SigningKey;
use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Notify};

/// Opciones del servidor.
#[derive(Clone, Debug)]
pub struct Opciones {
    /// Bytes máximos de una descarga por el relé.
    pub max_relevo: u64,
    /// Bytes máximos de todos los relés a la vez.
    pub total_relevo: u64,
    /// ¿Sirve por HTTPS (o detrás de un proxy con HTTPS)? Decide la cookie `Secure` y HSTS.
    pub https: bool,
    /// Carpeta con la consola compilada (si no, una página mínima).
    pub consola: Option<PathBuf>,
    /// Instalador genérico del agente (v1.17) para «Descargar instalador listo».
    pub instalador_agente: Option<PathBuf>,
    /// Puerto en el que escucha (v1.19: para vincular el agente de esta misma máquina).
    pub puerto: Option<u16>,
    /// Hay un proxy con HTTPS en este mismo equipo (`--detras-de-proxy`): la IP de
    /// quien pide es la de `X-Forwarded-For` (solo si la conexión viene de 127.0.0.1
    /// o de `proxy_redes`; ver `crate::ip_real`).
    pub proxy: bool,
    /// Redes de los proxies de confianza en otra máquina o contenedor (`--proxy-red`).
    pub proxy_redes: Vec<crate::RedIp>,
    /// v1.34: la dirección que se da a los agentes y a las otras consolas cuando no
    /// es la de la consola (consola en internet: `https://agentes.<dominio>`).
    pub url_agentes: Option<String>,
    /// v1.34: consola en internet (`--dominio` o `--publico`): cuotas
    /// predeterminadas más estrictas (ver `crate::cuotas`).
    pub publico: bool,
    /// Llaves de publicación de **pruebas** (texto de un `.pub` de minisign), además de las
    /// fijadas al compilar. Solo cuentan en una compilación de desarrollo
    /// (`RESGUARDO_LLAVES_PRUEBAS`; ver `crate::publicaciones::llaves`).
    pub llaves_pruebas: Option<String>,
}

impl Default for Opciones {
    fn default() -> Self {
        Self {
            max_relevo: 500 * 1024 * 1024,
            total_relevo: 2 * 1024 * 1024 * 1024,
            https: true,
            consola: None,
            instalador_agente: None,
            puerto: None,
            proxy: false,
            proxy_redes: Vec::new(),
            url_agentes: None,
            publico: false,
            llaves_pruebas: None,
        }
    }
}

pub struct Estado {
    pub db: Arc<dyn Almacen>,
    pub identidad: SigningKey,
    /// Ed25519 pública del servidor (base64).
    pub identidad_pub: String,
    pub huella_ca: String,
    pub datos: PathBuf,
    pub opciones: Opciones,
    /// Agentes con el WebSocket abierto: equipo → cola de mensajes (JSON).
    pub conectados: Mutex<HashMap<String, mpsc::UnboundedSender<String>>>,
    /// Hay mensajes nuevos de alguna sesión interactiva (despierta las esperas largas).
    pub cambios: Notify,
    pub limites: Limites,
    /// Lo que está en marcha en cada equipo (v1.25), solo en memoria.
    pub progreso: crate::progreso::Progresos,
    /// Las consolas con el canal en vivo abierto (v1.39), por cliente.
    pub vivo: crate::vivo::Vivo,
    /// Notificaciones: clave de los secretos, transporte y lo último de cada equipo.
    pub notif: crate::notificaciones::Motor,
    /// Lo que ocupan los relés en disco (`relevos/`), sin recorrer la carpeta en cada trozo.
    pub uso_relevos: UsoRelevos,
}

/// Los bytes de todos los relés (la carpeta `relevos/`), llevados en memoria: se miden
/// al arrancar y en cada limpieza (que corrige cualquier desvío), se reservan antes de
/// escribir cada trozo y se descuentan al borrar un relé. Antes se recorría la carpeta
/// con E/S síncrona en cada trozo, dentro de una tarea async.
#[derive(Debug, Default)]
pub struct UsoRelevos(std::sync::atomic::AtomicU64);

impl UsoRelevos {
    /// Suma `n` si con ellos no se pasa de `tope` (de una vez: dos trozos a la vez no se
    /// cuelan los dos por el mismo hueco). `false` si no caben.
    pub fn reservar(&self, n: u64, tope: u64) -> bool {
        use std::sync::atomic::Ordering::SeqCst;
        self.0.try_update(SeqCst, SeqCst, |u| u.checked_add(n).filter(|t| *t <= tope)).is_ok()
    }
    /// Devuelve `n` (un trozo que no se llegó a escribir, un relé borrado).
    pub fn soltar(&self, n: u64) {
        use std::sync::atomic::Ordering::SeqCst;
        let _ = self.0.try_update(SeqCst, SeqCst, |u| Some(u.saturating_sub(n)));
    }
    /// Lo medido en disco ([`medir_relevos`]).
    pub fn poner(&self, n: u64) {
        self.0.store(n, std::sync::atomic::Ordering::SeqCst);
    }
    pub fn bytes(&self) -> u64 {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Los bytes de los archivos bajo `dir` (E/S síncrona: al arrancar o en `spawn_blocking`).
/// No sigue enlaces.
pub fn medir_relevos(dir: &std::path::Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .map(|e| match e.file_type() {
                    Ok(t) if t.is_dir() => medir_relevos(&e.path()),
                    Ok(t) if t.is_file() => e.metadata().map(|m| m.len()).unwrap_or(0),
                    _ => 0,
                })
                .sum()
        })
        .unwrap_or(0)
}

pub type St = Arc<Estado>;

impl Estado {
    /// Ejecuta una operación del almacén fuera del hilo asíncrono.
    pub async fn db<T: Send + 'static>(self: &Arc<Self>, f: impl FnOnce(&dyn Almacen) -> R<T> + Send + 'static) -> Res<T> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || f(db.as_ref())).await.map_err(ErrorApi::interno)?.map_err(ErrorApi::interno)
    }

    /// Igual, pero devuelve el error del almacén tal cual (para los que se enseñan al usuario).
    pub async fn db_crudo<T: Send + 'static>(self: &Arc<Self>, f: impl FnOnce(&dyn Almacen) -> R<T> + Send + 'static) -> Res<Result<T, String>> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || f(db.as_ref())).await.map_err(ErrorApi::interno)
    }

    /// Envía un mensaje al agente si tiene el WebSocket abierto.
    pub fn al_agente(&self, equipo: &str, mensaje: &serde_json::Value) -> bool {
        let mapa = self.conectados.lock().unwrap_or_else(|e| e.into_inner());
        mapa.get(equipo).is_some_and(|tx| tx.send(mensaje.to_string()).is_ok())
    }

    pub fn conectado(&self, equipo: &str) -> bool {
        self.conectados.lock().unwrap_or_else(|e| e.into_inner()).contains_key(equipo)
    }
}

/// Límite de intentos por clave (correo, IP…) en una ventana de tiempo.
#[derive(Default)]
pub struct Limites {
    mapa: Mutex<HashMap<String, (u32, Instant)>>,
}

impl Limites {
    /// Cuenta un intento; `false` si ya se pasó de `max` en `ventana`.
    /// ¿Ya pasó del máximo en esta ventana? (sin contar un intento más)
    pub fn superado(&self, clave: &str, max: u32, ventana: Duration) -> bool {
        let m = self.mapa.lock().unwrap_or_else(|e| e.into_inner());
        m.get(clave).is_some_and(|(n, t)| Instant::now().duration_since(*t) < ventana && *n > max)
    }

    pub fn intento(&self, clave: &str, max: u32, ventana: Duration) -> bool {
        self.intento_o_espera(clave, max, ventana).is_ok()
    }

    /// Como [`Limites::intento`], pero si ya se pasó dice cuánto falta para que
    /// se abra la ventana siguiente (para `retry_after`).
    pub fn intento_o_espera(&self, clave: &str, max: u32, ventana: Duration) -> Result<(), Duration> {
        let mut m = self.mapa.lock().unwrap_or_else(|e| e.into_inner());
        let ahora = Instant::now();
        if m.len() > 100_000 {
            m.retain(|_, (_, t)| ahora.duration_since(*t) < ventana);
        }
        let e = m.entry(clave.to_string()).or_insert((0, ahora));
        if ahora.duration_since(e.1) >= ventana {
            *e = (0, ahora);
        }
        e.0 = e.0.saturating_add(1);
        if e.0 <= max {
            Ok(())
        } else {
            Err(ventana.saturating_sub(ahora.duration_since(e.1)))
        }
    }

    /// Olvida los intentos (tras un acierto).
    pub fn olvidar(&self, clave: &str) {
        self.mapa.lock().unwrap_or_else(|e| e.into_inner()).remove(clave);
    }
}

/// IP del cliente (la pone el bucle que acepta conexiones).
#[derive(Clone, Copy, Debug)]
pub struct IpCliente(pub Option<IpAddr>);

/// Clave de una IP para los límites de intentos: la IPv4 tal cual (también
/// la que llega como IPv6 «::ffff:a.b.c.d») y de una IPv6 su /64, que es lo
/// que suele tener una sola conexión (cambiar los últimos 64 bits no da más
/// intentos).
pub fn clave_ip(ip: Option<IpAddr>) -> String {
    match ip {
        None => "?".into(),
        Some(IpAddr::V4(v4)) => v4.to_string(),
        Some(IpAddr::V6(v6)) => match v6.to_ipv4_mapped() {
            Some(v4) => v4.to_string(),
            None => {
                let s = v6.segments();
                format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limite_dice_cuanto_esperar() {
        let l = Limites::default();
        let hora = Duration::from_secs(3600);
        for _ in 0..3 {
            assert_eq!(l.intento_o_espera("k", 3, hora), Ok(()));
        }
        let espera = l.intento_o_espera("k", 3, hora).unwrap_err();
        assert!(espera <= hora && espera > Duration::from_secs(3590), "{espera:?}");
        assert!(!l.intento("k", 3, hora));
        // Otra clave no se entera; y al olvidar, se empieza de cero.
        assert!(l.intento("otra", 3, hora));
        l.olvidar("k");
        assert!(l.intento("k", 3, hora));
    }

    #[test]
    fn ipv6_por_prefijo_de_64() {
        let ip = |s: &str| Some(s.parse::<IpAddr>().unwrap());
        assert_eq!(clave_ip(ip("192.168.1.20")), "192.168.1.20");
        assert_eq!(clave_ip(ip("::ffff:192.168.1.20")), "192.168.1.20");
        assert_eq!(clave_ip(ip("2001:db8:1:2:aaaa::1")), "2001:db8:1:2::/64");
        assert_eq!(clave_ip(ip("2001:db8:1:2:bbbb:cccc:dddd:eeee")), clave_ip(ip("2001:db8:1:2::9")));
        assert_ne!(clave_ip(ip("2001:db8:1:3::1")), clave_ip(ip("2001:db8:1:2::1")));
        assert_eq!(clave_ip(None), "?");
    }

    #[test]
    fn cuenta_de_los_reles() {
        let u = UsoRelevos::default();
        assert!(u.reservar(30, 50) && u.reservar(20, 50));
        assert!(!u.reservar(1, 50), "no cabe");
        assert!(!u.reservar(u64::MAX, u64::MAX), "sin desbordar");
        assert_eq!(u.bytes(), 50);
        u.soltar(80);
        assert_eq!(u.bytes(), 0, "nunca por debajo de cero");
        // Muchos a la vez: nunca se pasa del tope.
        let u = std::sync::Arc::new(UsoRelevos::default());
        let hilos: Vec<_> = (0..8)
            .map(|_| {
                let u = u.clone();
                std::thread::spawn(move || (0..1000).filter(|_| u.reservar(7, 10_000)).count())
            })
            .collect();
        let si: usize = hilos.into_iter().map(|h| h.join().unwrap()).sum();
        assert_eq!((si as u64 * 7, u.bytes()), (9_996, 9_996));
    }
}
