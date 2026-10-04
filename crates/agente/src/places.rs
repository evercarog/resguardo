//! Destinos: los lugares donde viven los repositorios (un bucket con su
//! clave, un rest-server, un servidor SFTP, una carpeta local o de red).
//!
//! Cada repositorio pertenece a un destino. La agrupación sale de su
//! ubicación (`key`); el destino guarda solo su nombre visible. Ver
//! docs/destinos.md.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Place {
    pub id: String,
    pub name: String,
    /// Clave de agrupación (local, nunca se envía a la web).
    pub key: String,
    /// Ubicaciones de repositorios que se han usado aquí (para los destinos
    /// que no se pueden listar, como un rest-server).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub known: Vec<String>,
}

/// Tipo de destino, con el mismo vocabulario que el informe a la web.
pub fn kind(location: &str) -> &'static str {
    let l = location.trim();
    match l.split_once(':') {
        Some((p, _)) if p.len() == 1 => "local",
        Some(("rest", _)) => "rest",
        Some(("sftp", _)) => "sftp",
        Some(("s3", _)) => "s3",
        Some(("b2", _)) => "b2",
        Some(("azure", _)) => "azure",
        Some(("gs", _)) => "gs",
        Some(("swift", _)) => "swift",
        Some(("rclone", _)) => "rclone",
        None => "local",
        _ => "other",
    }
}

/// Quita `usuario:clave@` de una autoridad.
fn strip_auth(authority: &str) -> &str {
    authority.rsplit_once('@').map_or(authority, |(_, h)| h)
}

/// Ruta local normalizada (barras `/`, sin barra final, en minúsculas en Windows).
fn norm_local(path: &str) -> String {
    let p = path.trim().replace('\\', "/");
    let p = p.trim_end_matches('/').to_string();
    if cfg!(windows) || p.as_bytes().get(1) == Some(&b':') || p.starts_with("//") {
        p.to_lowercase()
    } else {
        p
    }
}

/// Carpeta padre de una ruta local normalizada (la raíz de una unidad o un
/// recurso compartido es su propio destino).
fn local_parent(p: &str) -> String {
    let is_unc = p.starts_with("//");
    let parts: Vec<&str> = p.trim_start_matches('/').split('/').filter(|s| !s.is_empty()).collect();
    // Unidad («e:») o recurso compartido («//nas/copias»): no se sube de ahí.
    let min = if is_unc { 2 } else { 1 };
    if parts.len() <= min {
        return p.to_string();
    }
    let parent = parts[..parts.len() - 1].join("/");
    if is_unc {
        format!("//{parent}")
    } else if p.starts_with('/') {
        format!("/{parent}")
    } else {
        parent
    }
}

/// Servidor y bucket de una ubicación S3 (`s3:https://host/bucket/…` o `s3:host/bucket/…`).
fn s3_parts(rest: &str) -> (String, String) {
    let r = rest.trim_start_matches("https://").trim_start_matches("http://");
    let mut it = r.split('/');
    let host = strip_auth(it.next().unwrap_or("")).to_lowercase();
    let bucket = it.next().unwrap_or("").to_string();
    (host, bucket)
}

/// Clave de agrupación de la ubicación de un repositorio.
pub fn key(location: &str) -> String {
    let l = location.trim();
    let Some((prefix, rest)) = l.split_once(':') else { return local_parent(&norm_local(l)) };
    match prefix {
        p if p.len() == 1 => local_parent(&norm_local(l)),
        "s3" => {
            let (host, bucket) = s3_parts(rest);
            format!("s3:{host}/{bucket}")
        }
        "b2" | "azure" | "gs" | "swift" | "rclone" => format!("{prefix}:{}", rest.split(':').next().unwrap_or("")),
        "rest" => {
            let (scheme, r) = if let Some(r) = rest.strip_prefix("https://") { ("https", r) } else { ("http", rest.trim_start_matches("http://")) };
            let host = strip_auth(r.split('/').next().unwrap_or("")).to_lowercase();
            format!("rest:{scheme}://{host}")
        }
        "sftp" => {
            let r = rest.trim_start_matches("//");
            let authority = r.split([':', '/']).next().unwrap_or("");
            let (user, host) = authority.rsplit_once('@').map_or(("", authority), |(u, h)| (u, h));
            if user.is_empty() {
                format!("sftp:{}", host.to_lowercase())
            } else {
                format!("sftp:{user}@{}", host.to_lowercase())
            }
        }
        _ => l.to_string(),
    }
}

/// Proveedor de un servidor S3 («Backblaze B2», «Wasabi»…).
fn s3_label(host: &str) -> &'static str {
    if host.ends_with("backblazeb2.com") {
        "Backblaze B2"
    } else if host.ends_with("wasabisys.com") {
        "Wasabi"
    } else if host.ends_with("r2.cloudflarestorage.com") {
        "Cloudflare R2"
    } else if host.ends_with("amazonaws.com") || !host.contains('.') {
        "Amazon S3"
    } else {
        "Nube S3"
    }
}

/// Nombre por defecto de un destino. Nunca lleva una ruta local completa (se envía a la web).
pub fn default_name(location: &str) -> String {
    let l = location.trim();
    let k = key(l);
    match kind(l) {
        "s3" => {
            let (host, bucket) = s3_parts(l.split_once(':').map_or("", |x| x.1));
            format!("{} · {bucket}", s3_label(&host))
        }
        "b2" => format!("Backblaze B2 · {}", k.trim_start_matches("b2:")),
        "azure" => format!("Azure · {}", k.trim_start_matches("azure:")),
        "gs" => format!("Google Cloud · {}", k.trim_start_matches("gs:")),
        "swift" => format!("Swift · {}", k.trim_start_matches("swift:")),
        "rclone" => format!("rclone {}", k.trim_start_matches("rclone:")),
        "rest" => format!("Servidor {}", k.rsplit("://").next().unwrap_or("").split(':').next().unwrap_or("")),
        "sftp" => format!("SFTP {}", k.trim_start_matches("sftp:").rsplit('@').next().unwrap_or("")),
        "local" if k.starts_with("//") => {
            let mut parts = k.trim_start_matches('/').split('/');
            let server = parts.next().unwrap_or("");
            let share = parts.next().unwrap_or("");
            if share.is_empty() {
                server.to_string()
            } else {
                format!("{server} · {share}")
            }
        }
        "local" => {
            // «e:/copias» → «Unidad E:»; en Linux/macOS, el nombre de la carpeta.
            let b = k.as_bytes();
            if b.len() >= 2 && b[1] == b':' {
                format!("Unidad {}:", (b[0] as char).to_ascii_uppercase())
            } else {
                k.rsplit('/').find(|s| !s.is_empty()).map_or_else(|| "Carpeta local".to_string(), |s| format!("Carpeta {s}"))
            }
        }
        _ => "Destino".to_string(),
    }
}

/// Asigna un destino a cada repositorio que no lo tenga (o cuyo destino ya no
/// existe), reutilizando el de su misma clave o creando uno. Quita los
/// destinos que se quedaron sin repositorios. Devuelve true si cambió algo.
pub fn assign(places: &mut Vec<Place>, repos: &mut [crate::store::Repo]) -> bool {
    let mut changed = false;
    for repo in repos.iter_mut() {
        if repo.place_id.as_ref().is_some_and(|id| places.iter().any(|p| &p.id == id)) {
            continue;
        }
        let k = key(&repo.location);
        let id = match places.iter().find(|p| p.key == k) {
            Some(p) => p.id.clone(),
            None => {
                let p = Place { id: uuid::Uuid::new_v4().to_string(), name: default_name(&repo.location), key: k, known: Vec::new() };
                let id = p.id.clone();
                places.push(p);
                id
            }
        };
        repo.place_id = Some(id);
        changed = true;
    }
    // Se recuerda dónde hubo repositorios (aunque luego se quiten de la app).
    for repo in repos.iter() {
        if let Some(p) = places.iter_mut().find(|p| Some(&p.id) == repo.place_id.as_ref()) {
            if !p.known.iter().any(|k| k.eq_ignore_ascii_case(&repo.location)) {
                p.known.push(repo.location.clone());
                changed = true;
            }
        }
    }
    let before = places.len();
    places.retain(|p| repos.iter().any(|r| r.place_id.as_deref() == Some(p.id.as_str())));
    changed || places.len() != before
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agrupa_por_lugar() {
        assert_eq!(key("s3:https://s3.us-west-004.backblazeb2.com/copias-ana/portatil"), "s3:s3.us-west-004.backblazeb2.com/copias-ana");
        assert_eq!(key("s3:https://S3.us-west-004.backblazeb2.com/copias-ana/disco"), "s3:s3.us-west-004.backblazeb2.com/copias-ana");
        assert_eq!(key("b2:copias:portatil"), "b2:copias");
        assert_eq!(key("azure:cont:/a/b"), "azure:cont");
        assert_eq!(key("rest:https://ana:clave@NAS.local:8000/ana/portatil/"), "rest:https://nas.local:8000");
        assert_eq!(key("rest:http://nas:8000/"), "rest:http://nas:8000");
        assert_eq!(key("sftp:ana@nas.local:/volume1/restic"), "sftp:ana@nas.local");
        assert_eq!(key("sftp://ana@nas.local:2222//restic"), "sftp:ana@nas.local");
        assert_eq!(key(r"E:\Copias\restic"), "e:/copias");
        assert_eq!(key(r"E:\Copias\fotos\"), "e:/copias");
        assert_eq!(key(r"E:\restic"), "e:");
        assert_eq!(key(r"E:\"), "e:");
        assert_eq!(key(r"\\NAS\copias\portatil"), "//nas/copias");
        assert_eq!(key(r"\\nas\copias"), "//nas/copias");
    }

    #[test]
    fn nombres_sin_rutas_locales() {
        assert_eq!(default_name("s3:https://s3.us-west-004.backblazeb2.com/copias-ana/portatil"), "Backblaze B2 · copias-ana");
        assert_eq!(default_name("rest:https://ana:x@nas.local:8000/ana/"), "Servidor nas.local");
        assert_eq!(default_name("sftp:ana@nas.local:/volume1/restic"), "SFTP nas.local");
        assert_eq!(default_name(r"E:\Copias\Privado\restic"), "Unidad E:");
        assert_eq!(default_name(r"\\nas\copias\portatil"), "nas · copias");
        assert!(!default_name(r"C:\Users\Ana\Secreto\repo").contains("Secreto"));
    }

    fn repo(id: &str, location: &str) -> crate::store::Repo {
        serde_json::from_value(serde_json::json!({ "id": id, "name": id, "location": location })).unwrap()
    }

    #[test]
    fn asigna_sin_perder_nada_y_es_idempotente() {
        let mut places = Vec::new();
        let mut repos = vec![repo("a", r"E:\Copias\restic"), repo("b", r"E:\Copias\fotos"), repo("c", "rest:https://nas:8000/a/")];
        assert!(assign(&mut places, &mut repos));
        assert_eq!(places.len(), 2);
        assert_eq!(repos[0].place_id, repos[1].place_id);
        assert_ne!(repos[0].place_id, repos[2].place_id);
        // Una segunda vez no cambia nada.
        let (p0, r0) = (places.clone(), repos.iter().map(|r| r.place_id.clone()).collect::<Vec<_>>());
        assert!(!assign(&mut places, &mut repos));
        assert_eq!(places, p0);
        assert_eq!(repos.iter().map(|r| r.place_id.clone()).collect::<Vec<_>>(), r0);
        // Un destino sin repositorios desaparece; un id que ya no existe se reasigna.
        repos.truncate(2);
        assert!(assign(&mut places, &mut repos));
        assert_eq!(places.len(), 1);
        repos[0].place_id = Some("no-existe".into());
        assert!(assign(&mut places, &mut repos));
        assert_eq!(repos[0].place_id, repos[1].place_id);
    }
}
