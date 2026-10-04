//! Cuotas por cliente (v1.34): lo que cada cliente puede usar de un servidor
//! compartido (una consola en internet con varios clientes). Las pone quien
//! administra el servidor en «Clientes del servidor»: unas predeterminadas y,
//! si hace falta, otras para un cliente concreto.
//!
//! En cada campo: sin poner (`null`) = la predeterminada; `0` = sin límite; un
//! número = el límite.
//! - `equipos`: equipos dados de alta en el cliente (confirmados o no).
//! - `historial`: entradas del historial que se guardan por equipo (nunca más
//!   de [`MAX_HISTORIAL_EQUIPO`]); las más antiguas se borran.
//! - `relevo_mb_mes`: MB que los equipos del cliente pueden subir al relé de
//!   descargas (restaurar al navegador) en un mes natural (UTC).
//! - `ordenes_min`: órdenes por minuto del cliente (de todas sus personas).

use crate::almacen::{Almacen, Ts, MAX_HISTORIAL_EQUIPO, R};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cuotas {
    #[serde(default)]
    pub equipos: Option<i64>,
    #[serde(default)]
    pub historial: Option<i64>,
    #[serde(default)]
    pub relevo_mb_mes: Option<i64>,
    #[serde(default)]
    pub ordenes_min: Option<i64>,
}

/// Lo más alto que se puede poner en cada cuota.
const MAXIMOS: Cuotas = Cuotas { equipos: Some(100_000), historial: Some(MAX_HISTORIAL_EQUIPO), relevo_mb_mes: Some(100_000_000), ordenes_min: Some(10_000) };

impl Cuotas {
    /// Las de fábrica. En internet (`publico`), con límites; en la red local,
    /// como siempre (solo el tope del historial y un freno a las órdenes).
    pub fn de_fabrica(publico: bool) -> Self {
        if publico {
            Cuotas { equipos: Some(50), historial: Some(MAX_HISTORIAL_EQUIPO), relevo_mb_mes: Some(20 * 1024), ordenes_min: Some(60) }
        } else {
            Cuotas { equipos: Some(0), historial: Some(MAX_HISTORIAL_EQUIPO), relevo_mb_mes: Some(0), ordenes_min: Some(300) }
        }
    }

    /// Campo a campo: lo puesto aquí y, lo que falte, de `base`.
    pub fn sobre(self, base: Cuotas) -> Cuotas {
        Cuotas {
            equipos: self.equipos.or(base.equipos),
            historial: self.historial.or(base.historial),
            relevo_mb_mes: self.relevo_mb_mes.or(base.relevo_mb_mes),
            ordenes_min: self.ordenes_min.or(base.ordenes_min),
        }
    }

    /// Cada campo, sin poner o entre 0 y su máximo.
    pub fn validar(&self) -> Result<(), String> {
        let campos = [
            ("equipos", self.equipos, MAXIMOS.equipos),
            ("historial", self.historial, MAXIMOS.historial),
            ("relevo_mb_mes", self.relevo_mb_mes, MAXIMOS.relevo_mb_mes),
            ("ordenes_min", self.ordenes_min, MAXIMOS.ordenes_min),
        ];
        for (nombre, v, max) in campos {
            if let (Some(v), Some(max)) = (v, max) {
                if !(0..=max).contains(&v) {
                    return Err(format!("La cuota «{nombre}» tiene que estar entre 0 (sin límite) y {max}."));
                }
            }
        }
        Ok(())
    }
}

/// El límite de un campo ya resuelto: `None` = sin límite.
pub fn limite(v: Option<i64>) -> Option<i64> {
    v.filter(|n| *n > 0)
}

fn clave_cliente(cliente: &str) -> String {
    format!("cuotas:{cliente}")
}
const CLAVE_PREDETERMINADAS: &str = "cuotas_predeterminadas";

fn leer(db: &dyn Almacen, clave: &str) -> R<Cuotas> {
    Ok(db.valor(clave)?.and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default())
}

/// Las predeterminadas del servidor (lo que no se puso, de fábrica).
pub fn predeterminadas(db: &dyn Almacen, publico: bool) -> R<Cuotas> {
    Ok(leer(db, CLAVE_PREDETERMINADAS)?.sobre(Cuotas::de_fabrica(publico)))
}

pub fn poner_predeterminadas(db: &dyn Almacen, c: &Cuotas) -> R<()> {
    db.poner_valor(CLAVE_PREDETERMINADAS, &serde_json::to_string(c).map_err(|e| e.to_string())?)
}

/// Las propias de un cliente (sin resolver).
pub fn del_cliente(db: &dyn Almacen, cliente: &str) -> R<Cuotas> {
    leer(db, &clave_cliente(cliente))
}

pub fn poner_del_cliente(db: &dyn Almacen, cliente: &str, c: &Cuotas) -> R<()> {
    db.poner_valor(&clave_cliente(cliente), &serde_json::to_string(c).map_err(|e| e.to_string())?)
}

/// Las que valen para un cliente: las suyas y, lo que no tenga, las predeterminadas.
pub fn efectivas(db: &dyn Almacen, publico: bool, cliente: &str) -> R<Cuotas> {
    Ok(del_cliente(db, cliente)?.sobre(predeterminadas(db, publico)?))
}

/// Entradas del historial que se guardan por equipo en ese cliente.
pub fn tope_historial(db: &dyn Almacen, publico: bool, cliente: &str) -> R<i64> {
    Ok(limite(efectivas(db, publico, cliente)?.historial).map_or(MAX_HISTORIAL_EQUIPO, |n| n.min(MAX_HISTORIAL_EQUIPO)))
}

/// El mes natural (UTC) de una hora: `2026-10`.
pub fn mes(ts: Ts) -> String {
    chrono::DateTime::from_timestamp(ts, 0).map(|d| d.format("%Y-%m").to_string()).unwrap_or_default()
}

fn clave_relevo(cliente: &str, mes: &str) -> String {
    format!("relevo_mes:{cliente}:{mes}")
}

/// Bytes subidos al relé por los equipos del cliente en ese mes.
pub fn relevo_usado(db: &dyn Almacen, cliente: &str, mes: &str) -> R<u64> {
    Ok(db.valor(&clave_relevo(cliente, mes))?.and_then(|v| v.parse().ok()).unwrap_or(0))
}

/// Suma `bytes` a lo subido este mes (en una sola sentencia); devuelve el total.
pub fn sumar_relevo(db: &dyn Almacen, cliente: &str, mes: &str, bytes: u64) -> R<u64> {
    db.sumar_valor(&clave_relevo(cliente, mes), bytes as i64).map(|n| n.max(0) as u64)
}

/// ¿Caben `bytes` más en el relé de este cliente este mes?
pub fn cabe_en_relevo(db: &dyn Almacen, publico: bool, cliente: &str, mes: &str, bytes: u64) -> R<bool> {
    let Some(mb) = limite(efectivas(db, publico, cliente)?.relevo_mb_mes) else { return Ok(true) };
    Ok(relevo_usado(db, cliente, mes)? + bytes <= mb as u64 * 1024 * 1024)
}

/// ¿Cabe otro equipo en el cliente? `Err` con el mensaje para la persona.
pub fn cabe_otro_equipo(db: &dyn Almacen, publico: bool, cliente: &str, equipos: usize) -> R<Result<(), String>> {
    Ok(match limite(efectivas(db, publico, cliente)?.equipos) {
        Some(max) if equipos as i64 >= max => Err(mensaje_equipos(max)),
        _ => Ok(()),
    })
}

pub fn mensaje_equipos(max: i64) -> String {
    format!("Este cliente ya tiene {max} equipos, el máximo que permite este servidor. Pide más a quien lo administra (o quita alguno que ya no uses).")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn campo_a_campo_y_cero_es_sin_limite() {
        let propias = Cuotas { equipos: Some(5), ..Default::default() };
        let base = Cuotas { equipos: Some(50), historial: Some(100), relevo_mb_mes: Some(0), ordenes_min: None };
        let e = propias.sobre(base);
        assert_eq!(e, Cuotas { equipos: Some(5), historial: Some(100), relevo_mb_mes: Some(0), ordenes_min: None });
        assert_eq!(limite(e.equipos), Some(5));
        assert_eq!(limite(e.relevo_mb_mes), None, "0 = sin límite");
        assert_eq!(limite(e.ordenes_min), None, "sin poner y sin base: sin límite");
        assert!(Cuotas { equipos: Some(-1), ..Default::default() }.validar().is_err());
        assert!(Cuotas { historial: Some(MAX_HISTORIAL_EQUIPO + 1), ..Default::default() }.validar().is_err());
        assert!(Cuotas::de_fabrica(true).validar().is_ok() && Cuotas::de_fabrica(false).validar().is_ok());
        assert_eq!(mes(1_791_158_400), "2026-10");
    }

    #[test]
    fn en_la_base_de_datos() {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::almacen::sqlite::Sqlite::abrir(dir.path()).unwrap();
        // De fábrica: en internet, 50 equipos; en la red local, sin límite.
        assert_eq!(efectivas(&db, true, "c1").unwrap().equipos, Some(50));
        assert!(cabe_otro_equipo(&db, false, "c1", 10_000).unwrap().is_ok());
        poner_predeterminadas(&db, &Cuotas { equipos: Some(3), relevo_mb_mes: Some(1), ..Default::default() }).unwrap();
        poner_del_cliente(&db, "c2", &Cuotas { equipos: Some(0), ..Default::default() }).unwrap();
        assert!(cabe_otro_equipo(&db, true, "c1", 2).unwrap().is_ok());
        assert!(cabe_otro_equipo(&db, true, "c1", 3).unwrap().is_err());
        assert!(cabe_otro_equipo(&db, true, "c2", 3).unwrap().is_ok(), "c2 sin límite");
        // El relé: 1 MB al mes, por cliente y por mes.
        assert!(cabe_en_relevo(&db, true, "c1", "2026-10", 1024 * 1024).unwrap());
        assert_eq!(sumar_relevo(&db, "c1", "2026-10", 700 * 1024).unwrap(), 700 * 1024);
        assert_eq!(sumar_relevo(&db, "c1", "2026-10", 100 * 1024).unwrap(), 800 * 1024);
        assert!(!cabe_en_relevo(&db, true, "c1", "2026-10", 300 * 1024).unwrap());
        assert!(cabe_en_relevo(&db, true, "c1", "2026-11", 300 * 1024).unwrap(), "mes nuevo");
        assert!(cabe_en_relevo(&db, true, "c2", "2026-10", 300 * 1024).unwrap(), "otro cliente");
        assert_eq!(tope_historial(&db, true, "c1").unwrap(), MAX_HISTORIAL_EQUIPO);
        poner_del_cliente(&db, "c1", &Cuotas { historial: Some(10), ..Default::default() }).unwrap();
        assert_eq!(tope_historial(&db, true, "c1").unwrap(), 10);
    }
}
