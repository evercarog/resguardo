//! Canal en vivo de la consola (docs/api-servidor.md, §3 «Canal en vivo»):
//! `GET /api/clientes/{c}/vivo` abre un WebSocket por el que el servidor avisa
//! a las consolas abiertas de ese cliente de que algo cambió (un informe, el
//! progreso, una orden, un aviso, el historial, la configuración…) para que
//! vuelvan a pedir **solo** lo que cambió, sin recargar la página ni esperar al
//! siguiente sondeo.
//!
//! - Los mensajes son **pistas de invalidación**: tipo e ids (equipo, orden) y
//!   un estado; nunca nombres, rutas ni datos del cliente. Lo de verdad se pide
//!   por la API de siempre, con sus permisos.
//! - Sesión completa (cookie) y miembro del cliente, como cualquier ruta de
//!   `/api/clientes/{c}` (`guardia_cliente`); además el `Origin` tiene que ser
//!   este servidor (un WebSocket no pasa por CORS: sin esto, otra web abierta
//!   en el mismo navegador podría escuchar). Cada minuto se vuelve a comprobar
//!   la sesión y que sigue siendo miembro; si no, se cierra (4401 / 4403).
//! - Topes: conexiones por cuenta, por cliente y en total; una cola corta por
//!   conexión (quien se queda atrás recibe `resync` y vuelve a pedirlo todo).
//! - Latido: `{"t":"latido"}` y un ping cada 25 s (atraviesa proxies que cortan
//!   lo que está callado); sin respuesta en 75 s, se cierra.

use crate::almacen::Rol;
use crate::auth::Usuario;
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

/// Conexiones abiertas a la vez por cuenta (pestañas y dispositivos de una persona).
pub const MAX_POR_CUENTA: usize = 12;
/// Por cliente (todas las personas que lo miran).
pub const MAX_POR_CLIENTE: usize = 100;
/// En todo el servidor.
pub const MAX_TOTAL: usize = 5_000;
/// Mensajes en cola por cliente; quien se queda más atrás recibe `resync`.
pub const COLA: usize = 64;
/// Latido del servidor (texto y ping).
pub const LATIDO: Duration = Duration::from_secs(25);
/// Sin nada del otro lado (pong, texto…) en este tiempo, la conexión se da por muerta.
pub const SIN_RESPUESTA: Duration = Duration::from_secs(75);
/// Cada cuánto se vuelve a comprobar la sesión y el rol.
pub const REVISAR: Duration = Duration::from_secs(60);
/// Plazo para mandar un mensaje (un cliente que no lee no retiene la tarea).
const PLAZO_ENVIO: Duration = Duration::from_secs(10);
/// El mismo aviso a un cliente, como mucho una vez en este tiempo: los que se repiten
/// dentro (un equipo cuyo canal se cae y vuelve en bucle, órdenes entregadas seguidas…)
/// se juntan en uno que sale al final. Cada aviso hace que cada consola abierta vuelva
/// a pedir: sin esto, un equipo que parpadea multiplicaba las peticiones de todas.
pub const JUNTAR: Duration = Duration::from_secs(2);

/// Qué hace lo que está en marcha en un equipo (para `progreso`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paso {
    Empieza,
    Cambia,
    Termina,
}

impl Paso {
    fn texto(self) -> &'static str {
        match self {
            Paso::Empieza => "empieza",
            Paso::Cambia => "cambia",
            Paso::Termina => "termina",
        }
    }
}

/// Lo que cambió (lo que viaja por el canal: solo tipo, ids y estado).
#[derive(Clone, Debug, PartialEq)]
pub enum Cambio<'a> {
    /// Llegó un informe del equipo (cifras, última copia…).
    Informe(&'a str),
    /// Lo que está en marcha en el equipo empezó, cambió o terminó.
    Progreso(&'a str, Paso),
    /// Una orden cambió de estado (`pendiente`, `entregada`, `en_marcha`, `hecha`…).
    Orden { equipo: &'a str, orden: &'a str, estado: &'a str },
    /// Hay avisos nuevos (o se marcaron vistos).
    Avisos(Option<&'a str>),
    /// El equipo subió entradas nuevas a su historial.
    Historial(&'a str),
    /// El equipo subió su configuración (resumen: copias, repositorios…).
    Config(&'a str),
    /// El equipo cambió (conectado, nombre, etiquetas, confirmado, modo…) o es nuevo.
    Equipo(&'a str),
    /// 0.7.26 (bloque 8): cambiaron los datos comunes del cliente (colores de las etiquetas,
    /// catálogo de destinos, plantillas) o hay diferencias nuevas entre consolas.
    DatosComunes,
}

impl Cambio<'_> {
    pub fn json(&self) -> Value {
        match self {
            Cambio::Informe(e) => json!({ "t": "informe", "equipo": e }),
            Cambio::Progreso(e, p) => json!({ "t": "progreso", "equipo": e, "estado": p.texto() }),
            Cambio::Orden { equipo, orden, estado } => json!({ "t": "orden", "equipo": equipo, "orden": orden, "estado": estado }),
            Cambio::Avisos(e) => json!({ "t": "avisos", "equipo": e }),
            Cambio::Historial(e) => json!({ "t": "historial", "equipo": e }),
            Cambio::Config(e) => json!({ "t": "config", "equipo": e }),
            Cambio::Equipo(e) => json!({ "t": "equipo", "equipo": e }),
            Cambio::DatosComunes => json!({ "t": "datos_comunes" }),
        }
    }
}

#[derive(Default)]
struct Mapa {
    canales: HashMap<String, broadcast::Sender<Arc<str>>>,
    por_cuenta: HashMap<String, usize>,
    por_cliente: HashMap<String, usize>,
    total: usize,
    /// (cliente, aviso) → cuándo salió el último y si hay otro esperando a que pase `JUNTAR`.
    recientes: HashMap<(String, Arc<str>), (tokio::time::Instant, bool)>,
}

/// Las consolas que escuchan, por cliente.
#[derive(Default)]
pub struct Vivo(Arc<Mutex<Mapa>>);

/// Un hueco ocupado (lo suelta al cerrarse la conexión).
pub struct Hueco {
    mapa: Arc<Mutex<Mapa>>,
    cuenta: String,
    cliente: String,
}

impl Drop for Hueco {
    fn drop(&mut self) {
        let mut guarda = self.mapa.lock().unwrap_or_else(|e| e.into_inner());
        let m = &mut *guarda;
        m.total = m.total.saturating_sub(1);
        for (mapa, k) in [(&mut m.por_cuenta, &self.cuenta), (&mut m.por_cliente, &self.cliente)] {
            if let Some(n) = mapa.get_mut(k) {
                *n = n.saturating_sub(1);
                if *n == 0 {
                    mapa.remove(k);
                }
            }
        }
        if m.canales.get(&self.cliente).is_some_and(|t| t.receiver_count() == 0) {
            m.canales.remove(&self.cliente);
        }
    }
}

impl Vivo {
    /// Avisa a las consolas de ese cliente. Sin nadie escuchando no hace nada.
    /// El mismo aviso repetido en menos de `JUNTAR` sale una sola vez más, al final.
    pub fn avisar(&self, cliente: &str, cambio: Cambio<'_>) {
        let mut m = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let Some(tx) = m.canales.get(cliente).cloned() else { return };
        let texto: Arc<str> = Arc::from(cambio.json().to_string());
        let ahora = tokio::time::Instant::now();
        if m.recientes.len() > 10_000 {
            m.recientes.retain(|_, (t, pendiente)| *pendiente || ahora.duration_since(*t) < JUNTAR);
        }
        let clave = (cliente.to_string(), texto.clone());
        if let Some((t, pendiente)) = m.recientes.get_mut(&clave) {
            if ahora.duration_since(*t) < JUNTAR {
                if !*pendiente {
                    // Sin tokio (no debería pasar), sale ya.
                    let Ok(rt) = tokio::runtime::Handle::try_current() else {
                        *t = ahora;
                        let _ = tx.send(texto);
                        return;
                    };
                    *pendiente = true;
                    let (mapa, espera) = (self.0.clone(), JUNTAR - ahora.duration_since(*t));
                    rt.spawn(async move {
                        tokio::time::sleep(espera).await;
                        let mut m = mapa.lock().unwrap_or_else(|e| e.into_inner());
                        let tx = m.canales.get(&clave.0).cloned();
                        if let Some(r) = m.recientes.get_mut(&clave) {
                            *r = (tokio::time::Instant::now(), false);
                        }
                        drop(m);
                        if let Some(tx) = tx {
                            let _ = tx.send(clave.1);
                        }
                    });
                }
                return;
            }
        }
        m.recientes.insert(clave, (ahora, false));
        drop(m);
        // Error = nadie escucha ya: da igual.
        let _ = tx.send(texto);
    }

    /// Ocupa un hueco para una conexión de `cuenta` a `cliente`, si cabe.
    pub fn ocupar(&self, cuenta: &str, cliente: &str) -> Option<Hueco> {
        let mut m = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if m.total >= MAX_TOTAL
            || m.por_cuenta.get(cuenta).copied().unwrap_or(0) >= MAX_POR_CUENTA
            || m.por_cliente.get(cliente).copied().unwrap_or(0) >= MAX_POR_CLIENTE
        {
            return None;
        }
        m.total += 1;
        *m.por_cuenta.entry(cuenta.to_string()).or_insert(0) += 1;
        *m.por_cliente.entry(cliente.to_string()).or_insert(0) += 1;
        Some(Hueco { mapa: self.0.clone(), cuenta: cuenta.to_string(), cliente: cliente.to_string() })
    }

    /// Empieza a escuchar un cliente (con el hueco ya ocupado).
    pub fn escuchar(&self, hueco: &Hueco) -> broadcast::Receiver<Arc<str>> {
        let mut m = self.0.lock().unwrap_or_else(|e| e.into_inner());
        m.canales.entry(hueco.cliente.clone()).or_insert_with(|| broadcast::channel(COLA).0).subscribe()
    }

    /// Conexiones abiertas (para las pruebas y el registro).
    pub fn abiertas(&self) -> usize {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).total
    }
}

/// ¿El WebSocket lo abre una página de este mismo servidor? `Origin` es
/// obligatorio (los navegadores siempre lo mandan en un WebSocket) y tiene que
/// ser `https://<Host>` o `http://<Host>`; si llega `Sec-Fetch-Site`, `same-origin`.
pub fn origen_valido(h: &HeaderMap) -> bool {
    let sitio_ok = h.get("sec-fetch-site").map(|v| v.to_str().ok()).is_none_or(|s| s == Some("same-origin"));
    let origen = h.get(header::ORIGIN).and_then(|v| v.to_str().ok());
    let host = h.get(header::HOST).and_then(|v| v.to_str().ok());
    sitio_ok && matches!((origen, host), (Some(o), Some(host)) if o == format!("https://{host}") || o == format!("http://{host}"))
}

/// `GET /api/clientes/{c}/vivo` (WebSocket): el canal en vivo del cliente, para cualquier rol.
pub async fn canal(State(st): State<St>, u: Usuario, Path(c): Path<String>, cabeceras: HeaderMap, ws: WebSocketUpgrade) -> Res<Response> {
    if !origen_valido(&cabeceras) {
        return Err(ErrorApi::nuevo(StatusCode::FORBIDDEN, "csrf", "El canal en vivo solo se abre desde la consola de este servidor."));
    }
    let (ctx, _) = u.miembro(&st, &c, Rol::Lectura).await?;
    let cuenta = u.0.cuenta.id.clone();
    let hueco = st.vivo.ocupar(&cuenta, ctx.id()).ok_or_else(ErrorApi::demasiados)?;
    let token = u.0.token_hash.clone();
    Ok(ws.max_message_size(4 * 1024).max_frame_size(4 * 1024).on_upgrade(move |socket| atender(st, socket, hueco, token)))
}

/// ¿Sigue valiendo? `None`: sí; `Some(código)`: se cierra con ese código.
pub async fn revisar(st: &St, token: &str, cuenta: &str, cliente: &str) -> Option<u16> {
    let (t, cu, cl) = (token.to_string(), cuenta.to_string(), cliente.to_string());
    match st.db(move |db| Ok((db.sesion(&t)?, db.rol(&cu, &cl)?))).await {
        Ok((Some(s), _)) if s.aal < 2 || s.cuenta_id != cuenta => Some(4401),
        Ok((None, _)) => Some(4401),
        Ok((_, None)) => Some(4403),
        // Un fallo de la base de datos no corta la conexión (se vuelve a mirar en un minuto).
        Ok(_) | Err(_) => None,
    }
}

type Envio = futures_util::stream::SplitSink<WebSocket, Message>;

/// Manda un mensaje con plazo. `false` si no salió (la conexión se da por cerrada).
async fn enviar(m: Message, envio: &mut Envio) -> bool {
    matches!(tokio::time::timeout(PLAZO_ENVIO, envio.send(m)).await, Ok(Ok(())))
}

async fn atender(st: St, socket: WebSocket, hueco: Hueco, token: String) {
    let mut rx = st.vivo.escuchar(&hueco);
    let (mut envio, mut recepcion) = socket.split();
    let hola = json!({ "t": "hola", "v": 1, "latido_s": LATIDO.as_secs() }).to_string();
    if !enviar(Message::Text(hola.into()), &mut envio).await {
        return;
    }
    let mut latido = tokio::time::interval_at(tokio::time::Instant::now() + LATIDO, LATIDO);
    let mut revision = tokio::time::interval_at(tokio::time::Instant::now() + REVISAR, REVISAR);
    let mut visto = Instant::now();
    let mut cierre: Option<u16> = None;
    loop {
        tokio::select! {
            ev = rx.recv() => {
                let texto = match ev {
                    Ok(t) => t.to_string(),
                    Err(broadcast::error::RecvError::Lagged(_)) => json!({ "t": "resync" }).to_string(),
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                if !enviar(Message::Text(texto.into()), &mut envio).await {
                    break;
                }
            }
            entrada = recepcion.next() => match entrada {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                // Pong, o lo que mande la consola (no se procesa: el canal es de ida).
                Some(Ok(_)) => visto = Instant::now(),
            },
            _ = latido.tick() => {
                if visto.elapsed() > SIN_RESPUESTA {
                    break;
                }
                let texto = json!({ "t": "latido" }).to_string();
                if !enviar(Message::Ping(Vec::new().into()), &mut envio).await || !enviar(Message::Text(texto.into()), &mut envio).await {
                    break;
                }
            }
            _ = revision.tick() => {
                if let Some(c) = revisar(&st, &token, &hueco.cuenta, &hueco.cliente).await {
                    cierre = Some(c);
                    break;
                }
            }
        }
    }
    if let Some(code) = cierre {
        let razon = if code == 4401 { "sesion" } else { "sin_acceso" };
        let _ = enviar(Message::Close(Some(CloseFrame { code, reason: razon.into() })), &mut envio).await;
    }
    drop(rx);
    drop(hueco);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topes_por_cuenta_cliente_y_se_sueltan() {
        let v = Vivo::default();
        let mut hs: Vec<Hueco> = (0..MAX_POR_CUENTA).map(|_| v.ocupar("ana", "c1").unwrap()).collect();
        assert!(v.ocupar("ana", "c2").is_none(), "una cuenta no pasa de su tope");
        assert!(v.ocupar("bea", "c1").is_some(), "otra cuenta sí");
        hs.pop();
        assert!(v.ocupar("ana", "c2").is_some(), "al cerrarse una, cabe otra");
        drop(hs);
        assert_eq!(v.abiertas(), 0);
        let hs: Vec<Hueco> = (0..MAX_POR_CLIENTE).map(|i| v.ocupar(&format!("p{i}"), "c1").unwrap()).collect();
        assert!(v.ocupar("otra", "c1").is_none(), "un cliente no pasa de su tope");
        drop(hs);
        assert!(v.0.lock().unwrap().canales.is_empty() && v.0.lock().unwrap().por_cliente.is_empty());
    }

    #[test]
    fn solo_a_su_cliente_y_resync_si_se_queda_atras() {
        let v = Vivo::default();
        let (h1, h2) = (v.ocupar("ana", "c1").unwrap(), v.ocupar("bea", "c2").unwrap());
        let (mut r1, mut r2) = (v.escuchar(&h1), v.escuchar(&h2));
        v.avisar("c1", Cambio::Informe("e1"));
        v.avisar("c3", Cambio::Informe("e9"));
        let m: Value = serde_json::from_str(&r1.try_recv().unwrap()).unwrap();
        assert_eq!(m, json!({ "t": "informe", "equipo": "e1" }));
        assert!(r2.try_recv().is_err(), "nada de otro cliente");
        for i in 0..COLA + 5 {
            v.avisar("c1", Cambio::Progreso(&format!("e{i}"), Paso::Cambia));
        }
        assert!(matches!(r1.try_recv(), Err(broadcast::error::TryRecvError::Lagged(_))));
        drop(r1);
        drop(h1);
        assert!(!v.0.lock().unwrap().canales.contains_key("c1"), "sin nadie, el canal se quita");
    }

    /// Un equipo que parpadea (conecta y se cae en bucle): un aviso enseguida y otro al final.
    #[tokio::test(start_paused = true)]
    async fn avisos_repetidos_se_juntan() {
        let v = Vivo::default();
        let h = v.ocupar("ana", "c1").unwrap();
        let mut r = v.escuchar(&h);
        for _ in 0..500 {
            v.avisar("c1", Cambio::Equipo("e1"));
        }
        v.avisar("c1", Cambio::Equipo("e2"));
        let primero = |r: &mut broadcast::Receiver<Arc<str>>| r.try_recv().ok().map(|m| serde_json::from_str::<Value>(&m).unwrap()["equipo"].clone());
        assert_eq!(primero(&mut r), Some(json!("e1")));
        assert_eq!(primero(&mut r), Some(json!("e2")), "otro equipo no espera");
        assert_eq!(primero(&mut r), None, "los 499 repetidos, juntos y aún no");
        tokio::time::sleep(JUNTAR + Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
        assert_eq!(primero(&mut r), Some(json!("e1")), "uno al final, por si lo último cambió");
        assert_eq!(primero(&mut r), None);
        // Pasado el tiempo, sale enseguida.
        tokio::time::sleep(JUNTAR).await;
        v.avisar("c1", Cambio::Equipo("e1"));
        assert_eq!(primero(&mut r), Some(json!("e1")));
    }

    #[test]
    fn origen() {
        let h = |pares: &[(&str, &str)]| {
            let mut m = HeaderMap::new();
            for (k, v) in pares {
                m.insert(axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(), v.parse().unwrap());
            }
            m
        };
        assert!(origen_valido(&h(&[("host", "consola.ejemplo.com"), ("origin", "https://consola.ejemplo.com")])));
        assert!(origen_valido(&h(&[("host", "127.0.0.1:8443"), ("origin", "https://127.0.0.1:8443"), ("sec-fetch-site", "same-origin")])));
        assert!(!origen_valido(&h(&[("host", "consola.ejemplo.com")])), "sin Origin, no");
        assert!(!origen_valido(&h(&[("host", "consola.ejemplo.com"), ("origin", "https://otra.example")])));
        assert!(!origen_valido(&h(&[("host", "consola.ejemplo.com"), ("origin", "https://consola.ejemplo.com"), ("sec-fetch-site", "cross-site")])));
        assert!(!origen_valido(&h(&[("host", "consola.ejemplo.com"), ("origin", "null")])));
    }

    #[test]
    fn mensajes_sin_datos() {
        let o = Cambio::Orden { equipo: "e1", orden: "o1", estado: "hecha" }.json();
        assert_eq!(o, json!({ "t": "orden", "equipo": "e1", "orden": "o1", "estado": "hecha" }));
        assert_eq!(Cambio::Progreso("e1", Paso::Empieza).json()["estado"], "empieza");
        assert_eq!(Cambio::Avisos(None).json(), json!({ "t": "avisos", "equipo": null }));
    }
}
