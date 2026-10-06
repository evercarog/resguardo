//! Equipos preparados (v1.17): el instalador del agente «listo para
//! vincular» (Windows) y la línea de Linux, con un código de un solo uso que
//! caduca a las 24 h. Ver crates/protocolo/src/instalador.rs y
//! docs/plataforma.md («Instalador listo: lo que sabe el servidor»).

use super::cuentas::MIEMBRO;
use super::fecha;
use crate::almacen::{ahora, ClienteCtx, Emparejamiento, Rol};
use crate::auth::Usuario;
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use resguardo_protocolo::instalador::{cola, DatosInstalador};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

/// Lo que dura un código preparado.
pub const CADUCA_S: i64 = 24 * 3600;

/// Códigos nuevos para añadir equipos (de 15 min, preparados y «Vincular este
/// servidor») por cuenta y hora en cada cliente…
pub const MAX_CODIGOS_CUENTA_H: u32 = 30;
/// …y entre todas las cuentas del cliente. Volver a pedir uno que aún sirve
/// (la consola al recargar, el mismo instalador otra vez) no cuenta.
pub const MAX_CODIGOS_CLIENTE_H: u32 = 100;

/// v1.48: lo que se guarda en `emparejamientos.codigo` cuando el código lo generó el
/// navegador: `sha256:<hash>`. El servidor nunca tiene el código; esto solo marca, como
/// el código en claro de antes, que el emparejamiento sigue vivo (hasta el alta del equipo).
pub const PREFIJO_NAVEGADOR: &str = "sha256:";

/// El código en claro, si lo generó el servidor (forma de antes de v1.48).
pub fn codigo_en_claro(e: &Emparejamiento) -> Option<&str> {
    e.codigo.as_deref().filter(|c| !c.starts_with(PREFIJO_NAVEGADOR))
}

/// El hash del código, si lo generó el navegador (v1.48).
pub fn hash_del_navegador(e: &Emparejamiento) -> Option<&str> {
    e.codigo.as_deref().and_then(|c| c.strip_prefix(PREFIJO_NAVEGADOR))
}

/// El `codigo_hash` que manda la consola (SHA-256 en hex del código normalizado, como
/// `protocolo::mensajes::code_hash`), en minúsculas; `None` si no tiene esa forma.
pub fn hash_valido(h: &str) -> Option<String> {
    (h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit())).then(|| h.to_ascii_lowercase())
}

/// 409 si el hash ya está en el índice: el navegador genera otro código y vuelve a pedir.
pub fn codigo_repetido() -> ErrorApi {
    ErrorApi::nuevo(StatusCode::CONFLICT, "codigo_repetido", "Ese código ya existe. Genera otro.")
}

/// Cuenta un código nuevo de la cuenta `cuenta` en el cliente `c`. Pasado el
/// límite, 429 con `retry_after` (segundos hasta que se pueda pedir otro).
/// No es la protección contra probar códigos (esa va por IP al usarlos, en
/// `POST /api/agente/unirse`): esto evita que una cuenta (o una sesión robada)
/// llene el servidor de códigos válidos.
pub fn limite_codigos(st: &St, c: &str, cuenta: &str) -> Res<()> {
    let hora = Duration::from_secs(3600);
    st.limites
        .intento_o_espera(&format!("emparejar:{c}:{cuenta}"), MAX_CODIGOS_CUENTA_H, hora)
        .and_then(|()| st.limites.intento_o_espera(&format!("emparejar:{c}"), MAX_CODIGOS_CLIENTE_H, hora))
        .map_err(|espera| {
            let min = espera.as_secs().div_ceil(60).max(1);
            ErrorApi::demasiados_esperar(
                "codigos",
                format!(
                    "Se han pedido muchos códigos para añadir equipos en poco tiempo. Por seguridad hay un máximo por hora (cada código deja entrar a un equipo nuevo). Podrás pedir otro en {min} min; mientras tanto, usa o anula los que ya tienes."
                ),
                espera,
            )
        })
}

/// Un emparejamiento de esta cuenta que aún sirve y se puede volver a dar en vez de
/// crear otro: abierto, con su código en claro (los del navegador no: el servidor no
/// puede volver a darlo) y al menos `margen_s` segundos por delante.
/// `nombre_so`: `None` para los códigos de 15 min; `Some((nombre, so))` para un preparado
/// con ese nombre (sin distinguir mayúsculas) y sistema.
pub async fn reutilizable(st: &St, ctx: &ClienteCtx, cuenta: &str, nombre_so: Option<(&str, &str)>, margen_s: i64) -> Res<Option<Emparejamiento>> {
    let (ctx, cuenta) = (ctx.clone(), cuenta.to_string());
    let l = st.db(move |db| db.emparejamientos_vigentes_de(&ctx, &cuenta, ahora())).await?;
    let limite = ahora() + margen_s;
    Ok(l.into_iter().find(|e| {
        e.estado == "abierto"
            && e.caduca > limite
            && codigo_en_claro(e).is_some()
            && match nombre_so {
                None => e.nombre.is_none(),
                Some((n, so)) => e.nombre.as_deref().is_some_and(|x| x.to_lowercase() == n.to_lowercase()) && e.so.as_deref() == Some(so),
            }
    }))
}

#[derive(Deserialize)]
pub struct Navegador {
    /// v1.48: `1` si la consola sabe guardar sus propios códigos: entonces también se dan
    /// los generados en el navegador (sin el código, con su hash).
    navegador: Option<String>,
}

/// `GET /api/clientes/{c}/codigo-abierto` (administrador): el código de 15 min que pidió
/// esta cuenta y aún sirve (abierto o ya unido), o `null`. La consola lo vuelve a
/// enseñar al recargar «Añadir equipo» en vez de pedir otro. v1.48: con `?navegador=1`,
/// también uno generado en el navegador: `{ id, codigo: null, codigo_hash, codigo_navegador:
/// true, caduca, estado }` (el código lo tiene ese navegador); sin él, como antes.
pub async fn codigo_abierto(State(st): State<St>, u: Usuario, Path(c): Path<String>, Query(q): Query<Navegador>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let cuenta = u.id().to_string();
    let con_navegador = q.navegador.as_deref() == Some("1");
    let l = st.db(move |db| db.emparejamientos_vigentes_de(&ctx, &cuenta, ahora())).await?;
    Ok(Json(match l.iter().find(|e| e.nombre.is_none() && (con_navegador || codigo_en_claro(e).is_some())) {
        Some(e) => match hash_del_navegador(e) {
            Some(h) => json!({ "id": e.id, "codigo": null, "codigo_hash": h, "codigo_navegador": true, "caduca": fecha(e.caduca), "estado": e.estado }),
            None => json!({ "id": e.id, "codigo": e.codigo, "caduca": fecha(e.caduca), "estado": e.estado }),
        },
        None => Value::Null,
    }))
}

#[derive(Deserialize)]
pub struct Preparar {
    nombre: String,
    /// «windows» (descarga el instalador) o «linux» (da la línea con el código).
    so: String,
    /// La dirección con la que los equipos llegan a este servidor (`https://…`).
    servidor: String,
    /// v1.48: el código lo generó el navegador y solo manda su hash. Entonces la respuesta es
    /// siempre JSON, sin código ni instalador: la consola arma la línea de Linux o la cola del
    /// instalador (que baja aparte con `GET …/instalador-agente`).
    #[serde(default)]
    codigo_hash: Option<String>,
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
    if let Some(h) = p.codigo_hash.as_deref() {
        let hash = hash_valido(h).ok_or_else(|| ErrorApi::datos("Código no válido."))?;
        return preparar_del_navegador(&st, &u, &c, ctx, nombre, so, servidor, hash).await;
    }
    // Antes de gastar nada: ¿hay instalador que servir?
    let instalador = if so == "windows" {
        let ruta = st.opciones.instalador_agente.clone().ok_or_else(sin_instalador)?;
        Some(tokio::fs::read(&ruta).await.map_err(|_| sin_instalador())?)
    } else {
        None
    };
    // Si esta cuenta ya preparó este equipo y su código aún sirve (le quedan más de 2 h),
    // se vuelve a dar el mismo: descargar otra vez no gasta códigos ni cuenta en el límite.
    let previo = reutilizable(&st, &ctx, u.id(), Some((&nombre, &so)), 2 * 3600).await?;
    let reutilizado = previo.is_some();
    let (id, codigo, caduca) = match previo {
        Some(e) => (e.id.clone(), codigo_en_claro(&e).unwrap_or_default().to_string(), e.caduca),
        None => (uuid::Uuid::new_v4().to_string(), resguardo_protocolo::mensajes::pairing_code(), ahora() + CADUCA_S),
    };
    let datos = DatosInstalador {
        v: 1,
        servidor: servidor.clone(),
        huella_ca: st.huella_ca.clone(),
        cliente: ctx.id().to_string(),
        nombre: nombre.clone(),
        codigo: codigo.clone(),
    };
    datos.validar().map_err(ErrorApi::datos)?;
    if !reutilizado {
        limite_codigos(&st, &c, u.id())?;
        let hash = resguardo_protocolo::mensajes::code_hash(&codigo);
        let (id2, por, actor, n2, so2, cod2) =
            (id.clone(), u.id().to_string(), format!("cuenta:{}", u.0.cuenta.correo), nombre.clone(), so.clone(), codigo.clone());
        st.db(move |db| {
            db.preparar_emparejamiento(&ctx, &id2, &por, caduca, &n2, &so2, &cod2)?;
            db.indexar_codigo(&hash, ctx.id(), &id2, caduca)?;
            db.auditar(&ctx, &actor, "preparar_equipo", &id2, &json!({ "nombre": n2, "so": so2 }).to_string())
        })
        .await?;
    }
    match instalador {
        None => Ok(Json(json!({
            "id": id, "nombre": nombre, "so": so, "codigo": codigo, "caduca": fecha(caduca), "servidor": servidor, "huella_ca": st.huella_ca,
            "reutilizado": reutilizado,
        }))
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
            if reutilizado {
                h.insert("x-resguardo-reutilizado", HeaderValue::from_static("1"));
            }
            Ok(r)
        }
    }
}

/// Un código de este formato (16 letras y cifras en 4 grupos) para validar los datos de la
/// cola cuando el código lo tiene el navegador: el resto de campos se comprueba igual.
const CODIGO_DE_FORMA: &str = "AAAA-AAAA-AAAA-AAAA";

/// `POST …/instaladores` con `codigo_hash` (v1.48): el código lo generó el navegador. Se
/// guarda solo su hash (para `POST /api/agente/unirse`) y se devuelve lo que la consola
/// necesita para armar la línea de Linux o la cola del instalador: nunca el código. No se
/// reutiliza un preparado anterior (el servidor no podría dar su código).
#[allow(clippy::too_many_arguments)]
async fn preparar_del_navegador(st: &St, u: &Usuario, c: &str, ctx: ClienteCtx, nombre: String, so: String, servidor: String, hash: String) -> Res<Response> {
    // Windows: antes de gastar nada, ¿hay instalador que servir? (la consola lo baja aparte).
    if so == "windows" && !st.opciones.instalador_agente.as_ref().is_some_and(|r| r.is_file()) {
        return Err(sin_instalador());
    }
    let datos = DatosInstalador {
        v: 1,
        servidor: servidor.clone(),
        huella_ca: st.huella_ca.clone(),
        cliente: ctx.id().to_string(),
        nombre: nombre.clone(),
        codigo: CODIGO_DE_FORMA.into(),
    };
    datos.validar().map_err(ErrorApi::datos)?;
    limite_codigos(st, c, u.id())?;
    let id = uuid::Uuid::new_v4().to_string();
    let caduca = ahora() + CADUCA_S;
    let (id2, por, actor, n2, so2) = (id.clone(), u.id().to_string(), format!("cuenta:{}", u.0.cuenta.correo), nombre.clone(), so.clone());
    let marca = format!("{PREFIJO_NAVEGADOR}{hash}");
    let nuevo = st
        .db(move |db| {
            if db.codigo_indexado(&hash)? {
                return Ok(false);
            }
            db.preparar_emparejamiento(&ctx, &id2, &por, caduca, &n2, &so2, &marca)?;
            db.indexar_codigo(&hash, ctx.id(), &id2, caduca)?;
            db.auditar(&ctx, &actor, "preparar_equipo", &id2, &json!({ "nombre": n2, "so": so2, "codigo_navegador": true }).to_string())?;
            Ok(true)
        })
        .await?;
    if !nuevo {
        return Err(codigo_repetido());
    }
    Ok(Json(json!({
        "id": id, "nombre": nombre, "so": so, "caduca": fecha(caduca), "servidor": servidor, "huella_ca": st.huella_ca,
        "cliente": datos.cliente, "codigo_navegador": true, "reutilizado": false,
    }))
    .into_response())
}

/// `GET /api/clientes/{c}/instalador-agente` (administrador, v1.48): el instalador genérico
/// del agente, sin cola. La consola le añade la cola con el código que generó ella
/// (crates/protocolo/src/instalador.rs): el servidor no la ve. 404 `sin_instalador`.
pub async fn instalador_generico(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Response> {
    u.miembro(&st, &c, Rol::Administrador).await?;
    let ruta = st.opciones.instalador_agente.clone().ok_or_else(sin_instalador)?;
    let exe = tokio::fs::read(&ruta).await.map_err(|_| sin_instalador())?;
    let mut r = Response::new(Body::from(exe));
    let h = r.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/vnd.microsoft.portable-executable"));
    h.insert(header::CONTENT_DISPOSITION, HeaderValue::from_static("attachment; filename=\"Resguardo-Agente-setup.exe\""));
    Ok(r)
}

fn sin_instalador() -> ErrorApi {
    ErrorApi::nuevo(
        StatusCode::NOT_FOUND,
        "sin_instalador",
        "Este servidor no tiene el instalador del agente (viene con Resguardo Server para Windows; en Linux: sudo resguardo-server poner-instalador-agente <Resguardo-Agente-setup.exe>). Usa el instalador normal y un código.",
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
    let nombre: String = crate::identidad::nombre_equipo()
        .unwrap_or_else(|| "servidor".into())
        .to_uppercase()
        .chars()
        .filter(|c| !c.is_control() && *c != '"')
        .take(80)
        .collect();
    let so = if cfg!(windows) { "windows" } else { "linux" };
    // El mismo que antes si aún sirve (más de 5 min): pulsar otra vez no gasta códigos.
    let previo = reutilizable(&st, &ctx, u.id(), Some((&nombre, so)), 5 * 60).await?;
    let reutilizado = previo.is_some();
    let (id, codigo, caduca, creado) = match previo {
        Some(e) => (e.id.clone(), codigo_en_claro(&e).unwrap_or_default().to_string(), e.caduca, e.creado),
        None => {
            limite_codigos(&st, &c, u.id())?;
            (uuid::Uuid::new_v4().to_string(), resguardo_protocolo::mensajes::pairing_code(), ahora() + 30 * 60, ahora())
        }
    };
    let datos = DatosInstalador {
        v: 1,
        servidor: format!("https://127.0.0.1:{puerto}"),
        huella_ca: st.huella_ca.clone(),
        cliente: ctx.id().to_string(),
        nombre: nombre.clone(),
        codigo: codigo.clone(),
    };
    datos.validar().map_err(ErrorApi::datos)?;
    if !reutilizado {
        let hash = resguardo_protocolo::mensajes::code_hash(&codigo);
        let (id2, por, actor, n2) = (id.clone(), u.id().to_string(), format!("cuenta:{}", u.0.cuenta.correo), nombre.clone());
        st.db(move |db| {
            db.preparar_emparejamiento(&ctx, &id2, &por, caduca, &n2, so, &codigo)?;
            db.indexar_codigo(&hash, ctx.id(), &id2, caduca)?;
            db.auditar(&ctx, &actor, "vincular_servidor_local", &id2, &json!({ "nombre": n2 }).to_string())
        })
        .await?;
    }
    // Se escribe aparte y se renombra: el agente nunca lee un archivo a medias.
    let destino = st.datos.join("vincular-local.json");
    let tmp = st.datos.join("vincular-local.json.tmp");
    let texto = serde_json::to_vec(&datos).map_err(ErrorApi::interno)?;
    tokio::fs::write(&tmp, &texto).await.map_err(ErrorApi::interno)?;
    tokio::fs::rename(&tmp, &destino).await.map_err(ErrorApi::interno)?;
    Ok(Json(
        json!({ "id": id, "nombre": nombre, "so": so, "estado": "abierto", "caduca": fecha(caduca), "creado": fecha(creado), "equipo": null, "reutilizado": reutilizado }),
    ))
}

/// `GET /api/clientes/{c}/emparejamientos` (administrador): los preparados que aún sirven.
pub async fn listar(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let l = st.db(move |db| db.emparejamientos_preparados(&ctx, ahora())).await?;
    Ok(Json(json!(l.iter().map(preparado_json).collect::<Vec<_>>())))
}

/// `GET /api/clientes/{c}/a-medias` (administrador, v1.42): los equipos que se unieron y se
/// quedaron sin terminar (sin comparar el número o sin el alta), de cualquier cuenta, para
/// seguir desde «Añadir equipo» en vez de empezar de nuevo: `[{ id, estado: "unido" |
/// "confirmado", caduca, creado, nombre, equipo: { id, nombre, so } }]` (el código, en
/// `GET …/emparejamientos/{p}`).
pub async fn a_medias(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let l = st
        .db(move |db| {
            let mut out = Vec::new();
            for e in db.a_medias(&ctx, ahora())? {
                let Some(eq) = e.equipo_id.as_deref() else { continue };
                if let Some(equipo) = db.equipo(&ctx, eq)? {
                    out.push((e, equipo));
                }
            }
            Ok(out)
        })
        .await?;
    Ok(Json(json!(l
        .iter()
        .map(|(e, q)| json!({
            "id": e.id, "estado": e.estado, "caduca": fecha(e.caduca), "creado": fecha(e.creado),
            "nombre": e.nombre.clone().unwrap_or_else(|| q.nombre.clone()),
            "equipo": { "id": q.id, "nombre": q.nombre, "so": q.so },
        }))
        .collect::<Vec<_>>())))
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
