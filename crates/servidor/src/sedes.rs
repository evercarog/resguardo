//! Plan 0.8.0, D1 y D2 (14ll, 14ss): **sedes, categorías de etiqueta y orden de
//! los equipos** de un cliente (docs/sedes.md).
//!
//! No tienen tabla propia: son datos comunes del cliente (`sede:`, `sede.equipo:`,
//! `sede.destino:`, `etiqueta.categoria:`, `orden.equipos:`), que se cambian con
//! `POST /api/clientes/{c}/datos-comunes` (administradores; la consola web los
//! reparte a los equipos, con la clave de administración la sede de un equipo o
//! de un destino, que cuenta en la regla 3-2-1) y llegan de las demás consolas
//! por los equipos (`crate::datos_comunes::recibir`). Aquí solo se leen, ya
//! limpios, para que los vea **cualquier miembro** del cliente (la regla y la
//! vista de Equipos los necesitan; los datos comunes en crudo son de administradores).

use crate::almacen::{Almacen, ClienteCtx, R};
use crate::datos_comunes as dcs;
use resguardo_protocolo::datos_cliente::{self as dc, Clase};
use serde_json::{json, Map, Value};

/// `{ sedes: [{ id, nombre, notas, principal }], equipos: { id: sede }, destinos: { id: sede },
/// categorias: [{ id, nombre, etiquetas }], orden: [id] }`. Una asignación a una sede que ya no
/// existe no sale (vuelve a la de por defecto). Las sedes y las categorías, por nombre.
pub fn organizacion(db: &dyn Almacen, ctx: &ClienteCtx) -> R<Value> {
    let filas = dcs::filas(db, ctx)?;
    let mut sedes = Vec::new();
    let mut categorias = Vec::new();
    let mut equipos = Map::new();
    let mut destinos = Map::new();
    let mut orden = Value::Array(Vec::new());
    for (clave, f) in &filas {
        let v = &f.entrada.valor;
        if v.is_null() {
            continue;
        }
        let Some((c, sujeto)) = dc::clase(clave) else { continue };
        // Lo que llegó de fuera ya está validado; se vuelve a limpiar por si cambió la validación.
        let Ok(v) = dc::valor_valido(clave, v) else { continue };
        match c {
            Clase::Sede => sedes.push(json!({
                "id": sujeto,
                "nombre": v["nombre"],
                "notas": v.get("notas").cloned().unwrap_or(Value::Null),
                "principal": v["principal"] == true,
            })),
            Clase::SedeEquipo => {
                equipos.insert(sujeto.to_string(), v["sede"].clone());
            }
            Clase::SedeDestino => {
                destinos.insert(sujeto.to_string(), v["sede"].clone());
            }
            Clase::EtiquetaCategoria => categorias.push(json!({ "id": sujeto, "nombre": v["nombre"], "etiquetas": v["etiquetas"] })),
            Clase::OrdenEquipos => orden = v["ids"].clone(),
            _ => {}
        }
    }
    let nombre = |x: &Value| x["nombre"].as_str().unwrap_or_default().to_lowercase();
    sedes.sort_by_key(nombre);
    categorias.sort_by_key(nombre);
    let existe = |s: &Value| sedes.iter().any(|x| x["id"] == *s);
    equipos.retain(|_, s| existe(s));
    destinos.retain(|_, s| existe(s));
    Ok(json!({ "sedes": sedes, "equipos": equipos, "destinos": destinos, "categorias": categorias, "orden": orden }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::almacen::sqlite::Sqlite;

    #[test]
    fn se_lee_limpio_y_sin_asignaciones_huerfanas() {
        let dir = tempfile::tempdir().unwrap();
        let db = Sqlite::abrir(dir.path()).unwrap();
        let c = db.crear_cliente("Cliente de prueba", &base64::Engine::encode(&base64::engine::general_purpose::STANDARD, [3u8; 16]), 24).unwrap();
        let ctx = ClienteCtx::autorizado(&c.id);
        let vacia = organizacion(&db, &ctx).unwrap();
        assert_eq!(vacia, json!({ "sedes": [], "equipos": {}, "destinos": {}, "categorias": [], "orden": [] }));
        for (k, v) in [
            ("sede:sur", json!({ "nombre": "Planta Sur" })),
            ("sede:norte", json!({ "nombre": "Oficina Norte", "notas": "Planta baja", "principal": true })),
            ("sede.equipo:e1", json!({ "sede": "sur" })),
            ("sede.equipo:e2", json!({ "sede": "quitada" })),
            ("sede.destino:zona:a:principal", json!({ "sede": "norte" })),
            ("etiqueta.categoria:tipo", json!({ "nombre": "Tipo", "etiquetas": ["Servidor", "Puesto"] })),
            ("orden.equipos:todos", json!({ "ids": ["e2", "e1"] })),
        ] {
            dcs::registrar(&db, &ctx, k, &v, false, "Ana", "yo").unwrap().unwrap();
        }
        // Una sede quitada (null) no sale.
        dcs::registrar(&db, &ctx, "sede:vieja", &json!({ "nombre": "Vieja" }), false, "Ana", "yo").unwrap();
        dcs::registrar(&db, &ctx, "sede:vieja", &Value::Null, false, "Ana", "yo").unwrap();
        let o = organizacion(&db, &ctx).unwrap();
        assert_eq!(o["sedes"].as_array().unwrap().iter().map(|s| s["id"].as_str().unwrap()).collect::<Vec<_>>(), ["norte", "sur"], "por nombre");
        assert_eq!(o["sedes"][0]["principal"], true);
        assert_eq!(o["sedes"][0]["notas"], "Planta baja");
        assert_eq!(o["equipos"], json!({ "e1": "sur" }), "sin la de una sede que no existe");
        assert_eq!(o["destinos"], json!({ "zona:a:principal": "norte" }));
        assert_eq!(o["categorias"][0]["etiquetas"], json!(["Servidor", "Puesto"]));
        assert_eq!(o["orden"], json!(["e2", "e1"]));
    }
}
