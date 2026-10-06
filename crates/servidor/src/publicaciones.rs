//! El servidor como espejo de las publicaciones del agente
//! (docs/actualizaciones.md §8) y la política de actualización de cada
//! cliente y equipo (§5).
//!
//! El servidor **nunca firma**: acepta una publicación solo si su manifiesto
//! viene firmado con una de las llaves fijadas al compilar
//! (`packaging/llave-publicacion.pub`) y cada archivo coincide con su SHA-256
//! y su tamaño; la sirve a sus equipos tal cual (byte a byte). La protección
//! de verdad es la del agente, que lo vuelve a comprobar todo.
//!
//! En disco: `publicaciones/agente/<versión>/` con el manifiesto, su firma y
//! los archivos, solo para el usuario del servicio. Se guardan las últimas
//! [`MAX_GUARDADAS`].
//!
//! La política se guarda en la tabla de valores del servidor (`act:…`): la del
//! cliente (`modo`, `dias_general`, `ventana`, `aprobada`, `retenidas`) y la de
//! cada equipo (`anillo`, `aprobada`).

use crate::almacen::{Almacen, ClienteCtx, Rol, R};
use crate::auth::Usuario;
use crate::error::{ErrorApi, Res};
use crate::estado::{Opciones, St};
use axum::body::Body;
use axum::extract::{Path as Ruta, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures_util::StreamExt;
use resguardo_protocolo::publicacion::{self as p, Llaves, Manifiesto, Politica, Ventana};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Versiones que se guardan (las más nuevas).
pub const MAX_GUARDADAS: usize = 3;

/// Las llaves fijadas al compilar.
pub const LLAVES_FIJADAS: &str = include_str!("../../../packaging/llave-publicacion.pub");

/// Las llaves en las que confía el servidor: las fijadas y, **solo en una
/// compilación de desarrollo**, las de pruebas de `Opciones::llaves_pruebas`
/// (`RESGUARDO_LLAVES_PRUEBAS`). Una compilación de publicación nunca las mira.
pub fn llaves(o: &Opciones) -> Llaves {
    let mut l = Llaves::leer(LLAVES_FIJADAS).unwrap_or_default();
    if cfg!(debug_assertions) {
        if let Some(extra) = o.llaves_pruebas.as_deref().and_then(|t| Llaves::leer(t).ok()) {
            l.anadir(extra);
        }
    }
    l
}

/// La carpeta de las publicaciones del agente.
pub fn carpeta(datos: &Path) -> PathBuf {
    datos.join("publicaciones").join("agente")
}

/// Una publicación guardada.
#[derive(Clone, Debug)]
pub struct Guardada {
    pub manifiesto: Manifiesto,
    /// El manifiesto y la firma tal cual se firmaron.
    pub texto: String,
    pub firma: String,
    pub dir: PathBuf,
    /// Los archivos que aún faltan.
    pub faltan: Vec<String>,
}

impl Guardada {
    pub fn completa(&self) -> bool {
        self.faltan.is_empty()
    }

    pub fn vista(&self) -> Value {
        let m = &self.manifiesto;
        json!({
            "version": m.version, "fecha": m.fecha, "notas": m.notas, "minimo_desde": m.minimo_desde,
            "completa": self.completa(),
            "archivos": m.archivos.iter().map(|a| json!({
                "plataforma": a.plataforma, "nombre": a.nombre, "tamano": a.tamano, "sha256": a.sha256,
                "presente": !self.faltan.contains(&a.nombre),
            })).collect::<Vec<_>>(),
        })
    }
}

/// ¿Está ese archivo con su tamaño y su SHA-256?
fn archivo_bien(ruta: &Path, a: &p::Archivo) -> bool {
    let Ok(meta) = std::fs::symlink_metadata(ruta) else { return false };
    if !meta.is_file() || meta.len() != a.tamano {
        return false;
    }
    sha256_de(ruta).is_ok_and(|h| h.eq_ignore_ascii_case(&a.sha256))
}

fn sha256_de(ruta: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(ruta)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Lee una versión guardada y vuelve a comprobar su firma (lo que no se
/// comprueba no se sirve). Los archivos se comprueban por tamaño; el SHA-256,
/// al guardarlos (y el agente, al bajarlos).
fn leer_guardada(dir: &Path, llaves: &Llaves) -> Option<Guardada> {
    let texto = std::fs::read_to_string(dir.join(p::NOMBRE_MANIFIESTO)).ok()?;
    let firma = std::fs::read_to_string(dir.join(p::NOMBRE_FIRMA)).ok()?;
    let v = p::verificar(texto.as_bytes(), &firma, llaves, &[], p::PRODUCTO_AGENTE).ok()?;
    if dir.file_name().and_then(|n| n.to_str()) != Some(v.manifiesto.version.as_str()) {
        return None;
    }
    let faltan = v
        .manifiesto
        .archivos
        .iter()
        .filter(|a| std::fs::symlink_metadata(dir.join(&a.nombre)).map(|m| !m.is_file() || m.len() != a.tamano).unwrap_or(true))
        .map(|a| a.nombre.clone())
        .collect();
    Some(Guardada { manifiesto: v.manifiesto, texto, firma, dir: dir.to_path_buf(), faltan })
}

/// Las publicaciones guardadas con firma buena, de la más nueva a la más vieja.
pub fn guardadas(datos: &Path, llaves: &Llaves) -> Vec<Guardada> {
    let mut v: Vec<Guardada> = std::fs::read_dir(carpeta(datos))
        .map(|it| it.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).filter_map(|e| leer_guardada(&e.path(), llaves)).collect())
        .unwrap_or_default();
    v.sort_by_key(|g| std::cmp::Reverse(g.manifiesto.version_leida()));
    v
}

/// La que se sirve a los equipos: la más nueva que está completa.
pub fn vigente(datos: &Path, llaves: &Llaves) -> Option<Guardada> {
    guardadas(datos, llaves).into_iter().find(Guardada::completa)
}

fn crear_privada(dir: &Path) -> Result<(), String> {
    // Nunca a través de un enlace (como `instalador_agente::poner`).
    for d in [dir.parent().and_then(Path::parent), dir.parent(), Some(dir)].into_iter().flatten() {
        if std::fs::symlink_metadata(d).is_ok_and(|m| !m.is_dir()) {
            return Err(format!("{} no es una carpeta (¿un enlace?).", d.display()));
        }
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("No se pudo crear {}: {e}", dir.display()))?;
    #[cfg(unix)]
    for d in [dir.parent().and_then(Path::parent), dir.parent(), Some(dir)].into_iter().flatten() {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

fn escribir_nuevo(ruta: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    let mut f = o.open(ruta).map_err(|e| e.to_string())?;
    f.write_all(bytes).and_then(|()| f.sync_all()).map_err(|e| e.to_string())
}

/// Escribe de una vez (archivo nuevo y renombrar).
fn escribir(ruta: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = ruta.with_file_name(format!(".{}.{}", ruta.file_name().and_then(|n| n.to_str()).unwrap_or("x"), uuid::Uuid::new_v4().simple()));
    let r = escribir_nuevo(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, ruta).map_err(|e| e.to_string()));
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    r
}

/// Acepta un manifiesto firmado: lo comprueba y lo guarda (sin archivos
/// todavía). Si ya había uno de esa versión distinto, lo sustituye y quita los
/// archivos que ya no coinciden. Deja solo las [`MAX_GUARDADAS`] más nuevas.
pub fn poner_manifiesto(datos: &Path, llaves: &Llaves, texto: &str, firma: &str) -> Result<Guardada, String> {
    let v = p::verificar(texto.as_bytes(), firma, llaves, &[], p::PRODUCTO_AGENTE)?;
    let m = &v.manifiesto;
    // La versión ya está comprobada (solo cifras, puntos, letras y guiones): vale como carpeta.
    let dir = carpeta(datos).join(&m.version);
    crear_privada(&dir)?;
    escribir(&dir.join(p::NOMBRE_MANIFIESTO), texto.as_bytes())?;
    escribir(&dir.join(p::NOMBRE_FIRMA), firma.as_bytes())?;
    // Lo que sobra o ya no coincide con este manifiesto, fuera.
    if let Ok(it) = std::fs::read_dir(&dir) {
        for e in it.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n == p::NOMBRE_MANIFIESTO || n == p::NOMBRE_FIRMA {
                continue;
            }
            let vale = m.archivos.iter().find(|a| a.nombre == n).is_some_and(|a| archivo_bien(&e.path(), a));
            if !vale {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    limpiar(datos, llaves);
    leer_guardada(&dir, llaves).ok_or_else(|| "No se pudo releer la publicación guardada.".into())
}

/// Deja solo las [`MAX_GUARDADAS`] más nuevas (y quita lo que no tenga firma buena).
pub fn limpiar(datos: &Path, llaves: &Llaves) {
    let buenas = guardadas(datos, llaves);
    let quedan: Vec<&Path> = buenas.iter().take(MAX_GUARDADAS).map(|g| g.dir.as_path()).collect();
    if let Ok(it) = std::fs::read_dir(carpeta(datos)) {
        for e in it.flatten() {
            let ruta = e.path();
            if e.file_type().is_ok_and(|t| t.is_dir()) && !quedan.contains(&ruta.as_path()) {
                let _ = std::fs::remove_dir_all(&ruta);
            }
        }
    }
}

/// La publicación guardada de esa versión y el archivo del manifiesto con ese nombre.
pub fn buscar(datos: &Path, llaves: &Llaves, version: &str, nombre: &str) -> Option<(Guardada, p::Archivo)> {
    if p::Version::leer(version).is_none() || !p::nombre_valido(nombre) {
        return None;
    }
    let g = leer_guardada(&carpeta(datos).join(version), llaves)?;
    let a = g.manifiesto.archivo_por_nombre(nombre)?.clone();
    Some((g, a))
}

/// Pone un archivo de la carpeta `origen` (la CLI): lo copia comprobando tamaño y SHA-256.
fn poner_archivo_de(g: &Guardada, a: &p::Archivo, origen: &Path) -> Result<(), String> {
    let meta = std::fs::metadata(origen).map_err(|e| format!("No se pudo abrir {}: {e}", origen.display()))?;
    if !meta.is_file() || meta.len() != a.tamano {
        return Err(format!("{}: el tamaño no coincide con el del manifiesto ({} bytes).", a.nombre, a.tamano));
    }
    let tmp = g.dir.join(format!(".{}.{}", a.nombre, uuid::Uuid::new_v4().simple()));
    let r = (|| {
        let mut o = std::fs::OpenOptions::new();
        o.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
        let mut salida = o.open(&tmp).map_err(|e| e.to_string())?;
        let mut entrada = std::fs::File::open(origen).map_err(|e| e.to_string())?;
        std::io::copy(&mut entrada, &mut salida).map_err(|e| e.to_string())?;
        salida.sync_all().map_err(|e| e.to_string())?;
        drop(salida);
        // El SHA-256 de lo que quedó escrito (no del origen, que podría cambiar mientras).
        if !archivo_bien(&tmp, a) {
            return Err(format!("{}: el SHA-256 no coincide con el del manifiesto. No se pone.", a.nombre));
        }
        std::fs::rename(&tmp, g.dir.join(&a.nombre)).map_err(|e| e.to_string())
    })();
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    r
}

/// `resguardo-server poner-publicacion CARPETA`: el manifiesto, su firma y los
/// archivos que estén en la carpeta (los que falten se pueden subir después).
pub fn poner_carpeta(datos: &Path, llaves: &Llaves, origen: &Path) -> Result<Guardada, String> {
    if !datos.is_dir() {
        return Err(format!("No existe la carpeta de datos {}: ¿está instalado el servidor? (o usa --datos)", datos.display()));
    }
    let leer = |n: &str| std::fs::read_to_string(origen.join(n)).map_err(|e| format!("No se pudo leer {}: {e}", origen.join(n).display()));
    let g = poner_manifiesto(datos, llaves, &leer(p::NOMBRE_MANIFIESTO)?, &leer(p::NOMBRE_FIRMA)?)?;
    for a in &g.manifiesto.archivos {
        if !g.faltan.contains(&a.nombre) {
            continue;
        }
        let ruta = origen.join(&a.nombre);
        if ruta.is_file() {
            poner_archivo_de(&g, a, &ruta)?;
        }
    }
    #[cfg(unix)]
    dar_al_dueno(datos);
    leer_guardada(&g.dir, llaves).ok_or_else(|| "No se pudo releer la publicación guardada.".into())
}

/// En Linux, como root (sudo): las carpetas y archivos de las publicaciones,
/// del dueño de la carpeta de datos (el usuario del servicio).
#[cfg(unix)]
fn dar_al_dueno(datos: &Path) {
    use std::os::unix::fs::MetadataExt;
    let Ok(m) = std::fs::metadata(datos) else { return };
    fn recorrer(d: &Path, uid: u32, gid: u32) {
        let _ = std::os::unix::fs::lchown(d, Some(uid), Some(gid));
        if let Ok(it) = std::fs::read_dir(d) {
            for e in it.flatten() {
                match e.file_type() {
                    Ok(t) if t.is_dir() => recorrer(&e.path(), uid, gid),
                    Ok(t) if t.is_file() => {
                        let _ = std::os::unix::fs::lchown(e.path(), Some(uid), Some(gid));
                    }
                    _ => {}
                }
            }
        }
    }
    recorrer(&datos.join("publicaciones"), m.uid(), m.gid());
}

// ---------------------------------------------------------------------------
// Política (tabla de valores del servidor)
// ---------------------------------------------------------------------------

/// La política de un cliente.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoliticaCliente {
    #[serde(default = "modo_auto")]
    pub modo: String,
    #[serde(default = "dias")]
    pub dias_general: u32,
    #[serde(default)]
    pub ventana: Option<Ventana>,
    #[serde(default)]
    pub aprobada: Option<String>,
    #[serde(default)]
    pub retenidas: Vec<String>,
}

fn modo_auto() -> String {
    "auto".into()
}

fn dias() -> u32 {
    p::DIAS_GENERAL
}

/// Lo de cada equipo.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoliticaEquipo {
    #[serde(default)]
    pub anillo: Option<String>,
    #[serde(default)]
    pub aprobada: Option<String>,
}

fn clave_cliente(c: &str) -> String {
    format!("act:politica:{c}")
}

fn clave_equipo(c: &str, e: &str) -> String {
    format!("act:equipo:{c}:{e}")
}

pub fn politica_cliente(db: &dyn Almacen, c: &str) -> R<PoliticaCliente> {
    let mut pc: PoliticaCliente = db.valor(&clave_cliente(c))?.and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| PoliticaCliente {
        modo: modo_auto(),
        dias_general: p::DIAS_GENERAL,
        ..Default::default()
    });
    if !p::MODOS.contains(&pc.modo.as_str()) {
        pc.modo = "manual".into();
    }
    Ok(pc)
}

fn guardar_cliente(db: &dyn Almacen, c: &str, pc: &PoliticaCliente) -> R<()> {
    db.poner_valor(&clave_cliente(c), &serde_json::to_string(pc).map_err(|e| e.to_string())?)
}

pub fn politica_equipo(db: &dyn Almacen, c: &str, e: &str) -> R<PoliticaEquipo> {
    Ok(db.valor(&clave_equipo(c, e))?.and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default())
}

fn guardar_equipo(db: &dyn Almacen, c: &str, e: &str, pe: &PoliticaEquipo) -> R<()> {
    db.poner_valor(&clave_equipo(c, e), &serde_json::to_string(pe).map_err(|e| e.to_string())?)
}

/// La mayor de dos versiones (o la que haya).
fn mayor(a: Option<String>, b: Option<String>) -> Option<String> {
    match (a, b) {
        (Some(x), Some(y)) => Some(if p::comparar(&x, &y).is_some_and(|o| o.is_lt()) { y } else { x }),
        (x, y) => x.or(y),
    }
}

/// Lo que esta consola dice a un equipo (`GET /api/agente/actualizacion`).
pub fn politica_para(db: &dyn Almacen, c: &str, e: &str) -> R<Politica> {
    let pc = politica_cliente(db, c)?;
    let pe = politica_equipo(db, c, e)?;
    let aprobada = mayor(pc.aprobada.clone(), pe.aprobada.clone());
    Ok(Politica {
        modo: pc.modo,
        anillo: pe.anillo.filter(|a| p::ANILLOS.contains(&a.as_str())).unwrap_or_else(|| "general".into()),
        dias_general: pc.dias_general.min(p::MAX_DIAS_GENERAL),
        ventana: pc.ventana,
        retenidas: pc.retenidas.into_iter().filter(|r| Some(r) != aprobada.as_ref()).collect(),
        aprobada,
    }
    .normalizada())
}

/// Un equipo dice que una versión falló y volvió a la anterior: se retiene para
/// el resto del cliente (salvo que ya esté aprobada a mano).
pub fn retener(db: &dyn Almacen, c: &str, version: &str) -> R<bool> {
    if p::Version::leer(version).is_none() {
        return Ok(false);
    }
    let mut pc = politica_cliente(db, c)?;
    if pc.retenidas.iter().any(|r| r == version) || pc.aprobada.as_deref() == Some(version) {
        return Ok(false);
    }
    pc.retenidas.push(version.to_string());
    if pc.retenidas.len() > 32 {
        pc.retenidas.remove(0);
    }
    guardar_cliente(db, c, &pc)?;
    Ok(true)
}

/// La versión que falló, del informe de un equipo (`actualizacion`), si dice que volvió atrás.
pub fn fallida_del_informe(datos: &Value) -> Option<String> {
    let a = datos.get("actualizacion")?;
    let estado = a.get("estado")?.as_str()?;
    if !matches!(estado, "vuelta_atras" | "fallida") {
        return None;
    }
    let v = a.get("version_fallida")?.as_str()?;
    p::Version::leer(v).map(|_| v.to_string())
}

// ---------------------------------------------------------------------------
// Rutas: el propietario del servidor
// ---------------------------------------------------------------------------

fn solo_propietario(u: &Usuario) -> Res<()> {
    if u.0.cuenta.superusuario {
        Ok(())
    } else {
        Err(ErrorApi::prohibido())
    }
}

fn estado_servidor(st: &St) -> Value {
    let llaves = llaves(&st.opciones);
    let todas = guardadas(&st.datos, &llaves);
    json!({
        "sin_llave": llaves.vacia(),
        "llaves": llaves.ids(),
        "vigente": todas.iter().find(|g| g.completa()).map(Guardada::vista),
        "guardadas": todas.iter().map(Guardada::vista).collect::<Vec<_>>(),
        "version_servidor": env!("CARGO_PKG_VERSION"),
    })
}

/// `GET /api/servidor/publicacion` (propietario del servidor).
pub async fn ver(State(st): State<St>, u: Usuario) -> Res<Json<Value>> {
    solo_propietario(&u)?;
    let st2 = st.clone();
    Ok(Json(tokio::task::spawn_blocking(move || estado_servidor(&st2)).await.map_err(ErrorApi::interno)?))
}

#[derive(Deserialize)]
pub struct NuevoManifiesto {
    manifiesto: String,
    firma: String,
}

/// `PUT /api/servidor/publicacion` (propietario del servidor): `{ manifiesto, firma }`,
/// el texto exacto de los dos archivos. Solo con firma buena.
pub async fn poner(State(st): State<St>, u: Usuario, Json(n): Json<NuevoManifiesto>) -> Res<Json<Value>> {
    solo_propietario(&u)?;
    if n.manifiesto.len() > p::MAX_MANIFIESTO || n.firma.len() > p::MAX_FIRMA {
        return Err(ErrorApi::datos("El manifiesto o la firma son demasiado grandes."));
    }
    let st2 = st.clone();
    let r = tokio::task::spawn_blocking(move || poner_manifiesto(&st2.datos, &llaves(&st2.opciones), &n.manifiesto, &n.firma))
        .await
        .map_err(ErrorApi::interno)?
        .map_err(ErrorApi::datos)?;
    let (correo, version) = (u.0.cuenta.correo.clone(), r.manifiesto.version.clone());
    st.db(move |db| db.auditar_servidor(&format!("cuenta:{correo}"), "poner_publicacion", &version, "{}")).await?;
    let st2 = st.clone();
    Ok(Json(tokio::task::spawn_blocking(move || estado_servidor(&st2)).await.map_err(ErrorApi::interno)?))
}

/// `PUT /api/servidor/publicacion/{version}/{nombre}` (propietario del servidor): el
/// archivo, tal cual, en el cuerpo. Se escribe al lado mientras se calcula su SHA-256 y
/// solo se queda si coincide con el del manifiesto (y no pasa de su tamaño).
pub async fn subir_archivo(State(st): State<St>, u: Usuario, Ruta((version, nombre)): Ruta<(String, String)>, req: Request) -> Res<Json<Value>> {
    solo_propietario(&u)?;
    let (st2, v2, n2) = (st.clone(), version.clone(), nombre.clone());
    let (g, a) = tokio::task::spawn_blocking(move || buscar(&st2.datos, &llaves(&st2.opciones), &v2, &n2))
        .await
        .map_err(ErrorApi::interno)?
        .ok_or_else(|| ErrorApi::datos("Ese archivo no está en el manifiesto de esa versión (pon antes el manifiesto firmado)."))?;
    let tmp = g.dir.join(format!(".{}.{}", a.nombre, uuid::Uuid::new_v4().simple()));
    let r = recibir(req.into_body(), &tmp, &a).await;
    if let Err(e) = r {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e);
    }
    tokio::fs::rename(&tmp, g.dir.join(&a.nombre)).await.map_err(ErrorApi::interno)?;
    let correo = u.0.cuenta.correo.clone();
    st.db(move |db| db.auditar_servidor(&format!("cuenta:{correo}"), "subir_publicacion", &format!("{version}/{nombre}"), "{}")).await?;
    let st2 = st.clone();
    Ok(Json(tokio::task::spawn_blocking(move || estado_servidor(&st2)).await.map_err(ErrorApi::interno)?))
}

async fn recibir(cuerpo: Body, tmp: &Path, a: &p::Archivo) -> Res<()> {
    use tokio::io::AsyncWriteExt;
    let mut o = tokio::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    o.mode(0o600);
    let mut f = o.open(tmp).await.map_err(ErrorApi::interno)?;
    let mut h = Sha256::new();
    let mut n: u64 = 0;
    let mut trozos = cuerpo.into_data_stream();
    while let Some(t) = trozos.next().await {
        let t = t.map_err(|_| ErrorApi::datos("La subida se cortó."))?;
        n += t.len() as u64;
        if n > a.tamano {
            return Err(ErrorApi::datos(format!("{} es más grande que en el manifiesto ({} bytes).", a.nombre, a.tamano)));
        }
        h.update(&t);
        f.write_all(&t).await.map_err(ErrorApi::interno)?;
    }
    f.sync_all().await.map_err(ErrorApi::interno)?;
    let suma: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if n != a.tamano || !suma.eq_ignore_ascii_case(&a.sha256) {
        return Err(ErrorApi::datos(format!("{} no coincide con el manifiesto firmado (tamaño o SHA-256): no se guarda.", a.nombre)));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Rutas: la consola de un cliente
// ---------------------------------------------------------------------------

/// `GET /api/clientes/{c}/actualizaciones` (cualquier persona del cliente).
pub async fn ver_cliente(State(st): State<St>, u: Usuario, Ruta(c): Ruta<String>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Lectura).await?;
    let st2 = st.clone();
    let disponible = tokio::task::spawn_blocking(move || {
        let l = llaves(&st2.opciones);
        (l.vacia(), vigente(&st2.datos, &l).map(|g| g.vista()))
    })
    .await
    .map_err(ErrorApi::interno)?;
    let (c2, ctx2) = (c.clone(), ctx.clone());
    let (pc, equipos) = st
        .db(move |db| {
            let pc = politica_cliente(db, &c2)?;
            let mut equipos = serde_json::Map::new();
            for e in db.equipos(&ctx2)? {
                let pe = politica_equipo(db, &c2, &e.id)?;
                equipos.insert(e.id.clone(), json!({ "anillo": pe.anillo.unwrap_or_else(|| "general".into()), "aprobada": pe.aprobada }));
            }
            Ok((pc, equipos))
        })
        .await?;
    Ok(Json(json!({
        "sin_llave": disponible.0,
        "disponible": disponible.1,
        "politica": pc,
        "equipos": equipos,
    })))
}

#[derive(Deserialize)]
pub struct CambioPolitica {
    modo: String,
    #[serde(default = "dias")]
    dias_general: u32,
    #[serde(default)]
    ventana: Option<Ventana>,
}

/// Los equipos conectados del cliente (o uno) buscan ya la actualización.
fn tocar(st: &St, equipos: &[String]) -> usize {
    equipos.iter().filter(|e| st.al_agente(e, &json!({ "t": "actualizacion" }))).count()
}

async fn ids_equipos(st: &St, ctx: &ClienteCtx) -> Res<Vec<String>> {
    let ctx = ctx.clone();
    st.db(move |db| Ok(db.equipos(&ctx)?.into_iter().map(|e| e.id).collect())).await
}

/// `PUT /api/clientes/{c}/actualizaciones` (administrador): `{ modo, dias_general, ventana }`.
pub async fn cambiar_cliente(State(st): State<St>, u: Usuario, Ruta(c): Ruta<String>, Json(n): Json<CambioPolitica>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    if !p::MODOS.contains(&n.modo.as_str()) {
        return Err(ErrorApi::datos("Modo no válido (auto, manual o pausada)."));
    }
    if n.dias_general > p::MAX_DIAS_GENERAL {
        return Err(ErrorApi::datos(format!("Como mucho {} días.", p::MAX_DIAS_GENERAL)));
    }
    if n.ventana.as_ref().is_some_and(|v| !v.valida()) {
        return Err(ErrorApi::datos("La ventana va de HH:MM a HH:MM."));
    }
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let (c2, ctx2) = (c.clone(), ctx.clone());
    let pc = st
        .db(move |db| {
            let mut pc = politica_cliente(db, &c2)?;
            pc.modo = n.modo;
            pc.dias_general = n.dias_general;
            pc.ventana = n.ventana;
            guardar_cliente(db, &c2, &pc)?;
            let datos = json!({ "modo": pc.modo, "dias_general": pc.dias_general, "ventana": pc.ventana }).to_string();
            db.auditar(&ctx2, &actor, "politica_actualizaciones", &c2, &datos)?;
            Ok(pc)
        })
        .await?;
    let equipos = ids_equipos(&st, &ctx).await?;
    tocar(&st, &equipos);
    Ok(Json(json!({ "politica": pc })))
}

#[derive(Deserialize, Default)]
pub struct Ahora {
    #[serde(default)]
    equipo: Option<String>,
}

/// `POST /api/clientes/{c}/actualizaciones/ahora` (administrador): `{ equipo? }`. Aprueba la
/// versión que sirve este servidor (para el cliente entero o un equipo) y da un toque a
/// los equipos conectados. Inofensiva: solo lleva a una versión firmada y más nueva.
pub async fn actualizar_ahora(State(st): State<St>, u: Usuario, Ruta(c): Ruta<String>, Json(n): Json<Ahora>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    let st2 = st.clone();
    let version = tokio::task::spawn_blocking(move || vigente(&st2.datos, &llaves(&st2.opciones)).map(|g| g.manifiesto.version))
        .await
        .map_err(ErrorApi::interno)?
        .ok_or_else(|| ErrorApi::conflicto("Este servidor aún no tiene ninguna versión del agente para dar."))?;
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let (c2, ctx2, v2, equipo) = (c.clone(), ctx.clone(), version.clone(), n.equipo.clone());
    let existe = st
        .db(move |db| {
            match &equipo {
                Some(e) => {
                    if db.equipo(&ctx2, e)?.is_none() {
                        return Ok(false);
                    }
                    let mut pe = politica_equipo(db, &c2, e)?;
                    pe.aprobada = Some(v2.clone());
                    guardar_equipo(db, &c2, e, &pe)?;
                }
                None => {
                    let mut pc = politica_cliente(db, &c2)?;
                    pc.aprobada = Some(v2.clone());
                    pc.retenidas.retain(|r| r != &v2);
                    guardar_cliente(db, &c2, &pc)?;
                }
            }
            db.auditar(&ctx2, &actor, "actualizar_ahora", equipo.as_deref().unwrap_or(&c2), &json!({ "version": v2 }).to_string())?;
            Ok(true)
        })
        .await?;
    if !existe {
        return Err(ErrorApi::no_existe());
    }
    let equipos = match n.equipo {
        Some(e) => vec![e],
        None => ids_equipos(&st, &ctx).await?,
    };
    let tocados = tocar(&st, &equipos);
    Ok(Json(json!({ "version": version, "avisados": tocados })))
}

#[derive(Deserialize)]
pub struct CambioAnillo {
    anillo: String,
}

/// `PUT /api/clientes/{c}/equipos/{e}/anillo` (administrador): `{ anillo: "prueba" | "general" }`.
pub async fn poner_anillo(State(st): State<St>, u: Usuario, Ruta((c, e)): Ruta<(String, String)>, Json(n): Json<CambioAnillo>) -> Res<Json<Value>> {
    let (ctx, _) = u.miembro(&st, &c, Rol::Administrador).await?;
    if !p::ANILLOS.contains(&n.anillo.as_str()) {
        return Err(ErrorApi::datos("Anillo no válido (prueba o general)."));
    }
    let actor = format!("cuenta:{}", u.0.cuenta.correo);
    let (c2, e2, anillo) = (c.clone(), e.clone(), n.anillo.clone());
    let existe = st
        .db(move |db| {
            if db.equipo(&ctx, &e2)?.is_none() {
                return Ok(false);
            }
            let mut pe = politica_equipo(db, &c2, &e2)?;
            pe.anillo = Some(anillo.clone());
            guardar_equipo(db, &c2, &e2, &pe)?;
            db.auditar(&ctx, &actor, "anillo_equipo", &e2, &json!({ "anillo": anillo }).to_string())?;
            Ok(true)
        })
        .await?;
    if !existe {
        return Err(ErrorApi::no_existe());
    }
    tocar(&st, std::slice::from_ref(&e));
    Ok(Json(json!({ "anillo": n.anillo })))
}

// ---------------------------------------------------------------------------
// Rutas: los agentes
// ---------------------------------------------------------------------------

/// `GET /api/agente/actualizacion` (equipo): la política de esta consola para el
/// equipo y, si la hay, la publicación que sirve (manifiesto y firma tal cual).
pub async fn para_agente(State(st): State<St>, a: crate::agentes::Agente) -> Res<Json<Value>> {
    if !st.limites.intento(&format!("actualizacion:{}", a.equipo), 60, std::time::Duration::from_secs(3600)) {
        return Err(ErrorApi::demasiados());
    }
    let (c, e) = (a.ctx.id().to_string(), a.equipo.clone());
    let politica = st.db(move |db| politica_para(db, &c, &e)).await?;
    let st2 = st.clone();
    let g = tokio::task::spawn_blocking(move || vigente(&st2.datos, &llaves(&st2.opciones))).await.map_err(ErrorApi::interno)?;
    let publicacion = g.map(|g| {
        json!({
            "manifiesto": g.texto,
            "firma": g.firma,
            "archivos": format!("/api/agente/actualizacion/archivos/{}/", g.manifiesto.version),
        })
    });
    Ok(Json(json!({ "politica": politica, "publicacion": publicacion })))
}

/// `GET /api/agente/actualizacion/archivos/{version}/{nombre}` (equipo): el archivo tal cual.
pub async fn archivo_para_agente(
    State(st): State<St>,
    a: crate::agentes::Agente,
    Ruta((version, nombre)): Ruta<(String, String)>,
    req: Request,
) -> Res<Response> {
    if !st.limites.intento(&format!("actualizacion-archivo:{}", a.equipo), 12, std::time::Duration::from_secs(3600)) {
        return Err(ErrorApi::demasiados());
    }
    let st2 = st.clone();
    let encontrado = tokio::task::spawn_blocking(move || buscar(&st2.datos, &llaves(&st2.opciones), &version, &nombre)).await.map_err(ErrorApi::interno)?;
    let Some((g, a)) = encontrado.filter(|(g, _)| g.completa()) else {
        return Err(ErrorApi::no_existe());
    };
    let servir = tower_http::services::ServeFile::new(g.dir.join(&a.nombre));
    let res = tower::ServiceExt::oneshot(servir, req).await.map_err(ErrorApi::interno)?;
    if res.status() != StatusCode::OK && res.status() != StatusCode::PARTIAL_CONTENT {
        return Err(ErrorApi::no_existe());
    }
    Ok(res.into_response())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use resguardo_protocolo::publicacion::pruebas as pp;

    fn llaves_prueba() -> Llaves {
        pp::llaves_a()
    }

    fn publicacion(dir: &Path, version: &str) -> (String, String, Vec<(String, Vec<u8>)>) {
        let exe = format!("instalador {version}").repeat(100).into_bytes();
        let tgz = format!("paquete {version}").repeat(50).into_bytes();
        let m = pp::manifiesto(version, &[("windows-x86_64", "setup.exe", &exe), ("linux-x86_64", "agente.tar.gz", &tgz)]);
        let texto = serde_json::to_string_pretty(&m).unwrap();
        let firma = pp::firmar_a(texto.as_bytes());
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(p::NOMBRE_MANIFIESTO), &texto).unwrap();
        std::fs::write(dir.join(p::NOMBRE_FIRMA), &firma).unwrap();
        std::fs::write(dir.join("setup.exe"), &exe).unwrap();
        std::fs::write(dir.join("agente.tar.gz"), &tgz).unwrap();
        (texto, firma, vec![("setup.exe".into(), exe), ("agente.tar.gz".into(), tgz)])
    }

    #[test]
    fn solo_acepta_lo_firmado() {
        let t = tempfile::tempdir().unwrap();
        let datos = t.path().join("datos");
        std::fs::create_dir(&datos).unwrap();
        let (texto, firma, _) = publicacion(&t.path().join("origen"), "0.7.25");
        let l = llaves_prueba();
        // Sin llave (el marcador de posición): nada.
        assert!(poner_manifiesto(&datos, &Llaves::default(), &texto, &firma).unwrap_err().contains("no tiene llave"));
        // Firma de otra llave, firma rota, manifiesto cambiado: nada.
        assert!(poner_manifiesto(&datos, &l, &texto, &pp::firmar_b(texto.as_bytes())).is_err());
        assert!(poner_manifiesto(&datos, &l, &texto.replace("0.7.25", "0.7.26"), &firma).is_err());
        assert!(poner_manifiesto(&datos, &l, &texto, "basura").is_err());
        assert!(guardadas(&datos, &l).is_empty());
        // Bien firmado: se guarda, pero sin archivos no se sirve.
        let g = poner_manifiesto(&datos, &l, &texto, &firma).unwrap();
        assert_eq!(g.faltan.len(), 2);
        assert!(vigente(&datos, &l).is_none());
        // Un manifiesto guardado que alguien cambia en disco deja de valer.
        std::fs::write(g.dir.join(p::NOMBRE_MANIFIESTO), texto.replace("estable", "estable ")).unwrap();
        assert!(guardadas(&datos, &l).is_empty());
    }

    #[test]
    fn carpeta_completa_y_archivos_cambiados() {
        let t = tempfile::tempdir().unwrap();
        let datos = t.path().join("datos");
        std::fs::create_dir(&datos).unwrap();
        let origen = t.path().join("origen");
        let (texto, firma, archivos) = publicacion(&origen, "0.7.25");
        let l = llaves_prueba();
        // Un archivo cambiado en la carpeta: no se pone.
        std::fs::write(origen.join("setup.exe"), b"otro contenido del mismo tamano?").unwrap();
        assert!(poner_carpeta(&datos, &l, &origen).is_err());
        std::fs::write(origen.join("setup.exe"), &archivos[0].1).unwrap();
        let g = poner_carpeta(&datos, &l, &origen).unwrap();
        assert!(g.completa(), "{:?}", g.faltan);
        let v = vigente(&datos, &l).unwrap();
        assert_eq!((v.texto.as_str(), v.firma.as_str()), (texto.as_str(), firma.as_str()), "se sirve tal cual");
        assert_eq!(std::fs::read(v.dir.join("agente.tar.gz")).unwrap(), archivos[1].1);
        // Nombres y versiones raros no llegan al disco.
        assert!(buscar(&datos, &l, "../0.7.25", "setup.exe").is_none());
        assert!(buscar(&datos, &l, "0.7.25", "../datos.db").is_none());
        assert!(buscar(&datos, &l, "0.7.25", "otro.exe").is_none());
        assert!(buscar(&datos, &l, "0.7.25", "setup.exe").is_some());
    }

    #[test]
    fn guarda_las_tres_ultimas_y_sirve_la_mas_nueva_completa() {
        let t = tempfile::tempdir().unwrap();
        let datos = t.path().join("datos");
        std::fs::create_dir(&datos).unwrap();
        let l = llaves_prueba();
        for v in ["0.7.22", "0.7.23", "0.7.24", "0.7.25"] {
            let o = t.path().join(v);
            publicacion(&o, v);
            poner_carpeta(&datos, &l, &o).unwrap();
        }
        let vs: Vec<String> = guardadas(&datos, &l).into_iter().map(|g| g.manifiesto.version).collect();
        assert_eq!(vs, ["0.7.25", "0.7.24", "0.7.23"]);
        // Una más nueva sin archivos: se sigue sirviendo la anterior completa.
        let o = t.path().join("0.7.26");
        let (texto, firma, _) = publicacion(&o, "0.7.26");
        poner_manifiesto(&datos, &l, &texto, &firma).unwrap();
        assert_eq!(vigente(&datos, &l).unwrap().manifiesto.version, "0.7.25");
    }

    #[test]
    fn politica_del_equipo_y_retenidas() {
        let t = tempfile::tempdir().unwrap();
        let db = crate::almacen::sqlite::Sqlite::abrir(t.path()).unwrap();
        // Sin nada: automática, general, 2 días.
        assert_eq!(politica_para(&db, "c1", "e1").unwrap(), Politica::default());
        guardar_cliente(&db, "c1", &PoliticaCliente { modo: "auto".into(), dias_general: 3, aprobada: Some("0.7.25".into()), ..Default::default() }).unwrap();
        guardar_equipo(&db, "c1", "e1", &PoliticaEquipo { anillo: Some("prueba".into()), aprobada: Some("0.7.26".into()) }).unwrap();
        let pe = politica_para(&db, "c1", "e1").unwrap();
        assert_eq!((pe.anillo.as_str(), pe.dias_general, pe.aprobada.as_deref()), ("prueba", 3, Some("0.7.26")));
        // Una vuelta atrás retiene la versión para los demás…
        assert!(retener(&db, "c1", "0.7.27").unwrap());
        assert!(!retener(&db, "c1", "0.7.27").unwrap());
        assert!(!retener(&db, "c1", "no").unwrap(), "versión no válida");
        assert_eq!(politica_para(&db, "c1", "e2").unwrap().retenidas, ["0.7.27"]);
        // …salvo la aprobada.
        assert!(!retener(&db, "c1", "0.7.25").unwrap());
        assert_eq!(fallida_del_informe(&json!({ "actualizacion": { "estado": "vuelta_atras", "version_fallida": "0.7.27" } })).as_deref(), Some("0.7.27"));
        assert_eq!(fallida_del_informe(&json!({ "actualizacion": { "estado": "al_dia", "version_fallida": "0.7.27" } })), None);
        assert_eq!(fallida_del_informe(&json!({})), None);
    }
}
