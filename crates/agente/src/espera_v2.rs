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
    /// 14oo: las claves que pide (la de administración y la del repositorio) ya se comprobaron
    /// al recibirla, con el mismo bloqueo por intentos fallidos; al aplicarla se vuelven a
    /// comprobar (por si cambiaron entretanto).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub claves_comprobadas: bool,
    /// 14oo: de qué repositorio era la contraseña comprobada (su nombre), si pedía una.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repositorio: Option<String>,
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
    // B1: con `nombre` («Contabilidad, de PC-01»), ese; si no, `usuario/repo` como antes.
    let almacen = || match c["nombre"].as_str().map(str::trim).filter(|n| !n.is_empty() && n.chars().count() <= 80) {
        Some(n) => format!("«{n}»"),
        None => format!("{}/{}", c["usuario"].as_str().unwrap_or("?"), c["repo"].as_str().unwrap_or("?")),
    };
    let d = match o.tipo.as_str() {
        "quitar_repositorio" => format!("Quitar el repositorio «{}»", r()),
        "dejar_de_copiar" => format!("Dejar de copiar en «{}»", r()),
        "cambiar_retencion" => format!("Cambiar la retención de «{}»", r()),
        "aplicar_retencion" => format!("Aplicar la retención en «{}»", r()),
        "cambiar_copia_externa" => format!("Quitar la copia derivada de cada día de «{}»", r()),
        "cambiar_derivada" => format!("Cambiar la retención o el destino de una copia derivada de «{}»", r()),
        "quitar_derivada" => format!("Quitar una copia derivada de «{}»", r()),
        "restaurar" => format!("Restaurar «{}» reemplazando los archivos originales", r()),
        "comprobar_contrasena" => format!("Comprobar la contraseña de «{}»", r()),
        "pausar" => "Pausar las copias".to_string(),
        "baja_equipo" => "Dar de baja el equipo".to_string(),
        "desvincular" => "Dejar de copiar y desvincular el equipo".to_string(),
        "config" => "Cambiar las copias (quedan sin ninguna activa)".to_string(),
        // 14c: qué cambia exactamente («Quitar «X»», «Cambiar el destino de «X» a la nube «B2»»).
        "guarda_copias" if c.get("espejo_equipo").is_some() => {
            describir_espejos(&crate::espejo_trabajos::cargar_equipo().trabajos, &c["espejo_equipo"], crate::espejo_trabajos::QUIEN_EQUIPO)
                .map(|d| format!("Espejos de este equipo: {d}"))
                .unwrap_or_else(|| "Cambiar o quitar los espejos de este equipo".to_string())
        }
        "guarda_copias" if c.get("espejo_freno").is_some() => {
            let id = c["espejo_freno"]["trabajo"].as_str().unwrap_or_default();
            let ts = if c["espejo_freno"]["quien"] == crate::espejo_trabajos::QUIEN_EQUIPO {
                crate::espejo_trabajos::cargar_equipo().trabajos
            } else {
                crate::server::load().espejo.map(|e| e.trabajos_efectivos()).unwrap_or_default()
            };
            match ts.iter().find(|t| t.id == id) {
                Some(t) => format!("Confirmar el freno del espejo «{}»: se borrará lo que falta en el original", corto(&t.nombre, 60)),
                None => "Confirmar el freno de un espejo".to_string(),
            }
        }
        "guarda_copias" if c.get("espejo").is_some() => {
            let actuales = crate::server::load().espejo.map(|e| e.trabajos_efectivos()).unwrap_or_default();
            describir_espejos(&actuales, &c["espejo"], crate::espejo_trabajos::QUIEN_ALMACEN)
                .map(|d| format!("Espejos del almacén: {d}"))
                .unwrap_or_else(|| "Cambiar el espejo del almacén".to_string())
        }
        // B1: «Ordenar nombres de carpeta» (no espera; se anota en el historial común).
        "guarda_copias" if c.get("mover").is_some() => {
            format!("Ordenar los nombres de carpeta de «{}» en el almacén", corto(c["mover"]["usuario"].as_str().unwrap_or("?"), 64))
        }
        "guarda_copias" => "Cambiar o quitar el almacén («Guarda copias»)".to_string(),
        "cambiar_espera" => format!("Acortar la espera a {} h", c["horas"].as_i64().unwrap_or(0)),
        "quitar_nube" => format!("Desconectar la nube «{}»", corto(c["nombre"].as_str().unwrap_or(""), 60)),
        "retencion_almacen" => format!("Cambiar la retención del almacén en {}", almacen()),
        "aplicar_retencion_almacen" => format!("Aplicar la retención del almacén en {}", almacen()),
        // v1.56: los datos del equipo que comparten sus consolas y olvidar un destino sin uso.
        "nombre_equipo" | "etiquetas_equipo" | "observacion_equipo" | "tipo_equipo" => crate::datos_equipo::descripcion(&o.tipo, c)?,
        // Plan 0.8.0, F2 (con el motivo de «Conservar»).
        "comentario_version" | "conservar_version" => crate::versiones_v2::descripcion(&o.tipo, v, c)?,
        "nombre_repositorio" => format!("Cambiar el nombre de un repositorio a «{}»", corto(c["nombre"].as_str().unwrap_or("").trim(), 80)),
        "quitar_destino" => format!("Quitar el destino «{}»", nombre_destino(v, c["destino"].as_str().unwrap_or(""))),
        // 0.7.26 (bloque 8): datos comunes del cliente (sin los valores).
        "datos_cliente" | "datos_cliente_admin" => crate::datos_cliente::descripcion(c)?,
        // 0.8.1 (H1, 15mm).
        "recuperar_en_equipo" => match c["activo"].as_bool() {
            Some(true) => "Dejar a cada persona recuperar sus propios archivos de una copia en el equipo".to_string(),
            Some(false) => "No dejar recuperar en el equipo los archivos de una copia".to_string(),
            None => "Que una copia siga lo del cliente para recuperar en el equipo".to_string(),
        },
        _ => return None,
    };
    Some(corto(&crate::web::public_message(&d), 160))
}

/// 14c: qué cambia en los espejos pedidos (`null`: quitarlos todos), en palabras y sin rutas.
fn describir_espejos(actuales: &[crate::espejo_trabajos::Trabajo], pedido: &Value, quien: &str) -> Option<String> {
    use crate::espejo_trabajos as et;
    let nuevos = if pedido.is_null() {
        Vec::new()
    } else if quien == et::QUIEN_EQUIPO {
        et::leer_pedido(pedido, quien).ok()?
    } else {
        crate::espejo::trabajos_del_pedido(pedido).ok()?.unwrap_or_default()
    };
    if nuevos.is_empty() && actuales.len() > 1 {
        return Some(format!("quitar los {} espejos", actuales.len()));
    }
    let zonas: Vec<(String, String)> = crate::server::load().zonas.iter().map(|z| (z.id.clone(), z.nombre.clone())).collect();
    et::describir_cambio(actuales, &nuevos, &zonas).map(|d| {
        let mut c = d.chars();
        c.next().map(|p| p.to_lowercase().collect::<String>() + c.as_str()).unwrap_or_default()
    })
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
        claves_comprobadas: false,
        repositorio: None,
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
    // 14oo: la espera mínima y las claves, ya al recibirla (con sus intentos fallidos y
    // bloqueos): si alguna no vale, se rechaza ahora y no a su hora. Al aplicarla se vuelve a
    // comprobar todo.
    let repo = match s::comprobar(v, o, tipo) {
        Ok(r) => r,
        Err(r) => return r,
    };
    let enlace = v.id_enlace();
    if v.en_espera.iter().any(|e| e.id == orden_id) {
        return (s::rechazada("Esa orden ya está en espera."), None);
    }
    if v.en_espera.iter().filter(|e| e.enlace == enlace).count() >= MAX_POR_CONSOLA {
        return (s::rechazada(format!("Demasiadas órdenes en espera desde esta consola (como mucho {MAX_POR_CONSOLA}): cancela alguna.")), None);
    }
    let mut e = ficha(v, o, orden_id, sellado);
    e.claves_comprobadas = true;
    e.repositorio = repo.map(|r| corto(&nombre_repo(v, &r), 80));
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
    anotar_con(e, resultado, mensaje, cancelada_desde, None);
}

/// Como [`anotar`]; con `aprobada_en_equipo` (14a), la cuenta del sistema que la aprobó en el
/// propio equipo antes de su hora.
fn anotar_con(e: &EnEspera, resultado: &str, mensaje: Option<&str>, cancelada_desde: Option<&str>, aprobada_en_equipo: Option<&str>) {
    crate::bitacora::anotar(
        "orden",
        json!({
            "orden": e.tipo, "orden_id": e.id, "descripcion": e.descripcion,
            "consola": (!e.consola.is_empty()).then_some(&e.consola), "identidad": e.identidad, "por": e.por,
            "resultado": resultado,
            "mensaje": mensaje.map(crate::web::public_message).filter(|m| !m.is_empty()),
            "aplica": (!e.aplica.is_empty()).then_some(&e.aplica),
            "cancelada_desde": cancelada_desde,
            "aprobada_en_equipo": aprobada_en_equipo,
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

// ---------- Aprobar en el propio equipo (14a) ----------

/// ¿Deja el cliente de la consola `enlace` aprobar en el equipo sus órdenes en espera? Por
/// defecto, sí: solo no si esa consola dijo que su cliente lo desactivó.
pub fn aprobable(v: &Vinculo, enlace: &str) -> bool {
    !v.sin_aprobar_en_equipo.iter().any(|e| e == enlace)
}

/// Lo que dice la consola `enlace` de la opción de su cliente «Aprobar en el equipo»
/// (`aprobar_en_equipo` en su `hola`, en `tomar` o en `{"t": "ajustes"}`). Un servidor
/// anterior no lo dice: entonces no cambia nada (y se puede, por defecto).
pub fn anotar_opcion(enlace: &str, m: &Value) {
    let Some(activa) = m.get("aprobar_en_equipo").and_then(Value::as_bool) else { return };
    if s::cargar().is_some_and(|v| aprobable(&v, enlace) == activa) {
        return;
    }
    let cambiado = crate::consolas_v2::modificar(|v| {
        let antes = aprobable(v, enlace);
        let vivos = v.ids_enlaces();
        v.sin_aprobar_en_equipo.retain(|e| e != enlace && vivos.contains(e));
        if !activa {
            v.sin_aprobar_en_equipo.push(enlace.to_string());
        }
        antes != activa
    });
    if cambiado == Some(true) {
        cambio();
        crate::agent::log(&format!("Una consola {} aprobar en el equipo sus órdenes en espera.", if activa { "vuelve a permitir" } else { "ya no permite" }));
    }
}

/// ¿Sigue esperando y su consola sigue siendo la misma? (Las caducadas y las de una consola
/// que ya no está no se enseñan ni se aprueban.)
fn viva(v: &Vinculo, e: &EnEspera, t: i64) -> bool {
    ts(&e.caduca).is_some_and(|c| c > t) && identidad_de(v, &e.enlace).as_deref() == Some(e.identidad.as_str())
}

/// Las órdenes en espera para la ventana y la línea de órdenes del propio equipo (quien está
/// delante), de la que antes se aplica a la que más tarda, numeradas desde 1. Sin el sobre ni
/// direcciones; con el nombre de la consola y si se pueden aprobar aquí.
pub fn lista_local(v: &Vinculo) -> Vec<Value> {
    let t = ahora();
    let nombres: HashMap<String, String> =
        std::iter::once((v.id_enlace(), v.nombre_consola.clone())).chain(v.otras.iter().map(|e| (e.id.clone(), e.nombre.clone()))).collect();
    let mut l: Vec<&EnEspera> = v.en_espera.iter().filter(|e| viva(v, e, t)).collect();
    l.sort_by_key(|e| ts(&e.aplica).unwrap_or(i64::MAX));
    l.iter()
        .enumerate()
        .map(|(i, e)| {
            let consola = nombres
                .get(&e.enlace)
                .map(|n| corto(n, 60))
                .filter(|n| !n.is_empty() && !n.contains("://"))
                .or_else(|| (!e.consola.is_empty()).then(|| e.consola.clone()));
            json!({
                "n": i + 1, "id": e.id, "tipo": e.tipo,
                "descripcion": e.descripcion.clone().unwrap_or_else(|| format!("«{}»", e.tipo)),
                "consola": consola, "por": e.por, "emitida": e.emitida, "aplica": e.aplica, "caduca": e.caduca,
                "repositorio": e.repositorio, "aprobable": aprobable(v, &e.enlace),
            })
        })
        .collect()
}

/// El mensaje del resultado de una orden aprobada en el equipo (va firmado: corto, para que
/// el servidor no lo recorte).
fn mensaje_aprobada(usuario: &str, r: &Resultado) -> String {
    let m = corto(&crate::web::public_message(&r.mensaje), 360);
    if matches!(r.estado, "hecha" | "en_marcha") {
        format!("Aprobada en el equipo por {usuario}: {m}")
    } else {
        format!("Aprobada en el equipo por {usuario}, pero no se pudo aplicar: {m}")
    }
}

/// El detalle con `aprobada_en_equipo` (si el que había es un objeto JSON, o no había).
fn detalle_aprobada(detalle: Option<String>, usuario: &str) -> Option<String> {
    match detalle.as_deref().map(serde_json::from_str::<Value>) {
        None => Some(json!({ "aprobada_en_equipo": usuario }).to_string()),
        Some(Ok(Value::Object(mut m))) => {
            m.insert("aprobada_en_equipo".into(), json!(usuario));
            Some(Value::Object(m).to_string())
        }
        _ => detalle,
    }
}

/// Aprueba en el propio equipo la orden en espera `id`: se aplica ya, sin esperar a su hora.
/// Quien llama ya comprobó la clave de administración (con su bloqueo por intentos) y que es
/// un administrador del equipo (ipc_local); `usuario` es la cuenta del sistema que la aprueba
/// (la da el sistema, no la petición). Lo demás se comprueba como al aplicarla a su hora: el
/// sobre, su consola, la espera mínima declarada y las claves **de este momento**. Queda en el
/// historial de todas las consolas («Aprobada en el equipo por …»), con un aviso en todas, y el
/// resultado firmado va a la consola que la mandó.
pub fn aprobar(id: &str, usuario: &str) -> Result<Value, String> {
    let usuario = {
        let u = corto(usuario, 80);
        if u.is_empty() {
            "una cuenta sin identificar".to_string()
        } else {
            u
        }
    };
    if !id_valido(id) {
        return Err("Falta qué orden aprobar.".into());
    }
    let ya_no = "Esa orden ya no está esperando: se aplicó, se canceló o caducó.";
    let antes = s::cargar().ok_or("Este equipo no está vinculado a ninguna consola.")?;
    let e = antes.en_espera.iter().find(|e| e.id == id && viva(&antes, e, ahora())).cloned().ok_or(ya_no)?;
    if !aprobable(&antes, &e.enlace) {
        return Err("El cliente de la consola que la mandó no deja aprobar órdenes en el equipo: se aplicará a su hora si nadie la cancela.".into());
    }
    let a = crate::consolas_v2::con_enlace(&e.enlace, |v| -> Result<Aplicada, String> {
        let t = ahora();
        descartar(v, t);
        let i = v.en_espera.iter().position(|x| x.id == id).ok_or(ya_no)?;
        if !aprobable(v, &v.en_espera[i].enlace) {
            return Err("El cliente de la consola que la mandó ya no deja aprobar órdenes en el equipo.".into());
        }
        let e = v.en_espera.remove(i);
        // Fuera de la lista (y en disco) antes de ejecutarla: se aplica una sola vez.
        let _ = s::guardar(v);
        cambio();
        let credenciales = v.clone();
        let (mut resultado, aviso) = match abrir_guardada(v, &e, t) {
            Ok(o) => match ordenes::tipo(&o.tipo) {
                Some(tipo) => s::autorizar_y_ejecutar(v, &o, tipo, &e.id),
                None => (s::rechazada("Tipo de orden desconocido."), None),
            },
            Err(m) => (s::rechazada(m), None),
        };
        resultado.mensaje = mensaje_aprobada(&usuario, &resultado);
        resultado.detalle = detalle_aprobada(resultado.detalle.take(), &usuario);
        anotar_con(&e, resultado.estado, Some(&resultado.mensaje), None, Some(&usuario));
        if resultado.estado == "hecha" && crate::consolas_v2::CAMBIAN_CONFIG.contains(&e.tipo.as_str()) {
            for o in v.otras.iter_mut() {
                o.config_pendiente = true;
            }
        }
        crate::agent::log(&format!(
            "Orden «{}» en espera, aprobada en el equipo por {usuario}: {} ({}).",
            e.tipo,
            resultado.estado.replace('_', " "),
            resultado.mensaje
        ));
        avisar_aprobada(&antes, &e, &usuario, &resultado);
        Ok(Aplicada { orden: e.id, seq: e.seq, tipo: e.tipo, resultado, aviso, credenciales })
    })
    .ok_or("La consola que la mandó ya no gestiona este equipo.")??;
    s::contestar_aplicada(&a);
    Ok(json!({ "estado": a.resultado.estado, "mensaje": a.resultado.mensaje }))
}

/// «Se aprobó en el equipo una orden que reducía la protección», a **todas** las consolas
/// (también a la que la mandó). Un servidor anterior no conoce el tipo: como cambio inusual.
fn avisar_aprobada(v: &Vinculo, e: &EnEspera, usuario: &str, r: &Resultado) {
    let todas = v.vistas();
    if todas.is_empty() {
        return;
    }
    let desde = if e.consola.is_empty() { "otra consola".to_string() } else { format!("la consola «{}»", e.consola) };
    let que = e.descripcion.clone().unwrap_or_else(|| format!("«{}»", e.tipo));
    let por = e.por.as_deref().map(|p| format!(", pedida por {p}")).unwrap_or_default();
    let como = if matches!(r.estado, "hecha" | "en_marcha") { "se aplicó ya" } else { "no se pudo aplicar" };
    let mensaje = format!(
        "Se aprobó en el equipo una orden que reducía la protección: {que} (desde {desde}{por}). La aprobó {usuario} con la clave de administración y {como}, sin esperar a su hora. Si no lo esperabas, cambia la clave de administración."
    );
    std::thread::spawn(move || {
        for w in todas {
            if let Ok((400 | 422, _)) = s::llamar(&w, "POST", "/api/agente/aviso", Some(&json!({ "tipo": "orden_aprobada_en_equipo", "mensaje": mensaje }))) {
                let _ = s::llamar(&w, "POST", "/api/agente/aviso", Some(&json!({ "tipo": "cambio_inusual", "mensaje": mensaje })));
            }
        }
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
            let mut x = json!({
                "id": e.id, "tipo": e.tipo, "descripcion": e.descripcion,
                "consola": { "nombre": nombre, "identidad": e.identidad, "esta": e.enlace == v.id_enlace() },
                "por": e.por, "emitida": e.emitida, "aplica": e.aplica, "caduca": e.caduca,
            });
            // 14oo (opcionales): las claves ya se comprobaron al recibirla, y de qué repositorio.
            if e.claves_comprobadas {
                x["claves_comprobadas"] = json!(true);
            }
            if let Some(r) = &e.repositorio {
                x["repositorio"] = json!(r);
            }
            // 14a (opcional): se puede aprobar en el propio equipo (lo deja el cliente de su consola).
            x["aprobable"] = json!(aprobable(v, &e.enlace));
            x
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
        // 14a: por defecto, todas se pueden aprobar en el equipo; las de una consola cuyo cliente lo
        // desactivó, no.
        assert_eq!(l[0]["aprobable"], true);
        v.sin_aprobar_en_equipo.push("b".into());
        let r = resumen(&v);
        assert_eq!(r[0]["aprobable"], true);
        assert_eq!(r[1]["aprobable"], false);
        // La lista del propio equipo: numerada, de la que antes se aplica a la que más tarda, sin
        // direcciones (el nombre de la consola «b» es su dirección: no se dice).
        v.en_espera[1].aplica = (chrono::Local::now() + chrono::Duration::hours(2)).to_rfc3339();
        let l = lista_local(&v);
        assert_eq!(l.iter().map(|x| (x["n"].as_u64().unwrap(), x["id"].as_str().unwrap())).collect::<Vec<_>>(), vec![(1, "o-2"), (2, "o-1")]);
        assert_eq!(l[0]["aprobable"], false);
        assert_eq!(l[1]["consola"], "Oficina");
        assert_eq!(l[0]["descripcion"], "«pausar»");
        let texto = json!(l).to_string();
        assert!(!texto.contains("192.168") && !texto.contains("ejemplo.com") && !texto.contains("SOBRE"), "{texto}");
    }

    /// 14a: lo que se contesta a la consola que la mandó, firmado: quién la aprobó, sin pisar
    /// el detalle que ya hubiera.
    #[test]
    fn resultado_de_una_aprobada() {
        let r = Resultado { estado: "hecha", mensaje: "Copias en pausa.".into(), detalle: None };
        assert_eq!(mensaje_aprobada("ana (PC-01)", &r), "Aprobada en el equipo por ana (PC-01): Copias en pausa.");
        let mal = Resultado { estado: "rechazada", mensaje: "La clave de administración no es correcta.".into(), detalle: None };
        assert!(mensaje_aprobada("ana", &mal).contains("pero no se pudo aplicar"));
        assert!(
            mensaje_aprobada("ana", &Resultado { estado: "hecha", mensaje: "x".repeat(2000), detalle: None }).chars().count() < 500,
            "que el servidor no lo recorte"
        );
        assert_eq!(detalle_aprobada(None, "ana").as_deref(), Some(r#"{"aprobada_en_equipo":"ana"}"#));
        let d: Value = serde_json::from_str(&detalle_aprobada(Some(r#"{"espera_min_horas":2}"#.into()), "ana").unwrap()).unwrap();
        assert_eq!((d["espera_min_horas"].as_i64(), d["aprobada_en_equipo"].as_str()), (Some(2), Some("ana")));
        assert_eq!(detalle_aprobada(Some("no es json".into()), "ana").as_deref(), Some("no es json"), "lo que no es un objeto, tal cual");
    }

    /// 14oo: las claves de una orden con espera se comprueban al recibirla (con el mismo
    /// bloqueo por intentos), se rechaza en el momento si no valen y, si valen, la orden en
    /// espera dice que ya se comprobaron y de qué repositorio era la contraseña.
    #[test]
    fn claves_comprobadas_al_recibirla() {
        let _l = crate::restic::tests::real_repo_lock();
        let dir = std::env::temp_dir().join(format!("resguardo-espera-claves-{}", uuid::Uuid::new_v4()));
        std::env::set_var("RESGUARDO_AGENT_DIR", &dir);
        crate::agent::prepare_dir().unwrap();
        let mut v = Vinculo { cliente_id: "c1".into(), equipo_id: "e1".into(), secreto: "s".into(), espera_min_horas: 1, ..Default::default() };
        v.repos_v2.push(crate::gestion_v2::RepoV2 {
            id: "docs".into(),
            nombre: "Ventas".into(),
            contrasena: "la buena de verdad".into(),
            ..Default::default()
        });
        let tipo = ordenes::tipo("quitar_derivada").unwrap();
        let t = chrono::Utc::now();
        let orden = |seq: u64, contrasena: &str| orden_v2::OrdenV2 {
            v: 2,
            cliente: "c1".into(),
            equipo: "e1".into(),
            seq,
            nonce: format!("n{seq}"),
            emitida: t.to_rfc3339(),
            caduca: (t + chrono::Duration::days(3)).to_rfc3339(),
            not_before: Some((t + chrono::Duration::hours(2)).to_rfc3339()),
            tipo: "quitar_derivada".into(),
            cuerpo: json!({ "repo": "docs", "id": "nube" }),
            autorizacion: orden_v2::Autorizacion {
                clave_repo: Some(orden_v2::ClaveRepo { repo: "docs".into(), contrasena: contrasena.into() }),
                ..Default::default()
            },
            responder_a: None,
            por: Some("Ana".into()),
        };
        // La contraseña mal: rechazada ya (no se guarda) y cuenta como intento fallido.
        let (r, _) = recibir(&mut v, &orden(1, "la de la copia externa"), tipo, "orden-1", "SOBRE");
        assert_eq!(r.estado, "rechazada");
        assert!(r.mensaje.contains("contraseña del repositorio"), "{}", r.mensaje);
        assert!(v.en_espera.is_empty());
        assert_eq!(v.fallos.get("repo:docs").map(|f| f.n), Some(1));
        // Bien: en espera, con las claves comprobadas y de qué repositorio era.
        let (r, _) = recibir(&mut v, &orden(2, "la buena de verdad"), tipo, "orden-2", "SOBRE");
        assert_eq!(r.estado, EN_ESPERA, "{}", r.mensaje);
        assert!(v.en_espera[0].claves_comprobadas);
        assert_eq!(v.en_espera[0].repositorio.as_deref(), Some("Ventas"));
        let l = resumen(&v);
        assert_eq!(l[0]["claves_comprobadas"], true, "{l}");
        assert_eq!(l[0]["repositorio"], "Ventas");
        assert!(!l.to_string().contains("la buena de verdad"), "sin la contraseña");
        // Una guardada por un agente anterior (sin el campo) se lee igual y no lo dice.
        let vieja: EnEspera = serde_json::from_value(
            json!({ "id": "x", "enlace": "principal", "identidad": "", "tipo": "pausar", "seq": 1, "emitida": "", "aplica": "", "caduca": "" }),
        )
        .unwrap();
        assert!(!vieja.claves_comprobadas && vieja.repositorio.is_none());
        std::env::remove_var("RESGUARDO_AGENT_DIR");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
