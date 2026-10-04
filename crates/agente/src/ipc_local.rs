//! `ipc_local`: lo que la ventana del equipo (como el usuario) pide al servicio
//! (como SYSTEM o root). El único camino privilegiado nuevo de la ventana
//! (docs/agente-ventana.md §4).
//!
//! - **Transporte.** Windows: una tubería con nombre que crea el servicio con
//!   una DACL explícita (SYSTEM y administradores; los usuarios con sesión
//!   interactiva pueden leer y escribir, pero **no** crear instancias: nadie
//!   puede suplantar la tubería) y que rechaza clientes remotos; el cliente
//!   comprueba además que la tubería es de SYSTEM o de los administradores y
//!   no deja que el servidor lo suplante. Linux: un socket Unix de root.
//!   Una petición por conexión: una línea JSON, como mucho [`MAX_MENSAJE`].
//! - **Prueba de administración**, la misma que la consola: `prueba =
//!   Argon2id(clave, sal_equipo)` contra `verificador = SHA-256(prueba)` en
//!   tiempo constante. La calcula la ventana: el servicio nunca ve la clave.
//!   Atada a un **reto de un solo uso** que da `hola` (caduca en
//!   [`RETO_VIDA_S`]; se gasta aunque falle: no se puede repetir).
//! - **Límites**: 5 fallos seguidos bloquean 1 minuto, el doble cada vez
//!   (hasta 1 hora); tras un fallo, como mucho un intento por segundo. Nada
//!   de la petición va al registro.

use base64::Engine;
use serde_json::{json, Value};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// Tamaño máximo de una petición o de una respuesta.
pub const MAX_MENSAJE: usize = 64 * 1024;
/// Respuestas grandes (listar una carpeta de un repositorio).
pub const MAX_RESPUESTA: usize = 4 * 1024 * 1024;
/// Lo que vive un reto sin usar.
pub const RETO_VIDA_S: i64 = 60;
/// Retos sin usar a la vez (los más viejos se olvidan).
const RETOS_MAX: usize = 16;
/// Fallos seguidos antes de bloquear.
pub const FALLOS_MAX: u32 = 5;
/// Primer bloqueo (y se duplica cada vez, hasta [`BLOQUEO_MAX_MS`]).
pub const BLOQUEO_MS: i64 = 60_000;
pub const BLOQUEO_MAX_MS: i64 = 3_600_000;
/// Entre dos intentos de prueba, como mucho uno por segundo.
pub const ENTRE_INTENTOS_MS: i64 = 1_000;
/// Lo que espera el servidor a que el cliente mande su petición.
pub const ESPERA_PETICION_MS: u64 = 10_000;

/// Qué operaciones llevan prueba de administración (todas menos estas).
fn sin_prueba(op: &str) -> bool {
    matches!(op, "hola" | "crear_clave")
}

// ---------- Retos y límites (sin sistema operativo: se prueban) ----------

/// Los retos dados y aún sin usar.
#[derive(Default)]
pub struct Retos {
    vivos: Vec<(String, i64)>,
}

impl Retos {
    /// Uno nuevo (32 bytes aleatorios).
    pub fn nuevo(&mut self, ahora_ms: i64) -> String {
        use crypto_box::aead::rand_core::RngCore;
        let mut b = [0u8; 32];
        crypto_box::aead::OsRng.fill_bytes(&mut b);
        let r = B64.encode(b);
        self.vivos.retain(|(_, c)| *c > ahora_ms);
        if self.vivos.len() >= RETOS_MAX {
            self.vivos.remove(0);
        }
        self.vivos.push((r.clone(), ahora_ms + RETO_VIDA_S * 1000));
        r
    }

    /// Lo gasta: `true` si estaba vivo. Nunca vale dos veces.
    pub fn gastar(&mut self, reto: &str, ahora_ms: i64) -> bool {
        let Some(i) = self.vivos.iter().position(|(r, _)| r == reto) else { return false };
        let (_, caduca) = self.vivos.remove(i);
        caduca > ahora_ms
    }
}

/// Intentos fallidos de la prueba de administración en este equipo.
#[derive(Default, Debug)]
pub struct Limitador {
    fallos: u32,
    rachas: u32,
    bloqueado_hasta: i64,
    /// El último fallo: tras uno, como mucho un intento por segundo.
    ultimo_fallo: Option<i64>,
}

impl Limitador {
    /// ¿Se puede intentar ahora? Si no, por qué (sin decir nada de la clave).
    pub fn puede(&self, ahora_ms: i64) -> Result<(), String> {
        if ahora_ms < self.bloqueado_hasta {
            let min = ((self.bloqueado_hasta - ahora_ms) + 59_999) / 60_000;
            return Err(format!("Demasiados intentos con una clave incorrecta. Espera {min} min y vuelve a probar."));
        }
        if self.ultimo_fallo.is_some_and(|u| ahora_ms - u < ENTRE_INTENTOS_MS) {
            return Err("Espera un momento antes de volver a intentarlo.".into());
        }
        Ok(())
    }

    /// Un fallo: al quinto seguido, bloqueo (1 min, 2, 4… hasta 1 h).
    pub fn fallo(&mut self, ahora_ms: i64) -> u32 {
        self.ultimo_fallo = Some(ahora_ms);
        self.fallos += 1;
        if self.fallos >= FALLOS_MAX {
            let ms = (BLOQUEO_MS << self.rachas.min(10)).min(BLOQUEO_MAX_MS);
            self.bloqueado_hasta = ahora_ms + ms;
            self.rachas += 1;
            self.fallos = 0;
        }
        self.fallos
    }

    pub fn acierto(&mut self) {
        self.fallos = 0;
        self.rachas = 0;
        self.bloqueado_hasta = 0;
        self.ultimo_fallo = None;
    }
}

/// El estado del servidor entre peticiones (en memoria del servicio).
#[derive(Default)]
pub struct Servidor {
    pub retos: Retos,
    pub limitador: Limitador,
}

/// Lo que dice el servidor cuando la prueba no vale (siempre lo mismo: no da pistas).
pub const CLAVE_INCORRECTA: &str = "La clave de administración no es correcta.";

impl Servidor {
    /// Comprueba el reto y la prueba de una petición contra el verificador.
    /// El reto se gasta siempre (aunque falle lo demás).
    pub fn autorizar(&mut self, pet: &Value, verificador: Option<&[u8]>, ahora_ms: i64) -> Result<(), String> {
        let reto = pet["reto"].as_str().unwrap_or("");
        if !self.retos.gastar(reto, ahora_ms) {
            return Err("La petición caducó o ya se usó: vuelve a intentarlo.".into());
        }
        self.limitador.puede(ahora_ms)?;
        let Some(ver) = verificador else { return Err("Este equipo aún no tiene clave de administración.".into()) };
        let prueba = pet["prueba"].as_str().and_then(|p| B64.decode(p).ok()).unwrap_or_default();
        // Tiempo constante (y la prueba vacía o de otro tamaño, igual que una mala).
        if prueba.len() == 32 && resguardo_protocolo::derivaciones::comprueba_prueba(&prueba, ver) {
            self.limitador.acierto();
            Ok(())
        } else {
            let n = self.limitador.fallo(ahora_ms);
            crate::agent::log(&format!("Ventana del equipo: clave de administración incorrecta{}.", if n == 0 { " (bloqueado un rato)" } else { "" }));
            Err(CLAVE_INCORRECTA.into())
        }
    }
}

// ---------- Lo que calcula la ventana (cliente) ----------

/// La prueba de administración para este equipo (Argon2id: ~0,5 s y 64 MiB).
pub fn prueba(clave: &str, sal_equipo_b64: &str) -> Result<String, String> {
    Ok(B64.encode(resguardo_protocolo::derivaciones::prueba_admin(clave, sal_equipo_b64)?))
}

/// Mínimo de la clave de administración que se pone en el equipo.
pub const MIN_CLAVE: usize = 12;

/// `crear_clave`: sales nuevas, verificador y `K_cfg`, como la consola al dar el alta.
pub fn datos_clave_nueva(clave: &str) -> Result<Value, String> {
    if clave.chars().count() < MIN_CLAVE {
        return Err(format!("La clave de administración tiene que tener al menos {MIN_CLAVE} caracteres."));
    }
    use crypto_box::aead::rand_core::RngCore;
    let mut sal_equipo = [0u8; 16];
    let mut sal_cliente = [0u8; 16];
    crypto_box::aead::OsRng.fill_bytes(&mut sal_equipo);
    crypto_box::aead::OsRng.fill_bytes(&mut sal_cliente);
    let (sal_equipo, sal_cliente) = (B64.encode(sal_equipo), B64.encode(sal_cliente));
    let p = resguardo_protocolo::derivaciones::prueba_admin(clave, &sal_equipo)?;
    let kcfg = resguardo_protocolo::derivaciones::k_cfg(clave, &sal_cliente)?;
    Ok(json!({
        "sal_equipo": sal_equipo,
        "verificador": B64.encode(resguardo_protocolo::derivaciones::verificador(&p)),
        "k_cfg": B64.encode(kcfg),
    }))
}

// ---------- Lo que hace el servicio ----------

fn b64_de(v: &Value, k: &str, largo: Option<usize>) -> Result<String, String> {
    let s = v[k].as_str().ok_or_else(|| format!("Falta «{k}»."))?;
    let b = B64.decode(s).map_err(|_| format!("«{k}» no válido."))?;
    match largo {
        Some(n) if b.len() != n => Err(format!("«{k}» no válido.")),
        None if b.len() < 16 => Err(format!("«{k}» no válido.")),
        _ => Ok(s.to_string()),
    }
}

/// El vínculo de un equipo nuevo en modo local, con la clave que dio la ventana.
pub fn vinculo_local(datos: &Value) -> Result<crate::servidor_v2::Vinculo, String> {
    use crypto_box::aead::rand_core::RngCore;
    let mut semilla = [0u8; 32];
    crypto_box::aead::OsRng.fill_bytes(&mut semilla);
    Ok(crate::servidor_v2::Vinculo {
        modo: "local".into(),
        box_secret: resguardo_protocolo::claves::new_key(),
        sign_seed: B64.encode(semilla),
        sal_equipo: b64_de(datos, "sal_equipo", None)?,
        verificador: Some(b64_de(datos, "verificador", Some(32))?),
        k_cfg: Some(b64_de(datos, "k_cfg", Some(32))?),
        config_v1: Some(json!({ "v": 1, "copias": [], "escritorio": crate::escritorio::Escritorio::default().a_json() })),
        ..Default::default()
    })
}

/// En qué modo está el equipo, para la ventana.
pub fn modo(v: Option<&crate::servidor_v2::Vinculo>, web: bool) -> &'static str {
    match v {
        _ if web => "web",
        Some(v) if v.verificador.is_some() && v.modo == "local" && v.url.is_empty() => "local",
        Some(v) if v.verificador.is_some() && v.modo == "gestionado" => "gestionado",
        Some(v) if v.verificador.is_some() => "desvinculado",
        Some(_) => "pendiente",
        None => "sin_clave",
    }
}

/// Ajustes del escritorio sobre la configuración que tiene el equipo.
pub fn poner_escritorio(v: &mut crate::servidor_v2::Vinculo, e: crate::escritorio::Escritorio) {
    let mut cfg = v.config_v1.clone().filter(|c| c.is_object()).unwrap_or_else(|| json!({ "v": 1, "copias": [] }));
    cfg["escritorio"] = e.a_json();
    cfg["cambiado_en_equipo"] = json!(chrono::Local::now().to_rfc3339());
    v.config_v1 = Some(cfg);
    v.cambio_local += 1;
}

/// Una petición, en el servicio. `servidor` guarda los retos y los fallos.
pub fn atender(servidor: &std::sync::Mutex<Servidor>, pet: &Value) -> Value {
    respuesta(atender_(servidor, pet))
}

fn respuesta(r: Result<Value, String>) -> Value {
    match r {
        Ok(v) => json!({ "ok": true, "datos": v }),
        Err(e) => json!({ "ok": false, "error": e }),
    }
}

/// Quien atiende las peticiones (el servicio: [`atender`]; en las pruebas, otro).
pub type Atiende = std::sync::Arc<dyn Fn(&Value) -> Value + Send + Sync>;

fn ahora_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn atender_(servidor: &std::sync::Mutex<Servidor>, pet: &Value) -> Result<Value, String> {
    use crate::servidor_v2 as s;
    let op = pet["op"].as_str().unwrap_or("");
    let web = crate::endpoint::load().is_some_and(|e| !e.stopped);
    let v = s::cargar();
    if op == "hola" {
        let reto = servidor.lock().map_err(|_| "Ocupado.")?.retos.nuevo(ahora_ms());
        return Ok(json!({
            "v": 1,
            "reto": reto,
            "modo": modo(v.as_ref(), web),
            "sal_equipo": v.as_ref().filter(|v| v.verificador.is_some()).map(|v| v.sal_equipo.clone()),
        }));
    }
    if op == "crear_clave" {
        // Solo un equipo sin clave, sin vincular y sin la consola web: quien llega
        // primero a un equipo recién instalado la pone (como el código de vincular).
        let mut sv = servidor.lock().map_err(|_| "Ocupado.")?;
        if !sv.retos.gastar(pet["reto"].as_str().unwrap_or(""), ahora_ms()) {
            return Err("La petición caducó o ya se usó: vuelve a intentarlo.".into());
        }
        sv.limitador.puede(ahora_ms())?;
        if v.is_some() || web {
            return Err("Este equipo ya tiene clave de administración o está vinculado a una consola.".into());
        }
        let nuevo = vinculo_local(pet)?;
        s::guardar(&nuevo)?;
        crate::agent::log("Ventana del equipo: clave de administración puesta; el equipo se usa sin consola (modo local).");
        crate::agente::refrescar_bandeja();
        return Ok(json!({ "modo": "local" }));
    }
    if sin_prueba(op) {
        return Err("Operación no válida.".into());
    }
    let verificador = v.as_ref().and_then(|v| v.verificador.as_deref()).and_then(|x| B64.decode(x).ok());
    servidor.lock().map_err(|_| "Ocupado.")?.autorizar(pet, verificador.as_deref(), ahora_ms())?;
    let mut v = v.ok_or("Este equipo aún no tiene clave de administración.")?;
    let local = modo(Some(&v), web) == "local";
    match op {
        // Solo comprobar la clave («Desbloquear»).
        "comprobar" => Ok(json!({ "modo": modo(Some(&v), web) })),
        "ajustes" => {
            if web {
                return Err("Este equipo lo gestiona la consola web: los ajustes se cambian allí.".into());
            }
            let e = crate::escritorio::Escritorio::validar(&pet["escritorio"])?;
            poner_escritorio(&mut v, e);
            let _ = crate::gestion_v2::subir_config(&mut v);
            s::guardar(&v)?;
            crate::agent::log("Ventana del equipo: ajustes de la ventana y los avisos cambiados en el equipo.");
            crate::agente::refrescar_bandeja();
            Ok(json!({ "escritorio": e.a_json() }))
        }
        _ if !local => Err("En un equipo gestionado por una consola, las copias se cambian en la consola.".into()),
        "estado_local" => Ok(estado_local(&v)),
        "crear_repositorio" => {
            let m = crate::gestion_v2::crear_repositorio(&mut v, &pet["repositorio"])?;
            s::guardar(&v)?;
            crate::agent::log("Ventana del equipo: repositorio creado (modo local).");
            Ok(json!({ "mensaje": m }))
        }
        "config" => {
            let m = crate::gestion_v2::aplicar_config_desde(&mut v, &json!({ "config": pet["config"] }), true)?;
            v.cambio_local += 1;
            s::guardar(&v)?;
            crate::agent::log("Ventana del equipo: copias cambiadas en el equipo (modo local).");
            crate::agente::refrescar_bandeja();
            Ok(json!({ "mensaje": m }))
        }
        "carpetas" => {
            let que = pet["que"].as_str().unwrap_or("carpetas");
            crate::sesiones_v2::operar_local(&crate::sesiones_v2::Tipo::Carpetas, que, &pet["p"])
        }
        "explorar" => {
            let acc = crate::gestion_v2::acceso(&v, pet["repo"].as_str().unwrap_or(""))?;
            crate::sesiones_v2::operar_local(&crate::sesiones_v2::Tipo::Explorar(Box::new(acc)), pet["que"].as_str().unwrap_or(""), &pet["p"])
        }
        "restaurar" => {
            let repo = pet["repo"].as_str().unwrap_or("");
            let acc = crate::gestion_v2::acceso(&v, repo)?;
            // Junto al original, en su sitio (sin reemplazar si no se pide) o en otra carpeta.
            let destino = pet["destino"].as_str().filter(|d| matches!(*d, "original" | "carpeta")).unwrap_or("junto");
            let c = json!({
                "repo": repo, "version": pet["version"], "rutas": pet["rutas"], "destino": destino,
                "carpeta": pet["carpeta"], "reemplazar": pet["reemplazar"].as_bool().unwrap_or(false),
            });
            let m = crate::sesiones_v2::restaurar(&acc, &c)?;
            crate::agent::log("Ventana del equipo: archivos restaurados (modo local).");
            Ok(json!({ "mensaje": m }))
        }
        "retencion" => {
            let repo = pet["repo"].as_str().unwrap_or("").to_string();
            let m = crate::gestion_v2::cambiar_retencion(&mut v, &pet["retencion"], &repo)?;
            s::guardar(&v)?;
            // Se aplica ya, en segundo plano (puede tardar: `forget --prune`).
            if pet["aplicar"] == true {
                let copia = v.clone();
                std::thread::spawn(move || {
                    let r = crate::gestion_v2::aplicar_retencion(&copia, &repo);
                    crate::agent::log(&format!("Ventana del equipo: retención aplicada: {}", r.unwrap_or_else(|e| format!("ERROR: {e}"))));
                });
            }
            Ok(json!({ "mensaje": m }))
        }
        "copia_externa" => {
            let repo = pet["repo"].as_str().unwrap_or("").to_string();
            let m = crate::gestion_v2::cambiar_copia_externa(&mut v, &pet["externa"], &repo)?;
            s::guardar(&v)?;
            Ok(json!({ "mensaje": m }))
        }
        "pausar" => Ok(json!({ "mensaje": crate::gestion_v2::pausar(&v, &json!({ "horas": pet["horas"] }))? })),
        "reanudar" => Ok(json!({ "mensaje": crate::gestion_v2::reanudar(&v, &json!({}))? })),
        "guarda_copias" => {
            // Este equipo guarda copias (su Servidor de copias) y su espejo; añadir
            // equipos cliente necesita una consola (su contraseña se sella para ella).
            if pet["cuerpo"].get("anadir").is_some() {
                return Err("Para que otros equipos guarden aquí, vincula este equipo a una consola.".into());
            }
            let (m, _) = crate::gestion_v2::guarda_copias(&pet["cuerpo"], false)?;
            Ok(json!({ "mensaje": m }))
        }
        "conectar_nube" => Ok(json!({
            "mensaje": crate::nube::anadir(pet["tipo"].as_str().unwrap_or(""), pet["nombre"].as_str().unwrap_or(""), pet["token"].as_str().unwrap_or(""))?
        })),
        "quitar_nube" => Ok(json!({ "mensaje": crate::nube::quitar(pet["nombre"].as_str().unwrap_or(""))? })),
        // «Usar uno que ya existe» (o solo probarlo) y «Traer historial» (en segundo
        // plano; la ventana pregunta cómo va), como esas órdenes desde la consola.
        "adoptar_repositorio" => {
            let c = &pet["repositorio"];
            let (m, detalle) = crate::adoptar_v2::adoptar_repositorio(&mut v, c, true)?;
            if c["solo_probar"] != true {
                s::guardar(&v)?;
                crate::agent::log("Ventana del equipo: repositorio que ya existía adoptado (modo local).");
            }
            Ok(json!({ "mensaje": m, "detalle": detalle }))
        }
        "copiar_historial" => Ok(json!({ "mensaje": crate::adoptar_v2::copiar_historial_local(&v, &pet["historial"])? })),
        "historial_traido" => Ok(crate::adoptar_v2::historial_local(pet["repo"].as_str().unwrap_or(""))),
        "historial" => Ok(historial()),
        "kit" => Ok(kit(&v)),
        "cambiar_clave" => {
            let nuevo = vinculo_local(pet)?;
            v.sal_equipo = nuevo.sal_equipo;
            v.verificador = nuevo.verificador;
            v.k_cfg = nuevo.k_cfg;
            s::guardar(&v)?;
            if let Ok(mut sv) = servidor.lock() {
                sv.limitador.acierto();
            }
            crate::agent::log("Ventana del equipo: clave de administración cambiada en el equipo.");
            Ok(json!({ "mensaje": "Clave de administración cambiada." }))
        }
        "vincular" => {
            // Conserva todo: con clave, queda pendiente del alta de la consola con la MISMA clave.
            let r = s::vincular(pet["url"].as_str().unwrap_or(""), pet["codigo"].as_str().unwrap_or(""), &crate::web::default_device_name())?;
            Ok(json!({ "sas": r.sas, "sas_v3": r.sas_v3, "huella_ca": r.huella_ca, "servidor": r.servidor, "espera_alta": r.espera_alta }))
        }
        _ => Err(format!("Operación desconocida: «{op}».")),
    }
}

/// Repositorios, destinos (sin credenciales), copias, nubes y el Servidor de
/// copias, para el editor local. Lo mismo que ve la consola en el resumen, más
/// la carpeta de los destinos locales (aquí la ve el administrador del equipo).
pub fn estado_local(v: &crate::servidor_v2::Vinculo) -> Value {
    let config = crate::agent::load_config();
    let pausa = config.repos.iter().filter_map(|r| r.active_pause(chrono::Local::now())).map(|p| p.until.clone()).next();
    json!({
        "repositorios": v.repos_v2.iter().filter(|r| !r.solo_lectura).map(|r| json!({
            "id": r.id, "nombre": r.nombre, "destino": r.destino, "retencion": r.retencion,
            "externa": r.externa.as_ref().map(|e| json!({ "destino": e["destino"], "hora": e["hora"] })),
        })).collect::<Vec<_>>(),
        "destinos": v.destinos.iter().map(|d| json!({ "id": d.id, "nombre": d.nombre, "tipo": d.tipo, "donde": d.donde })).collect::<Vec<_>>(),
        "config": v.config_v1.clone().unwrap_or_else(|| json!({ "v": 1, "copias": [] })),
        "resumen": crate::gestion_v2::resumen(v),
        "nubes": crate::nube::lista(),
        "pausado_hasta": pausa.map(|u| json!(u.unwrap_or_else(|| "indefinido".into()))),
        "nombre_equipo": crate::web::default_device_name(),
    })
}

/// Lo que ha pasado en el equipo (copias, verificaciones, cambios), lo último primero.
pub fn historial() -> Value {
    let mut e = crate::history::read(&crate::history::agent_file());
    e.reverse();
    e.truncate(300);
    json!(e
        .iter()
        .map(|x| json!({
            "tipo": x.kind, "origen": x.origin, "repo": x.repo_name, "copia": x.plan_name,
            "empezo": x.started, "cuando": x.finished, "resultado": x.result,
            "mensaje": crate::web::public_message(&x.message),
            "bytes": x.data_added, "nuevos": x.files_new, "cambiados": x.files_changed, "sin_cambios": x.unchanged,
        }))
        .collect::<Vec<_>>())
}

/// El kit de recuperación de cada repositorio: dónde está (sin usuario ni
/// contraseña del servidor) y su ID de restic. Las contraseñas no salen de
/// aquí: se escriben a mano en la hoja impresa (o al crear el repositorio).
pub fn kit(v: &crate::servidor_v2::Vinculo) -> Value {
    json!(v
        .repos_v2
        .iter()
        .filter(|r| !r.solo_lectura)
        .map(|r| {
            let destino = v.destinos.iter().find(|d| d.id == r.destino);
            let acc = crate::gestion_v2::acceso(v, &r.id);
            json!({
                "id": r.id, "nombre": r.nombre,
                "destino": destino.map(|d| d.nombre.clone()), "tipo": destino.map(|d| d.tipo.clone()),
                "ubicacion": acc.as_ref().ok().map(|a| crate::kit::public_location(&a.location)),
                "id_restic": acc.as_ref().ok().and_then(|a| crate::kit::config_id(a).ok()),
                "usuario_servidor": destino.and_then(|d| d.usuario.clone()),
            })
        })
        .collect::<Vec<_>>())
}

// ---------- Transporte ----------

/// El nombre de la tubería (Windows) o la ruta del socket (Unix). En pruebas,
/// uno propio de la carpeta de pruebas: nunca el del servicio instalado.
pub fn direccion() -> String {
    let pruebas = crate::agent::test_mode().then(|| {
        use sha2::Digest;
        let h = sha2::Sha256::digest(crate::agent::agent_dir().to_string_lossy().as_bytes());
        h[..6].iter().map(|b| format!("{b:02x}")).collect::<String>()
    });
    #[cfg(windows)]
    {
        match pruebas {
            Some(h) => format!(r"\\.\pipe\ResguardoAgente-pruebas-{h}"),
            None => r"\\.\pipe\ResguardoAgente".into(),
        }
    }
    #[cfg(not(windows))]
    {
        match pruebas {
            Some(_) => crate::agent::agent_dir().join("ipc.sock").to_string_lossy().into_owned(),
            None => "/run/resguardo-agente/ipc.sock".into(),
        }
    }
}

/// Lee una línea (hasta `max` bytes) de lo que vaya llegando.
fn leer_linea(r: &mut (impl std::io::Read + ?Sized), max: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut b = [0u8; 4096];
    loop {
        let n = r.read(&mut b).map_err(|e| format!("No se pudo leer: {e}"))?;
        if n == 0 {
            break;
        }
        out.extend_from_slice(&b[..n]);
        if let Some(i) = out.iter().position(|c| *c == b'\n') {
            out.truncate(i);
            break;
        }
        if out.len() > max {
            return Err("Mensaje demasiado grande.".into());
        }
    }
    if out.len() > max {
        return Err("Mensaje demasiado grande.".into());
    }
    Ok(out)
}

/// Atiende una conexión: una petición y su respuesta.
fn servir(atiende: &Atiende, conexion: &mut (impl std::io::Read + std::io::Write)) {
    let respuesta = match leer_linea(conexion, MAX_MENSAJE).and_then(|l| serde_json::from_slice::<Value>(&l).map_err(|_| "Petición no válida.".to_string())) {
        Ok(pet) => atiende(&pet),
        Err(e) => json!({ "ok": false, "error": e }),
    };
    let mut texto = respuesta.to_string();
    if texto.len() > MAX_RESPUESTA {
        texto = json!({ "ok": false, "error": "La respuesta es demasiado grande." }).to_string();
    }
    texto.push('\n');
    let _ = conexion.write_all(texto.as_bytes());
    let _ = conexion.flush();
}

/// Cliente: manda una petición y espera la respuesta (`datos`, o el error).
pub fn pedir(pet: &Value) -> Result<Value, String> {
    let mut c = transporte::conectar(&direccion())?;
    let mut texto = pet.to_string();
    texto.push('\n');
    use std::io::Write;
    c.write_all(texto.as_bytes()).map_err(|e| format!("No se pudo hablar con el servicio: {e}"))?;
    c.flush().map_err(|e| format!("No se pudo hablar con el servicio: {e}"))?;
    let linea = leer_linea(&mut c, MAX_RESPUESTA)?;
    let r: Value = serde_json::from_slice(&linea).map_err(|_| "El servicio respondió algo que no se entiende.".to_string())?;
    if r["ok"] == true {
        Ok(r["datos"].clone())
    } else {
        Err(r["error"].as_str().unwrap_or("Error desconocido.").to_string())
    }
}

/// Cliente: `hola` y, con su reto, la petición con la prueba.
pub fn pedir_con_prueba(op: &str, prueba_b64: &str, mut cuerpo: Value) -> Result<Value, String> {
    let hola = pedir(&json!({ "op": "hola" }))?;
    cuerpo["op"] = json!(op);
    cuerpo["reto"] = hola["reto"].clone();
    cuerpo["prueba"] = json!(prueba_b64);
    pedir(&cuerpo)
}

/// En el servicio: atiende la tubería (o el socket) en un hilo.
pub fn hilo() {
    std::thread::spawn(|| {
        let servidor = std::sync::Arc::new(std::sync::Mutex::new(Servidor::default()));
        let atiende: Atiende = std::sync::Arc::new(move |pet: &Value| atender(&servidor, pet));
        loop {
            if let Err(e) = transporte::escuchar(&direccion(), atiende.clone()) {
                crate::agent::log(&format!("Ventana del equipo: no se pudo abrir el canal local ({e}); se reintenta en un minuto."));
            }
            std::thread::sleep(std::time::Duration::from_secs(60));
        }
    });
}

#[cfg(windows)]
mod transporte {
    use super::*;
    use std::fs::File;
    use std::os::windows::io::FromRawHandle;
    use std::sync::Arc;
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, LocalFree, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo, SDDL_REVISION_1, SE_KERNEL_OBJECT,
    };
    use windows_sys::Win32::Security::{
        IsWellKnownSid, WinBuiltinAdministratorsSid, WinLocalSystemSid, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_FIRST_PIPE_INSTANCE, OPEN_EXISTING, PIPE_ACCESS_DUPLEX, SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT,
    };
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PeekNamedPipe, WaitNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE,
        PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };

    /// Lo que pide el cliente: leer y escribir datos (lo que la DACL da a los usuarios).
    const ACCESO_CLIENTE: u32 = 0x0012_008B;

    /// SYSTEM, administradores y quien la crea (el servicio): todo. Usuarios con
    /// sesión interactiva: leer (0x120089) y escribir datos (0x2), sin
    /// FILE_CREATE_PIPE_INSTANCE (0x4): no pueden crear otra instancia de la
    /// tubería ni suplantarla.
    const SDDL: &str = "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;OW)(A;;0x0012008B;;;IU)";

    fn ancho(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn crear(nombre: &[u16], sd: PSECURITY_DESCRIPTOR, primera: bool) -> Result<HANDLE, String> {
        let sa = SECURITY_ATTRIBUTES { nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32, lpSecurityDescriptor: sd, bInheritHandle: 0 };
        // SAFETY: nombre terminado en cero y atributos que viven durante la llamada.
        let h = unsafe {
            CreateNamedPipeW(
                nombre.as_ptr(),
                PIPE_ACCESS_DUPLEX | if primera { FILE_FLAG_FIRST_PIPE_INSTANCE } else { 0 },
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                64 * 1024,
                64 * 1024,
                0,
                &sa,
            )
        };
        if h == INVALID_HANDLE_VALUE {
            return Err(format!("CreateNamedPipe: error {}", unsafe { GetLastError() }));
        }
        Ok(h)
    }

    /// Una conexión con una espera para la petición (un cliente que no dice
    /// nada no deja el hilo bloqueado para siempre).
    pub struct Conexion {
        archivo: File,
        h: HANDLE,
        espera: std::time::Duration,
    }

    impl std::io::Read for Conexion {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let limite = std::time::Instant::now() + self.espera;
            loop {
                let mut hay: u32 = 0;
                // SAFETY: handle válido mientras vive la conexión; punteros a variables locales.
                let ok = unsafe { PeekNamedPipe(self.h, std::ptr::null_mut(), 0, std::ptr::null_mut(), &mut hay, std::ptr::null_mut()) };
                if ok == 0 {
                    return Ok(0);
                }
                if hay > 0 {
                    return self.archivo.read(buf);
                }
                if std::time::Instant::now() >= limite {
                    return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "sin petición"));
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
    }

    impl std::io::Write for Conexion {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.archivo.write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.archivo.flush()
        }
    }

    pub fn escuchar(direccion: &str, atiende: Atiende) -> Result<(), String> {
        let nombre = ancho(direccion);
        let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        let sddl = ancho(SDDL);
        // SAFETY: cadena terminada en cero; `sd` lo libera LocalFree al salir (nunca, en la práctica).
        if unsafe { ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), SDDL_REVISION_1, &mut sd, std::ptr::null_mut()) } == 0 {
            return Err("descriptor de seguridad no válido".into());
        }
        let r = (|| {
            // La primera instancia, solo si nadie tiene ya una tubería con ese nombre.
            let mut actual = crear(&nombre, sd, true)?;
            let activas = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            loop {
                // SAFETY: handle de tubería propio.
                let ok = unsafe { ConnectNamedPipe(actual, std::ptr::null_mut()) } != 0 || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
                // La siguiente instancia antes de atender esta: siempre hay una nuestra.
                let siguiente = crear(&nombre, sd, false)?;
                let h = actual;
                actual = siguiente;
                if !ok {
                    // SAFETY: handle propio.
                    unsafe { CloseHandle(h) };
                    continue;
                }
                // Como mucho 4 a la vez (una ventana usa una).
                if activas.load(std::sync::atomic::Ordering::SeqCst) >= 4 {
                    unsafe {
                        DisconnectNamedPipe(h);
                        CloseHandle(h);
                    }
                    continue;
                }
                activas.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let (atiende, activas) = (atiende.clone(), activas.clone());
                let hv = h as usize;
                std::thread::spawn(move || {
                    let h = hv as HANDLE;
                    // SAFETY: el handle pasa a ser del `File`, que lo cierra al soltarlo.
                    let mut c = Conexion { archivo: unsafe { File::from_raw_handle(h as _) }, h, espera: std::time::Duration::from_millis(ESPERA_PETICION_MS) };
                    servir(&atiende, &mut c);
                    // Se cierra sin DisconnectNamedPipe: así el cliente aún puede leer la
                    // respuesta (desconectar descarta lo que no haya leído).
                    drop(c);
                    activas.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                });
            }
        })();
        // SAFETY: lo reservó ConvertStringSecurityDescriptorToSecurityDescriptorW.
        unsafe { LocalFree(sd as _) };
        r
    }

    /// ¿La tubería es de SYSTEM o de los administradores? (Si no, la creó otro usuario.)
    fn de_confianza(h: HANDLE) -> bool {
        if cfg!(test) || crate::agent::test_mode() {
            return true;
        }
        let mut dueno: windows_sys::Win32::Security::PSID = std::ptr::null_mut();
        let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        // SAFETY: handle abierto; los punteros de salida son locales y `sd` se libera.
        let r = unsafe {
            GetSecurityInfo(
                h,
                SE_KERNEL_OBJECT,
                OWNER_SECURITY_INFORMATION,
                &mut dueno,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut sd,
            )
        };
        if r != 0 {
            return false;
        }
        // SAFETY: `dueno` apunta dentro de `sd`, que sigue vivo.
        let ok = unsafe { IsWellKnownSid(dueno, WinLocalSystemSid) != 0 || IsWellKnownSid(dueno, WinBuiltinAdministratorsSid) != 0 };
        unsafe { LocalFree(sd as _) };
        ok
    }

    /// Una conexión con el servicio. Espera la respuesta lo que haga falta (una
    /// restauración puede tardar), pero si el servicio cierra, no se queda colgada.
    pub fn conectar(direccion: &str) -> Result<Conexion, String> {
        let nombre = ancho(direccion);
        for _ in 0..20 {
            // SAFETY: nombre terminado en cero. SQOS: el servidor no puede suplantar al usuario.
            let h = unsafe {
                CreateFileW(
                    nombre.as_ptr(),
                    ACCESO_CLIENTE,
                    0,
                    std::ptr::null(),
                    OPEN_EXISTING,
                    SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                    std::ptr::null_mut(),
                )
            };
            if h != INVALID_HANDLE_VALUE {
                if !de_confianza(h) {
                    unsafe { CloseHandle(h) };
                    return Err("El canal local no es del servicio de Resguardo: no se usa.".into());
                }
                // SAFETY: el handle pasa a ser del `File`.
                return Ok(Conexion { archivo: unsafe { File::from_raw_handle(h as _) }, h, espera: std::time::Duration::from_secs(24 * 3600) });
            }
            if unsafe { GetLastError() } != ERROR_PIPE_BUSY {
                return Err("El servicio de Resguardo no responde (¿está en marcha?).".into());
            }
            unsafe { WaitNamedPipeW(nombre.as_ptr(), 500) };
        }
        Err("El servicio de Resguardo está ocupado: vuelve a intentarlo.".into())
    }
}

#[cfg(unix)]
mod transporte {
    use super::*;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::os::unix::net::{UnixListener, UnixStream};

    pub fn escuchar(direccion: &str, atiende: Atiende) -> Result<(), String> {
        let ruta = std::path::Path::new(direccion);
        if let Some(dir) = ruta.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            if !crate::agent::test_mode() {
                let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755));
            }
        }
        let _ = std::fs::remove_file(ruta);
        let l = UnixListener::bind(ruta).map_err(|e| e.to_string())?;
        // Cualquier usuario local puede hablar; lo que autoriza es la prueba.
        let _ = std::fs::set_permissions(ruta, std::fs::Permissions::from_mode(0o666));
        for c in l.incoming().flatten() {
            let atiende = atiende.clone();
            std::thread::spawn(move || {
                let mut c = c;
                let _ = c.set_read_timeout(Some(std::time::Duration::from_millis(ESPERA_PETICION_MS)));
                servir(&atiende, &mut c);
            });
        }
        Ok(())
    }

    pub fn conectar(direccion: &str) -> Result<UnixStream, String> {
        // El socket tiene que ser de root (si no, lo puso otro usuario).
        let dueno = std::fs::metadata(direccion).map(|m| m.uid()).map_err(|_| "El servicio de Resguardo no responde (¿está en marcha?).".to_string())?;
        if dueno != 0 && !cfg!(test) && !crate::agent::test_mode() {
            return Err("El canal local no es del servicio de Resguardo: no se usa.".into());
        }
        UnixStream::connect(direccion).map_err(|_| "El servicio de Resguardo no responde (¿está en marcha?).".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLAVE: &str = "caballo bateria grapa correcta";

    fn verificador_de(clave: &str, sal: &str) -> Vec<u8> {
        resguardo_protocolo::derivaciones::verificador(&resguardo_protocolo::derivaciones::prueba_admin(clave, sal).unwrap()).to_vec()
    }

    #[test]
    fn la_clave_buena_pasa_y_la_mala_no() {
        let sal = B64.encode([7u8; 16]);
        let ver = verificador_de(CLAVE, &sal);
        let mut s = Servidor::default();
        let t = 1_000_000;
        let reto = s.retos.nuevo(t);
        let bien = json!({ "reto": reto, "prueba": prueba(CLAVE, &sal).unwrap() });
        assert!(s.autorizar(&bien, Some(&ver), t).is_ok());
        // La mala, con su reto: no, y sin decir más.
        let reto = s.retos.nuevo(t + 2_000);
        let mal = json!({ "reto": reto, "prueba": prueba("otra clave cualquiera", &sal).unwrap() });
        assert_eq!(s.autorizar(&mal, Some(&ver), t + 2_000).unwrap_err(), CLAVE_INCORRECTA);
        // Sin prueba, o con basura: igual que una mala.
        let reto = s.retos.nuevo(t + 4_000);
        assert_eq!(s.autorizar(&json!({ "reto": reto }), Some(&ver), t + 4_000).unwrap_err(), CLAVE_INCORRECTA);
        let reto = s.retos.nuevo(t + 6_000);
        assert_eq!(s.autorizar(&json!({ "reto": reto, "prueba": "!!" }), Some(&ver), t + 6_000).unwrap_err(), CLAVE_INCORRECTA);
        // Un equipo sin clave no acepta nada.
        let reto = s.retos.nuevo(t + 8_000);
        assert!(s.autorizar(&json!({ "reto": reto, "prueba": prueba(CLAVE, &sal).unwrap() }), None, t + 8_000).is_err());
    }

    #[test]
    fn sin_repeticion_y_con_caducidad() {
        let sal = B64.encode([1u8; 16]);
        let ver = verificador_de(CLAVE, &sal);
        let mut s = Servidor::default();
        let t = 5_000_000;
        let reto = s.retos.nuevo(t);
        let pet = json!({ "reto": reto, "prueba": prueba(CLAVE, &sal).unwrap() });
        assert!(s.autorizar(&pet, Some(&ver), t).is_ok());
        // La misma petición otra vez (repetida por quien la vio): no.
        assert!(s.autorizar(&pet, Some(&ver), t + 5_000).unwrap_err().contains("ya se usó"));
        // Un reto inventado: no.
        assert!(s.autorizar(&json!({ "reto": B64.encode([9u8; 32]), "prueba": pet["prueba"] }), Some(&ver), t + 7_000).is_err());
        // Un reto caducado: no (y se gasta).
        let reto = s.retos.nuevo(t + 10_000);
        let tarde = t + 10_000 + RETO_VIDA_S * 1000 + 1;
        let pet = json!({ "reto": reto, "prueba": prueba(CLAVE, &sal).unwrap() });
        assert!(s.autorizar(&pet, Some(&ver), tarde).is_err());
        assert!(s.autorizar(&pet, Some(&ver), tarde + 2_000).is_err());
        // Un reto gastado por una prueba mala tampoco sirve para la buena.
        let reto = s.retos.nuevo(t + 20_000);
        assert!(s.autorizar(&json!({ "reto": reto, "prueba": B64.encode([0u8; 32]) }), Some(&ver), t + 20_000).is_err());
        assert!(s.autorizar(&json!({ "reto": reto, "prueba": prueba(CLAVE, &sal).unwrap() }), Some(&ver), t + 22_000).is_err());
        // No se acumulan retos sin fin.
        for i in 0..100 {
            s.retos.nuevo(t + 30_000 + i);
        }
        assert!(s.retos.vivos.len() <= RETOS_MAX);
    }

    #[test]
    fn limite_de_intentos() {
        let mut l = Limitador::default();
        let mut t = 0;
        // Con la clave buena, sin esperas (la ventana pide varias cosas seguidas).
        assert!(l.puede(t).is_ok());
        assert!(l.puede(t + 1).is_ok());
        // Tras un fallo, uno por segundo como mucho.
        l.fallo(t);
        assert!(l.puede(t + 500).is_err());
        assert!(l.puede(t + 1_000).is_ok());
        t += 1_000;
        l.acierto();
        // Cinco fallos: bloqueado un minuto.
        for _ in 0..FALLOS_MAX {
            t += 1_000;
            assert!(l.puede(t).is_ok());
            l.fallo(t);
        }
        assert!(l.puede(t + 1_000).unwrap_err().contains("Espera 1 min"));
        assert!(l.puede(t + BLOQUEO_MS - 1).is_err());
        assert!(l.puede(t + BLOQUEO_MS).is_ok());
        // Otra racha: el doble.
        t += BLOQUEO_MS;
        for _ in 0..FALLOS_MAX {
            t += 1_000;
            l.fallo(t);
        }
        assert!(l.puede(t + BLOQUEO_MS).is_err());
        assert!(l.puede(t + 2 * BLOQUEO_MS).is_ok());
        // Nunca más de una hora.
        for _ in 0..20 {
            t += BLOQUEO_MAX_MS;
            for _ in 0..FALLOS_MAX {
                t += 1_000;
                l.fallo(t);
            }
        }
        assert!(l.puede(t + BLOQUEO_MAX_MS).is_ok());
        // Un acierto lo deja como nuevo.
        l.acierto();
        assert!(l.puede(t + 1_000).is_ok());
    }

    #[test]
    fn bloqueado_ni_con_la_clave_buena() {
        let sal = B64.encode([3u8; 16]);
        let ver = verificador_de(CLAVE, &sal);
        let mut s = Servidor::default();
        let mut t = 10_000_000;
        let mala = prueba("no es la clave", &sal).unwrap();
        for _ in 0..FALLOS_MAX {
            t += 1_000;
            let reto = s.retos.nuevo(t);
            assert!(s.autorizar(&json!({ "reto": reto, "prueba": mala }), Some(&ver), t).is_err());
        }
        t += 1_000;
        let reto = s.retos.nuevo(t);
        let e = s.autorizar(&json!({ "reto": reto, "prueba": prueba(CLAVE, &sal).unwrap() }), Some(&ver), t).unwrap_err();
        assert!(e.contains("Demasiados intentos"), "{e}");
        t += BLOQUEO_MS;
        let reto = s.retos.nuevo(t);
        assert!(s.autorizar(&json!({ "reto": reto, "prueba": prueba(CLAVE, &sal).unwrap() }), Some(&ver), t).is_ok());
    }

    #[test]
    fn clave_nueva_como_la_consola() {
        assert!(datos_clave_nueva("corta").is_err());
        let d = datos_clave_nueva(CLAVE).unwrap();
        let v = vinculo_local(&d).unwrap();
        assert_eq!((v.modo.as_str(), v.url.as_str()), ("local", ""));
        assert_eq!(modo(Some(&v), false), "local");
        // La prueba de la clave (con la sal del equipo) corresponde al verificador.
        let p = B64.decode(prueba(CLAVE, &v.sal_equipo).unwrap()).unwrap();
        assert!(resguardo_protocolo::derivaciones::comprueba_prueba(&p, &B64.decode(v.verificador.as_ref().unwrap()).unwrap()));
        // Datos rotos: no.
        assert!(vinculo_local(&json!({ "sal_equipo": "AAAA", "verificador": d["verificador"], "k_cfg": d["k_cfg"] })).is_err());
        assert!(vinculo_local(&json!({ "sal_equipo": d["sal_equipo"], "verificador": "AAAA", "k_cfg": d["k_cfg"] })).is_err());
        // Modos.
        assert_eq!(modo(None, false), "sin_clave");
        assert_eq!(modo(None, true), "web");
        let pendiente = crate::servidor_v2::Vinculo { url: "https://x".into(), ..Default::default() };
        assert_eq!(modo(Some(&pendiente), false), "pendiente");
        let gestionado =
            crate::servidor_v2::Vinculo { url: "https://x".into(), modo: "gestionado".into(), verificador: Some("v".into()), ..Default::default() };
        assert_eq!(modo(Some(&gestionado), false), "gestionado");
    }

    #[test]
    fn ajustes_del_escritorio_sobre_la_configuracion() {
        let mut v = crate::servidor_v2::Vinculo {
            config_v1: Some(json!({ "v": 1, "copias": [], "verificacion": { "cada_dias": 7 }, "bandeja": { "visible": true, "avisos": false } })),
            ..Default::default()
        };
        let e = crate::escritorio::Escritorio { ventana: crate::escritorio::Ventana::AlTrabajar, avisos: crate::escritorio::Avisos::Todo };
        poner_escritorio(&mut v, e);
        let c = v.config_v1.as_ref().unwrap();
        assert_eq!(c["verificacion"]["cada_dias"], 7, "no toca lo demás");
        assert_eq!(crate::escritorio::Escritorio::de_config(c), e);
        assert!(c["cambiado_en_equipo"].is_string());
        assert_eq!(v.cambio_local, 1);
        // Y el documento que sube a la consola lo lleva (ida y vuelta por `Configuracion`).
        let cfg: crate::gestion_v2::Configuracion = serde_json::from_value(c.clone()).unwrap();
        assert_eq!(cfg.escritorio.as_ref(), Some(&e.a_json()));
        assert!(cfg.cambiado_en_equipo.is_some());
        let otra: Value = serde_json::to_value(&cfg).unwrap();
        assert_eq!(crate::escritorio::Escritorio::de_config(&otra), e);
    }

    /// La tubería (o el socket) de verdad: `hola`, una petición mal autorizada y
    /// un mensaje demasiado grande.
    #[test]
    fn transporte_de_ida_y_vuelta() {
        let dir = std::env::temp_dir().join(format!("resguardo-ipc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let direccion = {
            #[cfg(windows)]
            {
                format!(r"\\.\pipe\ResguardoAgente-prueba-unitaria-{}", std::process::id())
            }
            #[cfg(not(windows))]
            {
                dir.join("ipc.sock").to_string_lossy().into_owned()
            }
        };
        // Sin tocar el disco: un `hola` de verdad (reto) y lo demás, como si el equipo no tuviera clave.
        let servidor = std::sync::Mutex::new(Servidor::default());
        let atiende: Atiende = std::sync::Arc::new(move |pet: &Value| {
            respuesta(match pet["op"].as_str() {
                Some("hola") => Ok(json!({ "reto": servidor.lock().unwrap().retos.nuevo(ahora_ms()) })),
                _ => servidor.lock().unwrap().autorizar(pet, None, ahora_ms()).map(|_| Value::Null),
            })
        });
        let d = direccion.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let r = transporte::escuchar(&d, atiende);
            let _ = tx.send(r);
        });
        // Hasta que esté escuchando.
        let mut c = None;
        for _ in 0..100 {
            if let Ok(x) = transporte::conectar(&direccion) {
                c = Some(x);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let pedir_en = |c: &mut dyn ReadWrite, texto: &str| -> Value {
            c.write_all(texto.as_bytes()).unwrap();
            serde_json::from_slice(&leer_linea(c, MAX_RESPUESTA).unwrap()).unwrap()
        };
        let mut c = c.unwrap_or_else(|| panic!("no conecta: {:?}", rx.try_recv()));
        let r = pedir_en(&mut c, "{\"op\":\"hola\"}\n");
        assert_eq!(r["ok"], true, "{r}");
        assert!(r["datos"]["reto"].as_str().is_some_and(|x| x.len() > 40));
        // Sin prueba: no.
        let mut c = transporte::conectar(&direccion).unwrap();
        let r = pedir_en(&mut c, &format!("{}\n", json!({ "op": "ajustes", "reto": r["datos"]["reto"], "escritorio": { "ventana": "off", "avisos": "off" } })));
        assert_eq!(r["ok"], false);
        // Demasiado grande: no.
        let mut c = transporte::conectar(&direccion).unwrap();
        let grande = format!("{{\"op\":\"hola\",\"x\":\"{}\"}}\n", "a".repeat(MAX_MENSAJE + 10));
        let r = pedir_en(&mut c, &grande);
        assert_eq!(r["error"], "Mensaje demasiado grande.");
        let _ = std::fs::remove_dir_all(&dir);
    }

    trait ReadWrite: std::io::Read + std::io::Write {}
    impl<T: std::io::Read + std::io::Write> ReadWrite for T {}
}
