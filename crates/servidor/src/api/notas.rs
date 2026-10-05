//! Observaciones y comentarios (v1.3x, docs/api-servidor.md §6, «Observaciones
//! y comentarios»). Texto que escriben las personas sobre un equipo, un
//! repositorio, una copia, un destino o el cliente: se guarda en claro en el
//! archivo del cliente (`almacen::notas`). Leer, cualquier miembro; escribir,
//! técnico o más. Cada cambio se audita (sin el texto).

use super::fecha;
use crate::almacen::{ahora, Comentario, Observacion, Rol};
use crate::auth::Usuario;
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

/// Caracteres de una observación o de un comentario.
pub const MAX_TEXTO: usize = 2000;
/// Observaciones por cliente.
pub const MAX_OBSERVACIONES: i64 = 10_000;
/// Comentarios por objeto y por cliente.
pub const MAX_COMENTARIOS_OBJETO: i64 = 1_000;
pub const MAX_COMENTARIOS_CLIENTE: i64 = 50_000;
/// Comentarios que se devuelven de un objeto (los más recientes).
pub const COMENTARIOS_VISIBLES: i64 = 200;
/// Quien escribió un comentario lo puede cambiar o borrar durante este tiempo.
pub const MINUTOS_CAMBIO: i64 = 15;
/// Lo que se enseña de una observación en la lista (contadores y búsqueda).
const MAX_TITULO: usize = 80;

pub const TIPOS: [&str; 5] = ["cliente", "equipo", "repositorio", "copia", "destino"];

fn texto_limpio(t: &str) -> Res<String> {
    let t = t.replace("\r\n", "\n").replace('\r', "\n");
    let t = t.trim().to_string();
    if t.chars().count() > MAX_TEXTO {
        return Err(ErrorApi::datos(format!("Como mucho {MAX_TEXTO} caracteres.")));
    }
    if t.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
        return Err(ErrorApi::datos("El texto lleva caracteres no válidos."));
    }
    Ok(t)
}

fn id_valido(id: &str) -> bool {
    !id.is_empty() && id.chars().count() <= 128 && !id.chars().any(char::is_control)
}

/// Comprueba el objeto: `cliente` → el id del cliente; `equipo` → un equipo del
/// cliente; `repositorio` y `copia` → «<equipo>/<id>»; `destino` → su id.
async fn objeto_valido(st: &St, ctx: &crate::almacen::ClienteCtx, tipo: &str, objeto: &str) -> Res<()> {
    let equipo = match tipo {
        "cliente" if objeto == ctx.id() => return Ok(()),
        "destino" if id_valido(objeto) && !objeto.contains('/') => return Ok(()),
        "equipo" => objeto,
        "repositorio" | "copia" => match objeto.split_once('/') {
            Some((e, id)) if id_valido(id) => e,
            _ => return Err(ErrorApi::datos("Objeto no válido («<equipo>/<id>»).")),
        },
        t if !TIPOS.contains(&t) => return Err(ErrorApi::datos("Tipo no válido (cliente, equipo, repositorio, copia o destino).")),
        _ => return Err(ErrorApi::datos("Objeto no válido.")),
    };
    let (ctx, e) = (ctx.clone(), equipo.to_string());
    if !id_valido(&e) || st.db(move |db| db.equipo(&ctx, &e)).await?.is_none() {
        return Err(ErrorApi::no_existe());
    }
    Ok(())
}

/// La primera línea con texto, sin las marcas de Markdown, para la lista.
pub fn titulo(texto: &str) -> String {
    let linea = texto.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let linea = linea.trim_start_matches(['#', '>', '-', '*', '+', ' ']);
    // «1. algo» o «2) algo» (una lista numerada), no «3/10 cambié el disco».
    let sin_num = linea.trim_start_matches(|c: char| c.is_ascii_digit());
    let linea = if sin_num.len() < linea.len() && (sin_num.starts_with(". ") || sin_num.starts_with(") ")) { sin_num[2..].trim_start() } else { linea };
    // [texto](enlace) → texto; sin * _ ` sueltos.
    let mut out = String::new();
    let mut resto = linea;
    while let Some(i) = resto.find('[') {
        out.push_str(&resto[..i]);
        let tras = &resto[i + 1..];
        match tras.find("](").and_then(|j| tras[j..].find(')').map(|k| (j, j + k))) {
            Some((j, k)) => {
                out.push_str(&tras[..j]);
                resto = &tras[k + 1..];
            }
            None => {
                out.push('[');
                resto = tras;
            }
        }
    }
    out.push_str(resto);
    let limpio: String = out.chars().filter(|c| !matches!(c, '*' | '_' | '`')).collect();
    let limpio = limpio.trim();
    if limpio.chars().count() > MAX_TITULO {
        format!("{}…", limpio.chars().take(MAX_TITULO - 1).collect::<String>().trim_end())
    } else {
        limpio.to_string()
    }
}

fn comentario_json(k: &Comentario, yo: &str, rol: Rol, ahora: i64) -> Value {
    let mio = k.autor_id == yo;
    let a_tiempo = ahora - k.creado < MINUTOS_CAMBIO * 60;
    json!({
        "id": k.id, "texto": k.texto, "autor": { "id": k.autor_id, "nombre": k.autor },
        "creado": fecha(k.creado), "editado": k.editado.map(fecha),
        "editable": mio && a_tiempo && rol >= Rol::Tecnico,
        "borrable": (mio && a_tiempo && rol >= Rol::Tecnico) || rol >= Rol::Propietario,
    })
}

fn actor(u: &Usuario) -> String {
    format!("cuenta:{}", u.0.cuenta.correo)
}

/// `GET /api/clientes/{c}/notas` (lectura): `{ objetos: [{ tipo, objeto, titulo, comentarios, actualizada }] }`.
pub async fn indice(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Lectura).await?;
    let l = st.db(move |db| db.indice_notas(&ctx)).await?;
    Ok(Json(json!({
        "objetos": l.iter().map(|i| json!({
            "tipo": i.tipo, "objeto": i.objeto,
            "titulo": i.observacion.as_deref().map(titulo).filter(|t| !t.is_empty()),
            "observacion": i.observacion.is_some(),
            "comentarios": i.comentarios, "actualizada": fecha(i.actualizada),
        })).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
pub struct DeObjeto {
    tipo: String,
    objeto: String,
}

/// `GET /api/clientes/{c}/notas/objeto?tipo=&objeto=` (lectura): la observación y los comentarios.
pub async fn ver(State(st): State<St>, u: Usuario, Path(c): Path<String>, Query(q): Query<DeObjeto>) -> Res<Json<Value>> {
    let (ctx, rol) = u.miembro(&st, &c, Rol::Lectura).await?;
    if !TIPOS.contains(&q.tipo.as_str()) || !id_valido(&q.objeto) {
        return Err(ErrorApi::datos("Objeto no válido."));
    }
    let (t, o) = (q.tipo.clone(), q.objeto.clone());
    let (obs, coms) = st.db(move |db| Ok((db.observacion(&ctx, &t, &o)?, db.comentarios(&ctx, &t, &o, COMENTARIOS_VISIBLES)?))).await?;
    let (yo, ahora) = (u.id().to_string(), ahora());
    Ok(Json(json!({
        "tipo": q.tipo, "objeto": q.objeto,
        "observacion": obs.map(|o| json!({ "texto": o.texto, "actualizada": fecha(o.actualizada), "por": o.por })),
        "comentarios": coms.iter().map(|k| comentario_json(k, &yo, rol, ahora)).collect::<Vec<_>>(),
        "minutos_cambio": MINUTOS_CAMBIO,
    })))
}

#[derive(Deserialize)]
pub struct PonerObservacion {
    tipo: String,
    objeto: String,
    texto: String,
}

/// `PUT /api/clientes/{c}/notas/observacion` (técnico o más): `{ tipo, objeto, texto }`; vacía, se borra.
pub async fn poner_observacion(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(p): Json<PonerObservacion>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Tecnico).await?;
    let texto = texto_limpio(&p.texto)?;
    objeto_valido(&st, &ctx, &p.tipo, &p.objeto).await?;
    let o = Observacion { tipo: p.tipo, objeto: p.objeto, texto, actualizada: ahora(), por_id: u.id().to_string(), por: u.nombre().to_string() };
    let (o2, actor) = (o.clone(), actor(&u));
    let cabe = st
        .db(move |db| {
            let antes = db.observacion(&ctx, &o2.tipo, &o2.objeto)?;
            if antes.as_ref().map(|a| a.texto.as_str()).unwrap_or("") == o2.texto {
                return Ok(true);
            }
            if !db.poner_observacion(&ctx, &o2, MAX_OBSERVACIONES)? {
                return Ok(false);
            }
            let datos = json!({ "tipo": o2.tipo, "caracteres": o2.texto.chars().count(), "borrada": o2.texto.is_empty() });
            db.auditar(&ctx, &actor, "poner_observacion", &format!("{}:{}", o2.tipo, o2.objeto), &datos.to_string())?;
            Ok(true)
        })
        .await?;
    if !cabe {
        return Err(ErrorApi::datos(format!("Como mucho {MAX_OBSERVACIONES} observaciones por cliente.")));
    }
    Ok(Json(if o.texto.is_empty() { Value::Null } else { json!({ "texto": o.texto, "actualizada": fecha(o.actualizada), "por": o.por }) }))
}

#[derive(Deserialize)]
pub struct NuevoComentario {
    tipo: String,
    objeto: String,
    texto: String,
}

/// `POST /api/clientes/{c}/notas/comentarios` (técnico o más): `{ tipo, objeto, texto }` → el comentario.
pub async fn comentar(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(p): Json<NuevoComentario>) -> Res<Json<Value>> {
    let (ctx, rol) = u.miembro(&st, &c, Rol::Tecnico).await?;
    let texto = texto_limpio(&p.texto)?;
    if texto.is_empty() {
        return Err(ErrorApi::datos("Escribe algo."));
    }
    objeto_valido(&st, &ctx, &p.tipo, &p.objeto).await?;
    let k = Comentario {
        id: uuid::Uuid::new_v4().to_string(),
        tipo: p.tipo,
        objeto: p.objeto,
        texto,
        autor_id: u.id().to_string(),
        autor: u.nombre().to_string(),
        creado: ahora(),
        editado: None,
    };
    let (k2, actor) = (k.clone(), actor(&u));
    let cabe = st
        .db(move |db| {
            if !db.crear_comentario(&ctx, &k2, MAX_COMENTARIOS_OBJETO, MAX_COMENTARIOS_CLIENTE)? {
                return Ok(false);
            }
            db.auditar(
                &ctx,
                &actor,
                "comentar",
                &format!("{}:{}", k2.tipo, k2.objeto),
                &json!({ "id": k2.id, "caracteres": k2.texto.chars().count() }).to_string(),
            )?;
            Ok(true)
        })
        .await?;
    if !cabe {
        return Err(ErrorApi::datos(format!("Demasiados comentarios (como mucho {MAX_COMENTARIOS_OBJETO} por objeto).")));
    }
    Ok(Json(comentario_json(&k, u.id(), rol, ahora())))
}

#[derive(Deserialize)]
pub struct Editar {
    texto: String,
}

/// El comentario, si existe; y si esta persona lo puede tocar (`borrar`: también un propietario).
async fn tocable(st: &St, u: &Usuario, c: &str, id: &str, borrar: bool) -> Res<(crate::almacen::ClienteCtx, Rol, Comentario)> {
    let (ctx, rol) = u.miembro(st, c, Rol::Tecnico).await?;
    let (ctx2, id2) = (ctx.clone(), id.to_string());
    let k = st.db(move |db| db.comentario(&ctx2, &id2)).await?.ok_or_else(ErrorApi::no_existe)?;
    let mio = k.autor_id == u.id() && ahora() - k.creado < MINUTOS_CAMBIO * 60;
    if !(mio || (borrar && rol >= Rol::Propietario)) {
        return Err(ErrorApi::nuevo(
            StatusCode::FORBIDDEN,
            "prohibido",
            if k.autor_id == u.id() {
                format!("Solo se puede cambiar o borrar durante {MINUTOS_CAMBIO} minutos. Añade otro comentario.")
            } else {
                "Solo quien lo escribió (o una persona propietaria, para borrarlo).".to_string()
            },
        ));
    }
    Ok((ctx, rol, k))
}

/// `PATCH /api/clientes/{c}/notas/comentarios/{id}` (su autor, en sus 15 minutos): `{ texto }`.
pub async fn editar(State(st): State<St>, u: Usuario, Path((c, id)): Path<(String, String)>, Json(p): Json<Editar>) -> Res<Json<Value>> {
    let (ctx, rol, mut k) = tocable(&st, &u, &c, &id, false).await?;
    let texto = texto_limpio(&p.texto)?;
    if texto.is_empty() {
        return Err(ErrorApi::datos("Escribe algo (o bórralo)."));
    }
    let (cuando, actor) = (ahora(), actor(&u));
    let (id2, t2, obj) = (id.clone(), texto.clone(), format!("{}:{}", k.tipo, k.objeto));
    st.db(move |db| {
        db.editar_comentario(&ctx, &id2, &t2, cuando)?;
        db.auditar(&ctx, &actor, "editar_comentario", &obj, &json!({ "id": id2, "caracteres": t2.chars().count() }).to_string())
    })
    .await?;
    k.texto = texto;
    k.editado = Some(cuando);
    Ok(Json(comentario_json(&k, u.id(), rol, ahora())))
}

/// `DELETE /api/clientes/{c}/notas/comentarios/{id}` (su autor en sus 15 minutos, o un propietario).
pub async fn borrar(State(st): State<St>, u: Usuario, Path((c, id)): Path<(String, String)>) -> Res<StatusCode> {
    let (ctx, _, k) = tocable(&st, &u, &c, &id, true).await?;
    let actor = actor(&u);
    let ajeno = k.autor_id != u.id();
    st.db(move |db| {
        db.borrar_comentario(&ctx, &k.id)?;
        db.auditar(&ctx, &actor, "borrar_comentario", &format!("{}:{}", k.tipo, k.objeto), &json!({ "id": k.id, "de_otra_persona": ajeno }).to_string())
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/clientes/{c}/notas/todas` (administrador): todo, para el paquete de exportación.
pub async fn todas(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let (obs, coms) = st.db(move |db| db.todas_las_notas(&ctx)).await?;
    Ok(Json(json!({
        "observaciones": obs.iter().map(observacion_exportada).collect::<Vec<_>>(),
        "comentarios": coms.iter().map(comentario_exportado).collect::<Vec<_>>(),
    })))
}

fn observacion_exportada(o: &Observacion) -> Value {
    json!({ "tipo": o.tipo, "objeto": o.objeto, "texto": o.texto, "actualizada": fecha(o.actualizada), "por": o.por })
}

fn comentario_exportado(k: &Comentario) -> Value {
    json!({ "id": k.id, "tipo": k.tipo, "objeto": k.objeto, "texto": k.texto, "autor": k.autor, "creado": fecha(k.creado), "editado": k.editado.map(fecha) })
}

/// Notas que trae un paquete de exportación (`POST …/importar`, campo `notas`).
#[derive(Deserialize, Default)]
pub struct NotasImportadas {
    #[serde(default)]
    pub observaciones: Vec<ObsImportada>,
    #[serde(default)]
    pub comentarios: Vec<ComImportado>,
}

#[derive(Deserialize)]
pub struct ObsImportada {
    tipo: String,
    objeto: String,
    texto: String,
    actualizada: Value,
    #[serde(default)]
    por: String,
}

#[derive(Deserialize)]
pub struct ComImportado {
    id: String,
    tipo: String,
    objeto: String,
    texto: String,
    #[serde(default)]
    autor: String,
    creado: Value,
    #[serde(default)]
    editado: Option<Value>,
}

fn instante(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| v.as_str().and_then(crate::api::de_fecha))
}

fn nombre(n: &str) -> String {
    n.chars().filter(|c| !c.is_control()).take(200).collect()
}

/// Las notas importadas, comprobadas (las que no valen se saltan). Los autores
/// son de otro servidor: quedan como «importado» (nadie de aquí las puede editar).
pub fn preparar_importadas(n: NotasImportadas, origen: &str) -> (Vec<Observacion>, Vec<Comentario>) {
    let ok = |tipo: &str, objeto: &str| TIPOS.contains(&tipo) && id_valido(objeto);
    let por_id = format!("importado:{}", origen.chars().take(100).collect::<String>());
    let obs = n
        .observaciones
        .into_iter()
        .filter(|o| ok(&o.tipo, &o.objeto))
        .filter_map(|o| {
            let texto = texto_limpio(&o.texto).ok().filter(|t| !t.is_empty())?;
            Some(Observacion { texto, actualizada: instante(&o.actualizada)?, por_id: por_id.clone(), por: nombre(&o.por), tipo: o.tipo, objeto: o.objeto })
        })
        .take(MAX_OBSERVACIONES as usize)
        .collect();
    let coms = n
        .comentarios
        .into_iter()
        .filter(|k| ok(&k.tipo, &k.objeto) && uuid::Uuid::parse_str(&k.id).is_ok())
        .filter_map(|k| {
            let texto = texto_limpio(&k.texto).ok().filter(|t| !t.is_empty())?;
            Some(Comentario {
                id: k.id,
                texto,
                autor_id: por_id.clone(),
                autor: nombre(&k.autor),
                creado: instante(&k.creado)?,
                editado: k.editado.as_ref().and_then(instante),
                tipo: k.tipo,
                objeto: k.objeto,
            })
        })
        .take(MAX_COMENTARIOS_CLIENTE as usize)
        .collect();
    (obs, coms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titulo_sin_marcas() {
        assert_eq!(titulo("\n\n## **Disco** nuevo\nmás"), "Disco nuevo");
        assert_eq!(titulo("- llamar a [Luis](https://ejemplo.org) si _falla_"), "llamar a Luis si falla");
        assert_eq!(titulo("1. primero"), "primero");
        assert_eq!(titulo("3/10 cambié el disco"), "3/10 cambié el disco");
        assert_eq!(titulo("[sin cerrar"), "[sin cerrar");
        let largo = "a".repeat(200);
        assert_eq!(titulo(&largo).chars().count(), MAX_TITULO);
    }

    #[test]
    fn texto_limpio_y_limites() {
        assert_eq!(texto_limpio("  hola\r\nadiós \n").unwrap(), "hola\nadiós");
        assert!(texto_limpio(&"ñ".repeat(MAX_TEXTO)).is_ok());
        assert!(texto_limpio(&"ñ".repeat(MAX_TEXTO + 1)).is_err());
        assert!(texto_limpio("a\u{0007}b").is_err());
    }
}
