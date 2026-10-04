//! Equipos, emparejamiento, configuración, informes, avisos y auditoría.

use super::cuentas::MIEMBRO;
use super::{equipo_json, fecha};
use crate::almacen::{ahora, Rol};
use crate::auth::Usuario;
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

pub async fn listar(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, MIEMBRO).await?;
    let equipos = st.db(move |db| db.equipos(&ctx)).await?;
    Ok(Json(json!(equipos.iter().map(|e| equipo_json(e, st.conectado(&e.id))).collect::<Vec<_>>())))
}

pub async fn resumen(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, MIEMBRO).await?;
    let (equipos, avisos, pendientes) =
        st.db(move |db| Ok((db.equipos(&ctx)?, db.avisos(&ctx, true)?.len(), db.ordenes_con_espera(&ctx, ahora())?.len()))).await?;
    Ok(Json(json!({
        "equipos": equipos.iter().map(|e| equipo_json(e, st.conectado(&e.id))).collect::<Vec<_>>(),
        "avisos_abiertos": avisos,
        "pendientes": pendientes,
    })))
}

pub async fn ver(State(st): State<St>, u: Usuario, Path((c, e)): Path<(String, String)>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, MIEMBRO).await?;
    let e2 = e.clone();
    let (equipo, informe) = st.db(move |db| Ok((db.equipo(&ctx, &e2)?, db.informes(&ctx, &e2, 1)?))).await?;
    let equipo = equipo.ok_or_else(ErrorApi::no_existe)?;
    let mut v = equipo_json(&equipo, st.conectado(&equipo.id));
    v["ultimo_informe"] = informe.first().map(|(t, d)| json!({ "recibido": fecha(*t), "datos": d })).unwrap_or(Value::Null);
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct Renombrar {
    nombre: String,
}

pub async fn renombrar(State(st): State<St>, u: Usuario, Path((c, e)): Path<(String, String)>, Json(p): Json<Renombrar>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let nombre = p.nombre.trim().to_string();
    if nombre.is_empty() || nombre.chars().count() > 80 {
        return Err(ErrorApi::datos("Escribe un nombre (hasta 80 caracteres)."));
    }
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let (ctx2, e2) = (ctx.clone(), e.clone());
    let existe = st
        .db(move |db| {
            if db.equipo(&ctx2, &e2)?.is_none() {
                return Ok(false);
            }
            db.renombrar_equipo(&ctx2, &e2, &nombre)?;
            db.auditar(&ctx2, &actor, "renombrar_equipo", &e2, &json!({ "nombre": nombre }).to_string())?;
            Ok(true)
        })
        .await?;
    if !existe {
        return Err(ErrorApi::no_existe());
    }
    ver(State(st), u, Path((c, e))).await
}

/// Como mucho, etiquetas por equipo y caracteres por etiqueta.
pub const MAX_ETIQUETAS: usize = 10;
pub const MAX_LARGO_ETIQUETA: usize = 32;

/// Limpia las etiquetas: sin espacios de más, sin repetir (sin distinguir
/// mayúsculas), de 1 a 32 caracteres, sin comas ni caracteres de control.
pub fn normalizar_etiquetas(xs: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for x in xs {
        let t = x.split_whitespace().collect::<Vec<_>>().join(" ");
        if t.is_empty() {
            continue;
        }
        if t.chars().count() > MAX_LARGO_ETIQUETA || t.chars().any(|c| c.is_control() || c == ',') {
            return Err(format!("Etiqueta no válida: «{}» (hasta {MAX_LARGO_ETIQUETA} caracteres, sin comas).", t.chars().take(40).collect::<String>()));
        }
        if !out.iter().any(|o| o.to_lowercase() == t.to_lowercase()) {
            out.push(t);
        }
    }
    if out.len() > MAX_ETIQUETAS {
        return Err(format!("Como mucho {MAX_ETIQUETAS} etiquetas por equipo."));
    }
    Ok(out)
}

#[derive(Deserialize)]
pub struct Etiquetas {
    etiquetas: Vec<String>,
}

/// `PUT /api/clientes/{c}/equipos/{e}/etiquetas` (técnico o más, v1.18): metadatos
/// para agrupar y filtrar; no cambian nada en el equipo. Quedan auditadas.
pub async fn poner_etiquetas(State(st): State<St>, u: Usuario, Path((c, e)): Path<(String, String)>, Json(p): Json<Etiquetas>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Tecnico).await?;
    if p.etiquetas.len() > 50 {
        return Err(ErrorApi::datos("Demasiadas etiquetas."));
    }
    let etiquetas = normalizar_etiquetas(&p.etiquetas).map_err(ErrorApi::datos)?;
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let (ctx2, e2) = (ctx.clone(), e.clone());
    let existe = st
        .db(move |db| {
            if db.equipo(&ctx2, &e2)?.is_none() {
                return Ok(false);
            }
            db.poner_etiquetas_equipo(&ctx2, &e2, &etiquetas)?;
            db.auditar(&ctx2, &actor, "etiquetas_equipo", &e2, &json!({ "etiquetas": etiquetas }).to_string())?;
            Ok(true)
        })
        .await?;
    if !existe {
        return Err(ErrorApi::no_existe());
    }
    ver(State(st), u, Path((c, e))).await
}

pub async fn atencion(State(st): State<St>, u: Usuario, Path((c, e)): Path<(String, String)>) -> Res<StatusCode> {
    let (ctx, _) = u.miembro(&st, &c, MIEMBRO).await?;
    st.db(move |db| db.poner_atencion(&ctx, &e, ahora() + 600)).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn config(State(st): State<St>, u: Usuario, Path((c, e)): Path<(String, String)>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, MIEMBRO).await?;
    let cfg = st.db(move |db| db.config(&ctx, &e)).await?.ok_or_else(ErrorApi::no_existe)?;
    Ok(Json(json!({ "seq": cfg.0, "cifrado": cfg.1, "resumen": cfg.2 })))
}

#[derive(Deserialize)]
pub struct Limite {
    limite: Option<i64>,
}

pub async fn informes(State(st): State<St>, u: Usuario, Path((c, e)): Path<(String, String)>, Query(q): Query<Limite>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, MIEMBRO).await?;
    let limite = q.limite.unwrap_or(20).clamp(1, 200);
    let inf = st.db(move |db| db.informes(&ctx, &e, limite)).await?;
    Ok(Json(json!(inf.iter().map(|(t, d)| json!({ "recibido": fecha(*t), "datos": d })).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct FiltroHistorialQ {
    desde: Option<String>,
    hasta: Option<String>,
    tipo: Option<String>,
    antes: Option<String>,
    limite: Option<i64>,
}

/// Entradas del historial por página si no se pide otra cosa (v1.26; antes, 5000).
pub const HISTORIAL_POR_PAGINA: i64 = 500;
/// Lo más que da una página del historial (v1.26; antes, el tope entero: 20 000 ≈ 80 MB).
pub const HISTORIAL_MAX_PAGINA: i64 = 2000;

fn fecha_de(v: Option<&str>) -> Res<Option<i64>> {
    match v.filter(|d| !d.is_empty()) {
        Some(d) => Ok(Some(super::de_fecha(d).ok_or_else(|| ErrorApi::datos("Fecha no válida."))?)),
        None => Ok(None),
    }
}

/// `GET /api/clientes/{c}/equipos/{e}/historial?limite=&antes=&desde=&hasta=&tipo=`
/// (v1.23, por páginas desde v1.26): lo que el equipo cuenta de su historial (vueltas,
/// ganchos, verificaciones, espejo, avisos), de la más reciente a la más antigua. Cada
/// entrada, tal como la mandó el equipo. `limite` (500; como mucho 2000); `antes`: el id
/// de la última entrada de la página anterior; `tipo`: uno o varios separados por comas.
/// Una página con menos de `limite` entradas es la última.
pub async fn historial(State(st): State<St>, u: Usuario, Path((c, e)): Path<(String, String)>, Query(q): Query<FiltroHistorialQ>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, MIEMBRO).await?;
    let limite = q.limite.unwrap_or(HISTORIAL_POR_PAGINA).clamp(1, HISTORIAL_MAX_PAGINA);
    let mut tipos: Vec<String> = Vec::new();
    for t in q.tipo.as_deref().unwrap_or("").split(',').map(str::trim).filter(|t| !t.is_empty()) {
        if !crate::agentes::TIPOS_HISTORIAL.contains(&t) {
            return Err(ErrorApi::datos("Tipo de entrada del historial no válido."));
        }
        if !tipos.iter().any(|x| x == t) {
            tipos.push(t.to_string());
        }
    }
    let antes = q.antes.filter(|a| !a.is_empty());
    if antes.as_ref().is_some_and(|a| a.len() > 200) {
        return Err(ErrorApi::datos("Cursor no válido."));
    }
    let filtro = crate::almacen::FiltroHistorial { desde: fecha_de(q.desde.as_deref())?, hasta: fecha_de(q.hasta.as_deref())?, tipos, antes };
    let filas = st.db(move |db| db.historial(&ctx, &e, &filtro, limite)).await?;
    Ok(Json(json!(filas.iter().filter_map(|f| serde_json::from_str::<Value>(&f.datos).ok()).collect::<Vec<_>>())))
}

/// `GET /api/clientes/{c}/informes`: el último informe de cada equipo del
/// cliente, de una vez (la pantalla «Estado» no pide equipo a equipo).
pub async fn ultimos_informes(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, MIEMBRO).await?;
    let filas = st
        .db(move |db| {
            let mut out = Vec::new();
            for e in db.equipos(&ctx)? {
                if let Some((t, d)) = db.informes(&ctx, &e.id, 1)?.into_iter().next() {
                    out.push((e.id, t, d));
                }
            }
            Ok(out)
        })
        .await?;
    Ok(Json(json!(filas.iter().map(|(e, t, d)| json!({ "equipo": e, "recibido": fecha(*t), "datos": d })).collect::<Vec<_>>())))
}

// ---------- Emparejamiento ----------

pub async fn abrir_emparejamiento(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    super::cabe_otro_equipo(&st, &ctx).await?;
    if !st.limites.intento(&format!("emparejar:{c}"), 20, std::time::Duration::from_secs(3600)) {
        return Err(ErrorApi::demasiados());
    }
    let codigo = resguardo_protocolo::mensajes::pairing_code();
    let hash = resguardo_protocolo::mensajes::code_hash(&codigo);
    let id = uuid::Uuid::new_v4().to_string();
    let caduca = ahora() + 15 * 60;
    let (id2, por, actor) = (id.clone(), u.id().to_string(), format!("cuenta:{}", u.0.cuenta.correo));
    st.db(move |db| {
        db.crear_emparejamiento(&ctx, &id2, &por, caduca)?;
        db.indexar_codigo(&hash, ctx.id(), &id2, caduca)?;
        db.auditar(&ctx, &actor, "abrir_emparejamiento", &id2, "{}")
    })
    .await?;
    Ok(Json(json!({ "id": id, "codigo": codigo, "caduca": fecha(caduca) })))
}

pub async fn ver_emparejamiento(State(st): State<St>, u: Usuario, Path((c, p)): Path<(String, String)>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let (emp, equipo) = st
        .db(move |db| {
            let emp = db.emparejamiento(&ctx, &p)?;
            let equipo = match emp.as_ref().and_then(|e| e.equipo_id.clone()) {
                Some(id) => db.equipo(&ctx, &id)?,
                None => None,
            };
            Ok((emp, equipo))
        })
        .await?;
    let emp = emp.ok_or_else(ErrorApi::no_existe)?;
    let estado = if matches!(emp.estado.as_str(), "abierto" | "unido") && emp.caduca <= ahora() { "caducado".to_string() } else { emp.estado.clone() };
    let mut v = json!({ "estado": estado, "caduca": fecha(emp.caduca) });
    // Preparados (v1.17): su nombre y sistema y, mientras sirve, el código (la orden `alta` lo necesita).
    if emp.nombre.is_some() {
        v["nombre"] = json!(emp.nombre);
        v["so"] = json!(emp.so);
        if matches!(estado.as_str(), "abierto" | "unido") {
            v["codigo"] = json!(emp.codigo);
        }
    }
    if let Some(e) = equipo {
        v["equipo"] = json!({ "id": e.id, "nombre": e.nombre, "so": e.so, "box_pub": e.box_pub, "sign_pub": e.sign_pub, "sal_equipo": e.sal_equipo });
        // v1.26: v3 (con la huella de la autoridad TLS) si el equipo la anunció al unirse.
        let (version, sas) = crate::agentes::sas_de(&st, emp.sas_version, &e.box_pub, &e.sign_pub);
        v["sas"] = json!(sas);
        v["sas_version"] = json!(version);
    }
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct Confirmar {
    etiqueta: String,
}

pub async fn confirmar_emparejamiento(State(st): State<St>, u: Usuario, Path((c, p)): Path<(String, String)>, Json(b): Json<Confirmar>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    if b.etiqueta.is_empty() || b.etiqueta.len() > 100 {
        return Err(ErrorApi::datos("Etiqueta no válida."));
    }
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let r = st
        .db_crudo(move |db| {
            let emp = db.emparejamiento(&ctx, &p)?.ok_or("no_existe")?;
            if emp.estado != "unido" || emp.caduca <= ahora() {
                return Err("El equipo aún no se ha unido o el emparejamiento ha caducado.".into());
            }
            let equipo = emp.equipo_id.ok_or("no_existe")?;
            db.confirmar_equipo(&ctx, &equipo, &b.etiqueta)?;
            db.poner_estado_emparejamiento(&ctx, &p, "confirmado", None)?;
            db.auditar(&ctx, &actor, "confirmar_equipo", &equipo, "{}")?;
            db.equipo(&ctx, &equipo)
        })
        .await?;
    match r {
        Ok(Some(e)) => Ok(Json(equipo_json(&e, st.conectado(&e.id)))),
        Ok(None) => Err(ErrorApi::no_existe()),
        Err(e) if e == "no_existe" => Err(ErrorApi::no_existe()),
        Err(e) => Err(ErrorApi::datos(e)),
    }
}

pub async fn cancelar_emparejamiento(State(st): State<St>, u: Usuario, Path((c, p)): Path<(String, String)>) -> Res<StatusCode> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let existe = st
        .db(move |db| {
            let Some(emp) = db.emparejamiento(&ctx, &p)? else { return Ok(false) };
            if emp.estado != "confirmado" {
                if let Some(eq) = emp.equipo_id.as_deref() {
                    db.borrar_equipo(&ctx, eq)?;
                    db.desindexar_equipo(eq)?;
                }
                db.poner_estado_emparejamiento(&ctx, &p, "cancelado", None)?;
                db.auditar(&ctx, &actor, "cancelar_emparejamiento", &p, "{}")?;
            }
            Ok(true)
        })
        .await?;
    if !existe {
        return Err(ErrorApi::no_existe());
    }
    Ok(StatusCode::NO_CONTENT)
}

// ---------- Avisos y auditoría ----------

#[derive(Deserialize)]
pub struct Abiertos {
    abiertos: Option<String>,
}

pub async fn avisos(State(st): State<St>, u: Usuario, Path(c): Path<String>, Query(q): Query<Abiertos>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, MIEMBRO).await?;
    let solo = q.abiertos.as_deref().is_some_and(|v| v == "1" || v == "true");
    let avisos = st.db(move |db| db.avisos(&ctx, solo)).await?;
    Ok(Json(json!(avisos
        .iter()
        .map(|a| json!({ "id": a.id, "equipo": a.equipo, "tipo": a.tipo, "mensaje": a.mensaje, "creado": fecha(a.creado), "visto_por": a.visto_por }))
        .collect::<Vec<_>>())))
}

pub async fn aviso_visto(State(st): State<St>, u: Usuario, Path((c, a)): Path<(String, String)>) -> Res<StatusCode> {
    // Técnico o más (no los de lectura): un aviso de orden destructiva es lo
    // que deja ver a tiempo una orden que reduce la protección. Queda auditado.
    let (ctx, _) = u.miembro(&st, &c, Rol::Tecnico).await?;
    let por = u.0.cuenta.nombre.clone();
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    st.db(move |db| {
        db.marcar_aviso(&ctx, &a, &por)?;
        db.auditar(&ctx, &actor, "aviso_visto", &a, "{}")
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct Desde {
    desde: Option<i64>,
    limite: Option<i64>,
    /// `desc`: de la más reciente hacia atrás (con `antes` = el `n` más bajo recibido).
    orden: Option<String>,
    antes: Option<i64>,
}

/// La actividad del cliente. La leen los técnicos, administradores y
/// propietarios (no los de solo lectura); exportarla, solo administradores
/// y propietarios.
pub async fn auditoria(State(st): State<St>, u: Usuario, Path(c): Path<String>, Query(q): Query<Desde>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Tecnico).await?;
    let limite = q.limite.unwrap_or(100).clamp(1, 1000);
    let entradas = match q.orden.as_deref() {
        Some("desc") => st.db(move |db| db.auditoria_desc(&ctx, q.antes, limite)).await?,
        None | Some("asc") => {
            let desde = q.desde.unwrap_or(0).max(0);
            st.db(move |db| db.auditoria(&ctx, desde, limite)).await?
        }
        Some(_) => return Err(ErrorApi::datos("orden debe ser «asc» o «desc».")),
    };
    Ok(Json(json!(entradas
        .iter()
        .map(|e| json!({ "n": e.n, "creado": fecha(e.creado), "actor": e.actor, "accion": e.accion, "objetivo": e.objetivo, "datos": e.datos, "hash": e.hash, "prev_hash": e.prev_hash }))
        .collect::<Vec<_>>())))
}

pub async fn verificar_auditoria(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Tecnico).await?;
    let (total, rota) = st.db(move |db| db.verificar_auditoria(&ctx)).await?;
    Ok(Json(match rota {
        None => json!({ "ok": true, "entradas": total }),
        Some(n) => json!({ "ok": false, "rota_en": n, "entradas": total }),
    }))
}

#[cfg(test)]
mod pruebas {
    use super::normalizar_etiquetas;

    #[test]
    fn etiquetas_limpias() {
        let v = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(
            normalizar_etiquetas(&v(&["  Contabilidad ", "contabilidad", "Servidores", "", "Sede  norte"])).unwrap(),
            v(&["Contabilidad", "Servidores", "Sede norte"])
        );
        assert!(normalizar_etiquetas(&v(&["a,b"])).is_err(), "sin comas");
        assert!(normalizar_etiquetas(&v(&["x\u{7}"])).is_err(), "sin control");
        assert!(normalizar_etiquetas(&v(&[&"x".repeat(33)])).is_err());
        assert!(normalizar_etiquetas(&v(&[&"ñ".repeat(32)])).is_ok(), "32 caracteres, no bytes");
        let once: Vec<String> = (0..11).map(|i| format!("e{i}")).collect();
        assert!(normalizar_etiquetas(&once).is_err());
    }
}
