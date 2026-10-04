//! Equipos preparados (v1.17): el instalador del agente «listo para
//! vincular» (Windows) y la línea de Linux, con un código de un solo uso que
//! caduca a las 24 h. Ver crates/protocolo/src/instalador.rs y
//! docs/plataforma.md («Instalador listo: lo que sabe el servidor»).

use super::cuentas::MIEMBRO;
use super::fecha;
use crate::almacen::{ahora, Emparejamiento, Rol};
use crate::auth::Usuario;
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use resguardo_protocolo::instalador::{cola, DatosInstalador};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

/// Lo que dura un código preparado.
pub const CADUCA_S: i64 = 24 * 3600;

#[derive(Deserialize)]
pub struct Preparar {
    nombre: String,
    /// «windows» (descarga el instalador) o «linux» (da la línea con el código).
    so: String,
    /// La dirección con la que los equipos llegan a este servidor (`https://…`).
    servidor: String,
}

/// Lo que se enseña de un preparado en la consola (sin el código).
pub fn preparado_json(e: &Emparejamiento) -> Value {
    let estado = if matches!(e.estado.as_str(), "abierto" | "unido") && e.caduca <= ahora() { "caducado" } else { e.estado.as_str() };
    json!({
        "id": e.id, "nombre": e.nombre, "so": e.so, "estado": estado,
        "caduca": fecha(e.caduca), "creado": fecha(e.creado), "equipo": e.equipo_id,
    })
}

/// `POST /api/clientes/{c}/instaladores` (administrador).
pub async fn preparar(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(p): Json<Preparar>) -> Res<Response> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    super::cabe_otro_equipo(&st, &ctx).await?;
    let nombre = p.nombre.trim().to_string();
    let so = match p.so.as_str() {
        "windows" | "linux" => p.so.clone(),
        _ => return Err(ErrorApi::datos("Sistema no válido («windows» o «linux»).")),
    };
    let servidor = p.servidor.trim().trim_end_matches('/').to_string();
    // Antes de gastar nada: ¿hay instalador que servir?
    let instalador = if so == "windows" {
        let ruta = st.opciones.instalador_agente.clone().ok_or_else(sin_instalador)?;
        Some(tokio::fs::read(&ruta).await.map_err(|_| sin_instalador())?)
    } else {
        None
    };
    let codigo = resguardo_protocolo::mensajes::pairing_code();
    let datos = DatosInstalador {
        v: 1,
        servidor: servidor.clone(),
        huella_ca: st.huella_ca.clone(),
        cliente: ctx.id().to_string(),
        nombre: nombre.clone(),
        codigo: codigo.clone(),
    };
    datos.validar().map_err(ErrorApi::datos)?;
    // Un código cada vez, y no más de 20 por hora y cliente (como los códigos de 15 min).
    if !st.limites.intento(&format!("emparejar:{c}"), 20, Duration::from_secs(3600)) {
        return Err(ErrorApi::demasiados());
    }
    let hash = resguardo_protocolo::mensajes::code_hash(&codigo);
    let id = uuid::Uuid::new_v4().to_string();
    let caduca = ahora() + CADUCA_S;
    let (id2, por, actor, n2, so2, cod2) =
        (id.clone(), u.id().to_string(), format!("cuenta:{}", u.0.cuenta.correo), nombre.clone(), so.clone(), codigo.clone());
    st.db(move |db| {
        db.preparar_emparejamiento(&ctx, &id2, &por, caduca, &n2, &so2, &cod2)?;
        db.indexar_codigo(&hash, ctx.id(), &id2, caduca)?;
        db.auditar(&ctx, &actor, "preparar_equipo", &id2, &json!({ "nombre": n2, "so": so2 }).to_string())
    })
    .await?;
    match instalador {
        None => Ok(Json(
            json!({ "id": id, "nombre": nombre, "so": so, "codigo": codigo, "caduca": fecha(caduca), "servidor": servidor, "huella_ca": st.huella_ca }),
        )
        .into_response()),
        Some(mut exe) => {
            exe.extend(cola(&datos).map_err(ErrorApi::datos)?);
            let archivo = nombre_archivo(&cliente_nombre(&st, &c).await, &nombre);
            let mut r = Response::new(Body::from(exe));
            let h = r.headers_mut();
            h.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/vnd.microsoft.portable-executable"));
            if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{archivo}\"")) {
                h.insert(header::CONTENT_DISPOSITION, v);
            }
            if let Ok(v) = HeaderValue::from_str(&id) {
                h.insert("x-resguardo-emparejamiento", v);
            }
            if let Ok(v) = HeaderValue::from_str(&fecha(caduca)) {
                h.insert("x-resguardo-caduca", v);
            }
            Ok(r)
        }
    }
}

fn sin_instalador() -> ErrorApi {
    ErrorApi::nuevo(
        StatusCode::NOT_FOUND,
        "sin_instalador",
        "Este servidor no tiene el instalador del agente (se instala con Resguardo Server para Windows). Usa el instalador normal y un código.",
    )
}

async fn cliente_nombre(st: &St, c: &str) -> String {
    let c = c.to_string();
    st.db(move |db| db.cliente(&c)).await.ok().flatten().map(|x| x.nombre).unwrap_or_default()
}

/// «Resguardo-Agente_<cliente>_<equipo>.exe», solo con letras y cifras ASCII, `-` y `_`.
pub fn nombre_archivo(cliente: &str, equipo: &str) -> String {
    let limpio = |s: &str| {
        let t: String = s
            .chars()
            .map(|c| match c {
                'á' | 'à' | 'ä' | 'Á' | 'À' | 'Ä' => 'a',
                'é' | 'è' | 'ë' | 'É' | 'È' | 'Ë' => 'e',
                'í' | 'ì' | 'ï' | 'Í' | 'Ì' | 'Ï' => 'i',
                'ó' | 'ò' | 'ö' | 'Ó' | 'Ò' | 'Ö' => 'o',
                'ú' | 'ù' | 'ü' | 'Ú' | 'Ù' | 'Ü' => 'u',
                'ñ' => 'n',
                'Ñ' => 'N',
                c if c.is_ascii_alphanumeric() || c == '-' || c == '_' => c,
                _ => '-',
            })
            .collect();
        let t = t.split('-').filter(|x| !x.is_empty()).collect::<Vec<_>>().join("-");
        t.chars().take(40).collect::<String>()
    };
    let (c, e) = (limpio(cliente), limpio(equipo));
    match (c.is_empty(), e.is_empty()) {
        (false, false) => format!("Resguardo-Agente_{c}_{e}.exe"),
        (true, false) => format!("Resguardo-Agente_{e}.exe"),
        _ => "Resguardo-Agente.exe".into(),
    }
}

/// ¿Está Resguardo Agente instalado en esta misma máquina? (v1.19)
pub fn agente_local_instalado() -> bool {
    if cfg!(windows) {
        let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into());
        std::path::Path::new(&pf).join("Resguardo Agente").join("resguardo-agente.exe").is_file()
    } else {
        std::path::Path::new("/opt/resguardo-agente/resguardo-agente").is_file()
    }
}

/// `POST /api/clientes/{c}/equipo-local` (administrador, v1.19): «Vincular este
/// servidor». Prepara un emparejamiento (30 min) y deja sus datos, como la cola
/// de un instalador listo y hacia `https://127.0.0.1:<puerto>`, en
/// `vincular-local.json` de la carpeta de datos (solo SYSTEM/administradores o
/// el usuario del servidor). El agente de esta máquina, sin vincular, lo lee y se
/// une; después se compara el número y se da de alta con la clave, como siempre.
pub async fn vincular_local(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let puerto = st.opciones.puerto.filter(|_| !st.huella_ca.is_empty() && agente_local_instalado()).ok_or_else(|| {
        ErrorApi::nuevo(StatusCode::NOT_FOUND, "sin_agente_local", "En esta máquina no está instalado Resguardo Agente (o el servidor no usa su propio TLS).")
    })?;
    super::cabe_otro_equipo(&st, &ctx).await?;
    if !st.limites.intento(&format!("emparejar:{c}"), 20, Duration::from_secs(3600)) {
        return Err(ErrorApi::demasiados());
    }
    let nombre: String = crate::identidad::nombre_equipo()
        .unwrap_or_else(|| "servidor".into())
        .to_uppercase()
        .chars()
        .filter(|c| !c.is_control() && *c != '"')
        .take(80)
        .collect();
    let so = if cfg!(windows) { "windows" } else { "linux" };
    let codigo = resguardo_protocolo::mensajes::pairing_code();
    let datos = DatosInstalador {
        v: 1,
        servidor: format!("https://127.0.0.1:{puerto}"),
        huella_ca: st.huella_ca.clone(),
        cliente: ctx.id().to_string(),
        nombre: nombre.clone(),
        codigo: codigo.clone(),
    };
    datos.validar().map_err(ErrorApi::datos)?;
    let hash = resguardo_protocolo::mensajes::code_hash(&codigo);
    let id = uuid::Uuid::new_v4().to_string();
    let caduca = ahora() + 30 * 60;
    let (id2, por, actor, n2) = (id.clone(), u.id().to_string(), format!("cuenta:{}", u.0.cuenta.correo), nombre.clone());
    st.db(move |db| {
        db.preparar_emparejamiento(&ctx, &id2, &por, caduca, &n2, so, &codigo)?;
        db.indexar_codigo(&hash, ctx.id(), &id2, caduca)?;
        db.auditar(&ctx, &actor, "vincular_servidor_local", &id2, &json!({ "nombre": n2 }).to_string())
    })
    .await?;
    // Se escribe aparte y se renombra: el agente nunca lee un archivo a medias.
    let destino = st.datos.join("vincular-local.json");
    let tmp = st.datos.join("vincular-local.json.tmp");
    let texto = serde_json::to_vec(&datos).map_err(ErrorApi::interno)?;
    tokio::fs::write(&tmp, &texto).await.map_err(ErrorApi::interno)?;
    tokio::fs::rename(&tmp, &destino).await.map_err(ErrorApi::interno)?;
    Ok(Json(json!({ "id": id, "nombre": nombre, "so": so, "estado": "abierto", "caduca": fecha(caduca), "creado": fecha(ahora()), "equipo": null })))
}

/// `GET /api/clientes/{c}/emparejamientos` (administrador): los preparados que aún sirven.
pub async fn listar(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let l = st.db(move |db| db.emparejamientos_preparados(&ctx, ahora())).await?;
    Ok(Json(json!(l.iter().map(preparado_json).collect::<Vec<_>>())))
}

/// Cuántos preparados hay (para el aviso de «Equipos»): cualquier miembro.
pub async fn contar(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, MIEMBRO).await?;
    let l = st.db(move |db| db.emparejamientos_preparados(&ctx, ahora())).await?;
    Ok(Json(json!({ "esperando": l.iter().filter(|e| e.estado == "abierto").count(), "unidos": l.iter().filter(|e| e.estado == "unido").count() })))
}

#[cfg(test)]
mod pruebas {
    use super::nombre_archivo;

    #[test]
    fn nombres_de_archivo_seguros() {
        assert_eq!(nombre_archivo("Altamar", "SERVIDOR-01"), "Resguardo-Agente_Altamar_SERVIDOR-01.exe");
        assert_eq!(nombre_archivo("Clínica Señora", "Recepción 2"), "Resguardo-Agente_Clinica-Senora_Recepcion-2.exe");
        assert_eq!(nombre_archivo("a\"b/../c", "x\r\ny"), "Resguardo-Agente_a-b-c_x-y.exe");
        assert_eq!(nombre_archivo("", "PC"), "Resguardo-Agente_PC.exe");
        assert_eq!(nombre_archivo("", "///"), "Resguardo-Agente.exe");
    }
}
