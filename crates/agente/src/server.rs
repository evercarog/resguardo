//! «Servidor de copias de Resguardo» (fase 4, docs/compartir.md): este equipo
//! guarda las copias de otros equipos en una carpeta, con el rest-server
//! oficial de restic.
//!
//! - Siempre `--append-only --private-repos`: cada equipo solo añade a su
//!   carpeta `/<usuario>/`; nadie borra ni ve lo de otro.
//! - Un usuario por equipo cliente, con contraseña aleatoria (bcrypt en
//!   `.htpasswd`); la recibe el cliente con la entrega cifrada de la fase 3.
//! - TLS con un certificado propio generado aquí (los clientes lo fijan).
//! - Regla del firewall solo para el puerto (opcionalmente, solo la red local).
//!   En Linux, una tabla propia de nftables (`inet resguardo`).
//! - Lo arranca una tarea de SYSTEM al iniciar (`resguardo.exe --server-run`;
//!   en Linux, el servicio `resguardo-guarda-copias` de systemd), que
//!   comprueba el hash del binario antes de cada arranque y lo vuelve a
//!   lanzar si se cae. Nunca se abren puertos en el router (sin UPnP).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const TASK_NAME: &str = "Resguardo Servidor de copias";
/// Con `resguardo-agente.exe`, otros nombres: la tarea y la regla de la app
/// de escritorio (si también está instalada) no se tocan.
const TASK_NAME_AGENTE: &str = "Resguardo Agente Servidor de copias";

pub fn nombre_tarea() -> &'static str {
    if crate::agent::is_managed_agent() {
        TASK_NAME_AGENTE
    } else {
        TASK_NAME
    }
}

fn regla_firewall() -> &'static str {
    nombre_tarea()
}

/// SHA-256 del rest-server oficial que acompaña a esta versión (0.14.0,
/// comprobado con el SHA256SUMS del proyecto; ver scripts/fetch-rest-server.ps1
/// y, en Linux x86_64, scripts/fetch-binarios-linux.sh, que lo mete en el
/// paquete). Se puede sustituir al compilar con RESGUARDO_REST_SERVER_SHA256.
/// En otras plataformas (Linux aarch64) no hay huella fijada: la pone el
/// administrador (HUELLA_LOCAL) o el servidor de copias no arranca.
pub const PINNED_SHA256: Option<&str> = match option_env!("RESGUARDO_REST_SERVER_SHA256") {
    Some(v) => Some(v),
    None if cfg!(windows) => Some("66e185d33b4777bcc3b939085e48fc487de597f7c8607727f82b6be5a01d15f4"),
    None if cfg!(all(target_os = "linux", target_arch = "x86_64")) => Some("ec4fa7c3472bdc1cde6bfa994d1ee3344bb6e9dc3d9c10c8ff5316e3e77af6fd"),
    None => None,
};

/// Nombre del binario de rest-server en este sistema.
const BINARIO: &str = if cfg!(windows) { "rest-server.exe" } else { "rest-server" };

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ServerUser {
    pub name: String,
    pub created_at: String,
    /// Tarea 7b: para qué equipo se creó (el nombre pedido, normalizado). Con
    /// zonas, un mismo equipo puede tener un usuario en cada una («ana»,
    /// «ana-2»): así se sabe si ya lo tiene en esta. Sin él, `name`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub para: Option<String>,
}

impl ServerUser {
    fn es_para(&self, base: &str) -> bool {
        self.name == base || self.para.as_deref() == Some(base)
    }
}

/// Tarea 7b (docs/copias-en-cadena.md): otra carpeta que sirve este almacén,
/// con su propio rest-server en su puerto (la misma autoridad y el mismo
/// certificado, siempre `--append-only --private-repos`) y sus propios
/// usuarios (`servidor-zona-<id>.htpasswd`). La «principal» es la de siempre
/// (`ServerConfig::path`, `port` y `users`).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Zona {
    /// `z` y 6 cifras hexadecimales (lo pone el agente).
    pub id: String,
    /// Lo que se ve («Disco E»).
    #[serde(default)]
    pub nombre: String,
    pub path: String,
    pub port: u16,
    #[serde(default)]
    pub users: Vec<ServerUser>,
    #[serde(default)]
    pub creada: String,
}

/// Como mucho, zonas además de la principal.
pub const ZONAS_MAX: usize = 8;

/// Id de una zona: `z` y 6 cifras hexadecimales en minúscula.
pub fn zona_id_valido(id: &str) -> bool {
    id.len() == 7 && id.starts_with('z') && id[1..].chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerConfig {
    pub enabled: bool,
    /// Carpeta donde se guardan las copias de los demás.
    pub path: String,
    pub port: u16,
    /// El firewall solo deja entrar desde la red local.
    pub local_subnet_only: bool,
    #[serde(default)]
    pub users: Vec<ServerUser>,
    /// Huella SHA-256 de la autoridad propia que fijan los equipos (AB:CD:…).
    #[serde(default)]
    pub tls_sha256: Option<String>,
    /// Nombres e IP que cubre el certificado del servidor.
    #[serde(default)]
    pub cert_names: Vec<String>,
    /// Copia nocturna de todo lo guardado a otra carpeta (otro disco): espejo.rs.
    #[serde(default)]
    pub espejo: Option<crate::espejo::Espejo>,
    /// Tarea 7b: otras carpetas (otros discos) que sirve, cada una en su puerto.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub zonas: Vec<Zona>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            path: String::new(),
            port: 8000,
            local_subnet_only: true,
            users: Vec::new(),
            tls_sha256: None,
            cert_names: Vec::new(),
            espejo: None,
            zonas: Vec::new(),
        }
    }
}

impl ServerConfig {
    /// Tarea 7d.2: la carpeta de una zona (`None` o `"principal"`: la principal).
    pub fn carpeta_zona(&self, zona: Option<&str>) -> Option<&str> {
        match zona {
            None | Some("principal") => Some(self.path.as_str()),
            Some(id) => self.zonas.iter().find(|z| z.id == id).map(|z| z.path.as_str()),
        }
    }

    /// Los puertos de todo el almacén: el de la principal y el de cada zona.
    pub fn puertos(&self) -> Vec<u16> {
        std::iter::once(self.port).chain(self.zonas.iter().map(|z| z.port)).collect()
    }

    /// Los usuarios de una zona (`None`: la principal).
    pub fn usuarios(&self, zona: Option<&str>) -> Option<&Vec<ServerUser>> {
        match zona {
            None => Some(&self.users),
            Some(id) => self.zonas.iter().find(|z| z.id == id).map(|z| &z.users),
        }
    }

    fn usuarios_mut(&mut self, zona: Option<&str>) -> Option<&mut Vec<ServerUser>> {
        match zona {
            None => Some(&mut self.users),
            Some(id) => self.zonas.iter_mut().find(|z| z.id == id).map(|z| &mut z.users),
        }
    }

    /// ¿Hay ya un usuario con ese nombre en alguna zona (o en la principal)?
    /// Son únicos en todo el almacén: así la retención en el almacén
    /// (`{ usuario, repo }`) sabe siempre en qué carpeta está.
    pub fn usuario_existe(&self, nombre: &str) -> bool {
        self.users.iter().chain(self.zonas.iter().flat_map(|z| z.users.iter())).any(|u| u.name == nombre)
    }

    /// La carpeta de la zona donde está el usuario (la principal o una zona).
    pub fn carpeta_de_usuario(&self, nombre: &str) -> Option<(&str, Vec<String>)> {
        if self.users.iter().any(|u| u.name == nombre) {
            return Some((self.path.as_str(), self.users.iter().map(|u| u.name.clone()).collect()));
        }
        self.zonas.iter().find(|z| z.users.iter().any(|u| u.name == nombre)).map(|z| (z.path.as_str(), z.users.iter().map(|u| u.name.clone()).collect()))
    }

    /// Todas las carpetas que sirve (la principal y las zonas).
    pub fn carpetas(&self) -> Vec<&str> {
        std::iter::once(self.path.as_str()).chain(self.zonas.iter().map(|z| z.path.as_str())).collect()
    }
}

/// ¿Dentro de la carpeta de Windows, de los programas o de Resguardo?
pub(crate) fn carpeta_del_sistema(p: &Path) -> bool {
    let mut raices = vec![crate::agent::agent_dir()];
    for var in ["SystemRoot", "ProgramFiles", "ProgramFiles(x86)"] {
        if let Ok(v) = std::env::var(var) {
            raices.push(v.into());
        }
    }
    let p = p.to_string_lossy().to_lowercase();
    raices.iter().map(|r| r.to_string_lossy().to_lowercase()).any(|r| !r.is_empty() && Path::new(&p).starts_with(Path::new(&r)))
}

/// ¿Una carpeta está dentro de la otra (o son la misma)? Por componentes y, en
/// Windows, sin distinguir mayúsculas («E:\Copias» y «e:\copias\x» se solapan).
fn se_solapan(a: &str, b: &str) -> bool {
    let norma = |x: &str| if cfg!(windows) { x.to_lowercase() } else { x.to_string() };
    let (a, b) = (norma(a), norma(b));
    let (a, b) = (Path::new(&a), Path::new(&b));
    !a.as_os_str().is_empty() && !b.as_os_str().is_empty() && (a.starts_with(b) || b.starts_with(a))
}

/// Activa, cambia o quita (con `None`) el espejo del Servidor de copias en la forma
/// de antes (`destinos`, de una consola anterior o de la línea de órdenes). Se guarda
/// como trabajos equivalentes (plan 0.7.26). Si el almacén ya tiene trabajos que esa
/// forma no sabe decir (en cadena, «igual que el origen», varios al mismo destino…),
/// se rechaza: así una consola anterior no deshace lo que hizo una nueva.
pub fn poner_espejo(nuevo: Option<crate::espejo::Espejo>) -> Result<String, String> {
    crate::agent::require_admin()?;
    let c = load();
    if !c.enabled {
        return Err("Activa antes el Servidor de copias.".into());
    }
    if let Some(e) = &c.espejo {
        let actuales = e.trabajos_efectivos();
        if actuales.iter().any(|t| !crate::espejo_trabajos::exacto(t, &actuales, e.limite_kib)) {
            return Err("Este almacén tiene espejos que esta consola no sabe cambiar (en cadena, «igual que el origen», pausados…): cámbialos desde una consola actualizada.".into());
        }
    }
    let Some(mut nuevo) = nuevo else { return poner_trabajos(None) };
    nuevo.normalizar();
    if nuevo.destinos.is_empty() {
        return Err("Falta al menos un destino para el espejo.".into());
    }
    if chrono::NaiveTime::parse_from_str(&nuevo.hora, "%H:%M").is_err() {
        return Err("Hora no válida (HH:MM).".into());
    }
    for d in nuevo.destinos.iter_mut() {
        d.carpeta = d.carpeta.trim().to_string();
        if !matches!(d.tipo.as_str(), "carpeta" | "zona" | "nube") {
            return Err("Tipo de destino del espejo no válido (carpeta o nube).".into());
        }
    }
    let n = nuevo.destinos.len();
    if (0..n).any(|i| (0..i).any(|j| nuevo.destinos[i].mismo(&nuevo.destinos[j]))) {
        return Err("Hay un destino repetido en el espejo.".into());
    }
    let mut trabajos = nuevo.trabajos_efectivos();
    crate::espejo_trabajos::mapear_ids(&mut trabajos, &c.espejo.as_ref().map(|e| e.trabajos_efectivos()).unwrap_or_default());
    let texto = nuevo.destinos.iter().map(|d| format!("«{}»", d.texto())).collect::<Vec<_>>().join(" y ");
    let cuando =
        if nuevo.destinos.iter().all(|d| d.horario.is_none() && !d.tras_copia) { format!("cada día a las {}", nuevo.hora) } else { "con su horario".into() };
    poner_trabajos(Some(trabajos))?;
    Ok(format!("Espejo {cuando} en {texto}."))
}

/// Plan 0.7.26 (bloque 4): guarda los trabajos de espejo del almacén (o los quita
/// todos, con `None` o una lista vacía). Comprueba cada origen y cada destino aquí.
pub fn poner_trabajos(nuevos: Option<Vec<crate::espejo_trabajos::Trabajo>>) -> Result<String, String> {
    use crate::espejo_trabajos as et;
    crate::agent::require_admin()?;
    let mut c = load();
    if !c.enabled {
        return Err("Activa antes el Servidor de copias.".into());
    }
    let anterior = c.espejo.as_ref().map(|e| e.trabajos_efectivos()).unwrap_or_default();
    let Some(mut nuevos) = nuevos.filter(|n| !n.is_empty()) else {
        c.espejo = None;
        save(&c)?;
        return Ok("Espejo quitado (lo ya copiado se queda en su destino).".into());
    };
    let nubes = crate::nube::cargar();
    for t in nuevos.iter_mut() {
        et::validar(t, et::QUIEN_ALMACEN)?;
        match t.adonde.tipo.as_str() {
            "carpeta" => {
                let d = &t.adonde.carpeta;
                crate::platform::carpeta_local_valida(d)?;
                if se_solapan(d, &c.path) {
                    return Err("La carpeta del espejo no puede estar dentro de la del Servidor de copias (ni al revés).".into());
                }
                if c.zonas.iter().any(|z| se_solapan(d, &z.path)) {
                    return Err("La carpeta del espejo no puede estar dentro de una zona del Servidor de copias (ni al revés).".into());
                }
                if carpeta_del_sistema(Path::new(d)) {
                    return Err("La carpeta del espejo no puede estar en la carpeta de Windows, de los programas o de Resguardo.".into());
                }
            }
            // Tarea 7d.2: otra zona de este almacén, carpeta a carpeta (sin rest-server de por medio).
            "zona" => {
                if c.carpeta_zona(Some(&t.adonde.carpeta)).is_none() {
                    return Err("Esa zona ya no está en este almacén.".into());
                }
            }
            "nube" => {
                let nombre = t.adonde.nube.as_deref().unwrap_or_default();
                if !nubes.iter().any(|n| n.nombre == nombre) {
                    return Err(format!("No hay ninguna nube «{nombre}» conectada en este equipo: conéctala antes desde la consola («Conectar Dropbox»)."));
                }
            }
            _ => return Err("Adónde no válido (carpeta, zona o nube).".into()),
        }
        // Tarea 7d.2: el origen (sin él, la principal) tiene que ser una zona de aquí.
        if c.carpeta_zona(t.zona.as_deref()).is_none() {
            return Err(format!("La zona de origen de «{}» ya no está en este almacén.", t.nombre));
        }
    }
    et::validar_conjunto(&nuevos)?;
    // Las carpetas del espejo, solo para SYSTEM y Administradores: SYSTEM
    // escribe en ellas (nada de enlaces puestos por un usuario).
    for t in nuevos.iter().filter(|t| t.adonde.tipo == "carpeta") {
        crate::platform::carpeta_privada(Path::new(&t.adonde.carpeta))?;
    }
    // Lo ya hecho en los que siguen se conserva (su última vuelta cuenta para el
    // horario); uno nuevo (o a otro destino) empieza sin nada anotado (§3b).
    et::conservar_estado(&mut nuevos, &anterior, &et::olvidar_estado);
    et::fijar_vistos(&mut nuevos, &anterior, &|t| crate::espejo::repos_en(Path::new(c.carpeta_zona(t.zona.as_deref()).unwrap_or(&c.path))));
    nuevos.sort_by_key(|t| t.orden);
    let n = nuevos.len();
    c.espejo = Some(crate::espejo::Espejo::de_trabajos(nuevos));
    save(&c)?;
    Ok(if n == 1 { "Espejo guardado.".into() } else { format!("{n} espejos guardados.") })
}

/// Dónde están los archivos del servidor: en producción, la carpeta del
/// agente (lo público) y su carpeta privada (claves y usuarios); en las
/// pruebas, una carpeta temporal.
pub struct Files {
    pub public: PathBuf,
    pub private: PathBuf,
}

impl Files {
    pub fn agent() -> Self {
        Self { public: crate::agent::agent_dir(), private: crate::agent::private_dir() }
    }
    /// Autoridad propia: lo que fijan los equipos (`--cacert`).
    pub fn ca(&self) -> PathBuf {
        self.public.join("servidor-tls.crt")
    }
    /// Certificado del servidor (con sus IP) y la autoridad detrás.
    pub fn leaf(&self) -> PathBuf {
        self.public.join("servidor-tls-servidor.crt")
    }
    pub fn private(&self, name: &str) -> PathBuf {
        self.private.join(name)
    }
    pub fn htpasswd(&self) -> PathBuf {
        self.private("servidor.htpasswd")
    }
    /// Los usuarios de una zona (7b) o, sin ella, los de la principal.
    pub fn htpasswd_de(&self, zona: Option<&str>) -> PathBuf {
        match zona {
            None => self.htpasswd(),
            Some(id) => self.private(&format!("servidor-zona-{id}.htpasswd")),
        }
    }
    /// El PID del rest-server de una zona (o de la principal).
    pub fn pid_de(&self, zona: Option<&str>) -> PathBuf {
        match zona {
            None => self.private("servidor.pid"),
            Some(id) => self.private(&format!("servidor-zona-{id}.pid")),
        }
    }

    fn write_private(&self, name: &str, content: &str) -> Result<(), String> {
        let tmp = self.private(&format!("{name}.tmp"));
        let _ = std::fs::remove_file(&tmp);
        std::fs::write(&tmp, content).map_err(|e| format!("No se pudo guardar la clave del certificado: {e}"))?;
        std::fs::rename(&tmp, self.private(name)).map_err(|e| e.to_string())
    }

    /// Crea la autoridad propia y el certificado del servidor firmado por
    /// ella. Devuelve la huella SHA-256 de la autoridad.
    pub fn make_cert(&self, names: &[String]) -> Result<String, String> {
        let (ca_pem, ca_key, fp) = generate_ca()?;
        self.write_private("servidor-ca.key", &ca_key)?;
        std::fs::write(self.ca(), ca_pem).map_err(|e| format!("No se pudo guardar el certificado: {e}"))?;
        self.issue_leaf(names)?;
        Ok(fp)
    }

    /// Vuelve a emitir el certificado del servidor con la misma autoridad.
    pub fn issue_leaf(&self, names: &[String]) -> Result<(), String> {
        let ca_key = std::fs::read_to_string(self.private("servidor-ca.key"))
            .map_err(|_| "Falta la clave de la autoridad del servidor: vuelve a activar el Servidor de copias.".to_string())?;
        let ca_pem = std::fs::read_to_string(self.ca()).map_err(|_| "Falta el certificado de la autoridad del servidor.".to_string())?;
        let (pem, key) = generate_leaf(&ca_key, names)?;
        self.write_private("servidor-tls.key", &key)?;
        // El del servidor y, detrás, el de la autoridad (la cadena completa).
        std::fs::write(self.leaf(), format!("{pem}{ca_pem}")).map_err(|e| format!("No se pudo guardar el certificado: {e}"))
    }

    /// Argumentos del rest-server (siempre append-only y repos privados).
    pub fn args(&self, path: &str, listen: &str) -> Vec<String> {
        self.args_de(path, listen, None)
    }

    /// Igual, para una zona (7b): sus usuarios; el mismo certificado.
    pub fn args_de(&self, path: &str, listen: &str, zona: Option<&str>) -> Vec<String> {
        vec![
            "--path".into(),
            path.to_string(),
            "--listen".into(),
            listen.to_string(),
            "--append-only".into(),
            "--private-repos".into(),
            "--tls".into(),
            "--tls-cert".into(),
            self.leaf().display().to_string(),
            "--tls-key".into(),
            self.private("servidor-tls.key").display().to_string(),
            "--htpasswd-file".into(),
            self.htpasswd_de(zona).display().to_string(),
        ]
    }

    /// Reescribe `.htpasswd` con estas líneas (archivo nuevo en la carpeta privada).
    pub fn write_htpasswd(&self, lines: &[String]) -> Result<(), String> {
        self.write_htpasswd_de(None, lines)
    }

    /// Igual, el de una zona (7b) o el de la principal.
    pub fn write_htpasswd_de(&self, zona: Option<&str>, lines: &[String]) -> Result<(), String> {
        let destino = self.htpasswd_de(zona);
        let tmp = destino.with_extension("htpasswd.tmp");
        let _ = std::fs::remove_file(&tmp);
        std::fs::write(&tmp, lines.join("\n") + "\n").map_err(|e| format!("No se pudieron guardar los usuarios del servidor: {e}"))?;
        std::fs::rename(&tmp, destino).map_err(|e| e.to_string())
    }

    pub fn read_htpasswd_de(&self, zona: Option<&str>) -> Vec<String> {
        std::fs::read_to_string(self.htpasswd_de(zona)).map(|s| s.lines().filter(|l| !l.trim().is_empty()).map(str::to_string).collect()).unwrap_or_default()
    }
}

/// Autoridad propia del servidor: lo que fijan los equipos (`--cacert`).
pub fn cert_file() -> PathBuf {
    Files::agent().ca()
}

/// Certificado del servidor (con sus IP), firmado por la autoridad propia.
pub fn leaf_file() -> PathBuf {
    Files::agent().leaf()
}

pub fn load() -> ServerConfig {
    std::fs::read(crate::agent::agent_dir().join("servidor.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn save(c: &ServerConfig) -> Result<(), String> {
    crate::agent::write_json("servidor.json", c)
}

/// El rest-server que acompaña a Resguardo (junto al ejecutable).
pub fn binary() -> PathBuf {
    // Solo en pruebas (compilación de desarrollo con RESGUARDO_AGENT_DIR): otro
    // rest-server (el de src-tauri/binaries). Su huella se comprueba igual.
    if crate::agent::test_mode() {
        if let Some(p) = std::env::var_os("RESGUARDO_REST_SERVER_BIN").filter(|p| !p.is_empty()) {
            return PathBuf::from(p);
        }
    }
    std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(BINARIO))).unwrap_or_else(|| BINARIO.into())
}

/// Linux: huella del rest-server que fija el administrador (si la versión no
/// trae una): `/etc/resguardo-agente/rest-server.sha256`. Solo se acepta si el
/// archivo es de root y nadie más puede escribirlo (en Linux, root es el
/// límite de confianza del agente; ver packaging/linux/README.md).
#[cfg(unix)]
pub const HUELLA_LOCAL: &str = "/etc/resguardo-agente/rest-server.sha256";

/// La huella con la que se comprueba el rest-server antes de cada arranque.
pub fn huella_fijada() -> Option<String> {
    if let Some(h) = PINNED_SHA256 {
        return Some(h.to_string());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m = std::fs::symlink_metadata(HUELLA_LOCAL).ok()?;
        if !m.is_file() || m.uid() != 0 || m.mode() & 0o022 != 0 {
            crate::agent::log(&format!("AVISO: {HUELLA_LOCAL} no se usa: debe ser de root y solo él puede escribirlo."));
            return None;
        }
        let texto = std::fs::read_to_string(HUELLA_LOCAL).ok()?;
        // Admite la línea de SHA256SUMS («<huella>  rest-server»).
        let h = texto.split_whitespace().next()?.to_ascii_lowercase();
        if h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(h);
        }
    }
    None
}

/// ¿Está el binario y coincide con su hash fijado?
pub fn binary_check() -> Result<(), String> {
    binary_check_at(&binary())
}

/// Igual, para un rest-server en otra ruta (las pruebas).
pub fn binary_check_at(path: &Path) -> Result<(), String> {
    let pinned = huella_fijada().ok_or("Esta versión de Resguardo no incluye el servidor de copias (falta su huella fijada).")?;
    let pinned = pinned.as_str();
    let bytes = std::fs::read(path).map_err(|_| format!("Falta {BINARIO} junto a Resguardo: reinstala Resguardo."))?;
    let got: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    if got.eq_ignore_ascii_case(pinned.trim()) {
        Ok(())
    } else {
        Err(format!("{BINARIO} no es el que acompaña a esta versión (la huella no coincide): no se arranca. Reinstala Resguardo."))
    }
}

/// Usuario válido para rest-server y para una carpeta: minúsculas, cifras y guiones.
pub fn user_name(raw: &str) -> String {
    let s: String = raw
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            'ñ' => 'n',
            c if c.is_ascii_alphanumeric() => c,
            _ => '-',
        })
        .collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    s.chars().take(32).collect()
}

/// Crea un usuario del servidor (nombre normalizado y único) con contraseña
/// aleatoria y lo escribe en `.htpasswd`. Devuelve (usuario, contraseña).
pub fn create_user(c: &mut ServerConfig, raw: &str) -> Result<(String, String), String> {
    create_user_en(&Files::agent(), c, None, raw)
}

/// Igual, en una zona (7b) o en la principal. El nombre es único en todo el
/// almacén (si ya lo tiene otra zona, «ana-2»).
pub fn create_user_en(f: &Files, c: &mut ServerConfig, zona: Option<&str>, raw: &str) -> Result<(String, String), String> {
    let base = user_name(raw);
    if base.is_empty() {
        return Err("Escribe un nombre para el equipo (letras, cifras o guiones).".into());
    }
    if c.usuarios(zona).is_none() {
        return Err("Esa zona ya no está en este almacén.".into());
    }
    let mut user = base.clone();
    for i in 2.. {
        if !c.usuario_existe(&user) {
            break;
        }
        user = format!("{base}-{i}");
    }
    let password = new_password();
    let mut lines: Vec<String> = f.read_htpasswd_de(zona).into_iter().filter(|l| !l.starts_with(&format!("{user}:"))).collect();
    lines.push(htpasswd_line(&user, &password)?);
    f.write_htpasswd_de(zona, &lines)?;
    let para = (user != base).then_some(base);
    if let Some(l) = c.usuarios_mut(zona) {
        l.push(ServerUser { name: user.clone(), created_at: chrono::Local::now().to_rfc3339(), para });
    }
    Ok((user, password))
}

/// Contraseña aleatoria de 128 bits (32 caracteres hexadecimales).
pub fn new_password() -> String {
    let a = uuid::Uuid::new_v4().simple().to_string();
    let b = uuid::Uuid::new_v4().simple().to_string();
    // Cada UUID v4 aporta 122 bits aleatorios: con la mitad de cada uno sobran 128.
    format!("{}{}", &a[..16], &b[..16])
}

/// Línea de `.htpasswd` con bcrypt.
pub fn htpasswd_line(user: &str, password: &str) -> Result<String, String> {
    let hash = bcrypt::hash(password, 12).map_err(|e| e.to_string())?;
    Ok(format!("{user}:{hash}"))
}

/// Nombres del certificado del servidor: `localhost`, el nombre del equipo y
/// sus IP de la red local.
pub fn current_names() -> Vec<String> {
    let mut names = vec!["localhost".to_string(), crate::web::default_device_name()];
    names.extend(lan_addresses());
    names
}

/// Crea la autoridad propia (lo que fijan los equipos) y el certificado del
/// servidor firmado por ella. Devuelve la huella SHA-256 de la autoridad.
pub fn make_cert(names: &[String]) -> Result<String, String> {
    Files::agent().make_cert(names)
}

/// Vuelve a emitir el certificado del servidor (p. ej. si cambia su IP) con la
/// misma autoridad: los equipos que ya la fijaron siguen confiando en él.
pub fn issue_leaf(names: &[String]) -> Result<(), String> {
    Files::agent().issue_leaf(names)
}

/// Si este equipo tiene una IP de la red local que el certificado no cubre,
/// lo vuelve a emitir (con las de antes y las nuevas). Devuelve si cambió.
pub fn refresh_leaf_if_needed(c: &mut ServerConfig) -> Result<bool, String> {
    if !c.enabled {
        return Ok(false);
    }
    let names = current_names();
    if names.iter().all(|n| c.cert_names.contains(n)) {
        return Ok(false);
    }
    let mut all = c.cert_names.clone();
    for n in names {
        if !all.contains(&n) {
            all.push(n);
        }
    }
    issue_leaf(&all)?;
    c.cert_names = all;
    save(c)?;
    crate::agent::log("Servidor de copias: la IP del equipo cambió; certificado del servidor renovado (misma autoridad).");
    Ok(true)
}

const CA_NAME: &str = "Resguardo Servidor de copias";

// Autoridad propia y certificado del servidor: en el motor (también los usa Resguardo Server).
fn generate_ca() -> Result<(String, String, String), String> {
    resguardo_motor::tls::generar_ca(CA_NAME)
}

fn generate_leaf(ca_key_pem: &str, names: &[String]) -> Result<(String, String), String> {
    resguardo_motor::tls::emitir_certificado(ca_key_pem, CA_NAME, names)
}

/// Argumentos del rest-server (siempre append-only y repos privados).
pub fn args(c: &ServerConfig) -> Vec<String> {
    Files::agent().args(&c.path, &escuchar(c.port))
}

/// Dónde escucha un rest-server del almacén. En pruebas, solo en este equipo
/// (sin que el cortafuegos pregunte nada).
fn escuchar(port: u16) -> String {
    if crate::agent::test_mode() {
        format!("127.0.0.1:{port}")
    } else {
        format!(":{port}")
    }
}

/// Argumentos del rest-server de una zona (7b): su carpeta, su puerto y sus
/// usuarios; siempre append-only y repos privados, con el mismo certificado.
pub fn args_zona(z: &Zona) -> Vec<String> {
    Files::agent().args_de(&z.path, &escuchar(z.port), Some(&z.id))
}

/// Repositorios de cada usuario (`/<usuario>/<repo>/` con `config`): solo nombres.
pub fn repos_by_user(c: &ServerConfig) -> Vec<(String, Vec<String>)> {
    repos_de_usuarios(&c.path, &c.users)
}

/// Igual, en una carpeta cualquiera del almacén (la principal o una zona).
pub fn repos_de_usuarios(carpeta: &str, usuarios: &[ServerUser]) -> Vec<(String, Vec<String>)> {
    usuarios
        .iter()
        .map(|u| {
            let base = Path::new(carpeta).join(&u.name);
            let mut repos: Vec<String> = crate::discover::scan_local(&base)
                .iter()
                .filter_map(|p| p.strip_prefix(&base).ok().map(|r| r.display().to_string().replace('\\', "/")))
                .map(|r| if r.is_empty() { ".".into() } else { r })
                .collect();
            repos.sort();
            (u.name.clone(), repos)
        })
        .collect()
}

/// Direcciones IPv4 de la red local de este equipo.
pub fn lan_addresses() -> Vec<String> {
    let mut out = Vec::new();
    // Truco sin red real: la IP de salida hacia una dirección privada.
    for probe in ["10.255.255.255:1", "192.168.255.255:1", "172.31.255.255:1"] {
        if let Ok(sock) = std::net::UdpSocket::bind("0.0.0.0:0") {
            if sock.connect(probe).is_ok() {
                if let Ok(addr) = sock.local_addr() {
                    let ip = addr.ip().to_string();
                    if !ip.starts_with("0.") && !out.contains(&ip) {
                        out.push(ip);
                    }
                }
            }
        }
    }
    out
}

/// ¿Responde el servidor en su puerto (en este equipo)?
pub fn listening(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(&std::net::SocketAddr::from(([127, 0, 0, 1], port)), std::time::Duration::from_millis(400)).is_ok()
}

/// Un rest-server que debe estar en marcha: el de la principal (`None`) o el
/// de una zona, con sus argumentos y dónde guarda su PID.
struct Querido {
    zona: Option<String>,
    args: Vec<String>,
}

/// Los rest-server que tiene que haber ahora (la principal y una por zona).
fn queridos(c: &ServerConfig) -> Vec<Querido> {
    std::iter::once(Querido { zona: None, args: args(c) }).chain(c.zonas.iter().map(|z| Querido { zona: Some(z.id.clone()), args: args_zona(z) })).collect()
}

/// Uno en marcha.
struct Vivo {
    hijo: std::process::Child,
    args: Vec<String>,
    desde: std::time::Instant,
}

/// Cuándo se puede volver a arrancar uno que se cayó, y cuánto se esperó la última vez.
struct Espera {
    hasta: std::time::Instant,
    segundos: u64,
}

fn nombre_instancia(zona: &Option<String>) -> String {
    zona.as_ref().map_or_else(|| "rest-server".to_string(), |z| format!("rest-server de la zona {z}"))
}

/// Para uno de los que lanzó este proceso y borra su PID.
fn parar_vivo(zona: &Option<String>, mut v: Vivo) {
    let _ = v.hijo.kill();
    let _ = v.hijo.wait();
    let _ = std::fs::remove_file(Files::agent().pid_de(zona.as_deref()));
}

fn parar_vivos(vivos: &mut std::collections::HashMap<Option<String>, Vivo>) {
    for (z, v) in vivos.drain() {
        parar_vivo(&z, v);
    }
}

/// `resguardo.exe --server-run`: comprueba el binario y mantiene en marcha el
/// rest-server de la principal y el de cada zona (7b). Cada pocos segundos
/// vuelve a leer `servidor.json`: arranca las zonas nuevas, para las quitadas
/// (o cambiadas) y relanza la que se caiga, esperando cada vez más si se cae
/// enseguida. Antes de cada arranque comprueba la huella del binario: si no
/// cuadra, para todo y termina.
pub fn run_forever() -> i32 {
    use std::time::{Duration, Instant};
    let mut vivos: std::collections::HashMap<Option<String>, Vivo> = std::collections::HashMap::new();
    let mut esperas: std::collections::HashMap<Option<String>, Espera> = std::collections::HashMap::new();
    // El certificado se mira al empezar y después cada 5 minutos.
    let mut cert_mirado: Option<Instant> = None;
    #[cfg(unix)]
    let mut puertos_fw: Option<(Vec<u16>, bool)> = None;
    let pausa = if crate::agent::test_mode() { Duration::from_millis(500) } else { Duration::from_secs(5) };
    loop {
        let mut c = load();
        if !c.enabled {
            parar_vivos(&mut vivos);
            return 0;
        }
        if cert_mirado.is_none_or(|t| t.elapsed() > Duration::from_secs(300)) {
            let primera = cert_mirado.is_none();
            cert_mirado = Some(Instant::now());
            // Al arrancar (p. ej. tras reiniciar con otra IP) y mientras funciona: si
            // cambia la IP del equipo, se renueva el certificado y se reinician todos con él.
            match refresh_leaf_if_needed(&mut c) {
                Ok(true) if !primera => parar_vivos(&mut vivos),
                Ok(_) => {}
                Err(e) => crate::agent::log(&format!("ERROR: Servidor de copias: no se pudo renovar el certificado: {e}")),
            }
        }
        // Linux: las reglas de nftables no sobreviven a un reinicio; se ponen al
        // arrancar y cada vez que cambian los puertos (una zona nueva o quitada).
        #[cfg(unix)]
        {
            let ahora = (c.puertos(), c.local_subnet_only);
            if puertos_fw.as_ref() != Some(&ahora) {
                if let Err(e) = firewall(&ahora.0, ahora.1) {
                    crate::agent::log(&format!("ERROR: Servidor de copias: {e}"));
                }
                puertos_fw = Some(ahora);
            }
        }
        let quiero = queridos(&c);
        // Los que sobran (una zona quitada) o cambiaron (otro puerto o carpeta).
        let sobran: Vec<Option<String>> =
            vivos.iter().filter(|(z, v)| !quiero.iter().any(|q| &q.zona == *z && q.args == v.args)).map(|(z, _)| z.clone()).collect();
        for z in sobran {
            if let Some(v) = vivos.remove(&z) {
                crate::agent::log(&format!("Servidor de copias: se para el {} (ya no está o cambió).", nombre_instancia(&z)));
                parar_vivo(&z, v);
            }
        }
        // Los que terminaron solos.
        let caidos: Vec<Option<String>> = vivos.iter_mut().filter_map(|(z, v)| matches!(v.hijo.try_wait(), Ok(Some(_)) | Err(_)).then(|| z.clone())).collect();
        for z in caidos {
            if let Some(mut v) = vivos.remove(&z) {
                let estado = v.hijo.try_wait().ok().flatten().map(|s| s.to_string()).unwrap_or_default();
                crate::agent::log(&format!("Servidor de copias: {} terminó ({estado}).", nombre_instancia(&z)));
                // Si duró poco, se espera cada vez más (sin pasar de 5 minutos).
                let antes = esperas.get(&z).map_or(5, |e| e.segundos);
                let segundos = if v.desde.elapsed().as_secs() > 300 { 5 } else { (antes * 2).min(300) };
                esperas.insert(z, Espera { hasta: Instant::now() + Duration::from_secs(segundos), segundos });
            }
        }
        // Los que faltan.
        for q in quiero {
            if vivos.contains_key(&q.zona) || esperas.get(&q.zona).is_some_and(|e| Instant::now() < e.hasta) {
                continue;
            }
            if let Err(e) = binary_check() {
                crate::agent::log(&format!("ERROR: Servidor de copias: {e}"));
                parar_vivos(&mut vivos);
                return 1;
            }
            let mut orden = std::process::Command::new(binary());
            resguardo_motor::proceso::entorno_minimo(&mut orden);
            match orden.args(&q.args).stdin(std::process::Stdio::null()).spawn() {
                Ok(hijo) => {
                    // Para poder pararlo al desactivar el servidor o quitar la zona.
                    let _ = std::fs::write(Files::agent().pid_de(q.zona.as_deref()), hijo.id().to_string());
                    vivos.insert(q.zona.clone(), Vivo { hijo, args: q.args, desde: Instant::now() });
                }
                Err(e) => {
                    crate::agent::log(&format!("ERROR: Servidor de copias: no se pudo arrancar el {}: {e}", nombre_instancia(&q.zona)));
                    let segundos = (esperas.get(&q.zona).map_or(5, |e| e.segundos) * 2).min(300);
                    esperas.insert(q.zona, Espera { hasta: Instant::now() + Duration::from_secs(segundos), segundos });
                }
            }
        }
        // Lo de zonas que ya no están no se guarda para siempre.
        esperas.retain(|z, _| z.is_none() || c.zonas.iter().any(|x| Some(&x.id) == z.as_ref()));
        std::thread::sleep(pausa);
    }
}

/// En pruebas (compilación de desarrollo con RESGUARDO_AGENT_DIR), el
/// Servidor de copias corre dentro del proceso del agente: un solo hilo que
/// lo vigila (activarlo dos veces no lanza otro). Devuelve si lo lanzó.
pub fn arrancar_en_pruebas() -> bool {
    let mut h = HILO_PRUEBAS.lock().unwrap_or_else(|p| p.into_inner());
    if h.as_ref().is_some_and(|x| !x.is_finished()) {
        return false;
    }
    *h = Some(std::thread::spawn(run_forever));
    true
}

static HILO_PRUEBAS: std::sync::Mutex<Option<std::thread::JoinHandle<i32>>> = std::sync::Mutex::new(None);

/// En pruebas, tras desactivarlo: espera a que el hilo que lo vigila termine
/// (si no, seguiría leyendo `servidor.json` de la carpeta del agente de la
/// prueba siguiente).
fn esperar_fin_en_pruebas() {
    let h = HILO_PRUEBAS.lock().unwrap_or_else(|p| p.into_inner()).take();
    if let Some(h) = h {
        let _ = h.join();
    }
}

/// Regla del firewall solo para el puerto del servidor.
#[cfg(windows)]
pub fn firewall(ports: &[u16], local_only: bool) -> Result<(), String> {
    let _ = crate::platform::tool("netsh.exe", &["advfirewall", "firewall", "delete", "rule", &format!("name={}", regla_firewall())]);
    // Una sola regla con todos los puertos del almacén (la principal y sus zonas, 7b).
    let puertos = ports.iter().map(u16::to_string).collect::<Vec<_>>().join(",");
    let (ok, out) = crate::platform::tool(
        "netsh.exe",
        &[
            "advfirewall",
            "firewall",
            "add",
            "rule",
            &format!("name={}", regla_firewall()),
            "dir=in",
            "action=allow",
            "protocol=TCP",
            &format!("localport={puertos}"),
            if local_only { REMOTEIP_INTERNAS } else { "remoteip=any" },
        ],
    )?;
    if ok {
        Ok(())
    } else {
        Err(format!("No se pudo crear la regla del firewall: {}", out.trim()))
    }
}

/// Linux: una tabla propia de nftables. Con «solo la red local», el puerto
/// solo acepta las redes de este equipo (y fe80::/10) y descarta lo demás;
/// sin ella, no se pone ninguna regla. Si el equipo tiene otro cortafuegos
/// que lo bloquea todo (ufw, firewalld…), hay que abrir el puerto también ahí:
/// en nftables, aceptar en una tabla no anula lo que descarta otra.
/// «Solo la red local» = la subred del equipo y las redes privadas (otras
/// subredes o VLAN de la misma empresa), nunca internet.
#[cfg(windows)]
const REMOTEIP_INTERNAS: &str = "remoteip=localsubnet,10.0.0.0/8,172.16.0.0/12,192.168.0.0/16";

#[cfg(unix)]
pub fn firewall(ports: &[u16], local_only: bool) -> Result<(), String> {
    let nft = nft().ok_or("Falta nftables (el programa nft): instálalo con «apt install nftables».")?;
    if !local_only {
        firewall_remove();
        return Ok(());
    }
    let reglas = reglas_nft(ports, &redes_internas(redes_locales()));
    let archivo = crate::agent::private_dir().join("servidor.nft");
    std::fs::write(&archivo, reglas).map_err(|e| format!("No se pudieron preparar las reglas del cortafuegos: {e}"))?;
    let (ok, out) = crate::platform::tool(nft, &["-f", &archivo.to_string_lossy()])?;
    if ok {
        Ok(())
    } else {
        Err(format!("No se pudieron aplicar las reglas de nftables: {}", out.trim()))
    }
}

#[cfg(unix)]
fn nft() -> Option<&'static str> {
    ["/usr/sbin/nft", "/sbin/nft"].into_iter().find(|p| Path::new(p).is_file())
}

/// Reglas de nftables para los puertos del servidor (la principal y sus
/// zonas; se aplican de una vez: la tabla se crea, se vacía y se vuelve a llenar).
pub fn reglas_nft(ports: &[u16], redes: &[String]) -> String {
    let redes = if redes.is_empty() { REDES_PRIVADAS.iter().map(|r| r.to_string()).collect::<Vec<_>>() } else { redes.to_vec() };
    // «8000» o, con zonas, «{ 8000, 8002 }».
    let port = match ports {
        [p] => p.to_string(),
        _ => format!("{{ {} }}", ports.iter().map(u16::to_string).collect::<Vec<_>>().join(", ")),
    };
    format!(
        "# Resguardo: Servidor de copias (lo genera el agente; no lo edites).
table inet resguardo
delete table inet resguardo
table inet resguardo {{
  chain entrada {{
    type filter hook input priority filter - 10; policy accept;
    tcp dport {port} iif \"lo\" accept
    tcp dport {port} ip saddr {{ {} }} accept
    tcp dport {port} ip6 saddr fe80::/10 accept
    tcp dport {port} drop
  }}
}}
",
        redes.join(", ")
    )
}

/// Las redes privadas más las del equipo que no caen dentro de ellas (nftables
/// no admite intervalos solapados en un mismo conjunto): así entran también
/// otras subredes o VLAN de la empresa, nunca internet.
pub fn redes_internas(locales: Vec<String>) -> Vec<String> {
    let privada = |red: &str| {
        let ip = red.split('/').next().and_then(|i| i.parse::<std::net::Ipv4Addr>().ok());
        ip.is_some_and(|ip| ip.is_private())
    };
    let mut redes: Vec<String> = REDES_PRIVADAS.iter().map(|r| r.to_string()).collect();
    redes.extend(locales.into_iter().filter(|r| !privada(r)));
    redes
}

/// Si no se pueden leer las redes del equipo: las privadas de siempre.
const REDES_PRIVADAS: [&str; 3] = ["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"];

/// Redes IPv4 de este equipo (las de `ip -o -4 addr show scope global`).
#[cfg(unix)]
fn redes_locales() -> Vec<String> {
    let Some(ip) = ["/usr/sbin/ip", "/sbin/ip", "/usr/bin/ip", "/bin/ip"].into_iter().find(|p| Path::new(p).is_file()) else {
        return Vec::new();
    };
    match crate::platform::tool(ip, &["-o", "-4", "addr", "show", "scope", "global"]) {
        Ok((true, out)) => redes_de_ip_addr(&out),
        _ => Vec::new(),
    }
}

/// «2: eth0    inet 192.168.1.20/24 brd …» → «192.168.1.0/24».
pub fn redes_de_ip_addr(texto: &str) -> Vec<String> {
    let mut redes = Vec::new();
    for linea in texto.lines() {
        let mut palabras = linea.split_whitespace();
        while let Some(p) = palabras.next() {
            if p != "inet" {
                continue;
            }
            let Some((ip, bits)) = palabras.next().and_then(|c| c.split_once('/')) else { break };
            let (Ok(ip), Ok(bits)) = (ip.parse::<std::net::Ipv4Addr>(), bits.parse::<u32>()) else { break };
            if bits == 0 || bits > 32 {
                break;
            }
            let mascara = u32::MAX << (32 - bits);
            let red = format!("{}/{bits}", std::net::Ipv4Addr::from(u32::from(ip) & mascara));
            if !redes.contains(&red) {
                redes.push(red);
            }
            break;
        }
    }
    redes
}

/// ¿La línea CSV de `tasklist /FO CSV /NH` es de ese ejecutable? («"rest-server.exe","1234",…»)
fn tasklist_es(salida: &str, imagen: &str) -> bool {
    salida.lines().any(|l| l.trim().strip_prefix('"').and_then(|r| r.split_once('"')).is_some_and(|(nombre, _)| nombre.eq_ignore_ascii_case(imagen)))
}

/// ¿El proceso con ese PID es nuestro rest-server? Un `servidor.pid` viejo
/// (tras reiniciar el equipo, el PID puede ser de otro programa) no debe
/// parar nada más.
fn pid_es_rest_server(pid: u32) -> bool {
    let imagen = binary().file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| BINARIO.into());
    #[cfg(windows)]
    {
        let filtro = format!("PID eq {pid}");
        crate::platform::tool("tasklist.exe", &["/FI", &filtro, "/FO", "CSV", "/NH"]).is_ok_and(|(_, salida)| tasklist_es(&salida, &imagen))
    }
    #[cfg(unix)]
    {
        std::fs::read_link(format!("/proc/{pid}/exe")).is_ok_and(|exe| exe.file_name().is_some_and(|n| n.to_string_lossy() == imagen))
    }
}

/// Para un rest-server por el PID que guardó la tarea (nunca por nombre), si
/// ese PID sigue siendo el rest-server, y borra el archivo del PID.
fn parar_por_pid(archivo: &Path) {
    if let Some(pid) = std::fs::read_to_string(archivo).ok().and_then(|s| s.trim().parse::<u32>().ok()).filter(|p| pid_es_rest_server(*p)) {
        #[cfg(windows)]
        {
            let _ = crate::platform::tool("taskkill.exe", &["/PID", &pid.to_string(), "/T", "/F"]);
        }
        #[cfg(unix)]
        {
            // SAFETY: solo envía una señal; el PID es el que guardó run_forever.
            if let Ok(pid) = libc::pid_t::try_from(pid) {
                if pid > 1 {
                    unsafe { libc::kill(pid, libc::SIGTERM) };
                }
            }
        }
    }
    let _ = std::fs::remove_file(archivo);
}

/// Para los rest-server que lanzó la tarea: el de la principal y el de cada
/// zona (los PID que haya en la carpeta privada, también de zonas ya quitadas).
pub fn stop() {
    let f = Files::agent();
    parar_por_pid(&f.pid_de(None));
    if let Ok(l) = std::fs::read_dir(&f.private) {
        for e in l.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.strip_prefix("servidor-zona-").and_then(|x| x.strip_suffix(".pid")).is_some_and(zona_id_valido) {
                parar_por_pid(&e.path());
            }
        }
    }
}

/// Reescribe `.htpasswd` con estas líneas (archivo nuevo en la carpeta privada).
pub fn write_htpasswd(lines: &[String]) -> Result<(), String> {
    Files::agent().write_htpasswd(lines)
}

pub fn read_htpasswd() -> Vec<String> {
    Files::agent().read_htpasswd_de(None)
}

pub fn firewall_remove() {
    #[cfg(windows)]
    {
        let _ = crate::platform::tool("netsh.exe", &["advfirewall", "firewall", "delete", "rule", &format!("name={}", regla_firewall())]);
    }
    #[cfg(unix)]
    {
        if let Some(nft) = nft() {
            let _ = crate::platform::tool(nft, &["delete", "table", "inet", "resguardo"]);
        }
    }
}

/// ¿Está libre ese puerto ahora en este equipo? (lo intenta unos segundos).
fn puerto_libre_ahora(port: u16) -> bool {
    (0..20).any(|_| {
        let ok = std::net::TcpListener::bind((if crate::agent::test_mode() { "127.0.0.1" } else { "0.0.0.0" }, port)).is_ok();
        if !ok {
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        ok
    })
}

/// Activa (o cambia) el Servidor de copias: carpeta, puerto y si solo se
/// acepta la red local. Genera el certificado, la regla del firewall y la
/// tarea (o el servicio) que lo arranca. Requiere administrador.
pub fn activar(path: &str, port: u16, local_subnet_only: bool) -> Result<ServerConfig, String> {
    crate::agent::require_admin()?;
    if port < 1024 {
        return Err("Elige un puerto entre 1024 y 65535 (por ejemplo, 8000).".into());
    }
    let path = path.trim().to_string();
    // Una carpeta de un disco del equipo (no la raíz ni de red, sin enlaces),
    // solo para SYSTEM y Administradores: si los usuarios pudieran escribir
    // en ella, podrían borrar las copias (y el «solo añadir» no serviría).
    crate::platform::carpeta_local_valida(&path)?;
    let mut c = load();
    // 7b: ni dentro de una zona ni con su puerto.
    if c.zonas.iter().any(|z| se_solapan(&path, &z.path)) {
        return Err("Esa carpeta se solapa con una zona de este almacén: elige otra.".into());
    }
    if c.zonas.iter().any(|z| z.port == port) {
        return Err(format!("El puerto {port} ya es el de una zona de este almacén: elige otro."));
    }
    binary_check()?;
    crate::platform::carpeta_privada(Path::new(&path))?;
    crate::agent::prepare_dir()?;
    let names = current_names();
    if c.tls_sha256.is_none() || !cert_file().is_file() || !leaf_file().is_file() {
        c.tls_sha256 = Some(make_cert(&names)?);
        c.cert_names = names;
    }
    if read_htpasswd().is_empty() {
        write_htpasswd(&[])?;
    }
    // El puerto tiene que estar libre (otro programa, o Resguardo Server en el
    // mismo equipo), salvo que ya sea el suyo y esté activado. Se mira antes de
    // guardar nada: si no se puede, el almacén sigue como estaba.
    if !(c.enabled && c.port == port) && !puerto_libre_ahora(port) {
        return Err(format!("El puerto {port} ya está en uso en este equipo: elige otro."));
    }
    c.enabled = true;
    c.path = path;
    c.port = port;
    c.local_subnet_only = local_subnet_only;
    save(&c)?;
    stop();
    if crate::agent::test_mode() {
        // Pruebas (compilación de desarrollo con RESGUARDO_AGENT_DIR): sin
        // cortafuegos ni tarea de SYSTEM; el servidor corre en este proceso
        // (un solo hilo que lo vigila: si ya estaba, relanza solo lo que cambió).
        arrancar_en_pruebas();
    } else {
        firewall(&c.puertos(), local_subnet_only)?;
        crate::platform::install_server_task(&crate::agent::private_dir())?;
    }
    crate::agent::log(&format!("Servidor de copias activado en el puerto {port}."));
    Ok(c)
}

/// Desactiva el Servidor de copias. Las copias guardadas se quedan en la carpeta.
pub fn desactivar() -> Result<(), String> {
    crate::agent::require_admin()?;
    let mut c = load();
    c.enabled = false;
    save(&c)?;
    stop();
    if crate::agent::test_mode() {
        esperar_fin_en_pruebas();
    } else {
        crate::platform::uninstall_server_task();
        firewall_remove();
    }
    crate::agent::log("Servidor de copias desactivado.");
    Ok(())
}

/// Añade un equipo cliente: su usuario, su contraseña (128 bits) y la
/// ubicación del repositorio que le toca (`rest:https://<ip>:<puerto>/<usuario>/`).
pub fn anadir_equipo(name: &str) -> Result<(String, String, String), String> {
    anadir_equipo_en(name, None).map(|(u, p, l, _)| (u, p, l))
}

/// Igual, en una zona (7b) o en la principal. Devuelve también el puerto.
pub fn anadir_equipo_en(name: &str, zona: Option<&str>) -> Result<(String, String, String, u16), String> {
    crate::agent::require_admin()?;
    let mut c = load();
    if !c.enabled {
        return Err("Activa antes el Servidor de copias.".into());
    }
    let base = user_name(name);
    let (lista, port) = match zona {
        None => (&c.users, c.port),
        Some(id) => c.zonas.iter().find(|z| z.id == id).map(|z| (&z.users, z.port)).ok_or("Esa zona ya no está en este almacén.")?,
    };
    if lista.iter().any(|u| u.es_para(&base)) {
        return Err(match zona {
            None => format!("Ya hay un equipo «{base}» en este servidor."),
            Some(_) => format!("Ya hay un equipo «{base}» en esa zona."),
        });
    }
    let (user, password) = create_user_en(&Files::agent(), &mut c, zona, name)?;
    save(&c)?;
    // En pruebas el rest-server solo escucha en 127.0.0.1 (ver `args`): `localhost`, que el certificado cubre.
    let ip = if crate::agent::test_mode() { None } else { lan_addresses().into_iter().next() }.unwrap_or_else(|| "localhost".into());
    let location = format!("rest:https://{ip}:{port}/{user}/");
    crate::agent::log(&format!("Servidor de copias: nuevo equipo cliente «{user}»{}.", zona.map(|z| format!(" en la zona {z}")).unwrap_or_default()));
    Ok((user, password, location, port))
}

/// Cuánto se espera, como mucho, a que el rest-server acepte un usuario nuevo.
const ESPERA_USUARIO_NUEVO: std::time::Duration = std::time::Duration::from_secs(45);

/// rest-server vuelve a leer `.htpasswd` como mucho cada 30 s (al llegar una
/// petición, si el archivo cambió). Hasta entonces, a un usuario recién añadido
/// le contesta 401: el equipo cliente, que crea su repositorio en cuanto la
/// consola le da el acceso («Copiar en …»), fallaría con «el servidor rechazó el
/// usuario o la contraseña». Después de `anadir_equipo` se espera aquí a que lo
/// acepte (en Linux se le pide además que lo relea ya, con SIGHUP). Si el
/// servidor no está en marcha, no se espera (lo leerá al arrancar). Nunca
/// falla: el usuario ya está creado y su contraseña tiene que llegar a la consola.
pub fn esperar_usuario(user: &str, password: &str) {
    esperar_usuario_de(None, user, password)
}

/// Igual, en el rest-server de una zona (7b) o en el de la principal.
pub fn esperar_usuario_de(zona: Option<&str>, user: &str, password: &str) {
    if let Err(e) = esperar_usuario_o_error(zona, user, password) {
        crate::agent::log(&format!("Servidor de copias: {e}"));
    }
}

fn esperar_usuario_o_error(zona: Option<&str>, user: &str, password: &str) -> Result<(), String> {
    let c = load();
    let port = match zona {
        None => c.port,
        Some(id) => c.zonas.iter().find(|z| z.id == id).map_or(0, |z| z.port),
    };
    if !c.enabled || port == 0 || !listening(port) {
        return Ok(());
    }
    #[cfg(unix)]
    if let Some(pid) = std::fs::read_to_string(Files::agent().pid_de(zona)).ok().and_then(|s| s.trim().parse::<libc::pid_t>().ok()).filter(|p| *p > 1) {
        if pid_es_rest_server(pid as u32) {
            // SAFETY: solo envía una señal al rest-server que lanzó run_forever (rest-server relee .htpasswd con SIGHUP).
            unsafe { libc::kill(pid, libc::SIGHUP) };
        }
    }
    esperar_usuario_en(port, &cert_file(), user, password)
}

/// Espera a que el rest-server de `port` (con la autoridad propia `ca`) acepte a
/// `user`: hasta que una petición suya deje de recibir 401.
pub fn esperar_usuario_en(port: u16, ca: &Path, user: &str, password: &str) -> Result<(), String> {
    let tls =
        std::fs::read(ca).ok().and_then(|pem| crate::protection::tls_con_autoridad(&pem)).ok_or("No se pudo leer la autoridad del Servidor de copias.")?;
    let agente =
        ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(10))).http_status_as_error(false).tls_config(tls).build().new_agent();
    let url = format!("https://localhost:{port}/{user}/config");
    let basic = format!("Basic {}", crate::protection::base64(format!("{user}:{password}").as_bytes()));
    let inicio = std::time::Instant::now();
    loop {
        match agente.head(&url).header("Authorization", &basic).call() {
            Ok(r) if r.status().as_u16() != 401 => return Ok(()),
            Ok(_) => {}
            // Sin respuesta (p. ej. se está reiniciando): no se bloquea la orden por eso.
            Err(_) => return Ok(()),
        }
        if inicio.elapsed() > ESPERA_USUARIO_NUEVO {
            return Err(format!("el rest-server aún no acepta al usuario nuevo «{user}» tras {} s.", ESPERA_USUARIO_NUEVO.as_secs()));
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

/// La dirección de un usuario de este almacén para este mismo equipo (un
/// repositorio en su propio almacén): por `localhost`, que el certificado cubre.
pub fn ubicacion_local(port: u16, user: &str) -> String {
    format!("rest:https://localhost:{port}/{user}/")
}

/// Quita un equipo cliente: ya no puede entrar. Sus copias se quedan en la carpeta.
pub fn quitar_equipo(name: &str) -> Result<(), String> {
    quitar_equipo_en(name, None)
}

/// Igual, de una zona (7b) o de la principal.
pub fn quitar_equipo_en(name: &str, zona: Option<&str>) -> Result<(), String> {
    crate::agent::require_admin()?;
    let mut c = load();
    let l = c.usuarios_mut(zona).ok_or("Esa zona ya no está en este almacén.")?;
    l.retain(|u| u.name != name);
    save(&c)?;
    let f = Files::agent();
    let lines: Vec<String> = f.read_htpasswd_de(zona).into_iter().filter(|l| !l.starts_with(&format!("{name}:"))).collect();
    f.write_htpasswd_de(zona, &lines)
}

// ---------- Zonas (tarea 7b, docs/copias-en-cadena.md) ----------

/// El nombre que se propone para una zona: «Disco E» (Windows) o el nombre de su carpeta.
pub fn nombre_por_defecto(carpeta: &str) -> String {
    let t = carpeta.trim();
    let b = t.as_bytes();
    if b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic() {
        return format!("Disco {}", (b[0] as char).to_ascii_uppercase());
    }
    Path::new(t).file_name().map(|n| n.to_string_lossy().into_owned()).filter(|n| !n.is_empty()).unwrap_or_else(|| "Otra zona".into())
}

/// Nombre de una zona: 1 a 60 caracteres, sin caracteres de control.
fn nombre_zona_valido(n: &str) -> bool {
    let n = n.trim();
    !n.is_empty() && n.chars().count() <= 60 && !n.chars().any(char::is_control)
}

/// Comprueba una zona nueva contra el almacén (sin tocar nada): carpeta sin
/// solaparse con la principal, otra zona o el espejo; puerto distinto de los
/// del almacén. No mira el disco ni si el puerto está libre.
pub fn validar_zona_nueva(c: &ServerConfig, carpeta: &str, port: u16) -> Result<(), String> {
    if !c.enabled {
        return Err("Activa antes el Servidor de copias.".into());
    }
    if c.zonas.len() >= ZONAS_MAX {
        return Err(format!("Como mucho {ZONAS_MAX} zonas además de la principal."));
    }
    if port < 1024 {
        return Err("Elige un puerto entre 1024 y 65535 (por ejemplo, 8002).".into());
    }
    if c.puertos().contains(&port) {
        // De dos en dos, como los que propone el agente (8000, 8002, 8004…).
        let otro = (port..=u16::MAX).step_by(2).find(|p| !c.puertos().contains(p)).unwrap_or(8002);
        return Err(format!("El puerto {port} ya es de este almacén: elige otro (por ejemplo, {otro})."));
    }
    let carpeta = carpeta.trim();
    if carpeta.is_empty() {
        return Err("Falta la carpeta de la zona.".into());
    }
    if se_solapan(carpeta, &c.path) {
        return Err("La carpeta de la zona no puede estar dentro de la del almacén (ni al revés): elige otro disco u otra carpeta.".into());
    }
    if c.zonas.iter().any(|z| se_solapan(carpeta, &z.path)) {
        return Err("La carpeta de la zona se solapa con otra zona de este almacén.".into());
    }
    if c.espejo.as_ref().is_some_and(|e| e.destinos().iter().any(|d| d.tipo == "carpeta" && se_solapan(carpeta, &d.carpeta))) {
        return Err("La carpeta de la zona se solapa con una carpeta del espejo: elige otra.".into());
    }
    if carpeta_del_sistema(Path::new(carpeta)) {
        return Err("La carpeta de la zona no puede estar en la carpeta de Windows, de los programas o de Resguardo.".into());
    }
    Ok(())
}

/// Un id nuevo para una zona (`z` y 6 cifras hexadecimales), que no esté ya.
fn id_zona_nuevo(c: &ServerConfig) -> String {
    loop {
        let id = format!("z{}", &uuid::Uuid::new_v4().simple().to_string()[..6]);
        if !c.zonas.iter().any(|z| z.id == id) {
            return id;
        }
    }
}

/// Crea una zona: otra carpeta (otro disco) servida por su propio rest-server
/// en `port`, con el mismo certificado y sin usuarios todavía. La pone en
/// marcha la tarea del Servidor de copias (la vigila igual que la principal).
pub fn crear_zona(nombre: Option<&str>, carpeta: &str, port: u16) -> Result<Zona, String> {
    crate::agent::require_admin()?;
    let mut c = load();
    let carpeta = carpeta.trim().to_string();
    validar_zona_nueva(&c, &carpeta, port)?;
    let nombre = nombre.map(str::trim).filter(|n| !n.is_empty()).map_or_else(|| nombre_por_defecto(&carpeta), str::to_string);
    if !nombre_zona_valido(&nombre) {
        return Err("Escribe un nombre para la zona (hasta 60 caracteres).".into());
    }
    // Como la principal: un disco del equipo, no la raíz ni de red, sin
    // enlaces, solo para SYSTEM y Administradores.
    crate::platform::carpeta_local_valida(&carpeta)?;
    binary_check()?;
    if !puerto_libre_ahora(port) {
        return Err(format!("El puerto {port} ya está en uso en este equipo: elige otro."));
    }
    crate::platform::carpeta_privada(Path::new(&carpeta))?;
    let z = Zona { id: id_zona_nuevo(&c), nombre, path: carpeta, port, users: Vec::new(), creada: chrono::Local::now().to_rfc3339() };
    Files::agent().write_htpasswd_de(Some(&z.id), &[])?;
    c.zonas.push(z.clone());
    save(&c)?;
    if !crate::agent::test_mode() {
        firewall(&c.puertos(), c.local_subnet_only)?;
    }
    crate::agent::log(&format!("Servidor de copias: zona nueva «{}» en el puerto {port}.", z.nombre));
    Ok(z)
}

/// Cambia el nombre de una zona (solo lo que se ve).
pub fn renombrar_zona(id: &str, nombre: &str) -> Result<(), String> {
    crate::agent::require_admin()?;
    if !nombre_zona_valido(nombre) {
        return Err("Escribe un nombre para la zona (hasta 60 caracteres).".into());
    }
    let mut c = load();
    let z = c.zonas.iter_mut().find(|z| z.id == id).ok_or("Esa zona ya no está en este almacén.")?;
    z.nombre = nombre.trim().to_string();
    save(&c)
}

/// Quita una zona: su rest-server se para y sus usuarios ya no entran. Lo
/// guardado se queda en su carpeta (reduce la protección: la orden espera).
pub fn quitar_zona(id: &str) -> Result<Zona, String> {
    crate::agent::require_admin()?;
    let mut c = load();
    let pos = c.zonas.iter().position(|z| z.id == id).ok_or("Esa zona ya no está en este almacén.")?;
    // Tarea 7d.2: un espejo que copia desde ella o hacia ella dejaría de hacerse sin decirlo.
    if c.espejo.as_ref().is_some_and(|e| e.destinos().iter().any(|d| d.zona.as_deref() == Some(id) || (d.tipo == "zona" && d.carpeta == id))) {
        return Err("El espejo copia desde esa zona o hacia ella: quita antes esos destinos del espejo.".into());
    }
    let z = c.zonas.remove(pos);
    save(&c)?;
    let f = Files::agent();
    parar_por_pid(&f.pid_de(Some(id)));
    let _ = std::fs::remove_file(f.htpasswd_de(Some(id)));
    if !crate::agent::test_mode() && c.enabled {
        firewall(&c.puertos(), c.local_subnet_only)?;
    }
    crate::agent::log(&format!("Servidor de copias: zona «{}» quitada (lo guardado se queda en su carpeta).", z.nombre));
    Ok(z)
}

/// Espera (unos segundos) a que el rest-server de un puerto responda.
pub fn esperar_escucha(port: u16, plazo: std::time::Duration) -> bool {
    let inicio = std::time::Instant::now();
    loop {
        if listening(port) {
            return true;
        }
        if inicio.elapsed() > plazo {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
}

/// Lo que se ve de las zonas en el resumen (`guarda_copias.zonas`): sin
/// secretos; la carpeta, como la de la principal (v1.21).
pub fn resumen_zonas(c: &ServerConfig) -> Vec<serde_json::Value> {
    c.zonas
        .iter()
        .map(|z| {
            serde_json::json!({
                "id": z.id, "nombre": z.nombre, "carpeta": z.path, "puerto": z.port, "usuarios": z.users.len(),
                "escucha": c.enabled && listening(z.port),
                "espacio": crate::espacio::json_de(&z.path),
                "sistema_archivos": crate::espacio::json_fs(&z.path),
                "repositorios": repos_de_usuarios(&z.path, &z.users).into_iter().map(|(usuario, repos)| serde_json::json!({ "usuario": usuario, "repos": repos })).collect::<Vec<_>>(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redes_internas_sin_solapes() {
        let r = redes_internas(vec!["192.168.1.0/24".into(), "100.64.0.0/10".into()]);
        assert_eq!(r, ["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16", "100.64.0.0/10"]);
    }

    #[test]
    fn pid_viejo_de_otro_programa() {
        let salida = "\"rest-server.exe\",\"4321\",\"Services\",\"0\",\"12.345 KB\"\r\n";
        assert!(tasklist_es(salida, "rest-server.exe"));
        assert!(tasklist_es(salida, "REST-SERVER.EXE"));
        assert!(!tasklist_es("\"chrome.exe\",\"4321\",\"Console\",\"1\",\"99 KB\"", "rest-server.exe"));
        assert!(!tasklist_es("INFORMACIÓN: no hay tareas ejecutándose que coincidan con los criterios especificados.", "rest-server.exe"));
        assert!(!pid_es_rest_server(std::process::id()), "este proceso no es el rest-server");
    }

    #[test]
    fn espejo_y_servidor_no_se_solapan() {
        if cfg!(windows) {
            assert!(se_solapan(r"E:\Copias\espejo", r"e:\copias"), "sin distinguir mayúsculas");
            assert!(se_solapan(r"e:\COPIAS", r"E:\Copias\datos"), "ni al revés");
            assert!(!se_solapan(r"E:\Copias-espejo", r"E:\Copias"), "por componentes, no por texto");
        } else {
            assert!(se_solapan("/srv/copias/espejo", "/srv/copias"));
            assert!(!se_solapan("/srv/Copias", "/srv/copias"), "en Linux sí se distingue");
        }
        assert!(!se_solapan("", "/srv"));
    }

    #[test]
    fn usuarios_validos() {
        assert_eq!(user_name("Altamar PC1"), "altamar-pc1");
        assert_eq!(user_name("  Señora Íñiga!! "), "senora-iniga");
        assert_eq!(user_name("../../etc"), "etc");
        assert!(user_name(&"x".repeat(80)).len() <= 32);
    }

    #[test]
    fn contrasenas_y_htpasswd() {
        let p = new_password();
        assert_eq!(p.len(), 32);
        assert_ne!(p, new_password());
        let line = htpasswd_line("altamar", &p).unwrap();
        let (user, hash) = line.split_once(':').unwrap();
        assert_eq!(user, "altamar");
        assert!(bcrypt::verify(&p, hash).unwrap());
    }

    /// Tarea 7b: cada zona es otro rest-server con sus usuarios, pero siempre
    /// de solo añadir, con repos privados y el mismo certificado.
    #[test]
    fn cada_zona_es_de_solo_anadir() {
        let f = Files { public: PathBuf::from("pub"), private: PathBuf::from("priv") };
        let a = f.args_de(r"E:\Resguardo", "127.0.0.1:8002", Some("z1a2b3c"));
        for x in ["--append-only", "--private-repos", "--tls"] {
            assert!(a.contains(&x.to_string()), "{x} en {a:?}");
        }
        let valor = |k: &str| a.iter().position(|x| x == k).map(|i| a[i + 1].clone()).unwrap();
        assert!(valor("--htpasswd-file").ends_with("servidor-zona-z1a2b3c.htpasswd"), "sus propios usuarios");
        assert_eq!(valor("--tls-cert"), f.args("x", "y")[f.args("x", "y").iter().position(|x| x == "--tls-cert").unwrap() + 1], "el mismo certificado");
        assert_eq!(valor("--path"), r"E:\Resguardo");
        assert!(f.pid_de(Some("z1a2b3c")).ends_with("servidor-zona-z1a2b3c.pid") && f.pid_de(None).ends_with("servidor.pid"));
        assert!(zona_id_valido("z1a2b3c") && !zona_id_valido("z1A2b3c") && !zona_id_valido("../x") && !zona_id_valido("z12345"));
    }

    fn almacen() -> ServerConfig {
        let (d, e) = if cfg!(windows) { (r"D:\Resguardo", r"E:\Resguardo") } else { ("/srv/d", "/srv/e") };
        ServerConfig {
            enabled: true,
            path: d.into(),
            port: 8000,
            users: vec![ServerUser { name: "recepcion".into(), ..Default::default() }],
            zonas: vec![Zona {
                id: "z0a0b0c".into(),
                nombre: "Disco E".into(),
                path: e.into(),
                port: 8002,
                users: vec![ServerUser { name: "caja".into(), ..Default::default() }],
                creada: String::new(),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn zonas_sin_solapes_ni_puertos_repetidos() {
        let c = almacen();
        let (f, dentro_d, dentro_e) =
            if cfg!(windows) { (r"F:\Resguardo", r"D:\Resguardo\zona", r"e:\resguardo\otra") } else { ("/srv/f", "/srv/d/zona", "/srv/e/otra") };
        assert!(validar_zona_nueva(&c, f, 8004).is_ok());
        assert!(validar_zona_nueva(&c, f, 8000).unwrap_err().contains("8000"), "el de la principal");
        assert!(validar_zona_nueva(&c, f, 8002).unwrap_err().contains("8004"), "el de otra zona; propone uno libre");
        assert!(validar_zona_nueva(&c, f, 80).is_err());
        assert!(validar_zona_nueva(&c, dentro_d, 8004).unwrap_err().contains("almacén"));
        if cfg!(windows) {
            assert!(validar_zona_nueva(&c, dentro_e, 8004).unwrap_err().contains("otra zona"), "sin distinguir mayúsculas");
        } else {
            assert!(validar_zona_nueva(&c, dentro_e, 8004).unwrap_err().contains("otra zona"));
        }
        assert!(validar_zona_nueva(&c, "  ", 8004).is_err());
        let mut lleno = almacen();
        for i in 0..ZONAS_MAX {
            lleno.zonas.push(Zona { id: format!("z00000{i}"), path: format!("/z{i}"), port: 9000 + i as u16, ..Default::default() });
        }
        assert!(validar_zona_nueva(&lleno, f, 8004).unwrap_err().contains("Como mucho"));
        let apagado = ServerConfig { enabled: false, ..almacen() };
        assert!(validar_zona_nueva(&apagado, f, 8004).is_err());
        assert_eq!(c.puertos(), vec![8000, 8002]);
    }

    #[test]
    fn el_espejo_no_va_dentro_de_una_zona() {
        let mut c = almacen();
        let e = if cfg!(windows) { r"E:\Resguardo\espejo" } else { "/srv/e/espejo" };
        c.espejo = Some(crate::espejo::Espejo {
            destinos: vec![crate::espejo::Destino { tipo: "carpeta".into(), carpeta: e.into(), ..Default::default() }],
            ..Default::default()
        });
        c.zonas.clear();
        let zona = if cfg!(windows) { r"E:\Resguardo" } else { "/srv/e" };
        assert!(validar_zona_nueva(&c, zona, 8002).unwrap_err().contains("espejo"));
    }

    #[test]
    fn usuarios_unicos_en_todo_el_almacen() {
        let dir = std::env::temp_dir().join(format!("resguardo-zonas-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = Files { public: dir.clone(), private: dir.clone() };
        let mut c = almacen();
        // «caja» ya está en la zona E: en la principal es «caja-2», para el mismo equipo.
        let (u, p) = create_user_en(&f, &mut c, None, "Caja").unwrap();
        assert_eq!(u, "caja-2");
        assert!(c.users.iter().any(|x| x.name == "caja-2" && x.es_para("caja")));
        let lineas = f.read_htpasswd_de(None);
        assert!(lineas.iter().any(|l| l.starts_with("caja-2:")) && f.read_htpasswd_de(Some("z0a0b0c")).is_empty(), "en el archivo de su zona");
        assert!(bcrypt::verify(&p, lineas[0].split_once(':').unwrap().1).unwrap());
        // Y uno nuevo en la zona, en su propio archivo.
        let (u, _) = create_user_en(&f, &mut c, Some("z0a0b0c"), "Recepcion").unwrap();
        assert_eq!(u, "recepcion-2");
        assert!(f.read_htpasswd_de(Some("z0a0b0c"))[0].starts_with("recepcion-2:"));
        assert!(create_user_en(&f, &mut c, Some("z9999ff"), "x").is_err(), "una zona que no está");
        // La retención en el almacén encuentra la carpeta del usuario, esté donde esté.
        assert_eq!(c.carpeta_de_usuario("recepcion-2").map(|(p, _)| p.to_string()), Some(c.zonas[0].path.clone()));
        assert_eq!(c.carpeta_de_usuario("caja-2").map(|(p, _)| p.to_string()), Some(c.path.clone()));
        assert!(c.carpeta_de_usuario("nadie").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nombres_de_zona_y_lo_que_se_guarda() {
        assert_eq!(nombre_por_defecto(r"E:\Resguardo"), "Disco E");
        assert_eq!(nombre_por_defecto(r"e:\copias"), "Disco E");
        assert_eq!(nombre_por_defecto("/mnt/disco2/resguardo"), "resguardo");
        assert!(nombre_zona_valido("Disco E") && !nombre_zona_valido("  ") && !nombre_zona_valido(&"x".repeat(61)) && !nombre_zona_valido("a\nb"));
        // Un servidor.json de antes (sin zonas) se lee igual y no gana el campo al guardarse.
        let viejo: ServerConfig =
            serde_json::from_str(r#"{"enabled":true,"path":"/srv/d","port":8000,"local_subnet_only":true,"users":[{"name":"ana","created_at":""}]}"#).unwrap();
        assert!(viejo.zonas.is_empty() && viejo.users[0].para.is_none());
        let texto = serde_json::to_string(&viejo).unwrap();
        assert!(!texto.contains("zonas") && !texto.contains("para"), "{texto}");
        let nuevo = serde_json::to_value(almacen()).unwrap();
        assert_eq!(nuevo["zonas"][0]["port"], 8002);
    }

    #[test]
    fn siempre_append_only_y_privado() {
        let c = ServerConfig { path: r"D:\Copias".into(), port: 8443, ..Default::default() };
        let a = args(&c);
        assert!(a.contains(&"--append-only".to_string()) && a.contains(&"--private-repos".to_string()) && a.contains(&"--tls".to_string()));
        // «:8443» en producción; «127.0.0.1:8443» en pruebas con RESGUARDO_AGENT_DIR.
        assert!(a.iter().any(|x| x.ends_with(":8443")));
    }

    #[test]
    fn certificado_propio() {
        let (ca, ca_key, fp) = generate_ca().unwrap();
        assert!(ca.starts_with("-----BEGIN CERTIFICATE-----") && ca_key.contains("PRIVATE KEY"));
        assert_eq!(fp.split(':').count(), 32);
        // El del servidor se puede volver a emitir con la misma autoridad (otra IP).
        let (leaf, key) = generate_leaf(&ca_key, &["localhost".into(), "192.168.1.20".into()]).unwrap();
        assert!(leaf.starts_with("-----BEGIN CERTIFICATE-----") && key.contains("PRIVATE KEY"));
        if let Ok(dir) = std::env::var("RESGUARDO_CERT_OUT") {
            std::fs::write(format!("{dir}/ca.pem"), &ca).unwrap();
            std::fs::write(format!("{dir}/leaf.pem"), &leaf).unwrap();
        }
        assert!(generate_leaf(&ca_key, &["localhost".into(), "10.0.0.7".into()]).is_ok());
    }

    #[test]
    fn redes_y_reglas_de_nftables() {
        let salida = "2: eth0    inet 192.168.1.20/24 brd 192.168.1.255 scope global eth0\\       valid_lft forever
3: eth1    inet 10.4.7.9/16 brd 10.4.255.255 scope global eth1
4: wg0    inet 10.4.200.1/16 scope global wg0
5: rara    inet 1.2.3.4/99 scope global rara";
        assert_eq!(redes_de_ip_addr(salida), vec!["192.168.1.0/24".to_string(), "10.4.0.0/16".to_string()]);
        let r = reglas_nft(&[8000], &redes_de_ip_addr(salida));
        assert!(r.contains("tcp dport 8000 ip saddr { 192.168.1.0/24, 10.4.0.0/16 } accept") && r.contains("tcp dport 8000 drop"));
        assert!(r.starts_with("# Resguardo") && r.contains("delete table inet resguardo"));
        // Sin redes conocidas: las privadas.
        assert!(reglas_nft(&[8000], &[]).contains("{ 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16 }"));
        // 7b: con zonas, todos los puertos del almacén en las mismas reglas.
        let r = reglas_nft(&[8000, 8002], &["192.168.1.0/24".into()]);
        assert!(r.contains("tcp dport { 8000, 8002 } ip saddr { 192.168.1.0/24 } accept") && r.contains("tcp dport { 8000, 8002 } drop"), "{r}");
    }

    #[test]
    fn sin_huella_no_arranca() {
        if PINNED_SHA256.is_none() {
            assert!(binary_check().unwrap_err().contains("huella"));
        }
    }
}
