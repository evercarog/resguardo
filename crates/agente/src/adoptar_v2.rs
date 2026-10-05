//! Venir de la app de escritorio (docs/agente-gestionado.md, «Venir de la app
//! de escritorio»; docs/api-servidor.md §5, v1.14):
//!
//! - **Adoptar** (`adoptar_repositorio`, clave de administración): un
//!   repositorio de restic que ya existe (de la app de escritorio 0.6.x, de
//!   otro programa o hecho a mano) pasa a ser un repositorio gestionado más,
//!   **de lectura y escritura**: las copias siguen guardando en él y todo su
//!   historial se queda. Primero se comprueba que la contraseña lo abre
//!   (`restic cat config`); no se adopta uno que este equipo ya usa. Con
//!   `solo_probar` solo se comprueba (el «Probar» de la consola).
//! - **Traer el historial** (`copiar_historial`, clave de administración):
//!   `restic copy` de las versiones de otro repositorio a uno gestionado, en
//!   segundo plano. Solo añade (sirve con servidores de solo añadir) y no
//!   repite las que ya se trajeron.
//! - `crear_repositorio { …, parametros_de: <origen> }`: el repositorio nuevo
//!   nace con los parámetros de troceado del origen (`init --copy-chunker-params`),
//!   para que lo traído después se deduplique del todo.
//!
//! Las credenciales y contraseñas llegan dentro de la orden (sellada para el
//! equipo) y se quedan aquí; los resultados no llevan rutas ni secretos.

use crate::gestion_v2::{self as g, Destino, RepoV2};
use crate::servidor_v2::{self as s, Resultado, Vinculo};
use resguardo_motor::restic::{self, Access};
use serde_json::{json, Value};
use std::time::Duration;

const ABRIR: Duration = Duration::from_secs(300);
/// Traer un historial grande por internet puede llevar días.
const COPIAR: Duration = Duration::from_secs(7 * 24 * 3600);
/// Cada cuánto se avisa a la consola de cómo va (`en_marcha` con el progreso).
const AVISO_CADA: Duration = Duration::from_secs(60);

fn texto(c: &Value, k: &str) -> String {
    c[k].as_str().unwrap_or("").to_string()
}

/// La ruta del repositorio dentro de su destino («Siigo», «ana/portatil»), o
/// vacía si el repositorio es el propio destino. Sin `..`, ni `\`, ni nada que
/// cambie de destino o parezca una opción.
pub fn ruta_valida(r: &str) -> bool {
    r.is_empty()
        || (r.chars().count() <= 200
            && !r.chars().any(|c| c.is_control() || matches!(c, '\\' | ':' | '@' | '?' | '#' | '%'))
            && r.split('/').all(|x| !x.trim().is_empty() && x != "." && x != ".." && !x.starts_with('-')))
}

/// Un destino escrito en la orden: `{tipo, donde, nombre?, usuario?, secreto?, ca_pem?}`.
pub fn destino_de(d: &Value, id: &str) -> Result<Destino, String> {
    let tipo = texto(d, "tipo");
    if !matches!(tipo.as_str(), "local" | "rest" | "s3" | "b2" | "sftp") {
        return Err(format!("Tipo de destino no admitido: «{tipo}»."));
    }
    let ca_pem = match d.get("ca_pem") {
        None | Some(Value::Null) => None,
        Some(Value::String(p)) if p.trim().is_empty() => None,
        Some(Value::String(p)) if p.contains("BEGIN CERTIFICATE") => Some(p.clone()),
        Some(_) => return Err("El certificado no es válido: pega el bloque entero, desde «-----BEGIN CERTIFICATE-----».".into()),
    };
    let mut nombre: String = texto(d, "nombre").trim().chars().filter(|c| !c.is_control()).take(80).collect();
    if nombre.is_empty() {
        nombre = match tipo.as_str() {
            "local" => "Disco o carpeta del equipo".into(),
            "rest" => texto(d, "donde").split("://").nth(1).unwrap_or("").split('/').next().unwrap_or("").chars().take(80).collect(),
            "b2" => "Backblaze B2".into(),
            "s3" => "S3".into(),
            _ => "SFTP".into(),
        };
        if nombre.is_empty() {
            nombre = "Servidor de copias".into();
        }
    }
    Ok(Destino {
        id: id.into(),
        nombre,
        tipo,
        donde: texto(d, "donde").trim().to_string(),
        usuario: d["usuario"].as_str().filter(|s| !s.is_empty()).map(str::to_string),
        secreto: d["secreto"].as_str().filter(|s| !s.is_empty()).map(str::to_string),
        ca_pem,
        equipo_almacen: crate::gestion_v2::equipo_almacen_de(d),
    })
}

/// Para comparar ubicaciones (sin barras finales ni mayúsculas).
fn normal(location: &str) -> String {
    location.trim().trim_end_matches(['/', '\\']).to_lowercase()
}

/// El repositorio de este equipo que ya usa esa ubicación, si hay alguno.
fn ya_en_uso(v: &Vinculo, location: &str) -> Option<String> {
    let n = normal(location);
    if let Some(r) = v.repos_v2.iter().find(|r| g::acceso(v, &r.id).is_ok_and(|a| normal(&a.location) == n)) {
        return Some(r.nombre.clone());
    }
    // Los del agente en modo local (que no gestiona el servidor).
    crate::agent::load_config().repos.into_iter().find(|r| !v.repos_v2.iter().any(|x| x.id == r.id) && normal(&r.location) == n).map(|r| r.name)
}

/// Lo que se sabe de un repositorio al abrirlo.
#[derive(Debug, Default)]
pub struct Info {
    pub versiones: usize,
    pub ultima: Option<String>,
    /// Nombres de los equipos que guardaron versiones en él (para elegir qué traer).
    pub equipos: Vec<String>,
    pub etiquetas: Vec<String>,
}

/// Comprueba que la contraseña abre el repositorio (`restic cat config`) y
/// lee sus versiones. Los errores, en palabras normales y sin secretos.
pub fn abrir(acc: &Access) -> Result<Info, String> {
    let out = restic::run_raw(acc, &["cat", "config", "--no-lock"], ABRIR)?;
    let err = out.stderr.to_lowercase();
    match out.code {
        Some(0) => {}
        _ if err.contains("wrong password") || out.code == Some(12) => {
            return Err("La contraseña no abre ese repositorio: revisa que sea la suya (la del kit o la que usaba la app de escritorio).".into())
        }
        _ if out.code == Some(10) || err.contains("is there a repository") || err.contains("does not exist") => {
            return Err("No hay ningún repositorio en esa dirección. Revisa la dirección y el nombre de la carpeta del repositorio.".into())
        }
        code => return Err(restic::exit_error(code, &out.stderr)),
    }
    let snaps = restic::snapshots(acc)?;
    let mut equipos: Vec<String> = snaps.iter().map(|s| s.hostname.clone()).filter(|h| !h.is_empty()).collect();
    equipos.sort();
    equipos.dedup();
    let mut etiquetas: Vec<String> = snaps.iter().flat_map(|s| s.tags.iter().cloned()).collect();
    etiquetas.sort();
    etiquetas.dedup();
    Ok(Info { versiones: snaps.len(), ultima: snaps.iter().map(|s| s.time.clone()).max(), equipos, etiquetas })
}

fn fecha_corta(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso).map(|d| d.with_timezone(&chrono::Local).format("%d/%m/%Y").to_string()).unwrap_or_default()
}

fn versiones_texto(i: &Info) -> String {
    match (i.versiones, i.ultima.as_deref().map(fecha_corta)) {
        (0, _) => "todavía sin versiones".into(),
        (1, Some(f)) => format!("1 versión, del {f}"),
        (n, Some(f)) => format!("{n} versiones, la última del {f}"),
        (n, None) => format!("{n} versiones"),
    }
}

/// ¿Rest-server de solo añadir? (Nunca borra nada: ver protection.rs.)
fn probar_solo_anadir(d: &Destino, acc: &Access) -> Option<bool> {
    (d.tipo == "rest")
        .then(|| crate::protection::probe_append_only(&acc.location, acc.rest_auth.as_ref().map(|(u, p)| (u.as_str(), p.as_str())), acc.cacert.as_deref()))
        .flatten()
}

/// `adoptar_repositorio { id, nombre, contrasena, destino: {id} | {id, nombre?, tipo, donde, usuario?, secreto?, ca_pem?}, ruta, solo_probar? }`.
/// Devuelve el mensaje y el detalle (`{versiones, ultima, solo_anadir, en_uso?, equipos?, etiquetas?}`;
/// los nombres de equipos y etiquetas solo si va sellado para la consola).
pub fn adoptar_repositorio(v: &mut Vinculo, c: &Value, sellado: bool) -> Result<(String, Value), String> {
    let solo_probar = c["solo_probar"].as_bool().unwrap_or(false);
    let ruta = texto(c, "ruta").trim().trim_matches('/').to_string();
    if !ruta_valida(&ruta) {
        return Err("La carpeta del repositorio no es válida (sin «..», «\\» ni «:»).".into());
    }
    let contrasena = texto(c, "contrasena");
    if contrasena.is_empty() {
        return Err("Falta la contraseña del repositorio.".into());
    }
    let dest = &c["destino"];
    let destino_id = texto(dest, "id");
    let nuevo = if dest.get("tipo").is_some() {
        let id = if destino_id.is_empty() && solo_probar { "prueba".to_string() } else { destino_id.clone() };
        if !g::id_valido(&id) {
            return Err("Id de destino no válido.".into());
        }
        if !solo_probar && v.destinos.iter().any(|d| d.id == id) {
            return Err(format!("Ya hay un destino «{id}»."));
        }
        Some(destino_de(dest, &id)?)
    } else {
        None
    };
    let d = match &nuevo {
        Some(d) => d.clone(),
        None => v.destinos.iter().find(|d| d.id == destino_id).cloned().ok_or_else(|| format!("No hay ningún destino «{destino_id}» en este equipo."))?,
    };
    let acc = g::acceso_destino(&d, &ruta, &contrasena)?;
    let en_uso = ya_en_uso(v, &acc.location);
    let info = abrir(&acc)?;
    let solo_anadir = probar_solo_anadir(&d, &acc);
    let mut detalle = json!({ "versiones": info.versiones, "ultima": info.ultima, "solo_anadir": solo_anadir, "en_uso": en_uso });
    if sellado {
        detalle["equipos"] = json!(info.equipos);
        detalle["etiquetas"] = json!(info.etiquetas);
    }
    let servidor = if solo_anadir == Some(true) { " El servidor es de solo añadir: la retención se aplica en él." } else { "" };
    if solo_probar {
        let aviso = en_uso.as_ref().map(|n| format!(" Este equipo ya lo usa («{n}»).")).unwrap_or_default();
        return Ok((format!("Se abre con esa contraseña: {}.{servidor}{aviso}", versiones_texto(&info)), detalle));
    }
    if let Some(n) = en_uso {
        return Err(format!("Este equipo ya usa ese repositorio («{n}»): no hace falta adoptarlo otra vez."));
    }
    let (id, nombre) = (texto(c, "id"), texto(c, "nombre").trim().to_string());
    if !g::id_valido(&id) || v.repos_v2.iter().any(|r| r.id == id) {
        return Err("Id de repositorio no válido o ya en uso.".into());
    }
    if nombre.is_empty() || nombre.chars().count() > 80 || nombre.chars().any(char::is_control) {
        return Err("Escribe un nombre para el repositorio (hasta 80 caracteres).".into());
    }
    if let Some(d) = nuevo {
        v.destinos.push(d);
    }
    v.repos_v2.push(RepoV2 {
        id: id.clone(),
        nombre: nombre.clone(),
        destino: d.id.clone(),
        contrasena,
        // Su ubicación en el destino no es su id: la de siempre.
        ubicacion_origen: Some(ruta),
        solo_anadir,
        ..Default::default()
    });
    let _ = g::subir_config(v);
    Ok((format!("Repositorio «{nombre}» adoptado con todo su historial ({}). Las copias pueden guardar ya en él.{servidor}", versiones_texto(&info)), detalle))
}

// ---------- Origen de un historial ----------

/// El repositorio del que se lee: `{repo: "<id de este equipo>"}` o
/// `{destino: {tipo, donde, usuario?, secreto?, ca_pem?}, ruta, contrasena}`.
pub fn origen(v: &Vinculo, o: &Value) -> Result<Access, String> {
    if let Some(r) = o["repo"].as_str() {
        return g::acceso(v, r);
    }
    let ruta = texto(o, "ruta").trim().trim_matches('/').to_string();
    if !ruta_valida(&ruta) {
        return Err("La carpeta del repositorio de origen no es válida.".into());
    }
    let contrasena = texto(o, "contrasena");
    if contrasena.is_empty() {
        return Err("Falta la contraseña del repositorio de origen.".into());
    }
    // El certificado propio se guarda con un nombre de usar y tirar (se borra al terminar).
    let d = destino_de(&o["destino"], &format!("origen-{}", &uuid::Uuid::new_v4().simple().to_string()[..12]))?;
    g::acceso_destino(&d, &ruta, &contrasena)
}

/// Borra el certificado temporal de un origen escrito en la orden.
fn soltar(src: &Access) {
    if let Some(p) = &src.cacert {
        if std::path::Path::new(p).file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("ca-destino-origen-")) {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// `restic -r DESTINO <cmd>` leyendo de ORIGEN: la ubicación del origen (con
/// su usuario de rest-server) y su contraseña van en variables del proceso,
/// nunca en los argumentos (que se ven en la lista de procesos). Devuelve el
/// acceso y los argumentos de más (el certificado propio del origen).
pub fn con_origen(dest: &Access, src: &Access) -> Result<(Access, Vec<String>), String> {
    if src.env.iter().any(|(k, x)| dest.env.iter().any(|(k2, y)| k == k2 && x != y)) {
        return Err("Origen y destino están en dos cuentas de nube distintas: restic usa una sola clave para las dos. \
                    Trae primero el historial a un disco o a un servidor, y desde ahí a la nube."
            .into());
    }
    let mut both = dest.clone();
    // El usuario del destino (p. ej. el de este equipo en un almacén) va en su
    // propia dirección y no en RESTIC_REST_USERNAME/PASSWORD: restic aplica
    // esas variables también al origen si no trae usuario, y se mandarían a
    // otro servidor (el rest-server antiguo).
    if let Some(auth) = both.rest_auth.take() {
        both.location = crate::tasks::location_with_auth(&both.location, Some(&auth));
    }
    for kv in &src.env {
        if !both.env.iter().any(|(k, _)| *k == kv.0) {
            both.env.push(kv.clone());
        }
    }
    both.env.push(("RESTIC_FROM_REPOSITORY".into(), crate::tasks::location_with_auth(&src.location, src.rest_auth.as_ref())));
    both.env.push(("RESTIC_FROM_PASSWORD".into(), src.password.clone()));
    let mut extra = Vec::new();
    match (&both.cacert, &src.cacert) {
        (None, Some(ca)) => both.cacert = Some(ca.clone()),
        (Some(a), Some(b)) if a != b => extra = vec!["--cacert".to_string(), b.clone()],
        _ => {}
    }
    Ok((both, extra))
}

/// `restic init --copy-chunker-params` desde el origen (lo usa `crear_repositorio`).
pub fn init_como(v: &Vinculo, dest: &Access, o: &Value) -> Result<restic::RawOutput, String> {
    let src = origen(v, o)?;
    let r = (|| {
        abrir(&src).map_err(|e| format!("Repositorio de origen: {e}"))?;
        let (both, extra) = con_origen(dest, &src)?;
        let mut args: Vec<&str> = vec!["init", "--copy-chunker-params"];
        args.extend(extra.iter().map(String::as_str));
        restic::run_raw(&both, &args, ABRIR)
    })();
    soltar(&src);
    r
}

// ---------- Traer el historial ----------

/// Qué versiones traer: de qué equipos (`--host`) y con qué etiquetas (`--tag`).
#[derive(Debug, Default, Clone)]
pub struct Filtro {
    pub equipos: Vec<String>,
    pub etiquetas: Vec<String>,
}

impl Filtro {
    fn de(c: &Value) -> Result<Self, String> {
        let lista = |k: &str| -> Result<Vec<String>, String> {
            let l: Vec<String> = c[k]
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_str).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
                .unwrap_or_default();
            if l.len() > 50 || l.iter().any(|s| s.chars().count() > 200 || s.starts_with('-') || s.chars().any(char::is_control)) {
                return Err("Filtro no válido.".into());
            }
            Ok(l)
        };
        Ok(Filtro { equipos: lista("equipos")?, etiquetas: lista("etiquetas")? })
    }
    fn args(&self) -> Vec<String> {
        let mut a = Vec::new();
        for h in &self.equipos {
            a.extend(["--host".to_string(), h.clone()]);
        }
        for t in &self.etiquetas {
            a.extend(["--tag".to_string(), t.clone()]);
        }
        a
    }
    fn deja(&self, s: &restic::Snapshot) -> bool {
        (self.equipos.is_empty() || self.equipos.contains(&s.hostname))
            && (self.etiquetas.is_empty() || self.etiquetas.iter().any(|t| t.split(',').all(|x| s.tags.iter().any(|y| y == x))))
    }
}

/// Copia las versiones del origen que falten en el repositorio gestionado `repo`.
/// `progreso(hechas, total)` con cada versión que llega. Devuelve el mensaje final.
pub fn traer(v: &Vinculo, repo: &str, src: &Access, filtro: &Filtro, progreso: &mut dyn FnMut(usize, usize)) -> Result<String, String> {
    let dest = g::acceso(v, repo)?;
    if normal(&dest.location) == normal(&src.location) {
        return Err("El origen es el mismo repositorio: elige otro.".into());
    }
    abrir(src).map_err(|e| format!("Repositorio de origen: {e}"))?;
    let (both, extra) = con_origen(&dest, src)?;
    // Lo que ya está (traído antes): restic no lo repite; aquí solo se cuenta.
    let presentes: std::collections::HashSet<String> = restic::snapshots(&dest)?.into_iter().flat_map(|s| [Some(s.id), s.original]).flatten().collect();
    let candidatas: Vec<restic::Snapshot> = restic::snapshots(src)?.into_iter().filter(|s| filtro.deja(s)).collect();
    let ya = candidatas.iter().filter(|s| presentes.contains(&s.id) || s.original.as_ref().is_some_and(|o| presentes.contains(o))).count();
    let total = candidatas.len() - ya;
    if total == 0 {
        return Ok(if candidatas.is_empty() {
            "No hay versiones que traer (con ese filtro, el origen no tiene ninguna).".into()
        } else {
            format!("Nada nuevo: las {} versiones del origen ya estaban en este repositorio.", candidatas.len())
        });
    }
    progreso(0, total);
    let mut args: Vec<String> = vec!["copy".into(), "--retry-lock".into(), "30m".into()];
    args.extend(extra);
    args.extend(filtro.args());
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut hechas = 0usize;
    let out = restic::run_raw_lines(&both, &refs, COPIAR, &mut |linea| {
        let l = linea.trim().to_lowercase();
        if l.starts_with("snapshot ") && l.contains(" saved") {
            hechas += 1;
            progreso(hechas, total);
        }
    })?;
    if out.code != Some(0) {
        let e = restic::exit_error(out.code, &out.stderr);
        return Err(if hechas > 0 {
            format!("Se trajeron {hechas} de {total} versiones y después falló: {e} Puedes volver a pedirlo: sigue donde se quedó.")
        } else {
            e
        });
    }
    let mut m = match hechas.max(total) {
        1 => "Historial traído: 1 versión nueva.".to_string(),
        n => format!("Historial traído: {n} versiones nuevas."),
    };
    match ya {
        0 => {}
        1 => m.push_str(" 1 ya estaba."),
        n => m.push_str(&format!(" {n} ya estaban.")),
    }
    Ok(m)
}

/// Repositorios con un historial viniendo ahora mismo (uno a la vez por repositorio).
static EN_CURSO: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// `copiar_historial { repo, origen: {repo} | {destino, ruta, contrasena}, filtro?: {equipos?, etiquetas?} }`:
/// se comprueba lo que se puede al momento y la copia sigue en segundo plano;
/// la orden queda `en_marcha` (con el progreso cada minuto) hasta el resultado final.
/// Lo que comprueba `copiar_historial` antes de empezar (y el repositorio queda
/// «en curso»): el id y el nombre del repositorio, el origen y el filtro.
fn preparar_historial(v: &Vinculo, c: &Value) -> Result<(String, String, Access, Filtro), String> {
    let repo = texto(c, "repo");
    let r = v.repos_v2.iter().find(|r| r.id == repo).ok_or("Ese repositorio no lo gestiona este servidor.")?;
    if r.solo_lectura {
        return Err("Ese repositorio es de solo lectura (importado de otro equipo): trae el historial a uno propio.".into());
    }
    let filtro = Filtro::de(c.get("filtro").unwrap_or(&Value::Null))?;
    if !c["origen"].is_object() {
        return Err("Falta el repositorio de origen.".into());
    }
    let src = origen(v, &c["origen"])?;
    {
        let mut en_curso = EN_CURSO.lock().map_err(|_| "Estado interno no disponible.")?;
        if en_curso.contains(&repo) {
            soltar(&src);
            return Err("Ya se está trayendo un historial a este repositorio: espera a que termine.".into());
        }
        en_curso.push(repo.clone());
    }
    Ok((repo, r.nombre.clone(), src, filtro))
}

/// Al terminar (bien o mal): fuera el origen y el «en curso», y las versiones al día en el próximo informe.
fn terminar_historial(repo: &str, src: &Access) {
    soltar(src);
    if let Ok(mut en_curso) = EN_CURSO.lock() {
        en_curso.retain(|x| x != repo);
    }
    crate::informe_v2::invalidar(repo);
}

/// En modo local (ventana del equipo, sin consola): cómo va lo que se trae a
/// cada repositorio (`en_marcha`, `hecha` o `fallida`, y el mensaje). La
/// ventana lo pregunta mientras está abierta; lo último se queda hasta otro.
static LOCAL: std::sync::Mutex<Vec<(String, &'static str, String)>> = std::sync::Mutex::new(Vec::new());

fn anotar_local(repo: &str, estado: &'static str, mensaje: String) {
    if let Ok(mut l) = LOCAL.lock() {
        l.retain(|(r, _, _)| r != repo);
        l.push((repo.to_string(), estado, mensaje));
    }
}

/// `copiar_historial` en modo local (la ventana del equipo con la clave): lo
/// mismo, pero el progreso y el resultado se quedan aquí ([`historial_local`]).
pub fn copiar_historial_local(v: &Vinculo, c: &Value) -> Result<String, String> {
    let (repo, nombre, src, filtro) = preparar_historial(v, c)?;
    anotar_local(&repo, "en_marcha", "Preparando…".into());
    let v = v.clone();
    let m = format!("Trayendo el historial a «{nombre}»… Puede tardar: depende de cuánto haya que copiar.");
    std::thread::spawn(move || {
        let mut aviso = |hechas: usize, total: usize| anotar_local(&repo, "en_marcha", format!("Trayendo el historial: {hechas} de {total} versiones…"));
        let r = traer(&v, &repo, &src, &filtro, &mut aviso);
        terminar_historial(&repo, &src);
        crate::agent::log(&format!("Historial hacia «{nombre}» (ventana del equipo): {}.", if r.is_ok() { "hecho" } else { "falló" }));
        match r {
            Ok(m) => anotar_local(&repo, "hecha", m),
            Err(e) => anotar_local(&repo, "fallida", e),
        }
    });
    Ok(m)
}

/// Cómo va el historial que se trae a `repo` en modo local: `{ estado, mensaje }` o `null`.
pub fn historial_local(repo: &str) -> Value {
    LOCAL
        .lock()
        .ok()
        .and_then(|l| l.iter().find(|(r, _, _)| r == repo).map(|(_, e, m)| json!({ "estado": e, "mensaje": crate::web::public_message(m) })))
        .unwrap_or(Value::Null)
}

/// v1.4x: `mover: { paso: "historial" | "ultimo" }` en `copiar_historial`: es un
/// paso de «Mover a otro sitio…» (la consola que lo lleva lo dice; las demás lo
/// enseñan sin poder tocarlo). Otro valor o ninguno: traer el historial sin más.
pub fn paso_mover(c: &Value) -> Option<&'static str> {
    match c.get("mover")? {
        Value::Bool(true) => Some("historial"),
        m => match m["paso"].as_str()? {
            "historial" => Some("historial"),
            "ultimo" => Some("ultimo"),
            _ => None,
        },
    }
}

/// Lo que se cuenta a todas las consolas mientras se trae el historial (progreso) y al terminar (historial del equipo).
pub fn operacion_historial(v: &Vinculo, c: &Value, repo: &str, nombre: &str) -> crate::progreso_v2::ops::Operacion {
    let paso = paso_mover(c);
    let origen = c["origen"]["repo"].as_str().and_then(|r| v.repos_v2.iter().find(|x| x.id == r));
    let etapa = if paso.is_some() { "Moviéndose a otro sitio: preparando…" } else { "Preparando…" };
    crate::progreso_v2::ops::Operacion {
        nombre: Some(nombre.to_string()),
        origen: origen.map(|x| x.id.clone()),
        nombre_origen: origen.map(|x| x.nombre.clone()),
        mover: paso.is_some(),
        paso: paso.map(str::to_string),
        ..crate::progreso_v2::ops::Operacion::de_consola(v, "historial", repo, etapa)
    }
}

/// Lo que se está haciendo, en palabras (para el progreso).
pub fn etapa_historial(mover: bool) -> &'static str {
    if mover {
        "Moviéndose a otro sitio: trayendo el historial"
    } else {
        "Trayendo el historial"
    }
}

/// La entrada del historial del equipo al terminar de traer un historial (o un paso de un movimiento).
pub fn entrada_historial(o: &crate::progreso_v2::ops::Operacion, r: &Result<String, String>) -> Value {
    json!({
        "nombre": o.nombre,
        "nombre_origen": o.nombre_origen,
        "repo": o.repo,
        "origen": o.origen,
        "mover": o.mover.then_some(true),
        "paso": o.paso,
        "consola": o.consola,
        "resultado": if r.is_ok() { "ok" } else { "fallo" },
        "mensaje": crate::web::public_message(match r { Ok(m) | Err(m) => m }).chars().take(240).collect::<String>(),
    })
}

pub fn copiar_historial(v: &Vinculo, c: &Value, orden: &str, seq: u64) -> Result<String, String> {
    let (repo, nombre, src, filtro) = preparar_historial(v, c)?;
    let op = operacion_historial(v, c, &repo, &nombre);
    let (v, orden) = (v.clone(), orden.to_string());
    let m = format!("Trayendo el historial a «{nombre}»… Puede tardar: depende de cuánto haya que copiar.");
    std::thread::spawn(move || {
        let guarda = crate::progreso_v2::ops::empezar(op.clone());
        let mut ultimo = std::time::Instant::now();
        let r = traer(&v, &repo, &src, &filtro, &mut |hechas: usize, total: usize| {
            // A todas las consolas (progreso, cada pocos segundos)…
            guarda.avance(etapa_historial(op.mover), Some(hechas as u64), Some(total as u64));
            // … y a la que la mandó, también en la orden (como siempre).
            if ultimo.elapsed() >= AVISO_CADA || hechas == 0 {
                ultimo = std::time::Instant::now();
                let m = format!("Trayendo el historial: {hechas} de {total} versiones…");
                let _ = s::enviar_resultado(&v, &orden, seq, &Resultado { estado: "en_marcha", mensaje: m, detalle: None });
            }
        });
        // La lista de versiones y el espacio, al día en el próximo informe.
        terminar_historial(&repo, &src);
        // Fuera del progreso y, en el historial del equipo (que llega a todas las consolas), cómo acabó.
        drop(guarda);
        crate::bitacora::anotar("historial", entrada_historial(&op, &r));
        let res = match r {
            Ok(m) => Resultado { estado: "hecha", mensaje: m, detalle: None },
            Err(e) => Resultado { estado: "fallida", mensaje: e, detalle: None },
        };
        crate::agent::log(&format!("Historial hacia «{nombre}»: {} ({}).", res.estado, res.mensaje));
        for intento in 0..5 {
            if s::enviar_resultado(&v, &orden, seq, &res).is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_secs(30 << intento));
        }
    });
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hay_restic() -> bool {
        resguardo_motor::restic::version().is_ok()
    }

    /// Un repositorio «de la app de escritorio»: creado y con una versión, hecho con restic a mano.
    fn repo_antiguo(dir: &std::path::Path, contrasena: &str, datos: &std::path::Path, host: &str) -> Access {
        let acc = Access::new(dir.display().to_string(), contrasena);
        let out = restic::run_raw(&acc, &["init"], ABRIR).unwrap();
        assert_eq!(out.code, Some(0), "{}", out.stderr);
        let d = datos.display().to_string();
        let out = restic::run_raw(&acc, &["backup", "--host", host, "--tag", "escritorio", &d], ABRIR).unwrap();
        assert_eq!(out.code, Some(0), "{}", out.stderr);
        acc
    }

    fn base(nombre: &str) -> std::path::PathBuf {
        let b = std::env::temp_dir().join(format!("resguardo-adoptar-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&b);
        std::fs::create_dir_all(b.join("datos")).unwrap();
        std::fs::write(b.join("datos").join("factura.txt"), "factura 1").unwrap();
        b
    }

    fn local(b: &std::path::Path) -> Value {
        json!({ "tipo": "local", "donde": b.display().to_string() })
    }

    /// v1.4x: los pasos de «Mover a otro sitio…» se reconocen y lo que se cuenta a todas
    /// las consolas (progreso e historial) dice de qué repositorio a cuál, quién lo empezó
    /// (por su nombre) y cómo acabó, sin rutas.
    #[test]
    fn pasos_de_mover_y_su_historial() {
        assert_eq!(paso_mover(&json!({ "mover": { "paso": "historial" } })), Some("historial"));
        assert_eq!(paso_mover(&json!({ "mover": { "paso": "ultimo" } })), Some("ultimo"));
        assert_eq!(paso_mover(&json!({ "mover": true })), Some("historial"));
        assert_eq!(paso_mover(&json!({ "mover": { "paso": "otro" } })), None);
        assert_eq!(paso_mover(&json!({ "repo": "x" })), None);
        let mut v = Vinculo { nombre_consola: "Oficina".into(), url: "https://192.168.1.20:8443".into(), ..Default::default() };
        v.repos_v2.push(RepoV2 { id: "viejo".into(), nombre: "Contabilidad".into(), ..Default::default() });
        v.repos_v2.push(RepoV2 { id: "nuevo".into(), nombre: "Contabilidad (almacén)".into(), ..Default::default() });
        let c = json!({ "repo": "nuevo", "origen": { "repo": "viejo" }, "mover": { "paso": "ultimo" } });
        let o = operacion_historial(&v, &c, "nuevo", "Contabilidad (almacén)");
        assert_eq!(
            (o.tipo, o.origen.as_deref(), o.nombre_origen.as_deref(), o.mover, o.paso.as_deref()),
            ("historial", Some("viejo"), Some("Contabilidad"), true, Some("ultimo"))
        );
        assert_eq!(o.consola.as_deref(), Some("Oficina"));
        let e = entrada_historial(&o, &Err(r"Falló al leer C:\Users\Ana\clave.txt".into()));
        assert_eq!(
            (e["resultado"].as_str(), e["mover"].as_bool(), e["origen"].as_str(), e["consola"].as_str()),
            (Some("fallo"), Some(true), Some("viejo"), Some("Oficina"))
        );
        assert!(!e.to_string().contains("Ana") && !e.to_string().contains("192.168"), "sin rutas ni direcciones: {e}");
        // Traer el historial de fuera (no un repositorio de este equipo): sin origen ni mover.
        let o = operacion_historial(&v, &json!({ "repo": "nuevo", "origen": { "destino": {}, "ruta": "x" } }), "nuevo", "N");
        assert_eq!((o.origen.as_deref(), o.mover, o.paso.as_deref()), (None, false, None));
        let e = entrada_historial(&o, &Ok("Historial traído: 3 versiones nuevas.".into()));
        assert_eq!(e["resultado"], "ok");
        assert!(e["mover"].is_null());
    }

    #[test]
    fn rutas() {
        for ok in ["", "Siigo", "ana/portatil", "Copias Siigo"] {
            assert!(ruta_valida(ok), "{ok}");
        }
        for mal in ["..", "a/../b", "a\\b", "-x", "a//b", "c:x", "a@b", "a/./b"] {
            assert!(!ruta_valida(mal), "{mal}");
        }
        assert!(destino_de(&json!({ "tipo": "ftp", "donde": "x" }), "d").is_err());
        assert!(destino_de(&json!({ "tipo": "rest", "donde": "https://x", "ca_pem": "hola" }), "d").is_err());
        assert_eq!(destino_de(&json!({ "tipo": "rest", "donde": "http://192.168.1.30:8001" }), "d").unwrap().nombre, "192.168.1.30:8001");
        let f = Filtro { equipos: vec!["A".into()], etiquetas: vec!["x,y".into()] };
        assert_eq!(f.args(), ["--host", "A", "--tag", "x,y"]);
        assert!(Filtro::de(&json!({ "equipos": ["--borrar"] })).is_err());
    }

    /// Adoptar en un almacén («Copiar en …»): el destino ya lleva el prefijo
    /// del usuario del equipo (`https://ip:puerto/<usuario>/`, como lo da
    /// `guarda_copias {anadir}`), y la `ruta` es solo el nombre de la carpeta
    /// del repositorio dentro de él: lo mismo que usa `crear_repositorio`.
    #[test]
    fn adoptar_en_un_almacen_usa_la_carpeta_del_usuario() {
        let d = destino_de(
            &json!({ "tipo": "rest", "nombre": "ALMACEN-01", "donde": "https://192.168.1.50:8002/servidor-01/", "usuario": "servidor-01", "secreto": "s3creto" }),
            "almacen-1a2b3c4d",
        )
        .unwrap();
        let acc = g::acceso_destino(&d, "siigo", "clave").unwrap();
        assert_eq!(acc.location, "rest:https://192.168.1.50:8002/servidor-01/siigo");
        assert_eq!(acc.rest_auth, Some(("servidor-01".into(), "s3creto".into())));
        // El repositorio que crea «Copiar en …» está al lado, en la misma carpeta del usuario.
        assert_eq!(g::ubicacion(&d, "almacen-ab12").unwrap(), "rest:https://192.168.1.50:8002/servidor-01/almacen-ab12");
        // Con el destino ya guardado en el equipo (`destino: {id}`), basta la carpeta.
        let mut v = Vinculo::default();
        v.destinos.push(d);
        let c = json!({ "solo_probar": true, "destino": { "id": "otro" }, "ruta": "siigo", "contrasena": "x" });
        assert!(adoptar_repositorio(&mut v, &c, false).unwrap_err().contains("No hay ningún destino «otro»"));
        let c = json!({ "solo_probar": true, "destino": { "id": "almacen-1a2b3c4d" }, "ruta": "../beto/siigo", "contrasena": "x" });
        assert!(adoptar_repositorio(&mut v, &c, false).unwrap_err().contains("no es válida"), "no sale de la carpeta del usuario");
    }

    /// Crear en un almacén «para traer el historial» del rest-server antiguo
    /// (sin usuario): el usuario del almacén no se manda al origen.
    #[test]
    fn el_usuario_del_almacen_no_va_al_origen() {
        let mut dest = Access::new("rest:https://192.168.1.50:8002/servidor-01/siigo", "nueva");
        dest.rest_auth = Some(("servidor-01".into(), "s3creto".into()));
        let src = Access::new("rest:http://192.168.1.30:8001/Siigo", "antigua");
        let (both, extra) = con_origen(&dest, &src).unwrap();
        assert!(both.rest_auth.is_none(), "nada de RESTIC_REST_USERNAME/PASSWORD, que restic aplicaría también al origen");
        assert_eq!(both.location, "rest:https://servidor-01:s3creto@192.168.1.50:8002/servidor-01/siigo");
        let from = both.env.iter().find(|(k, _)| k == "RESTIC_FROM_REPOSITORY").map(|(_, x)| x.as_str());
        assert_eq!(from, Some("rest:http://192.168.1.30:8001/Siigo"));
        assert!(extra.is_empty());
        // Con usuario en el origen, cada uno con el suyo.
        let mut src2 = src.clone();
        src2.rest_auth = Some(("ana".into(), "otra".into()));
        let (both, _) = con_origen(&dest, &src2).unwrap();
        let from = both.env.iter().find(|(k, _)| k == "RESTIC_FROM_REPOSITORY").map(|(_, x)| x.clone());
        assert_eq!(from.as_deref(), Some("rest:http://ana:otra@192.168.1.30:8001/Siigo"));
    }

    /// Adoptar (lectura y escritura) un repositorio que ya existe y seguir copiando en él;
    /// la contraseña equivocada no entra y nada secreto sale en los resultados.
    #[test]
    fn adoptar_y_seguir_copiando() {
        if !hay_restic() {
            return; // sin restic (CI de Linux)
        }
        let b = base("rw");
        repo_antiguo(&b.join("Siigo"), "clave antigua de siigo", &b.join("datos"), "PC-CONTABLE");
        let mut v = Vinculo { equipo_id: uuid::Uuid::new_v4().to_string(), modo: "local".into(), ..Default::default() };
        let pedir = |solo: bool, clave: &str| {
            json!({ "id": "siigo", "nombre": "Siigo", "contrasena": clave, "ruta": "Siigo", "solo_probar": solo,
                    "destino": { "id": "d-siigo", "nombre": "Disco", "tipo": "local", "donde": b.display().to_string() } })
        };
        // Contraseña equivocada: no entra, y el mensaje no la repite ni da la ruta.
        let e = adoptar_repositorio(&mut v, &pedir(false, "otra clave"), true).unwrap_err();
        assert!(e.contains("contraseña no abre"), "{e}");
        assert!(!e.contains("otra clave") && !e.contains(&b.display().to_string()), "{e}");
        assert!(v.repos_v2.is_empty() && v.destinos.is_empty(), "no queda nada a medias");
        // Probar: no cambia nada.
        let (m, d) = adoptar_repositorio(&mut v, &pedir(true, "clave antigua de siigo"), true).unwrap();
        assert!(m.contains("1 versión"), "{m}");
        assert_eq!(d["equipos"], json!(["PC-CONTABLE"]));
        assert!(v.repos_v2.is_empty());
        // Sin sellar, sin nombres de equipos.
        let (_, d) = adoptar_repositorio(&mut v, &pedir(true, "clave antigua de siigo"), false).unwrap();
        assert!(d.get("equipos").is_none());
        // Adoptar de verdad.
        let (m, d) = adoptar_repositorio(&mut v, &pedir(false, "clave antigua de siigo"), false).unwrap();
        assert!(m.contains("adoptado") && m.contains("1 versión"), "{m}");
        let texto = format!("{m} {d}");
        assert!(!texto.contains("clave antigua") && !texto.contains(&b.display().to_string()), "sin secretos ni rutas: {texto}");
        assert!(!v.repos_v2[0].solo_lectura, "de lectura y escritura");
        // Otra vez el mismo: ya está en uso.
        let mut otra = pedir(false, "clave antigua de siigo");
        otra["id"] = json!("siigo-2");
        otra["destino"]["id"] = json!("d-2");
        assert!(adoptar_repositorio(&mut v, &otra, false).unwrap_err().contains("ya usa"));
        // Copiar en él (lo que hace cada copia programada) y el historial sigue ahí.
        let acc = g::acceso(&v, "siigo").unwrap();
        assert!(acc.location.ends_with("Siigo"));
        let out = restic::run_raw(&acc, &["backup", "--host", "PC-CONTABLE", &b.join("datos").display().to_string()], ABRIR).unwrap();
        assert_eq!(out.code, Some(0), "{}", out.stderr);
        assert_eq!(restic::snapshots(&acc).unwrap().len(), 2, "la antigua y la nueva");
        let _ = std::fs::remove_dir_all(&b);
    }

    /// Traer el historial de otro repositorio (dos repositorios locales), sin repetir lo ya traído.
    #[test]
    fn traer_historial_entre_dos_repositorios() {
        if !hay_restic() {
            return;
        }
        let b = base("copy");
        repo_antiguo(&b.join("antiguo"), "clave del antiguo", &b.join("datos"), "PC-A");
        std::fs::write(b.join("datos").join("factura.txt"), "factura 2").unwrap();
        let antiguo = Access::new(b.join("antiguo").display().to_string(), "clave del antiguo");
        let out = restic::run_raw(&antiguo, &["backup", "--host", "PC-B", &b.join("datos").display().to_string()], ABRIR).unwrap();
        assert_eq!(out.code, Some(0), "{}", out.stderr);
        // El gestionado, nuevo, con los parámetros de troceado del antiguo.
        let mut v = Vinculo { equipo_id: uuid::Uuid::new_v4().to_string(), modo: "local".into(), ..Default::default() };
        let origen_json = json!({ "destino": local(&b), "ruta": "antiguo", "contrasena": "clave del antiguo" });
        let m = g::crear_repositorio(
            &mut v,
            &json!({ "id": "nuevo", "nombre": "Nuevo", "contrasena": "clave del nuevo", "parametros_de": origen_json,
                     "destino": { "id": "d1", "nombre": "Disco", "tipo": "local", "donde": b.display().to_string() } }),
        )
        .unwrap();
        assert!(m.contains("creado"), "{m}");
        let nuevo = g::acceso(&v, "nuevo").unwrap();
        let cfg = |a: &Access| String::from_utf8(restic::run(a, &["cat", "config", "--no-lock"]).unwrap()).unwrap();
        let pol = |t: String| serde_json::from_str::<Value>(&t).unwrap()["chunker_polynomial"].clone();
        assert_eq!(pol(cfg(&nuevo)), pol(cfg(&antiguo)), "mismos parámetros de troceado");
        // Con contraseña equivocada en el origen: falla sin decirla.
        let malo = origen(&v, &json!({ "destino": local(&b), "ruta": "antiguo", "contrasena": "no es" })).unwrap();
        let e = traer(&v, "nuevo", &malo, &Filtro::default(), &mut |_, _| {}).unwrap_err();
        assert!(e.contains("contraseña no abre") && !e.contains("no es"), "{e}");
        // Solo las de PC-A.
        let src = origen(&v, &origen_json).unwrap();
        let mut visto = Vec::new();
        let m = traer(&v, "nuevo", &src, &Filtro { equipos: vec!["PC-A".into()], ..Default::default() }, &mut |h, t| visto.push((h, t))).unwrap();
        assert!(m.contains("1 versión nueva"), "{m}");
        assert_eq!(visto.last(), Some(&(1, 1)));
        // Todo: solo falta la de PC-B.
        let m = traer(&v, "nuevo", &src, &Filtro::default(), &mut |_, _| {}).unwrap();
        assert!(m.contains("1 versión nueva") && m.contains("1 ya estaba."), "{m}");
        assert_eq!(restic::snapshots(&nuevo).unwrap().len(), 2);
        // Otra vez: nada nuevo.
        let m = traer(&v, "nuevo", &src, &Filtro::default(), &mut |_, _| {}).unwrap();
        assert!(m.contains("Nada nuevo"), "{m}");
        assert!(!m.contains("clave") && !m.contains(&b.display().to_string()));
        // El mismo repositorio como origen y destino: no.
        let mismo = origen(&v, &json!({ "repo": "nuevo" })).unwrap();
        assert!(traer(&v, "nuevo", &mismo, &Filtro::default(), &mut |_, _| {}).unwrap_err().contains("mismo"));

        // En modo local (ventana del equipo): en segundo plano, y la ventana pregunta cómo va.
        assert_eq!(historial_local("nuevo"), Value::Null);
        let c = json!({ "repo": "nuevo", "origen": origen_json });
        let m = copiar_historial_local(&v, &c).unwrap();
        assert!(m.contains("Trayendo el historial a «Nuevo»"), "{m}");
        let mut fin = Value::Null;
        for _ in 0..600 {
            fin = historial_local("nuevo");
            if fin["estado"] != "en_marcha" {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert_eq!(fin["estado"], "hecha", "{fin}");
        assert!(fin["mensaje"].as_str().unwrap().contains("Nada nuevo"), "{fin}");
        // Ya no está «en curso»: se puede pedir otra vez.
        assert!(copiar_historial_local(&v, &json!({ "repo": "otro", "origen": c["origen"] })).unwrap_err().contains("no lo gestiona"));
        let _ = std::fs::remove_dir_all(&b);
    }
}
