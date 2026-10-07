//! Códigos de alta para varios equipos (bloque 7 de la 0.7.26, docs/api-servidor.md §4,
//! «Código para varios equipos»).
//!
//! El navegador genera el código (16 letras y cifras, ≈ 79 bits) y al servidor solo le manda su
//! hash, como en los códigos de v1.48. El servidor lo acepta hasta `usos` veces y hasta que
//! caduca o se anula. Cada equipo que se une con él crea **su propio emparejamiento**, ya unido y
//! sin confirmar (`agentes::unirse`): el número de comprobación (SAS v3) y la `prueba_codigo`
//! del `alta` son por equipo, así que los agentes no cambian. Nada entra sin que una persona
//! compare el número de cada equipo y lo confirme con la clave de administración.
//!
//! Para la línea de PowerShell, el instalador genérico se descarga de este mismo servidor sin
//! sesión (`GET /api/agente/instalador/{lote}`), solo mientras el lote sigue activo; la línea
//! lleva su SHA-256 (`GET …/instalador-agente/huella`) y no ejecuta nada que no coincida.

use super::fecha;
use super::instaladores::{codigo_repetido, hash_valido, limite_codigos, PREFIJO_NAVEGADOR};
use crate::almacen::{ahora, ClienteCtx, Emparejamiento, Lote, Rol, Ts};
use crate::auth::Usuario;
use crate::error::{ErrorApi, Res};
use crate::estado::{IpCliente, St};
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use axum::{Extension, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::{Duration, SystemTime};

/// Lo más que se puede pedir: equipos por código…
pub const MAX_USOS: i64 = 100;
/// …y días de validez.
pub const MAX_DIAS: i64 = 30;
/// Códigos para varios equipos activos a la vez en un cliente (cada uno deja entrar a muchos).
pub const MAX_ACTIVOS: usize = 5;
/// Intentos de unirse con un mismo código por hora, buenos o malos (además de los de la IP).
pub const MAX_UNIRSE_LOTE_H: u32 = 120;
/// Intentos rechazados de un código ya anulado, agotado o caducado que quedan en la auditoría
/// (los siguientes solo se cuentan: la auditoría no se llena aunque alguien insista).
pub const MAX_RECHAZOS_AUDITADOS: i64 = 20;
/// Descargas anónimas del instalador por IP y hora (`GET /api/agente/instalador/{lote}`).
pub const MAX_DESCARGAS_IP_H: u32 = 30;

#[derive(Deserialize)]
pub struct Crear {
    /// SHA-256 en hex del código normalizado (lo genera el navegador; el servidor nunca lo ve).
    codigo_hash: String,
    /// Cuántos equipos (1–100).
    usos: i64,
    /// Cuántos días sirve (1–30).
    dias: i64,
    /// Un nombre para reconocerlo (opcional, hasta 60 caracteres).
    #[serde(default)]
    nombre: Option<String>,
}

/// El nombre que se guarda: sin caracteres de control ni comillas, hasta 60; vacío, ninguno.
pub fn nombre_limpio(n: Option<&str>) -> Result<Option<String>, &'static str> {
    let Some(n) = n.map(str::trim).filter(|n| !n.is_empty()) else { return Ok(None) };
    if n.chars().count() > 60 {
        return Err("El nombre del código: hasta 60 caracteres.");
    }
    if n.chars().any(|c| c.is_control() || c == '"') {
        return Err("El nombre del código no puede llevar comillas ni saltos de línea.");
    }
    Ok(Some(n.to_string()))
}

/// Un lote como lo ve la consola.
pub fn lote_json(l: &Lote, ahora: Ts, pendientes: usize) -> Value {
    json!({
        "id": l.id, "codigo_hash": l.codigo_hash, "nombre": l.nombre,
        "usos": l.usos, "usados": l.usados, "quedan": (l.usos - l.usados).max(0),
        "caduca": fecha(l.caduca), "creado": fecha(l.creado), "estado": l.estado(ahora),
        "anulado": l.anulado.map(fecha), "rechazos": l.rechazos, "pendientes": pendientes,
    })
}

/// Los que esperan que alguien compare su número: unidos y sin caducar.
fn pendientes(l: &[Emparejamiento], ahora: Ts) -> usize {
    l.iter().filter(|e| e.estado == "unido" && e.caduca > ahora).count()
}

/// `POST /api/clientes/{c}/codigos-varios` (administrador): `{ codigo_hash, usos, dias, nombre? }`.
pub async fn crear(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(p): Json<Crear>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let hash = hash_valido(&p.codigo_hash).ok_or_else(|| ErrorApi::datos("Código no válido."))?;
    if !(1..=MAX_USOS).contains(&p.usos) {
        return Err(ErrorApi::datos(format!("Entre 1 y {MAX_USOS} equipos por código.")));
    }
    if !(1..=MAX_DIAS).contains(&p.dias) {
        return Err(ErrorApi::datos(format!("Entre 1 y {MAX_DIAS} días.")));
    }
    let nombre = nombre_limpio(p.nombre.as_deref()).map_err(ErrorApi::datos)?;
    super::cabe_otro_equipo(&st, &ctx).await?;
    let t = ahora();
    let (ctx2, t2) = (ctx.clone(), t);
    let activos = st.db(move |db| Ok(db.lotes(&ctx2)?.iter().filter(|l| l.estado(t2) == "activo").count())).await?;
    if activos >= MAX_ACTIVOS {
        return Err(ErrorApi::nuevo(
            StatusCode::CONFLICT,
            "demasiados_codigos_varios",
            format!("Ya hay {MAX_ACTIVOS} códigos para varios equipos activos en este cliente. Anula alguno que ya no uses antes de crear otro."),
        ));
    }
    // Cuenta como un código más en el límite por cuenta y por cliente.
    limite_codigos(&st, &c, u.id())?;
    let lote = Lote {
        id: uuid::Uuid::new_v4().to_string(),
        codigo_hash: hash.clone(),
        nombre,
        usos: p.usos,
        usados: 0,
        caduca: t + p.dias * 86_400,
        creado: t,
        creado_por: u.id().to_string(),
        anulado: None,
        rechazos: 0,
    };
    let (l2, actor) = (lote.clone(), format!("cuenta:{}", u.0.cuenta.correo));
    let nuevo = st
        .db(move |db| {
            if db.codigo_indexado(&hash)? {
                return Ok(false);
            }
            db.crear_lote(&ctx, &l2)?;
            db.indexar_codigo_varios(&hash, ctx.id(), &l2.id, l2.caduca)?;
            db.auditar(&ctx, &actor, "crear_codigo_varios", &l2.id, &json!({ "usos": l2.usos, "dias": p.dias, "nombre": l2.nombre }).to_string())?;
            Ok(true)
        })
        .await?;
    if !nuevo {
        return Err(codigo_repetido());
    }
    Ok(Json(lote_json(&lote, t, 0)))
}

/// `GET /api/clientes/{c}/codigos-varios` (administrador): los del cliente (los 50 últimos).
pub async fn listar(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let t = ahora();
    let l = st
        .db(move |db| {
            let mut out = Vec::new();
            for l in db.lotes(&ctx)? {
                let n = pendientes(&db.emparejamientos_de_lote(&ctx, &l.id)?, t);
                out.push(lote_json(&l, t, n));
            }
            Ok(out)
        })
        .await?;
    Ok(Json(json!(l)))
}

/// `GET /api/clientes/{c}/codigos-varios/{l}` (administrador): el lote y los equipos que se
/// unieron con él (`equipos`: su nombre, sistema, IP, llaves y el número de comprobación que
/// da el servidor; la consola calcula el suyo con las llaves y no sigue si no coincide).
pub async fn ver(State(st): State<St>, u: Usuario, Path((c, l)): Path<(String, String)>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let t = ahora();
    let (lote, emps) = st
        .db(move |db| {
            let Some(lote) = db.lote(&ctx, &l)? else { return Ok(None) };
            let mut emps = Vec::new();
            for e in db.emparejamientos_de_lote(&ctx, &l)? {
                let equipo = match e.equipo_id.as_deref() {
                    Some(id) => db.equipo(&ctx, id)?,
                    None => None,
                };
                emps.push((e, equipo));
            }
            Ok(Some((lote, emps)))
        })
        .await?
        .ok_or_else(ErrorApi::no_existe)?;
    let equipos: Vec<Value> = emps
        .iter()
        .map(|(e, q)| {
            let estado = if e.estado == "unido" && e.caduca <= t {
                "caducado"
            } else if e.estado == "confirmado" && e.codigo.is_none() {
                // El equipo ya hizo el alta (su código ya no hace falta).
                "dado_de_alta"
            } else {
                e.estado.as_str()
            };
            let mut v = json!({ "id": e.id, "estado": estado, "caduca": fecha(e.caduca), "unido": fecha(e.creado), "ip": e.ip });
            if let Some(q) = q {
                v["equipo"] = json!({ "id": q.id, "nombre": q.nombre, "so": q.so, "version_agente": q.version_agente, "box_pub": q.box_pub, "sign_pub": q.sign_pub, "sal_equipo": q.sal_equipo });
                let (version, sas) = crate::agentes::sas_de(&st, e.sas_version, &q.box_pub, &q.sign_pub);
                v["sas"] = json!(sas);
                v["sas_version"] = json!(version);
            }
            v
        })
        .collect();
    let mut v = lote_json(&lote, t, pendientes(&emps.iter().map(|(e, _)| e.clone()).collect::<Vec<_>>(), t));
    v["equipos"] = json!(equipos);
    Ok(Json(v))
}

/// `DELETE /api/clientes/{c}/codigos-varios/{l}` (administrador): «Anular el código». Ya no
/// entra ningún equipo más con él. Los que ya se unieron siguen esperando: se confirman o se
/// rechazan uno a uno (nunca entran solos).
pub async fn anular(State(st): State<St>, u: Usuario, Path((c, l)): Path<(String, String)>) -> Res<StatusCode> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let existe = st
        .db(move |db| {
            let Some(lote) = db.lote(&ctx, &l)? else { return Ok(false) };
            // Primero fuera del índice: desde aquí, nadie más se une con él.
            db.desindexar_codigo_varios(&l)?;
            if db.anular_lote(&ctx, &l, ahora())? {
                db.auditar(&ctx, &actor, "anular_codigo_varios", &l, &json!({ "usados": lote.usados, "usos": lote.usos }).to_string())?;
            }
            Ok(true)
        })
        .await?;
    if existe {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ErrorApi::no_existe())
    }
}

/// Lo que pasa al unirse con un código para varios equipos (`agentes::unirse`).
pub enum Union {
    /// No hay ningún lote con ese hash.
    NoEsLote,
    /// Lo hay, pero ya no admite equipos (anulado, agotado o caducado): cuenta como fallo.
    Rechazado,
    /// Demasiados intentos con este código en una hora.
    Demasiados,
    /// La cuota de equipos del cliente está llena.
    Cuota(String),
    /// Unido: el cliente.
    Unido(String),
}

/// Une `equipo` con el lote de ese hash, si lo hay y sigue activo: un emparejamiento nuevo, ya
/// unido (sin confirmar), con la IP desde la que llegó. Cada uso queda en la auditoría.
pub fn unirse_con_lote(
    st: &St,
    db: &dyn crate::almacen::Almacen,
    codigo_hash: &str,
    equipo: &crate::almacen::EquipoNuevo,
    sas_version: i64,
    ip: Option<&str>,
) -> crate::almacen::R<Union> {
    let Some((cliente, lote_id)) = db.codigo_varios(codigo_hash)? else { return Ok(Union::NoEsLote) };
    let ctx = ClienteCtx::autorizado(&cliente);
    let Some(lote) = db.lote(&ctx, &lote_id)? else { return Ok(Union::NoEsLote) };
    if !st.limites.intento(&format!("unirse-lote:{lote_id}"), MAX_UNIRSE_LOTE_H, Duration::from_secs(3600)) {
        return Ok(Union::Demasiados);
    }
    let t = ahora();
    let rechazar = |estado: &str| -> crate::almacen::R<Union> {
        db.rechazo_lote(&ctx, &lote_id)?;
        if lote.rechazos < MAX_RECHAZOS_AUDITADOS {
            db.auditar(&ctx, "servidor", "codigo_varios_rechazado", &lote_id, &json!({ "estado": estado, "nombre": equipo.nombre, "ip": ip }).to_string())?;
        }
        Ok(Union::Rechazado)
    };
    let estado = lote.estado(t);
    if estado != "activo" {
        return rechazar(estado);
    }
    // v1.34: la cuota de equipos del cliente, antes de gastar el uso.
    if let Err(m) = crate::cuotas::cabe_otro_equipo(db, st.opciones.publico, &cliente, db.equipos(&ctx)?.len())? {
        return Ok(Union::Cuota(m));
    }
    // Una sola sentencia: si otro equipo gastó el último uso a la vez, este se rechaza.
    if !db.usar_lote(&ctx, &lote_id, t)? {
        return rechazar("agotado");
    }
    db.crear_equipo(&ctx, equipo)?;
    db.indexar_equipo(&equipo.id, &cliente)?;
    let emp = uuid::Uuid::new_v4().to_string();
    // Hasta que caduca el código y, como poco, `PLAZO_UNIDO_S` para comparar el número.
    let caduca = lote.caduca.max(t + crate::almacen::PLAZO_UNIDO_S);
    db.emparejamiento_de_lote(&ctx, &emp, &lote_id, &lote.creado_por, caduca, &format!("{PREFIJO_NAVEGADOR}{codigo_hash}"), &equipo.id, ip)?;
    db.poner_sas_emparejamiento(&ctx, &emp, sas_version)?;
    db.auditar(
        &ctx,
        &format!("equipo:{}", equipo.id),
        "unirse",
        &equipo.id,
        &json!({ "nombre": equipo.nombre, "so": equipo.so, "lote": lote_id, "uso": lote.usados + 1, "usos": lote.usos, "ip": ip }).to_string(),
    )?;
    Ok(Union::Unido(cliente))
}

/// La IP para enseñarla: una IPv4 que llega como IPv6 («::ffff:a.b.c.d»), como IPv4.
pub fn ip_legible(ip: std::net::IpAddr) -> String {
    match ip {
        std::net::IpAddr::V6(v6) => v6.to_ipv4_mapped().map(|v4| v4.to_string()).unwrap_or_else(|| v6.to_string()),
        v4 => v4.to_string(),
    }
}

// ---------- El instalador para la línea de PowerShell ----------

/// SHA-256 del instalador genérico, recordado mientras el archivo no cambia (tamaño y fecha).
/// (ruta, tamaño, fecha de cambio, SHA-256 en hex).
type HuellaRecordada = (std::path::PathBuf, u64, Option<SystemTime>, String);
static HUELLA: std::sync::Mutex<Option<HuellaRecordada>> = std::sync::Mutex::new(None);

async fn huella_instalador(ruta: std::path::PathBuf) -> Option<(String, u64)> {
    let meta = tokio::fs::metadata(&ruta).await.ok()?;
    let (n, cuando) = (meta.len(), meta.modified().ok());
    if let Some((r, n0, c0, h)) = HUELLA.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        if *r == ruta && *n0 == n && *c0 == cuando {
            return Some((h.clone(), n));
        }
    }
    let bytes = tokio::fs::read(&ruta).await.ok()?;
    let n = bytes.len() as u64;
    let h = tokio::task::spawn_blocking(move || crate::instalador_agente::sha256_hex(&bytes)).await.ok()?;
    *HUELLA.lock().unwrap_or_else(|e| e.into_inner()) = Some((ruta, n, cuando, h.clone()));
    Some((h, n))
}

fn sin_instalador() -> ErrorApi {
    ErrorApi::nuevo(
        StatusCode::NOT_FOUND,
        "sin_instalador",
        "Este servidor no tiene el instalador del agente (viene con Resguardo Server para Windows; en Linux: sudo resguardo-server poner-instalador-agente <Resguardo-Agente-setup.exe>).",
    )
}

/// `GET /api/clientes/{c}/instalador-agente/huella` (administrador): `{ sha256, bytes }` del
/// instalador genérico que sirve este servidor. La consola lo pone en la línea de PowerShell, que
/// no ejecuta un instalador con otra huella. 404 `sin_instalador`.
pub async fn huella(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    u.miembro(&st, &c, Rol::Administrador).await?;
    let ruta = st.opciones.instalador_agente.clone().ok_or_else(sin_instalador)?;
    let (sha256, bytes) = huella_instalador(ruta).await.ok_or_else(sin_instalador)?;
    Ok(Json(json!({ "sha256": sha256, "bytes": bytes })))
}

/// `GET /api/agente/instalador/{lote}` (sin sesión, con límite por IP): el instalador genérico
/// del agente (el mismo de `…/instalador-agente`, sin cola ni código), solo mientras ese código
/// para varios equipos sigue activo. Lo usa la línea de PowerShell, que comprueba su SHA-256.
pub async fn instalador_publico(State(st): State<St>, ip: Option<Extension<IpCliente>>, Path(l): Path<String>) -> Res<Response> {
    let clave_ip = crate::estado::clave_ip(ip.as_ref().and_then(|Extension(IpCliente(i))| *i));
    if !st.limites.intento(&format!("instalador:{clave_ip}"), MAX_DESCARGAS_IP_H, Duration::from_secs(3600)) {
        return Err(ErrorApi::demasiados());
    }
    if uuid::Uuid::parse_str(&l).is_err() {
        return Err(ErrorApi::no_existe());
    }
    let l2 = l.clone();
    let activo = st
        .db(move |db| {
            let Some(cliente) = db.cliente_de_lote(&l2)? else { return Ok(false) };
            let ctx = ClienteCtx::autorizado(&cliente);
            Ok(db.lote(&ctx, &l2)?.is_some_and(|x| x.estado(ahora()) == "activo"))
        })
        .await?;
    if !activo {
        return Err(ErrorApi::nuevo(StatusCode::NOT_FOUND, "codigo", "Ese código para varios equipos ya no sirve (anulado, agotado o caducado)."));
    }
    let ruta = st.opciones.instalador_agente.clone().ok_or_else(sin_instalador)?;
    let exe = tokio::fs::read(&ruta).await.map_err(|_| sin_instalador())?;
    let mut r = Response::new(Body::from(exe));
    let h = r.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/vnd.microsoft.portable-executable"));
    h.insert(header::CONTENT_DISPOSITION, HeaderValue::from_static("attachment; filename=\"Resguardo-Agente-setup.exe\""));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(r)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn lote(usos: i64, usados: i64, caduca: Ts, anulado: Option<Ts>) -> Lote {
        Lote { id: "l".into(), codigo_hash: "0".repeat(64), nombre: None, usos, usados, caduca, creado: 0, creado_por: "ana".into(), anulado, rechazos: 0 }
    }

    #[test]
    fn estados_del_lote() {
        assert_eq!(lote(10, 0, 100, None).estado(50), "activo");
        assert_eq!(lote(10, 10, 100, None).estado(50), "agotado");
        assert_eq!(lote(10, 3, 100, None).estado(100), "caducado");
        assert_eq!(lote(10, 3, 100, Some(40)).estado(50), "anulado");
        // Anulado manda sobre caducado y agotado.
        assert_eq!(lote(10, 10, 10, Some(5)).estado(50), "anulado");
    }

    #[test]
    fn nombres_del_lote() {
        assert_eq!(nombre_limpio(None), Ok(None));
        assert_eq!(nombre_limpio(Some("   ")), Ok(None));
        assert_eq!(nombre_limpio(Some(" Planta 2 ")), Ok(Some("Planta 2".into())));
        assert!(nombre_limpio(Some("a\"b")).is_err());
        assert!(nombre_limpio(Some("a\nb")).is_err());
        assert!(nombre_limpio(Some(&"x".repeat(61))).is_err());
    }

    #[test]
    fn ip_para_ensenar() {
        assert_eq!(ip_legible("::ffff:192.0.2.7".parse().unwrap()), "192.0.2.7");
        assert_eq!(ip_legible("192.0.2.8".parse().unwrap()), "192.0.2.8");
        assert_eq!(ip_legible("2001:db8::1".parse().unwrap()), "2001:db8::1");
    }
}
