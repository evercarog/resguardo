//! Plan 0.8.1, C4 (plan-mejoras 15d; docs/infraestructura.md): **infraestructura,
//! fase 1**, para cualquier entorno (equipo físico, Hyper-V, VMware, Proxmox, NAS…).
//!
//! Por cliente, lo que la persona anota (con lo que proponen los equipos: su tipo,
//! sus discos y sus conjuntos): los **servidores físicos** (nombre, marca, modelo,
//! sede, notas), sus **conjuntos de discos** (RAID 1/5/6/10, espejo o raidz de ZFS,
//! Storage Spaces, disco suelto; discos, capacidad y estado anotado) y su
//! **hipervisor**; en qué servidor (y conjunto) está cada equipo de Resguardo (una
//! máquina virtual, un contenedor o el propio servidor) y cada destino o zona.
//!
//! Resguardo **no gestiona** nada de esto: solo sabe dónde está cada cosa para la
//! regla 3-2-1-1-0 (los soportes reales y el aviso «servidor físico compartido») y
//! la línea «Dónde está físicamente». Todo es opcional: sin nada anotado, todo
//! funciona como antes.
//!
//! Se guarda en los valores del servidor (`infra:<cliente>`), como la marca: sin
//! cambios en el esquema. **Sin secretos**: se rechaza cualquier campo de más y los
//! textos van limpios y cortos. De esta consola (la fase 1 no lo reparte por los
//! equipos como los datos comunes, docs/consolas-multiples.md §6.5).

use crate::almacen::{ahora, Almacen, Rol, Ts, R};
use crate::api::fecha_opt;
use crate::auth::Usuario;
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Como mucho tantos servidores físicos por cliente.
pub const MAX_SERVIDORES: usize = 100;
/// Como mucho tantos conjuntos de discos por servidor.
pub const MAX_CONJUNTOS: usize = 32;
/// Como mucho tantos equipos o destinos asignados.
pub const MAX_ASIGNADOS: usize = 2000;
/// El documento entero, como mucho (en JSON).
pub const MAX_BYTES: usize = 256 * 1024;

/// Los hipervisores que se conocen (`null`: sin hipervisor, un servidor de uso directo).
pub const HIPERVISORES: [&str; 8] = ["proxmox", "vmware", "hyperv", "xen", "kvm", "nutanix", "virtualbox", "otro"];
/// Las formas de un conjunto de discos.
pub const TIPOS_CONJUNTO: [&str; 13] =
    ["raid0", "raid1", "raid5", "raid6", "raid10", "zfs_espejo", "zfs_raidz1", "zfs_raidz2", "zfs_raidz3", "storage_spaces", "nas", "disco", "otro"];
/// El estado anotado de un conjunto.
pub const ESTADOS: [&str; 4] = ["bien", "degradado", "reconstruyendo", "sin_revisar"];

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Conjunto {
    pub id: String,
    pub nombre: String,
    pub tipo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discos: Option<u32>,
    /// Bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacidad: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estado: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notas: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Servidor {
    pub id: String,
    pub nombre: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marca: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modelo: Option<String>,
    /// El id de una sede del cliente (docs/sedes.md).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sede: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hipervisor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notas: Option<String>,
    #[serde(default)]
    pub conjuntos: Vec<Conjunto>,
}

/// En qué servidor físico (y, si se sabe, en qué conjunto) está un equipo o un destino.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Sitio {
    pub servidor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conjunto: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Infraestructura {
    #[serde(default)]
    pub servidores: Vec<Servidor>,
    /// Id del equipo de Resguardo → su sitio.
    #[serde(default)]
    pub equipos: BTreeMap<String, Sitio>,
    /// Clave del destino (la del catálogo: `zona:<almacén>:<zona>`, el id de un destino,
    /// `nube:<almacén>:<nombre>`, `espejo:<almacén>:<resumen>`) → su sitio.
    #[serde(default)]
    pub destinos: BTreeMap<String, Sitio>,
}

/// Lo que se guarda: el documento, cuándo y quién.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct Guardada {
    #[serde(default)]
    infra: Infraestructura,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    actualizada: Option<Ts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    por: Option<String>,
}

fn clave(cliente: &str) -> String {
    format!("infra:{cliente}")
}

fn leer_guardada(db: &dyn Almacen, cliente: &str) -> R<Guardada> {
    Ok(db.valor(&clave(cliente))?.filter(|v| !v.is_empty()).and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default())
}

/// La infraestructura de un cliente (vacía si nadie anotó nada).
pub fn leer(db: &dyn Almacen, cliente: &str) -> R<Infraestructura> {
    Ok(leer_guardada(db, cliente)?.infra)
}

/// Un id: letras minúsculas, cifras y `-` (de 1 a 40).
pub fn id_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Un id de equipo o la clave de un destino (como los del catálogo): sin espacios ni barras.
fn clave_valida(k: &str) -> bool {
    !k.is_empty() && k.len() <= 200 && k.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '-' | '_' | '.'))
}

/// Un texto de una línea, limpio, de 1 a `max` caracteres (o `None` si va vacío).
fn texto(t: &str, max: usize, campo: &str, lineas: bool) -> Result<Option<String>, String> {
    let t = t.replace("\r\n", "\n");
    let t = if lineas { t.trim().to_string() } else { t.split_whitespace().collect::<Vec<_>>().join(" ") };
    if t.is_empty() {
        return Ok(None);
    }
    if t.chars().count() > max || t.chars().any(|c| c.is_control() && !(lineas && (c == '\n' || c == '\t'))) {
        return Err(format!("{campo}: como mucho {max} caracteres, sin caracteres raros."));
    }
    Ok(Some(t))
}

fn opcional(t: &Option<String>, max: usize, campo: &str, lineas: bool) -> Result<Option<String>, String> {
    match t {
        None => Ok(None),
        Some(t) => texto(t, max, campo, lineas),
    }
}

/// Comprueba y limpia el documento entero (lo que se guarda).
pub fn limpiar(mut d: Infraestructura) -> Result<Infraestructura, String> {
    if d.servidores.len() > MAX_SERVIDORES {
        return Err(format!("Como mucho {MAX_SERVIDORES} servidores físicos."));
    }
    if d.equipos.len() > MAX_ASIGNADOS || d.destinos.len() > MAX_ASIGNADOS {
        return Err(format!("Como mucho {MAX_ASIGNADOS} equipos y {MAX_ASIGNADOS} destinos asignados."));
    }
    let mut ids = BTreeSet::new();
    let mut conjuntos: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for s in &mut d.servidores {
        if !id_valido(&s.id) || !ids.insert(s.id.clone()) {
            return Err("Hay un servidor con un id no válido o repetido.".into());
        }
        s.nombre = texto(&s.nombre, 80, "El nombre del servidor", false)?.ok_or("Cada servidor necesita un nombre.")?;
        s.marca = opcional(&s.marca, 80, "La marca", false)?;
        s.modelo = opcional(&s.modelo, 120, "El modelo", false)?;
        s.notas = opcional(&s.notas, 1000, "Las notas", true)?;
        s.sede = s.sede.take().filter(|x| !x.is_empty());
        if s.sede.as_deref().is_some_and(|x| !clave_valida(x)) {
            return Err("La sede de un servidor no es válida.".into());
        }
        s.hipervisor = s.hipervisor.take().filter(|x| !x.is_empty());
        if s.hipervisor.as_deref().is_some_and(|h| !HIPERVISORES.contains(&h)) {
            return Err("Ese hipervisor no se conoce (elige «Otro»).".into());
        }
        if s.conjuntos.len() > MAX_CONJUNTOS {
            return Err(format!("Como mucho {MAX_CONJUNTOS} conjuntos de discos por servidor."));
        }
        let mut suyos = BTreeSet::new();
        for c in &mut s.conjuntos {
            if !id_valido(&c.id) || !suyos.insert(c.id.clone()) {
                return Err("Hay un conjunto de discos con un id no válido o repetido.".into());
            }
            c.nombre = texto(&c.nombre, 80, "El nombre del conjunto", false)?.ok_or("Cada conjunto de discos necesita un nombre.")?;
            if !TIPOS_CONJUNTO.contains(&c.tipo.as_str()) {
                return Err("Ese tipo de conjunto de discos no se conoce (elige «Otro»).".into());
            }
            if c.discos.is_some_and(|n| n == 0 || n > 256) {
                return Err("Un conjunto tiene de 1 a 256 discos.".into());
            }
            if c.capacidad.is_some_and(|b| b > 1 << 60) {
                return Err("Esa capacidad no es razonable.".into());
            }
            c.estado = c.estado.take().filter(|x| !x.is_empty());
            if c.estado.as_deref().is_some_and(|e| !ESTADOS.contains(&e)) {
                return Err("Ese estado no se conoce.".into());
            }
            c.notas = opcional(&c.notas, 1000, "Las notas", true)?;
        }
        conjuntos.insert(s.id.clone(), suyos);
    }
    for (k, sitio) in d.equipos.iter().chain(d.destinos.iter()) {
        if !clave_valida(k) {
            return Err("Hay un equipo o un destino con una clave no válida.".into());
        }
        let Some(suyos) = conjuntos.get(&sitio.servidor) else {
            return Err("Un equipo o un destino apunta a un servidor que no existe.".into());
        };
        if sitio.conjunto.as_ref().is_some_and(|c| !suyos.contains(c)) {
            return Err("Un equipo o un destino apunta a un conjunto de discos que no es de su servidor.".into());
        }
    }
    let n = serde_json::to_string(&d).map_err(|e| e.to_string())?.len();
    if n > MAX_BYTES {
        return Err("Demasiados datos: como mucho 256 KB.".into());
    }
    Ok(d)
}

/// Lo que ve la consola: `{ servidores, equipos, destinos, actualizada, por }`.
fn json_de(g: &Guardada) -> Value {
    json!({
        "servidores": g.infra.servidores,
        "equipos": g.infra.equipos,
        "destinos": g.infra.destinos,
        "actualizada": fecha_opt(g.actualizada),
        "por": g.por,
    })
}

/// `GET /api/clientes/{c}/infraestructura` (cualquier miembro).
pub async fn ver(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    u.miembro(&st, &c, Rol::Lectura).await?;
    let g = st.db(move |db| leer_guardada(db, &c)).await?;
    Ok(Json(json_de(&g)))
}

/// `PUT /api/clientes/{c}/infraestructura` (administradores): el documento entero. Auditado.
pub async fn poner(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(d): Json<Infraestructura>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let d = limpiar(d).map_err(ErrorApi::datos)?;
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let por = u.0.cuenta.nombre.clone();
    let c2 = c.clone();
    let g = st
        .db(move |db| {
            let resumen = json!({ "servidores": d.servidores.len(), "equipos": d.equipos.len(), "destinos": d.destinos.len() });
            let g = Guardada { infra: d, actualizada: Some(ahora()), por: Some(por) };
            db.poner_valor(&clave(&c2), &serde_json::to_string(&g).map_err(|e| e.to_string())?)?;
            db.auditar(&ctx, &actor, "cambiar_infraestructura", &c2, &resumen.to_string())?;
            Ok(g)
        })
        .await?;
    Ok(Json(json_de(&g)))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn ejemplo() -> Value {
        json!({
            "servidores": [{
                "id": "hv-01", "nombre": " hv-01 ", "marca": "Marca X", "modelo": "Modelo 1U", "sede": "oficina-central",
                "hipervisor": "proxmox", "notas": "Rack 1\nabajo",
                "conjuntos": [
                    { "id": "tank", "nombre": "tank", "tipo": "zfs_espejo", "discos": 2, "capacidad": 1_200_000_000_000u64, "estado": "bien" },
                    { "id": "ssd", "nombre": "SSD del sistema", "tipo": "disco" }
                ]
            }],
            "equipos": { "e-vm": { "servidor": "hv-01", "conjunto": "ssd" } },
            "destinos": { "zona:e-almacen:principal": { "servidor": "hv-01", "conjunto": "tank" } }
        })
    }

    fn leer_json(v: Value) -> Result<Infraestructura, String> {
        let d: Infraestructura = serde_json::from_value(v).map_err(|e| e.to_string())?;
        limpiar(d)
    }

    #[test]
    fn se_limpia_y_se_valida() {
        let d = leer_json(ejemplo()).unwrap();
        assert_eq!(d.servidores[0].nombre, "hv-01");
        assert_eq!(d.servidores[0].notas.as_deref(), Some("Rack 1\nabajo"));
        assert_eq!(d.destinos["zona:e-almacen:principal"].conjunto.as_deref(), Some("tank"));
        // Vacía: vale (sin nada anotado, todo como antes).
        assert_eq!(leer_json(json!({})).unwrap(), Infraestructura::default());
    }

    #[test]
    fn rechaza_lo_que_no_cuadra() {
        let con = |f: &dyn Fn(&mut Value)| {
            let mut v = ejemplo();
            f(&mut v);
            leer_json(v)
        };
        assert!(con(&|v| v["servidores"][0]["contrasena"] = json!("x")).is_err(), "sin campos de más (ni secretos)");
        assert!(con(&|v| v["clave"] = json!("x")).is_err());
        assert!(con(&|v| v["servidores"][0]["id"] = json!("HV 01")).is_err());
        assert!(con(&|v| v["servidores"][0]["nombre"] = json!("  ")).is_err());
        assert!(con(&|v| v["servidores"][0]["hipervisor"] = json!("mainframe")).is_err());
        assert!(con(&|v| v["servidores"][0]["conjuntos"][0]["tipo"] = json!("raid99")).is_err());
        assert!(con(&|v| v["servidores"][0]["conjuntos"][0]["discos"] = json!(0)).is_err());
        assert!(con(&|v| v["servidores"][0]["conjuntos"][1]["id"] = json!("tank")).is_err(), "repetido");
        assert!(con(&|v| v["servidores"][0]["conjuntos"][0]["estado"] = json!("roto")).is_err());
        assert!(con(&|v| v["servidores"][0]["nombre"] = json!("a\u{7}b")).is_err());
        assert!(con(&|v| v["equipos"]["e-vm"]["servidor"] = json!("hv-09")).is_err(), "servidor que no existe");
        assert!(con(&|v| v["destinos"]["zona:e-almacen:principal"]["conjunto"] = json!("otro")).is_err(), "conjunto de otro servidor");
        assert!(con(&|v| v["destinos"] = json!({ "../x": { "servidor": "hv-01" } })).is_err());
        assert!(con(&|v| v["servidores"][0]["modelo"] = json!("m".repeat(121))).is_err());
        // Un hipervisor vacío es «sin hipervisor».
        assert_eq!(con(&|v| v["servidores"][0]["hipervisor"] = json!("")).unwrap().servidores[0].hipervisor, None);
    }

    #[test]
    fn se_guarda_por_cliente() {
        use crate::almacen::sqlite::Sqlite;
        let dir = tempfile::tempdir().unwrap();
        let db = Sqlite::abrir(dir.path()).unwrap();
        assert_eq!(leer(&db, "c1").unwrap(), Infraestructura::default());
        let d = leer_json(ejemplo()).unwrap();
        let g = Guardada { infra: d.clone(), actualizada: Some(ahora()), por: Some("Ana".into()) };
        db.poner_valor(&clave("c1"), &serde_json::to_string(&g).unwrap()).unwrap();
        assert_eq!(leer(&db, "c1").unwrap(), d);
        assert_eq!(leer(&db, "c2").unwrap(), Infraestructura::default(), "cada cliente lo suyo");
        let j = json_de(&g);
        assert_eq!(j["servidores"][0]["conjuntos"][0]["tipo"], "zfs_espejo");
        assert_eq!(j["por"], "Ana");
    }
}
