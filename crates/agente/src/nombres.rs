//! Nombres legibles en el almacén (plan 0.8.0, B1; docs/nombres-legibles.md).
//!
//! - Un equipo nuevo en el almacén tiene como usuario (y carpeta) su nombre
//!   normalizado con un sufijo corto al azar (`pc-01-3f2a`), no su id.
//! - «Ordenar nombres de carpeta» (`guarda_copias { mover }`): el almacén pasa
//!   la carpeta de un equipo (su usuario) y las de sus repositorios a nombres
//!   legibles, en el mismo disco y de una vez, y pone al día lo suyo que las
//!   nombra (`.htpasswd`, la retención del almacén y la selección de los
//!   espejos). Si algo falla, lo deshace. Después la consola manda al equipo
//!   dueño `cambiar_destino { donde, usuario, carpetas }`.
//! - En el destino de cada espejo no se mueve nada: la carpeta nueva se copia
//!   entera y la vieja se queda para la retención del espejo. Para que el freno
//!   no salte por eso, el almacén apunta cada cambio en `renombres`.

use crate::server::{Files, ServerConfig};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// `resumen.admite`: entiende `guarda_copias { anadir, nombre }`, `guarda_copias { mover }`,
/// `cambiar_destino { carpetas }` y `nombre_repositorio`.
pub const ADMITE: &str = "nombres_legibles";
/// Largo máximo del nombre del equipo en su carpeta (más «-» y 4 cifras: 40).
pub const MAX_EQUIPO: usize = 35;
/// Largo máximo de la carpeta de un repositorio.
pub const MAX_REPO: usize = 40;
/// Largo máximo de una carpeta del almacén que se acepta (un UUID son 36).
pub const MAX_CARPETA: usize = 64;
/// Cambios que se recuerdan (para el freno del espejo y para la consola).
pub const MAX_RENOMBRES: usize = 200;
/// Repositorios que se mueven en una orden, como mucho.
pub const MAX_REPOS: usize = 100;
/// Un archivo en `locks/` más reciente que esto: hay algo en marcha en el repositorio.
const BLOQUEO_RECIENTE: Duration = Duration::from_secs(15 * 60);

/// La letra sin su marca (tilde, diéresis…), para las letras latinas de siempre.
fn sin_marca(c: char) -> Option<char> {
    Some(match c {
        'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' => 'a',
        'é' | 'è' | 'ë' | 'ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' | 'õ' => 'o',
        'ú' | 'ù' | 'ü' | 'û' => 'u',
        'ñ' => 'n',
        'ç' => 'c',
        'ý' | 'ÿ' => 'y',
        c if c.is_ascii_lowercase() || c.is_ascii_digit() => c,
        _ => return None,
    })
}

/// El nombre de carpeta de un texto cualquiera: minúsculas, sin tildes, lo que no
/// sea letra o cifra pasa a guion (sin repetir ni en los extremos), hasta `max`.
/// Vacío: `defecto`. Igual que `normalizar` de la consola (vectores compartidos).
pub fn normalizar(raw: &str, max: usize, defecto: &str) -> String {
    let mut s = String::new();
    for c in raw.to_lowercase().chars() {
        match sin_marca(c) {
            Some(x) => s.push(x),
            None if !s.ends_with('-') => s.push('-'),
            None => {}
        }
    }
    let s: String = s.trim_matches('-').chars().take(max).collect();
    let s = s.trim_end_matches('-');
    if s.is_empty() {
        defecto.to_string()
    } else {
        s.to_string()
    }
}

/// ¿Una carpeta del almacén con un nombre aceptable? `a–z`, `0–9` y `-`, sin guion
/// en los extremos, de 1 a 64 (vale también un UUID: los usuarios de antes).
pub fn carpeta_valida(s: &str) -> bool {
    (1..=MAX_CARPETA).contains(&s.len())
        && !s.starts_with('-')
        && !s.ends_with('-')
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Cuatro cifras hexadecimales al azar.
pub fn sufijo() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..4].to_string()
}

/// Un cambio de carpeta en el almacén: `de` y `a` son `<usuario>` o `<usuario>/<repo>`
/// (relativos a la carpeta de su zona).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Renombre {
    pub de: String,
    pub a: String,
    /// RFC 3339.
    pub cuando: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zona: Option<String>,
}

/// La zona de un usuario (`None`: la principal) y su carpeta, si es de este almacén.
fn zona_de(c: &ServerConfig, usuario: &str) -> Option<(Option<String>, PathBuf)> {
    if c.users.iter().any(|u| u.name == usuario) {
        return Some((None, PathBuf::from(&c.path)));
    }
    c.zonas.iter().find(|z| z.users.iter().any(|u| u.name == usuario)).map(|z| (Some(z.id.clone()), PathBuf::from(&z.path)))
}

/// ¿Está ya en uso ese nombre de usuario? Un usuario, una carpeta en el disco de
/// cualquier zona (de un equipo quitado: el nuevo no debe heredarla) o un nombre
/// antiguo de una carpeta movida (en el espejo, la vieja sigue ahí).
pub fn usuario_ocupado(c: &ServerConfig, nombre: &str) -> bool {
    c.usuario_existe(nombre)
        || c.renombres.iter().any(|r| r.de == nombre || r.a == nombre || r.de.starts_with(&format!("{nombre}/")))
        || c.carpetas().iter().filter(|p| !p.is_empty()).any(|p| Path::new(p).join(nombre).exists())
}

/// Un usuario legible y libre para un equipo: `<nombre normalizado>-<sufijo>`.
pub fn usuario_legible(c: &ServerConfig, nombre: &str) -> Result<String, String> {
    let base = normalizar(nombre, MAX_EQUIPO, "equipo");
    for _ in 0..50 {
        let u = format!("{base}-{}", sufijo());
        if !usuario_ocupado(c, &u) {
            return Ok(u);
        }
    }
    Err("No se encontró un nombre libre para la carpeta del equipo.".into())
}

/// El nombre del equipo que pide la consola (`nombre` en `anadir`, `legible` en `mover`): un texto corto sin control.
pub fn texto_legible(v: &Value, campo: &str) -> Result<Option<String>, String> {
    match v.get(campo) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if !s.trim().is_empty() && s.chars().count() <= 80 && !s.chars().any(char::is_control) => Ok(Some(s.trim().to_string())),
        Some(_) => Err(format!("«{campo}» no es un nombre válido (hasta 80 caracteres).")),
    }
}

// ---------- Ordenar nombres de carpeta ----------

/// Lo que pide `guarda_copias { mover }`.
#[derive(Clone, Debug, PartialEq)]
pub struct Pedido {
    /// El usuario del equipo en el almacén, hoy.
    pub usuario: String,
    /// Si viene, su carpeta pasa a `<legible normalizado>-<sufijo>`.
    pub legible: Option<String>,
    /// Carpetas de repositorios (dentro de la del usuario) y el nombre que se quiere.
    pub repos: Vec<(String, String)>,
    /// Deshacer el último cambio de este usuario (volver a los nombres de antes).
    pub deshacer: bool,
}

/// `{ usuario, legible?, repos?: [{ de, nombre }] }` o `{ usuario, deshacer: true }`, comprobado.
pub fn leer_pedido(v: &Value) -> Result<Pedido, String> {
    let usuario = v["usuario"].as_str().unwrap_or("").to_string();
    if !crate::retencion_almacen::segmento_valido(&usuario) {
        return Err("Falta el equipo (su carpeta en el almacén) o no es válido.".into());
    }
    if v["deshacer"] == true {
        return Ok(Pedido { usuario, legible: None, repos: Vec::new(), deshacer: true });
    }
    let legible = texto_legible(v, "legible")?;
    let mut repos = Vec::new();
    if let Some(l) = v.get("repos").filter(|x| !x.is_null()) {
        let l = l.as_array().ok_or("«repos» tiene que ser una lista.")?;
        if l.len() > MAX_REPOS {
            return Err(format!("Como mucho {MAX_REPOS} repositorios de una vez."));
        }
        for r in l {
            let de = r["de"].as_str().unwrap_or("");
            if !crate::retencion_almacen::segmento_valido(de) {
                return Err("Carpeta de repositorio no válida.".into());
            }
            let nombre = texto_legible(r, "nombre")?.ok_or("Falta el nombre nuevo de un repositorio.")?;
            if repos.iter().any(|(d, _): &(String, String)| d == de) {
                return Err(format!("El repositorio «{de}» está dos veces."));
            }
            repos.push((de.to_string(), nombre));
        }
    }
    if legible.is_none() && repos.is_empty() {
        return Err("No hay nada que mover: di el nombre nuevo del equipo o de algún repositorio.".into());
    }
    Ok(Pedido { usuario, legible, repos, deshacer: false })
}

/// Lo último que se movió hacia `usuario` (el usuario y sus repositorios, de una misma
/// vez): `(usuario de antes si cambió, [(carpeta de ahora, carpeta de antes)], cuándo)`.
/// Lo último que se movió hacia un usuario: `(usuario de antes, [(ahora, antes)], cuándo)`.
type Cambio = (Option<String>, Vec<(String, String)>, String);

fn ultimo_cambio(c: &ServerConfig, usuario: &str) -> Option<Cambio> {
    let suyo = |r: &&Renombre| r.a == usuario || r.a.starts_with(&format!("{usuario}/"));
    let cuando = c.renombres.iter().rev().find(suyo)?.cuando.clone();
    let tanda: Vec<&Renombre> = c.renombres.iter().filter(suyo).filter(|r| r.cuando == cuando).collect();
    let antes_u = tanda.iter().find(|r| !r.a.contains('/')).map(|r| r.de.clone());
    let mut repos = Vec::new();
    for r in tanda.iter().filter(|r| r.a.contains('/')) {
        let (Some((_, ahora)), Some((_, antes))) = (r.a.split_once('/'), r.de.split_once('/')) else { continue };
        repos.push((ahora.to_string(), antes.to_string()));
    }
    Some((antes_u, repos, cuando))
}

/// Cómo deshacer lo hecho en el disco (en orden inverso).
#[derive(Clone, Debug, Default)]
pub struct Deshacer {
    renombrados: Vec<(PathBuf, PathBuf)>,
    /// El `.htpasswd` de antes (zona, líneas), si se cambió.
    htpasswd: Option<(Option<String>, Vec<String>)>,
}

impl Deshacer {
    /// Deshace lo hecho. Devuelve lo que no se pudo deshacer (para decirlo).
    pub fn aplicar(&self, f: &Files) -> Vec<String> {
        let mut mal = Vec::new();
        if let Some((zona, lineas)) = &self.htpasswd {
            if let Err(e) = f.write_htpasswd_de(zona.as_deref(), lineas) {
                mal.push(format!("los usuarios del almacén ({e})"));
            }
        }
        for (de, a) in self.renombrados.iter().rev() {
            if let Err(e) = std::fs::rename(a, de) {
                mal.push(format!("«{}» ({e})", nombre_corto(a)));
            }
        }
        mal
    }
}

fn nombre_corto(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Lo que se hizo.
#[derive(Clone, Debug, Default)]
pub struct Movido {
    /// El usuario de antes y el de ahora, si cambió.
    pub usuario: Option<(String, String)>,
    /// Carpetas de repositorios: la de antes y la de ahora (solo el nombre, dentro del usuario).
    pub repos: Vec<(String, String)>,
    pub zona: Option<String>,
    /// El usuario de antes (cambie o no).
    pub antes: String,
    pub deshacer: Deshacer,
}

impl Movido {
    /// El usuario después.
    pub fn usuario_final(&self) -> &str {
        self.usuario.as_ref().map_or(self.antes.as_str(), |(_, a)| a.as_str())
    }

    /// Una ruta del almacén (`<usuario>` o `<usuario>/<repo>`) después del cambio
    /// (`None` si no le toca).
    pub fn ruta_nueva(&self, ruta: &str) -> Option<String> {
        let (u, r) = match ruta.split_once('/') {
            Some((u, r)) => (u, Some(r)),
            None => (ruta, None),
        };
        if u != self.antes {
            return None;
        }
        let u2 = self.usuario_final();
        match r {
            None => self.usuario.is_some().then(|| u2.to_string()),
            Some(r) => {
                let r2 = self.repos.iter().find(|(de, _)| de == r).map_or(r, |(_, a)| a.as_str());
                (u2 != u || r2 != r).then(|| format!("{u2}/{r2}"))
            }
        }
    }

    /// Los cambios para `renombres` (rutas completas).
    pub fn renombres(&self, cuando: &str) -> Vec<Renombre> {
        let mut v: Vec<Renombre> = self
            .repos
            .iter()
            .map(|(de, a)| Renombre {
                de: format!("{}/{de}", self.antes),
                a: format!("{}/{a}", self.usuario_final()),
                cuando: cuando.into(),
                zona: self.zona.clone(),
            })
            .collect();
        if let Some((de, a)) = &self.usuario {
            v.push(Renombre { de: de.clone(), a: a.clone(), cuando: cuando.into(), zona: self.zona.clone() });
        }
        v
    }

    /// Lo que se contesta a la consola (solo nombres de carpeta).
    pub fn detalle(&self) -> Value {
        json!({ "mover": {
            "usuario": self.usuario.as_ref().map(|(de, a)| json!({ "de": de, "a": a })),
            "repos": self.repos.iter().map(|(de, a)| json!({ "de": de, "a": a })).collect::<Vec<_>>(),
            "zona": self.zona,
        }})
    }
}

/// ¿Hay algo en marcha en este repositorio (una copia, una poda, una verificación)? Un
/// archivo en `locks/` de hace menos de 15 min: restic lo renueva cada 5 min mientras trabaja.
fn en_uso(repo: &Path) -> bool {
    let Ok(rd) = std::fs::read_dir(repo.join("locks")) else { return false };
    rd.flatten()
        .any(|e| e.metadata().ok().and_then(|m| m.modified().ok()).is_some_and(|t| SystemTime::now().duration_since(t).map_or(true, |d| d < BLOQUEO_RECIENTE)))
}

/// Los repositorios directos de la carpeta de un usuario (con `config`), sin enlaces.
fn repos_de(dir: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<String> = rd
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()) && !crate::platform::is_reparse_point(&e.path()) && e.path().join("config").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

/// Mueve en el disco y pone al día la configuración `c` (en memoria: la guarda quien
/// llama, y si no puede, deshace con `Movido::deshacer`). La retención del almacén
/// también la pone al día quien llama (`retencion_almacen::renombrar`).
pub fn mover_en(f: &Files, c: &mut ServerConfig, p: &Pedido, ahora: &str) -> Result<Movido, String> {
    let (zona, base) = zona_de(c, &p.usuario).ok_or_else(|| format!("«{}» no es un equipo de este almacén.", p.usuario))?;
    if base.as_os_str().is_empty() || !base.is_dir() {
        return Err("No se encuentra la carpeta del almacén (¿un disco que no está?): no se mueve nada.".into());
    }
    let dir_u = base.join(&p.usuario);
    if crate::platform::is_reparse_point(&dir_u) {
        return Err("La carpeta de ese equipo es un enlace: no se mueve nada a través de enlaces.".into());
    }
    // Deshacer: los nombres de antes, exactos (y se olvida aquel cambio).
    let deshecho = if p.deshacer { Some(ultimo_cambio(c, &p.usuario).ok_or("No hay nada que deshacer en la carpeta de ese equipo.")?) } else { None };
    // Los repositorios: que existan, sin enlaces y con nombres nuevos libres y distintos.
    let existentes: Vec<String> =
        std::fs::read_dir(&dir_u).map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().to_lowercase()).collect()).unwrap_or_default();
    let mut repos: Vec<(String, String)> = Vec::new();
    for (de, a) in deshecho.iter().flat_map(|(_, r, _)| r.iter()) {
        if !carpeta_valida(a) && !crate::retencion_almacen::segmento_valido(a) {
            return Err(format!("No se puede volver a «{a}»."));
        }
        if !dir_u.join(de).join("config").is_file() || existentes.contains(&a.to_lowercase()) {
            return Err(format!("No se puede volver a «{a}»: ya no está «{de}» o «{a}» ya existe."));
        }
        repos.push((de.clone(), a.clone()));
    }
    for (de, nombre) in &p.repos {
        let d = dir_u.join(de);
        if crate::platform::is_reparse_point(&d) {
            return Err(format!("La carpeta de «{de}» es un enlace: no se mueve nada a través de enlaces."));
        }
        if !d.join("config").is_file() {
            return Err(format!("No hay ningún repositorio «{}/{de}» en este almacén.", p.usuario));
        }
        let base_n = normalizar(nombre, MAX_REPO, "repositorio");
        if base_n == *de {
            continue;
        }
        let libre = |n: &str| !existentes.contains(&n.to_string()) && !repos.iter().any(|(_, a)| a == n) && !d.with_file_name(n).exists();
        let mut a = base_n.clone();
        let mut i = 2;
        while !libre(&a) {
            a = format!("{}-{i}", base_n.chars().take(MAX_REPO - 1 - i.to_string().len()).collect::<String>().trim_end_matches('-'));
            i += 1;
            if i > 999 {
                return Err(format!("No se encontró un nombre libre para «{nombre}»."));
            }
        }
        repos.push((de.clone(), a));
    }
    let usuario = match (&p.legible, &deshecho) {
        (_, Some((Some(antes), _, _))) => {
            if c.usuario_existe(antes) || c.carpetas().iter().any(|x| Path::new(x).join(antes).exists()) {
                return Err(format!("No se puede volver a «{antes}»: ya hay un equipo o una carpeta con ese nombre."));
            }
            Some((p.usuario.clone(), antes.clone()))
        }
        (Some(l), None) => Some((p.usuario.clone(), usuario_legible(c, l)?)),
        _ => None,
    };
    if usuario.is_none() && repos.is_empty() {
        return Err("Ya tienen esos nombres: no hay nada que mover.".into());
    }
    // Nada en marcha en lo que se mueve (con la carpeta del equipo, en todos sus repositorios).
    let mirar: Vec<String> = if usuario.is_some() { repos_de(&dir_u) } else { repos.iter().map(|(de, _)| de.clone()).collect() };
    if let Some(r) = mirar.iter().find(|r| en_uso(&dir_u.join(r))) {
        return Err(format!("Hay algo en marcha en «{}/{r}» (una copia, una poda o una verificación): prueba cuando termine.", p.usuario));
    }
    // Ahora sí: en el disco, guardando cómo deshacerlo.
    let mut deshacer = Deshacer::default();
    let hecho = (|| -> Result<(), String> {
        for (de, a) in &repos {
            let (x, y) = (dir_u.join(de), dir_u.join(a));
            std::fs::rename(&x, &y).map_err(|e| format!("No se pudo mover «{de}»: {e}"))?;
            deshacer.renombrados.push((x, y));
        }
        if let Some((_, nuevo)) = &usuario {
            if dir_u.exists() {
                let y = base.join(nuevo);
                std::fs::rename(&dir_u, &y).map_err(|e| format!("No se pudo mover la carpeta del equipo: {e}"))?;
                deshacer.renombrados.push((dir_u.clone(), y));
            }
            // La misma contraseña: solo cambia el nombre delante del hash.
            let lineas = f.read_htpasswd_de(zona.as_deref());
            let prefijo = format!("{}:", p.usuario);
            if lineas.iter().filter(|l| l.starts_with(&prefijo)).count() != 1 {
                return Err("El usuario del equipo no está (o está repetido) en los usuarios del almacén.".into());
            }
            let nuevas: Vec<String> = lineas.iter().map(|l| l.strip_prefix(&prefijo).map_or_else(|| l.clone(), |h| format!("{nuevo}:{h}"))).collect();
            f.write_htpasswd_de(zona.as_deref(), &nuevas)?;
            deshacer.htpasswd = Some((zona.clone(), lineas));
        }
        Ok(())
    })();
    if let Err(e) = hecho {
        let mal = deshacer.aplicar(f);
        return Err(if mal.is_empty() {
            format!("{e} No se ha cambiado nada.")
        } else {
            format!("{e} No se pudo deshacer: {}. Revísalo en el almacén.", mal.join(", "))
        });
    }
    let m = Movido { usuario, repos, zona, antes: p.usuario.clone(), deshacer };
    // La configuración (en memoria): el usuario, la selección de los espejos y lo movido.
    if let Some((de, a)) = &m.usuario {
        for l in std::iter::once(&mut c.users).chain(c.zonas.iter_mut().map(|z| &mut z.users)) {
            for u in l.iter_mut().filter(|u| u.name == *de) {
                u.para = Some(u.para.clone().unwrap_or_else(|| de.clone()));
                u.name = a.clone();
            }
        }
    }
    renombrar_espejo(c, &m);
    // Al deshacer, aquel cambio se olvida y se apunta este (en el espejo, la carpeta
    // que se deja ahora es la «nueva» de entonces).
    if let Some((_, _, cuando)) = &deshecho {
        let usuario_antes = m.antes.clone();
        c.renombres.retain(|r| !(r.cuando == *cuando && (r.a == usuario_antes || r.a.starts_with(&format!("{usuario_antes}/")))));
    }
    c.renombres.extend(m.renombres(ahora));
    let sobran = c.renombres.len().saturating_sub(MAX_RENOMBRES);
    c.renombres.drain(..sobran);
    Ok(m)
}

/// La selección de los espejos del almacén con los nombres nuevos (trabajos y la vista de antes).
pub fn renombrar_espejo(c: &mut ServerConfig, m: &Movido) {
    let cambia = |l: &mut Vec<String>| {
        for x in l.iter_mut() {
            if let Some(n) = m.ruta_nueva(x) {
                *x = n;
            }
        }
    };
    let Some(e) = c.espejo.as_mut() else { return };
    if e.trabajos.is_empty() {
        for d in e.destinos.iter_mut() {
            if let Some(r) = d.repos.as_mut() {
                cambia(r);
            }
            cambia(&mut d.vistos);
        }
        return;
    }
    let mut trabajos = std::mem::take(&mut e.trabajos);
    for t in trabajos.iter_mut().filter(|t| !t.es_equipo()) {
        match &mut t.que {
            crate::espejo_trabajos::Que::Repos { repos } => cambia(repos),
            crate::espejo_trabajos::Que::Equipos { equipos } => cambia(equipos),
            crate::espejo_trabajos::Que::Todos => {}
        }
        cambia(&mut t.vistos);
    }
    *e = crate::espejo::Espejo::de_trabajos(trabajos);
}

/// Para el motor del espejo: lo movido en esta zona, `(de, a)`.
pub fn antiguas(c: &ServerConfig, zona: Option<&str>) -> Vec<(String, String)> {
    let z = zona.filter(|z| *z != "principal");
    c.renombres.iter().filter(|r| r.zona.as_deref() == z).map(|r| (r.de.clone(), r.a.clone())).collect()
}

/// `guarda_copias { mover }` en este almacén: mueve, guarda (o deshace) y espera a
/// que el rest-server acepte el usuario nuevo. Devuelve el mensaje y el detalle.
pub fn mover(v: &Value) -> Result<(String, Value), String> {
    crate::agent::require_admin()?;
    let p = leer_pedido(v)?;
    let _g = CANDADO.lock().map_err(|_| "Candado roto.".to_string())?;
    let mut c = crate::server::load();
    if !c.enabled {
        return Err("Este equipo no guarda copias.".into());
    }
    if crate::espejo::en_marcha() {
        return Err("Hay un espejo en marcha en este almacén: prueba cuando termine.".into());
    }
    let f = Files::agent();
    let ahora = chrono::Local::now().to_rfc3339();
    let m = mover_en(&f, &mut c, &p, &ahora)?;
    let deshacer = |e: String, ret: bool| -> String {
        if ret {
            let _ = crate::retencion_almacen::renombrar(m.usuario_final(), &m.antes, &m.repos.iter().map(|(d, a)| (a.clone(), d.clone())).collect::<Vec<_>>());
        }
        let mal = m.deshacer.aplicar(&f);
        if mal.is_empty() {
            format!("{e} No se ha cambiado nada.")
        } else {
            format!("{e} No se pudo deshacer: {}. Revísalo en el almacén.", mal.join(", "))
        }
    };
    if let Err(e) = crate::retencion_almacen::renombrar(&m.antes, m.usuario_final(), &m.repos) {
        return Err(deshacer(format!("No se pudo poner al día la retención del almacén: {e}."), false));
    }
    if let Err(e) = crate::server::save(&c) {
        return Err(deshacer(format!("No se pudo guardar la configuración del almacén: {e}."), true));
    }
    let antes = m.antes.clone();
    let partes: Vec<String> = m
        .usuario
        .iter()
        .map(|(de, a)| format!("la carpeta del equipo «{de}» → «{a}»"))
        .chain(m.repos.iter().map(|(de, a)| format!("«{de}» → «{a}»")))
        .collect();
    crate::agent::log(&format!("Servidor de copias: carpetas movidas de «{antes}»: {}.", partes.join(", ")));
    // Que el equipo ya entre con el usuario nuevo cuando le llegue su orden.
    if let Some((_, nuevo)) = &m.usuario {
        esperar_relectura(m.zona.as_deref(), nuevo);
    }
    Ok((format!("Carpetas movidas en el almacén: {}.", partes.join(", ")), m.detalle()))
}

static CANDADO: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Espera a que el rest-server haya leído el `.htpasswd` nuevo. No se puede probar
/// con el usuario movido (el almacén no tiene su contraseña), así que se añade un
/// usuario de prueba de un solo uso, se espera a que lo acepte (lo que prueba que
/// leyó el archivo con el cambio) y se quita. Nunca falla: lo dice en el registro.
fn esperar_relectura(zona: Option<&str>, nuevo: &str) {
    let f = Files::agent();
    let sonda = format!("resguardo-sonda-{}", sufijo());
    let clave = crate::server::new_password();
    let Ok(linea) = crate::server::htpasswd_line(&sonda, &clave) else { return };
    let mut lineas = f.read_htpasswd_de(zona);
    lineas.push(linea);
    if f.write_htpasswd_de(zona, &lineas).is_err() {
        return;
    }
    crate::server::esperar_usuario_de(zona, &sonda, &clave);
    let lineas: Vec<String> = f.read_htpasswd_de(zona).into_iter().filter(|l| !l.starts_with(&format!("{sonda}:"))).collect();
    if let Err(e) = f.write_htpasswd_de(zona, &lineas) {
        crate::agent::log(&format!("Servidor de copias: no se pudo quitar el usuario de prueba tras mover «{nuevo}»: {e}"));
    }
}

#[cfg(test)]
mod tests;
