//! Configuración de repositorios.
//!
//! En `repos.json` solo se guardan datos no secretos (nombre, ubicación,
//! usuario del servidor REST…). Las contraseñas, tanto la del repositorio como
//! la del servidor REST, van al almacén de credenciales del sistema operativo.

use crate::restic::Access;
use crate::retention::Policy;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[cfg(not(target_os = "linux"))]
const KEYRING_SERVICE: &str = "io.github.resguardo";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repo {
    pub id: String,
    pub name: String,
    pub location: String,
    /// Usuario HTTP del rest-server; su contraseña está en el almacén.
    #[serde(default)]
    pub rest_username: Option<String>,
    /// Certificado de CA propio para servidores HTTPS.
    #[serde(default)]
    pub cacert: Option<String>,
    /// Destinos en la nube (S3, B2, Azure): ID de la clave y región. La
    /// clave secreta está en el almacén de credenciales.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_key_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_region: Option<String>,
    /// Versiones anteriores: carpetas y exclusiones del repositorio. Ahora
    /// viven en los planes (se convierten solas al leer la configuración).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excludes: Vec<String>,
    /// Planes de copia: qué se copia, con qué etiquetas y cuándo.
    #[serde(default)]
    pub plans: Vec<crate::plans::Plan>,
    /// Política de retención guardada (solo se usa para la vista previa).
    #[serde(default)]
    pub retention: Option<Policy>,
    /// Cada cuántas horas se espera una copia. None: se detecta del historial.
    #[serde(default)]
    pub expected_hours: Option<u32>,
    /// Kit de recuperación guardado (cuándo y para qué ubicación).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kit: Option<crate::kit::KitStatus>,
    /// Nube: el usuario declara que el bucket tiene bloqueo de objetos (Object Lock).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub object_lock: bool,
    /// REST: última comprobación de si el servidor es de solo añadir.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub append_only: Option<crate::protection::AppendOnlyCheck>,
    /// Destino (lugar) al que pertenece este repositorio (ver places.rs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place_id: Option<String>,
    /// Nombre de su destino: copia de `places.json` que se rellena al leer
    /// (para el agente y la web, que no leen ese archivo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place_name: Option<String>,
}

/// Versiones anteriores guardaban carpetas y exclusiones en el repositorio:
/// pasan a un plan «Principal», con el horario que tenga el agente (si lo
/// tiene). Devuelve true si cambió algo.
fn migrate_to_plans(repos: &mut [Repo]) -> bool {
    let mut changed = false;
    if repos.iter().all(|r| !r.plans.is_empty() || r.paths.is_empty()) {
        return false;
    }
    let agent = crate::agent::load_config();
    for repo in repos.iter_mut() {
        if !repo.plans.is_empty() || repo.paths.is_empty() {
            continue;
        }
        let schedule = agent.repos.iter().find(|r| r.id == repo.id).and_then(|r| {
            // El agente ya convertido: su plan; si no, el horario anterior.
            r.plans
                .iter()
                .find(|p| p.id == crate::plans::LEGACY_PLAN)
                .map(|p| p.schedule.clone())
                .or_else(|| crate::agent::plan_schedule_from_legacy(&r.schedule))
        });
        repo.plans.push(crate::plans::Plan {
            id: crate::plans::LEGACY_PLAN.into(),
            name: "Principal".into(),
            paths: std::mem::take(&mut repo.paths),
            excludes: std::mem::take(&mut repo.excludes),
            tags: vec![],
            schedule,
            skip_unchanged: false,
            ganchos: vec![],
        });
        changed = true;
    }
    changed
}

/// Quita espacios, líneas vacías y duplicados conservando el orden.
fn clean_list(items: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in items {
        let item = item.trim().to_string();
        if !item.is_empty() && !out.contains(&item) {
            out.push(item);
        }
    }
    out
}

pub struct Store {
    path: PathBuf,
    lock: Mutex<()>,
}

impl Store {
    /// Carpeta de configuración de la app (la del historial de la app).
    pub fn config_dir(&self) -> PathBuf {
        self.path.parent().map(Path::to_path_buf).unwrap_or_default()
    }

    pub fn new(config_dir: PathBuf) -> Self {
        Self { path: config_dir.join("repos.json"), lock: Mutex::new(()) }
    }

    fn read(&self) -> Result<Vec<Repo>, String> {
        let mut repos: Vec<Repo> = match fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| format!("Configuración dañada ({}): {e}", self.path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(format!("No se pudo leer la configuración: {e}")),
        };
        if migrate_to_plans(&mut repos) {
            self.write(&repos)?;
        }
        // Cada repositorio, en su destino (sin intervención del usuario; ver docs/destinos.md).
        let mut places = self.read_places();
        if crate::places::assign(&mut places, &mut repos) {
            // Si no se puede escribir, la asignación vale en memoria y se reintenta la próxima vez.
            if self.write_places(&places).is_ok() {
                let _ = self.write(&repos);
            }
        }
        for repo in repos.iter_mut() {
            repo.place_name = places.iter().find(|p| Some(&p.id) == repo.place_id.as_ref()).map(|p| p.name.clone());
        }
        Ok(repos)
    }

    fn places_path(&self) -> PathBuf {
        self.config_dir().join("places.json")
    }

    fn read_places(&self) -> Vec<crate::places::Place> {
        fs::read(self.places_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    fn write_places(&self, places: &[crate::places::Place]) -> Result<(), String> {
        let json = serde_json::to_vec_pretty(places).map_err(|e| e.to_string())?;
        let path = self.places_path();
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json).map_err(|e| format!("No se pudo guardar los destinos: {e}"))?;
        fs::rename(&tmp, &path).map_err(|e| format!("No se pudo guardar los destinos: {e}"))
    }

    /// Destinos (lugares) con sus repositorios ya asignados.
    pub fn places(&self) -> Result<Vec<crate::places::Place>, String> {
        let _guard = self.lock.lock().unwrap();
        let repos = self.read()?;
        let mut places = self.read_places();
        let mut repos = repos;
        crate::places::assign(&mut places, &mut repos);
        Ok(places)
    }

    /// Cambia el nombre visible de un destino (no toca ningún repositorio).
    pub fn rename_place(&self, id: &str, name: &str) -> Result<crate::places::Place, String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 80 {
            return Err("El nombre debe tener entre 1 y 80 caracteres.".into());
        }
        let _guard = self.lock.lock().unwrap();
        let mut repos = self.read()?;
        let mut places = self.read_places();
        crate::places::assign(&mut places, &mut repos);
        let place = places.iter_mut().find(|p| p.id == id).ok_or("Ese destino ya no existe.")?;
        place.name = name.to_string();
        let out = place.clone();
        self.write_places(&places)?;
        Ok(out)
    }

    fn write(&self, repos: &[Repo]) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("No se pudo crear {}: {e}", dir.display()))?;
        }
        let json = serde_json::to_vec_pretty(repos).map_err(|e| e.to_string())?;
        // Escritura atómica: primero a un archivo temporal y luego se renombra.
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, json).map_err(|e| format!("No se pudo guardar la configuración: {e}"))?;
        fs::rename(&tmp, &self.path).map_err(|e| format!("No se pudo guardar la configuración: {e}"))
    }

    pub fn list(&self) -> Result<Vec<Repo>, String> {
        let _guard = self.lock.lock().unwrap();
        self.read()
    }

    pub fn get(&self, id: &str) -> Result<Repo, String> {
        self.list()?.into_iter().find(|r| r.id == id).ok_or_else(|| "Repositorio no encontrado.".to_string())
    }

    /// Guarda un repositorio ya comprobado. `access` trae los secretos, que
    /// van al almacén de credenciales; el resto se escribe en `repos.json`.
    pub fn add(&self, name: String, access: &Access, cloud: Option<CloudCreds>) -> Result<Repo, String> {
        let _guard = self.lock.lock().unwrap();
        let mut repos = self.read()?;
        let repo = Repo {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            location: access.location.clone(),
            rest_username: access.rest_auth.as_ref().map(|(user, _)| user.clone()),
            cacert: access.cacert.clone(),
            cloud_key_id: cloud.as_ref().map(|c| c.key_id.clone()),
            cloud_region: cloud.as_ref().and_then(|c| c.region.clone()),
            paths: Vec::new(),
            excludes: Vec::new(),
            plans: Vec::new(),
            retention: None,
            expected_hours: None,
            kit: None,
            object_lock: false,
            append_only: None,
            place_id: None,
            place_name: None,
        };
        let saved = set_secret(&repo.id, &access.password)
            .and_then(|()| match &access.rest_auth {
                Some((_, pass)) => set_secret(&rest_key(&repo.id), pass),
                None => Ok(()),
            })
            .and_then(|()| match &cloud {
                Some(c) => set_secret(&cloud_key(&repo.id), &c.key_secret),
                None => Ok(()),
            });
        repos.push(repo);
        // El destino del repositorio nuevo: el de su misma ubicación o uno nuevo.
        let mut places = self.read_places();
        crate::places::assign(&mut places, &mut repos);
        let mut repo = repos.last().cloned().unwrap();
        repo.place_name = places.iter().find(|p| Some(&p.id) == repo.place_id.as_ref()).map(|p| p.name.clone());
        let _ = self.write_places(&places);
        if let Err(e) = saved.and_then(|()| self.write(&repos)) {
            let _ = delete_secrets(&repo.id);
            return Err(e);
        }
        Ok(repo)
    }

    /// Guarda los planes de copia del repositorio (limpios y comprobados).
    pub fn set_plans(&self, id: &str, plans: Vec<crate::plans::Plan>) -> Result<Repo, String> {
        if plans.len() > 20 {
            return Err("Demasiados planes (máximo 20 por repositorio).".into());
        }
        // Carpetas: solo se comprueban las de copias nuevas o cambiadas (una
        // copia antigua a un disco desconectado no impide editar las demás).
        let current = self.get(id).map(|r| r.plans).unwrap_or_default();
        let mut clean: Vec<crate::plans::Plan> = Vec::new();
        for mut p in plans {
            if p.id.is_empty() {
                p.id = uuid::Uuid::new_v4().simple().to_string()[..12].to_string();
            }
            p.name = p.name.trim().to_string();
            p.paths = clean_list(p.paths);
            p.excludes = clean_list(p.excludes);
            p.tags = clean_list(p.tags);
            let unchanged = current.iter().any(|q| q.id == p.id && q.paths == p.paths);
            if !unchanged {
                if let Some(missing) = p.paths.iter().find(|x| !std::path::Path::new(x).exists()) {
                    return Err(format!("La carpeta no existe: {missing} (copia «{}»).", p.name));
                }
            }
            p.validate()?;
            if clean.iter().any(|q| q.id == p.id) {
                return Err("Hay dos planes con el mismo identificador.".into());
            }
            if clean.iter().any(|q| q.name.eq_ignore_ascii_case(&p.name)) {
                return Err(format!("Ya hay un plan llamado «{}».", p.name));
            }
            clean.push(p);
        }

        let _guard = self.lock.lock().unwrap();
        let mut repos = self.read()?;
        let repo = repos.iter_mut().find(|r| r.id == id).ok_or_else(|| "Repositorio no encontrado.".to_string())?;
        repo.plans = clean;
        repo.paths.clear();
        repo.excludes.clear();
        let updated = repo.clone();
        self.write(&repos)?;
        Ok(updated)
    }

    /// Mueve un plan a otro destino (repositorio). Si el destino ya tiene un
    /// plan con el mismo nombre o id, el movido se renombra o cambia de id.
    pub fn move_plan(&self, from: &str, plan_id: &str, to: &str) -> Result<(Repo, Repo), String> {
        if from == to {
            return Err("El plan ya está en ese repositorio.".into());
        }
        let _guard = self.lock.lock().unwrap();
        let mut repos = self.read()?;
        let src = repos.iter().position(|r| r.id == from).ok_or("Repositorio de origen no encontrado.")?;
        let dst = repos.iter().position(|r| r.id == to).ok_or("Repositorio nuevo no encontrado.")?;
        let pos = repos[src].plans.iter().position(|p| p.id == plan_id).ok_or("Ese plan ya no existe.")?;
        let mut plan = repos[src].plans.remove(pos);
        if repos[dst].plans.len() >= 20 {
            return Err("El repositorio nuevo ya tiene el máximo de planes (20).".into());
        }
        if repos[dst].plans.iter().any(|p| p.id == plan.id) {
            plan.id = uuid::Uuid::new_v4().simple().to_string()[..12].to_string();
        }
        let base = plan.name.clone();
        let mut n = 2;
        while repos[dst].plans.iter().any(|p| p.name.eq_ignore_ascii_case(&plan.name)) {
            plan.name = format!("{base} ({n})");
            n += 1;
        }
        repos[dst].plans.push(plan);
        let (a, b) = (repos[src].clone(), repos[dst].clone());
        self.write(&repos)?;
        Ok((a, b))
    }

    pub fn rename(&self, id: &str, name: &str) -> Result<Repo, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("El nombre no puede estar vacío.".into());
        }
        if name.chars().count() > 80 {
            return Err("El nombre es demasiado largo (máximo 80 caracteres).".into());
        }
        let _guard = self.lock.lock().unwrap();
        let mut repos = self.read()?;
        let repo = repos.iter_mut().find(|r| r.id == id).ok_or_else(|| "Repositorio no encontrado.".to_string())?;
        repo.name = name.to_string();
        let updated = repo.clone();
        self.write(&repos)?;
        Ok(updated)
    }

    pub fn set_expected_hours(&self, id: &str, hours: Option<u32>) -> Result<Repo, String> {
        if hours.is_some_and(|h| h == 0 || h > 24 * 366) {
            return Err("Frecuencia no válida.".into());
        }
        let _guard = self.lock.lock().unwrap();
        let mut repos = self.read()?;
        let repo = repos.iter_mut().find(|r| r.id == id).ok_or_else(|| "Repositorio no encontrado.".to_string())?;
        repo.expected_hours = hours;
        let updated = repo.clone();
        self.write(&repos)?;
        Ok(updated)
    }

    pub fn set_retention(&self, id: &str, policy: Option<Policy>) -> Result<Repo, String> {
        if let Some(p) = &policy {
            p.validate()?;
        }
        let _guard = self.lock.lock().unwrap();
        let mut repos = self.read()?;
        let repo = repos.iter_mut().find(|r| r.id == id).ok_or_else(|| "Repositorio no encontrado.".to_string())?;
        repo.retention = policy.filter(|p| !p.is_empty());
        let updated = repo.clone();
        self.write(&repos)?;
        Ok(updated)
    }

    /// Cambia un dato de un destino y lo guarda.
    pub fn update(&self, id: &str, f: impl FnOnce(&mut Repo)) -> Result<Repo, String> {
        let _guard = self.lock.lock().unwrap();
        let mut repos = self.read()?;
        let repo = repos.iter_mut().find(|r| r.id == id).ok_or_else(|| "Repositorio no encontrado.".to_string())?;
        f(repo);
        let updated = repo.clone();
        self.write(&repos)?;
        Ok(updated)
    }

    /// Anota (o quita) el kit de recuperación guardado de un destino.
    pub fn set_kit(&self, id: &str, kit: Option<crate::kit::KitStatus>) -> Result<Repo, String> {
        let _guard = self.lock.lock().unwrap();
        let mut repos = self.read()?;
        let repo = repos.iter_mut().find(|r| r.id == id).ok_or_else(|| "Repositorio no encontrado.".to_string())?;
        repo.kit = kit;
        let updated = repo.clone();
        self.write(&repos)?;
        Ok(updated)
    }

    /// Olvida el repositorio en la app. No toca los datos del repositorio.
    pub fn remove(&self, id: &str) -> Result<(), String> {
        let _guard = self.lock.lock().unwrap();
        let mut repos = self.read()?;
        repos.retain(|r| r.id != id);
        self.write(&repos)?;
        delete_secrets(id)
    }
}

/// Datos de acceso completos de un repositorio guardado.
pub fn access(repo: &Repo) -> Result<Access, String> {
    let rest_auth = match &repo.rest_username {
        Some(user) => Some((user.clone(), get_secret(&rest_key(&repo.id))?)),
        None => None,
    };
    let env = match &repo.cloud_key_id {
        Some(id) => {
            let secret = get_secret(&cloud_key(&repo.id))?;
            crate::tasks::cloud_env(&repo.location, repo.cloud_region.as_deref(), Some(id), Some(&secret))
        }
        None => Vec::new(),
    };
    Ok(Access { location: repo.location.clone(), password: get_secret(&repo.id)?, rest_auth, cacert: repo.cacert.clone(), env })
}

/// Compara en tiempo constante para no revelar cuántos caracteres coinciden.
fn same_secret(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Confirma que `password` es la contraseña del repositorio antes de una
/// acción destructiva o que cambia su configuración.
///
/// Se compara con la guardada en el almacén; si no se puede leer (por ejemplo,
/// se borró a mano), se comprueba contra el propio repositorio con restic.
pub fn verify_password(repo: &Repo, password: &str) -> Result<(), String> {
    const WRONG: &str = "Contraseña incorrecta.";
    if password.is_empty() {
        return Err(WRONG.into());
    }
    match get_secret(&repo.id) {
        Ok(stored) if same_secret(&stored, password) => Ok(()),
        Ok(_) => Err(WRONG.into()),
        Err(_) => {
            let mut access = access(repo).unwrap_or_else(|_| Access::new(repo.location.clone(), ""));
            access.password = password.to_string();
            crate::restic::run(&access, &["cat", "config", "--no-lock"]).map(|_| ())
        }
    }
}

/// Cambia la contraseña guardada de un repositorio (no la del repositorio:
/// para cuando se cambió en otro sitio, p. ej. con `restic key passwd`).
pub fn set_password(id: &str, password: &str) -> Result<(), String> {
    set_secret(id, password)
}

fn rest_key(id: &str) -> String {
    format!("{id}:rest")
}

#[cfg(not(target_os = "linux"))]
fn entry(key: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, key).map_err(|e| format!("Almacén de credenciales no disponible: {e}"))
}

#[cfg(not(target_os = "linux"))]
fn set_secret(key: &str, secret: &str) -> Result<(), String> {
    entry(key)?.set_password(secret).map_err(|e| format!("No se pudo guardar la contraseña: {e}"))
}

#[cfg(not(target_os = "linux"))]
fn get_secret(key: &str) -> Result<String, String> {
    entry(key)?.get_password().map_err(|e| format!("No se pudo leer la contraseña del almacén de credenciales: {e}"))
}

#[cfg(not(target_os = "linux"))]
fn delete_secret(key: &str) -> Result<(), String> {
    match entry(key)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("No se pudo borrar la contraseña: {e}")),
    }
}

// En Linux solo corre el agente (sus secretos van en su carpeta privada): no
// hay almacén de credenciales del usuario.
#[cfg(target_os = "linux")]
const SIN_ALMACEN: &str = "El almacén de credenciales del usuario no está disponible en Linux.";
#[cfg(target_os = "linux")]
fn set_secret(_: &str, _: &str) -> Result<(), String> {
    Err(SIN_ALMACEN.into())
}
#[cfg(target_os = "linux")]
fn get_secret(_: &str) -> Result<String, String> {
    Err(SIN_ALMACEN.into())
}
#[cfg(target_os = "linux")]
fn delete_secret(_: &str) -> Result<(), String> {
    Ok(())
}

fn delete_secrets(id: &str) -> Result<(), String> {
    delete_secret(id)?;
    delete_secret(&rest_key(id))?;
    delete_secret(&cloud_key(id))
}

fn cloud_key(id: &str) -> String {
    format!("{id}:cloud")
}

/// Credenciales de nube de un destino (S3, B2, Azure).
pub struct CloudCreds {
    pub key_id: String,
    pub key_secret: String,
    pub region: Option<String>,
}

/// Credenciales de nube guardadas de un destino (para reutilizarlas en otro).
pub fn cloud_creds(repo: &Repo) -> Result<Option<CloudCreds>, String> {
    match &repo.cloud_key_id {
        Some(id) => Ok(Some(CloudCreds { key_id: id.clone(), key_secret: get_secret(&cloud_key(&repo.id))?, region: repo.cloud_region.clone() })),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planes_conversion_validacion_y_mover() {
        let dir = std::env::temp_dir().join(format!("resguardo-store-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let data = dir.join("datos");
        fs::create_dir_all(&data).unwrap();
        let data = data.to_string_lossy().into_owned();
        // Configuración de una versión anterior: carpetas en el repositorio.
        let legacy = serde_json::json!([
            { "id": "a", "name": "Siigo", "location": "rest:http://x/siigo", "paths": [data], "excludes": ["*.tmp"] },
            { "id": "b", "name": "Nube", "location": "s3:https://x/b" }
        ]);
        fs::write(dir.join("repos.json"), legacy.to_string()).unwrap();
        let store = Store::new(dir.clone());
        let repos = store.list().unwrap();
        assert_eq!(repos[0].plans.len(), 1);
        assert_eq!(repos[0].plans[0].id, crate::plans::LEGACY_PLAN);
        assert_eq!(repos[0].plans[0].excludes, vec!["*.tmp".to_string()]);
        assert!(repos[0].paths.is_empty());
        // Se guardó convertido: el archivo ya no tiene "paths" en el repositorio.
        let saved: serde_json::Value = serde_json::from_slice(&fs::read(dir.join("repos.json")).unwrap()).unwrap();
        assert!(saved[0].get("paths").is_none());

        // Guardar planes: se asigna id, se limpian listas y se valida.
        let mut plan = repos[0].plans[0].clone();
        plan.id = String::new();
        plan.name = "  Domingo ".into();
        plan.tags = vec!["semanal".into(), "semanal".into()];
        let updated = store.set_plans("a", vec![repos[0].plans[0].clone(), plan.clone()]).unwrap();
        assert_eq!(updated.plans.len(), 2);
        assert!(!updated.plans[1].id.is_empty());
        assert_eq!(updated.plans[1].name, "Domingo");
        assert_eq!(updated.plans[1].tags, vec!["semanal".to_string()]);
        let mut dup = plan.clone();
        dup.name = "principal".into();
        assert!(store.set_plans("a", vec![repos[0].plans[0].clone(), dup]).is_err(), "nombre repetido");
        let mut missing = plan.clone();
        missing.paths = vec![dir.join("no-existe").to_string_lossy().into_owned()];
        assert!(store.set_plans("a", vec![missing]).is_err(), "ruta inexistente");

        // Mover un plan a otro destino.
        let domingo = updated.plans[1].id.clone();
        let (a, b) = store.move_plan("a", &domingo, "b").unwrap();
        assert_eq!(a.plans.len(), 1);
        assert_eq!(b.plans.len(), 1);
        assert_eq!(b.plans[0].name, "Domingo");
        assert!(store.move_plan("a", "no-existe", "b").is_err());
        assert!(store.move_plan("a", crate::plans::LEGACY_PLAN, "a").is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn comparacion_de_secretos() {
        assert!(same_secret("abc", "abc"));
        assert!(!same_secret("abc", "abd"));
        assert!(!same_secret("abc", "abcd"));
        assert!(!same_secret("", "a"));
    }
}
