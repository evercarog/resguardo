//! Catálogo de destinos de un cliente (tarea 7a, docs/copias-en-cadena.md).
//!
//! Un destino («Almacén · Disco E», «Backblaze B2 · copias-sur») se puede
//! crear sin crear un repositorio y renombrar cuando se quiera. Aquí solo se
//! guarda lo que lo describe, **en claro**: nombre, tipo y, en un destino de
//! red, el servidor o el bucket (lo mismo que ya dicen los resúmenes de los
//! equipos en `destinos[].donde`). **Nunca** credenciales ni rutas de carpetas
//! locales: las credenciales van selladas para cada equipo en la orden que las
//! usa, como siempre. Por eso este catálogo rechaza cualquier campo de más.
//! Cambiarlo no manda órdenes ni toca ningún equipo.

use super::fecha;
use crate::almacen::{DestinoCatalogo, Rol};
use crate::auth::Usuario;
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Como mucho, destinos por cliente en el catálogo.
pub const MAX_DESTINOS: usize = 200;

/// Los tipos que admite el catálogo.
pub const TIPOS: [&str; 7] = ["zona", "rest", "s3", "b2", "sftp", "nube", "local"];

/// Id: letras minúsculas, cifras y `:_.-`, hasta 120 (`zona:<equipo>:<zona>`, `destino-1a2b3c4d`…).
pub fn id_valido(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 120
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, ':' | '_' | '.' | '-'))
        && !id.split(':').any(|p| p.is_empty() || p == "." || p == "..")
}

/// Nombre: 1 a 80 caracteres, sin caracteres de control.
fn nombre_valido(n: &str) -> bool {
    let n = n.trim();
    !n.is_empty() && n.chars().count() <= 80 && !n.chars().any(char::is_control)
}

/// ¿Parece una ruta de una carpeta local o de red? (`C:\…`, `\\nas\…`, `/srv/…`, `~/…`).
fn parece_ruta_local(d: &str) -> bool {
    let b = d.as_bytes();
    (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && !d.contains("://")) || d.starts_with('\\') || d.starts_with('/') || d.starts_with('~')
}

/// `donde` de un destino de red: servidor o bucket, sin credenciales ni rutas locales.
pub fn donde_valido(tipo: &str, donde: Option<&str>) -> Result<Option<String>, &'static str> {
    let d = donde.map(str::trim).filter(|d| !d.is_empty());
    match tipo {
        "rest" | "s3" | "b2" | "sftp" => {}
        _ if d.is_none() => return Ok(None),
        _ => return Err("Solo los destinos de red (rest-server, S3, B2, SFTP) llevan dirección."),
    }
    let Some(d) = d else { return Err("Falta la dirección del destino (servidor o bucket).") };
    if d.chars().count() > 300 || d.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("Dirección no válida (hasta 300 caracteres, sin espacios).");
    }
    // `https://usuario:clave@host` o `usuario@host:clave`: nada de credenciales.
    let sin_esquema = d.split_once("://").map_or(d, |(_, r)| r);
    let autoridad = sin_esquema.split('/').next().unwrap_or_default();
    if autoridad.contains('@') && (tipo != "sftp" || autoridad.split_once('@').is_some_and(|(u, _)| u.contains(':'))) {
        return Err("La dirección no puede llevar usuario ni contraseña: las credenciales van selladas para cada equipo.");
    }
    if parece_ruta_local(d) {
        return Err("Una carpeta local no va en el catálogo: se elige al crear el repositorio en su equipo.");
    }
    Ok(Some(d.to_string()))
}

/// Tarea 8 (docs/regla-3-2-1.md): dónde está un destino.
pub const LUGARES: [&str; 4] = ["este_equipo", "oficina", "otra_sede", "nube"];
/// Tarea 8: si es inmutable (o está fuera del alcance de los equipos).
pub const INMUTABLES: [&str; 5] = ["solo_anadir", "object_lock", "instantaneas", "desconectado", "no"];

/// 0.7.26 (bloque 2): el tipo de un destino.
pub const TIPOS_DESTINO: [&str; 3] = ["local", "fuera", "nube"];

/// Tarea 8: lo que dice la persona de un destino para la regla 3-2-1-1-0. Solo estos campos.
/// 0.7.26: también `tipo`, `aislado`, `bloqueo_dias` y `aislado_dias` (opcionales).
#[derive(Deserialize, Serialize, Default, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Atributos {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lugar: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    inmutable: Option<String>,
    /// Un nombre para el soporte («USB rotado»): dos destinos con el mismo cuentan una vez.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    soporte: Option<String>,
    /// 0.7.26: `local`, `fuera` o `nube`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tipo: Option<String>,
    /// 0.7.26: marca «Aislado» (un medio que se desconecta y se rota).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    aislado: Option<bool>,
    /// 0.7.26: días del bloqueo de objetos (1 a 36 500).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bloqueo_dias: Option<u32>,
    /// 0.7.26: días sin conectarse tras los que avisa un medio aislado (1 a 365).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    aislado_dias: Option<u32>,
}

/// Los atributos, validados, en JSON (`None` si no dicen nada).
pub fn atributos_validos(a: Atributos) -> Result<Option<String>, &'static str> {
    if a.lugar.as_deref().is_some_and(|l| !LUGARES.contains(&l)) {
        return Err("Lugar no válido: este_equipo, oficina, otra_sede o nube.");
    }
    if a.inmutable.as_deref().is_some_and(|i| !INMUTABLES.contains(&i)) {
        return Err("Inmutable no válido: solo_anadir, object_lock, instantaneas, desconectado o no.");
    }
    if a.tipo.as_deref().is_some_and(|t| !TIPOS_DESTINO.contains(&t)) {
        return Err("Tipo no válido: local, fuera o nube.");
    }
    if a.bloqueo_dias.is_some_and(|d| !(1..=36_500).contains(&d)) {
        return Err("Días de bloqueo: de 1 a 36 500.");
    }
    if a.aislado_dias.is_some_and(|d| !(1..=365).contains(&d)) {
        return Err("Días sin conectarse: de 1 a 365.");
    }
    let soporte = a.soporte.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    if soporte.as_deref().is_some_and(|s| s.chars().count() > 60 || s.chars().any(char::is_control)) {
        return Err("El soporte: hasta 60 caracteres, sin caracteres de control.");
    }
    let a = Atributos { soporte, ..a };
    Ok((a != Atributos::default()).then(|| serde_json::to_string(&a).unwrap_or_default()))
}

/// `GET /api/clientes/{c}/destinos` (cualquier miembro): `[{ id, nombre, tipo, donde, atributos, actualizado, por }]`.
pub async fn listar(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Lectura).await?;
    let l = st.db(move |db| db.destinos_catalogo(&ctx)).await?;
    Ok(Json(json!(l
        .iter()
        .map(|d| {
            let atributos = d.atributos.as_deref().and_then(|a| serde_json::from_str::<Value>(a).ok()).unwrap_or(Value::Null);
            json!({ "id": d.id, "nombre": d.nombre, "tipo": d.tipo, "donde": d.donde, "atributos": atributos, "actualizado": fecha(d.actualizado), "por": d.por })
        })
        .collect::<Vec<_>>())))
}

/// El cuerpo de `PUT`: solo estos campos (cualquier otro, p. ej. un secreto, se rechaza).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Guardar {
    nombre: String,
    tipo: String,
    #[serde(default)]
    donde: Option<String>,
    /// Tarea 8. Sin el campo, se quedan los que había; `null` o `{}`, se quitan.
    #[serde(default)]
    atributos: Option<Atributos>,
}

/// `PUT /api/clientes/{c}/destinos/{id}` (administrador): crea o sustituye.
pub async fn guardar(State(st): State<St>, u: Usuario, Path((c, id)): Path<(String, String)>, Json(g): Json<Value>) -> Res<StatusCode> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    if !id_valido(&id) {
        return Err(ErrorApi::datos("Id de destino no válido."));
    }
    // Sin `atributos` (una consola anterior), los que ya tuviera se quedan.
    let mantener = g.get("atributos").is_none();
    let g: Guardar = serde_json::from_value(g)
        .map_err(|_| ErrorApi::datos("Destino no válido: solo nombre, tipo, dirección y atributos (las credenciales nunca van al servidor)."))?;
    let atributos = match g.atributos {
        Some(a) => atributos_validos(a).map_err(ErrorApi::datos)?,
        None => None,
    };
    // Sin nombre propio solo si se marca algo para la regla (sigue con el de siempre).
    if !(nombre_valido(&g.nombre) || (g.nombre.trim().is_empty() && atributos.is_some())) {
        return Err(ErrorApi::datos("Escribe un nombre para el destino (hasta 80 caracteres)."));
    }
    if !TIPOS.contains(&g.tipo.as_str()) {
        return Err(ErrorApi::datos("Tipo de destino no válido."));
    }
    let donde = donde_valido(&g.tipo, g.donde.as_deref()).map_err(ErrorApi::datos)?;
    let d = DestinoCatalogo {
        id: id.clone(),
        nombre: g.nombre.trim().to_string(),
        tipo: g.tipo,
        donde,
        atributos,
        actualizado: crate::almacen::ahora(),
        por: u.0.cuenta.nombre.clone(),
    };
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let mut datos = json!({ "nombre": d.nombre, "tipo": d.tipo });
    if !mantener {
        datos["atributos"] = d.atributos.as_deref().and_then(|a| serde_json::from_str(a).ok()).unwrap_or(Value::Null);
    }
    let datos = datos.to_string();
    let propia = st.identidad_pub.clone();
    let cabe = st
        .db(move |db| {
            let previo = db.destinos_catalogo(&ctx)?.into_iter().find(|x| x.id == id);
            let ok = db.guardar_destino(&ctx, &d, MAX_DESTINOS, mantener)?;
            if ok {
                db.auditar(&ctx, &actor, "guardar_destino", &id, &datos)?;
                // 0.7.26 (bloque 8): el nombre y, si cambiaron, el tipo y las marcas, para las demás consolas.
                let antes = previo.as_ref().map(|p| (p.nombre.trim().to_string(), p.tipo.clone(), p.donde.clone()));
                if antes != Some((d.nombre.clone(), d.tipo.clone(), d.donde.clone())) {
                    crate::datos_comunes::registrar_lo_de_aqui(db, &ctx, &format!("destino:{id}"), &d.por, &propia);
                }
                if !mantener && previo.and_then(|p| p.atributos) != d.atributos {
                    crate::datos_comunes::registrar_lo_de_aqui(db, &ctx, &format!("destino.regla:{id}"), &d.por, &propia);
                }
            }
            Ok(ok)
        })
        .await?;
    if !cabe {
        return Err(ErrorApi::datos(format!("Como mucho {MAX_DESTINOS} destinos en el catálogo: quita alguno.")));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /api/clientes/{c}/destinos/{id}` (administrador). Solo lo quita del
/// catálogo: no toca ningún equipo ni lo que hay en el destino.
pub async fn borrar(State(st): State<St>, u: Usuario, Path((c, id)): Path<(String, String)>) -> Res<StatusCode> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let (por, propia) = (u.0.cuenta.nombre.clone(), st.identidad_pub.clone());
    let existia = st
        .db(move |db| {
            let previo = db.destinos_catalogo(&ctx)?.into_iter().find(|x| x.id == id);
            let ok = db.borrar_destino(&ctx, &id)?;
            if ok {
                db.auditar(&ctx, &actor, "borrar_destino", &id, "{}")?;
                // 0.7.26 (bloque 8): vuelve a lo de siempre también en las demás consolas.
                if previo.as_ref().is_some_and(|p| !p.nombre.trim().is_empty()) {
                    crate::datos_comunes::registrar_lo_de_aqui(db, &ctx, &format!("destino:{id}"), &por, &propia);
                }
                if previo.is_some_and(|p| p.atributos.is_some()) {
                    crate::datos_comunes::registrar_lo_de_aqui(db, &ctx, &format!("destino.regla:{id}"), &por, &propia);
                }
            }
            Ok(ok)
        })
        .await?;
    if !existia {
        return Err(ErrorApi::no_existe());
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_y_direcciones() {
        for bueno in [
            "zona:0b5c1f8e-1d2a-4c3b-9e8f-7a6b5c4d3e2f:principal",
            "zona:0b5c1f8e:z1a2b3c",
            "destino-1a2b3c4d",
            "nube:0b5c1f8e:dropbox-oficina",
            "almacen-0b5c1f8e",
        ] {
            assert!(id_valido(bueno), "{bueno}");
        }
        for malo in ["", "Mayus", "a/b", "zona::x", "a b", "../x", "zona:..:x", &"x".repeat(121)] {
            assert!(!id_valido(malo), "{malo}");
        }
        assert_eq!(donde_valido("b2", Some(" copias-sur ")).unwrap().as_deref(), Some("copias-sur"));
        assert_eq!(donde_valido("rest", Some("https://almacen.ejemplo.com:8000")).unwrap().as_deref(), Some("https://almacen.ejemplo.com:8000"));
        assert_eq!(
            donde_valido("sftp", Some("copias@nas.ejemplo.com:/copias")).unwrap().as_deref(),
            Some("copias@nas.ejemplo.com:/copias"),
            "el usuario de SFTP no es secreto"
        );
        assert!(donde_valido("rest", Some("https://ana:secreta@almacen.ejemplo.com:8000")).is_err(), "sin credenciales");
        assert!(donde_valido("sftp", Some("ana:secreta@nas.ejemplo.com:/x")).is_err());
        assert!(donde_valido("s3", None).is_err(), "falta");
        for local in [r"D:\Copias", r"\\nas\copias", "/srv/copias", "~/copias"] {
            assert!(donde_valido("rest", Some(local)).is_err(), "{local}");
        }
        assert_eq!(donde_valido("zona", None).unwrap(), None);
        assert!(donde_valido("local", Some(r"D:\Copias")).is_err(), "una carpeta local nunca va en el catálogo");
        assert!(donde_valido("b2", Some("dos palabras")).is_err());
        assert!(nombre_valido("Almacén · Disco E") && !nombre_valido(" ") && !nombre_valido("a\u{7}") && !nombre_valido(&"x".repeat(81)));
    }

    #[test]
    fn atributos_de_la_regla() {
        let a = |v: Value| serde_json::from_value::<Atributos>(v);
        assert_eq!(
            atributos_validos(a(json!({ "lugar": "oficina", "inmutable": "instantaneas", "soporte": " USB rotado " })).unwrap()).unwrap().as_deref(),
            Some(r#"{"lugar":"oficina","inmutable":"instantaneas","soporte":"USB rotado"}"#)
        );
        assert_eq!(atributos_validos(a(json!({})).unwrap()).unwrap(), None, "vacío: lo deducido");
        assert_eq!(atributos_validos(a(json!({ "soporte": "  " })).unwrap()).unwrap(), None);
        assert!(atributos_validos(a(json!({ "lugar": "luna" })).unwrap()).is_err());
        assert!(atributos_validos(a(json!({ "inmutable": "si" })).unwrap()).is_err());
        assert!(atributos_validos(a(json!({ "soporte": "x".repeat(61) })).unwrap()).is_err());
        assert!(a(json!({ "lugar": "nube", "clave": "K001" })).is_err(), "nada de campos de más");
        // 0.7.26: tipo y marcas.
        assert_eq!(
            atributos_validos(a(json!({ "tipo": "fuera", "lugar": "otra_sede", "inmutable": "desconectado", "aislado": true, "aislado_dias": 14 })).unwrap())
                .unwrap()
                .as_deref(),
            Some(r#"{"lugar":"otra_sede","inmutable":"desconectado","tipo":"fuera","aislado":true,"aislado_dias":14}"#)
        );
        assert_eq!(
            atributos_validos(a(json!({ "inmutable": "object_lock", "bloqueo_dias": 30 })).unwrap()).unwrap().as_deref(),
            Some(r#"{"inmutable":"object_lock","bloqueo_dias":30}"#)
        );
        assert!(atributos_validos(a(json!({ "tipo": "luna" })).unwrap()).is_err());
        assert!(atributos_validos(a(json!({ "bloqueo_dias": 0 })).unwrap()).is_err());
        assert!(atributos_validos(a(json!({ "aislado_dias": 366 })).unwrap()).is_err());
        assert!(a(json!({ "aislado": "si" })).is_err());
    }
}
