//! Pruebas funcionales (simulacros de recuperación; plan 0.8.0, E3, 14k).
//!
//! Una prueba funcional es un simulacro de verdad: alguien recupera datos como lo
//! haría el día de un desastre y apunta qué pasó. Aquí:
//!
//! - **Registro** (`GET/POST /api/clientes/{c}/pruebas-funcionales`, `PUT/DELETE …/{id}`):
//!   fecha, motivo, alcance (todo el cliente o unos repositorios), quién la hizo, qué se
//!   restauró y dónde, desde dónde (el repositorio, un espejo…), resultado, incidencias,
//!   el **tiempo hasta tener los datos de vuelta** (RTO real, minutos), la **antigüedad de
//!   lo recuperado** (RPO, minutos), notas y la restauración registrada que se usó, si hay.
//!   Lectura para todos los miembros; registrar y cambiar, técnicos o más; borrar,
//!   administradores o más. Cada cambio se audita sin el texto.
//! - **Adjuntos** (`POST …/{id}/adjuntos`, `GET/DELETE …/{id}/adjuntos/{n}`): solo PDF, PNG
//!   o JPG comprobados por su firma, como mucho [`MAX_ADJUNTO`] cada uno, [`MAX_ADJUNTOS`]
//!   por prueba y [`MAX_BYTES_CLIENTE`] por cliente. Se sirven siempre como descarga
//!   (`attachment`, `nosniff`, `sandbox`): nunca se abren dentro de la consola.
//! - **Política** (`PUT …/pruebas-funcionales/ajustes`, administradores): cada cuántos meses
//!   toca (6 por defecto; se puede apagar), con cuántos días de antelación se avisa, plazos
//!   propios por repositorio y objetivos de RTO y RPO opcionales.
//! - **Avisos** (en la vuelta de limpieza, cada 10 min): al acercarse el plazo, uno
//!   **informativo** (`prueba_funcional_pronto`); al vencer, uno **importante**
//!   (`prueba_funcional_vencida`), que se repite cada 30 días mientras siga vencida. De un
//!   cliente que nunca hizo ninguna, el plazo cuenta desde que este servidor vio la política
//!   por primera vez (al actualizar no salen todos a la vez).
//!
//! La consola cuenta la última prueba correcta de cada repositorio como prueba de
//! restauración en el «0» de la regla 3-2-1-1-0 (con la misma caducidad); el informe
//! ejecutivo en PDF trae las del periodo.

use crate::almacen::{ahora, AdjuntoInfo, Almacen, ClienteCtx, PruebaGuardada, Rol, Ts, R};
use crate::auth::Usuario;
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// Pruebas por cliente.
pub const MAX_PRUEBAS: i64 = 5_000;
/// Un adjunto, como mucho (ya decodificado).
pub const MAX_ADJUNTO: usize = 2 * 1024 * 1024;
/// Adjuntos por prueba.
pub const MAX_ADJUNTOS: i64 = 5;
/// Todos los adjuntos de un cliente juntos.
pub const MAX_BYTES_CLIENTE: i64 = 100 * 1024 * 1024;
/// El cuerpo de `POST …/adjuntos` (el adjunto en base64 y su nombre).
pub const MAX_CUERPO_ADJUNTO: usize = MAX_ADJUNTO * 4 / 3 + 4096;
/// Textos.
pub const MAX_TEXTO_CORTO: usize = 200;
pub const MAX_TEXTO: usize = 2000;
/// Repositorios en el alcance de una prueba y con plazo propio.
pub const MAX_REPOS: usize = 200;
/// Plazos: de 1 a 36 meses; aviso de 1 a 90 días antes.
pub const MESES: u32 = 6;
pub const MAX_MESES: u32 = 36;
pub const AVISO_DIAS: u32 = 30;
pub const MAX_AVISO_DIAS: u32 = 90;
/// RTO y RPO, como mucho (en minutos): un año y diez años.
pub const MAX_RTO_MIN: u32 = 525_600;
pub const MAX_RPO_MIN: u32 = 5_256_000;
const DIA: Ts = 86_400;
const MES: Ts = 30 * DIA;
/// Mientras siga vencida, el aviso importante se repite cada tanto.
const REPETIR_VENCIDA: Ts = 30 * DIA;
const AJUSTE: &str = "pruebas_funcionales";
pub const TIPO_PRONTO: &str = "prueba_funcional_pronto";
pub const TIPO_VENCIDA: &str = "prueba_funcional_vencida";

pub const MOTIVOS: [&str; 5] = ["programada", "auditoria", "cambio", "incidente", "otro"];
pub const RESULTADOS: [&str; 3] = ["ok", "parcial", "fallo"];
pub const DESDE: [&str; 4] = ["repositorio", "espejo", "derivada", "otro"];

// ---------- Datos ----------

/// Un repositorio de un equipo.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RepoRef {
    pub equipo: String,
    pub repo: String,
}

/// Desde dónde se restauró.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Desde {
    /// `repositorio`, `espejo`, `derivada` u `otro`.
    pub tipo: String,
    /// La clave del destino (la del catálogo o la del paso de la regla), si se eligió uno.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destino: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nombre: Option<String>,
}

/// La restauración registrada que se usó (su entrada de la bitácora del equipo).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RestauracionRef {
    pub equipo: String,
    pub id: String,
}

/// Lo que manda la consola al registrar o cambiar una prueba.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Entrada {
    /// `AAAA-MM-DD`.
    pub fecha: String,
    pub motivo: String,
    /// `cliente` (todo) o `repos` (los de `repos`).
    pub alcance: String,
    #[serde(default)]
    pub repos: Vec<RepoRef>,
    /// Qué se restauró (en palabras).
    #[serde(default)]
    pub que: String,
    /// Dónde se restauró (una máquina de pruebas, otra carpeta…).
    #[serde(default)]
    pub donde: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desde: Option<Desde>,
    /// Quién la hizo (sin él, quien la registra).
    #[serde(default)]
    pub quien: String,
    pub resultado: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incidencias: Option<String>,
    /// Minutos hasta tener los datos de vuelta (el RTO real).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rto_min: Option<u32>,
    /// Antigüedad de lo recuperado, en minutos (el RPO).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rpo_min: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restauracion: Option<RestauracionRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notas: Option<String>,
}

/// Un plazo propio de un repositorio.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlazoRepo {
    pub equipo: String,
    pub repo: String,
    pub meses: u32,
}

/// La política del cliente.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Politica {
    #[serde(default = "si")]
    pub activo: bool,
    #[serde(default = "meses")]
    pub meses: u32,
    #[serde(default = "aviso_dias")]
    pub aviso_dias: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rto_objetivo_min: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rpo_objetivo_min: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repos: Vec<PlazoRepo>,
}

impl Default for Politica {
    fn default() -> Self {
        Politica { activo: true, meses: MESES, aviso_dias: AVISO_DIAS, rto_objetivo_min: None, rpo_objetivo_min: None, repos: Vec::new() }
    }
}

fn si() -> bool {
    true
}
fn meses() -> u32 {
    MESES
}
fn aviso_dias() -> u32 {
    AVISO_DIAS
}

// ---------- Validación (todo lo que llega es de fuera) ----------

fn id_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Texto de una línea: sin controles y como mucho `max` caracteres (ya recortado).
fn linea(campo: &str, t: &str, max: usize) -> Result<String, String> {
    let t = t.trim();
    if t.chars().count() > max {
        return Err(format!("{campo}: como mucho {max} caracteres."));
    }
    if t.chars().any(char::is_control) {
        return Err(format!("{campo}: tiene caracteres que no valen."));
    }
    Ok(t.to_string())
}

/// Texto de varias líneas (solo saltos de línea como controles).
fn parrafo(campo: &str, t: Option<&str>, max: usize) -> Result<Option<String>, String> {
    let Some(t) = t.map(str::trim).filter(|t| !t.is_empty()) else { return Ok(None) };
    if t.chars().count() > max {
        return Err(format!("{campo}: como mucho {max} caracteres."));
    }
    if t.chars().any(|c| c.is_control() && c != '\n' && c != '\r' && c != '\t') {
        return Err(format!("{campo}: tiene caracteres que no valen."));
    }
    Ok(Some(t.to_string()))
}

/// El mediodía UTC de un día `AAAA-MM-DD` (para ordenar y para el informe).
pub fn dia(fecha: &str) -> Option<Ts> {
    let d = chrono::NaiveDate::parse_from_str(fecha, "%Y-%m-%d").ok()?;
    Some(d.and_hms_opt(12, 0, 0)?.and_utc().timestamp())
}

/// Comprueba y limpia lo que manda la consola. `ahora`: no se registra una prueba futura
/// (con un día de margen por las zonas horarias) ni de antes del año 2000.
pub fn validar(mut e: Entrada, ahora: Ts) -> Result<(Entrada, Ts), String> {
    let t = dia(&e.fecha).ok_or("Fecha no válida: AAAA-MM-DD.")?;
    if t < 946_728_000 || t > ahora + 2 * DIA {
        return Err("La fecha de la prueba no puede ser futura.".into());
    }
    if !MOTIVOS.contains(&e.motivo.as_str()) {
        return Err("Motivo no válido.".into());
    }
    if !RESULTADOS.contains(&e.resultado.as_str()) {
        return Err("Resultado no válido.".into());
    }
    match e.alcance.as_str() {
        "cliente" => e.repos.clear(),
        "repos" if e.repos.is_empty() => return Err("Elige qué repositorios se probaron (o «Todo el cliente»).".into()),
        "repos" => {}
        _ => return Err("Alcance no válido.".into()),
    }
    if e.repos.len() > MAX_REPOS {
        return Err(format!("Como mucho {MAX_REPOS} repositorios por prueba."));
    }
    if e.repos.iter().any(|r| !id_valido(&r.equipo) || !id_valido(&r.repo)) {
        return Err("Repositorio no válido.".into());
    }
    e.repos.sort_by(|a, b| (&a.equipo, &a.repo).cmp(&(&b.equipo, &b.repo)));
    e.repos.dedup();
    e.que = parrafo("Qué se restauró", Some(&e.que), MAX_TEXTO)?.unwrap_or_default();
    e.donde = linea("Dónde se restauró", &e.donde, MAX_TEXTO_CORTO)?;
    e.quien = linea("Quién la hizo", &e.quien, 80)?;
    if let Some(d) = e.desde.as_mut() {
        if !DESDE.contains(&d.tipo.as_str()) {
            return Err("«Desde dónde» no válido.".into());
        }
        d.destino = d.destino.as_deref().map(|x| linea("Destino", x, 160)).transpose()?.filter(|x| !x.is_empty());
        d.nombre = d.nombre.as_deref().map(|x| linea("Destino", x, 120)).transpose()?.filter(|x| !x.is_empty());
    }
    e.incidencias = parrafo("Incidencias", e.incidencias.as_deref(), MAX_TEXTO)?;
    e.notas = parrafo("Notas", e.notas.as_deref(), MAX_TEXTO)?;
    if e.rto_min.is_some_and(|x| x > MAX_RTO_MIN) {
        return Err("El tiempo hasta tener los datos de vuelta, como mucho un año.".into());
    }
    if e.rpo_min.is_some_and(|x| x > MAX_RPO_MIN) {
        return Err("La antigüedad de lo recuperado, como mucho diez años.".into());
    }
    if let Some(r) = &e.restauracion {
        let ok = id_valido(&r.equipo)
            && r.id.len() <= 64
            && r.id.strip_prefix("restauracion-").is_some_and(|x| !x.is_empty() && x.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
        if !ok {
            return Err("Restauración no válida.".into());
        }
    }
    Ok((e, t))
}

/// Comprueba la política.
pub fn validar_politica(mut p: Politica) -> Result<Politica, String> {
    if !(1..=MAX_MESES).contains(&p.meses) {
        return Err(format!("Cada 1 a {MAX_MESES} meses."));
    }
    if !(1..=MAX_AVISO_DIAS).contains(&p.aviso_dias) {
        return Err(format!("El aviso, de 1 a {MAX_AVISO_DIAS} días antes."));
    }
    if p.rto_objetivo_min.is_some_and(|x| x == 0 || x > MAX_RTO_MIN) || p.rpo_objetivo_min.is_some_and(|x| x == 0 || x > MAX_RPO_MIN) {
        return Err("Objetivo de RTO o RPO no válido.".into());
    }
    if p.repos.len() > MAX_REPOS {
        return Err(format!("Como mucho {MAX_REPOS} repositorios con plazo propio."));
    }
    for r in &p.repos {
        if !id_valido(&r.equipo) || !id_valido(&r.repo) {
            return Err("Repositorio no válido.".into());
        }
        if !(1..=MAX_MESES).contains(&r.meses) {
            return Err(format!("Cada 1 a {MAX_MESES} meses."));
        }
    }
    p.repos.sort_by(|a, b| (&a.equipo, &a.repo).cmp(&(&b.equipo, &b.repo)));
    p.repos.dedup_by(|a, b| a.equipo == b.equipo && a.repo == b.repo);
    Ok(p)
}

// ---------- Firmas de los adjuntos ----------

/// El tipo de un adjunto por su contenido (nunca por el nombre): PDF, PNG o JPG.
pub fn tipo_adjunto(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(b"%PDF-") {
        Some("application/pdf")
    } else if b.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("image/png")
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else {
        None
    }
}

/// Un nombre de archivo que se puede enseñar y poner en `Content-Disposition`: sin
/// carpetas ni caracteres raros, con la extensión del tipo de verdad.
pub fn nombre_adjunto(n: &str, tipo: &str) -> String {
    let base = n.rsplit(['/', '\\']).next().unwrap_or_default();
    let base = base.rsplit_once('.').map_or(base, |(a, _)| a);
    let limpio: String =
        base.chars().filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '(' | ')' | ',')).take(80).collect::<String>().trim().to_string();
    let ext = match tipo {
        "application/pdf" => "pdf",
        "image/png" => "png",
        _ => "jpg",
    };
    format!("{}.{ext}", if limpio.is_empty() { "adjunto" } else { &limpio })
}

// ---------- El plazo ----------

/// En qué punto está un plazo.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Plazo {
    /// La última prueba que cuenta (correcta o con incidencias), si hay.
    pub ultima: Option<Ts>,
    /// Cuándo vence.
    pub vence: Ts,
    /// `al_dia`, `pronto` o `vencida`.
    pub fase: &'static str,
    pub meses: u32,
}

/// Lo que cuenta de cada prueba para los plazos.
#[derive(Clone, Debug)]
pub struct Hecha {
    pub fecha: Ts,
    pub resultado: String,
    pub alcance: String,
    pub repos: Vec<RepoRef>,
}

impl Hecha {
    fn cuenta(&self) -> bool {
        self.resultado == "ok" || self.resultado == "parcial"
    }
    fn incluye(&self, equipo: &str, repo: &str) -> bool {
        self.alcance == "cliente" || self.repos.iter().any(|r| r.equipo == equipo && r.repo == repo)
    }
}

/// El plazo con `meses`, desde `ultima` (o `visto` si nunca hubo ninguna).
pub fn plazo(meses: u32, aviso_dias: u32, ultima: Option<Ts>, visto: Ts, ahora: Ts) -> Plazo {
    let vence = ultima.unwrap_or(visto) + Ts::from(meses.clamp(1, MAX_MESES)) * MES;
    let fase = if ahora >= vence {
        "vencida"
    } else if ahora >= vence - Ts::from(aviso_dias.clamp(1, MAX_AVISO_DIAS)) * DIA {
        "pronto"
    } else {
        "al_dia"
    };
    Plazo { ultima, vence, fase, meses }
}

/// El del cliente (cuenta cualquier prueba correcta o con incidencias) y el de cada
/// repositorio con plazo propio (las de todo el cliente o las que lo incluyen).
pub fn plazos(p: &Politica, hechas: &[Hecha], visto: Ts, ahora: Ts) -> (Plazo, Vec<(PlazoRepo, Plazo)>) {
    let ultima = hechas.iter().filter(|h| h.cuenta()).map(|h| h.fecha).max();
    let cliente = plazo(p.meses, p.aviso_dias, ultima, visto, ahora);
    let repos = p
        .repos
        .iter()
        .map(|r| {
            let u = hechas.iter().filter(|h| h.cuenta() && h.incluye(&r.equipo, &r.repo)).map(|h| h.fecha).max();
            (r.clone(), plazo(r.meses, p.aviso_dias, u, visto, ahora))
        })
        .collect();
    (cliente, repos)
}

/// ¿Toca avisar? (`pronto`: una vez por vencimiento; `vencida`: una vez y cada 30 días.)
/// `avisado`: (vencimiento del que se avisó, cuándo) de la última vez en esa fase.
pub fn toca_aviso(pl: &Plazo, avisado: Option<(Ts, Ts)>, ahora: Ts) -> bool {
    match pl.fase {
        "pronto" => avisado.is_none_or(|(v, _)| v != pl.vence),
        "vencida" => avisado.is_none_or(|(v, cuando)| v != pl.vence || ahora - cuando >= REPETIR_VENCIDA),
        _ => false,
    }
}

fn fecha_txt(t: Ts) -> String {
    chrono::DateTime::from_timestamp(t, 0).map(|d| d.format("%d/%m/%Y").to_string()).unwrap_or_default()
}

/// El texto del aviso.
pub fn texto_aviso(pl: &Plazo, de: Option<&str>) -> String {
    let que = de.map_or("Toca una prueba funcional (simulacro de recuperación)".to_string(), |r| format!("Toca una prueba funcional de «{r}»"));
    let cuando = match pl.ultima {
        Some(u) => format!("la última fue el {}", fecha_txt(u)),
        None => "aún no se ha registrado ninguna".into(),
    };
    match pl.fase {
        "vencida" => {
            format!("{que}: el plazo de {} meses venció el {} ({cuando}). Hazla y regístrala en «Pruebas funcionales».", pl.meses, fecha_txt(pl.vence))
        }
        _ => format!("{que} antes del {} (cada {} meses; {cuando}). Prepárala y regístrala en «Pruebas funcionales».", fecha_txt(pl.vence), pl.meses),
    }
}

// ---------- Almacén ----------

pub fn politica(db: &dyn Almacen, ctx: &ClienteCtx) -> R<(Politica, Option<(Ts, String)>)> {
    Ok(match db.ajuste_cliente(ctx, AJUSTE)? {
        Some((d, t, por)) => (serde_json::from_str::<Politica>(&d).ok().and_then(|p| validar_politica(p).ok()).unwrap_or_default(), Some((t, por))),
        None => (Politica::default(), None),
    })
}

/// Desde cuándo cuenta el plazo de un cliente sin pruebas: la primera vez que se miró.
fn visto(db: &dyn Almacen, ctx: &ClienteCtx, ahora: Ts) -> R<Ts> {
    let k = format!("pf:{}:visto", ctx.id());
    match db.valor(&k)?.and_then(|t| t.parse::<Ts>().ok()) {
        Some(t) => Ok(t),
        None => {
            db.poner_valor(&k, &ahora.to_string())?;
            Ok(ahora)
        }
    }
}

fn hechas(ps: &[PruebaGuardada]) -> Vec<Hecha> {
    ps.iter()
        .filter_map(|p| {
            let e: Entrada = serde_json::from_str(&p.datos).ok()?;
            Some(Hecha { fecha: p.fecha, resultado: e.resultado, alcance: e.alcance, repos: e.repos })
        })
        .collect()
}

fn plazo_json(p: &Plazo) -> Value {
    json!({ "ultima": p.ultima.map(crate::api::fecha), "vence": crate::api::fecha(p.vence), "fase": p.fase, "meses": p.meses })
}

/// El JSON de una prueba guardada, con sus adjuntos (sin el contenido).
fn prueba_json(p: &PruebaGuardada, adjuntos: &[AdjuntoInfo], pol: &Politica) -> Value {
    let mut v: Value = serde_json::from_str(&p.datos).unwrap_or_else(|_| json!({}));
    let e: Option<Entrada> = serde_json::from_value(v.clone()).ok();
    v["id"] = json!(p.id);
    v["creada"] = json!(crate::api::fecha(p.creada));
    v["actualizada"] = json!(crate::api::fecha(p.actualizada));
    v["por"] = json!(p.por);
    v["adjuntos"] = json!(adjuntos
        .iter()
        .filter(|a| a.prueba == p.id)
        .map(|a| json!({ "n": a.n, "nombre": a.nombre, "tipo": a.tipo, "tamano": a.tamano }))
        .collect::<Vec<_>>());
    // Si cumple los objetivos de la política (null: sin objetivo o sin el dato).
    if let Some(e) = e {
        v["cumple_rto"] = json!(pol.rto_objetivo_min.zip(e.rto_min).map(|(o, x)| x <= o));
        v["cumple_rpo"] = json!(pol.rpo_objetivo_min.zip(e.rpo_min).map(|(o, x)| x <= o));
    }
    v
}

/// Todo lo de un cliente (para `GET` y tras cada cambio).
fn todo(db: &dyn Almacen, ctx: &ClienteCtx, ahora: Ts) -> R<Value> {
    let (pol, cambio) = politica(db, ctx)?;
    let ps = db.pruebas_funcionales(ctx)?;
    let adj = db.adjuntos_pruebas(ctx)?;
    let v = visto(db, ctx, ahora)?;
    let (cliente, repos) = plazos(&pol, &hechas(&ps), v, ahora);
    Ok(json!({
        "ajustes": {
            "activo": pol.activo, "meses": pol.meses, "aviso_dias": pol.aviso_dias,
            "rto_objetivo_min": pol.rto_objetivo_min, "rpo_objetivo_min": pol.rpo_objetivo_min, "repos": pol.repos,
            "actualizada": cambio.as_ref().map(|(t, _)| crate::api::fecha(*t)), "por": cambio.map(|(_, p)| p),
        },
        "estado": if pol.activo {
            json!({ "cliente": plazo_json(&cliente), "repos": repos.iter().map(|(r, p)| json!({ "equipo": r.equipo, "repo": r.repo, "plazo": plazo_json(p) })).collect::<Vec<_>>() })
        } else {
            Value::Null
        },
        "pruebas": ps.iter().map(|p| prueba_json(p, &adj, &pol)).collect::<Vec<_>>(),
        "limites": { "adjunto": MAX_ADJUNTO, "adjuntos": MAX_ADJUNTOS },
    }))
}

// ---------- Avisos ----------

/// En la vuelta de limpieza (cada 10 min): los avisos que tocan en un cliente.
pub fn recordar(st: &St, ctx: &ClienteCtx, ahora: Ts) {
    let db = st.db.as_ref();
    let Ok((pol, _)) = politica(db, ctx) else { return };
    if !pol.activo {
        return;
    }
    let (Ok(ps), Ok(v)) = (db.pruebas_funcionales(ctx), visto(db, ctx, ahora)) else { return };
    let (cliente, repos) = plazos(&pol, &hechas(&ps), v, ahora);
    let nombres_repo = |equipo: &str, repo: &str| -> (String, Option<String>) {
        let e = db.equipo(ctx, equipo).ok().flatten();
        let nombre_equipo = e.as_ref().map(|e| e.nombre.clone());
        let r = e
            .and_then(|e| e.resumen)
            .and_then(|r| r["repositorios"].as_array().and_then(|l| l.iter().find(|x| x["id"] == repo).and_then(|x| x["nombre"].as_str().map(str::to_string))))
            .unwrap_or_else(|| repo.to_string());
        (r, nombre_equipo)
    };
    let mut sujetos: Vec<(String, Option<String>, Option<String>, Plazo)> = vec![("cliente".into(), None, None, cliente)];
    for (r, p) in repos {
        if db.equipo(ctx, &r.equipo).ok().flatten().is_none() {
            continue;
        }
        let (nombre, nombre_equipo) = nombres_repo(&r.equipo, &r.repo);
        let de = match nombre_equipo {
            Some(e) => format!("{nombre}» de «{e}"),
            None => nombre,
        };
        sujetos.push((format!("{}/{}", r.equipo, r.repo), Some(r.equipo.clone()), Some(de), p));
    }
    for (sujeto, equipo, de, pl) in sujetos {
        if pl.fase == "al_dia" {
            continue;
        }
        let k = format!("pf:{}:{sujeto}:{}", ctx.id(), pl.fase);
        let avisado = db.valor(&k).ok().flatten().and_then(|t| {
            let (a, b) = t.split_once(':')?;
            Some((a.parse::<Ts>().ok()?, b.parse::<Ts>().ok()?))
        });
        if !toca_aviso(&pl, avisado, ahora) {
            continue;
        }
        let tipo = if pl.fase == "vencida" { TIPO_VENCIDA } else { TIPO_PRONTO };
        if crate::notificaciones::aviso(db, ctx, equipo.as_deref(), tipo, &texto_aviso(&pl, de.as_deref())).is_ok() {
            let _ = db.poner_valor(&k, &format!("{}:{ahora}", pl.vence));
            st.vivo.avisar(ctx.id(), crate::vivo::Cambio::Avisos(equipo.as_deref()));
        }
    }
}

// ---------- Para el informe ejecutivo ----------

/// Una prueba en el informe.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FilaInforme {
    pub fecha: Ts,
    pub alcance: String,
    pub resultado: String,
    pub quien: String,
    pub rto_min: Option<u32>,
    pub rpo_min: Option<u32>,
    pub cumple_rto: Option<bool>,
    pub cumple_rpo: Option<bool>,
}

/// Lo que va en el informe: las del periodo, los objetivos y el plazo de hoy.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ParaInforme {
    pub lista: Vec<FilaInforme>,
    #[serde(default)]
    pub mas: u32,
    pub rto_objetivo_min: Option<u32>,
    pub rpo_objetivo_min: Option<u32>,
    /// La última de cualquier fecha (correcta o con incidencias).
    pub ultima: Option<Ts>,
    /// El plazo del cliente (`None`: política apagada).
    pub vence: Option<Ts>,
    pub fase: Option<String>,
}

pub const MAX_INFORME: usize = 20;

pub fn para_informe(db: &dyn Almacen, ctx: &ClienteCtx, equipos: &[crate::almacen::Equipo], desde: Ts, hasta: Ts, ahora: Ts) -> R<ParaInforme> {
    let (pol, _) = politica(db, ctx)?;
    let ps = db.pruebas_funcionales(ctx)?;
    let nombre_repo = |equipo: &str, repo: &str| -> String {
        equipos
            .iter()
            .find(|e| e.id == equipo)
            .and_then(|e| e.resumen.as_ref())
            .and_then(|r| r["repositorios"].as_array().and_then(|l| l.iter().find(|x| x["id"] == repo).and_then(|x| x["nombre"].as_str().map(str::to_string))))
            .unwrap_or_else(|| "Repositorio quitado".into())
    };
    let mut lista = Vec::new();
    let mut mas = 0;
    for p in ps.iter().filter(|p| p.fecha >= desde - DIA / 2 && p.fecha < hasta + DIA / 2) {
        let Ok(e) = serde_json::from_str::<Entrada>(&p.datos) else { continue };
        if lista.len() >= MAX_INFORME {
            mas += 1;
            continue;
        }
        let alcance = if e.alcance == "cliente" {
            "Todo el cliente".to_string()
        } else {
            let nombres: Vec<String> = e.repos.iter().take(3).map(|r| nombre_repo(&r.equipo, &r.repo)).collect();
            let resto = e.repos.len().saturating_sub(3);
            if resto > 0 {
                format!("{} y {resto} más", nombres.join(", "))
            } else {
                nombres.join(", ")
            }
        };
        lista.push(FilaInforme {
            fecha: p.fecha,
            alcance,
            resultado: e.resultado.clone(),
            quien: if e.quien.is_empty() { p.por.clone() } else { e.quien.clone() },
            rto_min: e.rto_min,
            rpo_min: e.rpo_min,
            cumple_rto: pol.rto_objetivo_min.zip(e.rto_min).map(|(o, x)| x <= o),
            cumple_rpo: pol.rpo_objetivo_min.zip(e.rpo_min).map(|(o, x)| x <= o),
        });
    }
    let hs = hechas(&ps);
    let ultima = hs.iter().filter(|h| h.cuenta()).map(|h| h.fecha).max();
    let (vence, fase) = if pol.activo {
        let v = db.valor(&format!("pf:{}:visto", ctx.id()))?.and_then(|t| t.parse::<Ts>().ok()).unwrap_or(ahora);
        let (c, _) = plazos(&pol, &hs, v, ahora);
        (Some(c.vence), Some(c.fase.to_string()))
    } else {
        (None, None)
    };
    Ok(ParaInforme { lista, mas, rto_objetivo_min: pol.rto_objetivo_min, rpo_objetivo_min: pol.rpo_objetivo_min, ultima, vence, fase })
}

// ---------- API ----------

fn actor(u: &Usuario) -> String {
    format!("cuenta:{}", u.0.cuenta.correo)
}

/// `GET /api/clientes/{c}/pruebas-funcionales` (cualquier miembro).
pub async fn listar(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Lectura).await?;
    Ok(Json(st.db(move |db| todo(db, &ctx, ahora())).await?))
}

/// Comprueba que los equipos de la prueba son de este cliente.
fn equipos_del_cliente(db: &dyn Almacen, ctx: &ClienteCtx, e: &Entrada) -> Result<(), String> {
    let ids: Vec<String> = db.equipos(ctx)?.into_iter().map(|x| x.id).collect();
    let todos = e.repos.iter().map(|r| &r.equipo).chain(e.restauracion.as_ref().map(|r| &r.equipo));
    for eq in todos {
        if !ids.contains(eq) {
            return Err("Ese equipo no es de este cliente.".into());
        }
    }
    Ok(())
}

async fn guardar(st: St, u: Usuario, c: String, id: Option<String>, e: Entrada) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Tecnico).await?;
    let ahora = ahora();
    let (mut e, fecha) = validar(e, ahora).map_err(ErrorApi::datos)?;
    if e.quien.is_empty() {
        e.quien = u.0.cuenta.nombre.chars().take(80).collect();
    }
    let datos = serde_json::to_string(&e).map_err(ErrorApi::interno)?;
    let (actor, por) = (actor(&u), u.0.cuenta.nombre.clone());
    let nueva = id.is_none();
    let id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let r = st
        .db_crudo(move |db| {
            equipos_del_cliente(db, &ctx, &e)?;
            let creada = match db.prueba_funcional(&ctx, &id)? {
                Some(p) => p.creada,
                None if nueva => ahora,
                None => return Err("no_existe".into()),
            };
            let p = PruebaGuardada { id: id.clone(), fecha, datos, creada, actualizada: ahora, por };
            if !db.guardar_prueba_funcional(&ctx, &p, MAX_PRUEBAS)? {
                return Err(format!("Como mucho {MAX_PRUEBAS} pruebas por cliente."));
            }
            let accion = if nueva { "registrar_prueba_funcional" } else { "cambiar_prueba_funcional" };
            db.auditar(&ctx, &actor, accion, &id, &json!({ "fecha": e.fecha, "resultado": e.resultado, "alcance": e.alcance }).to_string())?;
            let mut v = todo(db, &ctx, ahora)?;
            v["id"] = json!(id);
            Ok(v)
        })
        .await?
        .map_err(|e| if e == "no_existe" { ErrorApi::no_existe() } else { ErrorApi::datos(e) })?;
    Ok(Json(r))
}

/// `POST /api/clientes/{c}/pruebas-funcionales` (técnico o más): registra una prueba.
pub async fn registrar(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(e): Json<Entrada>) -> Res<Json<Value>> {
    guardar(st, u, c, None, e).await
}

/// `PUT /api/clientes/{c}/pruebas-funcionales/{id}` (técnico o más): la cambia.
pub async fn cambiar(State(st): State<St>, u: Usuario, Path((c, id)): Path<(String, String)>, Json(e): Json<Entrada>) -> Res<Json<Value>> {
    if !id_valido(&id) {
        return Err(ErrorApi::no_existe());
    }
    guardar(st, u, c, Some(id), e).await
}

/// `DELETE /api/clientes/{c}/pruebas-funcionales/{id}` (administrador o más), con sus adjuntos.
pub async fn borrar(State(st): State<St>, u: Usuario, Path((c, id)): Path<(String, String)>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    if !id_valido(&id) {
        return Err(ErrorApi::no_existe());
    }
    let actor = actor(&u);
    let r = st
        .db_crudo(move |db| {
            if !db.borrar_prueba_funcional(&ctx, &id)? {
                return Err("no_existe".into());
            }
            db.auditar(&ctx, &actor, "borrar_prueba_funcional", &id, "{}")?;
            todo(db, &ctx, ahora())
        })
        .await?
        .map_err(|e| if e == "no_existe" { ErrorApi::no_existe() } else { ErrorApi::interno(e) })?;
    Ok(Json(r))
}

/// `PUT /api/clientes/{c}/pruebas-funcionales/ajustes` (administrador o más): la política.
pub async fn poner_ajustes(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(p): Json<Politica>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let p = validar_politica(p).map_err(ErrorApi::datos)?;
    let (actor, por) = (actor(&u), u.0.cuenta.nombre.clone());
    let r = st
        .db(move |db| {
            let datos = serde_json::to_string(&p).map_err(|e| e.to_string())?;
            db.poner_ajuste_cliente(&ctx, AJUSTE, &datos, &por)?;
            db.auditar(&ctx, &actor, "ajustes_pruebas_funcionales", "", &json!({ "activo": p.activo, "meses": p.meses, "repos": p.repos.len() }).to_string())?;
            todo(db, &ctx, ahora())
        })
        .await?;
    Ok(Json(r))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NuevoAdjunto {
    nombre: String,
    /// El archivo en base64.
    datos: String,
}

/// `POST /api/clientes/{c}/pruebas-funcionales/{id}/adjuntos` (técnico o más):
/// `{ nombre, datos }`. Solo PDF, PNG o JPG (por su contenido), como mucho 2 MB.
pub async fn anadir_adjunto(State(st): State<St>, u: Usuario, Path((c, id)): Path<(String, String)>, Json(a): Json<NuevoAdjunto>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Tecnico).await?;
    if !id_valido(&id) {
        return Err(ErrorApi::no_existe());
    }
    let bytes = B64.decode(a.datos.trim()).map_err(|_| ErrorApi::datos("Adjunto no válido."))?;
    if bytes.is_empty() {
        return Err(ErrorApi::datos("El adjunto está vacío."));
    }
    if bytes.len() > MAX_ADJUNTO {
        return Err(ErrorApi::datos("El adjunto pesa demasiado: como mucho 2 MB."));
    }
    let tipo = tipo_adjunto(&bytes).ok_or_else(|| ErrorApi::datos("Solo PDF, PNG o JPG."))?;
    let info = AdjuntoInfo {
        prueba: id.clone(),
        n: 0,
        nombre: nombre_adjunto(&a.nombre, tipo),
        tipo: tipo.into(),
        tamano: bytes.len() as i64,
        creado: ahora(),
        por: u.0.cuenta.nombre.clone(),
    };
    let actor = actor(&u);
    let r = st
        .db_crudo(move |db| {
            if db.prueba_funcional(&ctx, &id)?.is_none() {
                return Err("no_existe".into());
            }
            let Some(n) = db.anadir_adjunto_prueba(&ctx, &info, &bytes, MAX_ADJUNTOS, MAX_BYTES_CLIENTE)? else {
                return Err(format!("Como mucho {MAX_ADJUNTOS} adjuntos por prueba y 100 MB entre todos los del cliente."));
            };
            db.auditar(&ctx, &actor, "adjunto_prueba_funcional", &format!("{id}/{n}"), &json!({ "tipo": info.tipo, "tamano": info.tamano }).to_string())?;
            todo(db, &ctx, ahora())
        })
        .await?
        .map_err(|e| if e == "no_existe" { ErrorApi::no_existe() } else { ErrorApi::datos(e) })?;
    Ok(Json(r))
}

/// `GET /api/clientes/{c}/pruebas-funcionales/{id}/adjuntos/{n}` (cualquier miembro): siempre como descarga.
pub async fn adjunto(State(st): State<St>, u: Usuario, Path((c, id, n)): Path<(String, String, i64)>) -> Res<Response> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Lectura).await?;
    if !id_valido(&id) {
        return Err(ErrorApi::no_existe());
    }
    let (a, bytes) = st.db(move |db| db.adjunto_prueba(&ctx, &id, n)).await?.ok_or_else(ErrorApi::no_existe)?;
    // Lo guardado ya se comprobó; aun así, el tipo sale de la lista y el nombre se vuelve a limpiar.
    let tipo = tipo_adjunto(&bytes).unwrap_or("application/octet-stream");
    let nombre = nombre_adjunto(&a.nombre, tipo);
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, tipo.to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{nombre}\"")),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
            (header::CONTENT_SECURITY_POLICY, "sandbox".to_string()),
            (header::CACHE_CONTROL, "private, no-store".to_string()),
        ],
        bytes,
    )
        .into_response())
}

/// `DELETE /api/clientes/{c}/pruebas-funcionales/{id}/adjuntos/{n}` (técnico o más).
pub async fn borrar_adjunto(State(st): State<St>, u: Usuario, Path((c, id, n)): Path<(String, String, i64)>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Tecnico).await?;
    if !id_valido(&id) {
        return Err(ErrorApi::no_existe());
    }
    let actor = actor(&u);
    let r = st
        .db_crudo(move |db| {
            if !db.borrar_adjunto_prueba(&ctx, &id, n)? {
                return Err("no_existe".into());
            }
            db.auditar(&ctx, &actor, "quitar_adjunto_prueba_funcional", &format!("{id}/{n}"), "{}")?;
            todo(db, &ctx, ahora())
        })
        .await?
        .map_err(|e| if e == "no_existe" { ErrorApi::no_existe() } else { ErrorApi::interno(e) })?;
    Ok(Json(r))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn entrada() -> Entrada {
        serde_json::from_value(json!({
            "fecha": "2026-10-01", "motivo": "programada", "alcance": "repos",
            "repos": [{ "equipo": "eq-1", "repo": "docs" }, { "equipo": "eq-1", "repo": "docs" }],
            "que": "La carpeta de facturas", "donde": "Máquina virtual de pruebas", "quien": " Ana ",
            "resultado": "ok", "rto_min": 95, "rpo_min": 600, "notas": "Todo bien.\nSin incidencias."
        }))
        .unwrap()
    }

    #[test]
    fn valida_lo_que_llega() {
        let ahora = dia("2026-10-14").unwrap();
        let (e, t) = validar(entrada(), ahora).unwrap();
        assert_eq!((e.repos.len(), e.quien.as_str(), t), (1, "Ana", dia("2026-10-01").unwrap()));
        let mal = |f: &dyn Fn(&mut Entrada)| {
            let mut e = entrada();
            f(&mut e);
            validar(e, ahora).is_err()
        };
        assert!(mal(&|e| e.fecha = "2026-13-01".into()));
        assert!(mal(&|e| e.fecha = "2027-01-01".into()), "no se registra una prueba futura");
        assert!(mal(&|e| e.motivo = "inventado".into()));
        assert!(mal(&|e| e.resultado = "quizá".into()));
        assert!(mal(&|e| e.repos.clear()), "con alcance «repos», alguno");
        assert!(mal(&|e| e.repos[0].repo = "../x".into()));
        assert!(mal(&|e| e.donde = "a\u{0}b".into()));
        assert!(mal(&|e| e.que = "x".repeat(MAX_TEXTO + 1)));
        assert!(mal(&|e| e.rto_min = Some(MAX_RTO_MIN + 1)));
        assert!(mal(&|e| e.restauracion = Some(RestauracionRef { equipo: "eq-1".into(), id: "copia-1".into() })));
        assert!(!mal(&|e| e.restauracion = Some(RestauracionRef { equipo: "eq-1".into(), id: "restauracion-0a1b".into() })));
        let mut todo = entrada();
        todo.alcance = "cliente".into();
        assert!(validar(todo, ahora).unwrap().0.repos.is_empty(), "todo el cliente: sin lista");
        // Campos de más: fuera.
        assert!(serde_json::from_value::<Entrada>(
            json!({ "fecha": "2026-10-01", "motivo": "otro", "alcance": "cliente", "resultado": "ok", "contrasena": "x" })
        )
        .is_err());
    }

    #[test]
    fn adjuntos_por_su_contenido() {
        assert_eq!(tipo_adjunto(b"%PDF-1.7 ..."), Some("application/pdf"));
        assert_eq!(tipo_adjunto(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0]), Some("image/png"));
        assert_eq!(tipo_adjunto(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(tipo_adjunto(b"<svg onload=alert(1)>"), None);
        assert_eq!(tipo_adjunto(b"<html>"), None);
        assert_eq!(nombre_adjunto("C:\\Users\\x\\Acta \"final\".exe", "application/pdf"), "Acta final.pdf");
        assert_eq!(nombre_adjunto("../../", "image/png"), "adjunto.png");
        assert_eq!(nombre_adjunto("captura.png", "image/jpeg"), "captura.jpg", "la extensión es la del contenido");
    }

    #[test]
    fn plazos_y_avisos() {
        let mes = MES;
        let pol = Politica { repos: vec![PlazoRepo { equipo: "e".into(), repo: "r".into(), meses: 3 }], ..Default::default() };
        let hecha = |fecha: Ts, resultado: &str, alcance: &str| Hecha { fecha, resultado: resultado.into(), alcance: alcance.into(), repos: vec![] };
        // Sin pruebas: cuenta desde que se vio.
        let (c, r) = plazos(&pol, &[], 0, 5 * mes);
        assert_eq!((c.fase, c.vence), ("pronto", 6 * mes));
        assert_eq!(r[0].1.fase, "vencida", "el repositorio, cada 3 meses");
        // Una fallida no cuenta; una con incidencias sí.
        let (c, _) = plazos(&pol, &[hecha(4 * mes, "fallo", "cliente")], 0, 7 * mes);
        assert_eq!(c.fase, "vencida");
        let (c, r) = plazos(&pol, &[hecha(4 * mes, "parcial", "cliente")], 0, 5 * mes);
        assert_eq!((c.fase, c.ultima), ("al_dia", Some(4 * mes)));
        assert_eq!(r[0].1.fase, "al_dia", "la de todo el cliente cuenta para cada repositorio");
        // Una de otros repositorios no cuenta para ese.
        let otra = Hecha { fecha: 4 * mes, resultado: "ok".into(), alcance: "repos".into(), repos: vec![RepoRef { equipo: "e".into(), repo: "x".into() }] };
        let (_, r) = plazos(&pol, &[otra], 0, 5 * mes);
        assert_eq!(r[0].1.fase, "vencida");
        // Avisos: «pronto» una vez por vencimiento; «vencida» una vez y cada 30 días.
        let p = plazo(6, 30, None, 0, 5 * mes + DIA);
        assert!(toca_aviso(&p, None, 0));
        assert!(!toca_aviso(&p, Some((p.vence, 5 * mes)), 0));
        let v = plazo(6, 30, None, 0, 6 * mes);
        assert!(toca_aviso(&v, None, 6 * mes));
        assert!(!toca_aviso(&v, Some((v.vence, 6 * mes)), 6 * mes + 29 * DIA));
        assert!(toca_aviso(&v, Some((v.vence, 6 * mes)), 6 * mes + 30 * DIA));
        assert!(!toca_aviso(&plazo(6, 30, None, 0, DIA), None, DIA), "al día: nada");
        let t = texto_aviso(&v, None);
        assert!(t.contains("venció") && !t.to_lowercase().contains("restic"), "{t}");
        assert!(texto_aviso(&p, Some("Ventas")).contains("«Ventas»"));
    }

    /// Con la base de datos de verdad: nada al principio, un aviso informativo al acercarse,
    /// uno importante al vencer (sin repetirlo en cada vuelta) y nada con la política apagada.
    #[test]
    fn avisa_al_acercarse_y_al_vencer() {
        let dir = tempfile::tempdir().unwrap();
        let st = crate::preparar(dir.path(), Default::default()).unwrap();
        let c = st.db.crear_cliente("Oficina Sur", "c2Fs", 24).unwrap();
        let ctx = ClienteCtx::autorizado(&c.id);
        let t0 = 1_790_000_000;
        let avisos = |tipo: &str| st.db.avisos(&ctx, false).unwrap().into_iter().filter(|a| a.tipo == tipo).count();
        recordar(&st, &ctx, t0);
        assert_eq!(avisos(TIPO_PRONTO) + avisos(TIPO_VENCIDA), 0, "recién visto: al día");
        recordar(&st, &ctx, t0 + 6 * MES - 10 * DIA);
        recordar(&st, &ctx, t0 + 6 * MES - 9 * DIA);
        assert_eq!(avisos(TIPO_PRONTO), 1, "uno al acercarse");
        recordar(&st, &ctx, t0 + 6 * MES);
        recordar(&st, &ctx, t0 + 6 * MES + DIA);
        assert_eq!(avisos(TIPO_VENCIDA), 1, "uno al vencer");
        let a = st.db.avisos(&ctx, false).unwrap().into_iter().find(|a| a.tipo == TIPO_VENCIDA).unwrap();
        assert!(a.mensaje.contains("venció") && a.mensaje.contains("aún no se ha registrado ninguna"), "{}", a.mensaje);
        recordar(&st, &ctx, t0 + 7 * MES + DIA);
        assert_eq!(avisos(TIPO_VENCIDA), 2, "y otra vez pasados 30 días");
        // Apagada: nada más.
        st.db.poner_ajuste_cliente(&ctx, AJUSTE, &json!({ "activo": false }).to_string(), "Ana").unwrap();
        recordar(&st, &ctx, t0 + 9 * MES);
        assert_eq!(avisos(TIPO_VENCIDA), 2);
    }

    #[test]
    fn politica_valida() {
        assert!(validar_politica(Politica::default()).is_ok());
        assert!(validar_politica(Politica { meses: 0, ..Default::default() }).is_err());
        assert!(validar_politica(Politica { aviso_dias: 91, ..Default::default() }).is_err());
        assert!(validar_politica(Politica { rto_objetivo_min: Some(0), ..Default::default() }).is_err());
        assert!(validar_politica(Politica { repos: vec![PlazoRepo { equipo: "e".into(), repo: "a b".into(), meses: 3 }], ..Default::default() }).is_err());
        assert!(serde_json::from_value::<Politica>(json!({ "activo": true, "otra": 1 })).is_err());
        let p: Politica = serde_json::from_value(json!({})).unwrap();
        assert_eq!((p.activo, p.meses, p.aviso_dias), (true, MESES, AVISO_DIAS));
    }
}
