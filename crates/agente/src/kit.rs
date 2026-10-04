//! Kit de recuperación: lo necesario para abrir las copias de un destino si se
//! pierde este equipo (las contraseñas viven cifradas solo aquí).
//!
//! Aquí se preparan los datos que se imprimen, sin secretos: la ubicación sin
//! usuario ni contraseña del servidor, el ID de la clave de la nube (nunca la
//! clave secreta) y el ID del repositorio de restic. La contraseña del destino
//! solo se imprime si el usuario la escribe (la app nunca la revela).

use crate::restic::{self, Access};
use crate::store::Repo;
use serde::{Deserialize, Serialize};

/// Kit guardado: cuándo y para qué destino (si cambia la ubicación o el
/// repositorio, el kit ya no sirve y se vuelve a pedir).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KitStatus {
    /// RFC 3339.
    pub saved_at: String,
    pub location: String,
    /// ID del repositorio de restic (de `restic cat config`), si se conocía.
    #[serde(default)]
    pub config_id: Option<String>,
}

impl KitStatus {
    /// ¿Sigue valiendo para este destino? (Misma ubicación y, si se conocen los dos, mismo repositorio.)
    pub fn matches(&self, location: &str, config_id: Option<&str>) -> bool {
        self.location == location && (self.config_id.is_none() || config_id.is_none() || self.config_id.as_deref() == config_id)
    }
}

/// Datos de la nube que se imprimen (sin la clave secreta).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct CloudInfo {
    /// "s3", "b2", "azure", "gs".
    pub provider: String,
    /// Servidor (S3 compatibles).
    pub endpoint: Option<String>,
    /// Bucket o contenedor y carpeta dentro.
    pub bucket: Option<String>,
    pub prefix: Option<String>,
    pub key_id: Option<String>,
    pub region: Option<String>,
}

/// Un destino en el kit.
#[derive(Debug, Clone, Serialize)]
pub struct KitEntry {
    pub id: String,
    pub name: String,
    /// "local", "rest", "sftp", "s3", "b2", "azure", "gs", "rclone", "other".
    pub kind: String,
    /// Ubicación de restic sin usuario ni contraseña del servidor.
    pub location: String,
    /// REST: ruta del repositorio dentro de la carpeta de datos del servidor.
    pub rest_path: Option<String>,
    /// REST: el servidor pide usuario (no se imprime cuál ni su contraseña).
    pub rest_auth: bool,
    pub cloud: Option<CloudInfo>,
    /// ID del repositorio de restic (`restic cat config`); `None` si no se pudo leer.
    pub config_id: Option<String>,
    /// Por qué no se pudo leer el ID (p. ej. el servidor no responde).
    pub error: Option<String>,
    pub kit: Option<KitStatus>,
}

/// Tipo de ubicación de restic.
pub fn kind_of(location: &str) -> &'static str {
    let Some((prefix, _)) = location.split_once(':') else { return "local" };
    match prefix {
        "rest" => "rest",
        "sftp" => "sftp",
        "s3" => "s3",
        "b2" => "b2",
        "azure" => "azure",
        "gs" => "gs",
        "rclone" => "rclone",
        p if p.len() == 1 => "local", // C:\…
        _ => "other",
    }
}

/// Ubicación sin credenciales: en REST se quita `usuario:contraseña@`.
pub fn public_location(location: &str) -> String {
    for scheme in ["rest:https://", "rest:http://"] {
        if let Some(rest) = location.strip_prefix(scheme) {
            let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
            let host = authority.rsplit('@').next().unwrap_or(authority);
            return format!("{scheme}{host}{path}");
        }
    }
    location.to_string()
}

/// REST: ruta del repositorio en el servidor (`rest:https://h:8000/siigo/` → "siigo").
pub fn rest_path(location: &str) -> Option<String> {
    let rest = location.strip_prefix("rest:https://").or_else(|| location.strip_prefix("rest:http://"))?;
    let path = rest.find('/').map(|i| &rest[i..]).unwrap_or("");
    Some(path.trim_matches('/').to_string())
}

/// Datos de la nube a partir de la ubicación (`s3:https://endpoint/bucket/carpeta`,
/// `b2:bucket:carpeta`, `azure:contenedor:/carpeta`, `gs:bucket:/carpeta`).
pub fn cloud_info(location: &str, key_id: Option<&str>, region: Option<&str>) -> Option<CloudInfo> {
    let kind = kind_of(location);
    let rest = location.split_once(':')?.1;
    let split_path = |p: &str| {
        let p = p.trim_matches('/');
        match p.split_once('/') {
            Some((b, pre)) => (Some(b.to_string()), Some(pre.to_string()).filter(|x| !x.is_empty())),
            None => (Some(p.to_string()).filter(|x| !x.is_empty()), None),
        }
    };
    let mut info = CloudInfo { provider: kind.into(), key_id: key_id.map(String::from), region: region.map(String::from), ..Default::default() };
    match kind {
        "s3" => {
            let r = rest.trim_start_matches("https://").trim_start_matches("http://");
            match r.split_once('/') {
                Some((host, path)) if host.contains('.') || host.contains(':') => {
                    info.endpoint = Some(host.to_string());
                    (info.bucket, info.prefix) = split_path(path);
                }
                _ => (info.bucket, info.prefix) = split_path(r),
            }
        }
        "b2" | "azure" | "gs" => {
            let (bucket, prefix) = rest.split_once(':').unwrap_or((rest, ""));
            info.bucket = Some(bucket.to_string()).filter(|b| !b.is_empty());
            info.prefix = Some(prefix.trim_matches('/').to_string()).filter(|p| !p.is_empty());
        }
        _ => return None,
    }
    Some(info)
}

/// ID del repositorio de restic (`restic cat config`).
pub fn config_id(access: &Access) -> Result<String, String> {
    let out = restic::run_raw(access, &["cat", "config", "--no-lock"], restic::CHECK_TIMEOUT)?;
    if out.code != Some(0) {
        return Err(restic::exit_error(out.code, &out.stderr));
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| format!("Respuesta inesperada de restic: {e}"))?;
    v["id"].as_str().map(String::from).ok_or_else(|| "restic no devolvió el ID del repositorio.".into())
}

/// Lo que se imprime de un destino. `access`: para leer el ID del repositorio
/// (si no hay acceso, el kit se imprime igual, sin él).
pub fn entry(repo: &Repo, access: Option<&Access>) -> KitEntry {
    let (config_id, error) = match access.map(config_id) {
        Some(Ok(id)) => (Some(id), None),
        Some(Err(e)) => (None, Some(e)),
        None => (None, None),
    };
    KitEntry {
        id: repo.id.clone(),
        name: repo.name.clone(),
        kind: kind_of(&repo.location).into(),
        location: public_location(&repo.location),
        rest_path: rest_path(&repo.location),
        rest_auth: repo.rest_username.is_some() || public_location(&repo.location) != repo.location,
        cloud: cloud_info(&repo.location, repo.cloud_key_id.as_deref(), repo.cloud_region.as_deref()),
        config_id,
        error,
        kit: repo.kit.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ubicacion_sin_credenciales() {
        assert_eq!(public_location("rest:http://siigo:clave@192.168.1.30:8001/siigo/"), "rest:http://192.168.1.30:8001/siigo/");
        assert_eq!(public_location("rest:https://ana@h/r"), "rest:https://h/r");
        assert_eq!(public_location("rest:https://h:8000"), "rest:https://h:8000");
        assert_eq!(public_location(r"D:\Copias\restic"), r"D:\Copias\restic");
        assert_eq!(rest_path("rest:http://h:8001/siigo/"), Some("siigo".into()));
        assert_eq!(rest_path("rest:http://h:8001"), Some(String::new()));
        assert_eq!(rest_path("s3:x"), None);
        assert_eq!(kind_of(r"E:\Copias"), "local");
        assert_eq!(kind_of("b2:bucket:carpeta"), "b2");
    }

    #[test]
    fn datos_de_la_nube() {
        let s3 = cloud_info("s3:https://s3.us-west-004.backblazeb2.com/copias-ana/siigo", Some("004abc"), Some("us-west-004")).unwrap();
        assert_eq!(s3.endpoint.as_deref(), Some("s3.us-west-004.backblazeb2.com"));
        assert_eq!((s3.bucket.as_deref(), s3.prefix.as_deref()), (Some("copias-ana"), Some("siigo")));
        assert_eq!(s3.key_id.as_deref(), Some("004abc"));
        let aws = cloud_info("s3:s3.amazonaws.com/mi-bucket", None, None).unwrap();
        assert_eq!((aws.endpoint.as_deref(), aws.bucket.as_deref(), aws.prefix), (Some("s3.amazonaws.com"), Some("mi-bucket"), None));
        let b2 = cloud_info("b2:copias:equipo/siigo", None, None).unwrap();
        assert_eq!((b2.bucket.as_deref(), b2.prefix.as_deref()), (Some("copias"), Some("equipo/siigo")));
        assert!(cloud_info("rest:http://h/r", None, None).is_none());
        assert!(cloud_info(r"D:\x", None, None).is_none());
    }

    #[test]
    fn kit_vigente() {
        let k = KitStatus { saved_at: "x".into(), location: "rest:http://h/r".into(), config_id: Some("abc".into()) };
        assert!(k.matches("rest:http://h/r", Some("abc")));
        assert!(k.matches("rest:http://h/r", None), "sin ID a mano, vale con la ubicación");
        assert!(!k.matches("rest:http://h/otro", Some("abc")));
        assert!(!k.matches("rest:http://h/r", Some("def")), "otro repositorio en la misma ruta");
    }

    #[test]
    fn id_del_repositorio_real() {
        let (Ok(repo), Ok(pw)) = (std::env::var("RESGUARDO_TEST_REPO"), std::env::var("RESGUARDO_TEST_PASSWORD")) else {
            return;
        };
        let id = config_id(&Access::new(repo, pw)).unwrap();
        assert_eq!(id.len(), 64);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
