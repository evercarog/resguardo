//! Plan 0.8.1, C4 (plan-mejoras 15d; docs/infraestructura.md): **infraestructura**
//! para cualquier entorno (equipo físico, Hyper-V, VMware, Proxmox, NAS…).
//!
//! Por cliente, lo que la persona anota (con lo que proponen los equipos: su tipo,
//! sus discos y sus conjuntos): los **servidores físicos** (nombre, marca, modelo,
//! sede, notas), sus **discos físicos uno a uno** (0.8.2, A2: nombre, tipo, modelo,
//! número de serie escrito a mano, tamaño, bahía, estado), sus **conjuntos de
//! discos** (RAID 1/5/6/10, espejo o raidz de ZFS, Storage Spaces, disco suelto;
//! cada uno con sus discos, si se dicen) y su **hipervisor**; en qué servidor (y
//! conjunto o disco) está cada equipo de Resguardo (una máquina virtual, un
//! contenedor o el propio servidor, y desde la 0.8.2 cada uno de sus volúmenes) y
//! cada destino o zona.
//!
//! Resguardo **no gestiona** nada de esto: solo sabe dónde está cada cosa para la
//! regla 3-2-1-1-0 (los soportes reales y el aviso «servidor físico compartido») y
//! la línea «Dónde está físicamente». Todo es opcional: sin nada anotado, todo
//! funciona como antes. **Sin secretos**: se rechaza cualquier campo de más y los
//! textos van limpios y cortos.
//!
//! Se guarda en los valores del servidor (`infra:<cliente>`), como la marca: sin
//! cambios en el esquema. **0.8.2 (A5): es un dato común del cliente** (clase
//! `infra:` de `resguardo_protocolo::datos_cliente`, docs/consolas-multiples.md
//! §6.5): cada servidor, cada equipo y cada destino es un dato que viaja por los
//! equipos del cliente. Lo de aquí es la copia local (lo que enseña la consola);
//! `crate::datos_comunes` la lee (`valor_local`, `claves`) y la cambia (`aplicar`)
//! con lo que llega de las demás consolas, y un cambio hecho aquí queda «por
//! enviar» (`poner`). Lo de antes de la 0.8.2 se reparte una vez como semilla.

use crate::almacen::{ahora, Almacen, ClienteCtx, Rol, Ts, R};
use crate::api::fecha_opt;
use crate::auth::Usuario;
use crate::error::{ErrorApi, Res};
use crate::estado::St;
use axum::extract::{Path, State};
use axum::Json;
use resguardo_protocolo::datos_cliente as dc;
pub use resguardo_protocolo::datos_cliente::{ESTADOS_CONJUNTO as ESTADOS, ESTADOS_DISCO, HIPERVISORES, TIPOS_CONJUNTO, TIPOS_DISCO};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Como mucho tantos servidores físicos por cliente.
pub const MAX_SERVIDORES: usize = 100;
/// Como mucho tantos conjuntos de discos por servidor.
pub const MAX_CONJUNTOS: usize = dc::MAX_CONJUNTOS_SERVIDOR;
/// 0.8.2 (A2): como mucho tantos discos físicos por servidor.
pub const MAX_DISCOS: usize = dc::MAX_DISCOS_SERVIDOR;
/// Como mucho tantos equipos o destinos asignados.
pub const MAX_ASIGNADOS: usize = 2000;
/// El documento entero, como mucho (en JSON).
pub const MAX_BYTES: usize = 256 * 1024;

/// 0.8.2 (A2, 15vv): un disco físico de un servidor. El número de serie lo escribe la persona.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Disco {
    pub id: String,
    pub nombre: String,
    /// `ssd`, `hdd`, `nvme` u `otro`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tipo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modelo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serie: Option<String>,
    /// Bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tamano: Option<u64>,
    /// Bahía o ranura («2», «M.2 de abajo»).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bahia: Option<String>,
    /// `bien`, `aviso`, `averiado` o `sin_revisar` (lo dice la persona).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estado: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notas: Option<String>,
    /// Cómo lo llama el equipo que es este servidor (`disco0`, `sda`), si se sabe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visto: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Conjunto {
    pub id: String,
    pub nombre: String,
    pub tipo: String,
    /// Cuántos discos (lo de antes de la 0.8.2; con `miembros`, se cuentan).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discos: Option<u32>,
    /// 0.8.2 (A2): los discos del servidor que lo forman (sus ids).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub miembros: Vec<String>,
    /// Bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacidad: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estado: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notas: Option<String>,
    /// 0.8.2 (A3): cómo lo llama el equipo que es este servidor (`md0`, `tank`, `disco1`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visto: Option<String>,
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
    /// 0.8.2 (A2): sus discos físicos.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub discos: Vec<Disco>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conjuntos: Vec<Conjunto>,
}

/// 0.8.2 (A3, 15ww): dónde está un volumen de un equipo (`C:`, `/srv`): un conjunto o un disco
/// de su servidor.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Volumen {
    pub punto: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conjunto: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disco: Option<String>,
}

/// En qué servidor físico (y, si se sabe, en qué conjunto o disco) está un equipo o un destino.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Sitio {
    pub servidor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conjunto: Option<String>,
    /// 0.8.2 (A2): un disco suelto del servidor (en vez de un conjunto).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disco: Option<String>,
    /// 0.8.2 (A3): solo de un equipo, cada volumen en su sitio.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub volumenes: Vec<Volumen>,
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

fn guardar(db: &dyn Almacen, cliente: &str, g: &Guardada) -> R<()> {
    db.poner_valor(&clave(cliente), &serde_json::to_string(g).map_err(|e| e.to_string())?)
}

/// La infraestructura de un cliente (vacía si nadie anotó nada).
pub fn leer(db: &dyn Almacen, cliente: &str) -> R<Infraestructura> {
    Ok(leer_guardada(db, cliente)?.infra)
}

/// Un id: letras minúsculas, cifras y `-` (de 1 a 40).
pub fn id_valido(id: &str) -> bool {
    dc::id_infra_valido(id)
}

/// Un id de equipo o la clave de un destino (como los del catálogo): sin espacios ni barras.
fn clave_valida(k: &str) -> bool {
    dc::clave_destino_infra_valida(k)
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

/// Uno de una lista cerrada (vacío: nada).
fn de_lista(x: &mut Option<String>, lista: &[&str], error: &str) -> Result<(), String> {
    *x = x.take().filter(|v| !v.is_empty());
    if x.as_deref().is_some_and(|v| !lista.contains(&v)) {
        return Err(error.into());
    }
    Ok(())
}

/// 0.8.2: `id` de un conjunto o un disco opcional, vacío → nada.
fn vacio_nada(x: &mut Option<String>) {
    *x = x.take().filter(|v| !v.is_empty());
}

// ---------------------------------------------------------------------------
// 0.8.2 (A5): de la copia de aquí a los datos comunes y al revés
// ---------------------------------------------------------------------------

/// El prefijo de los datos comunes de cada parte.
pub const PREFIJO_SERVIDOR: &str = "infra:servidor:";
pub const PREFIJO_EQUIPO: &str = "infra:equipo:";
pub const PREFIJO_DESTINO: &str = "infra:destino:";

/// Un servidor como dato común (sin su id, que va en la clave).
pub fn valor_servidor(s: &Servidor) -> Value {
    let mut v = serde_json::to_value(s).unwrap_or(Value::Null);
    if let Some(m) = v.as_object_mut() {
        m.remove("id");
    }
    v
}

/// Un sitio como dato común.
pub fn valor_sitio(s: &Sitio) -> Value {
    serde_json::to_value(s).unwrap_or(Value::Null)
}

/// Todos los datos comunes de una infraestructura: clave → valor.
pub fn datos(d: &Infraestructura) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for s in &d.servidores {
        out.insert(format!("{PREFIJO_SERVIDOR}{}", s.id), valor_servidor(s));
    }
    for (k, s) in &d.equipos {
        out.insert(format!("{PREFIJO_EQUIPO}{k}"), valor_sitio(s));
    }
    for (k, s) in &d.destinos {
        out.insert(format!("{PREFIJO_DESTINO}{k}"), valor_sitio(s));
    }
    out
}

/// Lo que ocupa aproximadamente la infraestructura en el documento de cada equipo.
pub fn bytes_comunes(d: &Infraestructura) -> usize {
    datos(d).iter().map(|(k, v)| dc::bytes_dato(k, v)).sum()
}

/// El valor de un dato común `infra:` según lo de aquí (`null`: nada anotado).
pub fn valor_local(db: &dyn Almacen, cliente: &str, clave_dato: &str) -> R<Value> {
    Ok(datos(&leer(db, cliente)?).remove(clave_dato).unwrap_or(Value::Null))
}

/// Las claves de los datos comunes de lo anotado aquí (para repartirlo como semilla).
pub fn claves(db: &dyn Almacen, cliente: &str) -> R<Vec<String>> {
    Ok(datos(&leer(db, cliente)?).into_keys().collect())
}

/// Pone aquí lo que llega de otra consola (ya validado por el protocolo). Un sitio que apunta a
/// un servidor que aún no ha llegado se guarda igual: la consola no lo enseña hasta que llegue.
pub fn aplicar(db: &dyn Almacen, cliente: &str, clave_dato: &str, valor: &Value, por: &str) -> R<()> {
    let mut g = leer_guardada(db, cliente)?;
    let d = &mut g.infra;
    if let Some(id) = clave_dato.strip_prefix(PREFIJO_SERVIDOR) {
        let nuevo = if valor.is_null() {
            None
        } else {
            let mut v = valor.clone();
            v["id"] = json!(id);
            Some(serde_json::from_value::<Servidor>(v).map_err(|e| format!("Servidor de otra consola que no se entiende: {e}"))?)
        };
        match (d.servidores.iter().position(|s| s.id == id), nuevo) {
            (Some(i), Some(s)) => d.servidores[i] = s,
            (None, Some(s)) if d.servidores.len() < MAX_SERVIDORES => d.servidores.push(s),
            (None, Some(_)) => return Ok(()),
            (Some(i), None) => {
                d.servidores.remove(i);
            }
            (None, None) => {}
        }
    } else {
        let (mapa, k) = match (clave_dato.strip_prefix(PREFIJO_EQUIPO), clave_dato.strip_prefix(PREFIJO_DESTINO)) {
            (Some(k), _) => (&mut d.equipos, k),
            (_, Some(k)) => (&mut d.destinos, k),
            _ => return Ok(()),
        };
        if valor.is_null() {
            mapa.remove(k);
        } else if mapa.len() < MAX_ASIGNADOS || mapa.contains_key(k) {
            let s: Sitio = serde_json::from_value(valor.clone()).map_err(|e| format!("Sitio de otra consola que no se entiende: {e}"))?;
            mapa.insert(k.to_string(), s);
        }
    }
    g.actualizada = Some(ahora());
    g.por = Some(dc::texto_corto(por, 120));
    guardar(db, cliente, &g)
}

// ---------------------------------------------------------------------------
// Comprobar y limpiar
// ---------------------------------------------------------------------------

/// Comprueba y limpia el documento entero (lo que se guarda). `previa`: lo que había; un
/// sitio que ya estaba igual y apuntaba a un servidor que no está (llegó de otra consola antes
/// que su servidor) se deja tal cual.
pub fn limpiar(mut d: Infraestructura, previa: Option<&Infraestructura>) -> Result<Infraestructura, String> {
    if d.servidores.len() > MAX_SERVIDORES {
        return Err(format!("Como mucho {MAX_SERVIDORES} servidores físicos."));
    }
    if d.equipos.len() > MAX_ASIGNADOS || d.destinos.len() > MAX_ASIGNADOS {
        return Err(format!("Como mucho {MAX_ASIGNADOS} equipos y {MAX_ASIGNADOS} destinos asignados."));
    }
    let mut ids = BTreeSet::new();
    // De cada servidor: sus conjuntos y sus discos.
    let mut dentro: BTreeMap<String, (BTreeSet<String>, BTreeSet<String>)> = BTreeMap::new();
    for s in &mut d.servidores {
        if !id_valido(&s.id) || !ids.insert(s.id.clone()) {
            return Err("Hay un servidor con un id no válido o repetido.".into());
        }
        s.nombre = texto(&s.nombre, 80, "El nombre del servidor", false)?.ok_or("Cada servidor necesita un nombre.")?;
        s.marca = opcional(&s.marca, 80, "La marca", false)?;
        s.modelo = opcional(&s.modelo, 120, "El modelo", false)?;
        s.notas = opcional(&s.notas, 1000, "Las notas", true)?;
        s.sede = s.sede.take().filter(|x| !x.is_empty());
        if s.sede.as_deref().is_some_and(|x| !dc::id_simple(x)) {
            return Err("La sede de un servidor no es válida.".into());
        }
        de_lista(&mut s.hipervisor, &HIPERVISORES, "Ese hipervisor no se conoce (elige «Otro»).")?;
        if s.conjuntos.len() > MAX_CONJUNTOS {
            return Err(format!("Como mucho {MAX_CONJUNTOS} conjuntos de discos por servidor."));
        }
        if s.discos.len() > MAX_DISCOS {
            return Err(format!("Como mucho {MAX_DISCOS} discos por servidor."));
        }
        let mut discos = BTreeSet::new();
        for k in &mut s.discos {
            if !id_valido(&k.id) || !discos.insert(k.id.clone()) {
                return Err("Hay un disco con un id no válido o repetido.".into());
            }
            k.nombre = texto(&k.nombre, 80, "El nombre del disco", false)?.ok_or("Cada disco necesita un nombre.")?;
            de_lista(&mut k.tipo, &TIPOS_DISCO, "Ese tipo de disco no se conoce (elige «Otro»).")?;
            k.modelo = opcional(&k.modelo, 120, "El modelo del disco", false)?;
            k.serie = opcional(&k.serie, 64, "El número de serie", false)?;
            k.tamano = k.tamano.filter(|b| *b > 0);
            if k.tamano.is_some_and(|b| b > 1 << 60) {
                return Err("Ese tamaño de disco no es razonable.".into());
            }
            k.bahia = opcional(&k.bahia, 40, "La bahía", false)?;
            de_lista(&mut k.estado, &ESTADOS_DISCO, "Ese estado de disco no se conoce.")?;
            k.notas = opcional(&k.notas, 500, "Las notas del disco", true)?;
            k.visto = opcional(&k.visto, 64, "Lo que ve el equipo", false)?;
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
            let mut vistos = BTreeSet::new();
            for m in &c.miembros {
                if !discos.contains(m) || !vistos.insert(m.clone()) {
                    return Err("Un conjunto usa un disco que no es de su servidor (o lo repite).".into());
                }
            }
            c.capacidad = c.capacidad.filter(|b| *b > 0);
            if c.capacidad.is_some_and(|b| b > 1 << 60) {
                return Err("Esa capacidad no es razonable.".into());
            }
            de_lista(&mut c.estado, &ESTADOS, "Ese estado no se conoce.")?;
            c.notas = opcional(&c.notas, 1000, "Las notas", true)?;
            c.visto = opcional(&c.visto, 64, "Lo que ve el equipo", false)?;
        }
        dentro.insert(s.id.clone(), (suyos, discos));
    }
    let ya = |es_equipo: bool, k: &str, s: &Sitio| previa.is_some_and(|p| if es_equipo { p.equipos.get(k) } else { p.destinos.get(k) } == Some(s));
    for (es_equipo, mapa) in [(true, &mut d.equipos), (false, &mut d.destinos)] {
        for (k, sitio) in mapa.iter_mut() {
            if !clave_valida(k) {
                return Err("Hay un equipo o un destino con una clave no válida.".into());
            }
            vacio_nada(&mut sitio.conjunto);
            vacio_nada(&mut sitio.disco);
            if sitio.conjunto.is_some() && sitio.disco.is_some() {
                return Err("Un equipo o un destino va en un conjunto de discos o en un disco, no en los dos.".into());
            }
            if !es_equipo && !sitio.volumenes.is_empty() {
                return Err("Solo un equipo tiene volúmenes.".into());
            }
            if sitio.volumenes.len() > dc::MAX_VOLUMENES_EQUIPO {
                return Err("Como mucho 32 volúmenes anotados por equipo.".into());
            }
            let mut puntos = BTreeSet::new();
            let mut volumenes = Vec::new();
            for mut v in std::mem::take(&mut sitio.volumenes) {
                v.punto = texto(&v.punto, 64, "El volumen", false)?.ok_or("Un volumen sin nombre.")?;
                vacio_nada(&mut v.conjunto);
                vacio_nada(&mut v.disco);
                if v.conjunto.is_some() && v.disco.is_some() {
                    return Err("Un volumen va en un conjunto de discos o en un disco, no en los dos.".into());
                }
                // Un volumen sin sitio es «lo del equipo»: no hace falta guardarlo.
                if v.conjunto.is_none() && v.disco.is_none() {
                    continue;
                }
                if !puntos.insert(v.punto.clone()) {
                    return Err("Un volumen está dos veces.".into());
                }
                volumenes.push(v);
            }
            sitio.volumenes = volumenes;
            let Some((conjuntos, discos)) = dentro.get(&sitio.servidor) else {
                if ya(es_equipo, k, sitio) {
                    continue;
                }
                return Err("Un equipo o un destino apunta a un servidor que no existe.".into());
            };
            let fuera = sitio.conjunto.as_ref().is_some_and(|c| !conjuntos.contains(c))
                || sitio.disco.as_ref().is_some_and(|x| !discos.contains(x))
                || sitio
                    .volumenes
                    .iter()
                    .any(|v| v.conjunto.as_ref().is_some_and(|c| !conjuntos.contains(c)) || v.disco.as_ref().is_some_and(|x| !discos.contains(x)));
            if fuera {
                return Err("Un equipo o un destino apunta a un conjunto o a un disco que no es de su servidor.".into());
            }
        }
    }
    // Cada parte tiene que valer como dato común (lo mismo en todas las consolas).
    let todos = datos(&d);
    for (k, v) in &todos {
        dc::valor_valido(k, v).map_err(|e| format!("No se puede compartir con las demás consolas: {e}"))?;
    }
    let comunes: usize = todos.iter().map(|(k, v)| dc::bytes_dato(k, v)).sum();
    if comunes > dc::MAX_BYTES_INFRA {
        return Err(format!(
            "Demasiado anotado para repartirlo a todas las consolas (como mucho {} KB): acorta las notas o quita discos que no hagan falta.",
            dc::MAX_BYTES_INFRA / 1024
        ));
    }
    let n = serde_json::to_string(&d).map_err(|e| e.to_string())?.len();
    if n > MAX_BYTES {
        return Err("Demasiados datos: como mucho 256 KB.".into());
    }
    Ok(d)
}

/// ¿Guarda algún equipo confirmado del cliente la infraestructura para todas sus consolas?
fn compartida(db: &dyn Almacen, ctx: &ClienteCtx) -> R<bool> {
    Ok(db
        .equipos(ctx)?
        .iter()
        .any(|e| e.confirmado && e.resumen.as_ref().and_then(|r| r["admite"].as_array()).is_some_and(|a| a.iter().any(|x| x == dc::ADMITE_INFRA))))
}

/// Lo que ve la consola: `{ servidores, equipos, destinos, actualizada, por, comun, bytes_comunes }`.
fn json_de(g: &Guardada, comun: bool) -> Value {
    json!({
        "servidores": g.infra.servidores,
        "equipos": g.infra.equipos,
        "destinos": g.infra.destinos,
        "actualizada": fecha_opt(g.actualizada),
        "por": g.por,
        "comun": comun,
        "bytes_comunes": bytes_comunes(&g.infra),
    })
}

/// Para el panel de todos los clientes: lo anotado (sin cuándo ni quién).
pub fn para_panel(db: &dyn Almacen, cliente: &str) -> R<Value> {
    let d = leer(db, cliente)?;
    Ok(if d.servidores.is_empty() { Value::Null } else { json!({ "servidores": d.servidores, "equipos": d.equipos, "destinos": d.destinos }) })
}

/// `GET /api/clientes/{c}/infraestructura` (cualquier miembro).
pub async fn ver(State(st): State<St>, u: Usuario, Path(c): Path<String>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Lectura).await?;
    let v = st.db(move |db| Ok(json_de(&leer_guardada(db, &c)?, compartida(db, &ctx)?))).await?;
    Ok(Json(v))
}

/// `PUT /api/clientes/{c}/infraestructura` (administradores): el documento entero. Auditado.
/// 0.8.2 (A5): lo que cambia queda «por enviar» a los equipos como dato común.
pub async fn poner(State(st): State<St>, u: Usuario, Path(c): Path<String>, Json(d): Json<Infraestructura>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let por = u.0.cuenta.nombre.clone();
    let propia = st.identidad_pub.clone();
    let c2 = c.clone();
    let v = st
        .db_crudo(move |db| {
            let previa = leer(db, &c2)?;
            let d = limpiar(d, Some(&previa))?;
            let resumen = json!({ "servidores": d.servidores.len(), "equipos": d.equipos.len(), "destinos": d.destinos.len() });
            let (antes, despues) = (datos(&previa), datos(&d));
            let g = Guardada { infra: d, actualizada: Some(ahora()), por: Some(por.clone()) };
            guardar(db, &c2, &g)?;
            db.auditar(&ctx, &actor, "cambiar_infraestructura", &c2, &resumen.to_string())?;
            // Lo que cambió, para las demás consolas (si algún equipo lo guarda).
            let cambiadas: BTreeSet<&String> = antes.keys().chain(despues.keys()).filter(|k| antes.get(*k) != despues.get(*k)).collect();
            for k in cambiadas {
                crate::datos_comunes::registrar_lo_de_aqui(db, &ctx, k, &por, &propia);
            }
            Ok(json_de(&g, compartida(db, &ctx)?))
        })
        .await?
        .map_err(ErrorApi::datos)?;
    st.vivo.avisar(&c, crate::vivo::Cambio::DatosComunes);
    Ok(Json(v))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn ejemplo() -> Value {
        json!({
            "servidores": [{
                "id": "hv-01", "nombre": " hv-01 ", "marca": "Marca X", "modelo": "Modelo 1U", "sede": "oficina-central",
                "hipervisor": "proxmox", "notas": "Rack 1\nabajo",
                "discos": [
                    { "id": "b1", "nombre": "Bahía 1", "tipo": "hdd", "serie": "SN-0001", "tamano": 2_000_000_000_000u64, "bahia": "1", "estado": "bien" },
                    { "id": "b2", "nombre": "Bahía 2", "tipo": "hdd", "serie": "SN-0002", "tamano": 2_000_000_000_000u64, "bahia": "2" },
                    { "id": "m2", "nombre": "SSD del sistema", "tipo": "nvme", "tamano": 0, "visto": "nvme0n1" }
                ],
                "conjuntos": [
                    { "id": "tank", "nombre": "tank", "tipo": "zfs_espejo", "discos": 2, "miembros": ["b1", "b2"], "capacidad": 1_200_000_000_000u64, "estado": "bien" },
                    { "id": "ssd", "nombre": "SSD del sistema", "tipo": "disco" }
                ]
            }],
            "equipos": { "e-vm": { "servidor": "hv-01", "conjunto": "ssd", "volumenes": [{ "punto": "D:", "conjunto": "tank" }, { "punto": "E:" }] } },
            "destinos": { "zona:e-almacen:principal": { "servidor": "hv-01", "conjunto": "tank" } }
        })
    }

    fn leer_json(v: Value) -> Result<Infraestructura, String> {
        let d: Infraestructura = serde_json::from_value(v).map_err(|e| e.to_string())?;
        limpiar(d, None)
    }

    #[test]
    fn se_limpia_y_se_valida() {
        let d = leer_json(ejemplo()).unwrap();
        assert_eq!(d.servidores[0].nombre, "hv-01");
        assert_eq!(d.servidores[0].notas.as_deref(), Some("Rack 1\nabajo"));
        assert_eq!(d.servidores[0].discos[2].tamano, None, "un tamaño de 0 es «sin decir»");
        assert_eq!(d.destinos["zona:e-almacen:principal"].conjunto.as_deref(), Some("tank"));
        assert_eq!(
            d.equipos["e-vm"].volumenes,
            vec![Volumen { punto: "D:".into(), conjunto: Some("tank".into()), disco: None }],
            "un volumen sin sitio no se guarda"
        );
        // Vacía: vale (sin nada anotado, todo como antes).
        assert_eq!(leer_json(json!({})).unwrap(), Infraestructura::default());
        // Lo de antes de la 0.8.2 (sin discos ni volúmenes) sigue valiendo.
        let vieja = json!({ "servidores": [{ "id": "hv-01", "nombre": "hv-01", "conjuntos": [{ "id": "tank", "nombre": "tank", "tipo": "zfs_espejo", "discos": 2 }] }], "equipos": { "e": { "servidor": "hv-01", "conjunto": "tank" } } });
        assert!(leer_json(vieja).is_ok());
    }

    #[test]
    fn rechaza_lo_que_no_cuadra() {
        let con = |f: &dyn Fn(&mut Value)| {
            let mut v = ejemplo();
            f(&mut v);
            leer_json(v)
        };
        assert!(con(&|v| v["servidores"][0]["contrasena"] = json!("x")).is_err(), "sin campos de más (ni secretos)");
        assert!(con(&|v| v["servidores"][0]["discos"][0]["clave"] = json!("x")).is_err());
        assert!(con(&|v| v["clave"] = json!("x")).is_err());
        assert!(con(&|v| v["servidores"][0]["id"] = json!("HV 01")).is_err());
        assert!(con(&|v| v["servidores"][0]["nombre"] = json!("  ")).is_err());
        assert!(con(&|v| v["servidores"][0]["hipervisor"] = json!("mainframe")).is_err());
        assert!(con(&|v| v["servidores"][0]["conjuntos"][0]["tipo"] = json!("raid99")).is_err());
        assert!(con(&|v| v["servidores"][0]["conjuntos"][0]["discos"] = json!(0)).is_err());
        assert!(con(&|v| v["servidores"][0]["conjuntos"][1]["id"] = json!("tank")).is_err(), "repetido");
        assert!(con(&|v| v["servidores"][0]["conjuntos"][0]["estado"] = json!("roto")).is_err());
        assert!(con(&|v| v["servidores"][0]["conjuntos"][0]["miembros"] = json!(["b9"])).is_err(), "disco de otro servidor");
        assert!(con(&|v| v["servidores"][0]["discos"][1]["id"] = json!("b1")).is_err(), "disco repetido");
        assert!(con(&|v| v["servidores"][0]["discos"][0]["tipo"] = json!("cinta")).is_err());
        assert!(con(&|v| v["servidores"][0]["discos"][0]["serie"] = json!("s".repeat(65))).is_err());
        assert!(con(&|v| v["servidores"][0]["nombre"] = json!("a\u{7}b")).is_err());
        assert!(con(&|v| v["equipos"]["e-vm"]["servidor"] = json!("hv-09")).is_err(), "servidor que no existe");
        assert!(con(&|v| v["destinos"]["zona:e-almacen:principal"]["conjunto"] = json!("otro")).is_err(), "conjunto de otro servidor");
        assert!(con(&|v| v["destinos"]["zona:e-almacen:principal"]["disco"] = json!("b1")).is_err(), "conjunto y disco a la vez");
        assert!(con(&|v| v["destinos"]["zona:e-almacen:principal"]["volumenes"] = json!([{ "punto": "C:", "disco": "b1" }])).is_err());
        assert!(con(&|v| v["equipos"]["e-vm"]["volumenes"][0]["disco"] = json!("b9")).is_err(), "disco que no es de su servidor");
        assert!(con(&|v| v["destinos"] = json!({ "../x": { "servidor": "hv-01" } })).is_err());
        assert!(con(&|v| v["servidores"][0]["modelo"] = json!("m".repeat(121))).is_err());
        // Un hipervisor vacío es «sin hipervisor».
        assert_eq!(con(&|v| v["servidores"][0]["hipervisor"] = json!("")).unwrap().servidores[0].hipervisor, None);
        // Un disco dicho: vale.
        assert_eq!(con(&|v| v["destinos"]["zona:e-almacen:principal"] = json!({ "servidor": "hv-01", "disco": "m2" })).unwrap().destinos.len(), 1);
    }

    /// 0.8.2 (A5): lo que cabe en los datos comunes del cliente, medido.
    #[test]
    fn se_mide_lo_que_ocupa_en_los_equipos() {
        let d = leer_json(ejemplo()).unwrap();
        let n = bytes_comunes(&d);
        assert!(n > 500 && n < 4 * 1024, "{n}");
        // Un cliente grande y realista: 6 servidores con 12 discos y 4 conjuntos, 80 equipos y 40 destinos.
        let mut g = Infraestructura::default();
        for s in 0..6 {
            let discos: Vec<Disco> = (0..12)
                .map(|i| Disco {
                    id: format!("b{i}"),
                    nombre: format!("Bahía {i}"),
                    tipo: Some("hdd".into()),
                    modelo: Some("Disco de 8 TB de una marca".into()),
                    serie: Some(format!("SN-{s}-{i:08}")),
                    tamano: Some(8_000_000_000_000),
                    bahia: Some(format!("{i}")),
                    estado: Some("bien".into()),
                    ..Default::default()
                })
                .collect();
            let conjuntos: Vec<Conjunto> = (0..4)
                .map(|c| Conjunto {
                    id: format!("c{c}"),
                    nombre: format!("Disco virtual {c}"),
                    tipo: "raid10".into(),
                    miembros: (0..3).map(|i| format!("b{}", c * 3 + i)).collect(),
                    capacidad: Some(12_000_000_000_000),
                    estado: Some("bien".into()),
                    ..Default::default()
                })
                .collect();
            g.servidores.push(Servidor {
                id: format!("srv-{s}"),
                nombre: format!("SERVIDOR-0{s}"),
                hipervisor: Some("hyperv".into()),
                discos,
                conjuntos,
                ..Default::default()
            });
        }
        for e in 0..80 {
            g.equipos.insert(
                format!("3f2a9c1e-0b5c-4d1e-9a7f-{e:012}"),
                Sitio { servidor: format!("srv-{}", e % 6), conjunto: Some("c1".into()), ..Default::default() },
            );
        }
        for k in 0..40 {
            g.destinos.insert(
                format!("zona:3f2a9c1e-0b5c-4d1e-9a7f-{k:012}:principal"),
                Sitio { servidor: "srv-0".into(), conjunto: Some("c2".into()), ..Default::default() },
            );
        }
        let n = bytes_comunes(&g);
        println!("Un cliente grande ocupa {n} bytes de infraestructura en cada equipo.");
        assert!(n < dc::MAX_BYTES_INFRA, "un cliente grande cabe: {n} bytes");
        assert!(limpiar(g.clone(), None).is_ok());
        // Muchos servidores con notas largas en cada disco: cada uno cabe, todos juntos no (lo dice al guardar).
        for s in 6..16 {
            let discos: Vec<Disco> =
                (0..20).map(|i| Disco { id: format!("b{i}"), nombre: format!("Bahía {i}"), notas: Some("n".repeat(500)), ..Default::default() }).collect();
            g.servidores.push(Servidor { id: format!("srv-{s}"), nombre: format!("SERVIDOR-{s}"), discos, ..Default::default() });
        }
        assert!(limpiar(g, None).unwrap_err().contains("todas las consolas"));
    }

    #[test]
    fn se_guarda_por_cliente_y_se_reparte() {
        use crate::almacen::sqlite::Sqlite;
        let dir = tempfile::tempdir().unwrap();
        let db = Sqlite::abrir(dir.path()).unwrap();
        assert_eq!(leer(&db, "c1").unwrap(), Infraestructura::default());
        let d = leer_json(ejemplo()).unwrap();
        let g = Guardada { infra: d.clone(), actualizada: Some(ahora()), por: Some("Ana".into()) };
        guardar(&db, "c1", &g).unwrap();
        assert_eq!(leer(&db, "c1").unwrap(), d);
        assert_eq!(leer(&db, "c2").unwrap(), Infraestructura::default(), "cada cliente lo suyo");
        let j = json_de(&g, false);
        assert_eq!(j["servidores"][0]["conjuntos"][0]["tipo"], "zfs_espejo");
        assert_eq!(j["por"], "Ana");
        // Como datos comunes: un dato por servidor, por equipo y por destino, y todos valen.
        let mut ks = claves(&db, "c1").unwrap();
        ks.sort();
        assert_eq!(ks, vec!["infra:destino:zona:e-almacen:principal", "infra:equipo:e-vm", "infra:servidor:hv-01"]);
        for k in &ks {
            let v = valor_local(&db, "c1", k).unwrap();
            assert_eq!(dc::valor_valido(k, &v).unwrap(), v, "{k}");
        }
        assert!(valor_local(&db, "c1", "infra:servidor:otro").unwrap().is_null());
        // Lo que llega de otra consola: un sitio antes que su servidor, el servidor, y quitarlo.
        aplicar(&db, "c2", "infra:equipo:e-1", &json!({ "servidor": "hv-02" }), "Bruno (desde la consola «Oficina»)").unwrap();
        assert_eq!(leer(&db, "c2").unwrap().equipos["e-1"].servidor, "hv-02");
        let s = valor_local(&db, "c1", "infra:servidor:hv-01").unwrap();
        aplicar(&db, "c2", "infra:servidor:hv-02", &s, "Bruno").unwrap();
        let c2 = leer(&db, "c2").unwrap();
        assert_eq!((c2.servidores[0].id.as_str(), c2.servidores[0].discos.len()), ("hv-02", 3));
        // Un PUT de aquí que no lleva ese sitio de otra consola (su servidor aún no estaba) no lo pierde.
        let mut sin = c2.clone();
        sin.servidores.clear();
        let previa = sin.clone();
        assert!(limpiar(sin, Some(&previa)).is_ok(), "un sitio ya guardado, aunque su servidor no esté, se deja");
        aplicar(&db, "c2", "infra:servidor:hv-02", &Value::Null, "Bruno").unwrap();
        assert!(leer(&db, "c2").unwrap().servidores.is_empty());
        aplicar(&db, "c2", "infra:equipo:e-1", &Value::Null, "Bruno").unwrap();
        assert!(leer(&db, "c2").unwrap().equipos.is_empty());
    }
}
