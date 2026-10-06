//! Órdenes en espera en el equipo (v1.49, docs/consolas-multiples.md §5).
//!
//! Un servidor nuevo entrega al momento las órdenes con espera (`not_before`) que
//! piden autorización. El equipo las comprueba como siempre (salvo la hora), anota
//! su `seq` y su `nonce`, y las guarda aquí **con el sobre sellado tal cual** (en
//! `servidor.bin`, cifrado como el resto del vínculo): la contraseña o la prueba de
//! la clave nunca quedan en claro. Todas sus consolas las ven en el resumen
//! (`en_espera`), cualquiera puede cancelarlas (`cancelar_espera`) y las demás
//! reciben un aviso.
//!
//! Se aplican cuando tocan con **dos relojes**: el del equipo (con la holgura de
//! siempre) y el de la consola que la mandó (su `ahora` más lo que pasa en el reloj
//! monotónico del equipo), y solo dentro del canal con esa consola, justo después
//! de hablar con ella (por donde llegan sus cancelaciones). Al aplicarla se vuelve
//! a abrir el sobre y se comprueba todo otra vez con lo de ese momento (clave de
//! administración, espera mínima, que esa consola sigue siendo la misma).
//!
//! El equipo anota en su historial (el que reciben todas sus consolas) lo que hace
//! con cada orden: tipo `orden` (§5.8).

use crate::servidor_v2::{self as s, Resultado, Vinculo};
use resguardo_protocolo::{claves, orden_v2, ordenes};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// Órdenes en espera que puede tener a la vez cada consola.
pub const MAX_POR_CONSOLA: usize = 20;
/// El «estado» con el que `procesar` dice que la guardó en espera: no se contesta nada
/// al servidor (allí sigue «entregada», que es lo que ya sabe cancelar).
pub const EN_ESPERA: &str = "en_espera";

/// Una orden guardada en espera.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct EnEspera {
    /// El id de la orden en el servidor que la mandó (con él se cancela y se contesta).
    pub id: String,
    /// El vínculo (consola) por el que llegó.
    pub enlace: String,
    /// La identidad de esa consola cuando llegó: si cambia (otra consola en su sitio), no se aplica.
    pub identidad: String,
    /// Cómo se llamaba esa consola en el equipo (nunca su dirección); vacío si no tenía nombre.
    #[serde(default)]
    pub consola: String,
    pub tipo: String,
    pub seq: u64,
    /// El sobre tal cual (sellado para la llave del equipo).
    #[serde(default)]
    pub sellado: String,
    #[serde(default)]
    pub descripcion: Option<String>,
    /// Quién la mandó, según la consola (informativo).
    #[serde(default)]
    pub por: Option<String>,
    pub emitida: String,
    /// `not_before`.
    pub aplica: String,
    pub caduca: String,
}

fn ahora() -> i64 {
    chrono::Utc::now().timestamp()
}

fn ts(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.timestamp())
}

// ---------- Relojes ----------

/// La última hora que dijo cada consola (`ahora` del saludo, de cada latido o de
/// `tomar`) y cuándo, en el reloj monotónico del equipo.
static HORAS: Mutex<Option<HashMap<String, (i64, Instant)>>> = Mutex::new(None);

/// Anota la hora que trae un mensaje de la consola `enlace` (si la trae: un servidor anterior no).
pub fn anotar_hora(enlace: &str, m: &Value) {
    let Some(t) = m.get("ahora").and_then(Value::as_str).and_then(ts) else { return };
    HORAS.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(enlace.to_string(), (t, Instant::now()));
}

/// La hora de la consola `enlace` ahora mismo, según lo último que dijo. `None` si no
/// la ha dicho (un servidor anterior: ese nunca entrega antes de tiempo).
pub fn hora_servidor(enlace: &str) -> Option<i64> {
    let g = HORAS.lock().unwrap_or_else(|e| e.into_inner());
    g.as_ref()?.get(enlace).map(|(t, cuando)| t.saturating_add(i64::try_from(cuando.elapsed().as_secs()).unwrap_or(i64::MAX)))
}

/// Lo que puede quedarse corta la hora estimada de la consola (su `ahora` y lo que pasó
/// desde entonces, los dos en segundos enteros).
const REDONDEO_S: i64 = 2;

/// ¿Toca ya una orden con este `not_before`? Con el reloj del equipo (y la holgura de
/// siempre) **y** con el de su consola, si se sabe: adelantar uno solo no basta.
pub fn toca(not_before: i64, equipo: i64, servidor: Option<i64>) -> bool {
    equipo >= not_before - orden_v2::HOLGURA_S && servidor.is_none_or(|s| s + REDONDEO_S >= not_before)
}

// ---------- Cambios (para mandar el resumen enseguida) ----------

static CAMBIOS: AtomicU64 = AtomicU64::new(0);

/// El valor de `CAMBIOS` con el que se vio la lista vacía: mientras no cambie, no hay
/// ninguna (solo este proceso las añade, en `recibir`, que cuenta el cambio después de
/// guardarla). Así el canal no lee el vínculo del disco cada pocos segundos para nada.
static VACIA_EN: AtomicU64 = AtomicU64::new(u64::MAX);

/// Cuántas veces cambió la lista en este proceso (va en la huella del informe: si
/// cambia, el resumen con `en_espera` sale enseguida hacia todas las consolas).
pub fn cambios() -> u64 {
    CAMBIOS.load(Ordering::SeqCst)
}

fn cambio() {
    CAMBIOS.fetch_add(1, Ordering::SeqCst);
}

// ---------- Recibir ----------

/// Un texto corto y sin control (lo que dice una consola: nombres).
fn corto(t: &str, max: usize) -> String {
    t.chars().filter(|c| !c.is_control()).collect::<String>().trim().chars().take(max).collect()
}

/// Id de una orden en su servidor: corto y sin rarezas (es lo que se enseña y se cancela).
fn id_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// El nombre de un repositorio gestionado (o su id).
fn nombre_repo(v: &Vinculo, id: &str) -> String {
    v.repos_v2.iter().find(|r| r.id == id).map(|r| r.nombre.clone()).unwrap_or_else(|| id.to_string())
}

fn nombre_destino(v: &Vinculo, id: &str) -> String {
    v.destinos.iter().find(|d| d.id == id).map(|d| d.nombre.clone()).unwrap_or_else(|| id.to_string())
}

/// Qué hace la orden, en palabras y sin rutas ni secretos (para enseñarla en todas las consolas).
pub fn descripcion(v: &Vinculo, o: &orden_v2::OrdenV2) -> Option<String> {
    let c = &o.cuerpo;
    let repo = o.autorizacion.clave_repo.as_ref().map(|r| r.repo.clone()).or_else(|| c["repo"].as_str().map(str::to_string)).unwrap_or_default();
    let r = || nombre_repo(v, &repo);
    let almacen = || format!("{}/{}", c["usuario"].as_str().unwrap_or("?"), c["repo"].as_str().unwrap_or("?"));
    let d = match o.tipo.as_str() {
        "quitar_repositorio" => format!("Quitar el repositorio «{}»", r()),
        "dejar_de_copiar" => format!("Dejar de copiar en «{}»", r()),
        "cambiar_retencion" => format!("Cambiar la retención de «{}»", r()),
        "aplicar_retencion" => format!("Aplicar la retención en «{}»", r()),
        "cambiar_copia_externa" => format!("Quitar la copia externa de «{}»", r()),
        "cambiar_derivada" => format!("Cambiar la retención o el destino de una copia derivada de «{}»", r()),
        "quitar_derivada" => format!("Quitar una copia derivada de «{}»", r()),
        "restaurar" => format!("Restaurar «{}» reemplazando los archivos originales", r()),
        "pausar" => "Pausar las copias".to_string(),
        "baja_equipo" => "Dar de baja el equipo".to_string(),
        "desvincular" => "Dejar de copiar y desvincular el equipo".to_string(),
        "config" => "Cambiar las copias (quedan sin ninguna activa)".to_string(),
        "guarda_copias" => "Cambiar o quitar el almacén («Guarda copias»)".to_string(),
        "cambiar_espera" => format!("Acortar la espera a {} h", c["horas"].as_i64().unwrap_or(0)),
        "quitar_nube" => format!("Desconectar la nube «{}»", corto(c["nombre"].as_str().unwrap_or(""), 60)),
        "retencion_almacen" => format!("Cambiar la retención del almacén en {}", almacen()),
        "aplicar_retencion_almacen" => format!("Aplicar la retención del almacén en {}", almacen()),
        // v1.56: los datos del equipo que comparten sus consolas y olvidar un destino sin uso.
        "nombre_equipo" | "etiquetas_equipo" | "observacion_equipo" => crate::datos_equipo::descripcion(&o.tipo, c)?,
        "quitar_destino" => format!("Quitar el destino «{}»", nombre_destino(v, c["destino"].as_str().unwrap_or(""))),
        _ => return None,
    };
    Some(corto(&crate::web::public_message(&d), 160))
}

/// La ficha de una orden de la consola activa de `v` (para guardarla o anotarla).
fn ficha(v: &Vinculo, o: &orden_v2::OrdenV2, orden_id: &str, sellado: &str) -> EnEspera {
    EnEspera {
        id: orden_id.to_string(),
        enlace: v.id_enlace(),
        identidad: v.identidad.clone(),
        consola: corto(&v.nombre_consola, 60),
        tipo: o.tipo.clone(),
        seq: o.seq,
        sellado: sellado.to_string(),
        descripcion: descripcion(v, o),
        por: o.por.as_deref().map(|p| corto(p, 60)).filter(|p| !p.is_empty()),
        emitida: o.emitida.clone(),
        aplica: o.not_before.clone().unwrap_or_default(),
        caduca: o.caduca.clone(),
    }
}

/// La orden `o` (ya abierta, con su `seq` y su `nonce` anotados) aún no toca: si pide
/// autorización y la trae buena, se guarda en espera. Devuelve el resultado (con
/// `estado: EN_ESPERA` si se guardó: no se contesta nada) y el aviso de intentos.
pub fn recibir(v: &mut Vinculo, o: &orden_v2::OrdenV2, tipo: &ordenes::Tipo, orden_id: &str, sellado: &str) -> (Resultado, Option<String>) {
    // Una inofensiva con espera: como siempre (no se guardan: nadie las firma con nada).
    if tipo.nivel == ordenes::Nivel::Inofensiva {
        return (s::rechazada("Todavía no es la hora de esta orden."), None);
    }
    if !id_valido(orden_id) {
        return (s::rechazada("Orden no válida."), None);
    }
    // La espera mínima y la autorización, ya (con sus intentos fallidos y bloqueos).
    if let Err(r) = s::comprobar(v, o, tipo) {
        return r;
    }
    let enlace = v.id_enlace();
    if v.en_espera.iter().any(|e| e.id == orden_id) {
        return (s::rechazada("Esa orden ya está en espera."), None);
    }
    if v.en_espera.iter().filter(|e| e.enlace == enlace).count() >= MAX_POR_CONSOLA {
        return (s::rechazada(format!("Demasiadas órdenes en espera desde esta consola (como mucho {MAX_POR_CONSOLA}): cancela alguna.")), None);
    }
    let e = ficha(v, o, orden_id, sellado);
    v.en_espera.push(e.clone());
    let _ = s::guardar(v);
    cambio();
    anotar(&e, EN_ESPERA, None, None);
    avisar_a_las_demas(v, &e);
    let cuando = cuando_texto(&e.aplica);
    crate::agent::log(&format!("Orden «{}» en espera: se aplicará {cuando} si nadie la cancela.", e.tipo));
    (Resultado { estado: EN_ESPERA, mensaje: format!("En espera: se aplicará {cuando}."), detalle: None }, None)
}

/// «el 07/10 a las 10:30» (hora del equipo).
fn cuando_texto(rfc: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(rfc)
        .map(|d| d.with_timezone(&chrono::Local).format("el %d/%m a las %H:%M").to_string())
        .unwrap_or_else(|_| "a su hora".into())
}

/// Avisa a las demás consolas (la que la mandó ya tiene su «Orden destructiva pendiente»).
fn avisar_a_las_demas(v: &Vinculo, e: &EnEspera) {
    let otras: Vec<Vinculo> = v.vistas().into_iter().filter(|w| w.id_enlace() != e.enlace).collect();
    if otras.is_empty() {
        return;
    }
    let desde = if e.consola.is_empty() { "otra consola".to_string() } else { format!("la consola «{}»", e.consola) };
    let que = e.descripcion.clone().unwrap_or_else(|| format!("«{}»", e.tipo));
    let por = e.por.as_deref().map(|p| format!(" (pedida por {p})")).unwrap_or_default();
    let mensaje = format!(
        "Orden en espera desde {desde}: {que}{por}. Se aplicará {} si nadie la cancela; puedes cancelarla desde cualquier consola.",
        cuando_texto(&e.aplica)
    );
    std::thread::spawn(move || {
        for w in otras {
            // Un servidor anterior no conoce el tipo (422): entonces, como cambio inusual.
            if let Ok((400 | 422, _)) = s::llamar(&w, "POST", "/api/agente/aviso", Some(&json!({ "tipo": "orden_en_espera", "mensaje": mensaje }))) {
                let _ = s::llamar(&w, "POST", "/api/agente/aviso", Some(&json!({ "tipo": "cambio_inusual", "mensaje": mensaje })));
            }
        }
    });
}

// ---------- Historial común ----------

/// Anota en el historial del equipo (el que reciben todas sus consolas) qué pasó con una orden.
fn anotar(e: &EnEspera, resultado: &str, mensaje: Option<&str>, cancelada_desde: Option<&str>) {
    crate::bitacora::anotar(
        "orden",
        json!({
            "orden": e.tipo, "orden_id": e.id, "descripcion": e.descripcion,
            "consola": (!e.consola.is_empty()).then_some(&e.consola), "identidad": e.identidad, "por": e.por,
            "resultado": resultado,
            "mensaje": mensaje.map(crate::web::public_message).filter(|m| !m.is_empty()),
            "aplica": (!e.aplica.is_empty()).then_some(&e.aplica),
            "cancelada_desde": cancelada_desde,
        }),
    );
}

/// Una orden que el equipo acaba de procesar sin espera: al historial común (salvo las
/// que abren una sesión, que van por su cuenta, y `cancelar_espera`, que ya cuenta en
/// la orden cancelada).
pub fn anotar_procesada(v: &Vinculo, o: &orden_v2::OrdenV2, orden_id: &str, r: &Resultado) {
    if o.tipo == "cancelar_espera" || ordenes::tipo(&o.tipo).is_some_and(|t| t.abre_sesion) || r.estado == EN_ESPERA {
        return;
    }
    let mut e = ficha(v, o, orden_id, "");
    if o.not_before.is_none() {
        e.aplica.clear();
    }
    anotar(&e, r.estado, Some(&r.mensaje), None);
}

// ---------- Aplicar ----------

/// Una orden en espera que se aplicó (o se rechazó al aplicarla): lo que hay que contestar a su consola.
pub struct Aplicada {
    pub orden: String,
    pub seq: u64,
    pub tipo: String,
    pub resultado: Resultado,
    pub aviso: Option<String>,
    /// Las credenciales de esa consola de antes de aplicarla (`baja_equipo` las borra).
    pub credenciales: Vinculo,
}

/// La identidad del vínculo `enlace` en `v`, si sigue (con credenciales).
fn identidad_de(v: &Vinculo, enlace: &str) -> Option<String> {
    if v.id_enlace() == enlace {
        return (!v.secreto.is_empty()).then(|| v.identidad.clone());
    }
    v.otras.iter().find(|e| e.id == enlace && !e.secreto.is_empty()).map(|e| e.identidad.clone())
}

/// Fuera las caducadas y las de consolas que ya no están (o que ahora son otra), anotadas.
fn descartar(v: &mut Vinculo, t: i64) -> bool {
    let antes = v.en_espera.len();
    let lista = std::mem::take(&mut v.en_espera);
    for e in lista {
        if ts(&e.caduca).is_none_or(|c| c <= t) {
            anotar(&e, "caducada", Some("Caducó mientras esperaba: no se aplicó."), None);
            crate::agent::log(&format!("Orden «{}» en espera: caducó sin aplicarse.", e.tipo));
        } else if identidad_de(v, &e.enlace).as_deref() != Some(e.identidad.as_str()) {
            anotar(&e, "cancelada", Some("La consola que la mandó ya no gestiona este equipo: no se aplicará."), None);
            crate::agent::log(&format!("Orden «{}» en espera: su consola ya no gestiona el equipo; no se aplica.", e.tipo));
        } else {
            v.en_espera.push(e);
        }
    }
    let cambio_lista = v.en_espera.len() != antes;
    if cambio_lista {
        cambio();
    }
    cambio_lista
}

/// Vuelve a abrir el sobre guardado y comprueba que sigue valiendo.
fn abrir_guardada(v: &Vinculo, e: &EnEspera, t: i64) -> Result<orden_v2::OrdenV2, String> {
    if v.identidad != e.identidad {
        return Err("La consola que la mandó ya no es la misma: no se aplica.".into());
    }
    let plano = claves::open_bytes(&v.box_secret, &e.sellado)?;
    let o: orden_v2::OrdenV2 = serde_json::from_slice(&plano).map_err(|_| "Orden no válida.".to_string())?;
    if o.cliente != v.cliente_id || o.equipo != v.equipo_id || o.tipo != e.tipo || o.seq != e.seq {
        return Err("La orden guardada no cuadra con la que llegó.".into());
    }
    if ts(&o.caduca).is_none_or(|c| c <= t) {
        return Err("Caducó mientras esperaba.".into());
    }
    Ok(o)
}

/// En el canal de la consola `id`, justo después de hablar con ella: aplica las suyas
/// que ya tocan (una a una, cada una fuera de la lista antes de ejecutarse) y descarta
/// las caducadas. Devuelve lo que hay que contestar a esa consola.
pub fn aplicar_las_que_tocan(id: &str) -> Vec<Aplicada> {
    let mut hechas = Vec::new();
    while hechas.len() < MAX_POR_CONSOLA {
        // Lo que se lee antes de mirar el disco: si `recibir` guarda otra después, este número cambia.
        let visto = CAMBIOS.load(Ordering::SeqCst);
        if VACIA_EN.load(Ordering::SeqCst) == visto {
            break;
        }
        let t = ahora();
        if proxima(id).is_some_and(|(g, hasta)| g == visto && t < hasta) {
            break;
        }
        // Primero sin cerrojo y sin escribir nada (el canal pasa por aquí cada pocos segundos).
        let Some(v) = s::cargar() else { break };
        if v.en_espera.is_empty() {
            VACIA_EN.store(visto, Ordering::SeqCst);
            break;
        }
        let servidor = hora_servidor(id);
        let toca_alguna = v.en_espera.iter().any(|e| e.enlace == id && ts(&e.aplica).is_some_and(|nb| toca(nb, t, servidor)));
        let sobra_alguna =
            v.en_espera.iter().any(|e| ts(&e.caduca).is_none_or(|c| c <= t) || identidad_de(&v, &e.enlace).as_deref() != Some(e.identidad.as_str()));
        if !toca_alguna && !sobra_alguna {
            // Hasta cuándo no hay nada que hacer aquí: la primera de esta consola que podría tocar
            // (con el reloj del equipo) o la primera que caduca.
            let hasta = v
                .en_espera
                .iter()
                .filter_map(|e| {
                    let c = ts(&e.caduca)?;
                    let nb = (e.enlace == id).then(|| ts(&e.aplica)).flatten().map(|nb| nb - orden_v2::HOLGURA_S);
                    Some(nb.map_or(c, |n| n.min(c)))
                })
                .min()
                .unwrap_or(i64::MAX);
            poner_proxima(id, visto, hasta);
            break;
        }
        let r = crate::consolas_v2::con_enlace(id, |v| {
            let t = ahora();
            descartar(v, t);
            let servidor = hora_servidor(id);
            let i = v.en_espera.iter().position(|e| e.enlace == id && ts(&e.aplica).is_some_and(|nb| toca(nb, t, servidor)))?;
            let e = v.en_espera.remove(i);
            // Fuera de la lista (y en disco) antes de ejecutarla: se aplica una sola vez.
            let _ = s::guardar(v);
            cambio();
            let credenciales = v.clone();
            let (resultado, aviso) = match abrir_guardada(v, &e, t) {
                Ok(o) => match ordenes::tipo(&o.tipo) {
                    Some(tipo) => s::autorizar_y_ejecutar(v, &o, tipo, &e.id),
                    None => (s::rechazada("Tipo de orden desconocido."), None),
                },
                Err(m) => (s::rechazada(m), None),
            };
            anotar(&e, resultado.estado, Some(&resultado.mensaje), None);
            // Cambió algo en el equipo: las demás consolas reciben la configuración enseguida.
            if resultado.estado == "hecha" && crate::consolas_v2::CAMBIAN_CONFIG.contains(&e.tipo.as_str()) {
                for o in v.otras.iter_mut() {
                    o.config_pendiente = true;
                }
            }
            crate::agent::log(&format!("Orden «{}» en espera, a su hora: {} ({}).", e.tipo, resultado.estado.replace('_', " "), resultado.mensaje));
            Some(Aplicada { orden: e.id, seq: e.seq, tipo: e.tipo, resultado, aviso, credenciales })
        })
        .flatten();
        match r {
            Some(a) => hechas.push(a),
            None => break,
        }
    }
    hechas
}

/// Para cada consola: (valor de `CAMBIOS` con el que se miró, hasta cuándo no hay nada que hacer).
static PROXIMA: Mutex<Option<HashMap<String, (u64, i64)>>> = Mutex::new(None);

fn proxima(id: &str) -> Option<(u64, i64)> {
    PROXIMA.lock().unwrap_or_else(|e| e.into_inner()).as_ref()?.get(id).copied()
}

fn poner_proxima(id: &str, visto: u64, hasta: i64) {
    PROXIMA.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(id.to_string(), (visto, hasta));
}

// ---------- Cancelar ----------

/// El nombre con el que se dice en otra consola desde cuál se canceló (nunca su dirección).
fn nombre_para_otras(v: &Vinculo) -> String {
    let n = corto(&v.nombre_consola, 60);
    if n.is_empty() {
        "otra consola".into()
    } else {
        n
    }
}

/// `cancelar_espera { id }` desde la consola activa de `v` (cualquiera): la quita, la
/// anota y le dice a la consola que la mandó (resultado firmado, cuando responda) que
/// no se aplicará.
pub fn cancelar(v: &mut Vinculo, c: &Value, por: Option<&str>) -> Result<String, String> {
    let id = c["id"].as_str().unwrap_or("").trim();
    if !id_valido(id) {
        return Err("Falta qué orden cancelar (id).".into());
    }
    let i = v.en_espera.iter().position(|e| e.id == id).ok_or("Esa orden ya no está esperando: se aplicó, se canceló o caducó.")?;
    let e = v.en_espera.remove(i);
    let _ = s::guardar(v);
    cambio();
    let desde = nombre_para_otras(v);
    let por = por.map(|p| corto(p, 60)).filter(|p| !p.is_empty());
    let quien = por.as_deref().map(|p| format!(" por {p}")).unwrap_or_default();
    let misma = e.enlace == v.id_enlace();
    let mensaje = if misma {
        format!("Cancelada{quien} en el equipo: no se aplicará.")
    } else {
        format!("Cancelada desde otra consola («{desde}»){quien}: no se aplicará.")
    };
    anotar(&e, "cancelada", Some(&mensaje), Some(&desde));
    crate::agent::log(&format!("Orden «{}» en espera: cancelada desde {desde}.", e.tipo));
    // A la consola que la mandó: rechazada, firmada, con `cancelada` (un servidor nuevo la
    // guarda tal cual: va firmada). Se manda en la próxima vuelta del servicio, y si esa consola
    // no responde, cuando vuelva.
    let r = Resultado { estado: "rechazada", mensaje: mensaje.clone(), detalle: Some(json!({ "cancelada": true, "consola": desde }).to_string()) };
    s::largas::guardar_resultado(&e.enlace, &e.id, e.seq, &e.tipo, &r);
    let que = e.descripcion.clone().unwrap_or_else(|| format!("«{}»", e.tipo));
    Ok(if misma {
        format!("Cancelada: {que}. No se aplicará.")
    } else {
        format!("Cancelada: {que} (la mandó {}). No se aplicará.", if e.consola.is_empty() { "otra consola".to_string() } else { format!("«{}»", e.consola) })
    })
}

/// La consola `id` canceló estas órdenes suyas (`{"t": "cancelada"}` o `canceladas` de
/// `tomar`). Solo valen para las de esa consola.
pub fn canceladas_por_su_consola(id: &str, ordenes: &[String]) {
    if ordenes.is_empty() || s::cargar().is_none_or(|v| !v.en_espera.iter().any(|e| e.enlace == id && ordenes.contains(&e.id))) {
        return;
    }
    crate::consolas_v2::con_enlace(id, |v| {
        let desde = nombre_para_otras(v);
        let lista = std::mem::take(&mut v.en_espera);
        for e in lista {
            if e.enlace == id && ordenes.contains(&e.id) {
                anotar(&e, "cancelada", Some("Cancelada desde la consola que la mandó: no se aplicará."), Some(&desde));
                crate::agent::log(&format!("Orden «{}» en espera: cancelada desde su consola.", e.tipo));
            } else {
                v.en_espera.push(e);
            }
        }
        cambio();
    });
}

// ---------- Resumen ----------

/// `resumen.en_espera` para la consola activa de `v`: las que siguen esperando, con
/// el nombre (nunca la dirección) y la identidad de la consola que la mandó.
pub fn resumen(v: &Vinculo) -> Value {
    let t = ahora();
    let nombres: HashMap<String, String> =
        std::iter::once((v.id_enlace(), v.nombre_consola.clone())).chain(v.otras.iter().map(|e| (e.id.clone(), e.nombre.clone()))).collect();
    json!(v
        .en_espera
        .iter()
        .filter(|e| ts(&e.caduca).is_some_and(|c| c > t) && identidad_de(v, &e.enlace).as_deref() == Some(e.identidad.as_str()))
        .map(|e| {
            let nombre = nombres.get(&e.enlace).map(|n| corto(n, 60)).filter(|n| !n.is_empty()).or_else(|| (!e.consola.is_empty()).then(|| e.consola.clone()));
            json!({
                "id": e.id, "tipo": e.tipo, "descripcion": e.descripcion,
                "consola": { "nombre": nombre, "identidad": e.identidad, "esta": e.enlace == v.id_enlace() },
                "por": e.por, "emitida": e.emitida, "aplica": e.aplica, "caduca": e.caduca,
            })
        })
        .collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hacen_falta_los_dos_relojes() {
        let nb = 1_000_000;
        // El del equipo, con la holgura de siempre (5 min); sin hora del servidor (uno anterior), basta.
        assert!(toca(nb, nb, None));
        assert!(toca(nb, nb - orden_v2::HOLGURA_S, None));
        assert!(!toca(nb, nb - orden_v2::HOLGURA_S - 1, None));
        // Con la hora del servidor: los dos.
        assert!(!toca(nb, nb + 86_400, Some(nb - 60)), "adelantar el reloj del equipo no basta");
        assert!(toca(nb, nb, Some(nb - REDONDEO_S)), "solo el redondeo de la hora estimada");
        assert!(!toca(nb, nb - 3600, Some(nb + 86_400)), "un servidor adelantado tampoco");
        assert!(toca(nb, nb, Some(nb)));
    }

    #[test]
    fn hora_del_servidor_con_el_reloj_monotonico() {
        let enlace = "prueba-espera-reloj";
        assert_eq!(hora_servidor(enlace), None);
        anotar_hora(enlace, &json!({ "t": "ping" }));
        assert_eq!(hora_servidor(enlace), None, "sin `ahora` (servidor anterior), nada");
        anotar_hora(enlace, &json!({ "t": "ping", "ahora": "2026-10-06T10:00:00Z" }));
        let h = hora_servidor(enlace).unwrap();
        let base = ts("2026-10-06T10:00:00Z").unwrap();
        assert!((base..base + 5).contains(&h), "{h}");
    }

    #[test]
    fn descripcion_sin_rutas_ni_secretos() {
        let mut v = Vinculo::default();
        v.repos_v2.push(crate::gestion_v2::RepoV2 { id: "r1".into(), nombre: "Documentos".into(), ..Default::default() });
        let o = |tipo: &str, cuerpo: Value, repo: Option<&str>| orden_v2::OrdenV2 {
            v: 2,
            cliente: "c".into(),
            equipo: "e".into(),
            seq: 1,
            nonce: "n".into(),
            emitida: String::new(),
            caduca: String::new(),
            not_before: None,
            tipo: tipo.into(),
            cuerpo,
            autorizacion: orden_v2::Autorizacion {
                clave_repo: repo.map(|r| orden_v2::ClaveRepo { repo: r.into(), contrasena: "secreta-de-prueba".into() }),
                ..Default::default()
            },
            responder_a: None,
            por: None,
        };
        let d = descripcion(&v, &o("quitar_repositorio", Value::Null, Some("r1"))).unwrap();
        assert_eq!(d, "Quitar el repositorio «Documentos»");
        let d = descripcion(&v, &o("restaurar", json!({ "destino": "original", "reemplazar": true, "ruta": "C:\\Datos\\x" }), Some("r1"))).unwrap();
        assert!(!d.contains("secreta") && !d.contains("Datos"), "{d}");
        assert_eq!(descripcion(&v, &o("cambiar_espera", json!({ "horas": 2 }), None)).as_deref(), Some("Acortar la espera a 2 h"));
        assert!(descripcion(&v, &o("copiar_ahora", Value::Null, None)).is_none());
    }

    #[test]
    fn resumen_sin_direccion_y_sin_caducadas() {
        let mut v = Vinculo {
            url: "https://192.168.1.20:8443".into(),
            identidad: "ID-A".into(),
            secreto: "s".into(),
            nombre_consola: "Oficina".into(),
            ..Default::default()
        };
        v.otras.push(crate::consolas_v2::Enlace {
            id: "b".into(),
            url: "https://consola.ejemplo.com".into(),
            identidad: "ID-B".into(),
            secreto: "s".into(),
            ..Default::default()
        });
        let luego = (chrono::Local::now() + chrono::Duration::hours(30)).to_rfc3339();
        let antes = (chrono::Local::now() - chrono::Duration::hours(1)).to_rfc3339();
        let e = |id: &str, enlace: &str, identidad: &str, caduca: &str| EnEspera {
            id: id.into(),
            enlace: enlace.into(),
            identidad: identidad.into(),
            tipo: "pausar".into(),
            seq: 1,
            sellado: "SOBRE".into(),
            aplica: luego.clone(),
            caduca: caduca.into(),
            ..Default::default()
        };
        v.en_espera = vec![
            e("o-1", "principal", "ID-A", &luego),
            e("o-2", "b", "ID-B", &luego),
            e("o-3", "b", "ID-B", &antes),
            e("o-4", "b", "ID-OTRA", &luego),
            e("o-5", "c", "ID-C", &luego),
        ];
        let r = resumen(&v);
        let l = r.as_array().unwrap();
        assert_eq!(l.iter().map(|x| x["id"].as_str().unwrap()).collect::<Vec<_>>(), vec!["o-1", "o-2"], "sin caducadas ni de consolas que ya no están");
        assert_eq!(l[0]["consola"]["nombre"], "Oficina");
        assert_eq!(l[0]["consola"]["esta"], true);
        assert_eq!(l[1]["consola"]["esta"], false);
        assert!(l[1]["consola"]["nombre"].is_null());
        let texto = r.to_string();
        assert!(!texto.contains("192.168") && !texto.contains("ejemplo.com") && !texto.contains("SOBRE"), "ni direcciones ni el sobre: {texto}");
    }
}
