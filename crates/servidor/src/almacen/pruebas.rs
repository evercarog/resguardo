//! Pruebas funcionales (simulacros de recuperación; plan 0.8.0, E3, 14k).
//!
//! Cada prueba la registra una persona en la consola: fecha, motivo y alcance,
//! quién la hizo, qué se restauró y dónde, resultado, incidencias, el tiempo
//! hasta tener los datos de vuelta (RTO real) y la antigüedad de lo recuperado
//! (RPO), notas y, si quiere, unos adjuntos pequeños (PDF, PNG o JPG ya
//! comprobados por la API). Se guardan **en claro** en el archivo del cliente,
//! como las observaciones: no son secretos (la consola recuerda «no pongas
//! contraseñas aquí») y viajan con la copia de la consola.

use super::{ClienteCtx, Ts, R};
use rusqlite::{params, OptionalExtension};

pub(super) const ESQUEMA: &str = r#"
CREATE TABLE IF NOT EXISTS pruebas_funcionales (
  id TEXT PRIMARY KEY, fecha INTEGER NOT NULL, datos TEXT NOT NULL,
  creada INTEGER NOT NULL, actualizada INTEGER NOT NULL, por TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS pruebas_funcionales_fecha ON pruebas_funcionales (fecha);
CREATE TABLE IF NOT EXISTS pruebas_adjuntos (
  prueba_id TEXT NOT NULL, n INTEGER NOT NULL, nombre TEXT NOT NULL, tipo TEXT NOT NULL,
  tamano INTEGER NOT NULL, datos BLOB NOT NULL, creado INTEGER NOT NULL, por TEXT NOT NULL,
  PRIMARY KEY (prueba_id, n));
"#;

/// Una prueba guardada: `datos` es el JSON ya validado por la API.
#[derive(Clone, Debug, PartialEq)]
pub struct PruebaGuardada {
    pub id: String,
    /// El día de la prueba (mediodía UTC de ese día), para ordenar y filtrar.
    pub fecha: Ts,
    pub datos: String,
    pub creada: Ts,
    pub actualizada: Ts,
    /// Nombre de quien la registró o la cambió por última vez.
    pub por: String,
}

/// Un adjunto de una prueba (sin el contenido).
#[derive(Clone, Debug, PartialEq)]
pub struct AdjuntoInfo {
    pub prueba: String,
    pub n: i64,
    pub nombre: String,
    /// `application/pdf`, `image/png` o `image/jpeg`.
    pub tipo: String,
    pub tamano: i64,
    pub creado: Ts,
    pub por: String,
}

/// Lo que se guarda de las pruebas funcionales de un cliente (lo implementa cada almacén).
pub trait AlmacenPruebas {
    /// Todas, de la más reciente a la más antigua.
    fn pruebas_funcionales(&self, c: &ClienteCtx) -> R<Vec<PruebaGuardada>>;
    fn prueba_funcional(&self, c: &ClienteCtx, id: &str) -> R<Option<PruebaGuardada>>;
    /// Crea o sustituye; `false` si es nueva y ya hay `maximo`.
    fn guardar_prueba_funcional(&self, c: &ClienteCtx, p: &PruebaGuardada, maximo: i64) -> R<bool>;
    /// Con sus adjuntos.
    fn borrar_prueba_funcional(&self, c: &ClienteCtx, id: &str) -> R<bool>;
    /// Los adjuntos de todas las pruebas (sin el contenido).
    fn adjuntos_pruebas(&self, c: &ClienteCtx) -> R<Vec<AdjuntoInfo>>;
    /// El contenido de un adjunto.
    fn adjunto_prueba(&self, c: &ClienteCtx, prueba: &str, n: i64) -> R<Option<(AdjuntoInfo, Vec<u8>)>>;
    /// Lo añade con el número siguiente. `None` si la prueba ya tiene `max_prueba` o el
    /// cliente se pasaría de `max_bytes` en total.
    fn anadir_adjunto_prueba(&self, c: &ClienteCtx, a: &AdjuntoInfo, datos: &[u8], max_prueba: i64, max_bytes: i64) -> R<Option<i64>>;
    fn borrar_adjunto_prueba(&self, c: &ClienteCtx, prueba: &str, n: i64) -> R<bool>;
}

fn s(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn fila(r: &rusqlite::Row<'_>) -> rusqlite::Result<PruebaGuardada> {
    Ok(PruebaGuardada { id: r.get(0)?, fecha: r.get(1)?, datos: r.get(2)?, creada: r.get(3)?, actualizada: r.get(4)?, por: r.get(5)? })
}

const COLS: &str = "id, fecha, datos, creada, actualizada, por";

fn fila_adjunto(r: &rusqlite::Row<'_>) -> rusqlite::Result<AdjuntoInfo> {
    Ok(AdjuntoInfo { prueba: r.get(0)?, n: r.get(1)?, nombre: r.get(2)?, tipo: r.get(3)?, tamano: r.get(4)?, creado: r.get(5)?, por: r.get(6)? })
}

const COLS_ADJ: &str = "prueba_id, n, nombre, tipo, tamano, creado, por";

impl AlmacenPruebas for super::sqlite::Sqlite {
    fn pruebas_funcionales(&self, c: &ClienteCtx) -> R<Vec<PruebaGuardada>> {
        self.con(c, |db| {
            let mut st = db.prepare(&format!("SELECT {COLS} FROM pruebas_funcionales ORDER BY fecha DESC, creada DESC")).map_err(s)?;
            let filas = st.query_map([], fila).map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn prueba_funcional(&self, c: &ClienteCtx, id: &str) -> R<Option<PruebaGuardada>> {
        self.con(c, |db| db.query_row(&format!("SELECT {COLS} FROM pruebas_funcionales WHERE id = ?1"), [id], fila).optional().map_err(s))
    }
    fn guardar_prueba_funcional(&self, c: &ClienteCtx, p: &PruebaGuardada, maximo: i64) -> R<bool> {
        self.con(c, |db| {
            let existe = db.query_row("SELECT 1 FROM pruebas_funcionales WHERE id = ?1", [&p.id], |_| Ok(())).optional().map_err(s)?.is_some();
            let total: i64 = db.query_row("SELECT COUNT(*) FROM pruebas_funcionales", [], |r| r.get(0)).map_err(s)?;
            if !existe && total >= maximo {
                return Ok(false);
            }
            db.execute(
                "INSERT INTO pruebas_funcionales (id, fecha, datos, creada, actualizada, por) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET fecha = ?2, datos = ?3, actualizada = ?5, por = ?6",
                params![p.id, p.fecha, p.datos, p.creada, p.actualizada, p.por],
            )
            .map_err(s)?;
            Ok(true)
        })
    }
    fn borrar_prueba_funcional(&self, c: &ClienteCtx, id: &str) -> R<bool> {
        self.con(c, |db| {
            db.execute("DELETE FROM pruebas_adjuntos WHERE prueba_id = ?1", [id]).map_err(s)?;
            Ok(db.execute("DELETE FROM pruebas_funcionales WHERE id = ?1", [id]).map_err(s)? > 0)
        })
    }
    fn adjuntos_pruebas(&self, c: &ClienteCtx) -> R<Vec<AdjuntoInfo>> {
        self.con(c, |db| {
            let mut st = db.prepare(&format!("SELECT {COLS_ADJ} FROM pruebas_adjuntos ORDER BY prueba_id, n")).map_err(s)?;
            let filas = st.query_map([], fila_adjunto).map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn adjunto_prueba(&self, c: &ClienteCtx, prueba: &str, n: i64) -> R<Option<(AdjuntoInfo, Vec<u8>)>> {
        self.con(c, |db| {
            db.query_row(&format!("SELECT {COLS_ADJ}, datos FROM pruebas_adjuntos WHERE prueba_id = ?1 AND n = ?2"), params![prueba, n], |r| {
                Ok((fila_adjunto(r)?, r.get::<_, Vec<u8>>(7)?))
            })
            .optional()
            .map_err(s)
        })
    }
    fn anadir_adjunto_prueba(&self, c: &ClienteCtx, a: &AdjuntoInfo, datos: &[u8], max_prueba: i64, max_bytes: i64) -> R<Option<i64>> {
        self.con(c, |db| {
            let (n_prueba, siguiente): (i64, i64) = db
                .query_row("SELECT COUNT(*), COALESCE(MAX(n), 0) + 1 FROM pruebas_adjuntos WHERE prueba_id = ?1", [&a.prueba], |r| Ok((r.get(0)?, r.get(1)?)))
                .map_err(s)?;
            let total: i64 = db.query_row("SELECT COALESCE(SUM(tamano), 0) FROM pruebas_adjuntos", [], |r| r.get(0)).map_err(s)?;
            if n_prueba >= max_prueba || total + datos.len() as i64 > max_bytes {
                return Ok(None);
            }
            db.execute(
                "INSERT INTO pruebas_adjuntos (prueba_id, n, nombre, tipo, tamano, datos, creado, por) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![a.prueba, siguiente, a.nombre, a.tipo, datos.len() as i64, datos, a.creado, a.por],
            )
            .map_err(s)?;
            Ok(Some(siguiente))
        })
    }
    fn borrar_adjunto_prueba(&self, c: &ClienteCtx, prueba: &str, n: i64) -> R<bool> {
        self.con(c, |db| Ok(db.execute("DELETE FROM pruebas_adjuntos WHERE prueba_id = ?1 AND n = ?2", params![prueba, n]).map_err(s)? > 0))
    }
}
