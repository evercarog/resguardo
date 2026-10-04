//! Agente v2, F6: moverse libremente (docs/plataforma.md, §3.3 y §3.5;
//! docs/api-servidor.md, §10 y §11).
//!
//! - **Cambiar de servidor** (`cambiar_servidor`, clave de administración):
//!   el equipo se da de alta en el servidor nuevo con la ficha de «Recibir un
//!   cliente», comprueba su identidad (la de la orden), sube su configuración
//!   y solo entonces confirma al antiguo y cambia. Si el nuevo no responde en
//!   24 h, sigue con el antiguo y la orden queda `fallida`.
//! - **Servidores de respaldo** (`servidores_respaldo`, hasta 3): si el
//!   principal no responde en N días, el equipo se muda al primero que le acepte.
//! - **Restaurar en otro equipo**: `compartir_acceso` (el de origen sella y
//!   firma su acceso para el de destino) e `importar_repositorio` (el de
//!   destino lo abre, o usa los datos del kit), como repositorio solo de lectura.

use crate::gestion_v2::{self as g, Destino, RepoV2};
use crate::servidor_v2::{self as s, Vinculo};
use base64::Engine;
use ed25519_dalek::{Signer, Verifier};
use resguardo_protocolo::{claves, derivaciones};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
/// Tiempo que se intenta un cambio de servidor antes de quedarse en el antiguo.
pub const PLAZO_CAMBIO_S: i64 = 24 * 3600;

/// Un servidor al que el equipo puede mudarse (con su ficha de alta).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Servidor {
    pub url: String,
    /// Identidad Ed25519 (base64) que tiene que demostrar.
    pub identidad: String,
    /// Su autoridad TLS (PEM); vacía solo para `http://` local.
    #[serde(default)]
    pub ca_pem: String,
    pub ficha: String,
}

/// Cambio de servidor en curso.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Cambio {
    pub destino: Servidor,
    /// La orden en el servidor antiguo (para su resultado final).
    pub orden: String,
    pub seq: u64,
    pub hasta: i64,
}

fn servidor_de(c: &Value) -> Result<Servidor, String> {
    let sv = Servidor {
        url: c["url"].as_str().unwrap_or("").trim().trim_end_matches('/').to_string(),
        identidad: c["identidad"].as_str().unwrap_or("").to_string(),
        ca_pem: c["ca_pem"].as_str().unwrap_or("").to_string(),
        ficha: c["ficha"].as_str().unwrap_or("").trim().to_string(),
    };
    let local = sv.url.starts_with("http://127.0.0.1") || sv.url.starts_with("http://localhost") || sv.url.starts_with("http://[::1]");
    if !sv.url.starts_with("https://") && !local {
        return Err("La dirección del servidor tiene que ser https://.".into());
    }
    if sv.url.starts_with("https://") && !sv.ca_pem.contains("BEGIN CERTIFICATE") {
        return Err("Falta la autoridad TLS del servidor (ca_pem).".into());
    }
    if B64.decode(&sv.identidad).map(|b| b.len()) != Ok(32) {
        return Err("Identidad del servidor no válida.".into());
    }
    if sv.ficha.len() < 16 || sv.ficha.len() > 128 {
        return Err("Ficha de alta no válida.".into());
    }
    Ok(sv)
}

/// `cambiar_servidor {url, identidad, ca_pem, ficha}`: empieza el cambio (fase 1).
pub fn cambiar_servidor(v: &mut Vinculo, c: &Value, orden: &str, seq: u64) -> Result<String, String> {
    if v.k_cfg.is_none() {
        return Err("Este equipo aún no tiene la clave de administración (falta el alta).".into());
    }
    let destino = servidor_de(c)?;
    // v1.36: a un servidor que ya es otra de sus consolas, no (sería la misma dos veces).
    if v.otras.iter().any(|e| e.identidad == destino.identidad) {
        return Err("Ese servidor ya gestiona este equipo (es otra de sus consolas): quita esta consola en vez de cambiarla.".into());
    }
    v.cambio = Some(Cambio { destino, orden: orden.to_string(), seq, hasta: chrono::Utc::now().timestamp() + PLAZO_CAMBIO_S });
    let _ = g::subir_config(v);
    Ok("Cambiando de servidor: el equipo se da de alta en el nuevo.".into())
}

/// Se da de alta en otro servidor y devuelve el vínculo nuevo (sin guardarlo),
/// con su configuración y su informe ya subidos.
pub fn trasladar(v: &Vinculo, destino: &Servidor) -> Result<Vinculo, String> {
    let mut n = recibir_en(v, destino, None)?;
    let _ = g::subir_config_enlace(&mut n);
    let _ = s::llamar_ok(&n, "/api/agente/informe", &json!({ "datos": g::informe(Some(&n)) }));
    Ok(n)
}

/// Se da de alta en otro servidor con su ficha (`POST /api/agente/recibir`, con
/// la etiqueta calculada con `v.k_cfg`) y comprueba que tiene la identidad
/// esperada y la demuestra. Devuelve el vínculo con ese servidor en los campos
/// de siempre (sin guardarlo ni subir nada). `motivo` (v1.36): `anadir_consola`.
pub fn recibir_en(v: &Vinculo, destino: &Servidor, motivo: Option<&str>) -> Result<Vinculo, String> {
    let k: [u8; 32] = v.k_cfg.as_deref().and_then(|k| B64.decode(k).ok()).and_then(|k| k.try_into().ok()).ok_or("Falta K_cfg.")?;
    let box_pub = claves::public_of(&v.box_secret)?;
    let sign_pub = B64.encode(firma(v)?.verifying_key().to_bytes());
    let mut n = v.clone();
    n.url = destino.url.clone();
    n.ca_pem = destino.ca_pem.clone();
    n.identidad = destino.identidad.clone();
    let mut cuerpo = json!({
        "ficha": destino.ficha, "equipo_id": v.equipo_id, "nombre": crate::web::default_device_name(), "so": crate::web::os_label(),
        "version": crate::version_programa(), "box_pub": box_pub, "sign_pub": sign_pub, "sal_equipo": v.sal_equipo,
        "etiqueta": derivaciones::etiqueta_equipo(&k, &v.equipo_id, &box_pub, &sign_pub), "espera_min_horas": v.espera_min_horas,
    });
    if let Some(m) = motivo {
        cuerpo["motivo"] = json!(m);
    }
    let agente = s::agente_de(&n)?;
    let mut r =
        agente.post(&format!("{}/api/agente/recibir", n.url)).send_json(&cuerpo).map_err(|e| format!("No se pudo conectar con el servidor nuevo: {e}"))?;
    let estado = r.status().as_u16();
    let resp: Value = r.body_mut().read_json().unwrap_or(Value::Null);
    if estado >= 400 {
        return Err(format!("El servidor nuevo no aceptó el equipo: {}", resp["mensaje"].as_str().unwrap_or("error")));
    }
    if resp["servidor"]["identidad"].as_str() != Some(destino.identidad.as_str()) {
        return Err("El servidor nuevo no tiene la identidad que dice la orden.".into());
    }
    n.cliente_id = resp["cliente_id"].as_str().ok_or("Respuesta sin cliente.")?.to_string();
    n.equipo_id = resp["equipo_id"].as_str().ok_or("Respuesta sin equipo.")?.to_string();
    n.secreto = resp["secreto"].as_str().ok_or("Respuesta sin secreto.")?.to_string();
    n.ultimo_seq = 0;
    n.nonces.clear();
    n.modo = "gestionado".into();
    n.config_seq = 0;
    n.cambio = None;
    n.ultimo_ok = chrono::Utc::now().timestamp();
    // v1.36: lo de la consola anterior no vale en esta.
    n.nombre_consola.clear();
    n.sal_cliente = None;
    n.fallos.clear();
    n.desde = n.ultimo_ok;
    n.config_pendiente = false;
    // Que demuestre su identidad (firma sobre un reto nuestro), como en cada conexión.
    let reto = B64.encode(rand_32());
    let r = s::llamar_ok(&n, "/api/agente/tomar", &json!({ "reto": reto }))?;
    s::comprueba_identidad(&n, &reto, r["firma"].as_str().unwrap_or(""))?;
    Ok(n)
}

fn rand_32() -> [u8; 32] {
    use crypto_box::aead::rand_core::RngCore;
    let mut b = [0u8; 32];
    crypto_box::aead::OsRng.fill_bytes(&mut b);
    b
}

fn firma(v: &Vinculo) -> Result<ed25519_dalek::SigningKey, String> {
    let sd: [u8; 32] = B64.decode(&v.sign_seed).ok().and_then(|b| b.try_into().ok()).ok_or("Clave del equipo dañada.")?;
    Ok(ed25519_dalek::SigningKey::from_bytes(&sd))
}

/// Fase 2 del cambio (en cada vuelta): si el nuevo acepta, confirma al antiguo
/// y se queda con el nuevo; pasado el plazo, se queda en el antiguo.
pub fn intentar_cambio(v: &mut Vinculo) {
    let Some(cambio) = v.cambio.clone() else { return };
    if chrono::Utc::now().timestamp() > cambio.hasta {
        crate::agent::log("Cambio de servidor: el nuevo no respondió a tiempo; el equipo sigue en este.");
        let _ = s::enviar_resultado_texto(
            v,
            &cambio.orden,
            cambio.seq,
            "fallida",
            "El servidor nuevo no respondió en 24 h: el equipo sigue en este servidor.",
            None,
        );
        let _ = s::llamar_ok(
            v,
            "/api/agente/aviso",
            &json!({ "tipo": "cambio_inusual", "mensaje": "El cambio de servidor no se completó: el nuevo no respondió en 24 h." }),
        );
        v.cambio = None;
        let _ = s::guardar(v);
        return;
    }
    match trasladar(v, &cambio.destino) {
        Ok(mut nuevo) => {
            // Primero se confirma al antiguo (con las credenciales viejas); después se cambia.
            let _ = s::enviar_resultado_texto(v, &cambio.orden, cambio.seq, "hecha", "El equipo ya está en el servidor nuevo.", None);
            nuevo.respaldo.retain(|r| r.url != nuevo.url);
            crate::agent::log(&format!("Cambio de servidor hecho: ahora en {}.", nuevo.url));
            *v = nuevo;
            let _ = s::guardar(v);
        }
        Err(e) => crate::agent::log(&format!("Cambio de servidor: {e} (se reintenta).")),
    }
}

/// `servidores_respaldo {servidores: [{url, identidad, ca_pem, ficha}], dias}` (hasta 3).
pub fn servidores_respaldo(v: &mut Vinculo, c: &Value) -> Result<String, String> {
    let lista = c["servidores"].as_array().ok_or("Falta la lista de servidores.")?;
    if lista.len() > 3 {
        return Err("Como mucho 3 servidores de respaldo.".into());
    }
    let dias = c["dias"].as_i64().unwrap_or(3);
    if !(1..=30).contains(&dias) {
        return Err("Los días sin respuesta deben estar entre 1 y 30.".into());
    }
    v.respaldo = lista.iter().map(servidor_de).collect::<Result<_, _>>()?;
    v.respaldo_dias = dias;
    let _ = g::subir_config(v);
    Ok(format!("{} servidores de respaldo (si este no responde en {dias} días).", v.respaldo.len()))
}

/// Tras una vuelta sin respuesta de la consola activa (`v`, v1.36: cada una
/// tiene sus respaldos): si pasaron los días fijados, se muda al primer
/// respaldo que le acepte. No guarda (lo hace quien la llama).
pub fn comprobar_respaldo(v: &mut Vinculo) {
    if v.modo != "gestionado" || v.respaldo.is_empty() {
        return;
    }
    let ahora = chrono::Utc::now().timestamp();
    if v.ultimo_ok == 0 {
        v.ultimo_ok = ahora;
        return;
    }
    if ahora - v.ultimo_ok < v.respaldo_dias.max(1) * 86_400 {
        return;
    }
    for r in v.respaldo.clone() {
        // Un respaldo que ya es otra de sus consolas: el equipo ya está allí.
        if v.otras.iter().any(|e| e.identidad == r.identidad) {
            continue;
        }
        match trasladar(v, &r) {
            Ok(mut nuevo) => {
                nuevo.respaldo.retain(|x| x.url != r.url);
                crate::agent::log(&format!("El servidor {} no respondía: el equipo pasa al de respaldo {}.", v.url, r.url));
                *v = nuevo;
                return;
            }
            Err(e) => crate::agent::log(&format!("Respaldo {}: {e}", r.url)),
        }
    }
}

// ---------- Volver a vincular con un código (camino 3) ----------

/// Plazo para que el servidor nuevo dé el alta tras `vincular`.
pub const PLAZO_ADOPCION_S: i64 = 7 * 86_400;

/// Vínculo con un servidor nuevo de un equipo que ya tiene clave de
/// administración (`vincular` con un código, por ejemplo tras perder la
/// consola). Hasta que ese servidor manda el `alta` con la MISMA clave, el
/// equipo sigue con su configuración y su servidor de antes; el nuevo no
/// recibe la configuración ni el resumen y no puede mandar otras órdenes.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Adopcion {
    pub url: String,
    pub ca_pem: String,
    pub identidad: String,
    pub cliente_id: String,
    pub equipo_id: String,
    pub secreto: String,
    /// El código de emparejamiento (el alta tiene que demostrar que lo vio).
    pub codigo: String,
    pub hasta: i64,
    #[serde(default)]
    pub ultimo_seq: u64,
    #[serde(default)]
    pub nonces: Vec<(String, i64)>,
}

/// El equipo, tal como quedaría en el servidor nuevo.
fn con_adopcion(v: &Vinculo, a: &Adopcion) -> Vinculo {
    let mut n = v.clone();
    n.url = a.url.clone();
    n.ca_pem = a.ca_pem.clone();
    n.identidad = a.identidad.clone();
    n.cliente_id = a.cliente_id.clone();
    n.equipo_id = a.equipo_id.clone();
    n.secreto = a.secreto.clone();
    n.codigo = Some(a.codigo.clone());
    n.modo = "gestionado".into();
    n.cambio = None;
    n.adopcion = None;
    // v1.36: sustituye a la consola principal; lo de esa no vale en esta.
    n.nombre_consola.clear();
    n.sal_cliente = None;
    n.fallos.clear();
    n.config_pendiente = false;
    n.desde = chrono::Utc::now().timestamp();
    n.otras.retain(|e| e.identidad != a.identidad);
    n
}

fn rechazo(m: &str) -> s::Resultado {
    s::Resultado { estado: "rechazada", mensaje: m.into(), detalle: None }
}

/// Abre una orden del servidor nuevo: solo vale el `alta`, con la clave de
/// administración que ya tiene el equipo y la prueba del código. Si se hace,
/// el equipo toma la `K_cfg` del alta (con la clave demostrada: lo mismo que
/// `cambiar_clave_admin`), por si el cliente se creó de nuevo con otra sal.
fn orden_pendiente(n: &mut Vinculo, a: &mut Adopcion, meta: &Value) -> s::Resultado {
    use resguardo_protocolo::{orden_v2, ordenes};
    let ahora = chrono::Utc::now().timestamp();
    let cx = orden_v2::Contexto {
        cliente: &n.cliente_id,
        equipo: &n.equipo_id,
        ultimo_seq: a.ultimo_seq,
        ahora,
        tipo_meta: meta["tipo"].as_str().unwrap_or(""),
        seq_meta: meta["seq"].as_u64().unwrap_or(0),
    };
    let o = match orden_v2::abrir(meta["sellado"].as_str().unwrap_or(""), &n.box_secret, &cx) {
        Ok(o) => o,
        Err(e) => return s::Resultado { estado: "rechazada", mensaje: e, detalle: None },
    };
    a.nonces.retain(|(_, exp)| *exp > ahora);
    if a.nonces.iter().any(|(x, _)| *x == o.nonce) {
        return rechazo("Orden repetida.");
    }
    a.ultimo_seq = o.seq;
    a.nonces.push((o.nonce.clone(), ahora + ordenes::MAX_CADUCIDAD_DIAS * 86_400 + 600));
    if o.tipo != "alta" {
        return rechazo(
            "Este equipo ya tiene clave de administración: este servidor tiene que darlo de alta con esa misma clave antes de mandarle otras órdenes.",
        );
    }
    match s::alta(n, &o) {
        Ok(r) if r.estado == "hecha" => {
            n.k_cfg = o.cuerpo["k_cfg"].as_str().map(str::to_string);
            s::Resultado { mensaje: "Equipo recibido con la misma clave de administración: conserva su configuración.".into(), ..r }
        }
        Ok(r) => r,
        Err(e) => s::Resultado { estado: "fallida", mensaje: e, detalle: None },
    }
}

/// Una vuelta con el servidor nuevo: si da el alta, devuelve el equipo ya en él.
fn adoptar(v: &Vinculo, a: &mut Adopcion) -> Result<Option<Vinculo>, String> {
    let mut n = con_adopcion(v, a);
    let reto = B64.encode(rand_32());
    let r = s::llamar_ok(&n, "/api/agente/tomar", &json!({ "reto": reto }))?;
    s::comprueba_identidad(&n, &reto, r["firma"].as_str().unwrap_or(""))?;
    for meta in r["ordenes"].as_array().cloned().unwrap_or_default() {
        let res = orden_pendiente(&mut n, a, &meta);
        crate::agent::log(&format!("Orden «{}» del servidor nuevo ({}): {} ({}).", meta["tipo"].as_str().unwrap_or("?"), a.url, res.estado, res.mensaje));
        let _ = s::enviar_resultado(&n, meta["id"].as_str().unwrap_or(""), meta["seq"].as_u64().unwrap_or(0), &res);
        if res.estado == "hecha" {
            n.codigo = None;
            n.ultimo_seq = a.ultimo_seq;
            n.nonces = std::mem::take(&mut a.nonces);
            n.ultimo_ok = chrono::Utc::now().timestamp();
            return Ok(Some(n));
        }
    }
    Ok(None)
}

/// En cada vuelta, si hay un servidor nuevo pendiente de dar el alta.
pub fn ronda_adopcion() {
    // Leído bajo el cerrojo (con la principal en su sitio); la red, fuera; el cambio, dentro otra vez.
    let Some(v) = crate::consolas_v2::modificar(|v| v.clone()) else { return };
    let Some(mut a) = v.adopcion.clone() else { return };
    let misma = |x: &Vinculo, a: &Adopcion| x.adopcion.as_ref().is_some_and(|p| p.url == a.url && p.equipo_id == a.equipo_id);
    if chrono::Utc::now().timestamp() > a.hasta {
        crate::agent::log(&format!("{} no dio el alta en 7 días: se olvida ese vínculo pendiente. El equipo sigue como estaba.", a.url));
        crate::consolas_v2::modificar(|x| {
            if misma(x, &a) {
                x.adopcion = None;
            }
        });
        return;
    }
    match adoptar(&v, &mut a) {
        Ok(Some(n)) => {
            crate::agent::log(&format!("El equipo pasa a {} con su misma configuración (alta con la clave de administración).", n.url));
            // La principal pasa a ser la nueva; lo del equipo, como esté ahora en el disco.
            let n = crate::consolas_v2::modificar(|x| {
                if !misma(x, &a) {
                    return None;
                }
                let mut e = n.enlace();
                e.id = x.id_enlace();
                x.adopcion = None;
                x.modo = "gestionado".into();
                // Los `nonce` son de todas las consolas: se suman los del servidor nuevo.
                for nonce in &n.nonces {
                    if !x.nonces.iter().any(|(y, _)| *y == nonce.0) {
                        x.nonces.push(nonce.clone());
                    }
                }
                x.verificador = n.verificador.clone();
                x.espera_min_horas = n.espera_min_horas;
                x.otras.retain(|o| o.identidad != e.identidad);
                x.poner_enlace(e);
                Some(x.clone())
            })
            .flatten();
            if let Some(n) = n {
                // La configuración, cifrada con la K_cfg del alta, y el informe: la consola nueva ya lo ve todo.
                crate::consolas_v2::con_enlace(&n.id_enlace(), |x| {
                    let _ = g::subir_config_enlace(x);
                });
                let _ = s::llamar_ok(&n, "/api/agente/informe", &json!({ "datos": g::informe(Some(&n)) }));
            }
        }
        Ok(None) => {
            crate::consolas_v2::modificar(|x| {
                if misma(x, &a) {
                    x.adopcion = Some(a);
                }
            });
        }
        // Sin respuesta del nuevo: se reintenta en la siguiente vuelta (sin llenar el registro).
        Err(_) => {}
    }
}

// ---------- Restaurar en otro equipo ----------

fn texto_firmado(para_equipo: &str, datos: &str) -> String {
    format!("resguardo-acceso-v1|{para_equipo}|{datos}")
}

/// `compartir_acceso {repo, para: {equipo, box_pub}, incluir_contrasena}`
/// (contraseña del repo + clave de administración): el acceso al destino del
/// repositorio (y su contraseña si se pide), firmado por este equipo y sellado
/// para el de destino. Devuelve el detalle (en claro: solo lo abre el destino).
pub fn compartir_acceso(v: &Vinculo, c: &Value, repo: &str) -> Result<String, String> {
    let r = v.repos_v2.iter().find(|x| x.id == repo).ok_or("Ese repositorio no lo gestiona este servidor.")?;
    let d = v.destinos.iter().find(|x| x.id == r.destino).ok_or("Falta el destino de ese repositorio.")?;
    let (para, box_pub) = (c["para"]["equipo"].as_str().unwrap_or(""), c["para"]["box_pub"].as_str().unwrap_or(""));
    if uuid::Uuid::parse_str(para).is_err() || B64.decode(box_pub).map(|b| b.len()) != Ok(32) {
        return Err("Falta el equipo de destino o su clave.".into());
    }
    // v1.14: `ubicacion`, su ruta en el destino si no es su id (adoptados e importados).
    let mut datos = json!({ "repo": { "id": r.id, "nombre": r.nombre, "ubicacion": r.ubicacion_origen }, "destino": d });
    if c["incluir_contrasena"].as_bool().unwrap_or(false) {
        datos["contrasena"] = json!(r.contrasena);
    }
    let datos = datos.to_string();
    let f = firma(v)?.sign(texto_firmado(para, &datos).as_bytes());
    let sobre = json!({ "datos": datos, "firma": B64.encode(f.to_bytes()), "de": v.equipo_id }).to_string();
    let sellado = claves::seal_bytes(box_pub, sobre.as_bytes())?;
    Ok(json!({ "acceso_sellado": sellado }).to_string())
}

/// `importar_repositorio {id, nombre, acceso_sellado, sign_pub_origen, contrasena?}` o, sin
/// el equipo de origen (kit), `{id, nombre, acceso: {destino: {tipo, donde, usuario?, secreto?, ca_pem?}, contrasena}}`.
/// Lo añade como **solo de lectura** (para explorar y restaurar; sin copias).
pub fn importar_repositorio(v: &mut Vinculo, c: &Value) -> Result<String, String> {
    let id = c["id"].as_str().unwrap_or("").to_string();
    let nombre = c["nombre"].as_str().unwrap_or("").to_string();
    if id.is_empty() || id.len() > 64 || !id.chars().all(|x| x.is_ascii_alphanumeric() || x == '-' || x == '_') || v.repos_v2.iter().any(|r| r.id == id) {
        return Err("Id de repositorio no válido o ya en uso.".into());
    }
    let (destino, contrasena): (Value, Option<String>) = if let Some(sellado) = c["acceso_sellado"].as_str() {
        let abierto: Value = serde_json::from_slice(&claves::open_bytes(&v.box_secret, sellado)?).map_err(|_| "Acceso sellado no válido.")?;
        let datos = abierto["datos"].as_str().ok_or("Acceso sellado no válido.")?;
        // La firma del equipo de origen (su sign_pub viene en la orden, que firma el administrador).
        let clave: [u8; 32] =
            B64.decode(c["sign_pub_origen"].as_str().unwrap_or("")).ok().and_then(|b| b.try_into().ok()).ok_or("Falta la clave del equipo de origen.")?;
        let f: [u8; 64] = B64.decode(abierto["firma"].as_str().unwrap_or("")).ok().and_then(|b| b.try_into().ok()).ok_or("Firma no válida.")?;
        ed25519_dalek::VerifyingKey::from_bytes(&clave)
            .map_err(|_| "Clave del equipo de origen no válida.")?
            .verify(texto_firmado(&v.equipo_id, datos).as_bytes(), &ed25519_dalek::Signature::from_bytes(&f))
            .map_err(|_| "El acceso no lo firmó el equipo de origen: no se importa.")?;
        let d: Value = serde_json::from_str(datos).map_err(|e| e.to_string())?;
        let mut destino = d["destino"].clone();
        destino["repo"] = if d["repo"]["ubicacion"].is_string() { d["repo"]["ubicacion"].clone() } else { d["repo"]["id"].clone() };
        (destino, d["contrasena"].as_str().map(str::to_string).or_else(|| c["contrasena"].as_str().map(str::to_string)))
    } else {
        // Con el kit: el destino y el id del repositorio en él.
        let mut destino = c["acceso"]["destino"].clone();
        destino["repo"] = c["acceso"]["repo"].clone();
        (destino, c["acceso"]["contrasena"].as_str().map(str::to_string))
    };
    let contrasena = contrasena.filter(|s| !s.is_empty()).ok_or("Falta la contraseña del repositorio.")?;
    let mut d: Destino = serde_json::from_value(json!({
        "id": format!("importado-{id}"), "nombre": destino["nombre"].as_str().unwrap_or("Importado"), "tipo": destino["tipo"], "donde": destino["donde"],
        "usuario": destino["usuario"], "secreto": destino["secreto"], "ca_pem": destino["ca_pem"],
    }))
    .map_err(|e| format!("Destino no válido: {e}"))?;
    d.nombre = d.nombre.chars().take(80).collect();
    // La ubicación del repositorio de origen: su id en el destino.
    let repo_origen = destino.get("repo").and_then(Value::as_str).map(str::to_string);
    let mut prueba = v.clone();
    prueba.destinos.retain(|x| x.id != d.id);
    prueba.destinos.push(d);
    prueba.repos_v2.push(RepoV2 {
        id: id.clone(),
        nombre: if nombre.is_empty() { id.clone() } else { nombre.clone() },
        destino: format!("importado-{id}"),
        contrasena,
        retencion: None,
        solo_lectura: true,
        ubicacion_origen: repo_origen,
        externa: None,
        solo_anadir: None,
    });
    let acc = g::acceso(&prueba, &id)?;
    let n = resguardo_motor::restic::snapshots(&acc).map_err(|e| format!("No se pudo abrir el repositorio: {e}"))?.len();
    *v = prueba;
    let _ = g::subir_config(v);
    Ok(format!("Repositorio importado (solo lectura, {n} versiones)."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn equipo(id: &str) -> Vinculo {
        let box_secret = claves::new_key();
        Vinculo { equipo_id: id.into(), box_secret, sign_seed: B64.encode(rand_32()), modo: "local".into(), espera_min_horas: 24, ..Default::default() }
    }

    /// Restaurar en otro equipo: A sella y firma su acceso para B; B lo importa como solo lectura.
    #[test]
    fn compartir_e_importar_un_repositorio() {
        if resguardo_motor::restic::version().is_err() {
            return; // sin restic (CI de Linux)
        }
        let base = std::env::temp_dir().join(format!("resguardo-compartir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (ida, idb) = (uuid::Uuid::new_v4().to_string(), uuid::Uuid::new_v4().to_string());
        let mut a = equipo(&ida);
        let mut b = equipo(&idb);
        g::crear_repositorio(
            &mut a,
            &json!({ "id": "r1", "nombre": "Principal", "contrasena": "contraseña del repo",
                     "destino": { "id": "d1", "nombre": "Disco", "tipo": "local", "donde": base.join("destino").display().to_string() } }),
        )
        .unwrap();
        let box_pub_b = claves::public_of(&b.box_secret).unwrap();
        let sign_pub_a = B64.encode(firma(&a).unwrap().verifying_key().to_bytes());
        let pedir = json!({ "repo": "r1", "para": { "equipo": idb, "box_pub": box_pub_b }, "incluir_contrasena": true });
        let detalle: Value = serde_json::from_str(&compartir_acceso(&a, &pedir, "r1").unwrap()).unwrap();
        let sellado = detalle["acceso_sellado"].as_str().unwrap().to_string();
        // Con la clave de otro equipo como «origen»: la firma no cuadra.
        let otra = B64.encode(firma(&b).unwrap().verifying_key().to_bytes());
        assert!(importar_repositorio(&mut b, &json!({ "id": "de-a", "acceso_sellado": sellado, "sign_pub_origen": otra })).unwrap_err().contains("firmó"));
        let m =
            importar_repositorio(&mut b, &json!({ "id": "de-a", "nombre": "Copias de A", "acceso_sellado": sellado, "sign_pub_origen": sign_pub_a })).unwrap();
        assert!(m.contains("solo lectura"), "{m}");
        assert!(b.repos_v2[0].solo_lectura);
        // Solo lectura: ninguna copia puede usarlo.
        let cfg = json!({ "config": { "v": 1, "copias": [{ "id": "k", "nombre": "K", "repo": "de-a", "carpetas": ["/x"], "horario": { "dias": [1], "horas": ["10:00"] } }] } });
        assert!(g::aplicar_config(&mut b, &cfg).unwrap_err().contains("solo de lectura"));
        // Sellado para B: un tercero no lo abre.
        let mut c = equipo(&uuid::Uuid::new_v4().to_string());
        assert!(importar_repositorio(&mut c, &json!({ "id": "x", "acceso_sellado": sellado, "sign_pub_origen": sign_pub_a })).is_err());
        // Con el kit (sin el equipo de origen).
        let m = importar_repositorio(
            &mut c,
            &json!({ "id": "kit", "acceso": { "destino": { "tipo": "local", "donde": base.join("destino").display().to_string() }, "repo": "r1", "contrasena": "contraseña del repo" } }),
        )
        .unwrap();
        assert!(m.contains("solo lectura"));
        let _ = std::fs::remove_dir_all(&base);
    }
}
