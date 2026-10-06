//! Varias consolas a la vez (docs/consolas-multiples.md): el mismo equipo
//! vinculado a varios Resguardo Server, cada uno por su lado.
//!
//! El vínculo de siempre (`Vinculo`) es «la consola principal»; las demás van
//! en `Vinculo::otras`, con los mismos campos de vínculo (`Enlace`). Para
//! trabajar con una de ellas se intercambia con la principal (`activar`), se
//! usa el código de siempre y se vuelve a poner en su sitio, todo bajo un
//! cerrojo del proceso y leyendo y guardando el disco (`con_enlace`).
//!
//! Lo que es de cada consola: dirección, autoridad TLS e identidad fijadas,
//! credenciales en ese servidor, `seq`, `K_cfg` (depende de la sal del cliente
//! en ese servidor), bloqueos por intentos, cambio de servidor en curso,
//! servidores de respaldo y último contacto. Lo demás es del equipo: claves,
//! verificador, espera, repositorios, configuración y los `nonce` ya vistos.

use crate::servidor_v2::{self as s, Vinculo};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
/// Consolas que puede tener un equipo a la vez (la principal incluida).
pub const MAX_CONSOLAS: usize = 5;
/// El id del vínculo principal en un archivo anterior (sin `enlace_id`).
pub const PRINCIPAL: &str = "principal";

/// Otra consola que gestiona este equipo: los campos de vínculo de `Vinculo`.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Enlace {
    /// Id interno (estable mientras exista el vínculo).
    pub id: String,
    /// Cómo se llama esa consola (para enseñarlo); vacío: su dirección.
    #[serde(default)]
    pub nombre: String,
    pub url: String,
    pub ca_pem: String,
    pub identidad: String,
    pub cliente_id: String,
    pub equipo_id: String,
    pub secreto: String,
    #[serde(default)]
    pub codigo: Option<String>,
    #[serde(default)]
    pub k_cfg: Option<String>,
    /// La sal del cliente en ese servidor, si se sabe (para la `K_cfg` al cambiar la clave).
    #[serde(default)]
    pub sal_cliente: Option<String>,
    #[serde(default)]
    pub ultimo_seq: u64,
    #[serde(default)]
    pub config_seq: u64,
    #[serde(default)]
    pub fallos: HashMap<String, s::Fallos>,
    #[serde(default)]
    pub cambio: Option<crate::traslado_v2::Cambio>,
    #[serde(default)]
    pub respaldo: Vec<crate::traslado_v2::Servidor>,
    #[serde(default)]
    pub respaldo_dias: i64,
    #[serde(default)]
    pub ultimo_ok: i64,
    /// Desde cuándo gestiona el equipo (Unix).
    #[serde(default)]
    pub desde: i64,
    /// Hay que subirle la configuración (la cambió otra consola).
    #[serde(default)]
    pub config_pendiente: bool,
}

/// El último cambio de configuración y desde qué consola (para «Cambiado desde otra consola»).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct UltimoCambio {
    pub tipo: String,
    /// RFC 3339.
    pub cuando: String,
    /// Id interno del vínculo por el que llegó.
    pub enlace: String,
    pub nombre: String,
    pub url: String,
    pub identidad: String,
}

/// Órdenes que cambian lo que el equipo hace o tiene (se anota de qué consola vinieron).
pub const CAMBIAN_CONFIG: &[&str] = &[
    "config",
    "crear_repositorio",
    "adoptar_repositorio",
    "importar_repositorio",
    "cambiar_destino",
    "cambiar_retencion",
    "cambiar_copia_externa",
    "cambiar_derivada",
    "quitar_derivada",
    "pausar",
    "reanudar",
    "dejar_de_copiar",
    "quitar_repositorio",
    "cambiar_espera",
    "cambiar_clave_admin",
    "guarda_copias",
    "retencion_almacen",
    "conectar_nube",
    "quitar_nube",
    "anadir_consola",
    "quitar_consola",
];

/// El nombre que se enseña de una consola: el suyo o el nombre de su dirección.
pub fn nombre_de(nombre: &str, url: &str) -> String {
    if !nombre.trim().is_empty() {
        return nombre.trim().to_string();
    }
    let sin = url.trim_start_matches("https://").trim_start_matches("http://");
    sin.split(['/', '?', '#']).next().unwrap_or(sin).to_string()
}

/// Huella corta de una identidad (los 8 primeros bytes, `AB:CD:…`), como en la consola.
pub fn huella_corta(identidad: &str) -> String {
    B64.decode(identidad).map(|b| b.iter().take(8).map(|x| format!("{x:02X}")).collect::<Vec<_>>().join(":")).unwrap_or_else(|_| "?".into())
}

impl Vinculo {
    /// El id del vínculo que está en los campos de siempre.
    pub fn id_enlace(&self) -> String {
        if self.enlace_id.is_empty() {
            PRINCIPAL.into()
        } else {
            self.enlace_id.clone()
        }
    }

    /// Los ids de todos los vínculos (el principal primero).
    pub fn ids_enlaces(&self) -> Vec<String> {
        std::iter::once(self.id_enlace()).chain(self.otras.iter().map(|e| e.id.clone())).collect()
    }

    /// El vínculo de los campos de siempre, como `Enlace` (copia).
    pub fn enlace(&self) -> Enlace {
        Enlace {
            id: self.id_enlace(),
            nombre: self.nombre_consola.clone(),
            url: self.url.clone(),
            ca_pem: self.ca_pem.clone(),
            identidad: self.identidad.clone(),
            cliente_id: self.cliente_id.clone(),
            equipo_id: self.equipo_id.clone(),
            secreto: self.secreto.clone(),
            codigo: self.codigo.clone(),
            k_cfg: self.k_cfg.clone(),
            sal_cliente: self.sal_cliente.clone(),
            ultimo_seq: self.ultimo_seq,
            config_seq: self.config_seq,
            fallos: self.fallos.clone(),
            cambio: self.cambio.clone(),
            respaldo: self.respaldo.clone(),
            respaldo_dias: self.respaldo_dias,
            ultimo_ok: self.ultimo_ok,
            desde: self.desde,
            config_pendiente: self.config_pendiente,
        }
    }

    /// Pone `e` en los campos de siempre (lo que hubiera se pierde).
    pub fn poner_enlace(&mut self, e: Enlace) {
        self.enlace_id = e.id;
        self.nombre_consola = e.nombre;
        self.url = e.url;
        self.ca_pem = e.ca_pem;
        self.identidad = e.identidad;
        self.cliente_id = e.cliente_id;
        self.equipo_id = e.equipo_id;
        self.secreto = e.secreto;
        self.codigo = e.codigo;
        self.k_cfg = e.k_cfg;
        self.sal_cliente = e.sal_cliente;
        self.ultimo_seq = e.ultimo_seq;
        self.config_seq = e.config_seq;
        self.fallos = e.fallos;
        self.cambio = e.cambio;
        self.respaldo = e.respaldo;
        self.respaldo_dias = e.respaldo_dias;
        self.ultimo_ok = e.ultimo_ok;
        self.desde = e.desde;
        self.config_pendiente = e.config_pendiente;
    }

    /// Intercambia el vínculo de los campos de siempre con `otras[i]`.
    pub fn intercambiar(&mut self, i: usize) {
        let actual = self.enlace();
        let otra = std::mem::replace(&mut self.otras[i], actual);
        self.poner_enlace(otra);
    }

    /// Pone el vínculo `id` en los campos de siempre (el que estaba pasa a `otras`).
    /// `false` si no existe.
    pub fn activar(&mut self, id: &str) -> bool {
        if self.id_enlace() == id {
            return true;
        }
        match self.otras.iter().position(|e| e.id == id) {
            Some(i) => {
                self.intercambiar(i);
                true
            }
            None => false,
        }
    }

    /// ¿Algún vínculo (también el de los campos de siempre) con esta identidad?
    pub fn tiene_identidad(&self, identidad: &str) -> bool {
        (!self.secreto.is_empty() && self.identidad == identidad) || self.otras.iter().any(|e| e.identidad == identidad)
    }

    /// Tras una orden: fuera los vínculos que se quedaron sin credenciales
    /// (desvinculados o quitados) y, si fue el de los campos de siempre y quedan
    /// otros, el siguiente pasa a ocupar su sitio y el equipo sigue gestionado.
    pub fn limpiar(&mut self) {
        self.otras.retain(|e| !e.secreto.is_empty());
        if self.secreto.is_empty() && !self.otras.is_empty() {
            let e = self.otras.remove(0);
            self.poner_enlace(e);
            self.modo = "gestionado".into();
        }
    }

    /// Todos los vínculos con credenciales, como `Vinculo` listos para llamar a su servidor.
    pub fn vistas(&self) -> Vec<Vinculo> {
        self.ids_enlaces()
            .into_iter()
            .filter_map(|id| {
                let mut w = self.clone();
                (w.activar(&id) && !w.secreto.is_empty()).then_some(w)
            })
            .collect()
    }
}

/// Un solo hilo del proceso cambia el vínculo a la vez (órdenes de dos consolas a la vez).
static CERROJO: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// También lo toma `ipc_local` (la ventana del equipo) para leer, cambiar y
/// guardar el vínculo sin pisar lo que haga a la vez una orden de una consola.
pub(crate) fn cerrojo() -> std::sync::MutexGuard<'static, ()> {
    CERROJO.lock().unwrap_or_else(|e| e.into_inner())
}

/// Lee el vínculo, pone el `id` en los campos de siempre, hace `f`, lo vuelve a
/// su sitio, limpia y guarda. `None` si ese vínculo ya no existe.
pub fn con_enlace<R>(id: &str, f: impl FnOnce(&mut Vinculo) -> R) -> Option<R> {
    let _g = cerrojo();
    let mut v = s::cargar()?;
    let principal = v.id_enlace();
    if !v.activar(id) {
        return None;
    }
    let r = f(&mut v);
    // Si la principal se quitó con esta orden, la que estaba activa se queda en su sitio.
    v.activar(&principal);
    v.limpiar();
    let _ = s::guardar(&v);
    Some(r)
}

/// Lee, cambia y guarda el vínculo (sin activar ninguno), bajo el cerrojo.
pub fn modificar<R>(f: impl FnOnce(&mut Vinculo) -> R) -> Option<R> {
    let _g = cerrojo();
    let mut v = s::cargar()?;
    let r = f(&mut v);
    v.limpiar();
    let _ = s::guardar(&v);
    Some(r)
}

/// El vínculo `id` (en los campos de siempre), si sigue y está gestionado.
pub fn vista(id: &str) -> Option<Vinculo> {
    let mut v = s::cargar()?;
    (v.activar(id) && v.modo == "gestionado" && !v.secreto.is_empty()).then_some(v)
}

/// Último contacto de cada consola en el resumen, redondeado a 15 min: así el
/// resumen no cambia (ni se vuelve a subir) en cada informe.
fn contacto(ts: i64) -> Option<String> {
    (ts > 0).then(|| chrono::DateTime::from_timestamp(ts - ts.rem_euclid(900), 0).map(|d| d.to_rfc3339())).flatten()
}

/// `resumen.consolas`: las consolas que gestionan el equipo (la que lo recibe, `esta: true`).
pub fn resumen(v: &Vinculo) -> Value {
    let fila = |e: &Enlace, esta: bool| {
        json!({
            "id": e.id, "nombre": nombre_de(&e.nombre, &e.url), "url": e.url, "identidad": e.identidad,
            "sal_cliente": e.sal_cliente, "ultimo_contacto": contacto(e.ultimo_ok),
            "desde": (e.desde > 0).then(|| chrono::DateTime::from_timestamp(e.desde, 0).map(|d| d.to_rfc3339())).flatten(),
            "esta": esta,
        })
    };
    let mut l = vec![fila(&v.enlace(), true)];
    l.extend(v.otras.iter().map(|e| fila(e, false)));
    json!(l)
}

/// `resumen.cambio_config`: el último cambio y desde qué consola.
pub fn resumen_cambio(v: &Vinculo) -> Value {
    match &v.ultimo_cambio {
        Some(c) => json!({ "tipo": c.tipo, "cuando": c.cuando, "consola": { "nombre": nombre_de(&c.nombre, &c.url), "url": c.url, "identidad": c.identidad } }),
        None => Value::Null,
    }
}

/// Tras una orden hecha que cambia algo: de qué consola vino.
pub fn anotar_cambio(v: &mut Vinculo, tipo: &str) {
    if !CAMBIAN_CONFIG.contains(&tipo) {
        return;
    }
    v.ultimo_cambio = Some(UltimoCambio {
        tipo: tipo.into(),
        cuando: chrono::Local::now().to_rfc3339(),
        enlace: v.id_enlace(),
        nombre: v.nombre_consola.clone(),
        url: v.url.clone(),
        identidad: v.identidad.clone(),
    });
}

// ---------- anadir_consola ----------

/// Lo que trae `anadir_consola`, comprobado.
#[derive(Clone, Debug)]
pub struct Pedido {
    pub url: String,
    pub identidad: String,
    pub huella_ca: String,
    pub ficha: String,
    pub sal_cliente: String,
    pub k_cfg: [u8; 32],
    pub nombre: String,
}

fn b64_de(s: &str, min: usize, max: usize) -> bool {
    B64.decode(s).is_ok_and(|b| (min..=max).contains(&b.len()))
}

/// Comprueba el cuerpo de `anadir_consola`.
pub fn pedido(c: &Value) -> Result<Pedido, String> {
    let t = |k: &str| c[k].as_str().unwrap_or("").trim().to_string();
    let url = t("url").trim_end_matches('/').to_string();
    // http:// solo hacia este mismo equipo, con el anfitrión exacto («http://localhost.otro.com»
    // o «http://localhost@otro» no lo son).
    let local = s::es_local(&url);
    if !url.starts_with("https://") && !local {
        return Err("La dirección de la otra consola tiene que ser https://.".into());
    }
    if url.len() > 300 || url.contains(char::is_whitespace) {
        return Err("Dirección de la otra consola no válida.".into());
    }
    let identidad = t("identidad");
    if !b64_de(&identidad, 32, 32) {
        return Err("Identidad de la otra consola no válida.".into());
    }
    let huella_ca = t("huella_ca");
    let cifras = huella_ca.chars().filter(|x| x.is_ascii_hexdigit()).count();
    if url.starts_with("https://") && cifras != 64 {
        return Err("Falta la huella de la autoridad TLS de la otra consola.".into());
    }
    let ficha = t("ficha");
    if !(16..=128).contains(&ficha.len()) {
        return Err("Ficha de alta no válida.".into());
    }
    let sal_cliente = t("sal_cliente");
    if !b64_de(&sal_cliente, 16, 64) {
        return Err("Sal del cliente de la otra consola no válida.".into());
    }
    let k_cfg: [u8; 32] = B64.decode(t("k_cfg")).ok().and_then(|b| b.try_into().ok()).ok_or("Falta la K_cfg de la otra consola.")?;
    let nombre: String = t("nombre").chars().filter(|x| !x.is_control()).take(80).collect();
    Ok(Pedido { url, identidad, huella_ca, ficha, sal_cliente, k_cfg, nombre })
}

/// Antes de empezar: que no sea ya una de sus consolas y que quepa.
pub fn admite(v: &Vinculo, p: &Pedido) -> Result<(), String> {
    if v.verificador.is_none() {
        return Err("Este equipo aún no tiene la clave de administración (falta el alta).".into());
    }
    if v.tiene_identidad(&p.identidad) {
        return Err("Esa consola ya gestiona este equipo.".into());
    }
    if 1 + v.otras.len() >= MAX_CONSOLAS {
        return Err(format!("Este equipo ya tiene {MAX_CONSOLAS} consolas: quita alguna antes."));
    }
    Ok(())
}

/// La autoridad TLS de la otra consola: se descarga (sin comprobarla esta única
/// vez) y tiene que ser UN certificado con la huella de la orden.
fn autoridad(p: &Pedido) -> Result<String, String> {
    if !p.url.starts_with("https://") {
        return Ok(String::new()); // http:// solo en este mismo equipo (pruebas)
    }
    let agente = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(30)))
        .http_status_as_error(false)
        .tls_config(ureq::tls::TlsConfig::builder().provider(ureq::tls::TlsProvider::Rustls).disable_verification(true).build())
        .build()
        .new_agent();
    let mut r = agente.get(&format!("{}/api/servidor/ca", p.url)).call().map_err(|e| s::explicar_error_conexion(&e))?;
    let pem = r.body_mut().read_to_string().unwrap_or_default();
    let real = s::huella_de_una_autoridad(&pem)?;
    if !s::huella_coincide(&p.huella_ca, &real) {
        return Err(format!("La autoridad TLS de {} no es la del código de conexión (tiene {real}): no se conecta.", p.url));
    }
    Ok(pem)
}

/// Se da de alta en la otra consola y comprueba su identidad. Devuelve el vínculo
/// nuevo (sin guardarlo).
pub fn conectar(v: &Vinculo, p: &Pedido) -> Result<Enlace, String> {
    let ca_pem = autoridad(p)?;
    let destino = crate::traslado_v2::Servidor { url: p.url.clone(), identidad: p.identidad.clone(), ca_pem, ficha: p.ficha.clone() };
    let mut base = v.clone();
    base.k_cfg = Some(B64.encode(p.k_cfg));
    let n = crate::traslado_v2::recibir_en(&base, &destino, Some("anadir_consola"))?;
    let ahora = chrono::Utc::now().timestamp();
    Ok(Enlace {
        id: uuid::Uuid::new_v4().to_string(),
        nombre: p.nombre.clone(),
        url: n.url,
        ca_pem: n.ca_pem,
        identidad: n.identidad,
        cliente_id: n.cliente_id,
        equipo_id: n.equipo_id,
        secreto: n.secreto,
        k_cfg: Some(B64.encode(p.k_cfg)),
        sal_cliente: Some(p.sal_cliente.clone()),
        ultimo_ok: ahora,
        desde: ahora,
        config_pendiente: true,
        ..Default::default()
    })
}

/// Avisa a todas las consolas del equipo (y lo deja en su historial).
pub fn avisar_a_todas(mensaje: &str) {
    crate::bitacora::aviso("cambio_inusual", mensaje);
    for w in s::cargar().map(|v| v.vistas()).unwrap_or_default() {
        let _ = s::llamar_ok(&w, "/api/agente/aviso", &json!({ "tipo": "cambio_inusual", "mensaje": mensaje }));
    }
}

/// `anadir_consola`: comprueba y sigue aparte (contesta `en_marcha`; el resultado
/// final, firmado, llega a la consola que la mandó al terminar).
pub fn anadir(v: &mut Vinculo, c: &Value, orden: &str, seq: u64) -> Result<String, String> {
    let p = pedido(c)?;
    admite(v, &p)?;
    // La sal de la consola que la manda (para la K_cfg de todas al cambiar la clave).
    if let Some(sal) = c["sal_origen"].as_str().filter(|x| b64_de(x, 16, 64)) {
        v.sal_cliente.get_or_insert_with(|| sal.to_string());
    }
    let (origen, orden, nombre) = (v.clone(), orden.to_string(), nombre_de(&p.nombre, &p.url));
    let respuesta = format!("Conectando también a {nombre}…");
    std::thread::spawn(move || {
        let r = conectar(&origen, &p).and_then(|e| {
            let resumen = format!("{} ({})", nombre_de(&e.nombre, &e.url), e.url);
            modificar(|v| {
                admite(v, &p)?;
                v.otras.push(e);
                // Todas las consolas ven enseguida la nueva en su resumen (`consolas`).
                v.config_pendiente = true;
                for o in v.otras.iter_mut() {
                    o.config_pendiente = true;
                }
                Ok::<_, String>(())
            })
            .ok_or("El equipo ya no está vinculado.")??;
            Ok(resumen)
        });
        let (estado, mensaje) = match &r {
            Ok(resumen) => {
                crate::agent::log(&format!("Conectado también a la consola {resumen}."));
                avisar_a_todas(&format!(
                    "Este equipo se conectó también a otra consola: {resumen}, identidad {}. Si no lo esperabas, quítala con la clave de administración.",
                    huella_corta(&p.identidad)
                ));
                ("hecha", format!("Conectado también a {resumen}: esa consola ya gestiona este equipo."))
            }
            Err(e) => {
                crate::agent::log(&format!("Conectar también a la consola {nombre}: {e}"));
                ("fallida", format!("No se pudo conectar a {nombre}: {e}"))
            }
        };
        for intento in 0..5 {
            if s::enviar_resultado_texto(&origen, &orden, seq, estado, &mensaje, None).is_ok() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(10 << intento));
        }
    });
    Ok(respuesta)
}

// ---------- quitar_consola ----------

/// `quitar_consola { identidad } | { id }`: deja de conectarse a esa consola.
/// Desde la propia consola (si quedan otras) es como «Dejar de gestionar».
pub fn quitar(v: &mut Vinculo, c: &Value) -> Result<String, String> {
    let identidad = c["identidad"].as_str().unwrap_or("").trim();
    let id = c["id"].as_str().unwrap_or("").trim();
    let es = |e: &Enlace| (!identidad.is_empty() && e.identidad == identidad) || (!id.is_empty() && e.id == id);
    if v.otras.is_empty() {
        return Err("Es la única consola de este equipo: para dejarla, usa «Desvincular».".into());
    }
    let (quitada, otra) = if es(&v.enlace()) {
        let e = v.enlace();
        v.secreto.clear(); // `limpiar` la quita y pone otra en su sitio
        (e, None)
    } else {
        let i = v.otras.iter().position(es).ok_or("Esa consola no gestiona este equipo.")?;
        let e = v.otras.remove(i);
        (e.clone(), Some(e))
    };
    let resumen = format!("{} ({})", nombre_de(&quitada.nombre, &quitada.url), quitada.url);
    crate::agent::log(&format!("Consola quitada: {resumen}."));
    let desde = nombre_de(&v.nombre_consola, &v.url);
    let (base, mensaje) = (v.clone(), format!("Este equipo dejó de conectarse a la consola {resumen} (lo pidió {desde})."));
    std::thread::spawn(move || {
        // A la que se va, un último aviso con sus credenciales; después, a las que quedan.
        if let Some(e) = otra {
            let mut w = base.clone();
            w.otras.push(e.clone());
            if w.activar(&e.id) {
                let _ = s::llamar_ok(&w, "/api/agente/aviso", &json!({ "tipo": "cambio_inusual", "mensaje": mensaje }));
            }
        }
        avisar_a_todas(&mensaje);
    });
    Ok(format!("Quitada la consola {resumen}: ya no gestiona este equipo."))
}

/// `resguardo-agente consolas`: la lista para la terminal.
pub fn listar(v: &Vinculo) -> Vec<String> {
    let ahora = chrono::Local::now();
    let fila = |n: usize, e: &Enlace| {
        let contacto = chrono::DateTime::from_timestamp(e.ultimo_ok, 0)
            .filter(|_| e.ultimo_ok > 0)
            .map(|t| {
                let min = (ahora.timestamp() - t.timestamp()).max(0) / 60;
                match min {
                    0..=1 => "hace un momento".to_string(),
                    2..=119 => format!("hace {min} min"),
                    120..=2879 => format!("hace {} h", min / 60),
                    _ => format!("hace {} días", min / 1440),
                }
            })
            .unwrap_or_else(|| "nunca".into());
        format!(
            "{n}. {} · {} · identidad {} · último contacto: {contacto}{}",
            nombre_de(&e.nombre, &e.url),
            e.url,
            huella_corta(&e.identidad),
            if n == 1 { " (principal)" } else { "" }
        )
    };
    let mut l = vec![fila(1, &v.enlace())];
    l.extend(v.otras.iter().enumerate().map(|(i, e)| fila(i + 2, e)));
    l
}

/// `resguardo-agente consolas quitar <n.º | dirección | huella>`: en el equipo,
/// para una consola que ya no existe. Como administrador local.
pub fn quitar_local(que: &str) -> Result<String, String> {
    crate::agent::require_admin()?;
    let que = que.trim();
    modificar(|v| {
        if v.modo != "gestionado" || v.secreto.is_empty() {
            return Err("Este equipo no está gestionado por ninguna consola.".to_string());
        }
        let todas: Vec<Enlace> = std::iter::once(v.enlace()).chain(v.otras.iter().cloned()).collect();
        let norm = |x: &str| x.chars().filter(|c| c.is_ascii_hexdigit()).collect::<String>().to_ascii_uppercase();
        let i = match que.parse::<usize>() {
            Ok(n) if (1..=todas.len()).contains(&n) => n - 1,
            _ => todas
                .iter()
                .position(|e| {
                    e.url.trim_end_matches('/') == que.trim_end_matches('/')
                        || (norm(que).len() >= 8 && norm(&huella_corta(&e.identidad)).starts_with(&norm(que)))
                })
                .ok_or_else(|| format!("No hay ninguna consola «{que}». Mira la lista con «resguardo-agente consolas»."))?,
        };
        if todas.len() == 1 {
            return Err("Es la única consola de este equipo: para dejarla, «resguardo-agente desvincular».".into());
        }
        let e = &todas[i];
        let resumen = format!("{} ({})", nombre_de(&e.nombre, &e.url), e.url);
        if i == 0 {
            v.secreto.clear();
        } else {
            v.otras.remove(i - 1);
        }
        crate::agent::log(&format!("Consola quitada en el equipo (resguardo-agente consolas quitar): {resumen}."));
        Ok(resumen)
    })
    .ok_or("Este equipo no está vinculado a ningún servidor.")?
}

/// `cambiar_clave_admin` con varias consolas: la `K_cfg` nueva de cada una.
/// `k_cfg_consolas` (de la consola que la cambia, que sabe la sal de cada una)
/// manda; si no viene, las que tenían la misma `K_cfg` que la consola que la
/// cambia (la misma sal) toman la nueva, y las demás se quedan como estaban.
pub fn cambiar_kcfg_de_las_demas(v: &mut Vinculo, vieja: Option<&str>, nueva: &str, c: &Value) {
    let mapa = c["k_cfg_consolas"].as_object();
    for e in v.otras.iter_mut() {
        let dada = mapa.and_then(|m| m.get(&e.identidad)).and_then(Value::as_str).filter(|k| b64_de(k, 32, 32));
        match dada {
            Some(k) => e.k_cfg = Some(k.to_string()),
            None if e.k_cfg.as_deref() == vieja => e.k_cfg = Some(nueva.to_string()),
            None => crate::agent::log(&format!(
                "Clave de administración cambiada: la consola {} usa otra sal y no llegó su K_cfg nueva; su configuración sigue cifrada con la anterior hasta que se mande.",
                nombre_de(&e.nombre, &e.url)
            )),
        }
        e.config_pendiente = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vinculo() -> Vinculo {
        Vinculo {
            url: "https://local:8443".into(),
            identidad: B64.encode([1u8; 32]),
            cliente_id: "c1".into(),
            equipo_id: "e1".into(),
            secreto: "s1".into(),
            k_cfg: Some(B64.encode([7u8; 32])),
            ultimo_seq: 10,
            modo: "gestionado".into(),
            verificador: Some(B64.encode([9u8; 32])),
            ..Default::default()
        }
    }

    fn otra(id: &str, n: u8) -> Enlace {
        Enlace {
            id: id.into(),
            url: format!("https://otra{n}:8443"),
            identidad: B64.encode([n; 32]),
            cliente_id: format!("c{n}"),
            equipo_id: "e1".into(),
            secreto: format!("s{n}"),
            k_cfg: Some(B64.encode([n; 32])),
            ultimo_seq: n as u64,
            ..Default::default()
        }
    }

    /// Un `servidor.bin` anterior (sin `otras` ni `enlace_id`) es una lista de uno.
    #[test]
    fn un_vinculo_anterior_es_una_lista_de_uno() {
        let antiguo = json!({
            "url": "https://192.168.1.20:8443", "ca_pem": "PEM", "identidad": B64.encode([1u8; 32]), "cliente_id": "c", "equipo_id": "e",
            "secreto": "x", "box_secret": "b", "sign_seed": "s", "sal_equipo": "sal", "ultimo_seq": 41, "modo": "gestionado",
            "fallos": { "admin": { "n": 2, "desde": 1, "bloqueado_hasta": 0, "rachas": 0 } },
        });
        let v: Vinculo = serde_json::from_value(antiguo).unwrap();
        assert_eq!(v.ids_enlaces(), vec![PRINCIPAL.to_string()]);
        assert!(v.otras.is_empty());
        assert_eq!(v.enlace().ultimo_seq, 41);
        assert_eq!(v.enlace().fallos["admin"].n, 2);
        assert_eq!(v.vistas().len(), 1);
        // Y se vuelve a guardar igual (más los campos nuevos, vacíos).
        let otra_vez: Vinculo = serde_json::from_value(serde_json::to_value(&v).unwrap()).unwrap();
        assert_eq!(otra_vez.url, v.url);
        assert_eq!(otra_vez.id_enlace(), PRINCIPAL);
    }

    #[test]
    fn activar_e_intercambiar_vuelve_a_dejarlo_igual() {
        let mut v = vinculo();
        v.otras = vec![otra("b", 2), otra("c", 3)];
        v.enlace_id = PRINCIPAL.into();
        let antes = serde_json::to_value(&v).unwrap();
        assert!(v.activar("c"));
        assert_eq!((v.url.as_str(), v.secreto.as_str(), v.ultimo_seq), ("https://otra3:8443", "s3", 3));
        assert!(v.otras.iter().any(|e| e.id == PRINCIPAL && e.ultimo_seq == 10), "la principal, en otras mientras tanto");
        // Lo del equipo no se mueve.
        assert_eq!(v.verificador, Some(B64.encode([9u8; 32])));
        v.ultimo_seq = 4; // una orden por «c»
        assert!(v.activar(PRINCIPAL));
        assert_eq!(v.ultimo_seq, 10);
        assert_eq!(v.otras.iter().find(|e| e.id == "c").unwrap().ultimo_seq, 4, "su seq, en su vínculo");
        let mut despues = serde_json::to_value(&v).unwrap();
        despues["otras"].as_array_mut().unwrap().iter_mut().for_each(|e| {
            if e["id"] == "c" {
                e["ultimo_seq"] = json!(3)
            }
        });
        assert_eq!(antes, despues);
        assert!(!v.activar("no-existe"));
    }

    #[test]
    fn quitar_la_activa_pone_otra_en_su_sitio() {
        let mut v = vinculo();
        v.otras = vec![otra("b", 2)];
        // Desvincular desde la principal: se va solo ella.
        v.secreto.clear();
        v.modo = "local".into();
        v.limpiar();
        assert_eq!((v.id_enlace().as_str(), v.modo.as_str(), v.url.as_str()), ("b", "gestionado", "https://otra2:8443"));
        assert!(v.otras.is_empty());
        // Sin otras, se queda en local como siempre.
        v.secreto.clear();
        v.modo = "local".into();
        v.limpiar();
        assert_eq!(v.modo, "local");
    }

    #[test]
    fn quitar_consola_por_identidad() {
        let mut v = vinculo();
        let propia = json!({ "identidad": v.identidad });
        assert!(quitar(&mut v, &propia).unwrap_err().contains("única"));
        v.otras = vec![otra("b", 2), otra("c", 3)];
        assert!(quitar(&mut v, &json!({ "identidad": B64.encode([8u8; 32]) })).unwrap_err().contains("no gestiona"));
        let m = quitar(&mut v, &json!({ "identidad": B64.encode([2u8; 32]) })).unwrap();
        assert!(m.contains("otra2"), "{m}");
        assert_eq!(v.otras.len(), 1);
        // La propia (la activa): se va al limpiar.
        let ident = v.identidad.clone();
        quitar(&mut v, &json!({ "identidad": ident })).unwrap();
        v.limpiar();
        assert_eq!(v.id_enlace(), "c");
        assert!(v.otras.is_empty());
    }

    #[test]
    fn anadir_comprueba_el_pedido() {
        let bien = json!({ "url": "https://consola.ejemplo.com/", "identidad": B64.encode([5u8; 32]), "huella_ca": vec!["AB"; 32].join(":"),
                           "ficha": "ABCDEFGHIJKLMNOPQRSTUVWX", "sal_cliente": B64.encode([3u8; 16]), "k_cfg": B64.encode([4u8; 32]), "nombre": "En línea" });
        let p = pedido(&bien).unwrap();
        assert_eq!(p.url, "https://consola.ejemplo.com");
        let mut v = vinculo();
        assert!(admite(&v, &p).is_ok());
        for (k, mal) in [
            ("url", json!("http://consola.ejemplo.com")),
            ("url", json!("http://localhost.ejemplo.com")),
            ("url", json!("http://127.0.0.1.ejemplo.com:8080")),
            ("url", json!("http://localhost@consola.ejemplo.com")),
            ("identidad", json!("corta")),
            ("huella_ca", json!("AB:CD")),
            ("ficha", json!("x")),
            ("k_cfg", json!(null)),
        ] {
            let mut c = bien.clone();
            c[k] = mal;
            assert!(pedido(&c).is_err(), "{k}");
        }
        // Ya es una de sus consolas.
        let mut c = bien.clone();
        c["identidad"] = json!(v.identidad);
        assert!(admite(&v, &pedido(&c).unwrap()).unwrap_err().contains("ya gestiona"));
        v.otras = (10..=13).map(|n| otra(&format!("o{n}"), n)).collect();
        assert!(admite(&v, &p).unwrap_err().contains("5 consolas"));
        v.otras.clear();
        v.verificador = None;
        assert!(admite(&v, &p).unwrap_err().contains("clave de administración"));
    }

    #[test]
    fn la_kcfg_nueva_de_las_demas_al_cambiar_la_clave() {
        let mut v = vinculo();
        let vieja = v.k_cfg.clone();
        let mut misma_sal = otra("b", 2);
        misma_sal.k_cfg = vieja.clone();
        v.otras = vec![misma_sal, otra("c", 3), otra("d", 4)];
        let nueva = B64.encode([42u8; 32]);
        let dada = B64.encode([43u8; 32]);
        cambiar_kcfg_de_las_demas(&mut v, vieja.as_deref(), &nueva, &json!({ "k_cfg_consolas": { B64.encode([3u8; 32]): dada } }));
        assert_eq!(v.otras[0].k_cfg.as_deref(), Some(nueva.as_str()), "misma sal: la misma K_cfg nueva");
        assert_eq!(v.otras[1].k_cfg.as_deref(), Some(dada.as_str()), "la que manda la consola");
        assert_eq!(v.otras[2].k_cfg, Some(B64.encode([4u8; 32])), "otra sal sin K_cfg: se queda");
        assert!(v.otras.iter().all(|e| e.config_pendiente));
    }

    #[test]
    fn resumen_de_las_consolas() {
        let mut v = vinculo();
        v.ultimo_ok = 1_800_000_123;
        v.otras = vec![otra("b", 2)];
        let r = resumen(&v);
        assert_eq!(r.as_array().unwrap().len(), 2);
        assert_eq!(r[0]["esta"], true);
        assert_eq!(r[1]["esta"], false);
        assert_eq!(r[1]["nombre"], "otra2:8443");
        // Redondeado a 15 min: el resumen no cambia en cada informe.
        let t = chrono::DateTime::parse_from_rfc3339(r[0]["ultimo_contacto"].as_str().unwrap()).unwrap().timestamp();
        assert_eq!(t % 900, 0);
        assert!(!r.to_string().contains("s1") && !r.to_string().contains("secreto"), "sin credenciales");
    }
}
