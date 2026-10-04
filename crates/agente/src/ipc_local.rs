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
//! - **Límites**, por cuenta del equipo (la que da el sistema, no la
//!   petición): 5 fallos seguidos bloquean 1 minuto, el doble cada vez (hasta
//!   1 hora); tras un fallo, como mucho un intento por segundo. Los retos
//!   también son de cada cuenta. Nada de la petición va al registro.
//! - La primera clave (modo sin consola) solo la pone un administrador del
//!   equipo ([`puede_crear_clave`]).

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

/// Retos sin usar a la vez entre todas las cuentas.
const RETOS_TOTAL: usize = 256;

/// Los retos dados y aún sin usar, cada uno de la cuenta que lo pidió: solo
/// ella puede gastarlo, y otra cuenta que pide muchos no echa los suyos.
#[derive(Default)]
pub struct Retos {
    vivos: Vec<(String, i64, String)>,
}

impl Retos {
    /// Uno nuevo (32 bytes aleatorios) para la cuenta `quien`.
    pub fn nuevo(&mut self, ahora_ms: i64, quien: &str) -> String {
        use crypto_box::aead::rand_core::RngCore;
        let mut b = [0u8; 32];
        crypto_box::aead::OsRng.fill_bytes(&mut b);
        let r = B64.encode(b);
        self.vivos.retain(|(_, c, _)| *c > ahora_ms);
        if self.vivos.iter().filter(|(_, _, q)| q == quien).count() >= RETOS_MAX {
            if let Some(i) = self.vivos.iter().position(|(_, _, q)| q == quien) {
                self.vivos.remove(i);
            }
        }
        if self.vivos.len() >= RETOS_TOTAL {
            self.vivos.remove(0);
        }
        self.vivos.push((r.clone(), ahora_ms + RETO_VIDA_S * 1000, quien.to_string()));
        r
    }

    /// Lo gasta: `true` si estaba vivo y es de `quien`. Nunca vale dos veces.
    pub fn gastar(&mut self, reto: &str, ahora_ms: i64, quien: &str) -> bool {
        let Some(i) = self.vivos.iter().position(|(r, _, q)| r == reto && q == quien) else { return false };
        let (_, caduca, _) = self.vivos.remove(i);
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

/// Quién está al otro lado del canal local. Lo dice el sistema (el token del
/// cliente de la tubería o las credenciales del socket), nunca la petición.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cliente {
    /// SID de la cuenta (Windows) o `uid:N` (Unix). Vacío si no se supo.
    pub id: String,
    /// ¿Es SYSTEM o root, o una cuenta del grupo Administradores (también con
    /// UAC, cuando el grupo va «solo para denegar» en su token filtrado)?
    pub admin: bool,
}

/// Cuentas distintas de las que se recuerdan los fallos a la vez (en un equipo
/// hay pocas; se olvidan primero las que no están bloqueadas).
const LIMITADORES_MAX: usize = 64;

/// El estado del servidor entre peticiones (en memoria del servicio).
#[derive(Default)]
pub struct Servidor {
    pub retos: Retos,
    /// Un límite de intentos **por cuenta** del equipo: un usuario que prueba
    /// claves solo se bloquea a sí mismo, no deja sin ventana al administrador.
    limitadores: std::collections::HashMap<String, Limitador>,
}

/// Lo que dice el servidor cuando la prueba no vale (siempre lo mismo: no da pistas).
pub const CLAVE_INCORRECTA: &str = "La clave de administración no es correcta.";

impl Servidor {
    /// El límite de intentos de la cuenta `quien` (vacío: las que no se pudieron identificar, juntas).
    fn limitador(&mut self, quien: &str, ahora_ms: i64) -> &mut Limitador {
        if !self.limitadores.contains_key(quien) && self.limitadores.len() >= LIMITADORES_MAX {
            self.limitadores.retain(|_, l| l.bloqueado_hasta > ahora_ms);
            if self.limitadores.len() >= LIMITADORES_MAX {
                // Todas bloqueadas (no pasa en un equipo de verdad): se olvida la que antes se desbloquea.
                if let Some(k) = self.limitadores.iter().min_by_key(|(_, l)| l.bloqueado_hasta).map(|(k, _)| k.clone()) {
                    self.limitadores.remove(&k);
                }
            }
        }
        self.limitadores.entry(quien.to_string()).or_default()
    }

    /// Tras cambiar la clave: los fallos de esa cuenta se olvidan.
    pub fn olvidar_fallos(&mut self, quien: &str) {
        self.limitadores.remove(quien);
    }

    /// Comprueba el reto y la prueba de una petición contra el verificador.
    /// El reto se gasta siempre (aunque falle lo demás). Los fallos cuentan
    /// para la cuenta `quien` (la del cliente, que da el sistema).
    pub fn autorizar(&mut self, pet: &Value, verificador: Option<&[u8]>, ahora_ms: i64, quien: &str) -> Result<(), String> {
        let reto = pet["reto"].as_str().unwrap_or("");
        if !self.retos.gastar(reto, ahora_ms, quien) {
            return Err("La petición caducó o ya se usó: vuelve a intentarlo.".into());
        }
        self.limitador(quien, ahora_ms).puede(ahora_ms)?;
        let Some(ver) = verificador else { return Err("Este equipo aún no tiene clave de administración.".into()) };
        let prueba = pet["prueba"].as_str().and_then(|p| B64.decode(p).ok()).unwrap_or_default();
        // Tiempo constante (y la prueba vacía o de otro tamaño, igual que una mala).
        if prueba.len() == 32 && resguardo_protocolo::derivaciones::comprueba_prueba(&prueba, ver) {
            self.limitador(quien, ahora_ms).acierto();
            Ok(())
        } else {
            let n = self.limitador(quien, ahora_ms).fallo(ahora_ms);
            let cuenta = if quien.is_empty() { "una cuenta sin identificar".to_string() } else { format!("la cuenta {quien}") };
            crate::agent::log(&format!(
                "Ventana del equipo: clave de administración incorrecta desde {cuenta}{}.",
                if n == 0 { " (bloqueada un rato)" } else { "" }
            ));
            Err(CLAVE_INCORRECTA.into())
        }
    }
}

/// ¿Puede `cliente` poner la clave de administración por primera vez (modo sin
/// consola)? Solo SYSTEM/root o un administrador del equipo: si no, el primer
/// usuario sin privilegios que abriera la ventana en un equipo recién
/// instalado se quedaría con la clave y, con ella, con lo que hace el servicio
/// como SYSTEM (qué se copia y dónde, restaurar…). En pruebas (compilación de
/// desarrollo con `RESGUARDO_AGENT_DIR`), cualquiera.
pub fn puede_crear_clave(cliente: &Cliente, pruebas: bool) -> Result<(), String> {
    if cliente.admin || pruebas {
        return Ok(());
    }
    Err("Solo un administrador de este equipo puede ponerle la clave de administración por primera vez. \
         Pide a quien lo administra que abra Resguardo con su cuenta (no hace falta «Ejecutar como administrador»)."
        .into())
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
pub fn atender(servidor: &std::sync::Mutex<Servidor>, pet: &Value, cliente: &Cliente) -> Value {
    respuesta(atender_(servidor, pet, cliente))
}

/// Las operaciones que cambian y guardan el vínculo: se hacen bajo el cerrojo
/// de las consolas, leyendo el vínculo ya dentro. Si no, una orden de una
/// consola que llegase mientras tanto (su `seq`, sus `nonce`, una clave nueva,
/// una consola quitada…) se perdería al guardar una copia vieja.
fn cambia_vinculo(op: &str) -> bool {
    matches!(op, "crear_clave" | "ajustes" | "crear_repositorio" | "config" | "retencion" | "copia_externa" | "cambiar_clave")
}

fn respuesta(r: Result<Value, String>) -> Value {
    match r {
        Ok(v) => json!({ "ok": true, "datos": v }),
        Err(e) => json!({ "ok": false, "error": e }),
    }
}

/// Quien atiende las peticiones (el servicio: [`atender`]; en las pruebas, otro),
/// con quién pide (lo dice el sistema).
pub type Atiende = std::sync::Arc<dyn Fn(&Value, &Cliente) -> Value + Send + Sync>;

fn ahora_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn atender_(servidor: &std::sync::Mutex<Servidor>, pet: &Value, cliente: &Cliente) -> Result<Value, String> {
    use crate::servidor_v2 as s;
    let op = pet["op"].as_str().unwrap_or("");
    // Lo que cambia el vínculo, bajo el cerrojo de las consolas desde antes de leerlo.
    let _cerrojo = cambia_vinculo(op).then(crate::consolas_v2::cerrojo);
    let web = crate::endpoint::load().is_some_and(|e| !e.stopped);
    let v = s::cargar();
    if op == "hola" {
        let reto = servidor.lock().map_err(|_| "Ocupado.")?.retos.nuevo(ahora_ms(), &cliente.id);
        return Ok(json!({
            "v": 1,
            "reto": reto,
            "modo": modo(v.as_ref(), web),
            "sal_equipo": v.as_ref().filter(|v| v.verificador.is_some()).map(|v| v.sal_equipo.clone()),
        }));
    }
    if op == "crear_clave" {
        // Solo un equipo sin clave, sin vincular y sin la consola web, y solo un
        // administrador del equipo (nunca el primer usuario que abra la ventana).
        {
            let mut sv = servidor.lock().map_err(|_| "Ocupado.")?;
            if !sv.retos.gastar(pet["reto"].as_str().unwrap_or(""), ahora_ms(), &cliente.id) {
                return Err("La petición caducó o ya se usó: vuelve a intentarlo.".into());
            }
            sv.limitador(&cliente.id, ahora_ms()).puede(ahora_ms())?;
        }
        if let Err(e) = puede_crear_clave(cliente, crate::agent::test_mode()) {
            crate::agent::log(&format!("Ventana del equipo: la cuenta {} (sin ser administradora) intentó poner la clave de administración.", cliente.id));
            return Err(e);
        }
        if v.is_some() || web {
            return Err("Este equipo ya tiene clave de administración o está vinculado a una consola.".into());
        }
        let nuevo = vinculo_local(pet)?;
        s::guardar(&nuevo)?;
        crate::agent::log(&format!(
            "Ventana del equipo: clave de administración puesta por la cuenta {}; el equipo se usa sin consola (modo local).",
            cliente.id
        ));
        crate::agente::refrescar_bandeja();
        return Ok(json!({ "modo": "local" }));
    }
    if sin_prueba(op) {
        return Err("Operación no válida.".into());
    }
    let verificador = v.as_ref().and_then(|v| v.verificador.as_deref()).and_then(|x| B64.decode(x).ok());
    servidor.lock().map_err(|_| "Ocupado.")?.autorizar(pet, verificador.as_deref(), ahora_ms(), &cliente.id)?;
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
            // La carpeta «Restaurado …» se crea solo para SYSTEM, Administradores y
            // quien la pide: nadie más puede cambiar nada dentro mientras restic escribe.
            let m = crate::sesiones_v2::restaurar(&acc, &c, (!cliente.id.is_empty()).then_some(cliente.id.as_str()))?;
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
        "historial" => Ok(historial()),
        "kit" => Ok(kit(&v)),
        "cambiar_clave" => {
            let nuevo = vinculo_local(pet)?;
            v.sal_equipo = nuevo.sal_equipo;
            v.verificador = nuevo.verificador;
            v.k_cfg = nuevo.k_cfg;
            s::guardar(&v)?;
            if let Ok(mut sv) = servidor.lock() {
                sv.olvidar_fallos(&cliente.id);
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
/// Quién es el cliente se pregunta al sistema después de leer la petición
/// (Windows solo deja identificar al cliente de una tubería cuando ya escribió).
fn servir<C: std::io::Read + std::io::Write>(atiende: &Atiende, conexion: &mut C, identificar: impl FnOnce(&C) -> Cliente) {
    let respuesta = match leer_linea(conexion, MAX_MENSAJE).and_then(|l| serde_json::from_slice::<Value>(&l).map_err(|_| "Petición no válida.".to_string())) {
        Ok(pet) => atiende(&pet, &identificar(conexion)),
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
        let atiende: Atiende = std::sync::Arc::new(move |pet: &Value, cliente: &Cliente| atender(&servidor, pet, cliente));
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
        GetTokenInformation, IsWellKnownSid, RevertToSelf, TokenGroups, TokenUser, WinBuiltinAdministratorsSid, WinLocalSystemSid, OWNER_SECURITY_INFORMATION,
        PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_GROUPS, TOKEN_QUERY, TOKEN_USER,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_FIRST_PIPE_INSTANCE, OPEN_EXISTING, PIPE_ACCESS_DUPLEX, SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT,
    };
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, ImpersonateNamedPipeClient, PeekNamedPipe, WaitNamedPipeW, PIPE_READMODE_BYTE,
        PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentThread, OpenThreadToken};

    /// Atributos de un grupo en un token (winnt.h).
    const SE_GROUP_ENABLED: u32 = 0x4;
    const SE_GROUP_USE_FOR_DENY_ONLY: u32 = 0x10;

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

    /// Una conexión con un plazo **total** para leer (en el servicio, la
    /// petición entera: un cliente que no dice nada, o que manda un byte de vez
    /// en cuando, no deja el hilo ocupado para siempre).
    pub struct Conexion {
        archivo: File,
        h: HANDLE,
        limite: std::time::Instant,
    }

    impl std::io::Read for Conexion {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let limite = self.limite;
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
                    let limite = std::time::Instant::now() + std::time::Duration::from_millis(ESPERA_PETICION_MS);
                    let mut c = Conexion { archivo: unsafe { File::from_raw_handle(h as _) }, h, limite };
                    servir(&atiende, &mut c, |c| identificar(c.h));
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

    /// Quién es el cliente de la tubería: su token (a nivel de identificación,
    /// que es lo que el cliente permite) dice su SID y si es administrador.
    /// Si algo falla, un cliente sin identificar y sin privilegios.
    pub fn identificar(h: HANDLE) -> Cliente {
        // SAFETY: handle de una tubería conectada de la que ya se leyó; el token
        // se abre como el servicio (OpenAsSelf) y se cierra; se deja de suplantar
        // antes de nada más.
        unsafe {
            if ImpersonateNamedPipeClient(h) == 0 {
                return Cliente::default();
            }
            let mut token: HANDLE = std::ptr::null_mut();
            let abierto = OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut token) != 0;
            if RevertToSelf() == 0 {
                // Seguir como otro usuario no es una opción.
                std::process::abort();
            }
            if !abierto {
                return Cliente::default();
            }
            let c = cliente_de_token(token);
            CloseHandle(token);
            c
        }
    }

    /// Lo que devuelve GetTokenInformation (alineado para leerlo como su estructura).
    unsafe fn info_token(token: HANDLE, clase: i32) -> Option<Vec<u64>> {
        let mut largo = 0u32;
        GetTokenInformation(token, clase, std::ptr::null_mut(), 0, &mut largo);
        if largo == 0 || largo > 1 << 20 {
            return None;
        }
        let mut buf = vec![0u64; (largo as usize).div_ceil(8)];
        (GetTokenInformation(token, clase, buf.as_mut_ptr() as _, largo, &mut largo) != 0).then_some(buf)
    }

    unsafe fn cliente_de_token(token: HANDLE) -> Cliente {
        use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
        let Some(usuario) = info_token(token, TokenUser) else { return Cliente::default() };
        let sid = (*(usuario.as_ptr() as *const TOKEN_USER)).User.Sid;
        let mut texto: windows_sys::core::PWSTR = std::ptr::null_mut();
        let id = if ConvertSidToStringSidW(sid, &mut texto) != 0 && !texto.is_null() {
            let largo = (0..).take_while(|&i| *texto.add(i) != 0).count();
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(texto, largo));
            LocalFree(texto as _);
            s
        } else {
            String::new()
        };
        let mut admin = IsWellKnownSid(sid, WinLocalSystemSid) != 0;
        if !admin {
            if let Some(grupos) = info_token(token, TokenGroups) {
                // Punteros sin referencias intermedias: la lista sigue más allá del primer elemento declarado.
                let g = grupos.as_ptr() as *const TOKEN_GROUPS;
                let n = ((*g).GroupCount as usize).min(grupos.len() * 8 / std::mem::size_of::<windows_sys::Win32::Security::SID_AND_ATTRIBUTES>());
                let lista = std::slice::from_raw_parts(std::ptr::addr_of!((*g).Groups) as *const windows_sys::Win32::Security::SID_AND_ATTRIBUTES, n);
                // Administradores, activo (elevado o sin UAC) o «solo para denegar» (el
                // token filtrado de UAC de un administrador: la ventana nunca va elevada).
                admin = lista
                    .iter()
                    .any(|x| IsWellKnownSid(x.Sid, WinBuiltinAdministratorsSid) != 0 && x.Attributes & (SE_GROUP_ENABLED | SE_GROUP_USE_FOR_DENY_ONLY) != 0);
            }
        }
        Cliente { id, admin }
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
                let limite = std::time::Instant::now() + std::time::Duration::from_secs(24 * 3600);
                return Ok(Conexion { archivo: unsafe { File::from_raw_handle(h as _) }, h, limite });
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
    use std::io::{Read as _, Write as _};
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::os::unix::io::AsRawFd;
    use std::os::unix::net::{UnixListener, UnixStream};

    /// Un socket con un plazo **total** para leer la petición (no por lectura:
    /// un cliente que manda un byte de vez en cuando no retiene el hilo).
    pub struct Conexion {
        s: UnixStream,
        limite: std::time::Instant,
    }

    impl std::io::Read for Conexion {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let queda = self.limite.saturating_duration_since(std::time::Instant::now());
            if queda.is_zero() {
                return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "sin petición"));
            }
            self.s.set_read_timeout(Some(queda))?;
            self.s.read(buf)
        }
    }

    impl std::io::Write for Conexion {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.s.write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.s.flush()
        }
    }

    /// Quién es el cliente: el uid que da el núcleo (SO_PEERCRED / getpeereid).
    fn identificar(s: &UnixStream) -> Cliente {
        let fd = s.as_raw_fd();
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let uid = {
            let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
            let mut largo = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
            // SAFETY: fd abierto; estructura de salida local con su tamaño.
            let r = unsafe { libc::getsockopt(fd, libc::SOL_SOCKET, libc::SO_PEERCRED, &mut cred as *mut _ as *mut libc::c_void, &mut largo) };
            (r == 0).then_some(cred.uid)
        };
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        let uid = {
            let (mut uid, mut gid) = (0 as libc::uid_t, 0 as libc::gid_t);
            // SAFETY: fd abierto; salidas locales.
            (unsafe { libc::getpeereid(fd, &mut uid, &mut gid) } == 0).then_some(uid)
        };
        match uid {
            Some(u) => Cliente { id: format!("uid:{u}"), admin: u == 0 },
            None => Cliente::default(),
        }
    }

    pub fn escuchar(direccion: &str, atiende: Atiende) -> Result<(), String> {
        let ruta = std::path::Path::new(direccion);
        if let Some(dir) = ruta.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            if !cfg!(test) && !crate::agent::test_mode() {
                // La carpeta, de root y sin escritura para nadie más (nadie puede
                // cambiar el socket por otro entre que se crea y se usa).
                let m = std::fs::symlink_metadata(dir).map_err(|e| e.to_string())?;
                if !m.is_dir() || m.uid() != 0 {
                    return Err(format!("{} no es una carpeta de root", dir.display()));
                }
                std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
            }
        }
        let _ = std::fs::remove_file(ruta);
        let l = UnixListener::bind(ruta).map_err(|e| e.to_string())?;
        // Cualquier usuario local puede hablar; lo que autoriza es la prueba.
        let _ = std::fs::set_permissions(ruta, std::fs::Permissions::from_mode(0o666));
        let activas = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        for c in l.incoming().flatten() {
            // Como mucho 4 a la vez (como en Windows): un usuario no agota los hilos del servicio.
            if activas.load(std::sync::atomic::Ordering::SeqCst) >= 4 {
                drop(c);
                continue;
            }
            activas.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let (atiende, activas) = (atiende.clone(), activas.clone());
            std::thread::spawn(move || {
                let limite = std::time::Instant::now() + std::time::Duration::from_millis(ESPERA_PETICION_MS);
                let mut c = Conexion { s: c, limite };
                servir(&atiende, &mut c, |c| identificar(&c.s));
                drop(c);
                activas.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            });
        }
        Ok(())
    }

    pub fn conectar(direccion: &str) -> Result<Conexion, String> {
        // El socket tiene que ser de root (si no, lo puso otro usuario).
        let dueno = std::fs::metadata(direccion).map(|m| m.uid()).map_err(|_| "El servicio de Resguardo no responde (¿está en marcha?).".to_string())?;
        if dueno != 0 && !cfg!(test) && !crate::agent::test_mode() {
            return Err("El canal local no es del servicio de Resguardo: no se usa.".into());
        }
        let s = UnixStream::connect(direccion).map_err(|_| "El servicio de Resguardo no responde (¿está en marcha?).".to_string())?;
        Ok(Conexion { s, limite: std::time::Instant::now() + std::time::Duration::from_secs(24 * 3600) })
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
        let reto = s.retos.nuevo(t, "S-1-5-21-1");
        let bien = json!({ "reto": reto, "prueba": prueba(CLAVE, &sal).unwrap() });
        assert!(s.autorizar(&bien, Some(&ver), t, "S-1-5-21-1").is_ok());
        // La mala, con su reto: no, y sin decir más.
        let reto = s.retos.nuevo(t + 2_000, "S-1-5-21-1");
        let mal = json!({ "reto": reto, "prueba": prueba("otra clave cualquiera", &sal).unwrap() });
        assert_eq!(s.autorizar(&mal, Some(&ver), t + 2_000, "S-1-5-21-1").unwrap_err(), CLAVE_INCORRECTA);
        // Sin prueba, o con basura: igual que una mala.
        let reto = s.retos.nuevo(t + 4_000, "S-1-5-21-1");
        assert_eq!(s.autorizar(&json!({ "reto": reto }), Some(&ver), t + 4_000, "S-1-5-21-1").unwrap_err(), CLAVE_INCORRECTA);
        let reto = s.retos.nuevo(t + 6_000, "S-1-5-21-1");
        assert_eq!(s.autorizar(&json!({ "reto": reto, "prueba": "!!" }), Some(&ver), t + 6_000, "S-1-5-21-1").unwrap_err(), CLAVE_INCORRECTA);
        // Un equipo sin clave no acepta nada.
        let reto = s.retos.nuevo(t + 8_000, "S-1-5-21-1");
        assert!(s.autorizar(&json!({ "reto": reto, "prueba": prueba(CLAVE, &sal).unwrap() }), None, t + 8_000, "S-1-5-21-1").is_err());
    }

    #[test]
    fn sin_repeticion_y_con_caducidad() {
        let sal = B64.encode([1u8; 16]);
        let ver = verificador_de(CLAVE, &sal);
        let mut s = Servidor::default();
        let t = 5_000_000;
        let reto = s.retos.nuevo(t, "S-1-5-21-1");
        let pet = json!({ "reto": reto, "prueba": prueba(CLAVE, &sal).unwrap() });
        assert!(s.autorizar(&pet, Some(&ver), t, "S-1-5-21-1").is_ok());
        // La misma petición otra vez (repetida por quien la vio): no.
        assert!(s.autorizar(&pet, Some(&ver), t + 5_000, "S-1-5-21-1").unwrap_err().contains("ya se usó"));
        // Un reto inventado: no.
        assert!(s.autorizar(&json!({ "reto": B64.encode([9u8; 32]), "prueba": pet["prueba"] }), Some(&ver), t + 7_000, "S-1-5-21-1").is_err());
        // Un reto caducado: no (y se gasta).
        let reto = s.retos.nuevo(t + 10_000, "S-1-5-21-1");
        let tarde = t + 10_000 + RETO_VIDA_S * 1000 + 1;
        let pet = json!({ "reto": reto, "prueba": prueba(CLAVE, &sal).unwrap() });
        assert!(s.autorizar(&pet, Some(&ver), tarde, "S-1-5-21-1").is_err());
        assert!(s.autorizar(&pet, Some(&ver), tarde + 2_000, "S-1-5-21-1").is_err());
        // Un reto gastado por una prueba mala tampoco sirve para la buena.
        let reto = s.retos.nuevo(t + 20_000, "S-1-5-21-1");
        assert!(s.autorizar(&json!({ "reto": reto, "prueba": B64.encode([0u8; 32]) }), Some(&ver), t + 20_000, "S-1-5-21-1").is_err());
        assert!(s.autorizar(&json!({ "reto": reto, "prueba": prueba(CLAVE, &sal).unwrap() }), Some(&ver), t + 22_000, "S-1-5-21-1").is_err());
        // No se acumulan retos sin fin.
        for i in 0..100 {
            s.retos.nuevo(t + 30_000 + i, "S-1-5-21-1");
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
            let reto = s.retos.nuevo(t, "S-1-5-21-1");
            assert!(s.autorizar(&json!({ "reto": reto, "prueba": mala }), Some(&ver), t, "S-1-5-21-1").is_err());
        }
        t += 1_000;
        let reto = s.retos.nuevo(t, "S-1-5-21-1");
        let e = s.autorizar(&json!({ "reto": reto, "prueba": prueba(CLAVE, &sal).unwrap() }), Some(&ver), t, "S-1-5-21-1").unwrap_err();
        assert!(e.contains("Demasiados intentos"), "{e}");
        t += BLOQUEO_MS;
        let reto = s.retos.nuevo(t, "S-1-5-21-1");
        assert!(s.autorizar(&json!({ "reto": reto, "prueba": prueba(CLAVE, &sal).unwrap() }), Some(&ver), t, "S-1-5-21-1").is_ok());
    }

    /// Un usuario que prueba claves solo se bloquea a sí mismo: el
    /// administrador (otra cuenta) sigue pudiendo entrar.
    #[test]
    fn el_bloqueo_es_de_cada_cuenta() {
        let sal = B64.encode([4u8; 16]);
        let ver = verificador_de(CLAVE, &sal);
        let mut s = Servidor::default();
        let mut t = 20_000_000;
        let mala = prueba("no es la clave", &sal).unwrap();
        for _ in 0..FALLOS_MAX * 3 {
            t += 1_000;
            let reto = s.retos.nuevo(t, "S-1-5-21-1001");
            assert!(s.autorizar(&json!({ "reto": reto, "prueba": mala }), Some(&ver), t, "S-1-5-21-1001").is_err());
        }
        // El intruso, bloqueado.
        t += 1_000;
        let reto = s.retos.nuevo(t, "S-1-5-21-1001");
        let e = s.autorizar(&json!({ "reto": reto, "prueba": prueba(CLAVE, &sal).unwrap() }), Some(&ver), t, "S-1-5-21-1001").unwrap_err();
        assert!(e.contains("Demasiados intentos"), "{e}");
        // El administrador, no (ni siquiera espera el segundo entre intentos del otro).
        let reto = s.retos.nuevo(t, "S-1-5-21-500");
        assert!(s.autorizar(&json!({ "reto": reto, "prueba": prueba(CLAVE, &sal).unwrap() }), Some(&ver), t, "S-1-5-21-500").is_ok());
        // Muchas cuentas distintas no hacen crecer la memoria sin fin.
        for i in 0..(LIMITADORES_MAX as i64 * 3) {
            let reto = s.retos.nuevo(t + i, &format!("S-1-5-21-{i}"));
            let _ = s.autorizar(&json!({ "reto": reto, "prueba": mala }), Some(&ver), t + i, &format!("S-1-5-21-{i}"));
        }
        assert!(s.limitadores.len() <= LIMITADORES_MAX);
        // Cambiar la clave olvida los fallos de esa cuenta.
        s.olvidar_fallos("S-1-5-21-1001");
        let reto = s.retos.nuevo(t + 10_000, "S-1-5-21-1001");
        assert!(s.autorizar(&json!({ "reto": reto, "prueba": prueba(CLAVE, &sal).unwrap() }), Some(&ver), t + 10_000, "S-1-5-21-1001").is_ok());
    }

    /// Un reto solo vale para la cuenta que lo pidió, y otra cuenta que pide
    /// muchos no echa los de las demás.
    #[test]
    fn retos_de_cada_cuenta() {
        let sal = B64.encode([6u8; 16]);
        let ver = verificador_de(CLAVE, &sal);
        let mut s = Servidor::default();
        let t = 30_000_000;
        let del_admin = s.retos.nuevo(t, "S-1-5-21-500");
        for i in 0..1000 {
            s.retos.nuevo(t + i, "S-1-5-21-1001");
        }
        assert!(s.retos.vivos.len() <= RETOS_TOTAL);
        assert!(s.retos.vivos.iter().filter(|x| x.2 == "S-1-5-21-1001").count() <= RETOS_MAX);
        let bien = prueba(CLAVE, &sal).unwrap();
        // Otra cuenta no puede usar el reto del administrador…
        let otro = s.retos.nuevo(t + 2_000, "S-1-5-21-1001");
        assert!(s.autorizar(&json!({ "reto": del_admin, "prueba": bien }), Some(&ver), t + 2_000, "S-1-5-21-1001").unwrap_err().contains("ya se usó"));
        // …ni el administrador el de otra; el suyo sigue vivo.
        assert!(s.autorizar(&json!({ "reto": otro, "prueba": bien }), Some(&ver), t + 2_000, "S-1-5-21-500").is_err());
        assert!(s.autorizar(&json!({ "reto": del_admin, "prueba": bien }), Some(&ver), t + 2_000, "S-1-5-21-500").is_ok());
    }

    /// Lo que guarda el vínculo se hace bajo el cerrojo de las consolas.
    #[test]
    fn lo_que_cambia_el_vinculo_va_con_cerrojo() {
        for op in ["crear_clave", "ajustes", "crear_repositorio", "config", "retencion", "copia_externa", "cambiar_clave"] {
            assert!(cambia_vinculo(op), "{op}");
        }
        // Lo largo (restaurar, explorar) no retiene las órdenes de las consolas.
        for op in ["hola", "restaurar", "explorar", "carpetas", "estado_local", "historial"] {
            assert!(!cambia_vinculo(op), "{op}");
        }
    }

    /// La primera clave (modo sin consola) solo la pone un administrador del equipo.
    #[test]
    fn solo_un_administrador_pone_la_primera_clave() {
        let usuario = Cliente { id: "S-1-5-21-1001".into(), admin: false };
        let admin = Cliente { id: "S-1-5-21-500".into(), admin: true };
        assert!(puede_crear_clave(&usuario, false).unwrap_err().contains("Solo un administrador"));
        assert!(puede_crear_clave(&Cliente::default(), false).is_err(), "sin identificar: no");
        assert!(puede_crear_clave(&admin, false).is_ok());
        assert!(puede_crear_clave(&usuario, true).is_ok(), "en pruebas, cualquiera");
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
        let atiende: Atiende = std::sync::Arc::new(move |pet: &Value, cliente: &Cliente| {
            respuesta(match pet["op"].as_str() {
                // Quién dice el sistema que es el cliente (esta misma prueba).
                Some("quien") => Ok(json!({ "id": cliente.id, "admin": cliente.admin })),
                Some("hola") => Ok(json!({ "reto": servidor.lock().unwrap().retos.nuevo(ahora_ms(), &cliente.id) })),
                _ => servidor.lock().unwrap().autorizar(pet, None, ahora_ms(), &cliente.id).map(|_| Value::Null),
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
        // El servicio sabe quién pide (lo dice el sistema, no la petición): esta cuenta.
        let mut c = transporte::conectar(&direccion).unwrap();
        let r = pedir_en(
            &mut c,
            "{\"op\":\"quien\",\"id\":\"S-1-5-18\"}
",
        );
        let id = r["datos"]["id"].as_str().unwrap_or("").to_string();
        #[cfg(windows)]
        assert!(id.starts_with("S-1-5-"), "{r}");
        #[cfg(unix)]
        {
            assert_eq!(id, format!("uid:{}", unsafe { libc::geteuid() }), "{r}");
            assert_eq!(r["datos"]["admin"], unsafe { libc::geteuid() } == 0);
        }
        // Una petición que llega a trozos, más despacio que el plazo total: se corta.
        use std::io::Write;
        let mut c = transporte::conectar(&direccion).unwrap();
        let inicio = std::time::Instant::now();
        let mut cortada = false;
        while inicio.elapsed() < std::time::Duration::from_millis(ESPERA_PETICION_MS + 5_000) {
            if c.write_all(b" ").is_err() {
                cortada = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        assert!(
            cortada || leer_linea(&mut c, MAX_RESPUESTA).is_ok_and(|l| String::from_utf8_lossy(&l).contains("ok")),
            "una conexión lenta no se queda para siempre"
        );
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
