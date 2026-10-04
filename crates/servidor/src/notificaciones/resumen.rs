//! Resúmenes diario y semanal («Resumen semanal de copias»): por cliente y
//! equipo, cómo están, la última copia correcta, los fallos del periodo, lo
//! que ocupan y lo que necesita atención. Salen a la hora de los ajustes del
//! servidor (el semanal, el día elegido) a quien los quiere.

use super::ajustes::{self, Ajustes, TipoCanal};
use super::{encolar_uno, local, reglas, Incidente, Mensaje, Severidad};
use crate::almacen::{Almacen, ClienteCtx, Equipo, Ts, R};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Periodo {
    Diario,
    Semanal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Resumen {
    pub periodo: Periodo,
    pub desde: Ts,
    pub hasta: Ts,
    pub clientes: Vec<ResumenCliente>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ResumenCliente {
    pub id: String,
    pub nombre: String,
    pub equipos: Vec<ResumenEquipo>,
    /// Lo que necesita atención (títulos, ya sin rutas).
    pub atencion: Vec<String>,
    pub copias_ok: u32,
    pub fallos: u32,
    pub bytes: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ResumenEquipo {
    pub id: String,
    pub nombre: String,
    /// `ok`, `aviso`, `fallo`, `sin_contacto` o `sin_datos`.
    pub estado: String,
    pub ultima_ok: Option<Ts>,
    pub ultimo_contacto: Option<Ts>,
    pub copias_ok: u32,
    pub fallos: u32,
    pub bytes: Option<u64>,
}

/// Un día sin conectar ya es «sin contacto».
const CALLADO_S: Ts = 24 * 3600;

/// El resumen de un cliente con lo que se sabe de sus equipos (su último informe) y los
/// incidentes abiertos.
pub fn de_cliente(
    id: &str,
    nombre: &str,
    equipos: &[Equipo],
    informes: &HashMap<String, Value>,
    incidentes: &[Incidente],
    desde: Ts,
    hasta: Ts,
) -> ResumenCliente {
    let mut r = ResumenCliente { id: id.into(), nombre: nombre.into(), equipos: Vec::new(), atencion: Vec::new(), copias_ok: 0, fallos: 0, bytes: 0 };
    for e in equipos.iter().filter(|e| e.confirmado && e.modo == "gestionado") {
        let inf = informes.get(&e.id);
        let mut q = ResumenEquipo {
            id: e.id.clone(),
            nombre: e.nombre.clone(),
            estado: "ok".into(),
            ultima_ok: None,
            ultimo_contacto: e.ultimo_contacto,
            copias_ok: 0,
            fallos: 0,
            bytes: None,
        };
        let mut con_ejecuciones = false;
        for repo in inf.and_then(|i| i["repos"].as_array()).into_iter().flatten() {
            if let Some(b) = repo["espacio"]["en_disco_bytes"].as_u64() {
                q.bytes = Some(q.bytes.unwrap_or(0) + b);
            }
            for x in repo["ejecuciones"].as_array().into_iter().flatten() {
                con_ejecuciones = true;
                let Some(t) = x["hora"].as_str().and_then(crate::api::de_fecha) else { continue };
                let bien = matches!(x["resultado"].as_str(), Some("ok" | "sin_cambios" | "aviso"));
                if bien {
                    q.ultima_ok = q.ultima_ok.max(Some(t));
                }
                if t > desde && t <= hasta {
                    match x["resultado"].as_str() {
                        Some("fallo") => q.fallos += 1,
                        _ if bien => q.copias_ok += 1,
                        _ => {}
                    }
                }
            }
        }
        // Sin ejecuciones (agentes antiguos): lo último de cada copia.
        let mut peor_copia = "ok";
        for c in inf.and_then(|i| i["copias"].as_array()).into_iter().flatten() {
            let t = c["cuando"].as_str().and_then(crate::api::de_fecha);
            match c["estado"].as_str() {
                Some("fallo") => {
                    peor_copia = "fallo";
                    if !con_ejecuciones && t.is_some_and(|t| t > desde) {
                        q.fallos += 1;
                    }
                }
                Some("ok" | "aviso") => {
                    if c["estado"] == "aviso" && peor_copia == "ok" {
                        peor_copia = "aviso";
                    }
                    if !con_ejecuciones {
                        q.ultima_ok = q.ultima_ok.max(t);
                        if t.is_some_and(|t| t > desde) {
                            q.copias_ok += 1;
                        }
                    }
                }
                _ => {}
            }
        }
        let abiertos: Vec<&Incidente> = incidentes.iter().filter(|i| i.abierto && i.equipo.as_deref() == Some(e.id.as_str())).collect();
        q.estado = if e.ultimo_contacto.is_none_or(|t| hasta - t > CALLADO_S) {
            "sin_contacto"
        } else if inf.is_none() {
            "sin_datos"
        } else if peor_copia == "fallo" || abiertos.iter().any(|i| i.severidad == Severidad::Critico) {
            "fallo"
        } else if peor_copia == "aviso" || !abiertos.is_empty() {
            "aviso"
        } else {
            "ok"
        }
        .into();
        if q.estado == "sin_contacto" && !abiertos.iter().any(|i| i.tipo == "equipo_sin_contacto") {
            r.atencion.push(format!("«{}» no conecta con el servidor", e.nombre));
        }
        for i in abiertos {
            let veces = if i.veces > 1 { format!(" ({} veces)", i.veces) } else { String::new() };
            r.atencion.push(format!("{}{veces}", i.titulo));
        }
        r.copias_ok += q.copias_ok;
        r.fallos += q.fallos;
        r.bytes += q.bytes.unwrap_or(0);
        r.equipos.push(q);
    }
    // Primero lo que peor está.
    let peso = |s: &str| match s {
        "fallo" => 0,
        "sin_contacto" => 1,
        "aviso" => 2,
        "sin_datos" => 3,
        _ => 4,
    };
    r.equipos.sort_by(|a, b| peso(&a.estado).cmp(&peso(&b.estado)).then_with(|| a.nombre.to_lowercase().cmp(&b.nombre.to_lowercase())));
    r.atencion.truncate(30);
    r
}

fn resumen_de(db: &dyn Almacen, cliente: &str, desde: Ts, hasta: Ts) -> R<Option<ResumenCliente>> {
    let Some(c) = db.cliente(cliente)? else { return Ok(None) };
    let ctx = ClienteCtx::autorizado(cliente);
    let equipos = db.equipos(&ctx)?;
    let mut informes = HashMap::new();
    for e in equipos.iter().filter(|e| e.confirmado && e.modo == "gestionado") {
        if let Some((_, v)) = db.informes(&ctx, &e.id, 1)?.into_iter().next() {
            informes.insert(e.id.clone(), v);
        }
    }
    let incidentes = db.notif_incidentes_abiertos(Some(cliente))?;
    let r = de_cliente(&c.id, &c.nombre, &equipos, &informes, &incidentes, desde, hasta);
    Ok((!r.equipos.is_empty()).then_some(r))
}

const CLAVE_DIARIO: &str = "notif:resumen:diario";
const CLAVE_SEMANAL: &str = "notif:resumen:semanal";

/// Si es la hora (y no se hizo ya hoy o esta semana), prepara los resúmenes.
pub fn si_toca(db: &dyn Almacen, ahora: Ts) -> R<()> {
    let a = ajustes::ajustes(db)?;
    let t = local(ahora);
    let Some(h) = reglas::minutos(&a.hora_resumen) else { return Ok(()) };
    if chrono::Timelike::hour(&t) * 60 + chrono::Timelike::minute(&t) < h {
        return Ok(());
    }
    let hoy = t.format("%Y-%m-%d").to_string();
    if db.valor(CLAVE_DIARIO)?.as_deref() != Some(hoy.as_str()) {
        db.poner_valor(CLAVE_DIARIO, &hoy)?;
        generar(db, &a, Periodo::Diario, ahora)?;
    }
    let dia = chrono::Datelike::weekday(&t).number_from_monday() as u8;
    let semana = t.format("%G-W%V").to_string();
    if dia == a.dia_semanal && db.valor(CLAVE_SEMANAL)?.as_deref() != Some(semana.as_str()) {
        db.poner_valor(CLAVE_SEMANAL, &semana)?;
        generar(db, &a, Periodo::Semanal, ahora)?;
    }
    Ok(())
}

/// Prepara y pone en la cola los resúmenes de un periodo: uno por persona (con todos sus
/// clientes) y por canal compartido que los quiera.
pub fn generar(db: &dyn Almacen, a: &Ajustes, periodo: Periodo, ahora: Ts) -> R<usize> {
    let desde = ahora - if periodo == Periodo::Diario { 24 * 3600 } else { 7 * 24 * 3600 };
    let mut cache: HashMap<String, Option<ResumenCliente>> = HashMap::new();
    let mut resumen = |db: &dyn Almacen, id: &str| -> R<Option<ResumenCliente>> {
        if !cache.contains_key(id) {
            cache.insert(id.to_string(), resumen_de(db, id, desde, ahora)?);
        }
        Ok(cache[id].clone())
    };
    let quiere = |diario: bool, semanal: bool| if periodo == Periodo::Diario { diario } else { semanal };
    let mut n = 0;
    // Personas: por correo, una por persona y servidor de correo.
    // (cuenta, ámbito, canal) → (correo, horas de silencio, clientes).
    type Persona = (String, Option<super::Silencio>, Vec<String>);
    let mut por_persona: BTreeMap<(String, String, String), Persona> = BTreeMap::new();
    let clientes = db.todos_los_clientes()?;
    for ctx in &clientes {
        let propios = ajustes::ajustes_cliente(db, ctx.id())?;
        let Some((ambito, canal)) = super::correo_de(a, &propios, ctx.id()) else { continue };
        for m in db.miembros(ctx.id())? {
            let persona = ajustes::prefs_persona(db, &m.cuenta)?;
            let (p, _) = ajustes::prefs_cliente(db, ctx.id(), &m.cuenta, m.rol)?;
            if p.resumen && quiere(persona.resumen_diario, persona.resumen_semanal) {
                let e =
                    por_persona.entry((m.cuenta.clone(), ambito.clone(), canal.id.clone())).or_insert((m.correo.clone(), persona.silencio.clone(), Vec::new()));
                e.2.push(ctx.id().to_string());
            }
        }
    }
    for ((_, ambito, canal), (correo, silencio, ids)) in por_persona {
        let mut cs = Vec::new();
        for id in &ids {
            cs.extend(resumen(db, id)?);
        }
        if cs.is_empty() {
            continue;
        }
        let cliente = (ids.len() == 1).then(|| ids[0].clone());
        let m = Mensaje::Resumen(Resumen { periodo, desde, hasta: ahora, clientes: cs });
        encolar_uno(db, &ambito, cliente.as_deref(), &canal, &correo, silencio.as_ref(), Severidad::Informativo, None, &m, ahora)?;
        n += 1;
    }
    // Canales compartidos que lo quieren.
    let todos: Vec<String> = clientes.iter().map(|c| c.id().to_string()).collect();
    let mut compartidos: Vec<(String, ajustes::Canal, Vec<String>)> = Vec::new();
    for c in a.canales.iter().filter(|c| c.tipo != TipoCanal::Correo && c.activo && ajustes::completo(c)) {
        if quiere(c.reglas.resumen_diario, c.reglas.resumen_semanal) {
            let ids = c.reglas.clientes.clone().unwrap_or_else(|| todos.clone());
            compartidos.push(("servidor".into(), c.clone(), ids));
        }
    }
    for id in &todos {
        for c in ajustes::ajustes_cliente(db, id)?.canales.into_iter().filter(|c| c.tipo != TipoCanal::Correo && c.activo && ajustes::completo(c)) {
            if quiere(c.reglas.resumen_diario, c.reglas.resumen_semanal) {
                compartidos.push((super::ambito_cliente(id), c, vec![id.clone()]));
            }
        }
    }
    for (ambito, canal, ids) in compartidos {
        let mut cs = Vec::new();
        for id in &ids {
            cs.extend(resumen(db, id)?);
        }
        if cs.is_empty() {
            continue;
        }
        let cliente = ambito.strip_prefix("cliente:").map(str::to_string);
        let m = Mensaje::Resumen(Resumen { periodo, desde, hasta: ahora, clientes: cs });
        encolar_uno(db, &ambito, cliente.as_deref(), &canal.id, "", canal.reglas.silencio.as_ref(), Severidad::Informativo, None, &m, ahora)?;
        n += 1;
    }
    Ok(n)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    pub fn equipo(id: &str, nombre: &str, contacto: Option<Ts>) -> Equipo {
        Equipo {
            id: id.into(),
            nombre: nombre.into(),
            so: "windows".into(),
            version_agente: "0.7.11".into(),
            box_pub: String::new(),
            sign_pub: String::new(),
            sal_equipo: String::new(),
            etiqueta: None,
            rol: "agente".into(),
            modo: "gestionado".into(),
            confirmado: true,
            ultimo_contacto: contacto,
            estado_servicio: None,
            siguiente_seq: 1,
            atencion_hasta: None,
            resumen: None,
            espera_min_horas: None,
            etiquetas: vec![],
        }
    }

    pub fn fecha(ts: Ts) -> String {
        chrono::DateTime::from_timestamp(ts, 0).unwrap().to_rfc3339()
    }

    /// Un cliente con tres equipos: uno bien, uno con fallos y uno callado.
    pub fn cliente_de_prueba(hasta: Ts) -> ResumenCliente {
        let dia = 24 * 3600;
        let equipos = vec![
            equipo("e1", "PC-Contabilidad", Some(hasta - 300)),
            equipo("e2", "Servidor", Some(hasta - 60)),
            equipo("e3", "Portátil de Ana", Some(hasta - 3 * dia)),
        ];
        let informes = HashMap::from([
            (
                "e1".to_string(),
                json!({ "copias": [{ "id": "c1", "nombre": "Documentos", "estado": "ok", "cuando": fecha(hasta - 3600) }],
                        "repos": [{ "id": "r1", "espacio": { "en_disco_bytes": 5_368_709_120u64 },
                                    "ejecuciones": [{ "hora": fecha(hasta - 3600), "resultado": "ok" }, { "hora": fecha(hasta - dia - 3600), "resultado": "sin_cambios" },
                                                    { "hora": fecha(hasta - 9 * dia), "resultado": "fallo" }] }] }),
            ),
            (
                "e2".to_string(),
                json!({ "copias": [{ "id": "c9", "nombre": "SQL", "estado": "fallo", "cuando": fecha(hasta - 7200), "mensaje": "C:\\Datos\\base.bak bloqueado" }],
                        "repos": [{ "id": "r2", "espacio": { "en_disco_bytes": 1_610_612_736u64 },
                                    "ejecuciones": [{ "hora": fecha(hasta - 7200), "resultado": "fallo" }, { "hora": fecha(hasta - 2 * dia), "resultado": "fallo" },
                                                    { "hora": fecha(hasta - 4 * dia), "resultado": "ok" }] }] }),
            ),
        ]);
        let incidentes = vec![Incidente {
            clave: "cl|e2|copia_fallida|c9".into(),
            cliente: "cl".into(),
            equipo: Some("e2".into()),
            tipo: "copia_fallida".into(),
            sujeto: "c9".into(),
            severidad: Severidad::Critico,
            titulo: "Falló la copia «SQL» en «Servidor»".into(),
            mensaje: "bloqueado".into(),
            abierto: true,
            primero: hasta - 2 * dia,
            ultimo: hasta - 7200,
            veces: 2,
            marca: Some("x".into()),
            notificado: Some(hasta - 2 * dia),
            cerrado: None,
        }];
        de_cliente("cl", "Altamar & Asociados", &equipos, &informes, &incidentes, hasta - 7 * dia, hasta)
    }

    #[test]
    fn contenido_del_resumen() {
        let hasta = 1_791_100_800;
        let r = cliente_de_prueba(hasta);
        assert_eq!(
            r.equipos.iter().map(|e| (e.nombre.as_str(), e.estado.as_str())).collect::<Vec<_>>(),
            vec![("Servidor", "fallo"), ("Portátil de Ana", "sin_contacto"), ("PC-Contabilidad", "ok")]
        );
        let pc = &r.equipos[2];
        assert_eq!((pc.copias_ok, pc.fallos, pc.ultima_ok, pc.bytes), (2, 0, Some(hasta - 3600), Some(5_368_709_120)));
        let srv = &r.equipos[0];
        assert_eq!((srv.copias_ok, srv.fallos, srv.ultima_ok), (1, 2, Some(hasta - 4 * 24 * 3600)));
        assert_eq!((r.copias_ok, r.fallos, r.bytes), (3, 2, 5_368_709_120 + 1_610_612_736));
        assert_eq!(r.atencion, vec!["Falló la copia «SQL» en «Servidor» (2 veces)".to_string(), "«Portátil de Ana» no conecta con el servidor".to_string()]);
    }
}
