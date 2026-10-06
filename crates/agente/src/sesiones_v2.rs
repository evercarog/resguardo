//! Agente v2: sesiones interactivas, restaurar y descargar por el relé
//! (docs/api-servidor.md, §7 y §9; formatos en crates/protocolo/src/simetrico.rs).
//!
//! **Sesión.** La orden que la abre (`elegir_carpetas`, `explorar` o
//! `abrir_sesion`) lleva en su cuerpo sellado `sesion` (el id) y
//! `clave_sesion`. El equipo usa solo el id de dentro del sobre. Cada mensaje
//! es JSON cifrado: de la consola `{ i, op, … }`, del equipo
//! `{ i, re?, op, … | error }`; `i` crece en cada lado (se descartan
//! repeticiones). El equipo empieza con `{ op: "lista" }`.
//!
//! Operaciones: `carpetas {ruta}`, `sugerencias` y `crear_carpeta {ruta, nombre}`
//! (v1.15; solo en `elegir_carpetas`),
//! `versiones`, `listar {version, ruta}` y `buscar {version, texto}` (solo en
//! `explorar`, del repositorio autorizado), y `cerrar`. Dentro de una versión
//! las rutas van como las guarda restic (`/C/Users/Ana`).
//!
//! v1.33 (en `explorar`): `diferencias {hasta, desde?, tamanos?, indice?}`
//! (qué cambió entre dos versiones, por páginas), `ocupa {version}` (lo que
//! más ocupa) e `historial_archivo {ruta}` (en qué versiones está un archivo).
//! Mientras una operación tarda, el equipo manda `{ op: "trabajando", sobre }`
//! cada 20 s (sin `re`: una consola anterior lo ignora).
//!
//! v1.44 (en `explorar`): `buscar_todas {texto, desde?, hasta?, max?, indice?}`
//! (un archivo por su nombre en todas las versiones, agrupado por archivo y
//! por páginas; ver `resguardo_motor::buscar`).

use crate::servidor_v2::{llamar, Vinculo};
use base64::Engine;
use resguardo_motor::{buscar as busqueda, diferencias, restic};
use resguardo_protocolo::simetrico::{self, Lado};
use serde_json::{json, Value};
use std::io::Read;
use std::time::{Duration, Instant};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
/// Sin mensajes de la consola durante este tiempo, la sesión se cierra.
const INACTIVA: Duration = Duration::from_secs(10 * 60);
const MAX_ENTRADAS: usize = 2000;
/// Cada cuánto dice el equipo que sigue con una operación larga.
const LATIDO: Duration = Duration::from_secs(20);
/// Cada página de `diferencias` cabe holgada en un mensaje (256 KiB).
const PAGINA_BYTES: usize = 150 * 1024;
const PAGINA_MAX: usize = 1000;

#[derive(Clone)]
pub enum Tipo {
    /// Elegir carpetas del equipo (nivel administración).
    Carpetas,
    /// Explorar las versiones de un repositorio (su contraseña).
    Explorar(Box<restic::Access>),
    /// Solo progreso (inofensiva).
    Basica,
}

fn clave32(b64: &str) -> Result<[u8; 32], String> {
    B64.decode(b64).ok().and_then(|b| <[u8; 32]>::try_from(b).ok()).ok_or_else(|| "Clave no válida (se esperan 32 bytes en base64).".to_string())
}

/// Abre la sesión del cuerpo de la orden y la atiende en otro hilo.
pub fn abrir(v: &Vinculo, cuerpo: &Value, tipo: Tipo) -> Result<String, String> {
    let id = cuerpo["sesion"].as_str().filter(|s| uuid::Uuid::parse_str(s).is_ok()).ok_or("Falta el id de la sesión.")?.to_string();
    let clave = clave32(cuerpo["clave_sesion"].as_str().unwrap_or(""))?;
    let v = v.clone();
    std::thread::spawn(move || {
        match atender(&v, &id, &clave, &tipo) {
            // La consola cerró la sesión (se salió de la pantalla): no es un error.
            Err(e) if e == SESION_CERRADA => {}
            Err(e) => crate::agent::log(&format!("Sesión con la consola: {e}")),
            Ok(()) => {}
        }
    });
    Ok("Sesión abierta.".into())
}

/// La sesión ya no existe en el servidor (la consola la cerró o caducó).
const SESION_CERRADA: &str = "La sesión con la consola ya se cerró.";

struct Canal<'a> {
    v: &'a Vinculo,
    id: &'a str,
    k_equipo: [u8; 32],
    i: u64,
}

impl Canal<'_> {
    fn enviar(&mut self, mut m: Value) -> Result<(), String> {
        self.i += 1;
        m["i"] = json!(self.i);
        let c = simetrico::cifrar_mensaje(&self.k_equipo, self.id, m.to_string().as_bytes(), &simetrico::nonce_aleatorio());
        let (estado, _) = llamar(self.v, "POST", &format!("/api/agente/sesiones/{}/mensajes", self.id), Some(&json!({ "cifrado": B64.encode(c) })))?;
        if estado == 404 || estado == 410 {
            return Err(SESION_CERRADA.into());
        }
        if estado >= 400 {
            return Err(format!("El servidor no aceptó el mensaje ({estado})."));
        }
        Ok(())
    }
}

fn atender(v: &Vinculo, id: &str, clave: &[u8; 32], tipo: &Tipo) -> Result<(), String> {
    let k_consola = simetrico::clave_direccion(clave, id, Lado::Consola);
    let mut canal = Canal { v, id, k_equipo: simetrico::clave_direccion(clave, id, Lado::Equipo), i: 0 };
    // v1.15: con `ops`, la consola sabe qué puede pedir (sin `ops`: un agente anterior).
    canal.enviar(json!({ "op": "lista", "ops": ops(tipo) }))?;
    let (mut desde, mut ultimo_i, mut actividad) = (0i64, 0u64, Instant::now());
    let mut memoria = Memoria::default();
    loop {
        if actividad.elapsed() > INACTIVA {
            return Ok(());
        }
        let (estado, msgs) = llamar(v, "GET", &format!("/api/agente/sesiones/{id}/mensajes?desde={desde}"), None)?;
        if estado == 404 {
            return Ok(()); // la cerró la consola o caducó
        }
        if estado >= 400 {
            std::thread::sleep(Duration::from_secs(2));
            continue;
        }
        for m in msgs.as_array().cloned().unwrap_or_default() {
            desde = desde.max(m["n"].as_i64().unwrap_or(desde));
            let Ok(cifrado) = B64.decode(m["cifrado"].as_str().unwrap_or("")) else { continue };
            // Lo que no se descifra con la clave de la consola, se descarta.
            let Ok(plano) = simetrico::descifrar_mensaje(&k_consola, id, &cifrado) else { continue };
            let Ok(p) = serde_json::from_slice::<Value>(&plano) else { continue };
            let i = p["i"].as_u64().unwrap_or(0);
            if i <= ultimo_i {
                continue; // repetido
            }
            ultimo_i = i;
            actividad = Instant::now();
            let op = p["op"].as_str().unwrap_or("").to_string();
            if op == "cerrar" {
                return Ok(());
            }
            // En otro hilo: mientras tarda (un `diff` en un repositorio remoto),
            // se avisa a la consola de que sigue (y la sesión no caduca).
            let resultado = std::thread::scope(|hilos| {
                let h = hilos.spawn(|| operar(tipo, &op, &p, &mut memoria));
                let mut ultimo = Instant::now();
                while !h.is_finished() {
                    std::thread::sleep(Duration::from_millis(20));
                    if ultimo.elapsed() >= LATIDO {
                        let _ = canal.enviar(json!({ "op": "trabajando", "sobre": i }));
                        ultimo = Instant::now();
                    }
                }
                h.join().unwrap_or_else(|_| Err("El equipo no pudo terminar la operación.".into()))
            });
            let mut r = match resultado {
                Ok(datos) => datos,
                Err(e) => json!({ "error": e }),
            };
            r["re"] = json!(i);
            r["op"] = json!(op);
            canal.enviar(r)?;
        }
    }
}

/// Lo que se puede pedir en cada tipo de sesión (se anuncia en `lista`).
fn ops(tipo: &Tipo) -> &'static [&'static str] {
    match tipo {
        Tipo::Carpetas => &["carpetas", "sugerencias", "crear_carpeta"],
        Tipo::Explorar(_) => &["versiones", "listar", "buscar", "diferencias", "ocupa", "historial_archivo", "buscar_todas"],
        Tipo::Basica => &[],
    }
}

/// Lo último que se calculó en la sesión (para dar las páginas siguientes sin repetirlo).
#[derive(Default)]
struct Memoria {
    /// (con tamaños, cuándo era `desde`, el cálculo).
    diferencias: Option<(bool, Option<String>, diferencias::Diferencias)>,
    ocupa: Option<(String, Value)>,
    /// (lo que se pidió, el resultado) de `buscar_todas`.
    busqueda: Option<(String, busqueda::Busqueda)>,
}

/// Lo mismo que una sesión, desde la ventana del equipo en modo local
/// (ipc_local, con la clave de administración): elegir carpetas y explorar.
pub fn operar_local(tipo: &Tipo, op: &str, p: &Value) -> Result<Value, String> {
    if !ops(tipo).contains(&op) {
        return Err(format!("Operación no disponible: «{op}»."));
    }
    operar(tipo, op, p, &mut Memoria::default())
}

fn operar(tipo: &Tipo, op: &str, p: &Value, memoria: &mut Memoria) -> Result<Value, String> {
    match (tipo, op) {
        (Tipo::Carpetas, "carpetas") => Ok(json!({ "entradas": carpetas(p["ruta"].as_str().unwrap_or(""))? })),
        (Tipo::Carpetas, "crear_carpeta") => {
            let (ruta, ya) = crear_carpeta(p["ruta"].as_str().unwrap_or(""), p["nombre"].as_str().unwrap_or(""))?;
            Ok(json!({ "ruta": ruta, "ya_existia": ya }))
        }
        (Tipo::Carpetas, "sugerencias") => Ok(json!({ "sugerencias": sugerencias() })),
        (Tipo::Explorar(acc), "versiones") => Ok(json!({ "versiones": versiones(acc)? })),
        (Tipo::Explorar(acc), "listar") => {
            let version = p["version"].as_str().unwrap_or("");
            let ruta = p["ruta"].as_str().unwrap_or("/");
            let entradas = restic::list_dir(acc, version, ruta)?;
            Ok(json!({ "entradas": entradas.iter().take(MAX_ENTRADAS).map(|e| json!({
                "nombre": e.name, "tipo": if e.kind == "dir" { "dir" } else { "archivo" }, "bytes": e.size, "modificado": e.mtime,
            })).collect::<Vec<_>>() }))
        }
        (Tipo::Explorar(acc), "buscar") => Ok(json!({ "resultados": buscar(acc, p["version"].as_str().unwrap_or(""), p["texto"].as_str().unwrap_or(""))? })),
        (Tipo::Explorar(acc), "diferencias") => que_cambio(acc, p, memoria),
        (Tipo::Explorar(acc), "ocupa") => ocupa(acc, p["version"].as_str().unwrap_or(""), memoria),
        (Tipo::Explorar(acc), "historial_archivo") => {
            let ruta = p["ruta"].as_str().unwrap_or("");
            Ok(json!({ "ruta": ruta, "versiones": diferencias::versiones_de_archivo(acc, ruta)? }))
        }
        (Tipo::Explorar(acc), "buscar_todas") => buscar_todas(acc, p, memoria, busqueda::TIEMPO),
        _ => Err(format!("Operación no disponible en esta sesión: «{op}».")),
    }
}

/// Contenido de una carpeta del equipo (vacía: las unidades, o `/`).
fn carpetas(ruta: &str) -> Result<Vec<Value>, String> {
    if ruta.trim().is_empty() {
        #[cfg(windows)]
        {
            return Ok((b'A'..=b'Z')
                .map(|l| format!("{}:\\", l as char))
                .filter(|d| std::path::Path::new(d).exists())
                .map(|d| json!({ "nombre": d, "tipo": "dir" }))
                .collect());
        }
        #[cfg(not(windows))]
        {
            return carpetas("/");
        }
    }
    let p = std::path::Path::new(ruta);
    if !p.is_absolute() || ruta.contains('\0') {
        return Err("Ruta no válida.".into());
    }
    let mut out: Vec<(bool, String, Value)> = Vec::new();
    for e in std::fs::read_dir(p).map_err(|e| format!("No se pudo abrir la carpeta: {e}"))?.flatten() {
        let Ok(m) = std::fs::symlink_metadata(e.path()) else { continue };
        let nombre = e.file_name().to_string_lossy().into_owned();
        let dir = m.is_dir();
        let sistema = nombre.starts_with('$')
            || nombre.starts_with('.')
            || ["System Volume Information", "Windows", "ProgramData", "Recovery", "proc", "sys", "dev", "run"].contains(&nombre.as_str());
        let modificado = m.modified().ok().map(|t| chrono::DateTime::<chrono::Local>::from(t).to_rfc3339());
        let mut v = json!({ "nombre": nombre, "tipo": if dir { "dir" } else { "archivo" }, "bytes": (!dir).then_some(m.len()), "modificado": modificado, "sistema": sistema });
        // Una pista para «Usar uno que ya existe»: la carpeta parece un repositorio
        // de copias (solo se mira si existen sus piezas; no se abre nada). Los
        // navegadores que no la conocen la ignoran.
        if dir && parece_repositorio(&e.path()) {
            v["repositorio"] = json!(true);
        }
        out.push((dir, nombre.to_lowercase(), v));
        if out.len() >= MAX_ENTRADAS {
            break;
        }
    }
    out.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    Ok(out.into_iter().map(|x| x.2).collect())
}

/// ¿Parece `dir` un repositorio de restic? Tiene el archivo `config` y las
/// carpetas `data`, `index`, `keys` y `snapshots` (sin seguir enlaces). Primero
/// `config`: en una carpeta normal es una sola consulta al disco.
fn parece_repositorio(dir: &std::path::Path) -> bool {
    let es = |n: &str, carpeta: bool| std::fs::symlink_metadata(dir.join(n)).is_ok_and(|m| if carpeta { m.is_dir() } else { m.is_file() });
    es("config", false) && ["data", "index", "keys", "snapshots"].iter().all(|n| es(n, true))
}

/// ¿Vale `nombre` como nombre de una carpeta nueva? (una sola parte, sin
/// caracteres que Windows no admite, sin nombres reservados).
fn nombre_carpeta_valido(nombre: &str) -> bool {
    let n = nombre.chars().count();
    if n == 0 || n > 100 || nombre.trim() != nombre || nombre == "." || nombre == ".." || nombre.ends_with('.') {
        return false;
    }
    if nombre.chars().any(|c| c.is_control() || "<>:\"/\\|?*".contains(c)) {
        return false;
    }
    // Windows: ni CON, PRN, AUX, NUL, COM1…9 ni LPT1…9 (con o sin extensión).
    let base = nombre.split('.').next().unwrap_or("").to_ascii_uppercase();
    let b = base.as_bytes();
    let reservado = ["CON", "PRN", "AUX", "NUL"].contains(&base.as_str())
        || (b.len() == 4 && (base.starts_with("COM") || base.starts_with("LPT")) && b[3].is_ascii_digit() && b[3] != b'0');
    !reservado
}

/// ¿Está `ruta` en una carpeta del sistema, de los programas o de Resguardo?
/// Ahí no se crea nada.
fn carpeta_del_sistema(ruta: &str) -> bool {
    let r = ruta.trim_end_matches(['\\', '/']).to_lowercase();
    let prohibidas: Vec<String> = if cfg!(windows) {
        let env = |k: &str, defecto: &str| std::env::var(k).unwrap_or_else(|_| defecto.to_string()).to_lowercase();
        let datos = env("ProgramData", r"C:\ProgramData");
        vec![
            env("SystemRoot", r"C:\Windows"),
            env("ProgramFiles", r"C:\Program Files"),
            env("ProgramFiles(x86)", r"C:\Program Files (x86)"),
            format!(r"{datos}\resguardoagente"),
            format!(r"{datos}\resguardo"),
            format!(r"{datos}\resguardo server"),
        ]
    } else {
        ["/proc", "/sys", "/dev", "/run", "/boot", "/bin", "/sbin", "/lib", "/lib64", "/usr", "/etc", "/var/lib/resguardo-agente", "/opt/resguardo-agente"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    };
    let sep = if cfg!(windows) { '\\' } else { '/' };
    prohibidas.iter().any(|p| r == *p || r.starts_with(&format!("{p}{sep}")))
}

/// `crear_carpeta {ruta, nombre}` (v1.15): crea `nombre` dentro de `ruta`, de
/// un disco del equipo (la raíz de una unidad vale como sitio). Las mismas
/// reglas que las carpetas del Servidor de copias, del espejo y de los
/// volcados: nada de red, enlaces ni carpetas del sistema o de Resguardo.
/// Devuelve la ruta nueva y si ya existía (entonces solo se elige).
fn crear_carpeta(padre: &str, nombre: &str) -> Result<(String, bool), String> {
    if !nombre_carpeta_valido(nombre) {
        return Err(r#"Ese nombre no vale para una carpeta: hasta 100 caracteres, sin \ / : * ? " < > | ni punto al final."#.into());
    }
    let p = std::path::Path::new(padre);
    let b = padre.as_bytes();
    let raiz_windows = cfg!(windows) && b.len() == 3 && b[0].is_ascii_alphabetic() && padre.ends_with(":\\");
    if padre.chars().any(char::is_control) || !p.is_absolute() || padre.starts_with(r"\\") || padre.starts_with("//") {
        return Err("Ruta no válida.".into());
    }
    if !raiz_windows && padre != "/" {
        crate::platform::carpeta_local_valida(padre)?;
    }
    match std::fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() && !crate::platform::is_reparse_point(p) => {}
        _ => return Err("La carpeta donde crearla ya no existe o es un enlace.".into()),
    }
    let nueva = p.join(nombre);
    let texto = nueva.display().to_string();
    crate::platform::carpeta_local_valida(&texto)?;
    if carpeta_del_sistema(&texto) {
        return Err("Aquí no se pueden crear carpetas (es del sistema, de los programas o de Resguardo): elige otro sitio.".into());
    }
    match std::fs::create_dir(&nueva) {
        Ok(()) => Ok((texto, false)),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => match std::fs::symlink_metadata(&nueva) {
            Ok(m) if m.is_dir() && !crate::platform::is_reparse_point(&nueva) => Ok((texto, true)),
            _ => Err("Ya hay un archivo o un enlace con ese nombre: elige otro.".into()),
        },
        Err(e) => Err(format!("No se pudo crear la carpeta: {e}")),
    }
}

/// Carpetas que suele interesar copiar.
fn sugerencias() -> Vec<Value> {
    let mut out = Vec::new();
    #[cfg(windows)]
    {
        if let Ok(it) = std::fs::read_dir(r"C:\Users") {
            for u in it.flatten() {
                let nombre = u.file_name().to_string_lossy().into_owned();
                if ["Public", "Default", "Default User", "All Users"].contains(&nombre.as_str()) || !u.path().is_dir() {
                    continue;
                }
                let rutas: Vec<String> =
                    ["Documents", "Desktop", "Pictures"].iter().map(|s| u.path().join(s)).filter(|p| p.is_dir()).map(|p| p.display().to_string()).collect();
                if !rutas.is_empty() {
                    out.push(json!({ "id": format!("usuario-{nombre}"), "nombre": format!("Documentos de {nombre}"), "detalle": "Documentos, Escritorio e Imágenes", "rutas": rutas }));
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
        for (id, nombre, ruta) in [("home", "Carpetas personales", "/home"), ("etc", "Configuración del sistema", "/etc"), ("www", "Sitios web", "/var/www")] {
            if std::path::Path::new(ruta).is_dir() {
                out.push(json!({ "id": id, "nombre": nombre, "rutas": [ruta] }));
            }
        }
    }
    out
}

fn versiones(acc: &restic::Access) -> Result<Vec<Value>, String> {
    let mut s = restic::snapshots(acc)?;
    s.sort_by(|a, b| b.time.cmp(&a.time));
    Ok(s.iter()
        .map(|x| {
            json!({
                "id": x.id, "cuando": x.time, "etiquetas": x.tags,
                "archivos": x.summary.as_ref().and_then(|s| s.total_files_processed),
                "bytes": x.summary.as_ref().and_then(|s| s.total_bytes_processed),
            })
        })
        .collect())
}

/// `diferencias {hasta, desde?, tamanos?, indice?}`: qué cambió de `desde` a
/// `hasta` (sin `desde`: la versión anterior de la misma copia). Se calcula una
/// vez y se da por páginas (`indice`: el primer cambio; `siguiente`: el de la
/// página siguiente o `null`). Los recuentos (`resumen`) son de todo.
fn que_cambio(acc: &restic::Access, p: &Value, memoria: &mut Memoria) -> Result<Value, String> {
    let hasta = p["hasta"].as_str().unwrap_or("");
    if !restic::valid_snapshot_id(hasta) {
        return Err("Versión no válida.".into());
    }
    let tamanos = p["tamanos"].as_bool().unwrap_or(true);
    let indice = usize::try_from(p["indice"].as_u64().unwrap_or(0)).unwrap_or(usize::MAX);
    let pedido = p["desde"].as_str().filter(|d| !d.is_empty());
    if pedido.is_some_and(|d| !restic::valid_snapshot_id(d)) {
        return Err("Versión no válida.".into());
    }
    // Lo ya calculado vale para las páginas siguientes (y para la misma pareja).
    let ya = memoria
        .diferencias
        .as_ref()
        .is_some_and(|(t, cuando, d)| (*t || !tamanos) && mismo(&d.hasta, hasta) && pedido.map_or(cuando.is_some(), |x| mismo(&d.desde, x)));
    if !ya {
        let (desde, cuando) = match pedido {
            Some(d) => (d.to_string(), None),
            None => {
                let versiones = restic::snapshots(acc)?;
                if !versiones.iter().any(|s| mismo(&s.id, hasta)) {
                    return Err("Esa versión ya no está en el repositorio.".into());
                }
                match diferencias::anterior(&versiones, hasta) {
                    Some(s) => (s.short_id.clone(), Some(s.time.clone())),
                    // La primera versión de su copia: todo lo que tiene es nuevo.
                    None => return Ok(json!({ "hasta": hasta, "desde": null, "primera": true })),
                }
            }
        };
        memoria.diferencias = None;
        let d = diferencias::diferencias(acc, &desde, hasta, tamanos)?;
        memoria.diferencias = Some((tamanos, cuando, d));
    }
    let (_, cuando, d) = memoria.diferencias.as_ref().ok_or("Sin diferencias.")?;
    Ok(pagina(d, cuando.as_deref(), indice))
}

/// ¿El mismo id de versión (uno puede ser el corto)?
fn mismo(a: &str, b: &str) -> bool {
    a.len() >= 8 && b.len() >= 8 && (a.starts_with(b) || b.starts_with(a))
}

/// Una página de cambios desde `indice`, que quepa en un mensaje.
fn pagina(d: &diferencias::Diferencias, desde_cuando: Option<&str>, indice: usize) -> Value {
    let mut cambios = Vec::new();
    let mut ocupa = 0usize;
    let mut i = indice.min(d.cambios.len());
    while i < d.cambios.len() && cambios.len() < PAGINA_MAX {
        let c = serde_json::to_value(&d.cambios[i]).unwrap_or(Value::Null);
        let tam = c.to_string().len() + 1;
        if !cambios.is_empty() && ocupa + tam > PAGINA_BYTES {
            break;
        }
        ocupa += tam;
        cambios.push(c);
        i += 1;
    }
    json!({
        "desde": d.desde, "hasta": d.hasta, "desde_cuando": desde_cuando, "resumen": d.resumen, "total": d.cambios.len(),
        "recortado": d.recortado, "con_tamanos": d.con_tamanos, "indice": indice, "siguiente": (i < d.cambios.len()).then_some(i), "cambios": cambios,
    })
}

/// `ocupa {version}`: las carpetas y los archivos que más ocupan de una versión (50 de cada).
fn ocupa(acc: &restic::Access, version: &str, memoria: &mut Memoria) -> Result<Value, String> {
    if let Some((v, r)) = &memoria.ocupa {
        if v == version {
            return Ok(r.clone());
        }
    }
    let l = resguardo_motor::sizes::largest(acc, version, 50)?;
    let item = |x: &resguardo_motor::sizes::SizeItem| json!({ "ruta": x.path, "bytes": x.size, "archivos": x.files });
    let r = json!({
        "version": version, "total_bytes": l.total_size, "total_archivos": l.total_files,
        "carpetas": l.folders.iter().map(item).collect::<Vec<_>>(), "archivos": l.files.iter().map(item).collect::<Vec<_>>(),
    });
    memoria.ocupa = Some((version.to_string(), r.clone()));
    Ok(r)
}

/// `buscar_todas {texto, desde?, hasta?, max?, indice?}`: los archivos cuyo
/// nombre lleva `texto` en las versiones de `desde` a `hasta` (RFC 3339), con
/// las versiones en las que está cada uno. Se busca con `indice` 0 (o sin él)
/// y se da por páginas (`indice` de la página, `siguiente` o `null`); las
/// páginas siguientes salen de lo ya encontrado si se pide lo mismo.
fn buscar_todas(acc: &restic::Access, p: &Value, memoria: &mut Memoria, limite: Duration) -> Result<Value, String> {
    let texto = busqueda::texto_valido(p["texto"].as_str().unwrap_or(""))?;
    let (desde, hasta) = (p["desde"].as_str(), p["hasta"].as_str());
    busqueda::rango(desde, hasta)?;
    let max = match (&p["max"], p["max"].as_u64().and_then(|n| usize::try_from(n).ok())) {
        (Value::Null, _) => busqueda::MAX_COINCIDENCIAS,
        (_, Some(n)) if (1..=busqueda::MAX_COINCIDENCIAS).contains(&n) => n,
        _ => return Err(format!("«max» no válido (de 1 a {}).", busqueda::MAX_COINCIDENCIAS)),
    };
    let indice = match (&p["indice"], p["indice"].as_u64()) {
        (Value::Null, _) => 0,
        (_, Some(i)) => usize::try_from(i).unwrap_or(usize::MAX),
        _ => return Err("«indice» no válido.".into()),
    };
    let clave = json!([texto, desde.unwrap_or(""), hasta.unwrap_or(""), max]).to_string();
    let ya = memoria.busqueda.as_ref().is_some_and(|(k, _)| *k == clave);
    if indice == 0 || !ya {
        memoria.busqueda = None;
        let b = busqueda::buscar(acc, &texto, desde, hasta, max, limite)?;
        memoria.busqueda = Some((clave, b));
    }
    let (_, b) = memoria.busqueda.as_ref().ok_or("Sin búsqueda.")?;
    Ok(pagina_busqueda(b, indice))
}

/// Una página de archivos encontrados desde `indice`, que quepa en un mensaje.
fn pagina_busqueda(b: &busqueda::Busqueda, indice: usize) -> Value {
    let mut archivos = Vec::new();
    let mut ocupa = 0usize;
    let mut i = indice.min(b.archivos.len());
    while i < b.archivos.len() && archivos.len() < PAGINA_MAX {
        let a = serde_json::to_value(&b.archivos[i]).unwrap_or(Value::Null);
        let tam = a.to_string().len() + 1;
        if !archivos.is_empty() && ocupa + tam > PAGINA_BYTES {
            break;
        }
        ocupa += tam;
        archivos.push(a);
        i += 1;
    }
    json!({
        "texto": b.texto, "total_archivos": b.archivos.len(), "coincidencias": b.coincidencias,
        "versiones_buscadas": b.versiones_buscadas, "versiones_en_rango": b.versiones_en_rango,
        "recortado": b.recortado, "motivo": b.motivo, "indice": indice, "siguiente": (i < b.archivos.len()).then_some(i), "archivos": archivos,
    })
}

fn buscar(acc: &restic::Access, version: &str, texto: &str) -> Result<Vec<Value>, String> {
    if !restic::valid_snapshot_id(version) {
        return Err("Versión no válida.".into());
    }
    let texto = texto.trim();
    if texto.is_empty() || texto.chars().count() > 100 || texto.chars().any(char::is_control) {
        return Err("Escribe qué buscar (hasta 100 caracteres).".into());
    }
    // Los caracteres especiales de los patrones pasan a `*` (con `[` restic daba
    // «syntax error in pattern»); luego, solo lo que se llama así de verdad.
    let patron = busqueda::patron(texto);
    let aguja = texto.to_lowercase();
    let out = restic::run(acc, &["find", "--json", "--no-lock", "--ignore-case", "--snapshot", version, "--", &patron])?;
    let v: Value = serde_json::from_slice(&out).unwrap_or(Value::Null);
    let mut res = Vec::new();
    for snap in v.as_array().cloned().unwrap_or_default() {
        for m in snap["matches"].as_array().cloned().unwrap_or_default() {
            if !busqueda::nombre_coincide(m["path"].as_str().unwrap_or(""), &aguja) {
                continue;
            }
            res.push(json!({
                "ruta": m["path"], "nombre": m["name"], "tipo": if m["type"] == "dir" { "dir" } else { "archivo" }, "bytes": m["size"], "modificado": m["mtime"],
            }));
            if res.len() >= 500 {
                return Ok(res);
            }
        }
    }
    Ok(res)
}

// ---------- Restaurar ----------

/// Ruta de una versión (`/C/Users/Ana`) como ruta del equipo (`C:\Users\Ana`).
/// En Windows solo `/<letra>/…` (las rutas que guarda restic): nada de
/// `//equipo/recurso` (SYSTEM se autenticaría en un equipo ajeno) ni `\` o `:`
/// dentro de un nombre (otra raíz o un flujo alternativo), ni nombres hechos
/// solo de puntos o que acaban en punto o espacio (Windows los recorta: `...`
/// o `a.` no serían lo que parecen) ni nombres de dispositivo (`CON`, `NUL`,
/// `COM1.txt`…: se escribiría en el dispositivo, no en un archivo). En los dos,
/// absoluta, sin `.`, `..` ni NUL.
pub fn ruta_local(ruta_version: &str) -> Result<String, String> {
    let no_valida = || format!("Ruta no válida en este equipo: {ruta_version}");
    if ruta_version.contains('\0') {
        return Err(no_valida());
    }
    if cfg!(windows) {
        let resto = ruta_version.strip_prefix('/').ok_or_else(no_valida)?;
        let (unidad, r) = resto.split_once('/').unwrap_or((resto, ""));
        if unidad.len() != 1 || !unidad.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(no_valida());
        }
        let nombre_malo = |c: &str| c.is_empty() || c.contains(['\\', ':']) || c.ends_with(['.', ' ']) || dispositivo_de_windows(c);
        if !r.is_empty() && r.split('/').any(nombre_malo) {
            return Err(no_valida());
        }
        Ok(format!("{unidad}:\\{}", r.replace('/', "\\")))
    } else {
        if !ruta_version.starts_with('/') || ruta_version.split('/').any(|c| c == "." || c == "..") {
            return Err(no_valida());
        }
        Ok(ruta_version.to_string())
    }
}

/// ¿Es un nombre que Windows convierte en un dispositivo? `CON`, `PRN`, `AUX`,
/// `NUL`, `COM0`–`COM9`, `LPT0`–`LPT9` (también con los dígitos ¹²³),
/// `CONIN$` y `CONOUT$`, sin distinguir mayúsculas y aunque lleven extensión
/// o espacios antes de ella (`nul.txt`, `CON .log`).
fn dispositivo_de_windows(nombre: &str) -> bool {
    let base = nombre.split('.').next().unwrap_or("").trim_end_matches(' ').to_uppercase();
    if matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$") {
        return true;
    }
    let mut c = base.chars();
    let prefijo: String = c.by_ref().take(3).collect();
    let resto: Vec<char> = c.collect();
    (prefijo == "COM" || prefijo == "LPT") && resto.len() == 1 && (resto[0].is_ascii_digit() || matches!(resto[0], '¹' | '²' | '³'))
}

/// ¿Está `ruta` (local) dentro de alguna carpeta de las copias de `repo` en este equipo?
fn dentro_de_copias(repo: &str, ruta: &str) -> bool {
    let config = crate::agent::load_config();
    let carpetas: Vec<String> =
        config.repos.iter().filter(|r| r.id == repo).flat_map(|r| r.plans.iter().flat_map(|p| p.paths.iter()).chain(r.paths.iter())).cloned().collect();
    dentro(ruta, &carpetas)
}

fn dentro(ruta: &str, carpetas: &[String]) -> bool {
    let normal = |s: &str| {
        let s = s.trim_end_matches(['\\', '/']).to_string();
        if cfg!(windows) {
            s.to_lowercase()
        } else {
            s
        }
    };
    let ruta = normal(ruta);
    if ruta.split(['\\', '/']).any(|s| s == "..") {
        return false;
    }
    carpetas.iter().any(|c| {
        let c = normal(c);
        !c.is_empty() && std::path::Path::new(&ruta).starts_with(std::path::Path::new(&c))
    })
}

/// Carpeta y nombre de una ruta de versión (al menos dos niveles: nada de raíces).
fn partes(ruta: &str) -> Result<(String, String), String> {
    let r = restic::snapshot_dir(ruta)?;
    let (padre, nombre) = r.rsplit_once('/').ok_or("Ruta no válida.")?;
    if nombre.is_empty() || padre.is_empty() {
        return Err(format!("No se puede restaurar una unidad o la raíz entera: {r}"));
    }
    Ok((padre.to_string(), nombre.to_string()))
}

/// «En otra carpeta» (solo la ventana del equipo): una carpeta que ya existe en
/// un disco de este equipo, sin enlaces en su camino (un usuario podría haber
/// puesto una unión hacia la carpeta de Windows), sin nombres que Windows
/// recorta o interpreta (`a.`, `a `, `a:flujo`, `..`) y que no sea del sistema,
/// de los programas ni de Resguardo, tampoco escrita con nombres cortos
/// (`C:\PROGRA~1`): se mira también su ruta real.
fn carpeta_destino(d: &str) -> Result<std::path::PathBuf, String> {
    let mal = || "Elige una carpeta de un disco de este equipo donde restaurar (que no sea del sistema ni de los programas).".to_string();
    if d.is_empty() || d.chars().any(char::is_control) || d.len() > 1024 {
        return Err(mal());
    }
    if cfg!(windows) {
        let b = d.as_bytes();
        if b.len() < 3 || !b[0].is_ascii_alphabetic() || b[1] != b':' || b[2] != b'\\' {
            return Err(mal());
        }
        let resto = d[3..].trim_end_matches('\\');
        if !resto.is_empty()
            && resto.split('\\').any(|c| {
                c.is_empty()
                    || c == "."
                    || c == ".."
                    || c.ends_with(['.', ' '])
                    || c.contains(['/', ':', '*', '?', '"', '<', '>', '|'])
                    || dispositivo_de_windows(c)
            })
        {
            return Err(mal());
        }
    } else if !d.starts_with('/') || d.starts_with("//") || d.split('/').any(|c| c == "." || c == "..") {
        return Err(mal());
    }
    let p = std::path::Path::new(d);
    if crate::platform::hay_enlace_en_el_camino(p) {
        return Err("Esa carpeta es un enlace o está dentro de uno: elige una carpeta normal.".into());
    }
    match std::fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() => {}
        _ => return Err("Esa carpeta no existe (o no es una carpeta): elígela con el explorador.".into()),
    }
    let real = std::fs::canonicalize(p).map_err(|_| mal())?;
    let real = real.to_string_lossy();
    if real.starts_with(r"\\?\UNC\") || carpeta_del_sistema(d) || carpeta_del_sistema(real.strip_prefix(r"\\?\").unwrap_or(&real)) {
        return Err(mal());
    }
    Ok(p.to_path_buf())
}

/// La carpeta «Restaurado …» dentro de `padre`: **nueva** (si ya hay algo con
/// ese nombre, «… (2)», «… (3)»…), nunca una que ya estaba (podría ser una
/// unión que puso otro usuario, con el nombre adivinado por la hora), y con
/// `padre` sin enlaces en su camino. Con `dueno` nace solo para SYSTEM,
/// Administradores y esa cuenta (ver [`crate::platform::crear_carpeta_nueva`]).
fn carpeta_restaurado(padre: &std::path::Path, nombre: &str, dueno: Option<&str>) -> Result<std::path::PathBuf, String> {
    match std::fs::symlink_metadata(padre) {
        Ok(m) if m.is_dir() => {}
        _ => return Err(format!("La carpeta donde restaurar ya no existe: {}", padre.display())),
    }
    if crate::platform::hay_enlace_en_el_camino(padre) {
        return Err(format!("{} es un enlace o está dentro de uno: no se restaura ahí.", padre.display()));
    }
    for n in 1..=50 {
        let p = if n == 1 { padre.join(nombre) } else { padre.join(format!("{nombre} ({n})")) };
        match crate::platform::crear_carpeta_nueva(&p, dueno) {
            Ok(()) => return Ok(p),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("No se pudo crear la carpeta donde restaurar: {e}")),
        }
    }
    Err("Ya hay demasiadas carpetas «Restaurado» con esa hora: vuelve a intentarlo en un minuto.".into())
}

/// `restaurar {repo, version, rutas, destino: "junto"|"original"|"carpeta", carpeta?, reemplazar}`.
/// «junto»: en una carpeta nueva «Restaurado …» al lado de cada ruta; «original»:
/// en su sitio (sin `reemplazar`, nunca sobrescribe); «carpeta» (solo la ventana
/// del equipo): en una carpeta nueva «Restaurado …» dentro de la que se elija.
/// `dueno`: la cuenta que lo pide en el equipo (la ventana), si se sabe.
pub fn restaurar(acc: &restic::Access, c: &Value, dueno: Option<&str>) -> Result<String, String> {
    let version = c["version"].as_str().unwrap_or("");
    if !restic::valid_snapshot_id(version) {
        return Err("Versión no válida.".into());
    }
    let rutas: Vec<String> = c["rutas"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
    if rutas.is_empty() || rutas.len() > 100 {
        return Err("Elige entre 1 y 100 archivos o carpetas.".into());
    }
    // v1.36 (solo la ventana del equipo, con la clave): «carpeta», en la que elija el administrador.
    let carpeta = match c["destino"].as_str() {
        Some("carpeta") => Some(carpeta_destino(c["carpeta"].as_str().unwrap_or(""))?),
        _ => None,
    };
    let junto = match c["destino"].as_str() {
        Some("junto") | Some("carpeta") | None => true,
        Some("original") => false,
        Some(_) => return Err("Destino no válido («junto» u «original»).".into()),
    };
    let reemplazar = !junto && c["reemplazar"].as_bool().unwrap_or(false);
    // «En su sitio» solo dentro de las carpetas que copia este equipo en ese
    // repositorio: una versión con rutas inventadas (quien tiene la contraseña
    // del repositorio puede crearla) no escribe como SYSTEM en otra parte del
    // disco (p. ej. en la carpeta de Windows).
    if !junto {
        let repo = c["repo"].as_str().unwrap_or("");
        if let Some(r) = rutas.iter().find(|r| !ruta_local(r).is_ok_and(|l| dentro_de_copias(repo, &l))) {
            return Err(format!("«{r}» no está en las carpetas que copia este equipo: restáuralo junto al original."));
        }
    }
    // Para la ventana y los avisos del escritorio (el nombre del repositorio, sin rutas).
    let repo = c["repo"].as_str().unwrap_or("");
    let nombre = crate::agent::load_config().repos.iter().find(|r| r.id == repo).map(|r| r.name.clone()).unwrap_or_else(|| "Copias".into());
    let guarda = crate::escritorio::en_marcha::empezar("restauracion", repo, &nombre);
    let sello = chrono::Local::now().format("%Y-%m-%d %H%M").to_string();
    let mut destinos = Vec::new();
    // Las carpetas «Restaurado …» ya creadas en esta restauración (una por cada carpeta de arriba).
    let mut creadas: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
    for ruta in &rutas {
        let (padre, nombre) = partes(ruta)?;
        let mut objetivo = match &carpeta {
            Some(d) => d.clone(),
            None => std::path::PathBuf::from(ruta_local(&padre)?),
        };
        if junto {
            objetivo = match creadas.iter().find(|(arriba, _)| *arriba == objetivo) {
                Some((_, hecha)) => hecha.clone(),
                None => {
                    let hecha = carpeta_restaurado(&objetivo, &format!("Restaurado {sello}"), dueno)?;
                    creadas.push((objetivo, hecha.clone()));
                    hecha
                }
            };
        } else if crate::platform::hay_enlace_en_el_camino(&objetivo) {
            // En su sitio: la carpeta (o una de arriba) se cambió por un enlace después de la copia.
            return Err(format!("«{ruta}»: su carpeta en el equipo es ahora un enlace; restáuralo junto al original."));
        }
        let destino = objetivo.display().to_string();
        let origen = format!("{version}:{padre}");
        let incluir = format!("/{nombre}");
        // Con `--json`, restic dice cuánto lleva: la ventana del equipo lo enseña (escritura en el disco).
        let mut al_avanzar = |l: &str| {
            if let Ok(v) = serde_json::from_str::<Value>(l) {
                if v["message_type"] == "status" {
                    guarda.progreso(v["bytes_restored"].as_u64(), v["total_bytes"].as_u64());
                }
            }
        };
        let out = restic::run_raw_lines(
            acc,
            &["restore", &origen, "--target", &destino, "--include", &incluir, "--overwrite", if reemplazar { "always" } else { "never" }, "--json"],
            Duration::from_secs(24 * 3600),
            &mut al_avanzar,
        )?;
        if out.code != Some(0) {
            return Err(restic::exit_error(out.code, &out.stderr));
        }
        if !destinos.contains(&destino) {
            destinos.push(destino);
        }
    }
    guarda.terminar("ok");
    // Rutas entre comillas: así se quitan enteras (con espacios) donde no deben verse.
    Ok(format!("Restaurado ({} elementos) en «{}».", rutas.len(), destinos.join("», «")))
}

// ---------- Descargar por el relé ----------

/// `descargar {repo, version, rutas, formato: "archivo"|"zip", relevo: {id, clave}}`,
/// cifrado en trozos de 4 MiB. Una ruta: `restic dump` (el archivo tal cual,
/// o la carpeta en zip). Varias (solo en zip): se restauran en una carpeta
/// temporal del agente, se comprimen y se borra todo al terminar.
/// Devuelve el número de trozos.
pub fn descargar(v: &Vinculo, acc: &restic::Access, c: &Value) -> Result<u64, String> {
    let version = c["version"].as_str().unwrap_or("");
    if !restic::valid_snapshot_id(version) {
        return Err("Versión no válida.".into());
    }
    let rutas: Vec<String> = c["rutas"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
    if rutas.is_empty() || rutas.len() > 100 {
        return Err("Elige entre 1 y 100 archivos o carpetas.".into());
    }
    let rutas = rutas.iter().map(|r| restic::snapshot_dir(r)).collect::<Result<Vec<_>, _>>()?;
    let relevo = c["relevo"]["id"].as_str().filter(|s| uuid::Uuid::parse_str(s).is_ok()).ok_or("Falta el relé.")?.to_string();
    let clave = clave32(c["relevo"]["clave"].as_str().unwrap_or(""))?;
    let zip = match c["formato"].as_str().unwrap_or("archivo") {
        "archivo" => false,
        "zip" => true,
        _ => return Err("Formato no válido («archivo» o «zip»).".into()),
    };
    if rutas.len() > 1 && !zip {
        return Err("Varios archivos o carpetas se descargan en un zip.".into());
    }
    if rutas.len() == 1 {
        let mut args: Vec<String> = vec!["dump".into(), "--no-lock".into()];
        if zip {
            args.extend(["--archive".into(), "zip".into()]);
        }
        args.extend([version.to_string(), rutas[0].clone()]);
        let mut hijo = restic::spawn(acc, &args)?;
        // stderr en otro hilo: si se llenara, restic se pararía.
        let mut stderr = hijo.stderr.take();
        let errores = std::thread::spawn(move || {
            let mut t = String::new();
            if let Some(e) = stderr.as_mut() {
                let _ = e.read_to_string(&mut t);
            }
            t
        });
        let mut salida = hijo.stdout.take().ok_or("restic no dio salida.")?;
        let r = subir(v, &relevo, &clave, &mut salida);
        if r.is_err() {
            let _ = hijo.kill();
        }
        let estado = hijo.wait().map_err(|e| e.to_string())?;
        let err = errores.join().unwrap_or_default();
        let n = r?;
        if !estado.success() {
            return Err(restic::exit_error(estado.code(), &err));
        }
        return Ok(n);
    }
    // Varias rutas: restaurar en una carpeta temporal y comprimir.
    let tmp = crate::agent::private_dir().join(format!("descarga-{relevo}"));
    let _ = std::fs::remove_dir_all(&tmp);
    let r = (|| {
        let datos = tmp.join("datos");
        std::fs::create_dir_all(&datos).map_err(|e| e.to_string())?;
        // Por carpeta de origen, `version:carpeta` con `--include /nombre` (como «restaurar»):
        // nunca `version:/`, que restauraba también las carpetas de arriba (C:\, C:\Users…)
        // con sus permisos de Windows; sin ser administrador, la carpeta temporal quedaba
        // sin permiso de escritura y restic fallaba con «acceso denegado». Y el zip lleva
        // lo elegido, no la ruta entera del equipo.
        let grupos = por_carpeta(&rutas)?;
        let mut usados: Vec<String> = Vec::new();
        for (padre, nombres) in &grupos {
            let destino = if grupos.len() == 1 { datos.clone() } else { datos.join(nombre_de_grupo(padre, &mut usados)) };
            std::fs::create_dir_all(&destino).map_err(|e| e.to_string())?;
            let mut args: Vec<String> = vec!["restore".into(), format!("{version}:{padre}"), "--target".into(), destino.display().to_string()];
            for n in nombres {
                args.extend(["--include".into(), format!("/{n}")]);
            }
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            let out = restic::run_raw(acc, &refs, Duration::from_secs(24 * 3600))?;
            if out.code != Some(0) {
                return Err(restic::exit_error(out.code, &out.stderr));
            }
        }
        let archivo = tmp.join("descarga.zip");
        comprimir(&datos, &archivo)?;
        let mut f = std::fs::File::open(&archivo).map_err(|e| e.to_string())?;
        subir(v, &relevo, &clave, &mut f)
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    r
}

/// Las rutas de una descarga, juntas por su carpeta (en el orden en que llegan).
fn por_carpeta(rutas: &[String]) -> Result<Vec<(String, Vec<String>)>, String> {
    let mut grupos: Vec<(String, Vec<String>)> = Vec::new();
    for ruta in rutas {
        let (padre, nombre) = partes(ruta)?;
        match grupos.iter_mut().find(|(p, _)| *p == padre) {
            Some((_, n)) => n.push(nombre),
            None => grupos.push((padre, vec![nombre])),
        }
    }
    Ok(grupos)
}

/// La carpeta del zip para lo de `padre` cuando vienen de varias: su último nombre
/// (sin caracteres que Windows no admite), sin repetir.
fn nombre_de_grupo(padre: &str, usados: &mut Vec<String>) -> String {
    let base: String = padre.rsplit('/').find(|s| !s.is_empty()).unwrap_or("carpeta").chars().map(|c| if "<>:\"|?*\\".contains(c) { '_' } else { c }).collect();
    let mut nombre = base.clone();
    let mut n = 2;
    while usados.iter().any(|u| u.eq_ignore_ascii_case(&nombre)) {
        nombre = format!("{base} ({n})");
        n += 1;
    }
    usados.push(nombre.clone());
    nombre
}

/// Un zip con todo lo de `dir` (rutas relativas, con `/`).
pub fn comprimir(dir: &std::path::Path, destino: &std::path::Path) -> Result<(), String> {
    use std::io::Write;
    let f = std::fs::File::create(destino).map_err(|e| e.to_string())?;
    let mut z = zip::ZipWriter::new(std::io::BufWriter::new(f));
    let opciones = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).large_file(true);
    let mut pendientes = vec![dir.to_path_buf()];
    while let Some(d) = pendientes.pop() {
        let mut hijos: Vec<_> = std::fs::read_dir(&d).map_err(|e| e.to_string())?.flatten().collect();
        hijos.sort_by_key(|e| e.file_name());
        for e in hijos {
            let p = e.path();
            let rel = p.strip_prefix(dir).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
            let m = std::fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
            if m.is_dir() {
                z.add_directory(format!("{rel}/"), opciones).map_err(|e| e.to_string())?;
                pendientes.push(p);
            } else if m.is_file() {
                z.start_file(rel, opciones).map_err(|e| e.to_string())?;
                let mut origen = std::fs::File::open(&p).map_err(|e| e.to_string())?;
                std::io::copy(&mut origen, &mut z).map_err(|e| e.to_string())?;
            }
        }
    }
    z.finish().map_err(|e| e.to_string())?.flush().map_err(|e| e.to_string())
}

/// Sube lo que se lee de `datos` al relé, en trozos cifrados, y lo cierra.
fn subir(v: &Vinculo, relevo: &str, clave: &[u8; 32], datos: &mut dyn Read) -> Result<u64, String> {
    let leer = |s: &mut dyn Read| -> Result<Vec<u8>, String> {
        let mut buf = Vec::with_capacity(simetrico::TROZO);
        s.take(simetrico::TROZO as u64).read_to_end(&mut buf).map_err(|e| e.to_string())?;
        Ok(buf)
    };
    let agente = crate::servidor_v2::agente_de(v)?;
    let auth = format!("Equipo {}:{}", v.equipo_id, v.secreto);
    let (mut n, mut total) = (0u64, 0u64);
    let mut actual = leer(datos)?;
    loop {
        let siguiente = if actual.len() == simetrico::TROZO { leer(datos)? } else { Vec::new() };
        let ultimo = siguiente.is_empty();
        let trozo = simetrico::cifrar_trozo(clave, relevo, n, ultimo, &actual, &simetrico::nonce_aleatorio());
        let url = format!("{}/api/agente/relevos/{relevo}/trozos/{n}", v.url.trim_end_matches('/'));
        let r = agente.put(&url).header("authorization", &auth).send(&trozo[..]).map_err(|e| format!("No se pudo subir al relé: {e}"))?;
        if r.status().as_u16() >= 400 {
            return Err(format!("El relé no aceptó el trozo {n} ({}): la descarga puede ser demasiado grande.", r.status().as_u16()));
        }
        total += trozo.len() as u64;
        n += 1;
        if ultimo {
            break;
        }
        actual = siguiente;
    }
    crate::servidor_v2::llamar_ok(v, &format!("/api/agente/relevos/{relevo}/fin"), &json!({ "trozos": n, "bytes": total }))?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn en_su_sitio_solo_dentro_de_las_copias() {
        let carpetas = vec![r"D:\WO\Datos".to_string(), r"C:\Users\Ana\Documents\".to_string()];
        assert!(dentro(r"D:\WO\Datos\Empresa1\x.db", &carpetas));
        assert!(dentro(r"c:\users\ana\documents\carta.docx", &carpetas));
        assert!(!dentro(r"C:\Windows\System32\x.dll", &carpetas));
        assert!(!dentro(r"D:\WO\DatosOtros\x", &carpetas));
        assert!(!dentro(r"D:\WO\Datos\..\..\Windows\x", &carpetas));
        assert!(!dentro(r"D:\WO\Datos\x", &[]));
    }

    #[test]
    fn descarga_de_varios_por_carpeta() {
        let rutas: Vec<String> = ["/C/Datos/a.txt", "/C/Datos/b.txt", "/D/Otra/Datos/c.txt", "/E/x/y"].iter().map(|s| s.to_string()).collect();
        let g = por_carpeta(&rutas).unwrap();
        assert_eq!(g[0], ("/C/Datos".to_string(), vec!["a.txt".to_string(), "b.txt".to_string()]));
        assert_eq!(g[1].0, "/D/Otra/Datos");
        assert_eq!(g.len(), 3);
        // Dos carpetas «Datos»: la segunda, «Datos (2)», en el zip.
        let mut usados = Vec::new();
        assert_eq!(nombre_de_grupo(&g[0].0, &mut usados), "Datos");
        assert_eq!(nombre_de_grupo(&g[1].0, &mut usados), "Datos (2)");
        assert_eq!(nombre_de_grupo("/C/a:b", &mut usados), "a_b");
        // Una unidad entera no se descarga así (como al restaurar).
        assert!(por_carpeta(&["/C".to_string()]).is_err());
    }

    #[test]
    fn paginas_de_diferencias() {
        use resguardo_motor::diferencias::{Cambio, Diferencias, Resumen};
        let ruta = |i: usize| format!("/C/Datos/{}/archivo-{i:05}.txt", "x".repeat(200));
        let d = Diferencias {
            desde: "aaaaaaaa".into(),
            hasta: "bbbbbbbb".into(),
            resumen: Resumen { nuevos: 3000, ..Resumen::default() },
            cambios: (0..3000).map(|i| Cambio { ruta: ruta(i), tipo: "nuevo", bytes: Some(i as u64), bytes_antes: None }).collect(),
            recortado: false,
            con_tamanos: true,
        };
        let (mut indice, mut vistos, mut paginas) = (0usize, 0usize, 0);
        loop {
            let p = pagina(&d, Some("2026-10-01T10:00:00Z"), indice);
            assert!(p.to_string().len() < 200 * 1024, "cabe en un mensaje");
            let n = p["cambios"].as_array().unwrap().len();
            assert!(n > 0 && n <= PAGINA_MAX);
            assert_eq!(p["cambios"][0]["ruta"], ruta(indice));
            assert_eq!(p["total"], 3000);
            vistos += n;
            paginas += 1;
            match p["siguiente"].as_u64() {
                Some(s) => indice = s as usize,
                None => break,
            }
        }
        assert_eq!(vistos, 3000);
        assert!(paginas > 3, "{paginas} páginas");
        // Más allá del final: vacía, sin siguiente.
        let p = pagina(&d, None, 99_999);
        assert!(p["cambios"].as_array().unwrap().is_empty() && p["siguiente"].is_null());
        assert!(mismo("aaaaaaaa", "aaaaaaaabbbb") && !mismo("aaaaaaaa", "aaaaaaab") && !mismo("", ""));
    }

    /// Las operaciones nuevas de la sesión con restic de verdad (el del PATH):
    /// sin `desde`, la anterior de la misma copia; la primera no tiene; lo que
    /// más ocupa y las versiones de un archivo.
    #[test]
    fn detalle_con_restic_de_verdad() {
        if restic::version().is_err() {
            eprintln!("omitido: no hay restic");
            return;
        }
        let base = std::env::temp_dir().join(format!("resguardo-sesion-dif-{}", uuid::Uuid::new_v4().simple()));
        let datos = base.join("datos");
        std::fs::create_dir_all(&datos).unwrap();
        std::fs::write(datos.join("a.txt"), "uno").unwrap();
        let acc = restic::Access::new(base.join("repo").display().to_string(), "contraseña");
        let lim = Duration::from_secs(120);
        assert_eq!(restic::run_raw(&acc, &["init"], lim).unwrap().code, Some(0));
        let d = datos.display().to_string();
        assert_eq!(restic::run_raw(&acc, &["backup", &d], lim).unwrap().code, Some(0));
        std::fs::write(datos.join("a.txt"), "uno y dos").unwrap();
        std::fs::write(datos.join("b.txt"), "nuevo").unwrap();
        assert_eq!(restic::run_raw(&acc, &["backup", &d], lim).unwrap().code, Some(0));
        let mut vs = restic::snapshots(&acc).unwrap();
        vs.sort_by(|a, b| a.time.cmp(&b.time));
        let tipo = Tipo::Explorar(Box::new(acc));
        let mut m = Memoria::default();

        let r = operar(&tipo, "diferencias", &json!({ "hasta": vs[1].short_id }), &mut m).unwrap();
        assert_eq!(r["desde"], vs[0].short_id, "{r}");
        assert!(r["desde_cuando"].is_string() && r["siguiente"].is_null());
        assert_eq!((r["resumen"]["nuevos"].as_u64(), r["resumen"]["cambiados"].as_u64()), (Some(1), Some(1)));
        let a = r["cambios"].as_array().unwrap().iter().find(|c| c["ruta"].as_str().unwrap().ends_with("/a.txt")).unwrap().clone();
        assert_eq!((a["tipo"].as_str(), a["bytes_antes"].as_u64(), a["bytes"].as_u64()), (Some("cambiado"), Some(3), Some(9)));
        // La primera versión de su copia: nada con qué compararla.
        let p = operar(&tipo, "diferencias", &json!({ "hasta": vs[0].short_id }), &mut m).unwrap();
        assert_eq!(p["primera"], true);

        let o = operar(&tipo, "ocupa", &json!({ "version": vs[1].short_id }), &mut m).unwrap();
        assert_eq!(o["total_archivos"], 2);
        let h = operar(&tipo, "historial_archivo", &json!({ "ruta": a["ruta"] }), &mut m).unwrap();
        assert_eq!(h["versiones"].as_array().unwrap().len(), 2);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn operaciones_nuevas_validan_lo_que_piden() {
        let acc = restic::Access::new("C:/no-existe", "x");
        let tipo = Tipo::Explorar(Box::new(acc));
        let mut m = Memoria::default();
        assert!(ops(&tipo).contains(&"diferencias") && ops(&tipo).contains(&"ocupa") && ops(&tipo).contains(&"historial_archivo"));
        for (op, p) in [
            ("diferencias", json!({ "hasta": "--help" })),
            ("diferencias", json!({ "hasta": "aaaaaaaa", "desde": "-x" })),
            ("ocupa", json!({ "version": "../x" })),
            ("historial_archivo", json!({ "ruta": "/C/../Windows" })),
            ("historial_archivo", json!({ "ruta": "relativa.txt" })),
        ] {
            let e = operar(&tipo, op, &p, &mut m).unwrap_err();
            assert!(e.contains("no válida"), "{op}: {e}");
        }
        // Fuera de «explorar», no.
        assert!(operar(&Tipo::Carpetas, "diferencias", &json!({ "hasta": "aaaaaaaa" }), &mut m).is_err());
    }

    #[test]
    fn buscar_todas_valida_lo_que_pide() {
        let tipo = Tipo::Explorar(Box::new(restic::Access::new("C:/no-existe", "x")));
        let mut m = Memoria::default();
        assert!(ops(&tipo).contains(&"buscar_todas"));
        for (p, error) in [
            (json!({ "texto": "" }), "Escribe qué buscar"),
            (json!({ "texto": "a" }), "Escribe qué buscar"),
            (json!({ "texto": "x".repeat(101) }), "Escribe qué buscar"),
            (json!({ "texto": "../x" }), "sin / ni"),
            (json!({ "texto": "fac\u{1}tura" }), "sin / ni"),
            (json!({ "texto": "factura", "desde": "ayer" }), "Fecha no válida"),
            (json!({ "texto": "factura", "desde": "2026-10-03T00:00:00Z", "hasta": "2026-10-01T00:00:00Z" }), "posterior"),
            (json!({ "texto": "factura", "max": 0 }), "«max» no válido"),
            (json!({ "texto": "factura", "max": 2001 }), "«max» no válido"),
            (json!({ "texto": "factura", "max": "10" }), "«max» no válido"),
            (json!({ "texto": "factura", "indice": -1 }), "«indice» no válido"),
        ] {
            let e = operar(&tipo, "buscar_todas", &p, &mut m).unwrap_err();
            assert!(e.contains(error), "{p}: {e}");
        }
        assert!(operar(&Tipo::Carpetas, "buscar_todas", &json!({ "texto": "factura" }), &mut m).is_err());
    }

    #[test]
    fn paginas_de_la_busqueda() {
        use resguardo_motor::buscar::{Busqueda, Encontrado, Motivo};
        use resguardo_motor::diferencias::EnVersion;
        let v = |i: usize| EnVersion {
            version: format!("{i:08x}"),
            cuando: "2026-10-01T10:00:00Z".into(),
            bytes: Some(i as u64),
            modificado: Some("2026-10-01T09:00:00Z".into()),
        };
        let b = Busqueda {
            texto: "factura".into(),
            archivos: (0..400)
                .map(|i| Encontrado { ruta: format!("/C/Datos/{}/factura-{i:04}.pdf", "x".repeat(150)), versiones: (0..5).map(v).collect(), recortado: false })
                .collect(),
            coincidencias: 2000,
            versiones_buscadas: 5,
            versiones_en_rango: 5,
            recortado: true,
            motivo: Some(Motivo::Limite),
        };
        let (mut indice, mut vistos, mut paginas) = (0usize, 0usize, 0);
        loop {
            let p = pagina_busqueda(&b, indice);
            assert!(p.to_string().len() < 200 * 1024, "cabe en un mensaje");
            assert_eq!((p["total_archivos"].as_u64(), p["motivo"].as_str(), p["recortado"].as_bool()), (Some(400), Some("limite"), Some(true)));
            let n = p["archivos"].as_array().unwrap().len();
            assert!(n > 0);
            assert_eq!(p["archivos"][0]["ruta"], b.archivos[indice].ruta);
            assert_eq!(p["archivos"][0]["versiones"].as_array().unwrap().len(), 5);
            vistos += n;
            paginas += 1;
            match p["siguiente"].as_u64() {
                Some(s) => indice = s as usize,
                None => break,
            }
        }
        assert_eq!(vistos, 400);
        assert!(paginas > 1, "{paginas} páginas");
        let p = pagina_busqueda(&b, 99_999);
        assert!(p["archivos"].as_array().unwrap().is_empty() && p["siguiente"].is_null());
    }

    /// `buscar_todas` y `buscar` (en una versión) con restic de verdad: un
    /// nombre con corchetes, en dos versiones y con dos tamaños.
    #[test]
    fn buscar_con_restic_de_verdad() {
        if restic::version().is_err() {
            eprintln!("omitido: no hay restic");
            return;
        }
        let base = std::env::temp_dir().join(format!("resguardo-sesion-buscar-{}", uuid::Uuid::new_v4().simple()));
        let datos = base.join("datos");
        std::fs::create_dir_all(datos.join("Facturas [2026]")).unwrap();
        std::fs::write(datos.join("Facturas [2026]").join("Factura [1].txt"), "uno").unwrap();
        let acc = restic::Access::new(base.join("repo").display().to_string(), "contraseña");
        let lim = Duration::from_secs(120);
        assert_eq!(restic::run_raw(&acc, &["init"], lim).unwrap().code, Some(0));
        let d = datos.display().to_string();
        assert_eq!(restic::run_raw(&acc, &["backup", &d], lim).unwrap().code, Some(0));
        std::fs::write(datos.join("Facturas [2026]").join("Factura [1].txt"), "uno y dos").unwrap();
        assert_eq!(restic::run_raw(&acc, &["backup", &d], lim).unwrap().code, Some(0));
        let vs = restic::snapshots(&acc).unwrap();
        let tipo = Tipo::Explorar(Box::new(acc));
        let mut m = Memoria::default();
        let r = operar(&tipo, "buscar_todas", &json!({ "texto": "factura [1]" }), &mut m).unwrap();
        assert_eq!((r["total_archivos"].as_u64(), r["versiones_buscadas"].as_u64(), r["siguiente"].is_null()), (Some(1), Some(2), true), "{r}");
        let a = &r["archivos"][0];
        assert!(a["ruta"].as_str().unwrap().ends_with("/Facturas [2026]/Factura [1].txt"), "{a}");
        assert_eq!(a["versiones"].as_array().unwrap().iter().map(|v| v["bytes"].as_u64().unwrap()).collect::<Vec<_>>(), [9, 3]);
        // La página siguiente sale de lo ya encontrado (sin otro `find`).
        assert!(m.busqueda.is_some());
        let r2 = operar(&tipo, "buscar_todas", &json!({ "texto": "factura [1]", "indice": 1 }), &mut m).unwrap();
        assert!(r2["archivos"].as_array().unwrap().is_empty() && r2["total_archivos"] == 1);
        // En una versión: con corchetes ya no falla, y solo lo que se llama así (no lo de dentro de la carpeta).
        let r = operar(&tipo, "buscar", &json!({ "version": vs[0].short_id, "texto": "[1]" }), &mut m).unwrap();
        assert_eq!(r["resultados"].as_array().unwrap().len(), 1, "{r}");
        let r = operar(&tipo, "buscar", &json!({ "version": vs[0].short_id, "texto": "Facturas" }), &mut m).unwrap();
        assert_eq!(r["resultados"].as_array().unwrap().iter().map(|x| x["tipo"].as_str().unwrap()).collect::<Vec<_>>(), ["dir"], "{r}");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn nombres_de_carpeta_nueva() {
        for bien in ["Resguardo", "Copias 2026", "Espejo-Sur", "a.b", "Ñandú", "COM0", "CONSOLA"] {
            assert!(nombre_carpeta_valido(bien), "{bien}");
        }
        for mal in ["", " x", "x ", ".", "..", "a/b", r"a\b", "a:b", "a*", "a?", "a\"b", "a<b", "a|b", "fin.", "CON", "con.txt", "LPT1", "nul", "x\u{7}"] {
            assert!(!nombre_carpeta_valido(mal), "{mal}");
        }
        assert!(!nombre_carpeta_valido(&"x".repeat(101)));
    }

    #[test]
    fn crear_carpeta_nueva_y_reglas() {
        let base = std::env::temp_dir().join(format!("resguardo-crear-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let padre = base.display().to_string();
        let (ruta, ya) = crear_carpeta(&padre, "Nueva").unwrap();
        assert!(!ya && std::path::Path::new(&ruta).is_dir());
        // Otra vez: ya existe, solo se elige.
        assert!(crear_carpeta(&padre, "Nueva").unwrap().1);
        // Un archivo con ese nombre: no.
        std::fs::write(base.join("archivo"), b"x").unwrap();
        assert!(crear_carpeta(&padre, "archivo").is_err());
        // Nombres y sitios que no valen.
        assert!(crear_carpeta(&padre, "a/b").is_err());
        assert!(crear_carpeta("relativa", "x").is_err());
        assert!(crear_carpeta(&base.join("no-existe").display().to_string(), "x").is_err());
        if cfg!(windows) {
            assert!(crear_carpeta(r"\\nas\copias", "x").is_err());
            assert!(carpeta_del_sistema(r"C:\Windows\Temp\x"));
            assert!(carpeta_del_sistema(r"c:\program files\Resguardo"));
            assert!(carpeta_del_sistema(r"C:\ProgramData\ResguardoAgente\x"));
            assert!(!carpeta_del_sistema(r"E:\Resguardo"));
            assert!(!carpeta_del_sistema(r"C:\WindowsCopias"));
        } else {
            assert!(carpeta_del_sistema("/etc/x") && carpeta_del_sistema("/proc"));
            assert!(!carpeta_del_sistema("/srv/copias"));
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    /// «En otra carpeta» (la ventana del equipo): solo carpetas normales de un
    /// disco del equipo, nunca del sistema, ni por un enlace ni con nombres
    /// que Windows recorta o interpreta.
    #[test]
    fn restaurar_en_otra_carpeta_solo_donde_se_puede() {
        let base = std::env::temp_dir().join(format!("resguardo-destino-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("Destino")).unwrap();
        let bien = base.join("Destino").display().to_string();
        assert!(carpeta_destino(&bien).is_ok(), "{bien}");
        assert!(carpeta_destino(&base.join("no-existe").display().to_string()).is_err());
        assert!(carpeta_destino("").is_err());
        assert!(carpeta_destino("relativa").is_err());
        if cfg!(windows) {
            for mal in [
                r"C:\Windows\System32".to_string(),
                r"c:\windows".to_string(),
                r"C:\Program Files".to_string(),
                r"C:\ProgramData\ResguardoAgente".to_string(),
                r"\\?\C:\Windows\System32".to_string(),
                r"\\nas\copias".to_string(),
                "C:/Windows/System32".to_string(),
                format!(r"{bien}\..\..\..\Windows"),
                format!(r"{bien}\."),
                format!("{bien}."),
                format!("{bien} "),
                format!("{bien}:flujo"),
            ] {
                assert!(carpeta_destino(&mal).is_err(), "{mal}");
            }
            // Con su nombre corto (8.3), la carpeta de los programas sigue siendo del sistema.
            let corto = std::process::Command::new("cmd").args(["/c", "for %I in (\"C:\\Program Files\") do @echo %~sI"]).output().ok();
            if let Some(c) = corto.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|c| c.contains('~')) {
                assert!(carpeta_destino(&c).is_err(), "{c}");
            }
            // Una unión (la puede crear cualquier usuario) hacia otra carpeta: no.
            let union = base.join("Union");
            let ok =
                std::process::Command::new("cmd").args(["/c", "mklink", "/J"]).arg(&union).arg(base.join("Destino")).output().is_ok_and(|o| o.status.success());
            if ok {
                assert!(carpeta_destino(&union.display().to_string()).unwrap_err().contains("enlace"));
                std::fs::create_dir_all(union.join("dentro")).ok();
                assert!(carpeta_destino(&union.join("dentro").display().to_string()).is_err());
            }
        } else {
            assert!(carpeta_destino("/etc").is_err() && carpeta_destino("/usr/lib").is_err());
            assert!(carpeta_destino(&format!("{bien}/../../../etc")).is_err());
            #[cfg(unix)]
            {
                let enlace = base.join("Enlace");
                if std::os::unix::fs::symlink(base.join("Destino"), &enlace).is_ok() {
                    assert!(carpeta_destino(&enlace.display().to_string()).is_err());
                }
            }
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    /// La carpeta «Restaurado …» siempre es nueva: si alguien dejó antes una
    /// con ese nombre (o una unión, adivinando la hora), se usa otra.
    #[test]
    fn restaurado_siempre_en_una_carpeta_nueva() {
        let base = std::env::temp_dir().join(format!("resguardo-restaurado-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("Otra")).unwrap();
        let nombre = "Restaurado 2026-10-05 1200";
        let a = carpeta_restaurado(&base, nombre, None).unwrap();
        assert_eq!(a, base.join(nombre));
        // Ya existe (la de antes, o una puesta a propósito): otra.
        let b = carpeta_restaurado(&base, nombre, None).unwrap();
        assert_eq!(b, base.join(format!("{nombre} (2)")));
        if cfg!(windows) {
            let trampa = format!("{nombre} (3)");
            let ok = std::process::Command::new("cmd")
                .args(["/c", "mklink", "/J"])
                .arg(base.join(&trampa))
                .arg(base.join("Otra"))
                .output()
                .is_ok_and(|o| o.status.success());
            if ok {
                let c = carpeta_restaurado(&base, nombre, None).unwrap();
                assert_eq!(c, base.join(format!("{nombre} (4)")), "la unión no se usa");
                assert!(!crate::platform::hay_enlace_en_el_camino(&c));
                // Y dentro de una unión, tampoco.
                assert!(carpeta_restaurado(&base.join(&trampa), nombre, None).is_err());
                assert!(std::fs::read_dir(base.join("Otra")).unwrap().next().is_none(), "nada escrito por la unión");
            }
        }
        assert!(carpeta_restaurado(&base.join("no-existe"), nombre, None).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Con la cuenta de quien la pide (la ventana), la carpeta nace solo para
    /// SYSTEM, Administradores y esa cuenta: quien la pide puede usarla.
    #[cfg(windows)]
    #[test]
    fn restaurado_con_dueno() {
        let base = std::env::temp_dir().join(format!("resguardo-dueno-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let sid = std::process::Command::new("whoami")
            .args(["/user", "/fo", "csv", "/nh"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().rsplit(',').next().map(|s| s.trim_matches('"').to_string()))
            .filter(|s| crate::platform::sid_valido(s));
        if let Some(sid) = sid {
            let d = carpeta_restaurado(&base, "Restaurado 2026-10-05 1200", Some(&sid)).unwrap();
            std::fs::write(d.join("x.txt"), b"x").unwrap();
            assert_eq!(std::fs::read(d.join("x.txt")).unwrap(), b"x");
        }
        // Un «SID» que no lo es no entra en los permisos.
        assert!(carpeta_restaurado(&base, "Otra", Some("S-1-5-21)(A;;FA;;;WD")).is_err());
        assert!(!crate::platform::sid_valido("S-1-5-21)(A;;FA;;;WD") && crate::platform::sid_valido("S-1-5-21-1-2-3-1001"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn nombres_de_dispositivo_de_windows() {
        for d in ["CON", "con", "Nul.txt", "AUX .log", "com1", "LPT9.tar.gz", "COM¹", "conin$", "CONOUT$"] {
            assert!(dispositivo_de_windows(d), "{d}");
        }
        for n in ["CONSOLA", "nulo.txt", "COM", "COM10", "LPTX", "com1a", "a.CON", "PRNT", ""] {
            assert!(!dispositivo_de_windows(n), "{n}");
        }
    }

    #[test]
    fn rutas_de_version_a_rutas_del_equipo() {
        if cfg!(windows) {
            assert_eq!(ruta_local("/C/Users/Ana").unwrap(), r"C:\Users\Ana");
            assert_eq!(ruta_local("/D").unwrap(), r"D:\");
            // Rutas de red, de otra raíz o con flujos alternativos: nunca.
            for mala in ["//equipo/recurso/x", "/host/recurso", r"/C/a\..\b", "/C/a:flujo", "/C//x", "/C/./x", "/1/x", "C/x", "/"] {
                assert!(ruta_local(mala).is_err(), "{mala}");
            }
            // Nombres que Windows recorta (solo puntos, o con punto o espacio al final) y NUL.
            for mala in ["/C/.../x", "/C/a./b", "/C/a /b", "/C/x\0y"] {
                assert!(ruta_local(mala).is_err(), "{mala}");
            }
            // Nombres de dispositivo: se escribiría en el dispositivo, no en un archivo.
            for mala in ["/C/x/CON", "/C/nul.txt", "/C/x/com1/y", "/C/LPT9.log"] {
                assert!(ruta_local(mala).is_err(), "{mala}");
            }
            assert!(ruta_local("/C/x/CONSOLA.txt").is_ok());
        } else {
            assert_eq!(ruta_local("/home/ana").unwrap(), "/home/ana");
            for mala in ["home/ana", "/home/../etc", "/home/./ana", "/x\0y", ""] {
                assert!(ruta_local(mala).is_err(), "{mala}");
            }
        }
        assert_eq!(partes("/C/Users/Ana/Doc.txt").unwrap(), ("/C/Users/Ana".to_string(), "Doc.txt".to_string()));
        assert!(partes("/").is_err());
        assert!(partes("/C/../x").is_err());
    }

    #[test]
    fn zip_de_una_carpeta() {
        let dir = std::env::temp_dir().join(format!("resguardo-zip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("datos/sub")).unwrap();
        std::fs::write(dir.join("datos/a.txt"), b"uno").unwrap();
        std::fs::write(dir.join("datos/sub/b.txt"), b"dos").unwrap();
        comprimir(&dir.join("datos"), &dir.join("x.zip")).unwrap();
        let mut z = zip::ZipArchive::new(std::fs::File::open(dir.join("x.zip")).unwrap()).unwrap();
        let mut b = String::new();
        z.by_name("sub/b.txt").unwrap().read_to_string(&mut b).unwrap();
        assert_eq!(b, "dos");
        assert!(z.by_name("a.txt").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn marca_las_carpetas_que_parecen_un_repositorio() {
        let base = std::env::temp_dir().join(format!("resguardo-marca-repo-{}", uuid::Uuid::new_v4().simple()));
        let repo = base.join("Contabilidad");
        for d in ["data", "index", "keys", "snapshots", "locks"] {
            std::fs::create_dir_all(repo.join(d)).unwrap();
        }
        std::fs::write(repo.join("config"), b"x").unwrap();
        // Le falta `keys`: no es un repositorio.
        let medio = base.join("Medio");
        for d in ["data", "index", "snapshots"] {
            std::fs::create_dir_all(medio.join(d)).unwrap();
        }
        std::fs::write(medio.join("config"), b"x").unwrap();
        std::fs::create_dir_all(base.join("Fotos")).unwrap();
        let l = carpetas(&base.display().to_string()).unwrap();
        let marca = |n: &str| l.iter().find(|e| e["nombre"] == n).map(|e| e["repositorio"] == true).unwrap();
        assert!(marca("Contabilidad"));
        assert!(!marca("Medio"));
        assert!(!marca("Fotos"));
        assert!(l.iter().find(|e| e["nombre"] == "Fotos").unwrap().get("repositorio").is_none(), "sin la pista, el campo no va");
        let _ = std::fs::remove_dir_all(&base);
    }
}
