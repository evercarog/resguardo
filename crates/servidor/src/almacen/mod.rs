//! Almacenamiento de Resguardo Server (docs/plataforma.md, §2.3).
//!
//! La interfaz es el trait [`Almacen`]: la API no sabe qué base de datos hay
//! detrás. Implementaciones:
//! - [`sqlite::Sqlite`] (por defecto): `control.db` con cuentas, clientes y
//!   pertenencias, y **un archivo por cliente** (`clientes/<id>.db`) con sus
//!   equipos, órdenes, informes, avisos y auditoría. Un error de consulta no
//!   puede cruzar clientes.
//! - [`postgres`]: pendiente (un esquema por cliente); hoy solo explica que
//!   aún no está disponible.
//!
//! Todo lo que es de un cliente exige un [`ClienteCtx`], que solo crea la
//! capa de autorización (miembro comprobado o equipo autenticado).

pub mod notas;
pub mod postgres;
pub mod sqlite;

pub use notas::{AlmacenNotas, Comentario, IndiceNotas, Observacion};

use crate::notificaciones::{Envio, Incidente, Severidad};
use serde::{Deserialize, Serialize};

/// Unix (segundos).
pub type Ts = i64;

pub fn ahora() -> Ts {
    chrono::Utc::now().timestamp()
}

/// Un cliente ya autorizado: la única forma de llegar a sus datos.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ClienteCtx(String);

impl ClienteCtx {
    /// Solo la capa de autorización (y las pruebas) lo crean.
    pub(crate) fn autorizado(id: &str) -> Self {
        Self(id.to_string())
    }
    pub fn id(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Rol {
    Lectura = 0,
    Tecnico = 1,
    Administrador = 2,
    Propietario = 3,
}

impl Rol {
    pub fn texto(self) -> &'static str {
        match self {
            Rol::Lectura => "lectura",
            Rol::Tecnico => "tecnico",
            Rol::Administrador => "administrador",
            Rol::Propietario => "propietario",
        }
    }
    pub fn de(texto: &str) -> Option<Self> {
        Some(match texto {
            "lectura" => Rol::Lectura,
            "tecnico" => Rol::Tecnico,
            "administrador" => Rol::Administrador,
            "propietario" => Rol::Propietario,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Cuenta {
    pub id: String,
    pub correo: String,
    pub nombre: String,
    pub hash: String,
    pub superusuario: bool,
    pub totp_secreto: Option<String>,
    pub totp_activo: bool,
    /// Último paso de TOTP aceptado (contra la reutilización del mismo código).
    pub totp_ultimo: i64,
}

#[derive(Clone, Debug)]
pub struct Sesion {
    pub cuenta_id: String,
    /// 1: falta el TOTP; 2: completa.
    pub aal: u8,
    pub expira: Ts,
}

#[derive(Clone, Debug, Serialize)]
pub struct Cliente {
    pub id: String,
    pub nombre: String,
    pub sal_cliente: String,
    pub espera_min_horas: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Miembro {
    pub cuenta: String,
    pub correo: String,
    pub nombre: String,
    pub rol: Rol,
}

#[derive(Clone, Debug)]
pub struct EquipoNuevo {
    pub id: String,
    pub nombre: String,
    pub so: String,
    pub version: String,
    pub box_pub: String,
    pub sign_pub: String,
    pub sal_equipo: String,
    pub secreto_hash: String,
}

#[derive(Clone, Debug)]
pub struct Equipo {
    pub id: String,
    pub nombre: String,
    pub so: String,
    pub version_agente: String,
    pub box_pub: String,
    pub sign_pub: String,
    pub sal_equipo: String,
    pub etiqueta: Option<String>,
    pub rol: String,
    pub modo: String,
    pub confirmado: bool,
    pub ultimo_contacto: Option<Ts>,
    pub estado_servicio: Option<String>,
    pub siguiente_seq: u64,
    pub atencion_hasta: Option<Ts>,
    pub resumen: Option<serde_json::Value>,
    /// Espera mínima que confirmó el propio equipo (resultado firmado de
    /// `cambiar_espera`); `None`: la del cliente.
    pub espera_min_horas: Option<i64>,
    /// Etiquetas libres para agrupar (v1.18: «Contabilidad», «Servidores»…), en claro.
    /// No confundir con `etiqueta` (el HMAC con `K_cfg`).
    pub etiquetas: Vec<String>,
    /// v1.58: el número que debe llevar una orden con espera para un agente que no las
    /// guarda (`ordenes_en_espera`): por encima de las que se manden mientras espera, para
    /// que el equipo no la descarte por «antigua» al llegar su hora (ver `numeros_orden`).
    pub seq_espera: u64,
}

/// Holgura de números entre `siguiente_seq` y una orden con espera para un agente anterior:
/// caben tantas órdenes normales mientras espera sin pasarla.
pub const HUECO_SEQ_ESPERA: u64 = 1000;

/// v1.58: los números de orden de un equipo, con lo que ya tiene guardado:
/// - `siguiente`: el de la próxima orden normal. Si por encima del guardado solo quedan
///   órdenes con espera que ya terminaron (canceladas, caducadas…), se salta por encima
///   (el equipo admite huecos), para no tropezar con sus números;
/// - `espera`: el de una orden con espera para un agente que no las guarda, por encima
///   de todas (con `HUECO_SEQ_ESPERA` de holgura para las normales que vengan mientras).
pub fn numeros_orden(siguiente: u64, max_seq: Option<u64>, vivas_arriba: bool) -> (u64, u64) {
    let max = max_seq.unwrap_or(0);
    let siguiente = if vivas_arriba { siguiente } else { siguiente.max(max + 1) };
    let espera = siguiente.saturating_sub(1).max(max) + HUECO_SEQ_ESPERA;
    (siguiente, espera)
}

/// Los equipos sin confirmar que son la misma máquina que `alta` (mismo nombre, sin
/// distinguir mayúsculas, o la misma clave de firma): restos de un intento anterior de
/// vincularla que se quedó en «Falta confirmar el número de comprobación». Nunca
/// recibieron la clave de administración ni guardaron nada: al confirmar `alta` sobran.
pub fn duplicados_sin_confirmar<'a>(alta: &Equipo, equipos: &'a [Equipo]) -> Vec<&'a Equipo> {
    let nombre = alta.nombre.trim().to_lowercase();
    equipos
        .iter()
        .filter(|e| e.id != alta.id && !e.confirmado)
        .filter(|e| (!nombre.is_empty() && e.nombre.trim().to_lowercase() == nombre) || (!alta.sign_pub.is_empty() && e.sign_pub == alta.sign_pub))
        .collect()
}

/// Al confirmar `alta`, quita los equipos sin confirmar que son la misma máquina (un intento
/// anterior de vincularla que se quedó en «Falta confirmar el número de comprobación»): si
/// no, la consola la enseñaba dos veces. Nunca recibieron la clave de administración ni
/// guardaron nada. Queda en la auditoría como `quitar_equipo_duplicado` (detalle: el equipo
/// que se quedó). Devuelve los quitados.
pub fn quitar_duplicados_sin_confirmar(db: &dyn Almacen, ctx: &ClienteCtx, alta: &Equipo) -> R<Vec<String>> {
    let equipos = db.equipos(ctx)?;
    let mut quitados = Vec::new();
    for d in duplicados_sin_confirmar(alta, &equipos) {
        db.anular_emparejamientos_equipo(ctx, &d.id)?;
        db.borrar_equipo(ctx, &d.id)?;
        db.desindexar_equipo(&d.id)?;
        db.auditar(ctx, "servidor", "quitar_equipo_duplicado", &d.id, &serde_json::json!({ "queda": alta.id }).to_string())?;
        quitados.push(d.id.clone());
    }
    Ok(quitados)
}

/// Al unirse un equipo, lo mínimo que queda para comparar el número y dar de alta.
pub const PLAZO_UNIDO_S: Ts = 24 * 3600;

#[derive(Clone, Debug)]
pub struct Emparejamiento {
    pub id: String,
    pub estado: String,
    pub caduca: Ts,
    pub equipo_id: Option<String>,
    /// Preparados (v1.17, instalador listo o línea de Linux): el nombre que tendrá el equipo.
    pub nombre: Option<String>,
    /// «windows» o «linux» (preparados).
    pub so: Option<String>,
    /// El código mientras sirve (abierto o unido): la consola lo necesita para la orden
    /// `alta` y para volver a enseñarlo. Se borra al confirmar, anular o caducar. Los códigos
    /// de 15 min lo guardan desde v1.42 (antes, solo los preparados). v1.48: si el código lo
    /// generó el navegador, aquí solo va `sha256:<hash>` (el servidor nunca lo ve; ver
    /// `api::instaladores::codigo_en_claro`): marca igual que el emparejamiento sigue a medias.
    pub codigo: Option<String>,
    pub creado: Ts,
    /// Versión del código de comprobación que anunció el equipo al unirse (v1.26): 3 si
    /// incluye la huella de la autoridad TLS (agentes ≥ 0.7.10); `None` o 2, el de antes.
    pub sas_version: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct OrdenNueva {
    pub id: String,
    pub equipo_id: String,
    pub tipo: String,
    pub seq: u64,
    pub sellado: String,
    pub emitida_por: String,
    pub not_before: Option<Ts>,
    pub caduca: Ts,
    pub sesion: Option<String>,
    pub relevo: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Orden {
    pub id: String,
    pub equipo_id: String,
    pub tipo: String,
    pub seq: u64,
    pub sellado: String,
    pub emitida: Ts,
    pub emitida_por: String,
    pub not_before: Option<Ts>,
    pub caduca: Ts,
    pub estado: String,
    pub mensaje: Option<String>,
    pub detalle: Option<String>,
    pub firma_agente: Option<String>,
    pub actualizada: Ts,
    /// v1.58: por qué caducó sin aplicarse: `sin_entregar` (no llegó al equipo) o
    /// `sin_respuesta` (llegó y el equipo no contestó). `None` en las demás.
    pub motivo: Option<String>,
}

/// Resultado firmado de una orden, tal como lo envía el agente.
#[derive(Clone, Debug)]
pub struct ResultadoOrden {
    pub orden: String,
    pub estado: String,
    pub mensaje: Option<String>,
    pub detalle: Option<String>,
    pub firma: String,
    /// v1.49: también si ya estaba `cancelada` (el equipo la aplicó antes de saber que
    /// se había cancelado: su resultado firmado manda).
    pub pisar_cancelada: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Aviso {
    pub id: String,
    pub equipo: Option<String>,
    pub tipo: String,
    pub mensaje: String,
    pub creado: Ts,
    pub visto_por: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EntradaAuditoria {
    pub n: i64,
    pub creado: Ts,
    pub actor: String,
    pub accion: String,
    pub objetivo: String,
    pub datos: String,
    pub hash: String,
    pub prev_hash: String,
}

/// Una plantilla de copia tal como la guarda el servidor: cifrada por la consola
/// con una clave derivada de `K_cfg` (no puede leer ni el nombre ni las carpetas).
#[derive(Clone, Debug)]
pub struct PlantillaCifrada {
    pub id: String,
    pub cifrado: String,
    pub actualizada: Ts,
    /// Nombre de quien la guardó.
    pub por: String,
}

/// Tarea 7a (docs/copias-en-cadena.md): un destino del catálogo del cliente.
/// Solo lo que lo describe, en claro (lo mismo que ya dicen los resúmenes de
/// los equipos, más un nombre): **nunca** credenciales ni rutas locales.
#[derive(Clone, Debug, PartialEq)]
pub struct DestinoCatalogo {
    /// `zona:<equipo>:<zona>`, `nube:<equipo>:<nombre>` o el id de un destino.
    pub id: String,
    pub nombre: String,
    /// `zona`, `rest`, `s3`, `b2`, `sftp`, `nube` o `local`.
    pub tipo: String,
    /// Servidor o bucket (solo `rest`, `s3`, `b2` y `sftp`).
    pub donde: Option<String>,
    /// Tarea 8 (docs/regla-3-2-1.md): para la regla 3-2-1-1-0, lo que dice la persona
    /// de este destino (`{ lugar?, inmutable?, soporte? }`, JSON ya validado). En claro:
    /// no son secretos. `None`: lo deducido del tipo.
    pub atributos: Option<String>,
    pub actualizado: Ts,
    /// Nombre de quien lo guardó.
    pub por: String,
}

/// Lo que se ajusta de una etiqueta de equipos (v1.52): su color, la plantilla de
/// copia que se propone a un equipo nuevo con ella y cómo se avisa de sus equipos.
/// En claro, como las etiquetas: son metadatos (la plantilla es un id opaco; su nombre
/// y sus carpetas siguen cifrados).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AjusteEtiqueta {
    /// Tal como se escribe (la clave es en minúsculas).
    pub nombre: String,
    /// Índice de la paleta de la consola (0–6); sin él, el de siempre (por el nombre).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<u8>,
    /// Id de una plantilla de copia del cliente.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plantilla: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avisos: Option<AvisosEtiqueta>,
    #[serde(default)]
    pub actualizada: Ts,
    #[serde(default)]
    pub por: String,
}

/// Cómo se avisa de los equipos con una etiqueta (lo pone el propietario del cliente).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvisosEtiqueta {
    /// Sus avisos (los importantes o críticos de por sí) cuentan como mínimo con esta gravedad.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub importancia: Option<Severidad>,
    /// Canales compartidos (webhook, ntfy, Telegram) que reciben siempre sus avisos,
    /// sea cual sea la gravedad que tengan elegida.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub canales: Vec<CanalRef>,
}

impl AvisosEtiqueta {
    pub fn vacio(&self) -> bool {
        self.importancia.is_none() && self.canales.is_empty()
    }
}

/// Un canal de notificación: del servidor o del propio cliente.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanalRef {
    /// `servidor` o `cliente`.
    pub ambito: String,
    pub id: String,
}

#[derive(Clone, Debug)]
pub struct Relevo {
    pub id: String,
    pub equipo_id: String,
    pub max_bytes: u64,
    pub trozos: u64,
    pub bytes: u64,
    pub estado: String,
    pub caduca: Ts,
}

#[derive(Clone, Debug, Serialize)]
pub struct MensajeSesion {
    pub n: i64,
    pub de: String,
    pub cifrado: String,
}

#[derive(Clone, Debug)]
pub struct SesionInteractiva {
    pub id: String,
    pub equipo_id: String,
    pub abierta_por: String,
    pub expira: Ts,
}

/// Una entrada del historial que guarda el propio equipo (v1.23: lo que pasó
/// antes de llegar a este servidor y lo que va pasando). `datos`: la entrada
/// tal como la manda el agente (JSON, sin rutas).
#[derive(Clone, Debug)]
pub struct EntradaHistorial {
    pub id: String,
    pub hora: Ts,
    pub tipo: String,
    pub datos: String,
    /// Solo en las de tipo «aviso»: (tipo de aviso, mensaje) para la lista de avisos.
    pub aviso: Option<(String, String)>,
}

/// Como mucho tantas entradas del historial por equipo (las más antiguas se borran).
/// El equipo guarda su historial para siempre (lo antiguo resumido por día), así que
/// no se borra por antigüedad: solo por este tope.
pub const MAX_HISTORIAL_EQUIPO: i64 = 20_000;

/// Qué parte del historial de un equipo se pide (v1.26: por páginas).
#[derive(Clone, Debug, Default)]
pub struct FiltroHistorial {
    /// Solo lo posterior a esta hora.
    pub desde: Option<Ts>,
    /// Solo lo de esta hora o anterior.
    pub hasta: Option<Ts>,
    /// Solo estos tipos (vacío: todos).
    pub tipos: Vec<String>,
    /// Cursor: el id de la última entrada de la página anterior. Sigue con las que van
    /// detrás de ella (de la más reciente a la más antigua). Si ya no existe, nada.
    pub antes: Option<String>,
}

pub type R<T> = Result<T, String>;

/// Un cliente visto desde «Clientes del servidor» (v1.34).
#[derive(Clone, Debug, Serialize)]
pub struct ClienteServidor {
    pub id: String,
    pub nombre: String,
    pub creado: Ts,
    pub personas: i64,
    pub propietarios: i64,
}

/// Lo que usa un cliente: solo cifras y fechas.
#[derive(Clone, Debug, Default, Serialize)]
pub struct UsoCliente {
    pub equipos: i64,
    pub equipos_confirmados: i64,
    /// Entradas del historial de todos sus equipos.
    pub historial: i64,
    /// Órdenes de los últimos 30 días.
    pub ordenes_30d: i64,
    /// El último contacto de un equipo o la última entrada de su auditoría.
    pub ultima_actividad: Option<Ts>,
    /// Lo que ocupa su base de datos en el disco.
    pub bytes: u64,
}

/// Lo que la API necesita guardar. Ver [`sqlite::Sqlite`].
pub trait Almacen: Send + Sync + AlmacenNotas {
    // ---------- Servidor ----------
    fn valor(&self, clave: &str) -> R<Option<String>>;
    fn poner_valor(&self, clave: &str, valor: &str) -> R<()>;
    /// Suma `delta` al número guardado en `clave` (0 si no hay), en una sola
    /// sentencia (dos a la vez no se pisan); devuelve el total.
    fn sumar_valor(&self, clave: &str, delta: i64) -> R<i64>;
    /// Auditoría del servidor (inicios de sesión, cuentas, clientes).
    fn auditar_servidor(&self, actor: &str, accion: &str, objetivo: &str, datos: &str) -> R<()>;

    // ---------- Cuentas y sesiones ----------
    fn contar_cuentas(&self) -> R<i64>;
    fn crear_cuenta(&self, correo: &str, nombre: &str, hash: &str, superusuario: bool) -> R<Cuenta>;
    fn cuenta_por_correo(&self, correo: &str) -> R<Option<Cuenta>>;
    fn cuenta(&self, id: &str) -> R<Option<Cuenta>>;
    fn poner_totp(&self, cuenta: &str, secreto: Option<&str>, activo: bool) -> R<()>;
    fn poner_totp_ultimo(&self, cuenta: &str, paso: i64) -> R<()>;
    /// Gasta el paso de TOTP solo si es posterior al último usado (en la misma
    /// sentencia: dos peticiones a la vez con el mismo código no entran las dos).
    fn gastar_totp(&self, cuenta: &str, paso: i64) -> R<bool>;
    fn poner_codigos_recuperacion(&self, cuenta: &str, hashes: &[String]) -> R<()>;
    /// Gasta un código de recuperación si existe y no se usó.
    fn usar_codigo_recuperacion(&self, cuenta: &str, hash: &str) -> R<bool>;
    fn cambiar_contrasena(&self, cuenta: &str, hash: &str) -> R<()>;
    fn renombrar_cuenta(&self, cuenta: &str, nombre: &str) -> R<()>;
    fn crear_sesion(&self, token_hash: &str, cuenta: &str, aal: u8, expira: Ts) -> R<()>;
    fn sesion(&self, token_hash: &str) -> R<Option<Sesion>>;
    fn elevar_sesion(&self, token_hash: &str, expira: Ts) -> R<()>;
    fn borrar_sesion(&self, token_hash: &str) -> R<()>;
    fn borrar_sesiones_de(&self, cuenta: &str, salvo: Option<&str>) -> R<()>;

    // ---------- Clientes y pertenencias ----------
    fn crear_cliente(&self, nombre: &str, sal: &str, espera_min_horas: i64) -> R<Cliente>;
    fn cliente(&self, id: &str) -> R<Option<Cliente>>;
    fn renombrar_cliente(&self, id: &str, nombre: &str) -> R<()>;
    /// Solo tras resultados firmados de `cambiar_espera` (nunca desde la consola).
    fn poner_espera_cliente(&self, id: &str, horas: i64) -> R<()>;
    fn clientes_de(&self, cuenta: &str) -> R<Vec<(Cliente, Rol)>>;
    fn rol(&self, cuenta: &str, cliente: &str) -> R<Option<Rol>>;
    fn poner_rol(&self, cuenta: &str, cliente: &str, rol: Rol) -> R<()>;
    fn quitar_miembro(&self, cuenta: &str, cliente: &str) -> R<()>;
    fn miembros(&self, cliente: &str) -> R<Vec<Miembro>>;
    fn crear_invitacion(&self, token_hash: &str, cliente: &str, rol: Rol, caduca: Ts, por: &str) -> R<()>;
    /// Gasta la invitación (un solo uso) y devuelve su cliente y su rol.
    fn tomar_invitacion(&self, token_hash: &str) -> R<Option<(String, Rol)>>;

    // ---------- Índices globales (de un código o un equipo a su cliente) ----------
    fn indexar_codigo(&self, codigo_hash: &str, cliente: &str, emparejamiento: &str, caduca: Ts) -> R<()>;
    /// Gasta el código: (cliente, emparejamiento) si existe y no ha caducado.
    fn tomar_codigo(&self, codigo_hash: &str) -> R<Option<(String, String)>>;
    /// ¿Hay ya un código con ese hash en el índice (aunque haya caducado y aún no se haya
    /// limpiado)? v1.48: el hash lo manda el navegador y no puede pisar el de otro.
    fn codigo_indexado(&self, codigo_hash: &str) -> R<bool>;
    fn indexar_equipo(&self, equipo: &str, cliente: &str) -> R<()>;
    fn cliente_de_equipo(&self, equipo: &str) -> R<Option<ClienteCtx>>;
    fn desindexar_equipo(&self, equipo: &str) -> R<()>;
    /// Ficha de recepción de un cliente («Recibir un cliente», F6): `usos` altas de equipos hasta `caduca`.
    fn crear_ficha(&self, hash: &str, cliente: &str, usos: i64, caduca: Ts) -> R<()>;
    /// Gasta un uso de la ficha si vale: su cliente.
    fn usar_ficha(&self, hash: &str, ahora: Ts) -> R<Option<String>>;

    // ---------- Equipos y emparejamientos (de un cliente) ----------
    /// Código de 15 min: se guarda el código, como en los preparados, para que la consola
    /// pueda volver a enseñárselo a quien lo pidió mientras sirve (no crear otro al recargar).
    fn crear_emparejamiento(&self, c: &ClienteCtx, id: &str, por: &str, caduca: Ts, codigo: &str) -> R<()>;
    /// Emparejamiento preparado (v1.17): con nombre del equipo, sistema y el código (24 h).
    #[allow(clippy::too_many_arguments)]
    fn preparar_emparejamiento(&self, c: &ClienteCtx, id: &str, por: &str, caduca: Ts, nombre: &str, so: &str, codigo: &str) -> R<()>;
    fn emparejamiento(&self, c: &ClienteCtx, id: &str) -> R<Option<Emparejamiento>>;
    /// Los preparados que aún sirven (abiertos o unidos y sin caducar), del más reciente al más antiguo.
    fn emparejamientos_preparados(&self, c: &ClienteCtx, ahora: Ts) -> R<Vec<Emparejamiento>>;
    /// Los que pidió la cuenta `por` y aún sirven (abiertos o unidos, sin caducar y con su
    /// código guardado), códigos de 15 min y preparados, del más reciente al más antiguo.
    fn emparejamientos_vigentes_de(&self, c: &ClienteCtx, por: &str, ahora: Ts) -> R<Vec<Emparejamiento>>;
    fn poner_estado_emparejamiento(&self, c: &ClienteCtx, id: &str, estado: &str, equipo: Option<&str>) -> R<()>;
    /// Los que se quedaron a medias: unidos (sin caducar) o confirmados sin el alta del equipo.
    fn a_medias(&self, c: &ClienteCtx, ahora: Ts) -> R<Vec<Emparejamiento>>;
    /// Anula los emparejamientos aún vivos de un equipo (abiertos, unidos o confirmados sin el
    /// alta), sin tocar el equipo: al quitar un duplicado sin confirmar.
    fn anular_emparejamientos_equipo(&self, c: &ClienteCtx, equipo: &str) -> R<()>;
    /// El equipo hizo el alta: su código ya no hace falta.
    fn alta_hecha(&self, c: &ClienteCtx, equipo: &str) -> R<()>;
    /// La versión del SAS que anunció el equipo al unirse (v1.26).
    fn poner_sas_emparejamiento(&self, c: &ClienteCtx, id: &str, version: i64) -> R<()>;
    fn crear_equipo(&self, c: &ClienteCtx, e: &EquipoNuevo) -> R<()>;
    fn equipos(&self, c: &ClienteCtx) -> R<Vec<Equipo>>;
    fn equipo(&self, c: &ClienteCtx, id: &str) -> R<Option<Equipo>>;
    fn secreto_equipo(&self, c: &ClienteCtx, id: &str) -> R<Option<String>>;
    fn renombrar_equipo(&self, c: &ClienteCtx, id: &str, nombre: &str) -> R<()>;
    /// Etiquetas libres del equipo (v1.18), ya validadas.
    fn poner_etiquetas_equipo(&self, c: &ClienteCtx, id: &str, etiquetas: &[String]) -> R<()>;
    fn confirmar_equipo(&self, c: &ClienteCtx, id: &str, etiqueta: &str) -> R<()>;
    fn borrar_equipo(&self, c: &ClienteCtx, id: &str) -> R<()>;
    fn contacto_equipo(&self, c: &ClienteCtx, id: &str, cuando: Ts) -> R<()>;
    fn poner_estado_servicio(&self, c: &ClienteCtx, id: &str, estado: Option<&str>, version: Option<&str>) -> R<()>;
    fn poner_atencion(&self, c: &ClienteCtx, id: &str, hasta: Ts) -> R<()>;
    /// Que el siguiente número de orden sea como poco `minimo` (nunca lo baja).
    fn adelantar_seq(&self, c: &ClienteCtx, id: &str, minimo: u64) -> R<()>;
    /// "gestionado" o "local" (el equipo dejó el servidor y sigue solo).
    fn poner_modo(&self, c: &ClienteCtx, id: &str, modo: &str) -> R<()>;
    /// «agente» o «almacenamiento» (guarda copias de otros), según su resumen.
    fn poner_papel(&self, c: &ClienteCtx, id: &str, papel: &str) -> R<()>;
    /// Un equipo que vuelve (con las mismas claves): secreto nuevo y gestionado otra vez.
    fn reactivar_equipo(&self, c: &ClienteCtx, id: &str, secreto_hash: &str) -> R<()>;
    /// La espera mínima que confirmó el equipo (resultado firmado de `cambiar_espera`).
    fn poner_espera_equipo(&self, c: &ClienteCtx, id: &str, horas: i64) -> R<()>;

    // ---------- Configuración e informes ----------
    fn guardar_config(&self, c: &ClienteCtx, equipo: &str, seq: u64, cifrado: &str, resumen: &serde_json::Value) -> R<()>;
    fn config(&self, c: &ClienteCtx, equipo: &str) -> R<Option<(u64, String, serde_json::Value)>>;
    fn guardar_informe(&self, c: &ClienteCtx, equipo: &str, datos: &serde_json::Value) -> R<()>;
    fn informes(&self, c: &ClienteCtx, equipo: &str, limite: i64) -> R<Vec<(Ts, serde_json::Value)>>;

    // ---------- Órdenes ----------
    /// Inserta la orden si `seq` es exactamente el siguiente del equipo; si no, `Err("seq:<siguiente>")`.
    fn insertar_orden(&self, c: &ClienteCtx, o: &OrdenNueva) -> R<Orden>;
    fn orden(&self, c: &ClienteCtx, id: &str) -> R<Option<Orden>>;
    fn ordenes_equipo(&self, c: &ClienteCtx, equipo: &str, limite: i64) -> R<Vec<Orden>>;
    fn ordenes_con_espera(&self, c: &ClienteCtx, ahora: Ts) -> R<Vec<Orden>>;
    /// Órdenes de todo el cliente, de la más reciente a la más antigua, con
    /// filtros opcionales y cursor `antes` = (emitida, id) de la última recibida.
    fn ordenes_cliente(&self, c: &ClienteCtx, equipo: Option<&str>, estado: Option<&str>, antes: Option<(Ts, String)>, limite: i64) -> R<Vec<Orden>>;
    /// Órdenes que el agente aún no tiene (pendientes y sin caducar). Las marca como entregadas.
    /// v1.58: en orden de `seq` y sin saltarse ninguna: una que aún no toca retiene a las de
    /// número mayor (el equipo rechaza por «antigua» la que llega después de otra posterior).
    /// Al entregar una con número ≥ `siguiente_seq`, este sube por encima.
    /// Con `adelantar` (v1.49, el agente admite `ordenes_en_espera`), también las que
    /// piden autorización y aún esperan su `not_before`: el equipo las guarda en espera.
    fn entregar_ordenes(&self, c: &ClienteCtx, equipo: &str, ahora: Ts, adelantar: bool) -> R<Vec<Orden>>;
    /// Las entregadas que el equipo no llegó a recibir (con un número mayor que el último
    /// que aceptó de este servidor: se perdieron en una conexión que ya estaba muerta)
    /// vuelven a pendientes, para entregarlas otra vez. Sin caducar. Devuelve cuántas.
    fn reponer_no_recibidas(&self, c: &ClienteCtx, equipo: &str, ultimo_seq: u64, ahora: Ts) -> R<usize>;
    /// Canceladas desde la última entrega (para avisar al agente) y las marca como avisadas.
    fn canceladas_sin_avisar(&self, c: &ClienteCtx, equipo: &str) -> R<Vec<String>>;
    fn resultado_orden(&self, c: &ClienteCtx, equipo: &str, r: &ResultadoOrden) -> R<bool>;
    fn cancelar_orden(&self, c: &ClienteCtx, id: &str, por: &str, ahora: Ts) -> R<bool>;
    /// v1.58: un agente anterior rechazó una orden con espera porque su reloj aún no llegaba
    /// a `not_before` («Todavía no es la hora»; no anotó su número): vuelve a pendientes y no se
    /// entrega antes de `desde`. Como mucho `max` veces; si no, `false` (se queda rechazada).
    fn reintentar_orden(&self, c: &ClienteCtx, id: &str, desde: Ts, max: i64) -> R<bool>;
    /// v1.58: las órdenes con espera que caducaron sin aplicarse y aún no se avisaron
    /// («No se aplicó…»). Las marca como avisadas.
    fn caducadas_por_avisar(&self, c: &ClienteCtx) -> R<Vec<Orden>>;

    // ---------- Avisos y auditoría ----------
    fn crear_aviso(&self, c: &ClienteCtx, equipo: Option<&str>, tipo: &str, mensaje: &str) -> R<()>;
    fn avisos(&self, c: &ClienteCtx, solo_abiertos: bool) -> R<Vec<Aviso>>;
    fn marcar_aviso(&self, c: &ClienteCtx, id: &str, por: &str) -> R<()>;
    fn auditar(&self, c: &ClienteCtx, actor: &str, accion: &str, objetivo: &str, datos: &str) -> R<()>;
    fn auditoria(&self, c: &ClienteCtx, desde: i64, limite: i64) -> R<Vec<EntradaAuditoria>>;
    /// De la más reciente hacia atrás: entradas con `n < antes` (todas si `None`).
    fn auditoria_desc(&self, c: &ClienteCtx, antes: Option<i64>, limite: i64) -> R<Vec<EntradaAuditoria>>;
    /// `Ok(None)` si la cadena está entera; `Ok(Some(n))` con la primera entrada rota.
    fn verificar_auditoria(&self, c: &ClienteCtx) -> R<(i64, Option<i64>)>;
    /// Auditoría de otro servidor (paquete de exportación): se comprueba la cadena entera
    /// antes de guardarla, de solo añadir, y solo una vez por cliente.
    fn importar_auditoria(&self, c: &ClienteCtx, origen: &str, entradas: &[EntradaAuditoria]) -> R<()>;
    fn auditoria_importada(&self, c: &ClienteCtx, desde: i64, limite: i64) -> R<Vec<EntradaAuditoria>>;
    fn importar_informe(&self, c: &ClienteCtx, equipo: &str, recibido: Ts, datos: &serde_json::Value) -> R<()>;
    fn importar_aviso(&self, c: &ClienteCtx, equipo: Option<&str>, tipo: &str, mensaje: &str, creado: Ts) -> R<()>;

    // ---------- Sesiones interactivas ----------
    fn crear_sesion_interactiva(&self, c: &ClienteCtx, id: &str, equipo: &str, por: &str, expira: Ts) -> R<()>;
    fn sesion_interactiva(&self, c: &ClienteCtx, id: &str) -> R<Option<SesionInteractiva>>;
    fn mensaje_sesion(&self, c: &ClienteCtx, sesion: &str, de: &str, cifrado: &str, expira: Ts) -> R<i64>;
    fn mensajes_sesion(&self, c: &ClienteCtx, sesion: &str, para: &str, desde: i64) -> R<Vec<MensajeSesion>>;
    fn sesiones_abiertas(&self, c: &ClienteCtx, equipo: &str, ahora: Ts) -> R<Vec<String>>;
    fn cerrar_sesion_interactiva(&self, c: &ClienteCtx, id: &str) -> R<()>;

    // ---------- Relé ----------
    fn crear_relevo(&self, c: &ClienteCtx, id: &str, equipo: &str, max_bytes: u64, caduca: Ts) -> R<()>;
    fn relevo(&self, c: &ClienteCtx, id: &str) -> R<Option<Relevo>>;
    fn actualizar_relevo(&self, c: &ClienteCtx, id: &str, trozos: u64, bytes: u64, estado: &str, caduca: Ts) -> R<()>;
    fn borrar_relevo(&self, c: &ClienteCtx, id: &str) -> R<()>;

    // ---------- Plantillas de copia (v1.20, cifradas por la consola) ----------
    fn plantillas(&self, c: &ClienteCtx) -> R<Vec<PlantillaCifrada>>;
    /// Crea o sustituye; `false` si es nueva y ya hay `maximo`.
    fn guardar_plantilla(&self, c: &ClienteCtx, id: &str, cifrado: &str, por: &str, maximo: usize) -> R<bool>;
    fn borrar_plantilla(&self, c: &ClienteCtx, id: &str) -> R<bool>;

    // ---------- Catálogo de destinos (tarea 7a, en claro y sin secretos) ----------
    fn destinos_catalogo(&self, c: &ClienteCtx) -> R<Vec<DestinoCatalogo>>;
    /// Crea o sustituye; `false` si es nuevo y ya hay `maximo`. Con `mantener_atributos`,
    /// los `atributos` que ya tuviera se quedan (una consola anterior que solo renombra).
    fn guardar_destino(&self, c: &ClienteCtx, d: &DestinoCatalogo, maximo: usize, mantener_atributos: bool) -> R<bool>;
    fn borrar_destino(&self, c: &ClienteCtx, id: &str) -> R<bool>;
    // ---------- Ajustes de las etiquetas de los equipos (v1.52) ----------
    fn ajustes_etiquetas(&self, c: &ClienteCtx) -> R<Vec<AjusteEtiqueta>>;
    /// Crea o sustituye (por el nombre, sin distinguir mayúsculas); `false` si es nueva y ya hay `maximo`.
    fn poner_ajuste_etiqueta(&self, c: &ClienteCtx, a: &AjusteEtiqueta, maximo: usize) -> R<bool>;
    /// Vuelve a lo de siempre (color por el nombre, sin plantilla ni avisos propios).
    fn borrar_ajuste_etiqueta(&self, c: &ClienteCtx, nombre: &str) -> R<bool>;
    // ---------- Datos comunes del cliente (0.7.26, bloque 8; `crate::datos_comunes`) ----------
    /// Las filas guardadas: (clave, JSON de la fila).
    fn datos_comunes(&self, c: &ClienteCtx) -> R<Vec<(String, String)>>;
    /// Crea o sustituye una fila.
    fn poner_dato_comun(&self, c: &ClienteCtx, clave: &str, datos: &str) -> R<()>;

    // ---------- Historial de los equipos (v1.23) ----------
    /// Guarda las entradas que no estuvieran ya (por id); devuelve cuántas son nuevas.
    /// Las de tipo «aviso» nuevas entran también en la lista de avisos, ya vistas,
    /// salvo que ya haya uno igual de ese equipo con menos de 10 min de diferencia
    /// (el que mandó el equipo en su momento).
    fn guardar_historial(&self, c: &ClienteCtx, equipo: &str, entradas: &[EntradaHistorial]) -> R<usize> {
        self.guardar_historial_tope(c, equipo, entradas, MAX_HISTORIAL_EQUIPO)
    }
    /// Igual, guardando como mucho `tope` entradas de ese equipo (la cuota del cliente, v1.34).
    fn guardar_historial_tope(&self, c: &ClienteCtx, equipo: &str, entradas: &[EntradaHistorial], tope: i64) -> R<usize>;
    /// Del más reciente al más antiguo, con `hora > desde`.
    /// De la más reciente a la más antigua (por hora y, a igual hora, por id).
    fn historial(&self, c: &ClienteCtx, equipo: &str, filtro: &FiltroHistorial, limite: i64) -> R<Vec<EntradaHistorial>>;
    /// La hora de la entrada más reciente de ese equipo.
    fn ultima_historial(&self, c: &ClienteCtx, equipo: &str) -> R<Option<Ts>>;

    // ---------- Notificaciones (`crate::notificaciones`; en `control.db`) ----------
    /// Apunta un evento (JSON) para la tarea de fondo.
    fn notif_evento(&self, datos: &str) -> R<()>;
    /// Los eventos apuntados, del más antiguo al más reciente: (n, datos).
    fn notif_eventos(&self, limite: i64) -> R<Vec<(i64, String)>>;
    fn notif_borrar_evento(&self, n: i64) -> R<()>;
    fn notif_incidente(&self, clave: &str) -> R<Option<Incidente>>;
    /// Crea o sustituye.
    fn notif_guardar_incidente(&self, i: &Incidente) -> R<()>;
    /// Los abiertos (de un cliente o de todos).
    fn notif_incidentes_abiertos(&self, cliente: Option<&str>) -> R<Vec<Incidente>>;
    /// Crea o sustituye un envío de la cola.
    fn notif_guardar_envio(&self, e: &Envio) -> R<()>;
    /// Los pendientes cuya hora ya llegó, del más antiguo al más reciente.
    fn notif_envios_debidos(&self, ahora: Ts, limite: i64) -> R<Vec<Envio>>;
    /// Las entregas (un mensaje, aunque lleve varios avisos agrupados) a ese canal y destino
    /// desde `desde`, sin los resúmenes ni las pruebas: la hora de cada una.
    fn notif_entregas_desde(&self, canal: &str, destino: &str, desde: Ts) -> R<Vec<Ts>>;
    fn notif_envios_incidente(&self, incidente: &str) -> R<Vec<Envio>>;
    /// Lo último que se mandó o se intentó mandar (de un cliente: lo suyo y lo de sus canales).
    fn notif_registro(&self, cliente: Option<&str>, limite: i64) -> R<Vec<Envio>>;
    /// Borra lo terminado de antes de `antes` (envíos e incidentes cerrados).
    fn notif_limpiar(&self, antes: Ts) -> R<()>;

    // ---------- Mantenimiento ----------
    fn todos_los_clientes(&self) -> R<Vec<ClienteCtx>>;
    /// Todos los clientes con cuándo se crearon y cuántas personas y propietarios tienen
    /// («Clientes del servidor», v1.34: sin nada de dentro de cada cliente).
    fn clientes_del_servidor(&self) -> R<Vec<ClienteServidor>>;
    /// Cuánto usa un cliente (solo cifras, para «Clientes del servidor»).
    fn uso_cliente(&self, c: &ClienteCtx) -> R<UsoCliente>;
    /// Borra sesiones, códigos, invitaciones y mensajes caducados; devuelve los relevos caducados (para borrar sus archivos).
    fn limpiar(&self, c: &ClienteCtx, ahora: Ts) -> R<Vec<String>>;
    fn limpiar_servidor(&self, ahora: Ts) -> R<()>;
    /// Equipos que se unieron con un código y nunca se confirmaron, cuyo
    /// emparejamiento ya caducó o se anuló: quedaban «Sin confirmar» para siempre.
    fn equipos_sin_alta(&self, c: &ClienteCtx) -> R<Vec<String>>;
}
