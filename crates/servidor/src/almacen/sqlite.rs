//! SQLite: `control.db` y un archivo por cliente (`clientes/<id>.db`), en
//! modo WAL. Cada conexión va tras su propio `Mutex`; las consultas son
//! cortas.

use super::*;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct Sqlite {
    dir: PathBuf,
    control: Mutex<Connection>,
    clientes: Mutex<Conexiones>,
}

/// Conexiones abiertas a la vez con las bases de los clientes, como mucho (las menos
/// usadas se cierran). En la consola en línea puede haber muchos clientes: sin tope,
/// cada uno dejaba su archivo abierto para siempre.
pub const MAX_CONEXIONES_CLIENTES: usize = 128;

/// Caché de conexiones por cliente (LRU acotada). Nunca cierra una que se está usando
/// (alguien más tiene su `Arc`): si se abriera otra para el mismo cliente, dos
/// conexiones podrían escribir a la vez (p. ej. dos entradas de auditoría con el mismo
/// número). Si todas están en uso, se pasa del tope un momento.
struct Conexiones {
    tope: usize,
    /// Cliente → (conexión, último uso).
    abiertas: HashMap<String, (Arc<Mutex<Connection>>, u64)>,
    reloj: u64,
}

impl Conexiones {
    fn nuevas(tope: usize) -> Self {
        Self { tope: tope.max(1), abiertas: HashMap::new(), reloj: 0 }
    }

    fn tomar(&mut self, id: &str) -> Option<Arc<Mutex<Connection>>> {
        self.reloj += 1;
        let reloj = self.reloj;
        self.abiertas.get_mut(id).map(|(con, uso)| {
            *uso = reloj;
            con.clone()
        })
    }

    /// Guarda una nueva y devuelve las que hay que cerrar (fuera del cerrojo).
    fn poner(&mut self, id: &str, con: Arc<Mutex<Connection>>) -> Vec<Arc<Mutex<Connection>>> {
        let mut cerrar = Vec::new();
        while self.abiertas.len() >= self.tope {
            let libre = self.abiertas.iter().filter(|(_, (c, _))| Arc::strong_count(c) == 1).min_by_key(|(_, (_, uso))| *uso).map(|(k, _)| k.clone());
            match libre.and_then(|k| self.abiertas.remove(&k)) {
                Some((c, _)) => cerrar.push(c),
                None => break,
            }
        }
        self.reloj += 1;
        self.abiertas.insert(id.to_string(), (con, self.reloj));
        cerrar
    }
}

const ESQUEMA_CONTROL: &str = r#"
CREATE TABLE IF NOT EXISTS servidor (clave TEXT PRIMARY KEY, valor TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS cuentas (
  id TEXT PRIMARY KEY, correo TEXT NOT NULL UNIQUE COLLATE NOCASE, nombre TEXT NOT NULL, hash TEXT NOT NULL,
  superusuario INTEGER NOT NULL DEFAULT 0, totp_secreto TEXT, totp_activo INTEGER NOT NULL DEFAULT 0,
  totp_ultimo INTEGER NOT NULL DEFAULT 0, creada INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS recuperacion (cuenta_id TEXT NOT NULL, hash TEXT NOT NULL, usado INTEGER NOT NULL DEFAULT 0, PRIMARY KEY (cuenta_id, hash));
CREATE TABLE IF NOT EXISTS sesiones (token_hash TEXT PRIMARY KEY, cuenta_id TEXT NOT NULL, aal INTEGER NOT NULL, expira INTEGER NOT NULL, creada INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS clientes (id TEXT PRIMARY KEY, nombre TEXT NOT NULL, sal TEXT NOT NULL, espera_min_horas INTEGER NOT NULL, creado INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS pertenencias (cuenta_id TEXT NOT NULL, cliente_id TEXT NOT NULL, rol TEXT NOT NULL, PRIMARY KEY (cuenta_id, cliente_id));
CREATE TABLE IF NOT EXISTS invitaciones (token_hash TEXT PRIMARY KEY, cliente_id TEXT NOT NULL, rol TEXT NOT NULL, caduca INTEGER NOT NULL, creada_por TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS codigos (codigo_hash TEXT PRIMARY KEY, cliente_id TEXT NOT NULL, emparejamiento_id TEXT NOT NULL, caduca INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS codigos_varios (codigo_hash TEXT PRIMARY KEY, lote_id TEXT NOT NULL UNIQUE, cliente_id TEXT NOT NULL, caduca INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS indice_equipos (equipo_id TEXT PRIMARY KEY, cliente_id TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS fichas (hash TEXT PRIMARY KEY, cliente_id TEXT NOT NULL, usos INTEGER NOT NULL, caduca INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS notif_eventos (n INTEGER PRIMARY KEY, creado INTEGER NOT NULL, datos TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS notif_incidentes (
  clave TEXT PRIMARY KEY, cliente_id TEXT NOT NULL, equipo_id TEXT, tipo TEXT NOT NULL, sujeto TEXT NOT NULL, severidad TEXT NOT NULL,
  titulo TEXT NOT NULL, mensaje TEXT NOT NULL, abierto INTEGER NOT NULL, primero INTEGER NOT NULL, ultimo INTEGER NOT NULL,
  veces INTEGER NOT NULL, marca TEXT, notificado INTEGER, cerrado INTEGER);
CREATE INDEX IF NOT EXISTS notif_incidentes_abiertos ON notif_incidentes (abierto, cliente_id);
CREATE TABLE IF NOT EXISTS notif_envios (
  id TEXT PRIMARY KEY, creado INTEGER NOT NULL, ambito TEXT NOT NULL, cliente_id TEXT, canal_id TEXT NOT NULL, destino TEXT NOT NULL,
  tipo TEXT NOT NULL, severidad TEXT NOT NULL, incidente TEXT, titulo TEXT NOT NULL, mensaje TEXT NOT NULL, estado TEXT NOT NULL,
  intentos INTEGER NOT NULL, siguiente INTEGER NOT NULL, error TEXT, enviado INTEGER, entrega TEXT, nota TEXT);
CREATE INDEX IF NOT EXISTS notif_envios_cola ON notif_envios (estado, siguiente);
CREATE INDEX IF NOT EXISTS notif_envios_destino ON notif_envios (canal_id, destino, enviado);
CREATE INDEX IF NOT EXISTS notif_envios_incidente ON notif_envios (incidente);
CREATE INDEX IF NOT EXISTS notif_envios_creado ON notif_envios (creado);
"#;

const COLS_INCIDENTE: &str =
    "clave, cliente_id, equipo_id, tipo, sujeto, severidad, titulo, mensaje, abierto, primero, ultimo, veces, marca, notificado, cerrado";
fn fila_incidente(r: &rusqlite::Row) -> rusqlite::Result<Incidente> {
    Ok(Incidente {
        clave: r.get(0)?,
        cliente: r.get(1)?,
        equipo: r.get(2)?,
        tipo: r.get(3)?,
        sujeto: r.get(4)?,
        severidad: crate::notificaciones::Severidad::de(&r.get::<_, String>(5)?).unwrap_or(crate::notificaciones::Severidad::Informativo),
        titulo: r.get(6)?,
        mensaje: r.get(7)?,
        abierto: r.get::<_, i64>(8)? != 0,
        primero: r.get(9)?,
        ultimo: r.get(10)?,
        veces: r.get(11)?,
        marca: r.get(12)?,
        notificado: r.get(13)?,
        cerrado: r.get(14)?,
    })
}

const COLS_ENVIO: &str =
    "id, creado, ambito, cliente_id, canal_id, destino, tipo, severidad, incidente, titulo, mensaje, estado, intentos, siguiente, error, enviado, entrega, nota";
fn fila_envio(r: &rusqlite::Row) -> rusqlite::Result<Option<Envio>> {
    let mensaje: String = r.get(10)?;
    let Ok(mensaje) = serde_json::from_str(&mensaje) else { return Ok(None) };
    Ok(Some(Envio {
        id: r.get(0)?,
        creado: r.get(1)?,
        ambito: r.get(2)?,
        cliente: r.get(3)?,
        canal: r.get(4)?,
        destino: r.get(5)?,
        tipo: r.get(6)?,
        severidad: crate::notificaciones::Severidad::de(&r.get::<_, String>(7)?).unwrap_or(crate::notificaciones::Severidad::Informativo),
        incidente: r.get(8)?,
        titulo: r.get(9)?,
        mensaje,
        estado: r.get(11)?,
        intentos: r.get::<_, i64>(12)? as u32,
        siguiente: r.get(13)?,
        error: r.get(14)?,
        enviado: r.get(15)?,
        entrega: r.get(16)?,
        nota: r.get(17)?,
    }))
}

fn envios(c: &Connection, sql: &str, p: impl rusqlite::Params) -> R<Vec<Envio>> {
    let mut st = c.prepare(sql).map_err(s)?;
    let filas = st.query_map(p, fila_envio).map_err(s)?;
    let mut out = Vec::new();
    for f in filas {
        out.extend(f.map_err(s)?);
    }
    Ok(out)
}

const ESQUEMA_AUDITORIA: &str = r#"
CREATE TABLE IF NOT EXISTS auditoria (
  n INTEGER PRIMARY KEY, creado INTEGER NOT NULL, actor TEXT NOT NULL, accion TEXT NOT NULL, objetivo TEXT NOT NULL,
  datos TEXT NOT NULL, prev_hash TEXT NOT NULL, hash TEXT NOT NULL);
CREATE TRIGGER IF NOT EXISTS auditoria_sin_cambios BEFORE UPDATE ON auditoria BEGIN SELECT RAISE(ABORT, 'auditoria: solo añadir'); END;
CREATE TRIGGER IF NOT EXISTS auditoria_sin_borrar BEFORE DELETE ON auditoria BEGIN SELECT RAISE(ABORT, 'auditoria: solo añadir'); END;
"#;

const ESQUEMA_CLIENTE: &str = r#"
CREATE TABLE IF NOT EXISTS equipos (
  id TEXT PRIMARY KEY, nombre TEXT NOT NULL, so TEXT NOT NULL, version_agente TEXT NOT NULL,
  box_pub TEXT NOT NULL, sign_pub TEXT NOT NULL, sal_equipo TEXT NOT NULL, etiqueta TEXT,
  rol TEXT NOT NULL DEFAULT 'agente', modo TEXT NOT NULL DEFAULT 'gestionado', confirmado INTEGER NOT NULL DEFAULT 0,
  secreto_hash TEXT NOT NULL, ultimo_contacto INTEGER, estado_servicio TEXT, siguiente_seq INTEGER NOT NULL DEFAULT 1,
  atencion_hasta INTEGER, creado INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS emparejamientos (id TEXT PRIMARY KEY, estado TEXT NOT NULL, caduca INTEGER NOT NULL, creado_por TEXT NOT NULL, equipo_id TEXT, creado INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS lotes (
  id TEXT PRIMARY KEY, codigo_hash TEXT NOT NULL, nombre TEXT, usos INTEGER NOT NULL, usados INTEGER NOT NULL DEFAULT 0,
  caduca INTEGER NOT NULL, creado INTEGER NOT NULL, creado_por TEXT NOT NULL, anulado INTEGER, rechazos INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS configs (equipo_id TEXT PRIMARY KEY, seq INTEGER NOT NULL, cifrado TEXT NOT NULL, resumen TEXT NOT NULL, actualizada INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS informes (id INTEGER PRIMARY KEY, equipo_id TEXT NOT NULL, recibido INTEGER NOT NULL, datos TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS informes_equipo ON informes (equipo_id, recibido);
CREATE TABLE IF NOT EXISTS ordenes (
  id TEXT PRIMARY KEY, equipo_id TEXT NOT NULL, tipo TEXT NOT NULL, seq INTEGER NOT NULL, sellado TEXT NOT NULL,
  emitida INTEGER NOT NULL, emitida_por TEXT NOT NULL, not_before INTEGER, caduca INTEGER NOT NULL,
  estado TEXT NOT NULL, mensaje TEXT, detalle TEXT, firma_agente TEXT, actualizada INTEGER NOT NULL,
  sesion TEXT, relevo TEXT, cancelacion_avisada INTEGER NOT NULL DEFAULT 1, UNIQUE (equipo_id, seq));
CREATE TABLE IF NOT EXISTS avisos (id TEXT PRIMARY KEY, equipo_id TEXT, tipo TEXT NOT NULL, mensaje TEXT NOT NULL, creado INTEGER NOT NULL, visto_por TEXT);
CREATE TABLE IF NOT EXISTS sesiones_i (id TEXT PRIMARY KEY, equipo_id TEXT NOT NULL, abierta_por TEXT NOT NULL, expira INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS mensajes_i (sesion_id TEXT NOT NULL, n INTEGER NOT NULL, de TEXT NOT NULL, cifrado TEXT NOT NULL, creado INTEGER NOT NULL, PRIMARY KEY (sesion_id, n));
CREATE TABLE IF NOT EXISTS relevos (id TEXT PRIMARY KEY, equipo_id TEXT NOT NULL, max_bytes INTEGER NOT NULL, trozos INTEGER NOT NULL DEFAULT 0, bytes INTEGER NOT NULL DEFAULT 0, estado TEXT NOT NULL, caduca INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS historial (equipo_id TEXT NOT NULL, id TEXT NOT NULL, hora INTEGER NOT NULL, tipo TEXT NOT NULL, datos TEXT NOT NULL, PRIMARY KEY (equipo_id, id));
CREATE INDEX IF NOT EXISTS historial_hora ON historial (equipo_id, hora);
CREATE TABLE IF NOT EXISTS plantillas (id TEXT PRIMARY KEY, cifrado TEXT NOT NULL, actualizada INTEGER NOT NULL, por TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS destinos (id TEXT PRIMARY KEY, nombre TEXT NOT NULL, tipo TEXT NOT NULL, donde TEXT, actualizado INTEGER NOT NULL, por TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS etiquetas_ajustes (clave TEXT PRIMARY KEY, datos TEXT NOT NULL, actualizada INTEGER NOT NULL, por TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS datos_comunes (clave TEXT PRIMARY KEY, datos TEXT NOT NULL, actualizada INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS auditoria_importada (
  n INTEGER PRIMARY KEY, creado INTEGER NOT NULL, actor TEXT NOT NULL, accion TEXT NOT NULL, objetivo TEXT NOT NULL,
  datos TEXT NOT NULL, prev_hash TEXT NOT NULL, hash TEXT NOT NULL, origen TEXT NOT NULL);
CREATE TRIGGER IF NOT EXISTS auditoria_importada_sin_cambios BEFORE UPDATE ON auditoria_importada BEGIN SELECT RAISE(ABORT, 'auditoria: solo añadir'); END;
CREATE TRIGGER IF NOT EXISTS auditoria_importada_sin_borrar BEFORE DELETE ON auditoria_importada BEGIN SELECT RAISE(ABORT, 'auditoria: solo añadir'); END;
"#;

fn abrir(path: &Path, esquema: &str) -> R<Connection> {
    let c = Connection::open(path).map_err(|e| format!("No se pudo abrir {}: {e}", path.display()))?;
    c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;").map_err(s)?;
    c.execute_batch(esquema).map_err(s)?;
    c.execute_batch(ESQUEMA_AUDITORIA).map_err(s)?;
    Ok(c)
}

/// Columnas añadidas después de la primera versión del esquema.
fn migrar_cliente(db: &Connection) -> R<()> {
    let tiene = |tabla: &str, col: &str| -> R<bool> {
        let mut st = db.prepare(&format!("SELECT name FROM pragma_table_info('{tabla}')")).map_err(s)?;
        let cols = st.query_map([], |r| r.get::<_, String>(0)).map_err(s)?.collect::<Result<Vec<_>, _>>().map_err(s)?;
        Ok(cols.iter().any(|c| c == col))
    };
    if !tiene("equipos", "espera_min_horas")? {
        db.execute_batch("ALTER TABLE equipos ADD COLUMN espera_min_horas INTEGER").map_err(s)?;
    }
    // v1.18: etiquetas libres de cada equipo (lista JSON, en claro; no es la `etiqueta` HMAC).
    if !tiene("equipos", "etiquetas")? {
        db.execute_batch("ALTER TABLE equipos ADD COLUMN etiquetas TEXT NOT NULL DEFAULT '[]'").map_err(s)?;
    }
    // v1.17: emparejamientos preparados (instalador listo o línea de Linux): nombre, sistema y
    // el código (lo genera el servidor; se borra al confirmar, anular o caducar).
    for col in ["nombre", "so", "codigo"] {
        if !tiene("emparejamientos", col)? {
            db.execute_batch(&format!("ALTER TABLE emparejamientos ADD COLUMN {col} TEXT")).map_err(s)?;
        }
    }
    // v1.26: la versión del SAS que anunció el equipo al unirse (3: con la huella TLS).
    if !tiene("emparejamientos", "sas_version")? {
        db.execute_batch("ALTER TABLE emparejamientos ADD COLUMN sas_version INTEGER").map_err(s)?;
    }
    // Bloque 7: el código para varios equipos con el que se unió y desde qué IP.
    for col in ["lote", "ip"] {
        if !tiene("emparejamientos", col)? {
            db.execute_batch(&format!("ALTER TABLE emparejamientos ADD COLUMN {col} TEXT")).map_err(s)?;
        }
    }
    // Tarea 8: lo que dice la persona de cada destino para la regla 3-2-1-1-0 (JSON).
    if !tiene("destinos", "atributos")? {
        db.execute_batch("ALTER TABLE destinos ADD COLUMN atributos TEXT").map_err(s)?;
    }
    // v1.58 (órdenes con espera que no se aplicaban): por qué caducó (`sin_entregar` o
    // `sin_respuesta`), si falta avisar de que no se aplicó, desde cuándo se puede volver a
    // entregar (reloj del equipo atrasado) y cuántas veces se volvió a intentar.
    for (col, def) in
        [("motivo", "TEXT"), ("fin_avisado", "INTEGER NOT NULL DEFAULT 1"), ("entregar_desde", "INTEGER"), ("reintentos", "INTEGER NOT NULL DEFAULT 0")]
    {
        if !tiene("ordenes", col)? {
            db.execute_batch(&format!("ALTER TABLE ordenes ADD COLUMN {col} {def}")).map_err(s)?;
        }
    }
    Ok(())
}

const COLS_EMP: &str = "id, estado, caduca, equipo_id, nombre, so, codigo, creado, sas_version, lote, ip";

fn fila_emparejamiento(r: &rusqlite::Row<'_>) -> rusqlite::Result<Emparejamiento> {
    Ok(Emparejamiento {
        id: r.get(0)?,
        estado: r.get(1)?,
        caduca: r.get(2)?,
        equipo_id: r.get(3)?,
        nombre: r.get(4)?,
        so: r.get(5)?,
        codigo: r.get(6)?,
        creado: r.get(7)?,
        sas_version: r.get(8)?,
        lote: r.get(9)?,
        ip: r.get(10)?,
    })
}

const COLS_LOTE: &str = "id, codigo_hash, nombre, usos, usados, caduca, creado, creado_por, anulado, rechazos";

fn fila_lote(r: &rusqlite::Row<'_>) -> rusqlite::Result<Lote> {
    Ok(Lote {
        id: r.get(0)?,
        codigo_hash: r.get(1)?,
        nombre: r.get(2)?,
        usos: r.get(3)?,
        usados: r.get(4)?,
        caduca: r.get(5)?,
        creado: r.get(6)?,
        creado_por: r.get(7)?,
        anulado: r.get(8)?,
        rechazos: r.get(9)?,
    })
}

fn s(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn nuevo_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Id de cliente válido como nombre de archivo (lo genera el propio servidor, pero se comprueba).
fn id_seguro(id: &str) -> R<&str> {
    if !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        Ok(id)
    } else {
        Err("Id de cliente no válido.".into())
    }
}

const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn hash_entrada(prev: &str, n: i64, creado: Ts, actor: &str, accion: &str, objetivo: &str, datos: &str) -> String {
    let h = Sha256::digest(format!("{prev}|{n}|{creado}|{actor}|{accion}|{objetivo}|{datos}").as_bytes());
    h.iter().map(|b| format!("{b:02x}")).collect()
}

fn auditar_en(c: &Connection, actor: &str, accion: &str, objetivo: &str, datos: &str) -> R<()> {
    let (n, prev): (i64, String) = c
        .query_row("SELECT n, hash FROM auditoria ORDER BY n DESC LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()
        .map_err(s)?
        .unwrap_or((0, GENESIS.into()));
    let n = n + 1;
    let creado = ahora();
    let hash = hash_entrada(&prev, n, creado, actor, accion, objetivo, datos);
    c.execute(
        "INSERT INTO auditoria (n, creado, actor, accion, objetivo, datos, prev_hash, hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![n, creado, actor, accion, objetivo, datos, prev, hash],
    )
    .map_err(s)?;
    Ok(())
}

impl Sqlite {
    /// Abre (o crea) los archivos en `dir`.
    pub fn abrir(dir: &Path) -> R<Self> {
        std::fs::create_dir_all(dir.join("clientes")).map_err(s)?;
        let control = abrir(&dir.join("control.db"), ESQUEMA_CONTROL)?;
        Ok(Self { dir: dir.to_path_buf(), control: Mutex::new(control), clientes: Mutex::new(Conexiones::nuevas(MAX_CONEXIONES_CLIENTES)) })
    }

    /// Con otro tope de conexiones por cliente (para las pruebas).
    #[cfg(test)]
    fn abrir_con_tope(dir: &Path, tope: usize) -> R<Self> {
        let a = Self::abrir(dir)?;
        *a.clientes.lock().unwrap_or_else(|e| e.into_inner()) = Conexiones::nuevas(tope);
        Ok(a)
    }

    fn ctl(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.control.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn conexion(&self, c: &ClienteCtx) -> R<Arc<Mutex<Connection>>> {
        let mut mapa = self.clientes.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(con) = mapa.tomar(c.id()) {
            return Ok(con);
        }
        let path = self.dir.join("clientes").join(format!("{}.db", id_seguro(c.id())?));
        let db = abrir(&path, ESQUEMA_CLIENTE)?;
        migrar_cliente(&db)?;
        db.execute_batch(super::notas::ESQUEMA).map_err(s)?;
        let con = Arc::new(Mutex::new(db));
        let cerrar = mapa.poner(c.id(), con.clone());
        drop(mapa);
        // Cerrar una conexión puede pasar el WAL a la base: sin bloquear a los demás.
        drop(cerrar);
        Ok(con)
    }

    /// Cuántas conexiones con clientes hay abiertas (para las pruebas).
    #[cfg(test)]
    fn abiertas(&self) -> usize {
        self.clientes.lock().unwrap_or_else(|e| e.into_inner()).abiertas.len()
    }

    pub(super) fn con<T>(&self, c: &ClienteCtx, f: impl FnOnce(&Connection) -> R<T>) -> R<T> {
        let con = self.conexion(c)?;
        let guard = con.lock().unwrap_or_else(|e| e.into_inner());
        f(&guard)
    }
}

fn fila_cuenta(r: &rusqlite::Row) -> rusqlite::Result<Cuenta> {
    Ok(Cuenta {
        id: r.get(0)?,
        correo: r.get(1)?,
        nombre: r.get(2)?,
        hash: r.get(3)?,
        superusuario: r.get::<_, i64>(4)? != 0,
        totp_secreto: r.get(5)?,
        totp_activo: r.get::<_, i64>(6)? != 0,
        totp_ultimo: r.get(7)?,
    })
}
const COLS_CUENTA: &str = "id, correo, nombre, hash, superusuario, totp_secreto, totp_activo, totp_ultimo";

fn fila_auditoria(r: &rusqlite::Row) -> rusqlite::Result<EntradaAuditoria> {
    Ok(EntradaAuditoria {
        n: r.get(0)?,
        creado: r.get(1)?,
        actor: r.get(2)?,
        accion: r.get(3)?,
        objetivo: r.get(4)?,
        datos: r.get(5)?,
        hash: r.get(6)?,
        prev_hash: r.get(7)?,
    })
}

fn fila_cliente(r: &rusqlite::Row) -> rusqlite::Result<Cliente> {
    Ok(Cliente { id: r.get(0)?, nombre: r.get(1)?, sal_cliente: r.get(2)?, espera_min_horas: r.get(3)? })
}

const COLS_EQUIPO: &str = "e.id, e.nombre, e.so, e.version_agente, e.box_pub, e.sign_pub, e.sal_equipo, e.etiqueta, e.rol, e.modo, e.confirmado, \
                           e.ultimo_contacto, e.estado_servicio, e.siguiente_seq, e.atencion_hasta, c.resumen, e.espera_min_horas, e.etiquetas,                            (SELECT MAX(o.seq) FROM ordenes o WHERE o.equipo_id = e.id),                            EXISTS (SELECT 1 FROM ordenes o WHERE o.equipo_id = e.id AND o.seq >= e.siguiente_seq AND o.estado IN ('pendiente', 'entregada', 'en_marcha'))";
fn fila_equipo(r: &rusqlite::Row) -> rusqlite::Result<Equipo> {
    let resumen: Option<String> = r.get(15)?;
    let (siguiente_seq, seq_espera) =
        super::numeros_orden(r.get::<_, i64>(13)? as u64, r.get::<_, Option<i64>>(18)?.map(|n| n as u64), r.get::<_, i64>(19)? != 0);
    Ok(Equipo {
        id: r.get(0)?,
        nombre: r.get(1)?,
        so: r.get(2)?,
        version_agente: r.get(3)?,
        box_pub: r.get(4)?,
        sign_pub: r.get(5)?,
        sal_equipo: r.get(6)?,
        etiqueta: r.get(7)?,
        rol: r.get(8)?,
        modo: r.get(9)?,
        confirmado: r.get::<_, i64>(10)? != 0,
        ultimo_contacto: r.get(11)?,
        estado_servicio: r.get(12)?,
        siguiente_seq,
        atencion_hasta: r.get(14)?,
        resumen: resumen.and_then(|t| serde_json::from_str(&t).ok()),
        espera_min_horas: r.get(16)?,
        etiquetas: r.get::<_, Option<String>>(17)?.and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default(),
        seq_espera,
    })
}

/// Los números de orden de un equipo (ver `numeros_orden`), dentro de una transacción.
fn numeros_equipo(db: &Connection, equipo: &str) -> R<Option<(u64, u64)>> {
    let fila = db
        .query_row(
            "SELECT e.siguiente_seq, (SELECT MAX(o.seq) FROM ordenes o WHERE o.equipo_id = e.id),
                    EXISTS (SELECT 1 FROM ordenes o WHERE o.equipo_id = e.id AND o.seq >= e.siguiente_seq AND o.estado IN ('pendiente', 'entregada', 'en_marcha'))
             FROM equipos e WHERE e.id = ?1",
            [equipo],
            |r| Ok((r.get::<_, i64>(0)? as u64, r.get::<_, Option<i64>>(1)?.map(|n| n as u64), r.get::<_, i64>(2)? != 0)),
        )
        .optional()
        .map_err(s)?;
    Ok(fila.map(|(sig, max, vivas)| super::numeros_orden(sig, max, vivas)))
}

/// Deja `siguiente_seq` en `desde` (si es mayor) y, si ese número ya lo tiene otra orden
/// (una con espera para un agente anterior), en el primero libre por encima.
fn poner_siguiente(db: &Connection, equipo: &str, desde: u64) -> R<()> {
    let mut n = i64::try_from(desde).unwrap_or(i64::MAX);
    while db.query_row("SELECT EXISTS (SELECT 1 FROM ordenes WHERE equipo_id = ?1 AND seq = ?2)", params![equipo, n], |r| r.get::<_, i64>(0)).map_err(s)? != 0 {
        n += 1;
    }
    db.execute("UPDATE equipos SET siguiente_seq = ?2 WHERE id = ?1 AND siguiente_seq < ?2", params![equipo, n]).map_err(s)?;
    Ok(())
}

/// Marca «caducada» lo que caducó sin aplicarse (de un equipo o de todos), con su motivo; las
/// que tenían espera quedan por avisar («No se aplicó…»).
fn caducar(db: &Connection, equipo: Option<&str>, ahora: Ts) -> R<()> {
    db.execute(
        "UPDATE ordenes SET estado = 'caducada', actualizada = ?2,
           motivo = CASE estado WHEN 'pendiente' THEN 'sin_entregar' ELSE 'sin_respuesta' END,
           fin_avisado = CASE WHEN not_before IS NULL THEN 1 ELSE 0 END
         WHERE (?1 IS NULL OR equipo_id = ?1) AND estado IN ('pendiente', 'entregada') AND caduca <= ?2",
        params![equipo, ahora],
    )
    .map_err(s)?;
    Ok(())
}

/// v1.49: ¿se puede entregar antes de su hora a un agente que admite `ordenes_en_espera`?
/// Solo las que piden autorización (el equipo la comprueba al recibirlas).
fn adelantable(tipo: &str) -> bool {
    resguardo_protocolo::ordenes::tipo(tipo).is_some_and(|t| t.nivel != resguardo_protocolo::ordenes::Nivel::Inofensiva)
}

const COLS_ORDEN: &str =
    "id, equipo_id, tipo, seq, sellado, emitida, emitida_por, not_before, caduca, estado, mensaje, detalle, firma_agente, actualizada, motivo";
fn fila_orden(r: &rusqlite::Row) -> rusqlite::Result<Orden> {
    Ok(Orden {
        id: r.get(0)?,
        equipo_id: r.get(1)?,
        tipo: r.get(2)?,
        seq: r.get::<_, i64>(3)? as u64,
        sellado: r.get(4)?,
        emitida: r.get(5)?,
        emitida_por: r.get(6)?,
        not_before: r.get(7)?,
        caduca: r.get(8)?,
        estado: r.get(9)?,
        mensaje: r.get(10)?,
        detalle: r.get(11)?,
        firma_agente: r.get(12)?,
        actualizada: r.get(13)?,
        motivo: r.get(14)?,
    })
}

impl Almacen for Sqlite {
    // ---------- Servidor ----------
    fn valor(&self, clave: &str) -> R<Option<String>> {
        self.ctl().query_row("SELECT valor FROM servidor WHERE clave = ?1", [clave], |r| r.get(0)).optional().map_err(s)
    }
    fn poner_valor(&self, clave: &str, valor: &str) -> R<()> {
        self.ctl()
            .execute("INSERT INTO servidor (clave, valor) VALUES (?1, ?2) ON CONFLICT(clave) DO UPDATE SET valor = excluded.valor", [clave, valor])
            .map_err(s)?;
        Ok(())
    }
    fn sumar_valor(&self, clave: &str, delta: i64) -> R<i64> {
        self.ctl()
            .query_row(
                "INSERT INTO servidor (clave, valor) VALUES (?1, CAST(?2 AS TEXT)) \
                 ON CONFLICT(clave) DO UPDATE SET valor = CAST(CAST(valor AS INTEGER) + ?2 AS TEXT) RETURNING CAST(valor AS INTEGER)",
                params![clave, delta],
                |r| r.get(0),
            )
            .map_err(s)
    }
    fn auditar_servidor(&self, actor: &str, accion: &str, objetivo: &str, datos: &str) -> R<()> {
        auditar_en(&self.ctl(), actor, accion, objetivo, datos)
    }

    // ---------- Cuentas y sesiones ----------
    fn contar_cuentas(&self) -> R<i64> {
        self.ctl().query_row("SELECT COUNT(*) FROM cuentas", [], |r| r.get(0)).map_err(s)
    }
    fn crear_cuenta(&self, correo: &str, nombre: &str, hash: &str, superusuario: bool) -> R<Cuenta> {
        let id = nuevo_id();
        self.ctl()
            .execute(
                "INSERT INTO cuentas (id, correo, nombre, hash, superusuario, creada) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, correo, nombre, hash, superusuario as i64, ahora()],
            )
            .map_err(|e| if e.to_string().contains("UNIQUE") { "Ya hay una cuenta con ese correo.".to_string() } else { s(e) })?;
        Ok(Cuenta { id, correo: correo.into(), nombre: nombre.into(), hash: hash.into(), superusuario, totp_secreto: None, totp_activo: false, totp_ultimo: 0 })
    }
    fn cuenta_por_correo(&self, correo: &str) -> R<Option<Cuenta>> {
        self.ctl().query_row(&format!("SELECT {COLS_CUENTA} FROM cuentas WHERE correo = ?1"), [correo], fila_cuenta).optional().map_err(s)
    }
    fn cuenta(&self, id: &str) -> R<Option<Cuenta>> {
        self.ctl().query_row(&format!("SELECT {COLS_CUENTA} FROM cuentas WHERE id = ?1"), [id], fila_cuenta).optional().map_err(s)
    }
    fn poner_totp(&self, cuenta: &str, secreto: Option<&str>, activo: bool) -> R<()> {
        self.ctl().execute("UPDATE cuentas SET totp_secreto = ?2, totp_activo = ?3 WHERE id = ?1", params![cuenta, secreto, activo as i64]).map_err(s)?;
        Ok(())
    }
    fn poner_totp_ultimo(&self, cuenta: &str, paso: i64) -> R<()> {
        self.ctl().execute("UPDATE cuentas SET totp_ultimo = ?2 WHERE id = ?1", params![cuenta, paso]).map_err(s)?;
        Ok(())
    }
    fn gastar_totp(&self, cuenta: &str, paso: i64) -> R<bool> {
        let n = self.ctl().execute("UPDATE cuentas SET totp_ultimo = ?2 WHERE id = ?1 AND totp_ultimo < ?2", params![cuenta, paso]).map_err(s)?;
        Ok(n == 1)
    }
    fn poner_codigos_recuperacion(&self, cuenta: &str, hashes: &[String]) -> R<()> {
        let mut c = self.ctl();
        let tx = c.transaction().map_err(s)?;
        tx.execute("DELETE FROM recuperacion WHERE cuenta_id = ?1", [cuenta]).map_err(s)?;
        for h in hashes {
            tx.execute("INSERT INTO recuperacion (cuenta_id, hash) VALUES (?1, ?2)", [cuenta, h]).map_err(s)?;
        }
        tx.commit().map_err(s)
    }
    fn usar_codigo_recuperacion(&self, cuenta: &str, hash: &str) -> R<bool> {
        let n = self.ctl().execute("UPDATE recuperacion SET usado = 1 WHERE cuenta_id = ?1 AND hash = ?2 AND usado = 0", [cuenta, hash]).map_err(s)?;
        Ok(n == 1)
    }
    fn cambiar_contrasena(&self, cuenta: &str, hash: &str) -> R<()> {
        self.ctl().execute("UPDATE cuentas SET hash = ?2 WHERE id = ?1", [cuenta, hash]).map_err(s)?;
        Ok(())
    }
    fn renombrar_cuenta(&self, cuenta: &str, nombre: &str) -> R<()> {
        self.ctl().execute("UPDATE cuentas SET nombre = ?2 WHERE id = ?1", [cuenta, nombre]).map_err(s)?;
        Ok(())
    }
    fn crear_sesion(&self, token_hash: &str, cuenta: &str, aal: u8, expira: Ts) -> R<()> {
        self.ctl()
            .execute(
                "INSERT INTO sesiones (token_hash, cuenta_id, aal, expira, creada) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![token_hash, cuenta, aal, expira, ahora()],
            )
            .map_err(s)?;
        Ok(())
    }
    fn sesion(&self, token_hash: &str) -> R<Option<Sesion>> {
        self.ctl()
            .query_row("SELECT cuenta_id, aal, expira FROM sesiones WHERE token_hash = ?1 AND expira > ?2", params![token_hash, ahora()], |r| {
                Ok(Sesion { cuenta_id: r.get(0)?, aal: r.get(1)?, expira: r.get(2)? })
            })
            .optional()
            .map_err(s)
    }
    fn elevar_sesion(&self, token_hash: &str, expira: Ts) -> R<()> {
        self.ctl().execute("UPDATE sesiones SET aal = 2, expira = ?2 WHERE token_hash = ?1", params![token_hash, expira]).map_err(s)?;
        Ok(())
    }
    fn borrar_sesion(&self, token_hash: &str) -> R<()> {
        self.ctl().execute("DELETE FROM sesiones WHERE token_hash = ?1", [token_hash]).map_err(s)?;
        Ok(())
    }
    fn borrar_sesiones_de(&self, cuenta: &str, salvo: Option<&str>) -> R<()> {
        self.ctl().execute("DELETE FROM sesiones WHERE cuenta_id = ?1 AND token_hash IS NOT ?2", params![cuenta, salvo]).map_err(s)?;
        Ok(())
    }

    // ---------- Clientes y pertenencias ----------
    fn crear_cliente(&self, nombre: &str, sal: &str, espera_min_horas: i64) -> R<Cliente> {
        let id = nuevo_id();
        self.ctl()
            .execute(
                "INSERT INTO clientes (id, nombre, sal, espera_min_horas, creado) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, nombre, sal, espera_min_horas, ahora()],
            )
            .map_err(s)?;
        Ok(Cliente { id, nombre: nombre.into(), sal_cliente: sal.into(), espera_min_horas })
    }
    fn cliente(&self, id: &str) -> R<Option<Cliente>> {
        self.ctl().query_row("SELECT id, nombre, sal, espera_min_horas FROM clientes WHERE id = ?1", [id], fila_cliente).optional().map_err(s)
    }
    fn renombrar_cliente(&self, id: &str, nombre: &str) -> R<()> {
        self.ctl().execute("UPDATE clientes SET nombre = ?2 WHERE id = ?1", [id, nombre]).map_err(s)?;
        Ok(())
    }
    fn poner_espera_cliente(&self, id: &str, horas: i64) -> R<()> {
        self.ctl().execute("UPDATE clientes SET espera_min_horas = ?2 WHERE id = ?1", params![id, horas]).map_err(s)?;
        Ok(())
    }
    fn clientes_de(&self, cuenta: &str) -> R<Vec<(Cliente, Rol)>> {
        let c = self.ctl();
        let mut st = c
            .prepare("SELECT c.id, c.nombre, c.sal, c.espera_min_horas, p.rol FROM clientes c JOIN pertenencias p ON p.cliente_id = c.id WHERE p.cuenta_id = ?1 ORDER BY c.nombre COLLATE NOCASE")
            .map_err(s)?;
        let filas = st.query_map([cuenta], |r| Ok((fila_cliente(r)?, r.get::<_, String>(4)?))).map_err(s)?;
        let mut out = Vec::new();
        for f in filas {
            let (cl, rol) = f.map_err(s)?;
            if let Some(rol) = Rol::de(&rol) {
                out.push((cl, rol));
            }
        }
        Ok(out)
    }
    fn rol(&self, cuenta: &str, cliente: &str) -> R<Option<Rol>> {
        let r: Option<String> = self
            .ctl()
            .query_row("SELECT rol FROM pertenencias WHERE cuenta_id = ?1 AND cliente_id = ?2", [cuenta, cliente], |r| r.get(0))
            .optional()
            .map_err(s)?;
        Ok(r.and_then(|r| Rol::de(&r)))
    }
    fn poner_rol(&self, cuenta: &str, cliente: &str, rol: Rol) -> R<()> {
        self.ctl()
            .execute(
                "INSERT INTO pertenencias (cuenta_id, cliente_id, rol) VALUES (?1, ?2, ?3) ON CONFLICT(cuenta_id, cliente_id) DO UPDATE SET rol = excluded.rol",
                [cuenta, cliente, rol.texto()],
            )
            .map_err(s)?;
        Ok(())
    }
    fn quitar_miembro(&self, cuenta: &str, cliente: &str) -> R<()> {
        self.ctl().execute("DELETE FROM pertenencias WHERE cuenta_id = ?1 AND cliente_id = ?2", [cuenta, cliente]).map_err(s)?;
        Ok(())
    }
    fn miembros(&self, cliente: &str) -> R<Vec<Miembro>> {
        let c = self.ctl();
        let mut st = c
            .prepare("SELECT a.id, a.correo, a.nombre, p.rol FROM pertenencias p JOIN cuentas a ON a.id = p.cuenta_id WHERE p.cliente_id = ?1 ORDER BY a.nombre COLLATE NOCASE")
            .map_err(s)?;
        let filas =
            st.query_map([cliente], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?))).map_err(s)?;
        let mut out = Vec::new();
        for f in filas {
            let (cuenta, correo, nombre, rol) = f.map_err(s)?;
            if let Some(rol) = Rol::de(&rol) {
                out.push(Miembro { cuenta, correo, nombre, rol });
            }
        }
        Ok(out)
    }
    fn crear_invitacion(&self, token_hash: &str, cliente: &str, rol: Rol, caduca: Ts, por: &str) -> R<()> {
        self.ctl()
            .execute(
                "INSERT INTO invitaciones (token_hash, cliente_id, rol, caduca, creada_por) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![token_hash, cliente, rol.texto(), caduca, por],
            )
            .map_err(s)?;
        Ok(())
    }
    fn tomar_invitacion(&self, token_hash: &str) -> R<Option<(String, Rol)>> {
        let mut c = self.ctl();
        let tx = c.transaction().map_err(s)?;
        let fila: Option<(String, String)> = tx
            .query_row("SELECT cliente_id, rol FROM invitaciones WHERE token_hash = ?1 AND caduca > ?2", params![token_hash, ahora()], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()
            .map_err(s)?;
        tx.execute("DELETE FROM invitaciones WHERE token_hash = ?1", [token_hash]).map_err(s)?;
        tx.commit().map_err(s)?;
        Ok(fila.and_then(|(c, r)| Rol::de(&r).map(|r| (c, r))))
    }

    // ---------- Índices globales ----------
    fn indexar_codigo(&self, codigo_hash: &str, cliente: &str, emparejamiento: &str, caduca: Ts) -> R<()> {
        self.ctl()
            .execute(
                "INSERT INTO codigos (codigo_hash, cliente_id, emparejamiento_id, caduca) VALUES (?1, ?2, ?3, ?4)",
                params![codigo_hash, cliente, emparejamiento, caduca],
            )
            .map_err(s)?;
        Ok(())
    }
    fn tomar_codigo(&self, codigo_hash: &str) -> R<Option<(String, String)>> {
        let mut c = self.ctl();
        let tx = c.transaction().map_err(s)?;
        let fila: Option<(String, String)> = tx
            .query_row("SELECT cliente_id, emparejamiento_id FROM codigos WHERE codigo_hash = ?1 AND caduca > ?2", params![codigo_hash, ahora()], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()
            .map_err(s)?;
        tx.execute("DELETE FROM codigos WHERE codigo_hash = ?1", [codigo_hash]).map_err(s)?;
        tx.commit().map_err(s)?;
        Ok(fila)
    }
    fn codigo_indexado(&self, codigo_hash: &str) -> R<bool> {
        // Bloque 7: tampoco puede pisar un código para varios equipos (ni al revés).
        self.ctl()
            .query_row(
                "SELECT 1 FROM codigos WHERE codigo_hash = ?1 UNION ALL SELECT 1 FROM codigos_varios WHERE codigo_hash = ?1 LIMIT 1",
                [codigo_hash],
                |_| Ok(()),
            )
            .optional()
            .map_err(s)
            .map(|x| x.is_some())
    }
    fn indexar_codigo_varios(&self, codigo_hash: &str, cliente: &str, lote: &str, caduca: Ts) -> R<()> {
        self.ctl()
            .execute(
                "INSERT INTO codigos_varios (codigo_hash, lote_id, cliente_id, caduca) VALUES (?1, ?2, ?3, ?4)",
                params![codigo_hash, lote, cliente, caduca],
            )
            .map_err(s)?;
        Ok(())
    }
    fn codigo_varios(&self, codigo_hash: &str) -> R<Option<(String, String)>> {
        let fila: Option<(String, String, String)> = self
            .ctl()
            .query_row("SELECT codigo_hash, cliente_id, lote_id FROM codigos_varios WHERE codigo_hash = ?1", [codigo_hash], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .optional()
            .map_err(s)?;
        // La búsqueda va por el hash (un SHA-256 de un código de ≈ 79 bits: lo que tarde no dice
        // nada del código); además se vuelve a comparar entero en tiempo constante.
        Ok(fila.filter(|(h, _, _)| bool::from(subtle::ConstantTimeEq::ct_eq(h.as_bytes(), codigo_hash.as_bytes()))).map(|(_, c, l)| (c, l)))
    }
    fn cliente_de_lote(&self, lote: &str) -> R<Option<String>> {
        self.ctl().query_row("SELECT cliente_id FROM codigos_varios WHERE lote_id = ?1", [lote], |r| r.get(0)).optional().map_err(s)
    }
    fn indexar_equipo(&self, equipo: &str, cliente: &str) -> R<()> {
        self.ctl().execute("INSERT INTO indice_equipos (equipo_id, cliente_id) VALUES (?1, ?2)", [equipo, cliente]).map_err(s)?;
        Ok(())
    }
    fn cliente_de_equipo(&self, equipo: &str) -> R<Option<ClienteCtx>> {
        let c: Option<String> =
            self.ctl().query_row("SELECT cliente_id FROM indice_equipos WHERE equipo_id = ?1", [equipo], |r| r.get(0)).optional().map_err(s)?;
        Ok(c.map(|c| ClienteCtx::autorizado(&c)))
    }
    fn desindexar_equipo(&self, equipo: &str) -> R<()> {
        self.ctl().execute("DELETE FROM indice_equipos WHERE equipo_id = ?1", [equipo]).map_err(s)?;
        Ok(())
    }
    fn crear_ficha(&self, hash: &str, cliente: &str, usos: i64, caduca: Ts) -> R<()> {
        self.ctl().execute("INSERT INTO fichas (hash, cliente_id, usos, caduca) VALUES (?1, ?2, ?3, ?4)", params![hash, cliente, usos, caduca]).map_err(s)?;
        Ok(())
    }
    fn usar_ficha(&self, hash: &str, ahora: Ts) -> R<Option<String>> {
        let c = self.ctl();
        let n = c.execute("UPDATE fichas SET usos = usos - 1 WHERE hash = ?1 AND usos > 0 AND caduca > ?2", params![hash, ahora]).map_err(s)?;
        if n == 0 {
            return Ok(None);
        }
        c.query_row("SELECT cliente_id FROM fichas WHERE hash = ?1", [hash], |r| r.get(0)).optional().map_err(s)
    }

    // ---------- Equipos y emparejamientos ----------
    fn crear_emparejamiento(&self, c: &ClienteCtx, id: &str, por: &str, caduca: Ts, codigo: &str) -> R<()> {
        self.con(c, |db| {
            db.execute(
                "INSERT INTO emparejamientos (id, estado, caduca, creado_por, creado, codigo) VALUES (?1, 'abierto', ?2, ?3, ?4, ?5)",
                params![id, caduca, por, ahora(), codigo],
            )
            .map_err(s)?;
            Ok(())
        })
    }
    fn preparar_emparejamiento(&self, c: &ClienteCtx, id: &str, por: &str, caduca: Ts, nombre: &str, so: &str, codigo: &str) -> R<()> {
        self.con(c, |db| {
            db.execute(
                "INSERT INTO emparejamientos (id, estado, caduca, creado_por, creado, nombre, so, codigo) VALUES (?1, 'abierto', ?2, ?3, ?4, ?5, ?6, ?7)",
                params![id, caduca, por, ahora(), nombre, so, codigo],
            )
            .map_err(s)?;
            Ok(())
        })
    }
    fn emparejamiento(&self, c: &ClienteCtx, id: &str) -> R<Option<Emparejamiento>> {
        self.con(c, |db| db.query_row(&format!("SELECT {COLS_EMP} FROM emparejamientos WHERE id = ?1"), [id], fila_emparejamiento).optional().map_err(s))
    }
    fn emparejamientos_preparados(&self, c: &ClienteCtx, ahora: Ts) -> R<Vec<Emparejamiento>> {
        self.con(c, |db| {
            let mut st = db
                .prepare(&format!(
                    "SELECT {COLS_EMP} FROM emparejamientos WHERE nombre IS NOT NULL AND estado IN ('abierto', 'unido') AND caduca > ?1 ORDER BY creado DESC"
                ))
                .map_err(s)?;
            let filas = st.query_map([ahora], fila_emparejamiento).map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn emparejamientos_vigentes_de(&self, c: &ClienteCtx, por: &str, ahora: Ts) -> R<Vec<Emparejamiento>> {
        self.con(c, |db| {
            let mut st = db
                .prepare(&format!(
                    "SELECT {COLS_EMP} FROM emparejamientos WHERE creado_por = ?1 AND codigo IS NOT NULL AND lote IS NULL AND estado IN ('abierto', 'unido') AND caduca > ?2 ORDER BY creado DESC"
                ))
                .map_err(s)?;
            let filas = st.query_map(params![por, ahora], fila_emparejamiento).map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn a_medias(&self, c: &ClienteCtx, ahora: Ts) -> R<Vec<Emparejamiento>> {
        self.con(c, |db| {
            let mut st = db
                .prepare(&format!(
                    "SELECT {COLS_EMP} FROM emparejamientos WHERE equipo_id IS NOT NULL AND \
                     ((estado = 'unido' AND caduca > ?1) OR (estado = 'confirmado' AND codigo IS NOT NULL)) ORDER BY creado DESC"
                ))
                .map_err(s)?;
            let filas = st.query_map([ahora], fila_emparejamiento).map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn anular_emparejamientos_equipo(&self, c: &ClienteCtx, equipo: &str) -> R<()> {
        self.con(c, |db| {
            db.execute(
                "UPDATE emparejamientos SET estado = 'cancelado', codigo = NULL WHERE equipo_id = ?1 AND estado IN ('abierto', 'unido', 'confirmado')",
                [equipo],
            )
            .map_err(s)?;
            Ok(())
        })
    }
    fn alta_hecha(&self, c: &ClienteCtx, equipo: &str) -> R<()> {
        self.con(c, |db| {
            db.execute("UPDATE emparejamientos SET codigo = NULL WHERE equipo_id = ?1 AND estado = 'confirmado'", [equipo]).map_err(s)?;
            Ok(())
        })
    }
    fn poner_sas_emparejamiento(&self, c: &ClienteCtx, id: &str, version: i64) -> R<()> {
        self.con(c, |db| {
            db.execute("UPDATE emparejamientos SET sas_version = ?2 WHERE id = ?1", params![id, version]).map_err(s)?;
            Ok(())
        })
    }
    fn crear_lote(&self, c: &ClienteCtx, l: &Lote) -> R<()> {
        self.con(c, |db| {
            db.execute(
                "INSERT INTO lotes (id, codigo_hash, nombre, usos, usados, caduca, creado, creado_por, anulado, rechazos) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![l.id, l.codigo_hash, l.nombre, l.usos, l.usados, l.caduca, l.creado, l.creado_por, l.anulado, l.rechazos],
            )
            .map_err(s)?;
            Ok(())
        })
    }
    fn lote(&self, c: &ClienteCtx, id: &str) -> R<Option<Lote>> {
        self.con(c, |db| db.query_row(&format!("SELECT {COLS_LOTE} FROM lotes WHERE id = ?1"), [id], fila_lote).optional().map_err(s))
    }
    fn lotes(&self, c: &ClienteCtx) -> R<Vec<Lote>> {
        self.con(c, |db| {
            let mut st = db.prepare(&format!("SELECT {COLS_LOTE} FROM lotes ORDER BY creado DESC, id LIMIT 50")).map_err(s)?;
            let filas = st.query_map([], fila_lote).map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn usar_lote(&self, c: &ClienteCtx, id: &str, ahora: Ts) -> R<bool> {
        self.con(c, |db| {
            // Una sola sentencia: dos equipos a la vez no pueden gastar el último uso los dos.
            let n = db
                .execute("UPDATE lotes SET usados = usados + 1 WHERE id = ?1 AND anulado IS NULL AND caduca > ?2 AND usados < usos", params![id, ahora])
                .map_err(s)?;
            Ok(n == 1)
        })
    }
    fn rechazo_lote(&self, c: &ClienteCtx, id: &str) -> R<()> {
        self.con(c, |db| {
            db.execute("UPDATE lotes SET rechazos = rechazos + 1 WHERE id = ?1", [id]).map_err(s)?;
            Ok(())
        })
    }
    fn anular_lote(&self, c: &ClienteCtx, id: &str, ahora: Ts) -> R<bool> {
        self.con(c, |db| {
            let n = db.execute("UPDATE lotes SET anulado = ?2 WHERE id = ?1 AND anulado IS NULL", params![id, ahora]).map_err(s)?;
            Ok(n == 1)
        })
    }
    fn emparejamiento_de_lote(&self, c: &ClienteCtx, id: &str, lote: &str, por: &str, caduca: Ts, codigo: &str, equipo: &str, ip: Option<&str>) -> R<()> {
        self.con(c, |db| {
            db.execute(
                "INSERT INTO emparejamientos (id, estado, caduca, creado_por, equipo_id, creado, codigo, lote, ip) VALUES (?1, 'unido', ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![id, caduca, por, equipo, ahora(), codigo, lote, ip],
            )
            .map_err(s)?;
            Ok(())
        })
    }
    fn emparejamientos_de_lote(&self, c: &ClienteCtx, lote: &str) -> R<Vec<Emparejamiento>> {
        self.con(c, |db| {
            let mut st = db.prepare(&format!("SELECT {COLS_EMP} FROM emparejamientos WHERE lote = ?1 ORDER BY creado DESC, id")).map_err(s)?;
            let filas = st.query_map([lote], fila_emparejamiento).map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn poner_estado_emparejamiento(&self, c: &ClienteCtx, id: &str, estado: &str, equipo: Option<&str>) -> R<()> {
        self.con(c, |db| {
            // El código se guarda mientras hace falta: abierto, unido y confirmado hasta que el
            // equipo hace el alta (`alta_hecha`; la consola lo necesita para mandarla). Al unirse,
            // hay al menos `PLAZO_UNIDO_S` para comparar el número y dar de alta (un equipo que
            // se unió no se queda a medias porque el código era de 15 o 30 min).
            db.execute(
                "UPDATE emparejamientos SET estado = ?2, equipo_id = COALESCE(?3, equipo_id), \
                 codigo = CASE WHEN ?2 IN ('abierto', 'unido', 'confirmado') THEN codigo ELSE NULL END, \
                 caduca = CASE WHEN ?2 = 'unido' THEN MAX(caduca, ?4) ELSE caduca END WHERE id = ?1",
                params![id, estado, equipo, ahora() + super::PLAZO_UNIDO_S],
            )
            .map_err(s)?;
            Ok(())
        })
    }
    fn crear_equipo(&self, c: &ClienteCtx, e: &EquipoNuevo) -> R<()> {
        self.con(c, |db| {
            db.execute(
                "INSERT INTO equipos (id, nombre, so, version_agente, box_pub, sign_pub, sal_equipo, secreto_hash, creado) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![e.id, e.nombre, e.so, e.version, e.box_pub, e.sign_pub, e.sal_equipo, e.secreto_hash, ahora()],
            )
            .map_err(s)?;
            Ok(())
        })
    }
    fn equipos(&self, c: &ClienteCtx) -> R<Vec<Equipo>> {
        self.con(c, |db| {
            let mut st = db
                .prepare(&format!("SELECT {COLS_EQUIPO} FROM equipos e LEFT JOIN configs c ON c.equipo_id = e.id ORDER BY e.nombre COLLATE NOCASE"))
                .map_err(s)?;
            let filas = st.query_map([], fila_equipo).map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn equipo(&self, c: &ClienteCtx, id: &str) -> R<Option<Equipo>> {
        self.con(c, |db| {
            db.query_row(&format!("SELECT {COLS_EQUIPO} FROM equipos e LEFT JOIN configs c ON c.equipo_id = e.id WHERE e.id = ?1"), [id], fila_equipo)
                .optional()
                .map_err(s)
        })
    }
    fn secreto_equipo(&self, c: &ClienteCtx, id: &str) -> R<Option<String>> {
        self.con(c, |db| db.query_row("SELECT secreto_hash FROM equipos WHERE id = ?1", [id], |r| r.get(0)).optional().map_err(s))
    }
    fn renombrar_equipo(&self, c: &ClienteCtx, id: &str, nombre: &str) -> R<()> {
        self.con(c, |db| {
            db.execute("UPDATE equipos SET nombre = ?2 WHERE id = ?1", [id, nombre]).map_err(s)?;
            Ok(())
        })
    }
    fn poner_etiquetas_equipo(&self, c: &ClienteCtx, id: &str, etiquetas: &[String]) -> R<()> {
        let t = serde_json::to_string(etiquetas).map_err(s)?;
        self.con(c, |db| {
            db.execute("UPDATE equipos SET etiquetas = ?2 WHERE id = ?1", [id, t.as_str()]).map_err(s)?;
            Ok(())
        })
    }
    fn confirmar_equipo(&self, c: &ClienteCtx, id: &str, etiqueta: &str) -> R<()> {
        self.con(c, |db| {
            db.execute("UPDATE equipos SET confirmado = 1, etiqueta = ?2 WHERE id = ?1", [id, etiqueta]).map_err(s)?;
            Ok(())
        })
    }
    fn borrar_equipo(&self, c: &ClienteCtx, id: &str) -> R<()> {
        self.con(c, |db| {
            db.execute("DELETE FROM equipos WHERE id = ?1", [id]).map_err(s)?;
            db.execute("DELETE FROM configs WHERE equipo_id = ?1", [id]).map_err(s)?;
            db.execute("DELETE FROM historial WHERE equipo_id = ?1", [id]).map_err(s)?;
            Ok(())
        })?;
        super::notas::AlmacenNotas::borrar_notas_equipo(self, c, id)
    }
    fn contacto_equipo(&self, c: &ClienteCtx, id: &str, cuando: Ts) -> R<()> {
        self.con(c, |db| {
            db.execute("UPDATE equipos SET ultimo_contacto = ?2 WHERE id = ?1", params![id, cuando]).map_err(s)?;
            Ok(())
        })
    }
    fn poner_estado_servicio(&self, c: &ClienteCtx, id: &str, estado: Option<&str>, version: Option<&str>) -> R<()> {
        self.con(c, |db| {
            db.execute("UPDATE equipos SET estado_servicio = ?2, version_agente = COALESCE(?3, version_agente) WHERE id = ?1", params![id, estado, version])
                .map_err(s)?;
            Ok(())
        })
    }
    fn reponer_no_recibidas(&self, c: &ClienteCtx, equipo: &str, ultimo_seq: u64, ahora: Ts) -> R<usize> {
        let ultimo = i64::try_from(ultimo_seq).unwrap_or(i64::MAX);
        self.con(c, |db| {
            db.execute(
                "UPDATE ordenes SET estado = 'pendiente', actualizada = ?3 WHERE equipo_id = ?1 AND estado = 'entregada' AND seq > ?2 AND caduca > ?3",
                params![equipo, ultimo, ahora],
            )
            .map_err(s)
        })
    }
    fn adelantar_seq(&self, c: &ClienteCtx, id: &str, minimo: u64) -> R<()> {
        // v1.58: sin caer en el número de una orden con espera reservada.
        self.con(c, |db| poner_siguiente(db, id, minimo))
    }
    fn poner_modo(&self, c: &ClienteCtx, id: &str, modo: &str) -> R<()> {
        self.con(c, |db| {
            db.execute("UPDATE equipos SET modo = ?2 WHERE id = ?1", [id, modo]).map_err(s)?;
            Ok(())
        })
    }
    fn poner_papel(&self, c: &ClienteCtx, id: &str, papel: &str) -> R<()> {
        self.con(c, |db| {
            db.execute("UPDATE equipos SET rol = ?2 WHERE id = ?1", [id, papel]).map_err(s)?;
            Ok(())
        })
    }
    fn reactivar_equipo(&self, c: &ClienteCtx, id: &str, secreto_hash: &str) -> R<()> {
        self.con(c, |db| {
            db.execute("UPDATE equipos SET secreto_hash = ?2, modo = 'gestionado' WHERE id = ?1", [id, secreto_hash]).map_err(s)?;
            Ok(())
        })
    }
    fn poner_espera_equipo(&self, c: &ClienteCtx, id: &str, horas: i64) -> R<()> {
        self.con(c, |db| {
            db.execute("UPDATE equipos SET espera_min_horas = ?2 WHERE id = ?1", params![id, horas]).map_err(s)?;
            Ok(())
        })
    }
    fn poner_atencion(&self, c: &ClienteCtx, id: &str, hasta: Ts) -> R<()> {
        self.con(c, |db| {
            db.execute("UPDATE equipos SET atencion_hasta = ?2 WHERE id = ?1", params![id, hasta]).map_err(s)?;
            Ok(())
        })
    }

    // ---------- Configuración e informes ----------
    fn guardar_config(&self, c: &ClienteCtx, equipo: &str, seq: u64, cifrado: &str, resumen: &serde_json::Value) -> R<()> {
        self.con(c, |db| {
            db.execute(
                "INSERT INTO configs (equipo_id, seq, cifrado, resumen, actualizada) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(equipo_id) DO UPDATE SET seq = excluded.seq, cifrado = excluded.cifrado, resumen = excluded.resumen, actualizada = excluded.actualizada",
                params![equipo, seq as i64, cifrado, resumen.to_string(), ahora()],
            )
            .map_err(s)?;
            Ok(())
        })
    }
    fn config(&self, c: &ClienteCtx, equipo: &str) -> R<Option<(u64, String, serde_json::Value)>> {
        self.con(c, |db| {
            let fila: Option<(i64, String, String)> = db
                .query_row("SELECT seq, cifrado, resumen FROM configs WHERE equipo_id = ?1", [equipo], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .optional()
                .map_err(s)?;
            Ok(fila.map(|(seq, cifrado, resumen)| (seq as u64, cifrado, serde_json::from_str(&resumen).unwrap_or_default())))
        })
    }
    fn guardar_informe(&self, c: &ClienteCtx, equipo: &str, datos: &serde_json::Value) -> R<()> {
        self.con(c, |db| {
            db.execute("INSERT INTO informes (equipo_id, recibido, datos) VALUES (?1, ?2, ?3)", params![equipo, ahora(), datos.to_string()]).map_err(s)?;
            // Como mucho los últimos 1000 de cada equipo (unos días): el disco no crece sin límite.
            db.execute(
                "DELETE FROM informes WHERE equipo_id = ?1 AND id NOT IN (SELECT id FROM informes WHERE equipo_id = ?1 ORDER BY id DESC LIMIT 1000)",
                params![equipo],
            )
            .map_err(s)?;
            Ok(())
        })
    }
    fn informes(&self, c: &ClienteCtx, equipo: &str, limite: i64) -> R<Vec<(Ts, serde_json::Value)>> {
        self.con(c, |db| {
            let mut st = db.prepare("SELECT recibido, datos FROM informes WHERE equipo_id = ?1 ORDER BY id DESC LIMIT ?2").map_err(s)?;
            let filas = st.query_map(params![equipo, limite], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))).map_err(s)?;
            let mut out = Vec::new();
            for f in filas {
                let (t, d) = f.map_err(s)?;
                out.push((t, serde_json::from_str(&d).unwrap_or_default()));
            }
            Ok(out)
        })
    }

    // ---------- Órdenes ----------
    fn insertar_orden(&self, c: &ClienteCtx, o: &OrdenNueva) -> R<Orden> {
        let con = self.conexion(c)?;
        let mut db = con.lock().unwrap_or_else(|e| e.into_inner());
        let tx = db.transaction().map_err(s)?;
        let (siguiente, espera) = numeros_equipo(&tx, &o.equipo_id)?.ok_or("Equipo no encontrado.")?;
        let ahora = ahora();
        // v1.58: una orden con espera puede llevar el número reservado (`seq_espera`), por
        // encima de las que se manden mientras espera; las demás, el siguiente.
        let reservada = o.seq == espera && o.not_before.is_some_and(|nb| nb > ahora);
        if o.seq != siguiente && !reservada {
            return Err(format!("seq:{siguiente}:{espera}"));
        }
        tx.execute(
            "INSERT INTO ordenes (id, equipo_id, tipo, seq, sellado, emitida, emitida_por, not_before, caduca, estado, actualizada, sesion, relevo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'pendiente', ?6, ?10, ?11)",
            params![o.id, o.equipo_id, o.tipo, o.seq as i64, o.sellado, ahora, o.emitida_por, o.not_before, o.caduca, o.sesion, o.relevo],
        )
        .map_err(s)?;
        // La reservada no mueve el siguiente (solo se deja al día si saltó por encima de
        // órdenes con espera que ya terminaron).
        poner_siguiente(&tx, &o.equipo_id, if reservada { siguiente } else { siguiente + 1 })?;
        let orden = tx.query_row(&format!("SELECT {COLS_ORDEN} FROM ordenes WHERE id = ?1"), [&o.id], fila_orden).map_err(s)?;
        tx.commit().map_err(s)?;
        Ok(orden)
    }
    fn orden(&self, c: &ClienteCtx, id: &str) -> R<Option<Orden>> {
        self.con(c, |db| db.query_row(&format!("SELECT {COLS_ORDEN} FROM ordenes WHERE id = ?1"), [id], fila_orden).optional().map_err(s))
    }
    fn ordenes_equipo(&self, c: &ClienteCtx, equipo: &str, limite: i64) -> R<Vec<Orden>> {
        self.con(c, |db| {
            let mut st = db.prepare(&format!("SELECT {COLS_ORDEN} FROM ordenes WHERE equipo_id = ?1 ORDER BY seq DESC LIMIT ?2")).map_err(s)?;
            let filas = st.query_map(params![equipo, limite], fila_orden).map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn ordenes_cliente(&self, c: &ClienteCtx, equipo: Option<&str>, estado: Option<&str>, antes: Option<(Ts, String)>, limite: i64) -> R<Vec<Orden>> {
        self.con(c, |db| {
            let (antes_ts, antes_id) = antes.unwrap_or((i64::MAX, String::new()));
            let mut st = db
                .prepare(&format!(
                    "SELECT {COLS_ORDEN} FROM ordenes
                     WHERE (?1 IS NULL OR equipo_id = ?1) AND (?2 IS NULL OR estado = ?2)
                       AND (emitida < ?3 OR (emitida = ?3 AND id < ?4))
                     ORDER BY emitida DESC, id DESC LIMIT ?5"
                ))
                .map_err(s)?;
            let filas = st
                .query_map(params![equipo, estado, antes_ts, if antes_id.is_empty() { "\u{10FFFF}".to_string() } else { antes_id }, limite], fila_orden)
                .map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn ordenes_con_espera(&self, c: &ClienteCtx, ahora: Ts) -> R<Vec<Orden>> {
        self.con(c, |db| {
            let mut st = db
                .prepare(&format!(
                    // v1.49: también las entregadas antes de su hora (el equipo las tiene en espera).
                    "SELECT {COLS_ORDEN} FROM ordenes WHERE estado IN ('pendiente', 'entregada') AND not_before IS NOT NULL AND not_before > ?1 ORDER BY not_before"
                ))
                .map_err(s)?;
            let filas = st.query_map([ahora], fila_orden).map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn entregar_ordenes(&self, c: &ClienteCtx, equipo: &str, ahora: Ts, adelantar: bool) -> R<Vec<Orden>> {
        let con = self.conexion(c)?;
        let mut db = con.lock().unwrap_or_else(|e| e.into_inner());
        let tx = db.transaction().map_err(s)?;
        // Las caducadas sin entregar, se marcan; y las entregadas sin respuesta del equipo
        // pasada su caducidad (el equipo ya no las aceptaría: si no contestó, no va a hacerlo).
        caducar(&tx, Some(equipo), ahora)?;
        let ordenes = {
            let mut st = tx
                .prepare(&format!("SELECT {COLS_ORDEN}, entregar_desde FROM ordenes WHERE equipo_id = ?1 AND estado = 'pendiente' ORDER BY seq"))
                .map_err(s)?;
            let filas = st.query_map(params![equipo], |r| Ok((fila_orden(r)?, r.get::<_, Option<Ts>>(15)?))).map_err(s)?;
            let todas = filas.collect::<Result<Vec<_>, _>>().map_err(s)?;
            // Las que ya tocan; con `adelantar`, también las que esperan su hora y piden
            // autorización (las inofensivas con espera, como siempre, a su hora).
            let toca = |o: &Orden, desde: Option<Ts>| {
                desde.is_none_or(|d| d <= ahora) && (o.not_before.is_none_or(|nb| nb <= ahora) || (adelantar && adelantable(&o.tipo)))
            };
            if adelantar {
                todas.into_iter().filter(|(o, d)| toca(o, *d)).map(|(o, _)| o).collect::<Vec<_>>()
            } else {
                // v1.58: a un agente que no guarda las órdenes con espera, en orden y sin saltarse
                // ninguna: la primera que aún no toca retiene a las siguientes. Si no, recibiría
                // antes una orden posterior y, al llegar la hora de la que esperaba, la rechazaría
                // por «antigua» (número menor que el último que aceptó). Al que las guarda se le
                // dan al momento, así que eso no le pasa (como en v1.49).
                todas.into_iter().take_while(|(o, d)| toca(o, *d)).map(|(o, _)| o).collect::<Vec<_>>()
            }
        };
        for o in &ordenes {
            tx.execute("UPDATE ordenes SET estado = 'entregada', actualizada = ?2, entregar_desde = NULL WHERE id = ?1", params![o.id, ahora]).map_err(s)?;
        }
        // Una con el número reservado ya está en el equipo: las siguientes, por encima de ella.
        if let Some(max) = ordenes.iter().map(|o| o.seq).max() {
            poner_siguiente(&tx, equipo, max + 1)?;
        }
        tx.commit().map_err(s)?;
        Ok(ordenes.into_iter().map(|o| Orden { estado: "entregada".into(), ..o }).collect())
    }
    fn canceladas_sin_avisar(&self, c: &ClienteCtx, equipo: &str) -> R<Vec<String>> {
        self.con(c, |db| {
            let ids = {
                let mut st = db.prepare("SELECT id FROM ordenes WHERE equipo_id = ?1 AND estado = 'cancelada' AND cancelacion_avisada = 0").map_err(s)?;
                let filas = st.query_map([equipo], |r| r.get::<_, String>(0)).map_err(s)?;
                filas.collect::<Result<Vec<_>, _>>().map_err(s)?
            };
            db.execute("UPDATE ordenes SET cancelacion_avisada = 1 WHERE equipo_id = ?1 AND estado = 'cancelada'", [equipo]).map_err(s)?;
            Ok(ids)
        })
    }
    fn resultado_orden(&self, c: &ClienteCtx, equipo: &str, r: &ResultadoOrden) -> R<bool> {
        self.con(c, |db| {
            let n = db
                .execute(
                    "UPDATE ordenes SET estado = ?3, mensaje = ?4, detalle = ?5, firma_agente = ?6, actualizada = ?7
                     WHERE id = ?1 AND equipo_id = ?2 AND (estado IN ('pendiente', 'entregada', 'en_marcha') OR (?8 AND estado = 'cancelada'))",
                    params![r.orden, equipo, r.estado, r.mensaje, r.detalle, r.firma, ahora(), r.pisar_cancelada],
                )
                .map_err(s)?;
            Ok(n == 1)
        })
    }
    fn cancelar_orden(&self, c: &ClienteCtx, id: &str, _por: &str, ahora: Ts) -> R<bool> {
        self.con(c, |db| {
            // Pendiente (no entregada), o entregada pero todavía esperando su not_before.
            let n = db
                .execute(
                    "UPDATE ordenes SET estado = 'cancelada', actualizada = ?2,
                       cancelacion_avisada = CASE WHEN estado = 'entregada' THEN 0 ELSE 1 END
                     WHERE id = ?1 AND (estado = 'pendiente' OR (estado = 'entregada' AND not_before IS NOT NULL AND not_before > ?2))",
                    params![id, ahora],
                )
                .map_err(s)?;
            Ok(n == 1)
        })
    }

    fn reintentar_orden(&self, c: &ClienteCtx, id: &str, desde: Ts, max: i64) -> R<bool> {
        self.con(c, |db| {
            let n = db
                .execute(
                    "UPDATE ordenes SET estado = 'pendiente', entregar_desde = ?2, reintentos = reintentos + 1, actualizada = ?3
                     WHERE id = ?1 AND estado = 'entregada' AND not_before IS NOT NULL AND reintentos < ?4 AND caduca > ?2",
                    params![id, desde, ahora(), max],
                )
                .map_err(s)?;
            Ok(n == 1)
        })
    }
    fn caducadas_por_avisar(&self, c: &ClienteCtx) -> R<Vec<Orden>> {
        self.con(c, |db| {
            let ordenes = {
                let mut st = db.prepare(&format!("SELECT {COLS_ORDEN} FROM ordenes WHERE estado = 'caducada' AND fin_avisado = 0 ORDER BY seq")).map_err(s)?;
                let filas = st.query_map([], fila_orden).map_err(s)?;
                filas.collect::<Result<Vec<_>, _>>().map_err(s)?
            };
            db.execute("UPDATE ordenes SET fin_avisado = 1 WHERE estado = 'caducada' AND fin_avisado = 0", []).map_err(s)?;
            Ok(ordenes)
        })
    }

    // ---------- Avisos y auditoría ----------
    fn crear_aviso(&self, c: &ClienteCtx, equipo: Option<&str>, tipo: &str, mensaje: &str) -> R<()> {
        self.con(c, |db| {
            db.execute(
                "INSERT INTO avisos (id, equipo_id, tipo, mensaje, creado) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![nuevo_id(), equipo, tipo, mensaje, ahora()],
            )
            .map_err(s)?;
            Ok(())
        })
    }
    fn avisos(&self, c: &ClienteCtx, solo_abiertos: bool) -> R<Vec<Aviso>> {
        self.con(c, |db| {
            let sql = if solo_abiertos {
                "SELECT id, equipo_id, tipo, mensaje, creado, visto_por FROM avisos WHERE visto_por IS NULL ORDER BY creado DESC LIMIT 500"
            } else {
                "SELECT id, equipo_id, tipo, mensaje, creado, visto_por FROM avisos ORDER BY creado DESC LIMIT 500"
            };
            let mut st = db.prepare(sql).map_err(s)?;
            let filas = st
                .query_map([], |r| Ok(Aviso { id: r.get(0)?, equipo: r.get(1)?, tipo: r.get(2)?, mensaje: r.get(3)?, creado: r.get(4)?, visto_por: r.get(5)? }))
                .map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn marcar_aviso(&self, c: &ClienteCtx, id: &str, por: &str) -> R<()> {
        self.con(c, |db| {
            db.execute("UPDATE avisos SET visto_por = ?2 WHERE id = ?1", [id, por]).map_err(s)?;
            Ok(())
        })
    }
    fn auditar(&self, c: &ClienteCtx, actor: &str, accion: &str, objetivo: &str, datos: &str) -> R<()> {
        self.con(c, |db| auditar_en(db, actor, accion, objetivo, datos))
    }
    fn auditoria_desc(&self, c: &ClienteCtx, antes: Option<i64>, limite: i64) -> R<Vec<EntradaAuditoria>> {
        self.con(c, |db| {
            let mut st = db
                .prepare("SELECT n, creado, actor, accion, objetivo, datos, hash, prev_hash FROM auditoria WHERE n < ?1 ORDER BY n DESC LIMIT ?2")
                .map_err(s)?;
            let filas = st.query_map(params![antes.unwrap_or(i64::MAX), limite], fila_auditoria).map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn auditoria(&self, c: &ClienteCtx, desde: i64, limite: i64) -> R<Vec<EntradaAuditoria>> {
        self.con(c, |db| {
            let mut st =
                db.prepare("SELECT n, creado, actor, accion, objetivo, datos, hash, prev_hash FROM auditoria WHERE n > ?1 ORDER BY n LIMIT ?2").map_err(s)?;
            let filas = st
                .query_map(params![desde, limite], |r| {
                    Ok(EntradaAuditoria {
                        n: r.get(0)?,
                        creado: r.get(1)?,
                        actor: r.get(2)?,
                        accion: r.get(3)?,
                        objetivo: r.get(4)?,
                        datos: r.get(5)?,
                        hash: r.get(6)?,
                        prev_hash: r.get(7)?,
                    })
                })
                .map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn importar_auditoria(&self, c: &ClienteCtx, origen: &str, entradas: &[EntradaAuditoria]) -> R<()> {
        // La cadena de otro servidor tiene que estar entera (desde su génesis).
        let mut prev = GENESIS.to_string();
        for (i, e) in entradas.iter().enumerate() {
            let calc = hash_entrada(&prev, e.n, e.creado, &e.actor, &e.accion, &e.objetivo, &e.datos);
            if e.n != i as i64 + 1 || e.prev_hash != prev || e.hash != calc {
                return Err(format!("La auditoría importada está rota en la entrada {}.", e.n));
            }
            prev = e.hash.clone();
        }
        let con = self.conexion(c)?;
        let mut db = con.lock().unwrap_or_else(|e| e.into_inner());
        let tx = db.transaction().map_err(s)?;
        let ya: i64 = tx.query_row("SELECT COUNT(*) FROM auditoria_importada", [], |r| r.get(0)).map_err(s)?;
        if ya > 0 {
            return Err("ya_importada".into());
        }
        for e in entradas {
            tx.execute(
                "INSERT INTO auditoria_importada (n, creado, actor, accion, objetivo, datos, prev_hash, hash, origen) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![e.n, e.creado, e.actor, e.accion, e.objetivo, e.datos, e.prev_hash, e.hash, origen],
            )
            .map_err(s)?;
        }
        tx.commit().map_err(s)
    }
    fn auditoria_importada(&self, c: &ClienteCtx, desde: i64, limite: i64) -> R<Vec<EntradaAuditoria>> {
        self.con(c, |db| {
            let mut st = db
                .prepare("SELECT n, creado, actor, accion, objetivo, datos, hash, prev_hash FROM auditoria_importada WHERE n > ?1 ORDER BY n LIMIT ?2")
                .map_err(s)?;
            let filas = st.query_map(params![desde, limite], fila_auditoria).map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn importar_informe(&self, c: &ClienteCtx, equipo: &str, recibido: Ts, datos: &serde_json::Value) -> R<()> {
        self.con(c, |db| {
            db.execute("INSERT INTO informes (equipo_id, recibido, datos) VALUES (?1, ?2, ?3)", params![equipo, recibido, datos.to_string()]).map_err(s)?;
            Ok(())
        })
    }
    fn importar_aviso(&self, c: &ClienteCtx, equipo: Option<&str>, tipo: &str, mensaje: &str, creado: Ts) -> R<()> {
        self.con(c, |db| {
            db.execute(
                "INSERT INTO avisos (id, equipo_id, tipo, mensaje, creado, visto_por) VALUES (?1, ?2, ?3, ?4, ?5, 'importado')",
                params![nuevo_id(), equipo, tipo, mensaje, creado],
            )
            .map_err(s)?;
            Ok(())
        })
    }
    fn verificar_auditoria(&self, c: &ClienteCtx) -> R<(i64, Option<i64>)> {
        self.con(c, |db| {
            let mut st = db.prepare("SELECT n, creado, actor, accion, objetivo, datos, hash, prev_hash FROM auditoria ORDER BY n").map_err(s)?;
            let mut filas = st.query([]).map_err(s)?;
            let mut prev = GENESIS.to_string();
            let mut esperado = 1i64;
            let mut total = 0;
            while let Some(r) = filas.next().map_err(s)? {
                let n: i64 = r.get(0).map_err(s)?;
                let hash: String = r.get(6).map_err(s)?;
                let prev_hash: String = r.get(7).map_err(s)?;
                let calc = hash_entrada(
                    &prev,
                    n,
                    r.get(1).map_err(s)?,
                    &r.get::<_, String>(2).map_err(s)?,
                    &r.get::<_, String>(3).map_err(s)?,
                    &r.get::<_, String>(4).map_err(s)?,
                    &r.get::<_, String>(5).map_err(s)?,
                );
                if n != esperado || prev_hash != prev || calc != hash {
                    return Ok((total, Some(n)));
                }
                prev = hash;
                esperado += 1;
                total += 1;
            }
            Ok((total, None))
        })
    }

    // ---------- Sesiones interactivas ----------
    fn crear_sesion_interactiva(&self, c: &ClienteCtx, id: &str, equipo: &str, por: &str, expira: Ts) -> R<()> {
        self.con(c, |db| {
            db.execute("INSERT INTO sesiones_i (id, equipo_id, abierta_por, expira) VALUES (?1, ?2, ?3, ?4)", params![id, equipo, por, expira])
                .map_err(|e| if e.to_string().contains("UNIQUE") { "Esa sesión ya existe.".to_string() } else { s(e) })?;
            Ok(())
        })
    }
    fn sesion_interactiva(&self, c: &ClienteCtx, id: &str) -> R<Option<SesionInteractiva>> {
        self.con(c, |db| {
            db.query_row("SELECT id, equipo_id, abierta_por, expira FROM sesiones_i WHERE id = ?1 AND expira > ?2", params![id, ahora()], |r| {
                Ok(SesionInteractiva { id: r.get(0)?, equipo_id: r.get(1)?, abierta_por: r.get(2)?, expira: r.get(3)? })
            })
            .optional()
            .map_err(s)
        })
    }
    fn mensaje_sesion(&self, c: &ClienteCtx, sesion: &str, de: &str, cifrado: &str, expira: Ts) -> R<i64> {
        let con = self.conexion(c)?;
        let mut db = con.lock().unwrap_or_else(|e| e.into_inner());
        let tx = db.transaction().map_err(s)?;
        let n: i64 = tx.query_row("SELECT COALESCE(MAX(n), 0) + 1 FROM mensajes_i WHERE sesion_id = ?1", [sesion], |r| r.get(0)).map_err(s)?;
        tx.execute("INSERT INTO mensajes_i (sesion_id, n, de, cifrado, creado) VALUES (?1, ?2, ?3, ?4, ?5)", params![sesion, n, de, cifrado, ahora()])
            .map_err(s)?;
        tx.execute("UPDATE sesiones_i SET expira = MAX(expira, ?2) WHERE id = ?1", params![sesion, expira]).map_err(s)?;
        tx.commit().map_err(s)?;
        Ok(n)
    }
    fn mensajes_sesion(&self, c: &ClienteCtx, sesion: &str, para: &str, desde: i64) -> R<Vec<MensajeSesion>> {
        self.con(c, |db| {
            let mut st = db.prepare("SELECT n, de, cifrado FROM mensajes_i WHERE sesion_id = ?1 AND de != ?2 AND n > ?3 ORDER BY n LIMIT 200").map_err(s)?;
            let filas = st.query_map(params![sesion, para, desde], |r| Ok(MensajeSesion { n: r.get(0)?, de: r.get(1)?, cifrado: r.get(2)? })).map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn sesiones_abiertas(&self, c: &ClienteCtx, equipo: &str, ahora: Ts) -> R<Vec<String>> {
        self.con(c, |db| {
            let mut st = db.prepare("SELECT id FROM sesiones_i WHERE equipo_id = ?1 AND expira > ?2").map_err(s)?;
            let filas = st.query_map(params![equipo, ahora], |r| r.get::<_, String>(0)).map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn cerrar_sesion_interactiva(&self, c: &ClienteCtx, id: &str) -> R<()> {
        self.con(c, |db| {
            db.execute("DELETE FROM mensajes_i WHERE sesion_id = ?1", [id]).map_err(s)?;
            db.execute("DELETE FROM sesiones_i WHERE id = ?1", [id]).map_err(s)?;
            Ok(())
        })
    }

    // ---------- Relé ----------
    fn crear_relevo(&self, c: &ClienteCtx, id: &str, equipo: &str, max_bytes: u64, caduca: Ts) -> R<()> {
        self.con(c, |db| {
            db.execute(
                "INSERT INTO relevos (id, equipo_id, max_bytes, estado, caduca) VALUES (?1, ?2, ?3, 'subiendo', ?4)",
                params![id, equipo, max_bytes as i64, caduca],
            )
            .map_err(|e| if e.to_string().contains("UNIQUE") { "Ese relé ya existe.".to_string() } else { s(e) })?;
            Ok(())
        })
    }
    fn relevo(&self, c: &ClienteCtx, id: &str) -> R<Option<Relevo>> {
        self.con(c, |db| {
            db.query_row("SELECT id, equipo_id, max_bytes, trozos, bytes, estado, caduca FROM relevos WHERE id = ?1", [id], |r| {
                Ok(Relevo {
                    id: r.get(0)?,
                    equipo_id: r.get(1)?,
                    max_bytes: r.get::<_, i64>(2)? as u64,
                    trozos: r.get::<_, i64>(3)? as u64,
                    bytes: r.get::<_, i64>(4)? as u64,
                    estado: r.get(5)?,
                    caduca: r.get(6)?,
                })
            })
            .optional()
            .map_err(s)
        })
    }
    fn actualizar_relevo(&self, c: &ClienteCtx, id: &str, trozos: u64, bytes: u64, estado: &str, caduca: Ts) -> R<()> {
        self.con(c, |db| {
            db.execute(
                "UPDATE relevos SET trozos = ?2, bytes = ?3, estado = ?4, caduca = ?5 WHERE id = ?1",
                params![id, trozos as i64, bytes as i64, estado, caduca],
            )
            .map_err(s)?;
            Ok(())
        })
    }
    fn borrar_relevo(&self, c: &ClienteCtx, id: &str) -> R<()> {
        self.con(c, |db| {
            db.execute("DELETE FROM relevos WHERE id = ?1", [id]).map_err(s)?;
            Ok(())
        })
    }

    // ---------- Plantillas de copia (cifradas por la consola) ----------
    fn plantillas(&self, c: &ClienteCtx) -> R<Vec<PlantillaCifrada>> {
        self.con(c, |db| {
            let mut st = db.prepare("SELECT id, cifrado, actualizada, por FROM plantillas ORDER BY actualizada DESC").map_err(s)?;
            let filas = st.query_map([], |r| Ok(PlantillaCifrada { id: r.get(0)?, cifrado: r.get(1)?, actualizada: r.get(2)?, por: r.get(3)? })).map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn guardar_plantilla(&self, c: &ClienteCtx, id: &str, cifrado: &str, por: &str, maximo: usize) -> R<bool> {
        self.con(c, |db| {
            let existe: bool = db.query_row("SELECT 1 FROM plantillas WHERE id = ?1", [id], |_| Ok(true)).optional().map_err(s)?.unwrap_or(false);
            let n: i64 = db.query_row("SELECT COUNT(*) FROM plantillas", [], |r| r.get(0)).map_err(s)?;
            if !existe && n as usize >= maximo {
                return Ok(false);
            }
            db.execute(
                "INSERT INTO plantillas (id, cifrado, actualizada, por) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(id) DO UPDATE SET cifrado = ?2, actualizada = ?3, por = ?4",
                params![id, cifrado, ahora(), por],
            )
            .map_err(s)?;
            Ok(true)
        })
    }
    fn borrar_plantilla(&self, c: &ClienteCtx, id: &str) -> R<bool> {
        self.con(c, |db| Ok(db.execute("DELETE FROM plantillas WHERE id = ?1", [id]).map_err(s)? > 0))
    }

    // ---------- Ajustes de las etiquetas (v1.52) ----------
    fn ajustes_etiquetas(&self, c: &ClienteCtx) -> R<Vec<AjusteEtiqueta>> {
        self.con(c, |db| {
            let mut st = db.prepare("SELECT datos, actualizada, por FROM etiquetas_ajustes ORDER BY clave").map_err(s)?;
            let filas = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Ts>(1)?, r.get::<_, String>(2)?))).map_err(s)?;
            let mut out = Vec::new();
            for f in filas {
                let (datos, actualizada, por) = f.map_err(s)?;
                // Una fila que no se entiende (de una versión futura) se salta.
                if let Ok(a) = serde_json::from_str::<AjusteEtiqueta>(&datos) {
                    out.push(AjusteEtiqueta { actualizada, por, ..a });
                }
            }
            Ok(out)
        })
    }
    fn poner_ajuste_etiqueta(&self, c: &ClienteCtx, a: &AjusteEtiqueta, maximo: usize) -> R<bool> {
        let clave = a.nombre.to_lowercase();
        let datos = serde_json::to_string(&AjusteEtiqueta { actualizada: 0, por: String::new(), ..a.clone() }).map_err(s)?;
        self.con(c, |db| {
            let existe: bool = db.query_row("SELECT 1 FROM etiquetas_ajustes WHERE clave = ?1", [&clave], |_| Ok(true)).optional().map_err(s)?.unwrap_or(false);
            let n: i64 = db.query_row("SELECT COUNT(*) FROM etiquetas_ajustes", [], |r| r.get(0)).map_err(s)?;
            if !existe && n as usize >= maximo {
                return Ok(false);
            }
            db.execute(
                "INSERT INTO etiquetas_ajustes (clave, datos, actualizada, por) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(clave) DO UPDATE SET datos = ?2, actualizada = ?3, por = ?4",
                params![clave, datos, ahora(), a.por],
            )
            .map_err(s)?;
            Ok(true)
        })
    }
    fn borrar_ajuste_etiqueta(&self, c: &ClienteCtx, nombre: &str) -> R<bool> {
        let clave = nombre.to_lowercase();
        self.con(c, |db| Ok(db.execute("DELETE FROM etiquetas_ajustes WHERE clave = ?1", [clave]).map_err(s)? > 0))
    }

    // ---------- Datos comunes del cliente (0.7.26, bloque 8) ----------
    fn datos_comunes(&self, c: &ClienteCtx) -> R<Vec<(String, String)>> {
        self.con(c, |db| {
            let mut st = db.prepare("SELECT clave, datos FROM datos_comunes ORDER BY clave").map_err(s)?;
            let filas = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn poner_dato_comun(&self, c: &ClienteCtx, clave: &str, datos: &str) -> R<()> {
        self.con(c, |db| {
            db.execute(
                "INSERT INTO datos_comunes (clave, datos, actualizada) VALUES (?1, ?2, ?3) ON CONFLICT(clave) DO UPDATE SET datos = ?2, actualizada = ?3",
                params![clave, datos, ahora()],
            )
            .map_err(s)?;
            Ok(())
        })
    }

    // ---------- Catálogo de destinos (tarea 7a) ----------
    fn destinos_catalogo(&self, c: &ClienteCtx) -> R<Vec<DestinoCatalogo>> {
        self.con(c, |db| {
            let mut st =
                db.prepare("SELECT id, nombre, tipo, donde, actualizado, por, atributos FROM destinos ORDER BY nombre COLLATE NOCASE, id").map_err(s)?;
            let filas = st
                .query_map([], |r| {
                    Ok(DestinoCatalogo {
                        id: r.get(0)?,
                        nombre: r.get(1)?,
                        tipo: r.get(2)?,
                        donde: r.get(3)?,
                        actualizado: r.get(4)?,
                        por: r.get(5)?,
                        atributos: r.get(6)?,
                    })
                })
                .map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn guardar_destino(&self, c: &ClienteCtx, d: &DestinoCatalogo, maximo: usize, mantener_atributos: bool) -> R<bool> {
        self.con(c, |db| {
            let existe: bool = db.query_row("SELECT 1 FROM destinos WHERE id = ?1", [&d.id], |_| Ok(true)).optional().map_err(s)?.unwrap_or(false);
            let n: i64 = db.query_row("SELECT COUNT(*) FROM destinos", [], |r| r.get(0)).map_err(s)?;
            if !existe && n as usize >= maximo {
                return Ok(false);
            }
            let sql = if mantener_atributos {
                "INSERT INTO destinos (id, nombre, tipo, donde, actualizado, por, atributos) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT(id) DO UPDATE SET nombre = ?2, tipo = ?3, donde = ?4, actualizado = ?5, por = ?6"
            } else {
                "INSERT INTO destinos (id, nombre, tipo, donde, actualizado, por, atributos) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT(id) DO UPDATE SET nombre = ?2, tipo = ?3, donde = ?4, actualizado = ?5, por = ?6, atributos = ?7"
            };
            db.execute(sql, params![d.id, d.nombre, d.tipo, d.donde, d.actualizado, d.por, d.atributos]).map_err(s)?;
            Ok(true)
        })
    }
    fn borrar_destino(&self, c: &ClienteCtx, id: &str) -> R<bool> {
        self.con(c, |db| Ok(db.execute("DELETE FROM destinos WHERE id = ?1", [id]).map_err(s)? > 0))
    }

    // ---------- Historial de los equipos (v1.23) ----------
    fn guardar_historial_tope(&self, c: &ClienteCtx, equipo: &str, entradas: &[EntradaHistorial], tope: i64) -> R<usize> {
        self.con(c, |db| {
            let tx = db.unchecked_transaction().map_err(s)?;
            let mut nuevas = 0;
            for e in entradas {
                let n = tx
                    .execute(
                        "INSERT OR IGNORE INTO historial (equipo_id, id, hora, tipo, datos) VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![equipo, e.id, e.hora, e.tipo, e.datos],
                    )
                    .map_err(s)?;
                if n == 0 {
                    continue;
                }
                nuevas += 1;
                if let Some((tipo, mensaje)) = &e.aviso {
                    // Nunca con fecha futura (lo primero de la lista hasta entonces).
                    let creado = e.hora.min(ahora());
                    let ya: bool = tx
                        .query_row(
                            "SELECT EXISTS (SELECT 1 FROM avisos WHERE equipo_id = ?1 AND tipo = ?2 AND mensaje = ?3 AND ABS(creado - ?4) < 600)",
                            params![equipo, tipo, mensaje, creado],
                            |r| r.get(0),
                        )
                        .map_err(s)?;
                    if !ya {
                        tx.execute(
                            "INSERT INTO avisos (id, equipo_id, tipo, mensaje, creado, visto_por) VALUES (?1, ?2, ?3, ?4, ?5, 'la consola anterior')",
                            params![nuevo_id(), equipo, tipo, mensaje, creado],
                        )
                        .map_err(s)?;
                    }
                }
            }
            // v1.45: las vueltas de la retención llevan las versiones que quitaron; solo las
            // más recientes las conservan (las demás, sus cifras), así que no crecen sin límite.
            if nuevas > 0 && entradas.iter().any(|e| e.tipo == "retencion") {
                tx.execute(
                    "UPDATE historial SET datos = json_set(json_remove(datos, '$.versiones', '$.grupos', '$.motivos'), '$.compactada', json('true'))                      WHERE equipo_id = ?1 AND tipo = 'retencion' AND json_valid(datos) AND json_type(datos, '$.versiones') IS NOT NULL                      AND id NOT IN (SELECT id FROM historial WHERE equipo_id = ?1 AND tipo = 'retencion' ORDER BY hora DESC, id LIMIT ?2)",
                    params![equipo, crate::agentes::RETENCIONES_CON_DETALLE],
                )
                .map_err(s)?;
            }
            // Como mucho las más recientes de cada equipo: el disco no crece sin límite.
            tx.execute(
                "DELETE FROM historial WHERE equipo_id = ?1 AND id NOT IN (SELECT id FROM historial WHERE equipo_id = ?1 ORDER BY hora DESC, id LIMIT ?2)",
                params![equipo, tope.clamp(1, MAX_HISTORIAL_EQUIPO)],
            )
            .map_err(s)?;
            tx.commit().map_err(s)?;
            Ok(nuevas)
        })
    }
    fn historial(&self, c: &ClienteCtx, equipo: &str, f: &FiltroHistorial, limite: i64) -> R<Vec<EntradaHistorial>> {
        self.con(c, |db| {
            // El cursor: la hora de la última entrada de la página anterior.
            let cursor = match &f.antes {
                Some(id) => {
                    let hora: Option<Ts> =
                        db.query_row("SELECT hora FROM historial WHERE equipo_id = ?1 AND id = ?2", params![equipo, id], |r| r.get(0)).optional().map_err(s)?;
                    match hora {
                        Some(h) => Some((h, id.clone())),
                        // Ya no está (el tope borra lo más antiguo): lo de detrás tampoco.
                        None => return Ok(Vec::new()),
                    }
                }
                None => None,
            };
            let mut sql = String::from("SELECT id, hora, tipo, datos FROM historial WHERE equipo_id = ?1 AND hora > ?2 AND hora <= ?3");
            let mut args: Vec<rusqlite::types::Value> =
                vec![equipo.to_string().into(), f.desde.unwrap_or(i64::MIN).into(), f.hasta.unwrap_or(i64::MAX).into(), limite.into()];
            if let Some((h, id)) = cursor {
                sql.push_str(" AND (hora < ?5 OR (hora = ?5 AND id > ?6))");
                args.push(h.into());
                args.push(id.into());
            }
            if !f.tipos.is_empty() {
                let mut marcas = Vec::new();
                for t in &f.tipos {
                    args.push(t.clone().into());
                    marcas.push(format!("?{}", args.len()));
                }
                sql.push_str(&format!(" AND tipo IN ({})", marcas.join(", ")));
            }
            sql.push_str(" ORDER BY hora DESC, id LIMIT ?4");
            let mut st = db.prepare(&sql).map_err(s)?;
            let filas = st
                .query_map(rusqlite::params_from_iter(args), |r| {
                    Ok(EntradaHistorial { id: r.get(0)?, hora: r.get(1)?, tipo: r.get(2)?, datos: r.get(3)?, aviso: None })
                })
                .map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn ultima_historial(&self, c: &ClienteCtx, equipo: &str) -> R<Option<Ts>> {
        self.con(c, |db| db.query_row("SELECT MAX(hora) FROM historial WHERE equipo_id = ?1", [equipo], |r| r.get(0)).map_err(s))
    }

    // ---------- Mantenimiento ----------
    // ---------- Notificaciones ----------
    fn notif_evento(&self, datos: &str) -> R<()> {
        self.ctl().execute("INSERT INTO notif_eventos (creado, datos) VALUES (?1, ?2)", params![ahora(), datos]).map_err(s)?;
        Ok(())
    }
    fn notif_eventos(&self, limite: i64) -> R<Vec<(i64, String)>> {
        let c = self.ctl();
        let mut st = c.prepare("SELECT n, datos FROM notif_eventos ORDER BY n LIMIT ?1").map_err(s)?;
        let filas = st.query_map([limite], |r| Ok((r.get(0)?, r.get(1)?))).map_err(s)?;
        filas.collect::<Result<Vec<_>, _>>().map_err(s)
    }
    fn notif_borrar_evento(&self, n: i64) -> R<()> {
        self.ctl().execute("DELETE FROM notif_eventos WHERE n = ?1", [n]).map_err(s)?;
        Ok(())
    }
    fn notif_incidente(&self, clave: &str) -> R<Option<Incidente>> {
        self.ctl().query_row(&format!("SELECT {COLS_INCIDENTE} FROM notif_incidentes WHERE clave = ?1"), [clave], fila_incidente).optional().map_err(s)
    }
    fn notif_guardar_incidente(&self, i: &Incidente) -> R<()> {
        self.ctl()
            .execute(
                &format!(
                    "INSERT OR REPLACE INTO notif_incidentes ({COLS_INCIDENTE}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"
                ),
                params![
                    i.clave,
                    i.cliente,
                    i.equipo,
                    i.tipo,
                    i.sujeto,
                    i.severidad.clave(),
                    i.titulo,
                    i.mensaje,
                    i.abierto as i64,
                    i.primero,
                    i.ultimo,
                    i.veces,
                    i.marca,
                    i.notificado,
                    i.cerrado
                ],
            )
            .map_err(s)?;
        Ok(())
    }
    fn notif_incidentes_abiertos(&self, cliente: Option<&str>) -> R<Vec<Incidente>> {
        let c = self.ctl();
        let mut st = c
            .prepare(&format!("SELECT {COLS_INCIDENTE} FROM notif_incidentes WHERE abierto = 1 AND (?1 IS NULL OR cliente_id = ?1) ORDER BY primero"))
            .map_err(s)?;
        let filas = st.query_map(params![cliente], fila_incidente).map_err(s)?;
        filas.collect::<Result<Vec<_>, _>>().map_err(s)
    }
    fn notif_guardar_envio(&self, e: &Envio) -> R<()> {
        let mensaje = serde_json::to_string(&e.mensaje).map_err(s)?;
        self.ctl()
            .execute(
                &format!(
                    "INSERT OR REPLACE INTO notif_envios ({COLS_ENVIO}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)"
                ),
                params![
                    e.id,
                    e.creado,
                    e.ambito,
                    e.cliente,
                    e.canal,
                    e.destino,
                    e.tipo,
                    e.severidad.clave(),
                    e.incidente,
                    e.titulo,
                    mensaje,
                    e.estado,
                    e.intentos as i64,
                    e.siguiente,
                    e.error,
                    e.enviado,
                    e.entrega,
                    e.nota
                ],
            )
            .map_err(s)?;
        Ok(())
    }
    fn notif_envios_debidos(&self, ahora: Ts, limite: i64) -> R<Vec<Envio>> {
        envios(
            &self.ctl(),
            &format!("SELECT {COLS_ENVIO} FROM notif_envios WHERE estado = 'pendiente' AND siguiente <= ?1 ORDER BY creado, rowid LIMIT ?2"),
            params![ahora, limite],
        )
    }
    fn notif_entregas_desde(&self, canal: &str, destino: &str, desde: Ts) -> R<Vec<Ts>> {
        let c = self.ctl();
        let mut st = c
            .prepare(
                "SELECT MIN(enviado) FROM notif_envios WHERE canal_id = ?1 AND destino = ?2 AND estado = 'enviado' AND enviado > ?3 \
                 AND tipo NOT IN ('resumen', 'prueba') GROUP BY COALESCE(entrega, id)",
            )
            .map_err(s)?;
        let filas = st.query_map(params![canal, destino, desde], |r| r.get::<_, i64>(0)).map_err(s)?;
        filas.collect::<Result<Vec<_>, _>>().map_err(s)
    }
    fn notif_envios_incidente(&self, incidente: &str) -> R<Vec<Envio>> {
        envios(&self.ctl(), &format!("SELECT {COLS_ENVIO} FROM notif_envios WHERE incidente = ?1 ORDER BY creado, rowid"), [incidente])
    }
    fn notif_registro(&self, cliente: Option<&str>, limite: i64) -> R<Vec<Envio>> {
        envios(
            &self.ctl(),
            &format!(
                "SELECT {COLS_ENVIO} FROM notif_envios WHERE ?1 IS NULL OR cliente_id = ?1 OR ambito = 'cliente:' || ?1 \
                 ORDER BY COALESCE(enviado, creado) DESC, rowid DESC LIMIT ?2"
            ),
            params![cliente, limite],
        )
    }
    fn notif_limpiar(&self, antes: Ts) -> R<()> {
        let c = self.ctl();
        c.execute("DELETE FROM notif_envios WHERE estado IN ('enviado', 'fallido', 'descartado') AND creado < ?1", [antes]).map_err(s)?;
        c.execute("DELETE FROM notif_incidentes WHERE abierto = 0 AND cerrado < ?1", [antes]).map_err(s)?;
        Ok(())
    }

    fn todos_los_clientes(&self) -> R<Vec<ClienteCtx>> {
        let c = self.ctl();
        let mut st = c.prepare("SELECT id FROM clientes").map_err(s)?;
        let filas = st.query_map([], |r| r.get::<_, String>(0)).map_err(s)?;
        Ok(filas.filter_map(Result::ok).map(|id| ClienteCtx::autorizado(&id)).collect())
    }
    fn clientes_del_servidor(&self) -> R<Vec<ClienteServidor>> {
        let c = self.ctl();
        let mut st = c
            .prepare(
                "SELECT c.id, c.nombre, c.creado, \
                   (SELECT COUNT(*) FROM pertenencias p WHERE p.cliente_id = c.id), \
                   (SELECT COUNT(*) FROM pertenencias p WHERE p.cliente_id = c.id AND p.rol = 'propietario') \
                 FROM clientes c ORDER BY c.nombre COLLATE NOCASE",
            )
            .map_err(s)?;
        let filas = st
            .query_map([], |r| Ok(ClienteServidor { id: r.get(0)?, nombre: r.get(1)?, creado: r.get(2)?, personas: r.get(3)?, propietarios: r.get(4)? }))
            .map_err(s)?;
        filas.collect::<Result<Vec<_>, _>>().map_err(s)
    }
    fn uso_cliente(&self, c: &ClienteCtx) -> R<UsoCliente> {
        let ruta = self.dir.join("clientes").join(format!("{}.db", id_seguro(c.id())?));
        let bytes = [ruta.clone(), ruta.with_extension("db-wal")].iter().filter_map(|p| std::fs::metadata(p).ok()).map(|m| m.len()).sum();
        let hace_30d = ahora() - 30 * 86_400;
        self.con(c, |db| {
            let n = |sql: &str| -> R<i64> { db.query_row(sql, [], |r| r.get(0)).map_err(s) };
            let equipos = n("SELECT COUNT(*) FROM equipos")?;
            let equipos_confirmados = n("SELECT COUNT(*) FROM equipos WHERE confirmado = 1")?;
            let historial = n("SELECT COUNT(*) FROM historial")?;
            let ordenes_30d: i64 = db.query_row("SELECT COUNT(*) FROM ordenes WHERE emitida >= ?1", [hace_30d], |r| r.get(0)).map_err(s)?;
            let contacto: Option<Ts> = db.query_row("SELECT MAX(ultimo_contacto) FROM equipos", [], |r| r.get(0)).map_err(s)?;
            let auditoria: Option<Ts> = db.query_row("SELECT MAX(creado) FROM auditoria", [], |r| r.get(0)).map_err(s)?;
            Ok(UsoCliente { equipos, equipos_confirmados, historial, ordenes_30d, ultima_actividad: contacto.max(auditoria), bytes })
        })
    }
    fn limpiar(&self, c: &ClienteCtx, ahora: Ts) -> R<Vec<String>> {
        self.con(c, |db| {
            db.execute("DELETE FROM mensajes_i WHERE sesion_id IN (SELECT id FROM sesiones_i WHERE expira <= ?1)", [ahora]).map_err(s)?;
            db.execute("DELETE FROM sesiones_i WHERE expira <= ?1", [ahora]).map_err(s)?;
            db.execute("UPDATE emparejamientos SET estado = 'caducado', codigo = NULL WHERE estado IN ('abierto', 'unido') AND caduca <= ?1", [ahora])
                .map_err(s)?;
            // Confirmado sin alta: el código, como mucho 7 días.
            db.execute("UPDATE emparejamientos SET codigo = NULL WHERE estado = 'confirmado' AND codigo IS NOT NULL AND creado <= ?1", [ahora - 7 * 86_400])
                .map_err(s)?;
            let caducados = {
                let mut st = db.prepare("SELECT id FROM relevos WHERE caduca <= ?1").map_err(s)?;
                let filas = st.query_map([ahora], |r| r.get::<_, String>(0)).map_err(s)?;
                filas.collect::<Result<Vec<_>, _>>().map_err(s)?
            };
            db.execute("DELETE FROM relevos WHERE caduca <= ?1", [ahora]).map_err(s)?;
            // Órdenes que caducaron sin que el equipo las recogiera o contestara (también con el
            // equipo apagado: así la consola no las enseña «pendientes» o «entregadas» para siempre).
            caducar(db, None, ahora)?;
            // Informes: se guardan 90 días.
            db.execute("DELETE FROM informes WHERE recibido <= ?1", [ahora - 90 * 86_400]).map_err(s)?;
            Ok(caducados)
        })
    }
    fn equipos_sin_alta(&self, c: &ClienteCtx) -> R<Vec<String>> {
        self.con(c, |db| {
            let mut st = db
                .prepare(
                    "SELECT DISTINCT e.id FROM equipos e JOIN emparejamientos p ON p.equipo_id = e.id \
                     WHERE e.confirmado = 0 AND p.estado IN ('caducado', 'cancelado') \
                     AND NOT EXISTS (SELECT 1 FROM emparejamientos q WHERE q.equipo_id = e.id AND q.estado NOT IN ('caducado', 'cancelado'))",
                )
                .map_err(s)?;
            let filas = st.query_map([], |r| r.get::<_, String>(0)).map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn limpiar_servidor(&self, ahora: Ts) -> R<()> {
        let c = self.ctl();
        c.execute("DELETE FROM sesiones WHERE expira <= ?1", [ahora]).map_err(s)?;
        c.execute("DELETE FROM codigos WHERE caduca <= ?1", [ahora]).map_err(s)?;
        c.execute("DELETE FROM codigos_varios WHERE caduca <= ?1", [ahora]).map_err(s)?;
        c.execute("DELETE FROM invitaciones WHERE caduca <= ?1", [ahora]).map_err(s)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn almacen() -> (tempfile::TempDir, Sqlite) {
        let dir = tempfile::tempdir().unwrap();
        let a = Sqlite::abrir(dir.path()).unwrap();
        (dir, a)
    }

    #[test]
    fn un_paso_de_totp_se_gasta_una_vez() {
        let (_d, a) = almacen();
        let c = a.crear_cuenta("ana@ejemplo.com", "Ana", "h", true).unwrap();
        assert!(a.gastar_totp(&c.id, 100).unwrap());
        assert!(!a.gastar_totp(&c.id, 100).unwrap(), "el mismo paso, otra vez, no");
        assert!(!a.gastar_totp(&c.id, 99).unwrap(), "uno anterior, tampoco");
        assert!(a.gastar_totp(&c.id, 101).unwrap());
        assert_eq!(a.cuenta(&c.id).unwrap().unwrap().totp_ultimo, 101);
    }

    #[test]
    fn preparados_caducan_y_olvidan_su_codigo() {
        let (_d, a) = almacen();
        let cl = a.crear_cliente("Ferretería Altamar", "sal", 24).unwrap();
        let c = ClienteCtx::autorizado(&cl.id);
        let t = ahora();
        // Uno vivo y uno que ya caducó.
        a.preparar_emparejamiento(&c, "vivo", "ana", t + 3600, "SERVIDOR-01", "windows", "AAAA-BBBB-CC").unwrap();
        a.indexar_codigo("h-vivo", &cl.id, "vivo", t + 3600).unwrap();
        a.preparar_emparejamiento(&c, "viejo", "ana", t - 1, "PC-VIEJO", "linux", "DDDD-EEEE-FF").unwrap();
        a.indexar_codigo("h-viejo", &cl.id, "viejo", t - 1).unwrap();
        let l = a.emparejamientos_preparados(&c, t).unwrap();
        assert_eq!(l.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["vivo"]);
        assert_eq!(l[0].codigo.as_deref(), Some("AAAA-BBBB-CC"));
        // El código caducado ya no se puede gastar.
        assert!(a.tomar_codigo("h-viejo").unwrap().is_none());
        // Un solo uso: el vivo, una vez.
        assert!(a.tomar_codigo("h-vivo").unwrap().is_some());
        assert!(a.tomar_codigo("h-vivo").unwrap().is_none());
        // Al caducar (limpieza) o al confirmar/anular, el código se borra.
        a.limpiar(&c, t).unwrap();
        let viejo = a.emparejamiento(&c, "viejo").unwrap().unwrap();
        assert_eq!((viejo.estado.as_str(), viejo.codigo), ("caducado", None));
        a.poner_estado_emparejamiento(&c, "vivo", "unido", Some("eq1")).unwrap();
        assert!(a.emparejamiento(&c, "vivo").unwrap().unwrap().codigo.is_some(), "unido: aún hace falta para el alta");
        // Al unirse, al menos 24 h para comparar el número y dar de alta.
        assert!(a.emparejamiento(&c, "vivo").unwrap().unwrap().caduca >= t + crate::almacen::PLAZO_UNIDO_S);
        assert_eq!(a.a_medias(&c, t).unwrap().iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["vivo"], "unido: a medias");
        // Confirmado sin el alta: aún a medias (la consola necesita el código para mandarla).
        a.poner_estado_emparejamiento(&c, "vivo", "confirmado", None).unwrap();
        assert!(a.emparejamiento(&c, "vivo").unwrap().unwrap().codigo.is_some(), "confirmado: hasta el alta");
        assert_eq!(a.a_medias(&c, t).unwrap().len(), 1);
        a.alta_hecha(&c, "eq1").unwrap();
        let vivo = a.emparejamiento(&c, "vivo").unwrap().unwrap();
        assert_eq!((vivo.codigo, vivo.equipo_id.as_deref(), vivo.nombre.as_deref()), (None, Some("eq1"), Some("SERVIDOR-01")));
        assert!(a.a_medias(&c, t).unwrap().is_empty(), "con el alta, nada a medias");
    }

    /// 9g: con más clientes que el tope, se cierran las conexiones menos usadas y
    /// todo sigue bien (al volver, se abre de nuevo con sus datos); la que está en
    /// uso no se cierra.
    #[test]
    fn conexiones_por_cliente_con_tope() {
        let dir = tempfile::tempdir().unwrap();
        let a = Sqlite::abrir_con_tope(dir.path(), 4).unwrap();
        let clientes: Vec<ClienteCtx> = (0..10).map(|i| ClienteCtx::autorizado(&a.crear_cliente(&format!("Cliente {i}"), "s", 24).unwrap().id)).collect();
        for (i, c) in clientes.iter().enumerate() {
            a.auditar(c, "ana", "orden", &format!("equipo-{i}"), "{}").unwrap();
            assert!(a.abiertas() <= 4, "{}", a.abiertas());
        }
        // Otra vuelta: cada uno con lo suyo, y la cadena de auditoría sigue entera.
        for (i, c) in clientes.iter().enumerate() {
            a.auditar(c, "ana", "orden", &format!("equipo-{i}-b"), "{}").unwrap();
            assert_eq!(a.verificar_auditoria(c).unwrap(), (2, None));
        }
        assert_eq!(a.abiertas(), 4);
        // La más usada últimamente sigue abierta: es la misma conexión.
        let ultima = a.conexion(&clientes[9]).unwrap();
        assert!(Arc::ptr_eq(&ultima, &a.conexion(&clientes[9]).unwrap()));
        // En uso (alguien tiene su Arc): no se cierra aunque sea la más antigua.
        let en_uso = a.conexion(&clientes[0]).unwrap();
        for c in &clientes[1..9] {
            a.conexion(c).unwrap();
        }
        assert!(Arc::ptr_eq(&en_uso, &a.conexion(&clientes[0]).unwrap()), "la que está en uso sigue siendo la misma");
        drop((en_uso, ultima));
        // Todas en uso: se pasa del tope en vez de abrir dos para el mismo cliente.
        let todas: Vec<_> = clientes.iter().map(|c| a.conexion(c).unwrap()).collect();
        assert_eq!(a.abiertas(), 10);
        drop(todas);
        a.conexion(&ClienteCtx::autorizado(&a.crear_cliente("Otro", "s", 24).unwrap().id)).unwrap();
        assert_eq!(a.abiertas(), 4, "en cuanto quedan libres, vuelve al tope");
    }

    #[test]
    fn auditoria_encadenada_y_de_solo_anadir() {
        let (_d, a) = almacen();
        let cl = a.crear_cliente("Café del Sur", "sal", 24).unwrap();
        let c = ClienteCtx::autorizado(&cl.id);
        for i in 0..5 {
            a.auditar(&c, "ana", "orden", &format!("equipo-{i}"), "{}").unwrap();
        }
        assert_eq!(a.verificar_auditoria(&c).unwrap(), (5, None));
        // Ni UPDATE ni DELETE: lo impiden los disparadores.
        let con = a.conexion(&c).unwrap();
        let db = con.lock().unwrap();
        assert!(db.execute("UPDATE auditoria SET actor = 'otro' WHERE n = 2", []).is_err());
        assert!(db.execute("DELETE FROM auditoria WHERE n = 2", []).is_err());
    }

    /// Las mismas huellas que calcula la consola (`consola/scripts/vectores-auditoria.ts`),
    /// que con ellas comprueba la cadena contra un ancla (plan-mejoras 9b).
    #[test]
    fn huellas_de_la_auditoria_como_en_la_consola() {
        let h1 = hash_entrada(GENESIS, 1, 1_790_000_000, "cuenta:ana@ejemplo.com", "crear_cliente", "cl-norte", r#"{"nombre":"Ferretería Rambla"}"#);
        assert_eq!(h1, "e891152ddde045e2a42c97dce84ab501d4420a91c33548482f6b7d3b85675cde");
        let h2 = hash_entrada(&h1, 2, 1_790_000_060, "cuenta:ana@ejemplo.com", "renombrar_cliente", "cl-norte", "{}");
        assert_eq!(h2, "db30da97f801c28f8d8b60a40f0d77fe5f3588fbe03e42f2ebd991d7f9d4bc3e");
        assert_eq!(
            resguardo_protocolo::derivaciones::linea_ancla("cl-norte", 2, 1_790_000_060, &h2),
            "resguardo-ancla:1:cl-norte:2:1790000060:db30da97f801c28f8d8b60a40f0d77fe5f3588fbe03e42f2ebd991d7f9d4bc3e"
        );
    }

    /// Bug de un cliente: la misma máquina salía dos veces porque un intento anterior de
    /// vincularla se quedó en «Falta confirmar el número de comprobación». Al confirmar la
    /// buena, el resto sin confirmar de esa máquina se quita (y se anula su código).
    #[test]
    fn al_confirmar_se_quita_el_duplicado_sin_confirmar() {
        let (_d, a) = almacen();
        let c = ClienteCtx::autorizado(&a.crear_cliente("Uno", "s", 24).unwrap().id);
        let equipo = |id: &str, nombre: &str, firma: &str| EquipoNuevo {
            id: id.into(),
            nombre: nombre.into(),
            so: "windows".into(),
            version: "1".into(),
            box_pub: "b".into(),
            sign_pub: firma.into(),
            sal_equipo: "sal".into(),
            secreto_hash: "h".into(),
        };
        let t = ahora();
        // Dos intentos con la misma máquina («CAJA-1»), uno con otro nombre pero la misma
        // clave de firma, otro equipo distinto sin confirmar y otro ya confirmado.
        for (emp, eq, nombre, firma) in
            [("p1", "viejo", "caja-1", "f1"), ("p2", "nuevo", "CAJA-1", "f2"), ("p3", "misma-firma", "Caja renombrada", "f2"), ("p4", "otro", "ALMACEN", "f3")]
        {
            a.preparar_emparejamiento(&c, emp, "ana", t + 900, nombre, "windows", "AAAA-BBBB-CC").unwrap();
            a.crear_equipo(&c, &equipo(eq, nombre, firma)).unwrap();
            a.poner_estado_emparejamiento(&c, emp, "unido", Some(eq)).unwrap();
        }
        a.crear_equipo(&c, &equipo("confirmado", "CAJA-1", "f9")).unwrap();
        a.confirmar_equipo(&c, "confirmado", "etiqueta-vieja").unwrap();
        // Se confirma «nuevo».
        a.poner_estado_emparejamiento(&c, "p2", "confirmado", None).unwrap();
        a.confirmar_equipo(&c, "nuevo", "etiqueta").unwrap();
        let alta = a.equipo(&c, "nuevo").unwrap().unwrap();
        let mut quitados = super::super::quitar_duplicados_sin_confirmar(&a, &c, &alta).unwrap();
        quitados.sort();
        assert_eq!(quitados, vec!["misma-firma".to_string(), "viejo".to_string()]);
        let quedan: Vec<String> = a.equipos(&c).unwrap().into_iter().map(|e| e.id).collect();
        assert!(quedan.contains(&"nuevo".to_string()) && quedan.contains(&"otro".to_string()), "lo demás sigue");
        assert!(quedan.contains(&"confirmado".to_string()), "nunca quita uno confirmado");
        assert!(!quedan.contains(&"viejo".to_string()) && !quedan.contains(&"misma-firma".to_string()));
        // Sus códigos, anulados: no salen «a medias» ni se pueden confirmar ya.
        assert_eq!(a.emparejamiento(&c, "p1").unwrap().unwrap().estado, "cancelado");
        assert!(a.emparejamiento(&c, "p1").unwrap().unwrap().codigo.is_none());
        assert!(a.a_medias(&c, t).unwrap().iter().all(|e| e.id != "p1" && e.id != "p3"));
        assert_eq!(a.emparejamiento(&c, "p4").unwrap().unwrap().estado, "unido", "el de otro equipo, intacto");
        // Y queda en la auditoría, con el equipo que se quedó.
        let aud = a.auditoria(&c, 0, 100).unwrap();
        let dup: Vec<_> = aud.iter().filter(|x| x.accion == "quitar_equipo_duplicado").collect();
        assert_eq!(dup.len(), 2);
        assert!(dup.iter().all(|x| x.actor == "servidor" && x.datos.contains("\"queda\":\"nuevo\"")));
        // Una segunda pasada no encuentra nada.
        assert!(super::super::quitar_duplicados_sin_confirmar(&a, &c, &alta).unwrap().is_empty());
    }

    #[test]
    fn equipos_que_nunca_se_confirmaron() {
        let (_d, a) = almacen();
        let c = ClienteCtx::autorizado(&a.crear_cliente("Uno", "s", 24).unwrap().id);
        let equipo = |id: &str| EquipoNuevo {
            id: id.into(),
            nombre: "PC".into(),
            so: "windows".into(),
            version: "1".into(),
            box_pub: "b".into(),
            sign_pub: "s".into(),
            sal_equipo: "sal".into(),
            secreto_hash: "h".into(),
        };
        let t = ahora();
        for (emp, eq) in [("p1", "fantasma"), ("p2", "bueno")] {
            a.preparar_emparejamiento(&c, emp, "ana", t + 900, "PC", "windows", "AAAA-BBBB-CC").unwrap();
            a.crear_equipo(&c, &equipo(eq)).unwrap();
            a.poner_estado_emparejamiento(&c, emp, "unido", Some(eq)).unwrap();
        }
        // «bueno»: se comparó el número y se dio de alta.
        a.poner_estado_emparejamiento(&c, "p2", "confirmado", None).unwrap();
        a.confirmar_equipo(&c, "bueno", "etiqueta").unwrap();
        // Mientras el código sirve, nada que quitar.
        assert!(a.equipos_sin_alta(&c).unwrap().is_empty());
        // Caducado sin confirmar: «fantasma» sobra; «bueno» no.
        a.limpiar(&c, t + 3 * 86_400).unwrap();
        assert_eq!(a.equipos_sin_alta(&c).unwrap(), vec!["fantasma".to_string()]);
        // Y uno sin emparejamiento (p. ej. recibido de otra consola) nunca.
        a.crear_equipo(&c, &equipo("sin-codigo")).unwrap();
        assert_eq!(a.equipos_sin_alta(&c).unwrap(), vec!["fantasma".to_string()]);
    }

    #[test]
    fn clientes_aislados_en_archivos() {
        let (dir, a) = almacen();
        let c1 = ClienteCtx::autorizado(&a.crear_cliente("Uno", "s", 24).unwrap().id);
        let c2 = ClienteCtx::autorizado(&a.crear_cliente("Dos", "s", 24).unwrap().id);
        let e = EquipoNuevo {
            id: "e1".into(),
            nombre: "PC".into(),
            so: "windows".into(),
            version: "1".into(),
            box_pub: "b".into(),
            sign_pub: "s".into(),
            sal_equipo: "sal".into(),
            secreto_hash: "h".into(),
        };
        a.crear_equipo(&c1, &e).unwrap();
        assert_eq!(a.equipos(&c1).unwrap().len(), 1);
        assert!(a.equipos(&c2).unwrap().is_empty());
        assert!(dir.path().join("clientes").join(format!("{}.db", c1.id())).is_file());
        assert!(id_seguro("../x").is_err());
    }

    #[test]
    fn seq_exacto_y_cancelacion() {
        let (_d, a) = almacen();
        let c = ClienteCtx::autorizado(&a.crear_cliente("Uno", "s", 24).unwrap().id);
        let e = EquipoNuevo {
            id: "e1".into(),
            nombre: "PC".into(),
            so: "w".into(),
            version: "1".into(),
            box_pub: "b".into(),
            sign_pub: "s".into(),
            sal_equipo: "sal".into(),
            secreto_hash: "h".into(),
        };
        a.crear_equipo(&c, &e).unwrap();
        let nueva = |seq: u64, nb: Option<Ts>| OrdenNueva {
            id: nuevo_id(),
            equipo_id: "e1".into(),
            tipo: "copiar_ahora".into(),
            seq,
            sellado: "x".into(),
            emitida_por: "ana".into(),
            not_before: nb,
            caduca: ahora() + 3600,
            sesion: None,
            relevo: None,
        };
        a.insertar_orden(&c, &nueva(1, None)).unwrap();
        assert_eq!(a.insertar_orden(&c, &nueva(1, None)).unwrap_err(), "seq:2:1001");
        let espera = a.insertar_orden(&c, &nueva(2, Some(ahora() + 3600))).unwrap();
        // Se entrega la 1; la 2 espera a su hora.
        let entregadas = a.entregar_ordenes(&c, "e1", ahora(), false).unwrap();
        assert_eq!(entregadas.iter().map(|o| o.seq).collect::<Vec<_>>(), vec![1]);
        assert_eq!(a.ordenes_con_espera(&c, ahora()).unwrap().len(), 1);
        assert!(a.cancelar_orden(&c, &espera.id, "ana", ahora()).unwrap());
        assert!(a.entregar_ordenes(&c, "e1", ahora() + 7200, false).unwrap().is_empty());
    }

    /// v1.49 (consolas-multiples.md §5): a un agente que admite `ordenes_en_espera` se le
    /// entregan al momento las que piden autorización y esperan su hora (las inofensivas
    /// no); siguen saliendo «esperando su turno», se pueden cancelar (y el equipo se entera)
    /// y, si el equipo dice que la aplicó igualmente, su resultado firmado manda.
    #[test]
    fn ordenes_con_espera_entregadas_antes() {
        let (_d, a) = almacen();
        let c = ClienteCtx::autorizado(&a.crear_cliente("Uno", "s", 24).unwrap().id);
        let e = EquipoNuevo {
            id: "e1".into(),
            nombre: "PC".into(),
            so: "w".into(),
            version: "1".into(),
            box_pub: "b".into(),
            sign_pub: "s".into(),
            sal_equipo: "sal".into(),
            secreto_hash: "h".into(),
        };
        a.crear_equipo(&c, &e).unwrap();
        let t = ahora();
        let nueva = |seq: u64, tipo: &str, nb: Option<Ts>| OrdenNueva {
            id: format!("o{seq}"),
            equipo_id: "e1".into(),
            tipo: tipo.into(),
            seq,
            sellado: "x".into(),
            emitida_por: "ana".into(),
            not_before: nb,
            caduca: t + 2 * 86_400,
            sesion: None,
            relevo: None,
        };
        a.insertar_orden(&c, &nueva(1, "pausar", Some(t + 86_400))).unwrap();
        a.insertar_orden(&c, &nueva(2, "copiar_ahora", Some(t + 3600))).unwrap();
        a.insertar_orden(&c, &nueva(3, "quitar_repositorio", Some(t + 86_400))).unwrap();
        // Un agente anterior: nada todavía.
        assert!(a.entregar_ordenes(&c, "e1", t, false).unwrap().is_empty());
        // Uno nuevo: las dos que piden autorización, en orden; la inofensiva, a su hora.
        let ya = a.entregar_ordenes(&c, "e1", t, true).unwrap();
        assert_eq!(ya.iter().map(|o| o.seq).collect::<Vec<_>>(), vec![1, 3]);
        assert_eq!(a.ordenes_con_espera(&c, t).unwrap().len(), 3, "entregadas o no, siguen esperando su turno");
        // Cancelar una entregada que espera: el equipo se entera en la próxima entrega.
        assert!(a.cancelar_orden(&c, "o1", "ana", t).unwrap());
        assert_eq!(a.canceladas_sin_avisar(&c, "e1").unwrap(), vec!["o1".to_string()]);
        // El equipo dice (firmado) que la aplicó igualmente: sin `pisar_cancelada` no cambia; con él, sí.
        let mut r = ResultadoOrden { orden: "o1".into(), estado: "hecha".into(), mensaje: None, detalle: None, firma: "f".into(), pisar_cancelada: false };
        assert!(!a.resultado_orden(&c, "e1", &r).unwrap());
        r.pisar_cancelada = true;
        assert!(a.resultado_orden(&c, "e1", &r).unwrap());
        assert_eq!(a.orden(&c, "o1").unwrap().unwrap().estado, "hecha");
        assert_eq!(a.entregar_ordenes(&c, "e1", t + 3600, true).unwrap().iter().map(|o| o.seq).collect::<Vec<_>>(), vec![2]);
    }

    /// Lo entregado por una conexión muerta vuelve a entregarse; lo que caduca sin
    /// respuesta del equipo (también con el equipo apagado) deja de estar «entregada».
    #[test]
    fn entregadas_perdidas_y_caducadas() {
        let (_d, a) = almacen();
        let c = ClienteCtx::autorizado(&a.crear_cliente("Uno", "s", 24).unwrap().id);
        let e = EquipoNuevo {
            id: "e1".into(),
            nombre: "PC".into(),
            so: "w".into(),
            version: "1".into(),
            box_pub: "b".into(),
            sign_pub: "s".into(),
            sal_equipo: "sal".into(),
            secreto_hash: "h".into(),
        };
        a.crear_equipo(&c, &e).unwrap();
        let t = ahora();
        let nueva = |seq: u64, caduca: Ts| OrdenNueva {
            id: format!("o{seq}"),
            equipo_id: "e1".into(),
            tipo: "copiar_ahora".into(),
            seq,
            sellado: "x".into(),
            emitida_por: "ana".into(),
            not_before: None,
            caduca,
            sesion: None,
            relevo: None,
        };
        for seq in 1..=3 {
            a.insertar_orden(&c, &nueva(seq, t + 3600)).unwrap();
        }
        assert_eq!(a.entregar_ordenes(&c, "e1", t, false).unwrap().len(), 3);
        // El equipo aceptó la 1 y luego la conexión murió: la 2 y la 3 se le vuelven a dar.
        assert_eq!(a.reponer_no_recibidas(&c, "e1", 1, t).unwrap(), 2);
        assert_eq!(a.entregar_ordenes(&c, "e1", t, false).unwrap().iter().map(|o| o.seq).collect::<Vec<_>>(), vec![2, 3]);
        // Las que ya aceptó (o caducadas) no.
        assert_eq!(a.reponer_no_recibidas(&c, "e1", 3, t).unwrap(), 0);
        assert_eq!(a.reponer_no_recibidas(&c, "e1", 1, t + 7200).unwrap(), 0);
        // Sin respuesta pasada su caducidad: «caducada», también sin que el equipo vuelva (limpieza).
        a.insertar_orden(&c, &nueva(4, t + 60)).unwrap();
        a.limpiar(&c, t + 7200).unwrap();
        let estados: Vec<String> = a.ordenes_equipo(&c, "e1", 10).unwrap().into_iter().map(|o| o.estado).collect();
        assert!(estados.iter().all(|e| e == "caducada"), "{estados:?}");
        // Sin espera: no hay que avisar de nada (se ven en «Órdenes» con su motivo).
        assert!(a.caducadas_por_avisar(&c).unwrap().is_empty());
        let motivos: Vec<Option<String>> = a.ordenes_equipo(&c, "e1", 10).unwrap().into_iter().map(|o| o.motivo).collect();
        assert_eq!(motivos, [Some("sin_entregar"), Some("sin_respuesta"), Some("sin_respuesta"), Some("sin_respuesta")].map(|m| m.map(String::from)));
    }

    /// Lo que hace un agente anterior a v1.49 (sin `ordenes_en_espera`) con lo que se le
    /// entrega: acepta solo números mayores que el último que aceptó; si no, «Orden repetida o
    /// antigua». Devuelve las que rechazó.
    fn agente_anterior(ultimo: &mut u64, entregadas: &[Orden]) -> Vec<u64> {
        let mut rechazadas = Vec::new();
        for o in entregadas {
            if o.seq > *ultimo {
                *ultimo = o.seq;
            } else {
                rechazadas.push(o.seq);
            }
        }
        rechazadas
    }

    fn equipo_de_prueba(a: &Sqlite) -> ClienteCtx {
        let c = ClienteCtx::autorizado(&a.crear_cliente("Uno", "s", 24).unwrap().id);
        let e = EquipoNuevo {
            id: "e1".into(),
            nombre: "PC".into(),
            so: "w".into(),
            version: "0.7.18".into(),
            box_pub: "b".into(),
            sign_pub: "s".into(),
            sal_equipo: "sal".into(),
            secreto_hash: "h".into(),
        };
        a.crear_equipo(&c, &e).unwrap();
        c
    }

    fn orden_nueva(id: &str, tipo: &str, seq: u64, nb: Option<Ts>, caduca: Ts) -> OrdenNueva {
        OrdenNueva {
            id: id.into(),
            equipo_id: "e1".into(),
            tipo: tipo.into(),
            seq,
            sellado: "x".into(),
            emitida_por: "ana".into(),
            not_before: nb,
            caduca,
            sesion: None,
            relevo: None,
        }
    }

    /// El caso que se veía en la consola en línea: «Quitar la copia externa» (con 24 h de
    /// espera) a un agente 0.7.18 y, mientras esperaba, otras órdenes al mismo equipo (abrir
    /// una sesión, copiar ahora…). Antes, esas salían al momento y, a su hora, el equipo
    /// rechazaba la que esperaba por «antigua»: desaparecía de «esperando su turno» y la copia
    /// externa seguía ahí. Ahora la consola le pone el número reservado (`seq_espera`), por
    /// encima de las que vengan, y todas se aplican en su orden.
    #[test]
    fn orden_con_espera_a_un_agente_anterior_no_se_queda_antigua() {
        let (_d, a) = almacen();
        let c = equipo_de_prueba(&a);
        let t = ahora();
        let nb = t + 24 * 3600;
        a.insertar_orden(&c, &orden_nueva("o1", "copiar_ahora", 1, None, t + 3600)).unwrap();
        let eq = a.equipo(&c, "e1").unwrap().unwrap();
        assert_eq!((eq.siguiente_seq, eq.seq_espera), (2, 1001));
        // La que espera, con el número reservado; no mueve el siguiente.
        a.insertar_orden(&c, &orden_nueva("externa", "cambiar_copia_externa", 1001, Some(nb), nb + 72 * 3600)).unwrap();
        let eq = a.equipo(&c, "e1").unwrap().unwrap();
        assert_eq!((eq.siguiente_seq, eq.seq_espera), (2, 2001));
        // El número reservado solo vale con espera.
        assert_eq!(a.insertar_orden(&c, &orden_nueva("x", "copiar_ahora", 2001, None, t + 3600)).unwrap_err(), "seq:2:2001");
        // Mientras espera, otras órdenes (normales) salen al momento.
        for seq in 2..=4 {
            a.insertar_orden(&c, &orden_nueva(&format!("o{seq}"), "abrir_sesion", seq, None, t + 3600)).unwrap();
        }
        let mut ultimo = 0;
        let ya = a.entregar_ordenes(&c, "e1", t, false).unwrap();
        assert_eq!(ya.iter().map(|o| o.seq).collect::<Vec<_>>(), vec![1, 2, 3, 4]);
        assert!(agente_anterior(&mut ultimo, &ya).is_empty());
        // A su hora: llega con un número mayor que todo lo anterior y el equipo la acepta.
        let ya = a.entregar_ordenes(&c, "e1", nb, false).unwrap();
        assert_eq!(ya.iter().map(|o| o.id.as_str()).collect::<Vec<_>>(), vec!["externa"]);
        assert!(agente_anterior(&mut ultimo, &ya).is_empty(), "no se rechaza por antigua");
        // Y las siguientes, por encima de ella.
        assert_eq!(a.equipo(&c, "e1").unwrap().unwrap().siguiente_seq, 1002);
        a.insertar_orden(&c, &orden_nueva("o5", "copiar_ahora", 1002, None, nb + 3600)).unwrap();
        let ya = a.entregar_ordenes(&c, "e1", nb, false).unwrap();
        assert_eq!(ya.len(), 1);
        assert!(agente_anterior(&mut ultimo, &ya).is_empty());
    }

    /// Una orden con espera mandada con el número de siempre (consola anterior, o las que ya
    /// estaban al actualizar el servidor): las posteriores esperan detrás de ella en vez de
    /// adelantarla. Antes: la 3 salía al momento y la 2 se rechazaba por antigua a su hora.
    #[test]
    fn las_posteriores_esperan_detras_de_la_que_espera() {
        let (_d, a) = almacen();
        let c = equipo_de_prueba(&a);
        let t = ahora();
        let nb = t + 24 * 3600;
        a.insertar_orden(&c, &orden_nueva("o1", "copiar_ahora", 1, None, t + 3600)).unwrap();
        a.insertar_orden(&c, &orden_nueva("o2", "cambiar_copia_externa", 2, Some(nb), nb + 24 * 3600)).unwrap();
        a.insertar_orden(&c, &orden_nueva("o3", "copiar_ahora", 3, None, nb + 3600)).unwrap();
        let mut ultimo = 0;
        let ya = a.entregar_ordenes(&c, "e1", t, false).unwrap();
        assert_eq!(ya.iter().map(|o| o.seq).collect::<Vec<_>>(), vec![1]);
        agente_anterior(&mut ultimo, &ya);
        let ya = a.entregar_ordenes(&c, "e1", nb, false).unwrap();
        assert_eq!(ya.iter().map(|o| o.seq).collect::<Vec<_>>(), vec![2, 3]);
        assert!(agente_anterior(&mut ultimo, &ya).is_empty());
        // Cancelar la que espera libera a las de detrás.
        a.insertar_orden(&c, &orden_nueva("o4", "pausar", 4, Some(nb + 86_400), nb + 2 * 86_400)).unwrap();
        a.insertar_orden(&c, &orden_nueva("o5", "copiar_ahora", 5, None, nb + 3600)).unwrap();
        assert!(a.entregar_ordenes(&c, "e1", nb, false).unwrap().is_empty());
        assert!(a.cancelar_orden(&c, "o4", "ana", nb).unwrap());
        assert_eq!(a.entregar_ordenes(&c, "e1", nb, false).unwrap().iter().map(|o| o.seq).collect::<Vec<_>>(), vec![5]);
    }

    /// Una reservada que se cancela (o caduca) no deja su número en medio: el siguiente salta
    /// por encima (el equipo admite huecos) y la próxima reservada va por encima de todo. Y si
    /// las normales llegan a una reservada viva, la saltan y esperan detrás de ella.
    #[test]
    fn numeros_tras_una_reservada() {
        let (_d, a) = almacen();
        let c = equipo_de_prueba(&a);
        let t = ahora();
        a.insertar_orden(&c, &orden_nueva("w", "pausar", 1000, Some(t + 86_400), t + 2 * 86_400)).unwrap();
        assert_eq!(a.equipo(&c, "e1").unwrap().unwrap().siguiente_seq, 1);
        assert!(a.cancelar_orden(&c, "w", "ana", t).unwrap());
        let eq = a.equipo(&c, "e1").unwrap().unwrap();
        assert_eq!((eq.siguiente_seq, eq.seq_espera), (1001, 2000));
        // El número que tenía la consola ya no vale (409 con los nuevos) y el nuevo, sí.
        assert_eq!(a.insertar_orden(&c, &orden_nueva("x", "copiar_ahora", 1, None, t + 3600)).unwrap_err(), "seq:1001:2000");
        a.insertar_orden(&c, &orden_nueva("x", "copiar_ahora", 1001, None, t + 3600)).unwrap();
        a.insertar_orden(&c, &orden_nueva("w2", "pausar", 2001, Some(t + 86_400), t + 2 * 86_400)).unwrap();
        // El equipo dice que ya aceptó hasta la 2000 (p. ej. tras restaurar la consola): el
        // siguiente no cae en el número de la reservada.
        a.adelantar_seq(&c, "e1", 2001).unwrap();
        assert_eq!(a.equipo(&c, "e1").unwrap().unwrap().siguiente_seq, 2002);
        a.insertar_orden(&c, &orden_nueva("y", "copiar_ahora", 2002, None, t + 2 * 86_400)).unwrap();
        assert_eq!(a.entregar_ordenes(&c, "e1", t, false).unwrap().iter().map(|o| o.seq).collect::<Vec<_>>(), vec![1001]);
        assert_eq!(a.entregar_ordenes(&c, "e1", t + 86_400, false).unwrap().iter().map(|o| o.seq).collect::<Vec<_>>(), vec![2001, 2002]);
    }

    /// Un agente anterior con el reloj atrasado rechaza la orden a su hora («Todavía no es la
    /// hora»): vuelve a pendientes y se le da otra vez pasados unos minutos. Y lo que tenía
    /// espera y caducó sin aplicarse queda por avisar, una vez.
    #[test]
    fn reloj_atrasado_y_caducadas_con_espera() {
        let (_d, a) = almacen();
        let c = equipo_de_prueba(&a);
        let t = ahora();
        let nb = t + 3600;
        a.insertar_orden(&c, &orden_nueva("w", "cambiar_copia_externa", 1, Some(nb), nb + 86_400)).unwrap();
        assert_eq!(a.entregar_ordenes(&c, "e1", nb, false).unwrap().len(), 1);
        assert!(a.reintentar_orden(&c, "w", nb + 600, 2).unwrap());
        assert!(a.entregar_ordenes(&c, "e1", nb + 60, false).unwrap().is_empty(), "aún no");
        assert_eq!(a.entregar_ordenes(&c, "e1", nb + 600, false).unwrap().len(), 1);
        assert!(a.reintentar_orden(&c, "w", nb + 1200, 2).unwrap());
        assert_eq!(a.entregar_ordenes(&c, "e1", nb + 1200, false).unwrap().len(), 1);
        assert!(!a.reintentar_orden(&c, "w", nb + 1800, 2).unwrap(), "como mucho 2 veces");
        // Caducó sin respuesta: por avisar, una sola vez y con su motivo.
        a.limpiar(&c, nb + 86_400).unwrap();
        let por_avisar = a.caducadas_por_avisar(&c).unwrap();
        assert_eq!(por_avisar.iter().map(|o| (o.id.as_str(), o.motivo.as_deref())).collect::<Vec<_>>(), vec![("w", Some("sin_respuesta"))]);
        assert!(a.caducadas_por_avisar(&c).unwrap().is_empty());
    }
}
