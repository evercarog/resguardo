//! Observaciones y comentarios (v1.3x): notas que escriben las personas sobre
//! un equipo, un repositorio, una copia, un destino o el propio cliente.
//!
//! Se guardan **en claro** en el archivo del cliente (`clientes/<id>.db`), así
//! que viajan con él en la copia de la consola y se protegen como todo lo
//! demás del cliente (miembros y papeles). No van dentro de la configuración
//! cifrada de los equipos: verlas no pide la clave de administración. Por eso
//! la consola recuerda «no pongas contraseñas aquí».
//!
//! - **Observación**: un texto libre por objeto (Markdown ligero), que se
//!   cambia entero; vacía, no existe.
//! - **Comentarios**: una bitácora por objeto, con autor y fecha.

use super::{ClienteCtx, Ts, R};
use rusqlite::{params, Connection, OptionalExtension};

pub(super) const ESQUEMA: &str = r#"
CREATE TABLE IF NOT EXISTS observaciones (
  tipo TEXT NOT NULL, objeto TEXT NOT NULL, texto TEXT NOT NULL, actualizada INTEGER NOT NULL,
  por_id TEXT NOT NULL, por TEXT NOT NULL, PRIMARY KEY (tipo, objeto));
CREATE TABLE IF NOT EXISTS comentarios (
  id TEXT PRIMARY KEY, tipo TEXT NOT NULL, objeto TEXT NOT NULL, texto TEXT NOT NULL,
  autor_id TEXT NOT NULL, autor TEXT NOT NULL, creado INTEGER NOT NULL, editado INTEGER);
CREATE INDEX IF NOT EXISTS comentarios_objeto ON comentarios (tipo, objeto, creado);
"#;

/// La observación de un objeto.
#[derive(Clone, Debug, PartialEq)]
pub struct Observacion {
    pub tipo: String,
    pub objeto: String,
    pub texto: String,
    pub actualizada: Ts,
    /// Id de la cuenta que la cambió por última vez (o el de otro servidor, si se importó).
    pub por_id: String,
    /// Su nombre en ese momento.
    pub por: String,
}

/// Un comentario de la bitácora de un objeto.
#[derive(Clone, Debug, PartialEq)]
pub struct Comentario {
    pub id: String,
    pub tipo: String,
    pub objeto: String,
    pub texto: String,
    pub autor_id: String,
    pub autor: String,
    pub creado: Ts,
    pub editado: Option<Ts>,
}

/// Lo que tiene un objeto (para los contadores y la búsqueda).
#[derive(Clone, Debug, PartialEq)]
pub struct IndiceNotas {
    pub tipo: String,
    pub objeto: String,
    /// El texto de su observación, si tiene.
    pub observacion: Option<String>,
    pub comentarios: i64,
    /// Lo más reciente (observación o comentario).
    pub actualizada: Ts,
}

/// Lo que se guarda de las notas de un cliente (lo implementa cada almacén).
pub trait AlmacenNotas {
    fn observacion(&self, c: &ClienteCtx, tipo: &str, objeto: &str) -> R<Option<Observacion>>;
    /// Crea, cambia o (con el texto vacío) borra. `false` si es nueva y ya hay `maximo`.
    fn poner_observacion(&self, c: &ClienteCtx, o: &Observacion, maximo: i64) -> R<bool>;
    /// Del más antiguo al más reciente (los últimos `limite`).
    fn comentarios(&self, c: &ClienteCtx, tipo: &str, objeto: &str, limite: i64) -> R<Vec<Comentario>>;
    fn comentario(&self, c: &ClienteCtx, id: &str) -> R<Option<Comentario>>;
    /// `false` si ese objeto ya tiene `maximo_objeto` comentarios o el cliente `maximo_cliente`.
    fn crear_comentario(&self, c: &ClienteCtx, k: &Comentario, maximo_objeto: i64, maximo_cliente: i64) -> R<bool>;
    fn editar_comentario(&self, c: &ClienteCtx, id: &str, texto: &str, cuando: Ts) -> R<bool>;
    fn borrar_comentario(&self, c: &ClienteCtx, id: &str) -> R<bool>;
    /// Cada objeto con algo (observación o comentarios).
    fn indice_notas(&self, c: &ClienteCtx) -> R<Vec<IndiceNotas>>;
    /// Todo (para exportar el cliente).
    fn todas_las_notas(&self, c: &ClienteCtx) -> R<(Vec<Observacion>, Vec<Comentario>)>;
    /// Las de otro servidor (al importar un cliente): no pisa lo que ya hay.
    fn importar_notas(&self, c: &ClienteCtx, obs: &[Observacion], coms: &[Comentario]) -> R<(usize, usize)>;
    /// Al quitar un equipo: las suyas y las de sus repositorios y copias.
    fn borrar_notas_equipo(&self, c: &ClienteCtx, equipo: &str) -> R<()>;
}

fn s(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn fila_obs(r: &rusqlite::Row<'_>) -> rusqlite::Result<Observacion> {
    Ok(Observacion { tipo: r.get(0)?, objeto: r.get(1)?, texto: r.get(2)?, actualizada: r.get(3)?, por_id: r.get(4)?, por: r.get(5)? })
}

const COLS_COM: &str = "id, tipo, objeto, texto, autor_id, autor, creado, editado";

fn fila_com(r: &rusqlite::Row<'_>) -> rusqlite::Result<Comentario> {
    Ok(Comentario {
        id: r.get(0)?,
        tipo: r.get(1)?,
        objeto: r.get(2)?,
        texto: r.get(3)?,
        autor_id: r.get(4)?,
        autor: r.get(5)?,
        creado: r.get(6)?,
        editado: r.get(7)?,
    })
}

fn poner_obs(db: &Connection, o: &Observacion, maximo: i64) -> R<bool> {
    if o.texto.is_empty() {
        db.execute("DELETE FROM observaciones WHERE tipo = ?1 AND objeto = ?2", params![o.tipo, o.objeto]).map_err(s)?;
        return Ok(true);
    }
    let existe =
        db.query_row("SELECT 1 FROM observaciones WHERE tipo = ?1 AND objeto = ?2", params![o.tipo, o.objeto], |_| Ok(())).optional().map_err(s)?.is_some();
    let n: i64 = db.query_row("SELECT COUNT(*) FROM observaciones", [], |r| r.get(0)).map_err(s)?;
    if !existe && n >= maximo {
        return Ok(false);
    }
    db.execute(
        "INSERT INTO observaciones (tipo, objeto, texto, actualizada, por_id, por) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(tipo, objeto) DO UPDATE SET texto = ?3, actualizada = ?4, por_id = ?5, por = ?6",
        params![o.tipo, o.objeto, o.texto, o.actualizada, o.por_id, o.por],
    )
    .map_err(s)?;
    Ok(true)
}

impl AlmacenNotas for super::sqlite::Sqlite {
    fn observacion(&self, c: &ClienteCtx, tipo: &str, objeto: &str) -> R<Option<Observacion>> {
        self.con(c, |db| {
            db.query_row(
                "SELECT tipo, objeto, texto, actualizada, por_id, por FROM observaciones WHERE tipo = ?1 AND objeto = ?2",
                params![tipo, objeto],
                fila_obs,
            )
            .optional()
            .map_err(s)
        })
    }
    fn poner_observacion(&self, c: &ClienteCtx, o: &Observacion, maximo: i64) -> R<bool> {
        self.con(c, |db| poner_obs(db, o, maximo))
    }
    fn comentarios(&self, c: &ClienteCtx, tipo: &str, objeto: &str, limite: i64) -> R<Vec<Comentario>> {
        self.con(c, |db| {
            let mut st = db
                .prepare(&format!(
                    "SELECT {COLS_COM} FROM (SELECT rowid AS orden_alta, {COLS_COM} FROM comentarios WHERE tipo = ?1 AND objeto = ?2 ORDER BY creado DESC, orden_alta DESC LIMIT ?3) ORDER BY creado, orden_alta"
                ))
                .map_err(s)?;
            let filas = st.query_map(params![tipo, objeto, limite], fila_com).map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn comentario(&self, c: &ClienteCtx, id: &str) -> R<Option<Comentario>> {
        self.con(c, |db| db.query_row(&format!("SELECT {COLS_COM} FROM comentarios WHERE id = ?1"), [id], fila_com).optional().map_err(s))
    }
    fn crear_comentario(&self, c: &ClienteCtx, k: &Comentario, maximo_objeto: i64, maximo_cliente: i64) -> R<bool> {
        self.con(c, |db| {
            let en_objeto: i64 =
                db.query_row("SELECT COUNT(*) FROM comentarios WHERE tipo = ?1 AND objeto = ?2", params![k.tipo, k.objeto], |r| r.get(0)).map_err(s)?;
            let total: i64 = db.query_row("SELECT COUNT(*) FROM comentarios", [], |r| r.get(0)).map_err(s)?;
            if en_objeto >= maximo_objeto || total >= maximo_cliente {
                return Ok(false);
            }
            db.execute(
                "INSERT INTO comentarios (id, tipo, objeto, texto, autor_id, autor, creado, editado) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![k.id, k.tipo, k.objeto, k.texto, k.autor_id, k.autor, k.creado, k.editado],
            )
            .map_err(s)?;
            Ok(true)
        })
    }
    fn editar_comentario(&self, c: &ClienteCtx, id: &str, texto: &str, cuando: Ts) -> R<bool> {
        self.con(c, |db| Ok(db.execute("UPDATE comentarios SET texto = ?2, editado = ?3 WHERE id = ?1", params![id, texto, cuando]).map_err(s)? > 0))
    }
    fn borrar_comentario(&self, c: &ClienteCtx, id: &str) -> R<bool> {
        self.con(c, |db| Ok(db.execute("DELETE FROM comentarios WHERE id = ?1", [id]).map_err(s)? > 0))
    }
    fn indice_notas(&self, c: &ClienteCtx) -> R<Vec<IndiceNotas>> {
        self.con(c, |db| {
            let mut st = db
                .prepare(
                    "SELECT tipo, objeto, MAX(obs), SUM(n), MAX(cuando) FROM (
                       SELECT tipo, objeto, texto AS obs, 0 AS n, actualizada AS cuando FROM observaciones
                       UNION ALL
                       SELECT tipo, objeto, NULL, COUNT(*), MAX(COALESCE(editado, creado)) FROM comentarios GROUP BY tipo, objeto)
                     GROUP BY tipo, objeto ORDER BY tipo, objeto",
                )
                .map_err(s)?;
            let filas = st
                .query_map([], |r| {
                    Ok(IndiceNotas { tipo: r.get(0)?, objeto: r.get(1)?, observacion: r.get(2)?, comentarios: r.get(3)?, actualizada: r.get(4)? })
                })
                .map_err(s)?;
            filas.collect::<Result<Vec<_>, _>>().map_err(s)
        })
    }
    fn todas_las_notas(&self, c: &ClienteCtx) -> R<(Vec<Observacion>, Vec<Comentario>)> {
        self.con(c, |db| {
            let mut st = db.prepare("SELECT tipo, objeto, texto, actualizada, por_id, por FROM observaciones ORDER BY tipo, objeto").map_err(s)?;
            let obs = st.query_map([], fila_obs).map_err(s)?.collect::<Result<Vec<_>, _>>().map_err(s)?;
            let mut st = db.prepare(&format!("SELECT {COLS_COM} FROM comentarios ORDER BY creado, rowid")).map_err(s)?;
            let coms = st.query_map([], fila_com).map_err(s)?.collect::<Result<Vec<_>, _>>().map_err(s)?;
            Ok((obs, coms))
        })
    }
    fn importar_notas(&self, c: &ClienteCtx, obs: &[Observacion], coms: &[Comentario]) -> R<(usize, usize)> {
        self.con(c, |db| {
            let tx = db.unchecked_transaction().map_err(s)?;
            let mut n_obs = 0;
            for o in obs.iter().filter(|o| !o.texto.is_empty()) {
                n_obs += tx
                    .execute(
                        "INSERT OR IGNORE INTO observaciones (tipo, objeto, texto, actualizada, por_id, por) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![o.tipo, o.objeto, o.texto, o.actualizada, o.por_id, o.por],
                    )
                    .map_err(s)?;
            }
            let mut n_coms = 0;
            for k in coms {
                n_coms += tx
                    .execute(
                        "INSERT OR IGNORE INTO comentarios (id, tipo, objeto, texto, autor_id, autor, creado, editado) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                        params![k.id, k.tipo, k.objeto, k.texto, k.autor_id, k.autor, k.creado, k.editado],
                    )
                    .map_err(s)?;
            }
            tx.commit().map_err(s)?;
            Ok((n_obs, n_coms))
        })
    }
    fn borrar_notas_equipo(&self, c: &ClienteCtx, equipo: &str) -> R<()> {
        self.con(c, |db| {
            for tabla in ["observaciones", "comentarios"] {
                db.execute(
                    &format!("DELETE FROM {tabla} WHERE (tipo = 'equipo' AND objeto = ?1) OR (tipo IN ('repositorio', 'copia') AND substr(objeto, 1, length(?1) + 1) = ?1 || '/')"),
                    [equipo],
                )
                .map_err(s)?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::almacen::sqlite::Sqlite;

    fn obs(tipo: &str, objeto: &str, texto: &str) -> Observacion {
        Observacion { tipo: tipo.into(), objeto: objeto.into(), texto: texto.into(), actualizada: 100, por_id: "a".into(), por: "Ana".into() }
    }
    fn com(id: &str, objeto: &str, creado: Ts) -> Comentario {
        Comentario {
            id: id.into(),
            tipo: "equipo".into(),
            objeto: objeto.into(),
            texto: format!("nota {id}"),
            autor_id: "a".into(),
            autor: "Ana".into(),
            creado,
            editado: None,
        }
    }

    #[test]
    fn observaciones_comentarios_e_indice() {
        let dir = tempfile::tempdir().unwrap();
        let a = Sqlite::abrir(dir.path()).unwrap();
        let c = ClienteCtx::autorizado("c1");
        assert!(a.poner_observacion(&c, &obs("equipo", "e1", "Disco **nuevo**"), 2).unwrap());
        assert!(a.poner_observacion(&c, &obs("copia", "e1/k1", "Llamar a Luis"), 2).unwrap());
        // Ya hay 2: una nueva no cabe, cambiar una que ya está sí.
        assert!(!a.poner_observacion(&c, &obs("cliente", "c1", "x"), 2).unwrap());
        assert!(a.poner_observacion(&c, &obs("equipo", "e1", "Disco nuevo"), 2).unwrap());
        assert_eq!(a.observacion(&c, "equipo", "e1").unwrap().unwrap().texto, "Disco nuevo");

        for (i, t) in [(1, 10), (2, 30), (3, 20)] {
            assert!(a.crear_comentario(&c, &com(&format!("k{i}"), "e1", t), 3, 100).unwrap());
        }
        assert!(!a.crear_comentario(&c, &com("k4", "e1", 40), 3, 100).unwrap(), "tope por objeto");
        // Los dos últimos, del más antiguo al más reciente.
        let l = a.comentarios(&c, "equipo", "e1", 2).unwrap();
        assert_eq!(l.iter().map(|k| k.id.as_str()).collect::<Vec<_>>(), ["k3", "k2"]);
        assert!(a.editar_comentario(&c, "k1", "otro", 50).unwrap());
        assert_eq!(a.comentario(&c, "k1").unwrap().unwrap().editado, Some(50));

        let ind = a.indice_notas(&c).unwrap();
        assert_eq!(ind.len(), 2);
        let e1 = ind.iter().find(|i| i.tipo == "equipo").unwrap();
        assert_eq!((e1.observacion.as_deref(), e1.comentarios, e1.actualizada), (Some("Disco nuevo"), 3, 100));
        let k1 = ind.iter().find(|i| i.tipo == "copia").unwrap();
        assert_eq!((k1.comentarios, k1.observacion.is_some()), (0, true));

        // Vacía: se borra.
        assert!(a.poner_observacion(&c, &obs("copia", "e1/k1", ""), 2).unwrap());
        assert!(a.observacion(&c, "copia", "e1/k1").unwrap().is_none());
        assert!(a.borrar_comentario(&c, "k2").unwrap());
        assert!(!a.borrar_comentario(&c, "k2").unwrap());

        // Importar no pisa lo que hay.
        let (o, k) =
            a.importar_notas(&c, &[obs("equipo", "e1", "de fuera"), obs("destino", "nas", "Rack 2")], &[com("k1", "e1", 1), com("k9", "e2", 1)]).unwrap();
        assert_eq!((o, k), (1, 1));
        assert_eq!(a.observacion(&c, "equipo", "e1").unwrap().unwrap().texto, "Disco nuevo");
        let (obs_t, coms_t) = a.todas_las_notas(&c).unwrap();
        assert_eq!((obs_t.len(), coms_t.len()), (2, 3));

        // Al quitar el equipo, se van las suyas (y las de sus repositorios y copias), no las de otro.
        a.poner_observacion(&c, &obs("repositorio", "e1/r1", "x"), 10).unwrap();
        a.poner_observacion(&c, &obs("repositorio", "e10/r1", "y"), 10).unwrap();
        a.borrar_notas_equipo(&c, "e1").unwrap();
        let (obs_t, coms_t) = a.todas_las_notas(&c).unwrap();
        assert_eq!(obs_t.iter().map(|o| o.objeto.as_str()).collect::<Vec<_>>(), ["nas", "e10/r1"]);
        assert_eq!(coms_t.iter().map(|k| k.id.as_str()).collect::<Vec<_>>(), ["k9"]);
    }
}
