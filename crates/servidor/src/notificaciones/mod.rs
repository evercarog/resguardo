//! Notificaciones: que los problemas lleguen a quien no abre la consola
//! (docs/api-servidor.md, «Notificaciones»).
//!
//! Cómo va:
//! 1. Algo pasa: se crea un aviso ([`aviso`]) o llega un informe o un resumen
//!    de un equipo con copias, verificaciones o el espejo que fallan
//!    ([`problemas`]). Eso solo **anota un evento** en `control.db` (rápido; la
//!    petición del agente o de la consola no espera a nada más).
//! 2. La tarea de fondo ([`arrancar`] → [`pasada`]) lee los eventos y decide
//!    con los **incidentes** (uno por equipo, tipo y copia o repositorio): lo
//!    repetido se cuenta en vez de avisar otra vez, lo que se arregla avisa
//!    «Volvió a funcionar» a quien recibió el aviso ([`reglas`]).
//! 3. Lo que hay que mandar va a la **cola** (`notif_envios`, persistente): un
//!    envío por canal y destinatario, con las horas de silencio de cada uno.
//! 4. La misma tarea entrega lo que toca, con un tope por hora y destinatario
//!    (lo que no cabe espera y sale **agrupado**), reintentos con espera
//!    creciente y un límite de tiempo en cada envío ([`transporte`]).
//!
//! Los mensajes solo llevan metadatos (estado, nombres de equipos y copias,
//! mensajes ya sin rutas: [`contenido::texto_publico`]); nunca contraseñas,
//! claves ni rutas. Los secretos de los canales van cifrados ([`cifrado`]).

pub mod ajustes;
pub mod cifrado;
pub mod contenido;
pub mod problemas;
pub mod reglas;
pub mod resumen;
pub mod transporte;

use crate::almacen::{Almacen, ClienteCtx, Ts, R};
use crate::estado::St;
use ajustes::{Ajustes, Canal, TipoCanal};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use transporte::Transporte;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severidad {
    Informativo = 0,
    Importante = 1,
    Critico = 2,
}

impl Severidad {
    pub fn texto(self) -> &'static str {
        match self {
            Severidad::Critico => "Crítico",
            Severidad::Importante => "Importante",
            Severidad::Informativo => "Informativo",
        }
    }
    pub fn clave(self) -> &'static str {
        match self {
            Severidad::Critico => "critico",
            Severidad::Importante => "importante",
            Severidad::Informativo => "informativo",
        }
    }
    pub fn de(s: &str) -> Option<Self> {
        Some(match s {
            "critico" => Severidad::Critico,
            "importante" => Severidad::Importante,
            "informativo" => Severidad::Informativo,
            _ => return None,
        })
    }
}

/// Horas de silencio («22:00» a «07:00», hora del servidor). Lo que llega dentro espera al
/// final (y sale agrupado); los críticos pasan si `salvo_criticos`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Silencio {
    pub desde: String,
    pub hasta: String,
    #[serde(default = "verdadero")]
    pub salvo_criticos: bool,
}

fn verdadero() -> bool {
    true
}

/// Un problema que se sigue: un equipo, un tipo y (si hace falta) una copia o un repositorio.
#[derive(Clone, Debug, PartialEq)]
pub struct Incidente {
    /// `cliente|equipo|tipo|sujeto`.
    pub clave: String,
    pub cliente: String,
    pub equipo: Option<String>,
    pub tipo: String,
    pub sujeto: String,
    pub severidad: Severidad,
    pub titulo: String,
    pub mensaje: String,
    pub abierto: bool,
    pub primero: Ts,
    pub ultimo: Ts,
    pub veces: i64,
    /// Lo que distingue una vuelta de otra (la hora de la copia que falló).
    pub marca: Option<String>,
    pub notificado: Option<Ts>,
    pub cerrado: Option<Ts>,
}

/// Lo que se apunta al pasar algo (tabla `notif_eventos`); lo procesa la tarea de fondo.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Evento {
    Aviso {
        cliente: String,
        equipo: Option<String>,
        tipo: String,
        mensaje: String,
        hora: Ts,
    },
    /// Lo que dice un informe (o el resumen) de un equipo: qué falla y qué está bien.
    Estado {
        cliente: String,
        equipo: String,
        fuente: problemas::Fuente,
        problemas: Vec<problemas::Problema>,
        sanos: Vec<String>,
        hora: Ts,
    },
}

/// Un aviso (o su recuperación) tal como se cuenta.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Alerta {
    pub tipo: String,
    pub severidad: Severidad,
    pub titulo: String,
    pub texto: String,
    pub cliente_id: String,
    pub cliente: String,
    #[serde(default)]
    pub equipo_id: Option<String>,
    #[serde(default)]
    pub equipo: Option<String>,
    /// Cuántas veces ha pasado (desde `desde`).
    pub veces: i64,
    pub desde: Ts,
    pub hora: Ts,
    /// Página de la consola (se le pone delante la dirección pública).
    pub ruta: String,
}

/// Lo que se manda (se guarda en la cola tal cual y se escribe al enviarlo).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Mensaje {
    Aviso(Alerta),
    Recuperacion(Alerta),
    Resumen(resumen::Resumen),
    Prueba { quien: String },
}

impl Mensaje {
    pub fn tipo(&self) -> &'static str {
        match self {
            Mensaje::Aviso(_) => "aviso",
            Mensaje::Recuperacion(_) => "recuperacion",
            Mensaje::Resumen(_) => "resumen",
            Mensaje::Prueba { .. } => "prueba",
        }
    }
    /// Los resúmenes y las pruebas no cuentan para el tope por hora ni se agrupan.
    pub fn suelto(&self) -> bool {
        matches!(self, Mensaje::Resumen(_) | Mensaje::Prueba { .. })
    }
}

/// Una entrega en la cola (y en el registro).
#[derive(Clone, Debug, PartialEq)]
pub struct Envio {
    pub id: String,
    pub creado: Ts,
    /// Dónde vive el canal: `servidor` o `cliente:<id>`.
    pub ambito: String,
    /// De qué cliente es (para el registro del cliente).
    pub cliente: Option<String>,
    pub canal: String,
    /// La dirección de correo, o vacío (el canal ya dice a dónde).
    pub destino: String,
    pub tipo: String,
    pub severidad: Severidad,
    pub incidente: Option<String>,
    pub titulo: String,
    pub mensaje: Mensaje,
    /// `pendiente`, `enviado`, `fallido` o `descartado`.
    pub estado: String,
    pub intentos: u32,
    pub siguiente: Ts,
    pub error: Option<String>,
    pub enviado: Option<Ts>,
    /// El id del primer envío de la entrega (varios agrupados comparten entrega).
    pub entrega: Option<String>,
    pub nota: Option<String>,
}

/// Lo que guarda el servidor para las notificaciones: la clave de los secretos, el
/// transporte (de verdad o, en las pruebas, uno falso) y lo último que dijo cada equipo.
pub struct Motor {
    pub clave: cifrado::Clave,
    pub despertar: tokio::sync::Notify,
    transporte: RwLock<Arc<dyn Transporte>>,
    /// Equipo → huella de su último estado (para no apuntar lo mismo con cada informe).
    huellas: Mutex<HashMap<String, u64>>,
    /// Una pasada a la vez.
    en_pasada: Mutex<()>,
    /// Clientes con eventos procesados en la última pasada (pueden tener avisos
    /// nuevos): la tarea de fondo se lo dice a sus consolas en vivo.
    tocados: Mutex<std::collections::BTreeSet<String>>,
}

impl Motor {
    pub fn nuevo(clave: cifrado::Clave) -> Self {
        Self {
            clave,
            despertar: tokio::sync::Notify::new(),
            transporte: RwLock::new(Arc::new(transporte::Real)),
            huellas: Mutex::new(HashMap::new()),
            en_pasada: Mutex::new(()),
            tocados: Mutex::new(Default::default()),
        }
    }

    /// Los clientes con eventos procesados desde la última vez (y los olvida).
    pub fn tomar_tocados(&self) -> Vec<String> {
        std::mem::take(&mut *self.tocados.lock().unwrap_or_else(|e| e.into_inner())).into_iter().collect()
    }

    pub fn transporte(&self) -> Arc<dyn Transporte> {
        self.transporte.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Cambia el transporte (las pruebas ponen uno falso).
    pub fn poner_transporte(&self, t: Arc<dyn Transporte>) {
        *self.transporte.write().unwrap_or_else(|e| e.into_inner()) = t;
    }

    /// ¿Cambió lo que dice este equipo desde la última vez? (Y lo recuerda.)
    pub fn cambio(&self, clave: &str, huella: u64) -> bool {
        let mut h = self.huellas.lock().unwrap_or_else(|e| e.into_inner());
        if h.len() > 100_000 {
            h.clear();
        }
        h.insert(clave.to_string(), huella) != Some(huella)
    }
}

// ---------- Lo que llama el resto del servidor ----------

/// Crea un aviso (como antes) y apunta el evento para notificarlo.
pub fn aviso(db: &dyn Almacen, ctx: &ClienteCtx, equipo: Option<&str>, tipo: &str, mensaje: &str) -> R<()> {
    aviso_a(db, ctx, equipo, tipo, mensaje, crate::almacen::ahora())
}

/// Igual, con la hora que se diga.
pub fn aviso_a(db: &dyn Almacen, ctx: &ClienteCtx, equipo: Option<&str>, tipo: &str, mensaje: &str, hora: Ts) -> R<()> {
    db.crear_aviso(ctx, equipo, tipo, mensaje)?;
    apuntar(db, &Evento::Aviso { cliente: ctx.id().into(), equipo: equipo.map(Into::into), tipo: tipo.into(), mensaje: mensaje.into(), hora })
}

pub fn apuntar(db: &dyn Almacen, e: &Evento) -> R<()> {
    db.notif_evento(&serde_json::to_string(e).map_err(|e| e.to_string())?)
}

/// Lo que dice un informe o el resumen de un equipo: el evento, solo si cambió.
pub fn evento_estado(motor: &Motor, ctx: &ClienteCtx, equipo: &str, fuente: problemas::Fuente, datos: &serde_json::Value) -> Option<Evento> {
    let (problemas, sanos) = match fuente {
        problemas::Fuente::Informe => problemas::de_informe(datos),
        problemas::Fuente::Resumen => problemas::de_resumen(datos),
    };
    let huella = problemas::huella(&problemas, &sanos);
    motor.cambio(&format!("{}|{equipo}|{fuente:?}", ctx.id()), huella).then(|| Evento::Estado {
        cliente: ctx.id().into(),
        equipo: equipo.into(),
        fuente,
        problemas,
        sanos,
        hora: crate::almacen::ahora(),
    })
}

/// La tarea de fondo: una pasada cada 10 s (o antes, si alguien la despierta).
pub fn arrancar(st: St) {
    tokio::spawn(async move {
        loop {
            let _ = tokio::time::timeout(Duration::from_secs(10), st.notif.despertar.notified()).await;
            let st2 = st.clone();
            let _ = tokio::task::spawn_blocking(move || {
                if let Err(e) = pasada(&st2.db, &st2.notif, crate::almacen::ahora()) {
                    eprintln!("Notificaciones: {e}");
                }
            })
            .await;
            // Lo procesado pudo crear avisos: las consolas en vivo vuelven a contar los suyos.
            for cliente in st.notif.tomar_tocados() {
                st.vivo.avisar(&cliente, crate::vivo::Cambio::Avisos(None));
            }
        }
    });
}

/// Hora local del servidor de un instante.
pub fn local(ts: Ts) -> chrono::NaiveDateTime {
    chrono::DateTime::from_timestamp(ts, 0).map(|d| d.with_timezone(&chrono::Local).naive_local()).unwrap_or_default()
}

/// El instante de una hora local (la primera, si se repite al atrasar el reloj).
pub fn de_local(t: chrono::NaiveDateTime) -> Ts {
    use chrono::TimeZone;
    chrono::Local.from_local_datetime(&t).earliest().map(|d| d.timestamp()).unwrap_or_else(|| t.and_utc().timestamp())
}

/// La zona del servidor ahora (para escribir las horas en los mensajes).
pub fn zona(ts: Ts) -> chrono::FixedOffset {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|d| *d.with_timezone(&chrono::Local).offset())
        .map(|o| chrono::Offset::fix(&o))
        .unwrap_or_else(|| chrono::FixedOffset::east_opt(0).expect("cero"))
}

/// Una pasada entera: eventos, equipos que vuelven, resúmenes y entregas.
pub fn pasada(db: &Arc<dyn Almacen>, motor: &Motor, ahora: Ts) -> R<()> {
    let _una = motor.en_pasada.lock().unwrap_or_else(|e| e.into_inner());
    let db = db.as_ref();
    // Antes que los eventos: una orden destructiva nueva tras cerrarse la anterior vuelve a avisar.
    let resueltos = ordenes_destructivas_terminadas(db, ahora)?;
    let mut tocados = procesar_eventos(db, ahora)?;
    tocados.extend(resueltos);
    motor.tocados.lock().unwrap_or_else(|e| e.into_inner()).extend(tocados);
    reconectados(db, ahora)?;
    resumen::si_toca(db, ahora)?;
    entregar(db, motor, ahora)?;
    if ahora % 3600 < 10 {
        db.notif_limpiar(ahora - 30 * 24 * 3600)?;
    }
    Ok(())
}

/// ¿Le queda al equipo alguna orden destructiva por aplicar? (Sin terminar y sin caducar.)
pub fn destructiva_pendiente(ordenes: &[crate::almacen::Orden], ahora: Ts) -> bool {
    ordenes.iter().any(|o| {
        resguardo_protocolo::ordenes::tipo(&o.tipo).is_some_and(|t| t.destructiva)
            && matches!(o.estado.as_str(), "pendiente" | "entregada" | "en_marcha")
            && o.caduca > ahora
    })
}

/// Quién «vio» un aviso que se cerró solo (lo que enseña la consola en su lista).
pub const AVISO_RESUELTO: &str = "Resguardo (ya no está pendiente)";

/// v1.30: «Orden destructiva pendiente» se cierra en cuanto el equipo ya no tiene
/// ninguna por aplicar (se aplicó, falló, se rechazó, se canceló o caducó). Sin
/// «Volvió a funcionar»: no era un fallo. Lo que aún no había salido se descarta.
fn ordenes_destructivas_terminadas(db: &dyn Almacen, ahora: Ts) -> R<Vec<String>> {
    let mut tocados = Vec::new();
    for inc in db.notif_incidentes_abiertos(None)? {
        if inc.tipo != "orden_destructiva" {
            continue;
        }
        let Some(equipo) = inc.equipo.clone() else { continue };
        let ctx = ClienteCtx::autorizado(&inc.cliente);
        if !destructiva_pendiente(&db.ordenes_equipo(&ctx, &equipo, 200)?, ahora) {
            // Y su aviso en la consola («Pendiente en …: puedes cancelarla antes de que se
            // aplique») deja de estar abierto: ya no hay nada que cancelar. Antes se quedaba
            // así para siempre (prueba de resistencia, docs/estabilidad.md).
            for a in db.avisos(&ctx, true)?.into_iter().filter(|a| a.tipo == "orden_destructiva" && a.equipo.as_deref() == Some(equipo.as_str())) {
                db.marcar_aviso(&ctx, &a.id, AVISO_RESUELTO)?;
                tocados.push(inc.cliente.clone());
            }
            cerrar(db, inc, false, "", "", None, ahora)?;
        }
    }
    Ok(tocados)
}

// ---------- Eventos → incidentes → cola ----------

/// Procesa los eventos apuntados. Devuelve los clientes a los que tocaban.
fn procesar_eventos(db: &dyn Almacen, ahora: Ts) -> R<Vec<String>> {
    let mut tocados = Vec::new();
    for (n, datos) in db.notif_eventos(500)? {
        if let Ok(e) = serde_json::from_str::<Evento>(&datos) {
            if let Err(err) = procesar(db, &e, ahora) {
                eprintln!("Notificaciones: evento {n}: {err}");
            }
            let (Evento::Aviso { cliente, .. } | Evento::Estado { cliente, .. }) = e;
            tocados.push(cliente);
        }
        db.notif_borrar_evento(n)?;
    }
    Ok(tocados)
}

fn nombre_cliente(db: &dyn Almacen, id: &str) -> R<String> {
    Ok(db.cliente(id)?.map(|c| c.nombre).unwrap_or_else(|| "Cliente".into()))
}

fn nombre_equipo(db: &dyn Almacen, ctx: &ClienteCtx, id: Option<&str>) -> R<Option<String>> {
    Ok(match id {
        Some(e) => Some(db.equipo(ctx, e)?.map(|e| e.nombre).unwrap_or_else(|| "Un equipo".into())),
        None => None,
    })
}

/// Lo que se cuenta de un aviso suelto: (título del problema, título al arreglarse).
pub fn titulos_aviso(tipo: &str, equipo: Option<&str>, cliente: &str) -> (String, String) {
    let e = equipo.map(|e| format!("«{e}»")).unwrap_or_else(|| cliente.to_string());
    let t = match tipo {
        "intentos_fallidos" => format!("Intentos fallidos con la clave en {e}"),
        "bloqueo" => format!("{e} bloqueó las órdenes por intentos fallidos"),
        "cambio_inusual" => format!("Cambio inusual en {e}"),
        "equipo_sin_contacto" => format!("{e} no conecta con el servidor"),
        "copia_fallida" => format!("Falló una copia en {e}"),
        "copia_atrasada" => format!("Hay una copia atrasada en {e}"),
        "servicio_detenido" => format!("El servicio de Resguardo está detenido en {e}"),
        "orden_destructiva" => format!("Orden destructiva pendiente en {e}"),
        // v1.4x: la mandó otra consola y el equipo la tiene en espera (consolas-multiples.md §5.6).
        "orden_en_espera" => format!("Orden en espera desde otra consola en {e}"),
        "cambio_clave" => format!("Se cambió la clave de administración de {e}"),
        "auditoria_rehecha" => format!("{e} vio que una consola rehízo su registro de actividad"),
        _ => format!("Aviso de {e}"),
    };
    let ok = match tipo {
        "equipo_sin_contacto" => format!("{e} volvió a conectar"),
        _ => format!("Resuelto: {t}"),
    };
    (t, ok)
}

/// La página de la consola que lo enseña.
fn ruta(cliente: &str, equipo: Option<&str>, tipo: &str) -> String {
    match equipo {
        // v1.4x: la de otra consola se ve (y se cancela) en «Órdenes».
        _ if tipo == "orden_en_espera" => format!("/c/{cliente}/ordenes"),
        _ if tipo == "auditoria_rehecha" => format!("/c/{cliente}/auditoria"),
        Some(e) if !matches!(tipo, "orden_destructiva") => format!("/c/{cliente}/equipos/{e}"),
        _ => format!("/c/{cliente}/avisos"),
    }
}

fn procesar(db: &dyn Almacen, e: &Evento, ahora: Ts) -> R<()> {
    match e {
        Evento::Aviso { cliente, equipo, tipo, mensaje, hora } => {
            let ctx = ClienteCtx::autorizado(cliente);
            let nc = nombre_cliente(db, cliente)?;
            let ne = nombre_equipo(db, &ctx, equipo.as_deref())?;
            let (titulo, _) = titulos_aviso(tipo, ne.as_deref(), &nc);
            let clave = format!("{cliente}|{}|{tipo}|", equipo.as_deref().unwrap_or(""));
            let texto = contenido::texto_publico(mensaje);
            ocurre(
                db,
                Ocurre {
                    clave,
                    cliente,
                    equipo: equipo.as_deref(),
                    tipo,
                    sujeto: "",
                    titulo,
                    texto,
                    marca: None,
                    cuando: Some(*hora),
                    nombre_cliente: &nc,
                    nombre_equipo: ne.as_deref(),
                    crear_aviso: false,
                },
                ahora,
            )
        }
        Evento::Estado { cliente, equipo, fuente, problemas, sanos, hora: _ } => {
            let ctx = ClienteCtx::autorizado(cliente);
            let nc = nombre_cliente(db, cliente)?;
            let ne = nombre_equipo(db, &ctx, Some(equipo))?.unwrap_or_default();
            for p in problemas {
                let (titulo, _) = problemas::titulos(p, &ne);
                let clave = format!("{cliente}|{equipo}|{}|{}", p.tipo, p.sujeto);
                ocurre(
                    db,
                    Ocurre {
                        clave,
                        cliente,
                        equipo: Some(equipo),
                        tipo: &p.tipo,
                        sujeto: &p.sujeto,
                        titulo,
                        texto: contenido::texto_publico(&p.mensaje),
                        marca: Some(&p.marca),
                        cuando: p.cuando,
                        nombre_cliente: &nc,
                        nombre_equipo: Some(&ne),
                        crear_aviso: true,
                    },
                    ahora,
                )?;
            }
            // Lo abierto de esta fuente que ya no falla: arreglado (o ya no existe).
            let tipos = problemas::tipos(*fuente);
            for inc in db.notif_incidentes_abiertos(Some(cliente))? {
                if inc.equipo.as_deref() != Some(equipo.as_str()) || !tipos.contains(&inc.tipo.as_str()) {
                    continue;
                }
                let id = format!("{}|{}", inc.tipo, inc.sujeto);
                if problemas.iter().any(|p| p.tipo == inc.tipo && p.sujeto == inc.sujeto) {
                    continue;
                }
                let sano = sanos.contains(&id);
                let ok = problemas::titulo_ok(&inc.titulo);
                cerrar(db, inc, sano, &ok, &nc, Some(&ne), ahora)?;
            }
            Ok(())
        }
    }
}

struct Ocurre<'a> {
    clave: String,
    cliente: &'a str,
    equipo: Option<&'a str>,
    tipo: &'a str,
    sujeto: &'a str,
    titulo: String,
    texto: String,
    marca: Option<&'a str>,
    cuando: Option<Ts>,
    nombre_cliente: &'a str,
    nombre_equipo: Option<&'a str>,
    /// Los problemas que salen de los informes también entran en la lista de avisos.
    crear_aviso: bool,
}

/// Lo que dicen las etiquetas de un equipo para sus avisos (v1.4x): sus etiquetas (para las
/// preferencias de cada persona), la importancia que piden y los canales que avisan siempre.
#[derive(Clone, Debug, Default)]
pub struct DeEtiquetas {
    pub etiquetas: Vec<String>,
    pub importancia: Vec<Severidad>,
    pub canales: Vec<crate::almacen::CanalRef>,
}

/// Lo de las etiquetas del equipo (vacío si no hay equipo o no tiene).
pub fn de_etiquetas(db: &dyn Almacen, cliente: &str, equipo: Option<&str>) -> R<DeEtiquetas> {
    let Some(eq) = equipo else { return Ok(DeEtiquetas::default()) };
    let ctx = ClienteCtx::autorizado(cliente);
    let etiquetas = db.equipo(&ctx, eq)?.map(|e| e.etiquetas).unwrap_or_default();
    if etiquetas.is_empty() {
        return Ok(DeEtiquetas::default());
    }
    let mut out = DeEtiquetas { etiquetas, ..Default::default() };
    for a in db.ajustes_etiquetas(&ctx)? {
        let Some(av) = a.avisos else { continue };
        if !out.etiquetas.iter().any(|e| e.to_lowercase() == a.nombre.to_lowercase()) {
            continue;
        }
        out.importancia.extend(av.importancia);
        for c in av.canales {
            if !out.canales.contains(&c) {
                out.canales.push(c);
            }
        }
    }
    Ok(out)
}

fn ocurre(db: &dyn Almacen, o: Ocurre<'_>, ahora: Ts) -> R<()> {
    let prev = db.notif_incidente(&o.clave)?;
    let de = de_etiquetas(db, o.cliente, o.equipo)?;
    let sev = reglas::severidad_con_etiquetas(reglas::severidad(o.tipo), &de.importancia);
    let hora = o.cuando.unwrap_or(ahora).min(ahora);
    let paso = reglas::al_ocurrir(prev.as_ref(), ahora, o.marca, o.cuando);
    let (mut inc, avisar) = match paso {
        reglas::Paso::Nada => return Ok(()),
        reglas::Paso::Abrir { avisar } => (
            Incidente {
                clave: o.clave.clone(),
                cliente: o.cliente.into(),
                equipo: o.equipo.map(Into::into),
                tipo: o.tipo.into(),
                sujeto: o.sujeto.into(),
                severidad: sev,
                titulo: o.titulo.clone(),
                mensaje: o.texto.clone(),
                abierto: true,
                primero: hora,
                ultimo: hora,
                veces: 1,
                marca: o.marca.map(Into::into),
                notificado: None,
                cerrado: None,
            },
            avisar,
        ),
        reglas::Paso::Repetir { recordar } => {
            let mut i = prev.clone().expect("repetir: hay incidente");
            i.veces += 1;
            i.ultimo = hora;
            i.mensaje = o.texto.clone();
            i.titulo = o.titulo.clone();
            i.marca = o.marca.map(Into::into);
            (i, recordar)
        }
    };
    if o.crear_aviso && matches!(paso, reglas::Paso::Abrir { .. }) {
        let ctx = ClienteCtx::autorizado(o.cliente);
        db.crear_aviso(&ctx, o.equipo, o.tipo, &format!("{}. {}", o.titulo, o.texto).chars().take(500).collect::<String>())?;
    }
    if avisar {
        let alerta = Alerta {
            tipo: o.tipo.into(),
            severidad: sev,
            titulo: if inc.veces > 1 { format!("Sigue pasando: {}", o.titulo) } else { o.titulo.clone() },
            texto: o.texto.clone(),
            cliente_id: o.cliente.into(),
            cliente: o.nombre_cliente.into(),
            equipo_id: o.equipo.map(Into::into),
            equipo: o.nombre_equipo.map(Into::into),
            veces: inc.veces,
            desde: inc.primero,
            hora,
            ruta: ruta(o.cliente, o.equipo, o.tipo),
        };
        let n = encolar_aviso(db, o.cliente, sev, &de, Some(&inc.clave), &Mensaje::Aviso(alerta), ahora)?;
        if n > 0 {
            inc.notificado = Some(ahora);
        }
    }
    db.notif_guardar_incidente(&inc)
}

/// Cierra un incidente: si estaba avisado y `avisar`, «Volvió a funcionar» a quien recibió el
/// aviso; lo que aún no había salido (en silencio, esperando el tope) ya no sale.
fn cerrar(db: &dyn Almacen, mut inc: Incidente, avisar: bool, titulo_ok: &str, nombre_cliente: &str, nombre_equipo: Option<&str>, ahora: Ts) -> R<()> {
    let recuperar = avisar && reglas::al_recuperar(&inc);
    let mut recibieron: Vec<(String, String, String)> = Vec::new();
    for mut e in db.notif_envios_incidente(&inc.clave)? {
        match e.estado.as_str() {
            "pendiente" => {
                e.estado = "descartado".into();
                e.nota = Some("Se arregló antes de enviarse.".into());
                db.notif_guardar_envio(&e)?;
            }
            "enviado" if e.tipo == "aviso" => {
                let k = (e.ambito.clone(), e.canal.clone(), e.destino.clone());
                if !recibieron.contains(&k) {
                    recibieron.push(k);
                }
            }
            _ => {}
        }
    }
    if recuperar && !recibieron.is_empty() {
        let alerta = Alerta {
            tipo: inc.tipo.clone(),
            severidad: Severidad::Informativo,
            titulo: titulo_ok.into(),
            texto: if inc.veces > 1 {
                format!("Falló {} veces desde el {}; ahora vuelve a ir bien.", inc.veces, contenido::fecha_corta(inc.primero, zona(ahora)))
            } else {
                "Ahora vuelve a ir bien.".into()
            },
            cliente_id: inc.cliente.clone(),
            cliente: nombre_cliente.into(),
            equipo_id: inc.equipo.clone(),
            equipo: nombre_equipo.map(Into::into),
            veces: inc.veces,
            desde: inc.primero,
            hora: ahora,
            ruta: ruta(&inc.cliente, inc.equipo.as_deref(), &inc.tipo),
        };
        let ajustes = ajustes::ajustes(db)?;
        let todos = destinos(db, &ajustes, &inc.cliente, None, &DeEtiquetas::default())?;
        for (ambito, canal, destino) in recibieron {
            // Solo a quien aún puede recibirlo (sigue en el cliente y el canal sigue ahí).
            let Some(d) = todos.iter().find(|d| d.ambito == ambito && d.canal.id == canal && d.destino == destino) else { continue };
            encolar_uno(
                db,
                &ambito,
                Some(&inc.cliente),
                &canal,
                &destino,
                d.silencio.as_ref(),
                Severidad::Informativo,
                Some(&inc.clave),
                &Mensaje::Recuperacion(alerta.clone()),
                ahora,
            )?;
        }
    }
    inc.abierto = false;
    inc.cerrado = Some(ahora);
    db.notif_guardar_incidente(&inc)
}

/// Equipos que llevaban más de un día sin conectar y ya conectan.
fn reconectados(db: &dyn Almacen, ahora: Ts) -> R<()> {
    for inc in db.notif_incidentes_abiertos(None)?.into_iter().filter(|i| i.tipo == "equipo_sin_contacto") {
        let ctx = ClienteCtx::autorizado(&inc.cliente);
        let Some(eq) = inc.equipo.as_deref() else { continue };
        let equipo = db.equipo(&ctx, eq)?;
        let vuelve = equipo.as_ref().is_some_and(|e| e.ultimo_contacto.is_some_and(|t| t > inc.ultimo && ahora - t < 3600));
        if vuelve || equipo.is_none() {
            let nc = nombre_cliente(db, &inc.cliente)?;
            let ne = equipo.map(|e| e.nombre);
            let (_, ok) = titulos_aviso(&inc.tipo, ne.as_deref(), &nc);
            cerrar(db, inc, vuelve, &ok, &nc, ne.as_deref(), ahora)?;
        }
    }
    Ok(())
}

/// A quién le llega algo de un cliente: canal, destinatario y sus horas de silencio.
#[derive(Clone, Debug)]
pub struct Destino {
    pub ambito: String,
    pub canal: Canal,
    pub destino: String,
    pub silencio: Option<Silencio>,
}

pub fn ambito_cliente(cliente: &str) -> String {
    format!("cliente:{cliente}")
}

/// El correo que vale para un cliente: el suyo, si tiene uno encendido y completo; si no, el del servidor.
pub fn correo_de(ajustes: &Ajustes, cliente: &ajustes::AjustesCliente, id_cliente: &str) -> Option<(String, Canal)> {
    let usable = |c: &&Canal| c.tipo == TipoCanal::Correo && c.activo && ajustes::completo(c);
    cliente
        .canales
        .iter()
        .find(usable)
        .map(|c| (ambito_cliente(id_cliente), c.clone()))
        .or_else(|| ajustes.canales.iter().find(usable).map(|c| ("servidor".to_string(), c.clone())))
}

/// Los destinos de un cliente para una gravedad (`None`: todos los posibles, sin mirar la gravedad).
/// `de`: lo de las etiquetas del equipo (v1.4x): las preferencias de cada persona para ellas
/// y los canales que reciben siempre sus avisos.
pub fn destinos(db: &dyn Almacen, ajustes: &Ajustes, cliente: &str, sev: Option<Severidad>, de: &DeEtiquetas) -> R<Vec<Destino>> {
    let propios = ajustes::ajustes_cliente(db, cliente)?;
    let mut out = Vec::new();
    if let Some((ambito, canal)) = correo_de(ajustes, &propios, cliente) {
        for m in db.miembros(cliente)? {
            let (p, _) = ajustes::prefs_cliente(db, cliente, &m.cuenta, m.rol)?;
            if sev.is_none_or(|s| p.inmediatos_para(&de.etiquetas).contains(&s)) {
                let persona = ajustes::prefs_persona(db, &m.cuenta)?;
                out.push(Destino { ambito: ambito.clone(), canal: canal.clone(), destino: m.correo.clone(), silencio: persona.silencio });
            }
        }
    }
    let siempre = |ambito: &str, c: &Canal| de.canales.iter().any(|r| r.ambito == ambito && r.id == c.id);
    let compartido = |ambito: &str, c: &Canal| {
        c.tipo != TipoCanal::Correo && c.activo && ajustes::completo(c) && (sev.is_none_or(|s| c.reglas.severidades.contains(&s)) || siempre(ambito, c))
    };
    for c in ajustes.canales.iter().filter(|c| compartido("servidor", c) && c.reglas.clientes.as_ref().is_none_or(|l| l.iter().any(|x| x == cliente))) {
        out.push(Destino { ambito: "servidor".into(), canal: c.clone(), destino: String::new(), silencio: c.reglas.silencio.clone() });
    }
    for c in propios.canales.iter().filter(|c| compartido("cliente", c)) {
        out.push(Destino { ambito: ambito_cliente(cliente), canal: c.clone(), destino: String::new(), silencio: c.reglas.silencio.clone() });
    }
    Ok(out)
}

/// Pone en la cola un aviso para todos los que lo quieren. Devuelve cuántos envíos.
fn encolar_aviso(db: &dyn Almacen, cliente: &str, sev: Severidad, de: &DeEtiquetas, incidente: Option<&str>, m: &Mensaje, ahora: Ts) -> R<usize> {
    let ajustes = ajustes::ajustes(db)?;
    let ds = destinos(db, &ajustes, cliente, Some(sev), de)?;
    for d in &ds {
        encolar_uno(db, &d.ambito, Some(cliente), &d.canal.id, &d.destino, d.silencio.as_ref(), sev, incidente, m, ahora)?;
    }
    Ok(ds.len())
}

fn titulo_de(m: &Mensaje) -> String {
    match m {
        Mensaje::Aviso(a) | Mensaje::Recuperacion(a) => a.titulo.clone(),
        Mensaje::Resumen(r) => contenido::asunto_resumen(r),
        Mensaje::Prueba { .. } => "Prueba".into(),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn encolar_uno(
    db: &dyn Almacen,
    ambito: &str,
    cliente: Option<&str>,
    canal: &str,
    destino: &str,
    silencio: Option<&Silencio>,
    sev: Severidad,
    incidente: Option<&str>,
    m: &Mensaje,
    ahora: Ts,
) -> R<()> {
    let espera = reglas::retrasar(silencio, sev, local(ahora));
    let siguiente = espera.map(de_local).unwrap_or(ahora).max(ahora);
    db.notif_guardar_envio(&Envio {
        id: uuid::Uuid::new_v4().to_string(),
        creado: ahora,
        ambito: ambito.into(),
        cliente: cliente.map(Into::into),
        canal: canal.into(),
        destino: destino.into(),
        tipo: m.tipo().into(),
        severidad: sev,
        incidente: incidente.map(Into::into),
        titulo: titulo_de(m),
        mensaje: m.clone(),
        estado: "pendiente".into(),
        intentos: 0,
        siguiente,
        error: None,
        enviado: None,
        entrega: None,
        nota: espera.map(|t| format!("En horas de silencio: sale a las {}.", t.format("%H:%M"))),
    })
}

// ---------- Entregar ----------

/// Busca un canal por ámbito e id.
pub fn canal_de(db: &dyn Almacen, ajustes: &Ajustes, ambito: &str, id: &str) -> R<Option<Canal>> {
    Ok(if ambito == "servidor" {
        ajustes.canales.iter().find(|c| c.id == id).cloned()
    } else if let Some(cl) = ambito.strip_prefix("cliente:") {
        ajustes::ajustes_cliente(db, cl)?.canales.into_iter().find(|c| c.id == id)
    } else {
        None
    })
}

/// Lo más que tarda una pasada en entregar (lo demás, en la siguiente).
const TIEMPO_ENTREGA: Duration = Duration::from_secs(60);

fn entregar(db: &dyn Almacen, motor: &Motor, ahora: Ts) -> R<()> {
    let debidos = db.notif_envios_debidos(ahora, 300)?;
    if debidos.is_empty() {
        return Ok(());
    }
    let ajustes = ajustes::ajustes(db)?;
    let mut grupos: BTreeMap<(String, String, String), Vec<Envio>> = BTreeMap::new();
    for e in debidos {
        grupos.entry((e.ambito.clone(), e.canal.clone(), e.destino.clone())).or_default().push(e);
    }
    let empezo = std::time::Instant::now();
    let transporte = motor.transporte();
    for ((ambito, canal_id, destino), envios) in grupos {
        if empezo.elapsed() > TIEMPO_ENTREGA {
            break;
        }
        let canal = canal_de(db, &ajustes, &ambito, &canal_id)?;
        let Some(canal) = canal.filter(|c| c.activo && ajustes::completo(c)) else {
            for mut e in envios {
                e.estado = "descartado".into();
                e.nota = Some("El canal ya no existe, está apagado o le faltan datos.".into());
                db.notif_guardar_envio(&e)?;
            }
            continue;
        };
        let secretos = ajustes::abrir_secretos(&canal, &motor.clave, &ambito);
        let formato = contenido::Formato { url_consola: ajustes.url_consola.clone(), zona: zona(ahora), marcas: BTreeMap::new() };
        let (sueltos, juntos): (Vec<Envio>, Vec<Envio>) = envios.into_iter().partition(|e| e.mensaje.suelto());
        for e in sueltos {
            let ms = std::slice::from_ref(&e.mensaje);
            let formato = con_marca(db, &formato, ms, None);
            let r = transporte::enviar(transporte.as_ref(), &ambito, &canal, &secretos, &destino, ms, &formato, &e.id, ahora);
            anotar(db, vec![e], r, ahora)?;
        }
        if juntos.is_empty() {
            continue;
        }
        let entregas = db.notif_entregas_desde(&canal_id, &destino, ahora - 3600)?;
        if reglas::cupo(entregas.len() as u32, ajustes.max_por_hora) == 0 {
            let libre = entregas.iter().min().copied().unwrap_or(ahora) + 3600 + 1;
            for mut e in juntos {
                e.siguiente = libre.max(ahora + 1);
                e.nota = Some(format!("Tope de {} por hora: sale agrupado cuando haya hueco.", ajustes.max_por_hora));
                db.notif_guardar_envio(&e)?;
            }
            continue;
        }
        let mensajes: Vec<Mensaje> = juntos.iter().map(|e| e.mensaje.clone()).collect();
        let formato = con_marca(db, &formato, &mensajes, None);
        let r = transporte::enviar(transporte.as_ref(), &ambito, &canal, &secretos, &destino, &mensajes, &formato, &juntos[0].id, ahora);
        anotar(db, juntos, r, ahora)?;
    }
    Ok(())
}

/// El formato con la marca del cliente (v1.32) si todo lo que se manda es de
/// uno solo y la tiene (su acento o su logo). `cliente`: el de una prueba de
/// un canal del cliente (el mensaje de prueba no lo lleva). Si no se puede
/// leer, sale sin marca: un aviso nunca se queda sin enviar por eso.
fn con_marca(db: &dyn Almacen, base: &contenido::Formato, ms: &[Mensaje], cliente: Option<&str>) -> contenido::Formato {
    let mut f = base.clone();
    let clientes = contenido::clientes_de(ms);
    let (id, nombre) = match (clientes.as_slice(), cliente) {
        ([(id, nombre)], _) => (id.clone(), nombre.clone()),
        ([], Some(c)) => match db.cliente(c) {
            Ok(Some(x)) => (x.id, x.nombre),
            _ => return f,
        },
        _ => return f,
    };
    let Ok(m) = crate::api::marca::leer(db, &id) else { return f };
    let acento = m.acento.as_deref().and_then(contenido::colores_acento);
    // El logo ya se comprobó al guardarlo (PNG); se vuelve a mirar por si acaso.
    let logo = m
        .logo
        .as_deref()
        .and_then(|b| base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b).ok())
        .filter(|b| crate::api::marca::png_valido(b).is_ok());
    if acento.is_some() || logo.is_some() {
        f.marcas.insert(id, contenido::MarcaCorreo { nombre, acento, logo });
    }
    f
}

/// Anota cómo fue una entrega (de uno o de varios agrupados).
fn anotar(db: &dyn Almacen, envios: Vec<Envio>, r: Result<(), transporte::Fallo>, ahora: Ts) -> R<()> {
    let n = envios.len();
    let entrega = envios.first().map(|e| e.id.clone());
    for mut e in envios {
        match &r {
            Ok(()) => {
                e.estado = "enviado".into();
                e.enviado = Some(ahora);
                e.entrega = entrega.clone();
                e.error = None;
                e.intentos += 1;
                e.nota = (n > 1).then(|| format!("Agrupado con {} más.", n - 1));
            }
            Err(f) => {
                e.intentos += 1;
                e.error = Some(f.texto.clone());
                if f.permanente || e.intentos >= reglas::MAX_INTENTOS {
                    e.estado = "fallido".into();
                    e.nota = Some(if f.permanente { "No se reintenta: el error no se arregla solo.".into() } else { format!("Falló {} veces.", e.intentos) });
                } else {
                    e.siguiente = ahora + reglas::espera_reintento(e.intentos);
                    e.nota = Some(format!("Reintento {} a las {}.", e.intentos + 1, local(e.siguiente).format("%H:%M")));
                }
            }
        }
        db.notif_guardar_envio(&e)?;
    }
    Ok(())
}

/// «Enviar prueba»: al momento (fuera de la cola), y queda en el registro.
#[allow(clippy::too_many_arguments)]
pub fn probar(
    db: &dyn Almacen,
    motor: &Motor,
    ambito: &str,
    canal: &Canal,
    destino: &str,
    quien: &str,
    cliente: Option<&str>,
    ahora: Ts,
) -> R<Result<(), String>> {
    let ajustes = ajustes::ajustes(db)?;
    let secretos = ajustes::abrir_secretos(canal, &motor.clave, ambito);
    let m = Mensaje::Prueba { quien: quien.into() };
    let id = uuid::Uuid::new_v4().to_string();
    let formato = contenido::Formato { url_consola: ajustes.url_consola.clone(), zona: zona(ahora), marcas: BTreeMap::new() };
    // La prueba de un canal del cliente sale con su marca.
    let formato = con_marca(db, &formato, std::slice::from_ref(&m), cliente);
    let r = if ajustes::completo(canal) {
        transporte::enviar(motor.transporte().as_ref(), ambito, canal, &secretos, destino, std::slice::from_ref(&m), &formato, &id, ahora)
    } else {
        Err(transporte::Fallo { permanente: true, texto: "Al canal le faltan datos.".into() })
    };
    db.notif_guardar_envio(&Envio {
        id,
        creado: ahora,
        ambito: ambito.into(),
        cliente: cliente.map(Into::into),
        canal: canal.id.clone(),
        destino: destino.into(),
        tipo: "prueba".into(),
        severidad: Severidad::Informativo,
        incidente: None,
        titulo: "Prueba".into(),
        mensaje: m,
        estado: if r.is_ok() { "enviado" } else { "fallido" }.into(),
        intentos: 1,
        siguiente: ahora,
        error: r.as_ref().err().map(|f| f.texto.clone()),
        enviado: r.is_ok().then_some(ahora),
        entrega: None,
        nota: Some(format!("Prueba de {quien}.")),
    })?;
    Ok(r.map_err(|f| f.texto))
}

#[cfg(test)]
mod pruebas;
