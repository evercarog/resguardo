//! 0.7.26 (bloque 8): los **datos comunes del cliente** en este servidor
//! (docs/consolas-multiples.md §6.5).
//!
//! Los colores y la plantilla de las etiquetas, el catálogo de destinos (nombre,
//! tipo y marcas) y las plantillas de copia viajan por los equipos del cliente
//! (`resguardo_protocolo::datos_cliente`). Aquí:
//!
//! - **Recibir**: cada equipo sube su documento con la configuración. Se junta con
//!   lo de los demás equipos dato a dato (gana el cambio más reciente) y se aplica a
//!   lo de aquí (ajustes de las etiquetas, catálogo, plantillas), con el historial
//!   «desde la consola X» en la auditoría y en el `por` de cada cosa.
//! - **Primera vez** (§6.5, «Diferencias»): si aquí ya había otro valor para ese
//!   dato y nunca se habían juntado, **no se pisa**: queda como diferencia hasta que
//!   alguien elija cuál vale para todas. Lo mismo con una semilla (lo que otra
//!   consola ya tenía) que no coincide con lo de aquí.
//! - **Registrar**: un cambio hecho aquí (en sus pantallas de siempre) queda como
//!   dato propio «por enviar»; la consola web lo manda a los equipos con su orden
//!   (las órdenes las sella el navegador) y lo marca enviado.
//! - **Plantillas**: llegan cifradas por la otra consola (con su sal y su cliente);
//!   aquí no se pueden abrir. Quedan «por traer» hasta que una persona con la clave
//!   de administración las abre en el navegador y las vuelve a cifrar para aquí.

use crate::almacen::{ahora, AjusteEtiqueta, Almacen, ClienteCtx, DestinoCatalogo, R};
use resguardo_protocolo::datos_cliente::{self as dc, Clase, Entrada};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Como mucho, lo que se acepta de un equipo (el documento, en JSON).
pub const MAX_DOCUMENTO: usize = 300 * 1024;

/// En qué está un dato común en este servidor.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Estado {
    /// Lo de aquí es lo que dice el dato.
    #[default]
    Aplicado,
    /// Aquí había otro valor: espera a que una persona elija.
    Conflicto,
    /// Una plantilla de otra consola que aún hay que abrir y volver a cifrar aquí.
    PorTraer,
}

/// Un dato común tal como lo guarda este servidor.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Fila {
    #[serde(flatten)]
    pub entrada: Entrada,
    #[serde(default)]
    pub estado: Estado,
    /// En una diferencia: lo que había aquí cuando llegó.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local: Option<Value>,
    /// Cambiado aquí y aún sin mandar a los equipos.
    #[serde(default, skip_serializing_if = "es_falso")]
    pub por_enviar: bool,
}

fn es_falso(b: &bool) -> bool {
    !*b
}

/// Las filas de este cliente, por clave (las que no se entienden se saltan).
pub fn filas(db: &dyn Almacen, ctx: &ClienteCtx) -> R<BTreeMap<String, Fila>> {
    Ok(db.datos_comunes(ctx)?.into_iter().filter_map(|(k, d)| serde_json::from_str::<Fila>(&d).ok().map(|f| (k, f))).collect())
}

fn guardar(db: &dyn Almacen, ctx: &ClienteCtx, clave: &str, f: &Fila) -> R<()> {
    db.poner_dato_comun(ctx, clave, &serde_json::to_string(f).map_err(|e| e.to_string())?)
}

fn hora_ms(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms).unwrap_or_default().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// Quién cambió un dato, en palabras: «Ana (desde la consola «Oficina»)».
pub fn quien(e: &Entrada) -> String {
    let por = e.por.as_deref().map(|p| dc::texto_corto(p, 60)).filter(|p| !p.is_empty());
    let consola = Some(dc::texto_corto(&e.consola, 60)).filter(|c| !c.is_empty());
    match (por, consola) {
        (Some(p), Some(c)) => format!("{p} (desde la consola «{c}»)"),
        (Some(p), None) => format!("{p} (desde otra consola)"),
        (None, Some(c)) => format!("Desde la consola «{c}»"),
        (None, None) => "Desde otra consola".to_string(),
    }
}

fn ajuste(db: &dyn Almacen, ctx: &ClienteCtx, sujeto: &str) -> R<Option<AjusteEtiqueta>> {
    Ok(db.ajustes_etiquetas(ctx)?.into_iter().find(|a| a.nombre.to_lowercase() == sujeto))
}

fn destino(db: &dyn Almacen, ctx: &ClienteCtx, id: &str) -> R<Option<DestinoCatalogo>> {
    Ok(db.destinos_catalogo(ctx)?.into_iter().find(|d| d.id == id))
}

/// El valor de un dato según lo que hay ahora en este servidor (`null`: nada propio).
/// Una plantilla que existe aquí es `{ "aqui": true }` (no se puede comparar cifrada).
pub fn valor_local(db: &dyn Almacen, ctx: &ClienteCtx, clave: &str) -> R<Value> {
    let Some((c, sujeto)) = dc::clase(clave) else { return Ok(Value::Null) };
    Ok(match c {
        Clase::EtiquetaColor => ajuste(db, ctx, sujeto)?.and_then(|a| a.color.map(|n| json!({ "nombre": a.nombre, "color": n }))).unwrap_or(Value::Null),
        Clase::EtiquetaPlantilla => {
            ajuste(db, ctx, sujeto)?.and_then(|a| a.plantilla.map(|p| json!({ "nombre": a.nombre, "plantilla": p }))).unwrap_or(Value::Null)
        }
        Clase::Destino => match destino(db, ctx, sujeto)? {
            Some(d) if !d.nombre.trim().is_empty() => {
                let mut v = json!({ "nombre": d.nombre, "clase": d.tipo });
                if let Some(w) = d.donde {
                    v["donde"] = json!(w);
                }
                v
            }
            _ => Value::Null,
        },
        Clase::DestinoRegla => destino(db, ctx, sujeto)?
            .and_then(|d| d.atributos.as_deref().and_then(|a| serde_json::from_str::<Value>(a).ok()).map(|a| json!({ "clase": d.tipo, "atributos": a })))
            .unwrap_or(Value::Null),
        Clase::Plantilla => {
            if db.plantillas(ctx)?.iter().any(|p| p.id == sujeto) {
                json!({ "aqui": true })
            } else {
                Value::Null
            }
        }
    })
}

/// ¿Dicen lo mismo el valor de aquí y el del dato? (Una plantilla: si las dos existen o ninguna.)
pub fn iguales(clave: &str, local: &Value, valor: &Value) -> bool {
    if clave.starts_with(Clase::Plantilla.prefijo()) {
        return local.is_null() == valor.is_null();
    }
    local == valor
}

/// Lo que hay que mandar como dato propio con lo que hay aquí (para «compartir»): el
/// valor de aquí, o la plantilla cifrada con la sal y el cliente de aquí.
pub fn valor_para_compartir(db: &dyn Almacen, ctx: &ClienteCtx, clave: &str) -> R<Option<Value>> {
    let Some((c, sujeto)) = dc::clase(clave) else { return Ok(None) };
    if c != Clase::Plantilla {
        return Ok(Some(valor_local(db, ctx, clave)?));
    }
    let Some(p) = db.plantillas(ctx)?.into_iter().find(|p| p.id == sujeto) else { return Ok(Some(Value::Null)) };
    let Some(cli) = db.cliente(ctx.id())? else { return Ok(None) };
    let v = json!({ "cifrado": p.cifrado, "sal": cli.sal_cliente, "cliente": ctx.id() });
    // Una plantilla muy grande (o de un formato que no se entiende) sigue siendo solo de aquí.
    Ok(dc::valor_valido(clave, &v).ok())
}

/// Pone aquí el valor de un dato (sin registrar nada: eso lo hace quien llama).
pub fn aplicar_local(db: &dyn Almacen, ctx: &ClienteCtx, clave: &str, valor: &Value, por: &str) -> R<()> {
    let Some((c, sujeto)) = dc::clase(clave) else { return Ok(()) };
    match c {
        Clase::EtiquetaColor | Clase::EtiquetaPlantilla => {
            let previo = ajuste(db, ctx, sujeto)?;
            let nombre =
                valor["nombre"].as_str().map(str::to_string).or_else(|| previo.as_ref().map(|a| a.nombre.clone())).unwrap_or_else(|| sujeto.to_string());
            let mut a = previo.unwrap_or(AjusteEtiqueta { nombre: nombre.clone(), ..Default::default() });
            a.nombre = nombre;
            a.por = por.to_string();
            if c == Clase::EtiquetaColor {
                a.color = valor["color"].as_u64().and_then(|n| u8::try_from(n).ok());
            } else {
                a.plantilla = valor["plantilla"].as_str().map(str::to_string);
            }
            if a.color.is_none() && a.plantilla.is_none() && a.avisos.is_none() {
                db.borrar_ajuste_etiqueta(ctx, &a.nombre)?;
            } else {
                db.poner_ajuste_etiqueta(ctx, &a, crate::api::etiquetas::MAX_AJUSTES)?;
            }
        }
        Clase::Destino => {
            let previo = destino(db, ctx, sujeto)?;
            let nombre = valor["nombre"].as_str().unwrap_or("").trim().to_string();
            let con_atributos = previo.as_ref().is_some_and(|d| d.atributos.is_some());
            if nombre.is_empty() {
                // Vuelve al nombre de siempre; si tenía marcas, se quedan.
                match previo {
                    Some(d) if con_atributos => {
                        db.guardar_destino(
                            ctx,
                            &DestinoCatalogo { nombre: String::new(), actualizado: ahora(), por: por.to_string(), ..d },
                            crate::api::destinos::MAX_DESTINOS,
                            true,
                        )?;
                    }
                    Some(_) => {
                        db.borrar_destino(ctx, sujeto)?;
                    }
                    None => {}
                }
            } else {
                let tipo = valor["clase"].as_str().map(str::to_string).or_else(|| previo.as_ref().map(|d| d.tipo.clone())).unwrap_or_default();
                let donde = valor["donde"].as_str().map(str::to_string).or_else(|| previo.as_ref().and_then(|d| d.donde.clone()));
                let d = DestinoCatalogo { id: sujeto.to_string(), nombre, tipo, donde, atributos: None, actualizado: ahora(), por: por.to_string() };
                db.guardar_destino(ctx, &d, crate::api::destinos::MAX_DESTINOS, true)?;
            }
        }
        Clase::DestinoRegla => {
            let previo = destino(db, ctx, sujeto)?;
            if valor.is_null() {
                match previo {
                    Some(d) if !d.nombre.trim().is_empty() => {
                        db.guardar_destino(
                            ctx,
                            &DestinoCatalogo { atributos: None, actualizado: ahora(), por: por.to_string(), ..d },
                            crate::api::destinos::MAX_DESTINOS,
                            false,
                        )?;
                    }
                    Some(_) => {
                        db.borrar_destino(ctx, sujeto)?;
                    }
                    None => {}
                }
            } else {
                let atributos = Some(valor["atributos"].to_string());
                let d = match previo {
                    Some(d) => DestinoCatalogo { atributos, actualizado: ahora(), por: por.to_string(), ..d },
                    None => DestinoCatalogo {
                        id: sujeto.to_string(),
                        nombre: String::new(),
                        tipo: valor["clase"].as_str().unwrap_or_default().to_string(),
                        donde: None,
                        atributos,
                        actualizado: ahora(),
                        por: por.to_string(),
                    },
                };
                db.guardar_destino(ctx, &d, crate::api::destinos::MAX_DESTINOS, false)?;
            }
        }
        Clase::Plantilla => {
            // Solo se puede quitar: traerla pide abrirla en el navegador (`traer`).
            if valor.is_null() {
                db.borrar_plantilla(ctx, sujeto)?;
            }
        }
    }
    Ok(())
}

/// Lo que dice un equipo (su documento, `datos_cliente` al subir la configuración),
/// junto con lo de aquí. `propia`: la identidad de este servidor. Devuelve cuántos
/// datos cambiaron aquí (aplicados, diferencias nuevas o plantillas por traer).
pub fn recibir(db: &dyn Almacen, ctx: &ClienteCtx, equipo: &str, propia: &str, doc: &Value) -> R<usize> {
    if doc.to_string().len() > MAX_DOCUMENTO {
        return Ok(0);
    }
    let doc = dc::leer_documento(doc);
    if doc.is_empty() {
        return Ok(0);
    }
    let mut actuales = filas(db, ctx)?;
    let actor = format!("equipo:{equipo}");
    let mut n = 0;
    for (clave, e) in doc {
        let actual = actuales.get(&clave);
        if actual.is_some_and(|f| !dc::gana(&e, &f.entrada)) {
            continue;
        }
        let Some((clase, _)) = dc::clase(&clave) else { continue };
        let en_diferencia = actual.is_some_and(|f| f.estado == Estado::Conflicto);
        let local = valor_local(db, ctx, &clave)?;
        let distinto = !local.is_null() && !iguales(&clave, &local, &e.valor);
        let conflicto = match (clase, actual.is_none()) {
            // Una diferencia sin resolver: otra semilla la deja igual (con su valor nuevo); un
            // cambio de verdad más reciente (alguien eligió en otra consola para todas) la resuelve.
            _ if en_diferencia => e.semilla && distinto,
            // Una plantilla solo choca si otra consola la quitó y aquí sigue (la primera vez).
            (Clase::Plantilla, primera) => primera && e.valor.is_null() && !local.is_null(),
            // La primera vez que se junta este dato: lo de aquí no se pisa sin preguntar.
            (_, true) => distinto,
            // Ya se juntó antes: gana el cambio más reciente, salvo una semilla (lo que otra
            // consola ya tenía) que no coincide con lo de aquí.
            (_, false) => e.semilla && distinto,
        };
        let propio = !e.identidad.is_empty() && e.identidad == propia;
        let datos = json!({ "consola": e.consola, "por": e.por, "desde_equipo": true, "conflicto": conflicto });
        if conflicto {
            let f = Fila { entrada: e, estado: Estado::Conflicto, local: Some(local), por_enviar: false };
            guardar(db, ctx, &clave, &f)?;
            db.auditar(ctx, &actor, "datos_comunes_diferencia", &clave, &datos.to_string())?;
            actuales.insert(clave, f);
            n += 1;
            continue;
        }
        // Una plantilla de otra consola no se puede abrir aquí: queda por traer.
        let estado = if clase == Clase::Plantilla && !e.valor.is_null() && !propio { Estado::PorTraer } else { Estado::Aplicado };
        if !iguales(&clave, &local, &e.valor) {
            aplicar_local(db, ctx, &clave, &e.valor, &quien(&e))?;
            if !propio {
                db.auditar(ctx, &actor, "datos_comunes", &clave, &datos.to_string())?;
            }
        }
        let f = Fila { entrada: e, estado, local: None, por_enviar: false };
        guardar(db, ctx, &clave, &f)?;
        actuales.insert(clave, f);
        n += 1;
    }
    Ok(n)
}

/// Un cambio hecho en este servidor: queda como dato propio por enviar a los equipos.
/// La hora nunca va por detrás de la del dato que hubiera (un reloj atrasado aquí no
/// haría perder el cambio). Con `semilla`, solo si aún no había nada (lo de aquí de
/// antes de compartir). Devuelve la entrada (para la orden) o `None` si no se registró.
pub fn registrar(db: &dyn Almacen, ctx: &ClienteCtx, clave: &str, valor: &Value, semilla: bool, por: &str, propia: &str) -> R<Option<Entrada>> {
    let Ok(valor) = dc::valor_valido(clave, valor) else { return Ok(None) };
    let actuales = filas(db, ctx)?;
    let previa = actuales.get(clave);
    if semilla && previa.is_some() {
        return Ok(None);
    }
    let mut ms = chrono::Utc::now().timestamp_millis();
    if let Some(m) = previa.and_then(|f| dc::milis(&f.entrada.cambiado)) {
        ms = ms.max(m + 1);
    }
    let e = Entrada {
        valor,
        cambiado: hora_ms(ms),
        consola: String::new(),
        identidad: propia.to_string(),
        por: Some(dc::texto_corto(por, 60)).filter(|p| !p.is_empty()),
        semilla,
    };
    guardar(db, ctx, clave, &Fila { entrada: e.clone(), estado: Estado::Aplicado, local: None, por_enviar: true })?;
    Ok(Some(e))
}

/// `registrar` para lo que cambian las pantallas de siempre: con el valor que hay ahora
/// aquí (o `null`), sin que un fallo impida el cambio local.
pub fn registrar_lo_de_aqui(db: &dyn Almacen, ctx: &ClienteCtx, clave: &str, por: &str, propia: &str) {
    // Sin ningún equipo que lo guarde, lo de aquí sigue siendo de aquí (se ofrecerá como
    // semilla, y comparado con lo de las demás, cuando haya uno).
    let r = compartiendo(db, ctx).and_then(|si| match si {
        false => Ok(()),
        true => valor_para_compartir(db, ctx, clave).and_then(|v| match v {
            Some(v) => registrar(db, ctx, clave, &v, false, por, propia).map(|_| ()),
            None => Ok(()),
        }),
    });
    if let Err(e) = r {
        eprintln!("No se pudo apuntar el dato común «{clave}»: {e}");
    }
}

/// ¿Tiene el cliente algún equipo confirmado que guarde los datos comunes?
pub fn compartiendo(db: &dyn Almacen, ctx: &ClienteCtx) -> R<bool> {
    Ok(db
        .equipos(ctx)?
        .iter()
        .any(|e| e.confirmado && e.resumen.as_ref().and_then(|r| r["admite"].as_array()).is_some_and(|a| a.iter().any(|x| x == "datos_cliente"))))
}

/// Lo que se manda a la consola web: una fila con su clave y si es de esta consola.
pub fn vista(clave: &str, f: &Fila, propia: &str) -> Value {
    json!({
        "clave": clave,
        "valor": f.entrada.valor,
        "cambiado": f.entrada.cambiado,
        "consola": (!f.entrada.consola.is_empty()).then_some(&f.entrada.consola),
        "esta": f.entrada.identidad == propia,
        "por": f.entrada.por,
        "semilla": f.entrada.semilla,
        "estado": f.estado,
        "local": f.local,
        "por_enviar": f.por_enviar,
    })
}

/// Lo de aquí que aún no es un dato común (de antes de compartir): para ofrecerlo como
/// semilla. Las plantillas, cifradas como están aquí.
pub fn sin_compartir(db: &dyn Almacen, ctx: &ClienteCtx, ya: &BTreeMap<String, Fila>) -> R<Vec<Value>> {
    let mut claves: Vec<String> = Vec::new();
    for a in db.ajustes_etiquetas(ctx)? {
        if a.color.is_some() {
            claves.extend(dc::clave_color(&a.nombre));
        }
        if a.plantilla.is_some() {
            claves.extend(dc::clave_plantilla_etiqueta(&a.nombre));
        }
    }
    for d in db.destinos_catalogo(ctx)? {
        if !d.nombre.trim().is_empty() {
            claves.push(format!("{}{}", Clase::Destino.prefijo(), d.id));
        }
        if d.atributos.is_some() {
            claves.push(format!("{}{}", Clase::DestinoRegla.prefijo(), d.id));
        }
    }
    for p in db.plantillas(ctx)? {
        claves.push(format!("{}{}", Clase::Plantilla.prefijo(), p.id));
    }
    let mut out = Vec::new();
    for k in claves {
        if ya.contains_key(&k) || dc::clase(&k).is_none() {
            continue;
        }
        if let Some(v) = valor_para_compartir(db, ctx, &k)?.filter(|v| !v.is_null()) {
            out.push(json!({ "clave": k, "valor": v }));
        }
    }
    Ok(out)
}

/// Una plantilla «por traer» ya abierta en el navegador y cifrada otra vez para aquí:
/// se guarda sin volver a repartirla. `false` si ya no está por traer (o cambió).
pub fn traer(db: &dyn Almacen, ctx: &ClienteCtx, clave: &str, cambiado: &str, cifrado: &str, por: &str) -> R<bool> {
    let Some((Clase::Plantilla, id)) = dc::clase(clave) else { return Ok(false) };
    let mut actuales = filas(db, ctx)?;
    let Some(f) = actuales.remove(clave).filter(|f| f.estado == Estado::PorTraer && f.entrada.cambiado == cambiado) else { return Ok(false) };
    if !db.guardar_plantilla(ctx, id, cifrado, por, crate::api::plantillas::MAX_PLANTILLAS)? {
        return Err(format!("Como mucho {} plantillas por cliente: borra alguna.", crate::api::plantillas::MAX_PLANTILLAS));
    }
    guardar(db, ctx, clave, &Fila { estado: Estado::Aplicado, ..f })?;
    Ok(true)
}

/// Las que ya se mandaron a los equipos (con la misma hora): dejan de estar por enviar.
pub fn marcar_enviadas(db: &dyn Almacen, ctx: &ClienteCtx, enviadas: &[(String, String)]) -> R<usize> {
    let actuales = filas(db, ctx)?;
    let mut n = 0;
    for (clave, cambiado) in enviadas {
        if let Some(f) = actuales.get(clave).filter(|f| f.por_enviar && &f.entrada.cambiado == cambiado) {
            guardar(db, ctx, clave, &Fila { por_enviar: false, ..f.clone() })?;
            n += 1;
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::almacen::sqlite::Sqlite;
    use base64::Engine;

    const AQUI: &str = "id-aqui";

    fn almacen() -> (Sqlite, ClienteCtx, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db = Sqlite::abrir(dir.path()).unwrap();
        let c = db.crear_cliente("Panadería Ejemplo", &base64::engine::general_purpose::STANDARD.encode([3u8; 16]), 24).unwrap();
        (db, ClienteCtx::autorizado(&c.id), dir)
    }

    fn entrada(valor: Value, cambiado: &str, identidad: &str, consola: &str) -> Value {
        json!({ "valor": valor, "cambiado": cambiado, "identidad": identidad, "consola": consola, "por": "Ana" })
    }

    fn color(db: &dyn Almacen, ctx: &ClienteCtx, nombre: &str) -> Option<u8> {
        db.ajustes_etiquetas(ctx).unwrap().into_iter().find(|a| a.nombre.to_lowercase() == nombre.to_lowercase()).and_then(|a| a.color)
    }

    fn poner_color(db: &dyn Almacen, ctx: &ClienteCtx, nombre: &str, c: u8) {
        db.poner_ajuste_etiqueta(ctx, &AjusteEtiqueta { nombre: nombre.into(), color: Some(c), ..Default::default() }, 200).unwrap();
    }

    #[test]
    fn llega_un_color_y_se_aplica_con_su_historial() {
        let (db, ctx, dir) = almacen();
        let doc = json!({ "etiqueta.color:servidor": entrada(json!({ "nombre": "Servidor", "color": 1 }), "2026-10-07T10:00:00Z", "id-oficina", "Oficina") });
        assert_eq!(recibir(&db, &ctx, "e1", AQUI, &doc).unwrap(), 1);
        assert_eq!(color(&db, &ctx, "Servidor"), Some(1), "aquí no había nada: se pone");
        let a = db.ajustes_etiquetas(&ctx).unwrap();
        assert_eq!(a[0].por, "Ana (desde la consola «Oficina»)");
        let aud = db.auditoria(&ctx, 0, 50).unwrap();
        assert!(aud.iter().any(|x| x.accion == "datos_comunes" && x.actor == "equipo:e1" && x.datos.contains("Oficina")), "{aud:?}");
        // Lo mismo otra vez (de otro equipo): nada nuevo.
        assert_eq!(recibir(&db, &ctx, "e2", AQUI, &doc).unwrap(), 0);
        // Uno más reciente gana; uno más antiguo (de un equipo atrasado) no.
        let nuevo =
            json!({ "etiqueta.color:servidor": entrada(json!({ "nombre": "Servidor", "color": 4 }), "2026-10-07T11:00:00Z", "id-en-linea", "En línea") });
        recibir(&db, &ctx, "e2", AQUI, &nuevo).unwrap();
        assert_eq!(color(&db, &ctx, "Servidor"), Some(4));
        recibir(&db, &ctx, "e3", AQUI, &doc).unwrap();
        assert_eq!(color(&db, &ctx, "Servidor"), Some(4), "el más antiguo no vuelve");
        // Quitarlo (null): vuelve al automático.
        let quitar = json!({ "etiqueta.color:servidor": entrada(Value::Null, "2026-10-07T12:00:00Z", "id-oficina", "Oficina") });
        recibir(&db, &ctx, "e1", AQUI, &quitar).unwrap();
        assert_eq!(color(&db, &ctx, "Servidor"), None);
        drop(db);
        drop(dir);
    }

    #[test]
    fn la_primera_vez_no_se_pisa_y_se_elige() {
        let (db, ctx, dir) = almacen();
        poner_color(&db, &ctx, "Servidor", 2);
        let otra = json!({ "etiqueta.color:servidor": entrada(json!({ "nombre": "Servidor", "color": 5 }), "2026-10-07T10:00:00Z", "id-oficina", "Oficina") });
        recibir(&db, &ctx, "e1", AQUI, &otra).unwrap();
        assert_eq!(color(&db, &ctx, "Servidor"), Some(2), "no se pisa sin preguntar");
        let f = &filas(&db, &ctx).unwrap()["etiqueta.color:servidor"];
        assert_eq!(f.estado, Estado::Conflicto);
        assert_eq!(f.local, Some(json!({ "nombre": "Servidor", "color": 2 })));
        // Una semilla de una tercera consola (aunque sea más reciente) no gana a un cambio: la
        // diferencia sigue con el valor de la otra.
        let mut s = entrada(json!({ "nombre": "Servidor", "color": 4 }), "2026-10-07T10:30:00Z", "id-tercera", "Sede norte");
        s["semilla"] = json!(true);
        recibir(&db, &ctx, "e1", AQUI, &json!({ "etiqueta.color:servidor": s })).unwrap();
        let f = &filas(&db, &ctx).unwrap()["etiqueta.color:servidor"];
        assert_eq!((f.estado, f.entrada.valor["color"].as_u64()), (Estado::Conflicto, Some(5)));
        // Una semilla tampoco resuelve; un cambio más reciente con el mismo valor que aquí, sí
        // (más abajo). Uno más reciente de la otra con otro valor la mantiene con ese valor nuevo
        // si es semilla; si es un cambio de verdad, la resuelve (alguien eligió para todas).
        let otra2 = json!({ "etiqueta.color:servidor": entrada(json!({ "nombre": "Servidor", "color": 6 }), "2026-10-07T10:40:00Z", "id-oficina", "Oficina") });
        let mut s2 = otra2["etiqueta.color:servidor"].clone();
        s2["semilla"] = json!(true);
        recibir(&db, &ctx, "e1", AQUI, &json!({ "etiqueta.color:servidor": s2 })).unwrap();
        assert_eq!(filas(&db, &ctx).unwrap()["etiqueta.color:servidor"].entrada.valor["color"], 5);
        assert_eq!(color(&db, &ctx, "Servidor"), Some(2));
        // Se elige «el de la otra»: se aplica aquí y queda por enviar a todos, más reciente.
        let elegido = json!({ "nombre": "Servidor", "color": 6 });
        aplicar_local(&db, &ctx, "etiqueta.color:servidor", &elegido, "Luis").unwrap();
        let e = registrar(&db, &ctx, "etiqueta.color:servidor", &elegido, false, "Luis", AQUI).unwrap().unwrap();
        assert!(dc::milis(&e.cambiado).unwrap() > dc::milis("2026-10-07T10:00:00Z").unwrap(), "nunca por detrás del dato que había");
        let f = &filas(&db, &ctx).unwrap()["etiqueta.color:servidor"];
        assert_eq!((f.estado, f.por_enviar), (Estado::Aplicado, true));
        assert_eq!(color(&db, &ctx, "Servidor"), Some(6));
        // Su propio dato de vuelta (por un equipo): nada.
        let vuelta = json!({ "etiqueta.color:servidor": serde_json::to_value(&e).unwrap() });
        assert_eq!(recibir(&db, &ctx, "e1", AQUI, &vuelta).unwrap(), 0);
        assert_eq!(marcar_enviadas(&db, &ctx, &[("etiqueta.color:servidor".into(), e.cambiado.clone())]).unwrap(), 1);
        assert!(!filas(&db, &ctx).unwrap()["etiqueta.color:servidor"].por_enviar);
        // Ya juntado: un cambio más reciente de la otra se aplica sin preguntar.
        let otra3 = json!({ "etiqueta.color:servidor": entrada(json!({ "nombre": "Servidor", "color": 0 }), "2099-01-01T00:00:00Z", "id-oficina", "Oficina") });
        recibir(&db, &ctx, "e1", AQUI, &otra3).unwrap();
        assert_eq!(color(&db, &ctx, "Servidor"), Some(0));
        // Otra diferencia que alguien resuelve en otra consola (un cambio de verdad, más
        // reciente): se resuelve también aquí.
        poner_color(&db, &ctx, "Sede", 1);
        let otra = json!({ "etiqueta.color:sede": entrada(json!({ "nombre": "Sede", "color": 3 }), "2026-10-07T10:00:00Z", "id-oficina", "Oficina") });
        recibir(&db, &ctx, "e1", AQUI, &otra).unwrap();
        assert_eq!(filas(&db, &ctx).unwrap()["etiqueta.color:sede"].estado, Estado::Conflicto);
        let elegida = json!({ "etiqueta.color:sede": entrada(json!({ "nombre": "Sede", "color": 3 }), "2026-10-07T12:00:00Z", "id-oficina", "Oficina") });
        recibir(&db, &ctx, "e1", AQUI, &elegida).unwrap();
        assert_eq!(filas(&db, &ctx).unwrap()["etiqueta.color:sede"].estado, Estado::Aplicado);
        assert_eq!(color(&db, &ctx, "Sede"), Some(3));
        drop(db);
        drop(dir);
    }

    #[test]
    fn una_semilla_distinta_es_diferencia_aunque_ya_se_juntara() {
        let (db, ctx, dir) = almacen();
        poner_color(&db, &ctx, "Servidor", 2);
        registrar(&db, &ctx, "etiqueta.color:servidor", &json!({ "nombre": "Servidor", "color": 2 }), true, "Ana", AQUI).unwrap().unwrap();
        // Otra semilla (lo que tenía la otra consola), más reciente: no gana sin preguntar.
        let mut s = entrada(json!({ "nombre": "Servidor", "color": 3 }), "2099-01-01T00:00:00Z", "id-oficina", "Oficina");
        s["semilla"] = json!(true);
        recibir(&db, &ctx, "e1", AQUI, &json!({ "etiqueta.color:servidor": s })).unwrap();
        assert_eq!(color(&db, &ctx, "Servidor"), Some(2));
        assert_eq!(filas(&db, &ctx).unwrap()["etiqueta.color:servidor"].estado, Estado::Conflicto);
        // Una semilla solo se registra si no había nada.
        assert!(registrar(&db, &ctx, "etiqueta.color:servidor", &json!({ "nombre": "Servidor", "color": 2 }), true, "Ana", AQUI).unwrap().is_none());
        drop(db);
        drop(dir);
    }

    #[test]
    fn catalogo_y_marcas() {
        let (db, ctx, dir) = almacen();
        let doc = json!({
            "destino:zona:e9:principal": entrada(json!({ "nombre": "Almacén · Disco D", "clase": "zona" }), "2026-10-07T10:00:00Z", "id-oficina", "Oficina"),
            "destino.regla:zona:e9:principal": entrada(json!({ "clase": "zona", "atributos": { "tipo": "fuera", "aislado": true } }), "2026-10-07T10:00:00Z", "id-oficina", "Oficina"),
        });
        assert_eq!(recibir(&db, &ctx, "e1", AQUI, &doc).unwrap(), 2);
        let d = db.destinos_catalogo(&ctx).unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!((d[0].nombre.as_str(), d[0].tipo.as_str()), ("Almacén · Disco D", "zona"));
        assert_eq!(serde_json::from_str::<Value>(d[0].atributos.as_deref().unwrap()).unwrap(), json!({ "tipo": "fuera", "aislado": true }));
        // Quitar las marcas deja el nombre; quitar el nombre (sin marcas) lo quita del catálogo.
        recibir(&db, &ctx, "e1", AQUI, &json!({ "destino.regla:zona:e9:principal": entrada(Value::Null, "2026-10-07T11:00:00Z", "id-oficina", "Oficina") }))
            .unwrap();
        assert!(db.destinos_catalogo(&ctx).unwrap()[0].atributos.is_none());
        recibir(&db, &ctx, "e1", AQUI, &json!({ "destino:zona:e9:principal": entrada(Value::Null, "2026-10-07T11:00:00Z", "id-oficina", "Oficina") })).unwrap();
        assert!(db.destinos_catalogo(&ctx).unwrap().is_empty());
        // Un nombre distinto aquí la primera vez: diferencia.
        let propio = DestinoCatalogo {
            id: "destino-1".into(),
            nombre: "Nube norte".into(),
            tipo: "b2".into(),
            donde: Some("copias-norte".into()),
            atributos: None,
            actualizado: 0,
            por: "Ana".into(),
        };
        db.guardar_destino(&ctx, &propio, 200, false).unwrap();
        recibir(&db, &ctx, "e1", AQUI, &json!({ "destino:destino-1": entrada(json!({ "nombre": "Nube sur", "clase": "b2", "donde": "copias-norte" }), "2026-10-07T10:00:00Z", "id-oficina", "Oficina") })).unwrap();
        assert_eq!(db.destinos_catalogo(&ctx).unwrap()[0].nombre, "Nube norte");
        assert_eq!(filas(&db, &ctx).unwrap()["destino:destino-1"].estado, Estado::Conflicto);
        drop(db);
        drop(dir);
    }

    #[test]
    fn plantillas_por_traer_y_sin_compartir() {
        let (db, ctx, dir) = almacen();
        let b64 = |n: usize| base64::engine::general_purpose::STANDARD.encode(vec![9u8; n]);
        let p = json!({ "cifrado": b64(80), "sal": b64(16), "cliente": "c-otra" });
        recibir(&db, &ctx, "e1", AQUI, &json!({ "plantilla:pla-1": entrada(p, "2026-10-07T10:00:00Z", "id-oficina", "Oficina") })).unwrap();
        let f = filas(&db, &ctx).unwrap()["plantilla:pla-1"].clone();
        assert_eq!(f.estado, Estado::PorTraer, "aquí no se puede abrir");
        assert!(db.plantillas(&ctx).unwrap().is_empty());
        // Traída (abierta y cifrada otra vez en el navegador): se guarda y no se vuelve a repartir.
        assert!(!traer(&db, &ctx, "plantilla:pla-1", "otra hora", &b64(90), "Ana").unwrap());
        assert!(traer(&db, &ctx, "plantilla:pla-1", &f.entrada.cambiado, &b64(90), "Ana").unwrap());
        let f = filas(&db, &ctx).unwrap()["plantilla:pla-1"].clone();
        assert_eq!((f.estado, f.por_enviar), (Estado::Aplicado, false));
        assert_eq!(db.plantillas(&ctx).unwrap().len(), 1);
        // Borrada en la otra: se borra aquí.
        recibir(&db, &ctx, "e1", AQUI, &json!({ "plantilla:pla-1": entrada(Value::Null, "2026-10-07T11:00:00Z", "id-oficina", "Oficina") })).unwrap();
        assert!(db.plantillas(&ctx).unwrap().is_empty());
        // Lo de aquí sin compartir: colores, destinos y plantillas (cifradas con la sal de aquí).
        poner_color(&db, &ctx, "Servidor", 1);
        db.guardar_plantilla(&ctx, "pla-2", &b64(100), "Ana", 100).unwrap();
        let ya = filas(&db, &ctx).unwrap();
        let s = sin_compartir(&db, &ctx, &ya).unwrap();
        let claves: Vec<&str> = s.iter().filter_map(|x| x["clave"].as_str()).collect();
        assert_eq!(claves, ["etiqueta.color:servidor", "plantilla:pla-2"]);
        assert_eq!(s[1]["valor"]["cliente"], ctx.id());
        drop(db);
        drop(dir);
    }

    #[test]
    fn lo_que_no_vale_se_ignora() {
        let (db, ctx, dir) = almacen();
        let doc = json!({
            "etiqueta.color:servidor": entrada(json!({ "nombre": "Servidor", "color": 99 }), "2026-10-07T10:00:00Z", "x", "Oficina"),
            "inventado:x": entrada(json!(1), "2026-10-07T10:00:00Z", "x", "Oficina"),
            "destino:d1": entrada(json!({ "nombre": "x", "clase": "rest", "donde": "https://ana:secreta@nas.ejemplo.com" }), "2026-10-07T10:00:00Z", "x", "Oficina"),
        });
        assert_eq!(recibir(&db, &ctx, "e1", AQUI, &doc).unwrap(), 0);
        assert!(filas(&db, &ctx).unwrap().is_empty());
        let grande = json!({ "x": "a".repeat(MAX_DOCUMENTO) });
        assert_eq!(recibir(&db, &ctx, "e1", AQUI, &grande).unwrap(), 0);
        drop(db);
        drop(dir);
    }
}
