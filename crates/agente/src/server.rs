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

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ServerUser {
    pub name: String,
    pub created_at: String,
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
        }
    }
}

/// ¿Dentro de la carpeta de Windows, de los programas o de Resguardo?
fn carpeta_del_sistema(p: &Path) -> bool {
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

/// Activa, cambia o quita (con `None`) el espejo nocturno del Servidor de
/// copias. `nuevo` trae los destinos, la hora y el límite (espejo::pedido).
pub fn poner_espejo(nuevo: Option<crate::espejo::Espejo>) -> Result<String, String> {
    crate::agent::require_admin()?;
    let mut c = load();
    if !c.enabled {
        return Err("Activa antes el Servidor de copias.".into());
    }
    let Some(mut nuevo) = nuevo else {
        c.espejo = None;
        save(&c)?;
        return Ok("Espejo quitado (lo ya copiado se queda en su destino).".into());
    };
    nuevo.normalizar();
    if nuevo.destinos.is_empty() {
        return Err("Falta al menos un destino para el espejo.".into());
    }
    if chrono::NaiveTime::parse_from_str(&nuevo.hora, "%H:%M").is_err() {
        return Err("Hora no válida (HH:MM).".into());
    }
    let nubes = crate::nube::cargar();
    for d in nuevo.destinos.iter_mut() {
        d.carpeta = d.carpeta.trim().to_string();
        match d.tipo.as_str() {
            "carpeta" => {
                crate::platform::carpeta_local_valida(&d.carpeta)?;
                if se_solapan(&d.carpeta, &c.path) {
                    return Err("La carpeta del espejo no puede estar dentro de la del Servidor de copias (ni al revés).".into());
                }
                if carpeta_del_sistema(Path::new(&d.carpeta)) {
                    return Err("La carpeta del espejo no puede estar en la carpeta de Windows, de los programas o de Resguardo.".into());
                }
            }
            "nube" => {
                let nombre = d.nube.as_deref().unwrap_or_default();
                if !nubes.iter().any(|n| n.nombre == nombre) {
                    return Err(format!("No hay ninguna nube «{nombre}» conectada en este equipo: conéctala antes desde la consola («Conectar Dropbox»)."));
                }
                if !crate::nube::carpeta_remota_valida(&d.carpeta) {
                    return Err("Carpeta de la nube no válida (por ejemplo, Resguardo/Sur).".into());
                }
                d.carpeta = d.carpeta.trim_matches('/').to_string();
            }
            _ => return Err("Tipo de destino del espejo no válido (carpeta o nube).".into()),
        }
    }
    let n = nuevo.destinos.len();
    if (0..n).any(|i| (0..i).any(|j| nuevo.destinos[i].mismo(&nuevo.destinos[j]))) {
        return Err("Hay un destino repetido en el espejo.".into());
    }
    // Las carpetas del espejo, solo para SYSTEM y Administradores: SYSTEM
    // escribe en ellas cada noche (nada de enlaces puestos por un usuario).
    for d in nuevo.destinos.iter().filter(|d| d.tipo == "carpeta") {
        crate::platform::carpeta_privada(Path::new(&d.carpeta))?;
    }
    // Lo ya hecho en los destinos que siguen se conserva (su última vuelta
    // cuenta para el horario); uno nuevo hace que toque ya.
    let anterior = c.espejo.take().map(|mut e| {
        e.normalizar();
        e
    });
    for d in nuevo.destinos.iter_mut() {
        if let Some(x) = anterior.as_ref().and_then(|a| a.destinos.iter().find(|x| x.mismo(d))) {
            (d.ultima, d.resultado, d.inicio, d.cuota) = (x.ultima.clone(), x.resultado.clone(), x.inicio.clone(), x.cuota.clone());
            (d.verificacion, d.danados_origen) = (x.verificacion.clone(), x.danados_origen);
            (d.por_borrar, d.freno) = (x.por_borrar.clone(), x.freno.clone());
        } else {
            // Uno nuevo (o que vuelve) empieza sin nada anotado de otra vez (§3b).
            crate::espejo::olvidar_estado(d);
        }
    }
    crate::espejo::fijar_vistos(&mut nuevo, anterior.as_ref(), &crate::espejo::repos_en(Path::new(&c.path)));
    if let Some(a) = anterior {
        nuevo.ultima = a.ultima;
    }
    nuevo.resultado = crate::espejo::resultado_global(&nuevo.destinos);
    let texto = nuevo.destinos.iter().map(|d| format!("«{}»", d.texto())).collect::<Vec<_>>().join(" y ");
    let cuando =
        if nuevo.destinos.iter().all(|d| d.horario.is_none() && !d.tras_copia) { format!("cada día a las {}", nuevo.hora) } else { "con su horario".into() };
    c.espejo = Some(nuevo);
    save(&c)?;
    Ok(format!("Espejo {cuando} en {texto}."))
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
            self.htpasswd().display().to_string(),
        ]
    }

    /// Reescribe `.htpasswd` con estas líneas (archivo nuevo en la carpeta privada).
    pub fn write_htpasswd(&self, lines: &[String]) -> Result<(), String> {
        let tmp = self.private("servidor.htpasswd.tmp");
        let _ = std::fs::remove_file(&tmp);
        std::fs::write(&tmp, lines.join("\n") + "\n").map_err(|e| format!("No se pudieron guardar los usuarios del servidor: {e}"))?;
        std::fs::rename(&tmp, self.htpasswd()).map_err(|e| e.to_string())
    }
}

fn private(name: &str) -> PathBuf {
    crate::agent::private_dir().join(name)
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
    let base = user_name(raw);
    if base.is_empty() {
        return Err("Escribe un nombre para el equipo (letras, cifras o guiones).".into());
    }
    let mut user = base.clone();
    for i in 2.. {
        if !c.users.iter().any(|u| u.name == user) {
            break;
        }
        user = format!("{base}-{i}");
    }
    let password = new_password();
    let mut lines: Vec<String> = read_htpasswd().into_iter().filter(|l| !l.starts_with(&format!("{user}:"))).collect();
    lines.push(htpasswd_line(&user, &password)?);
    write_htpasswd(&lines)?;
    c.users.push(ServerUser { name: user.clone(), created_at: chrono::Local::now().to_rfc3339() });
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
    // En pruebas, solo en este equipo (sin que el cortafuegos pregunte nada).
    let escuchar = if crate::agent::test_mode() { format!("127.0.0.1:{}", c.port) } else { format!(":{}", c.port) };
    Files::agent().args(&c.path, &escuchar)
}

/// Repositorios de cada usuario (`/<usuario>/<repo>/` con `config`): solo nombres.
pub fn repos_by_user(c: &ServerConfig) -> Vec<(String, Vec<String>)> {
    c.users
        .iter()
        .map(|u| {
            let base = Path::new(&c.path).join(&u.name);
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

/// `resguardo.exe --server-run`: comprueba el binario y lo mantiene en marcha.
pub fn run_forever() -> i32 {
    let mut backoff = 5;
    loop {
        let c = load();
        if !c.enabled {
            return 0;
        }
        if let Err(e) = binary_check() {
            crate::agent::log(&format!("ERROR: Servidor de copias: {e}"));
            return 1;
        }
        // Linux: las reglas de nftables no sobreviven a un reinicio; se ponen al arrancar.
        #[cfg(unix)]
        {
            if let Err(e) = firewall(c.port, c.local_subnet_only) {
                crate::agent::log(&format!("ERROR: Servidor de copias: {e}"));
            }
        }
        // Al arrancar (p. ej. tras reiniciar con otra IP), el certificado al día.
        let c = {
            let mut c = c;
            if let Err(e) = refresh_leaf_if_needed(&mut c) {
                crate::agent::log(&format!("ERROR: Servidor de copias: no se pudo renovar el certificado: {e}"));
            }
            c
        };
        let started = std::time::Instant::now();
        let mut orden = std::process::Command::new(binary());
        resguardo_motor::proceso::entorno_minimo(&mut orden);
        let status = orden.args(args(&c)).stdin(std::process::Stdio::null()).spawn().and_then(|mut child| {
            // Para poder pararlo al desactivar el servidor.
            let _ = std::fs::write(private("servidor.pid"), child.id().to_string());
            // Mientras funciona: si cambia la IP del equipo, se renueva su
            // certificado y se reinicia con él.
            let mut checked = std::time::Instant::now();
            loop {
                if let Some(s) = child.try_wait()? {
                    return Ok(s);
                }
                if checked.elapsed() > std::time::Duration::from_secs(300) {
                    checked = std::time::Instant::now();
                    let mut current = load();
                    match refresh_leaf_if_needed(&mut current) {
                        Ok(true) => {
                            let _ = child.kill();
                            return child.wait();
                        }
                        Ok(false) => {}
                        Err(e) => crate::agent::log(&format!("ERROR: Servidor de copias: no se pudo renovar el certificado: {e}")),
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
        });
        match status {
            Ok(s) => crate::agent::log(&format!("Servidor de copias: rest-server terminó ({s}).")),
            Err(e) => crate::agent::log(&format!("ERROR: Servidor de copias: no se pudo arrancar rest-server: {e}")),
        }
        // Si duró poco, se espera cada vez más (sin pasar de 5 minutos).
        backoff = if started.elapsed().as_secs() > 300 { 5 } else { (backoff * 2).min(300) };
        std::thread::sleep(std::time::Duration::from_secs(backoff));
    }
}

/// Regla del firewall solo para el puerto del servidor.
#[cfg(windows)]
pub fn firewall(port: u16, local_only: bool) -> Result<(), String> {
    let _ = crate::platform::tool("netsh.exe", &["advfirewall", "firewall", "delete", "rule", &format!("name={}", regla_firewall())]);
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
            &format!("localport={port}"),
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
pub fn firewall(port: u16, local_only: bool) -> Result<(), String> {
    let nft = nft().ok_or("Falta nftables (el programa nft): instálalo con «apt install nftables».")?;
    if !local_only {
        firewall_remove();
        return Ok(());
    }
    let reglas = reglas_nft(port, &redes_internas(redes_locales()));
    let archivo = private("servidor.nft");
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

/// Reglas de nftables para el puerto del servidor (se aplican de una vez:
/// la tabla se crea, se vacía y se vuelve a llenar).
pub fn reglas_nft(port: u16, redes: &[String]) -> String {
    let redes = if redes.is_empty() { REDES_PRIVADAS.iter().map(|r| r.to_string()).collect::<Vec<_>>() } else { redes.to_vec() };
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

/// Para el rest-server que lanzó la tarea (por su PID, nunca por nombre), si
/// ese PID sigue siendo el rest-server.
pub fn stop() {
    if let Some(pid) = std::fs::read_to_string(private("servidor.pid")).ok().and_then(|s| s.trim().parse::<u32>().ok()).filter(|p| pid_es_rest_server(*p)) {
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
    let _ = std::fs::remove_file(private("servidor.pid"));
}

/// Reescribe `.htpasswd` con estas líneas (archivo nuevo en la carpeta privada).
pub fn write_htpasswd(lines: &[String]) -> Result<(), String> {
    Files::agent().write_htpasswd(lines)
}

pub fn read_htpasswd() -> Vec<String> {
    std::fs::read_to_string(private("servidor.htpasswd")).map(|s| s.lines().filter(|l| !l.trim().is_empty()).map(str::to_string).collect()).unwrap_or_default()
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
    binary_check()?;
    crate::platform::carpeta_privada(Path::new(&path))?;
    crate::agent::prepare_dir()?;
    let mut c = load();
    let names = current_names();
    if c.tls_sha256.is_none() || !cert_file().is_file() || !leaf_file().is_file() {
        c.tls_sha256 = Some(make_cert(&names)?);
        c.cert_names = names;
    }
    if read_htpasswd().is_empty() {
        write_htpasswd(&[])?;
    }
    c.enabled = true;
    c.path = path;
    c.port = port;
    c.local_subnet_only = local_subnet_only;
    save(&c)?;
    stop();
    // El puerto tiene que estar libre (otro programa, o Resguardo Server en el mismo equipo).
    let libre = (0..20).any(|_| {
        let ok = std::net::TcpListener::bind((if crate::agent::test_mode() { "127.0.0.1" } else { "0.0.0.0" }, port)).is_ok();
        if !ok {
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        ok
    });
    if !libre {
        c.enabled = false;
        let _ = save(&c);
        return Err(format!("El puerto {port} ya está en uso en este equipo: elige otro."));
    }
    if crate::agent::test_mode() {
        // Pruebas (compilación de desarrollo con RESGUARDO_AGENT_DIR): sin
        // cortafuegos ni tarea de SYSTEM; el servidor corre en este proceso.
        std::thread::spawn(run_forever);
    } else {
        firewall(port, local_subnet_only)?;
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
    if !crate::agent::test_mode() {
        crate::platform::uninstall_server_task();
        firewall_remove();
    }
    crate::agent::log("Servidor de copias desactivado.");
    Ok(())
}

/// Añade un equipo cliente: su usuario, su contraseña (128 bits) y la
/// ubicación del repositorio que le toca (`rest:https://<ip>:<puerto>/<usuario>/`).
pub fn anadir_equipo(name: &str) -> Result<(String, String, String), String> {
    crate::agent::require_admin()?;
    let mut c = load();
    if !c.enabled {
        return Err("Activa antes el Servidor de copias.".into());
    }
    if c.users.iter().any(|u| u.name == user_name(name)) {
        return Err(format!("Ya hay un equipo «{}» en este servidor.", user_name(name)));
    }
    let (user, password) = create_user(&mut c, name)?;
    save(&c)?;
    // En pruebas el rest-server solo escucha en 127.0.0.1 (ver `args`): `localhost`, que el certificado cubre.
    let ip = if crate::agent::test_mode() { None } else { lan_addresses().into_iter().next() }.unwrap_or_else(|| "localhost".into());
    let location = format!("rest:https://{ip}:{}/{user}/", c.port);
    crate::agent::log(&format!("Servidor de copias: nuevo equipo cliente «{user}»."));
    Ok((user, password, location))
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
    if let Err(e) = esperar_usuario_o_error(user, password) {
        crate::agent::log(&format!("Servidor de copias: {e}"));
    }
}

fn esperar_usuario_o_error(user: &str, password: &str) -> Result<(), String> {
    let c = load();
    if !c.enabled || !listening(c.port) {
        return Ok(());
    }
    #[cfg(unix)]
    if let Some(pid) = std::fs::read_to_string(private("servidor.pid")).ok().and_then(|s| s.trim().parse::<libc::pid_t>().ok()).filter(|p| *p > 1) {
        if pid_es_rest_server(pid as u32) {
            // SAFETY: solo envía una señal al rest-server que lanzó run_forever (rest-server relee .htpasswd con SIGHUP).
            unsafe { libc::kill(pid, libc::SIGHUP) };
        }
    }
    esperar_usuario_en(c.port, &cert_file(), user, password)
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
    crate::agent::require_admin()?;
    let mut c = load();
    c.users.retain(|u| u.name != name);
    save(&c)?;
    let lines: Vec<String> = read_htpasswd().into_iter().filter(|l| !l.starts_with(&format!("{name}:"))).collect();
    write_htpasswd(&lines)
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
        let r = reglas_nft(8000, &redes_de_ip_addr(salida));
        assert!(r.contains("tcp dport 8000 ip saddr { 192.168.1.0/24, 10.4.0.0/16 } accept") && r.contains("tcp dport 8000 drop"));
        assert!(r.starts_with("# Resguardo") && r.contains("delete table inet resguardo"));
        // Sin redes conocidas: las privadas.
        assert!(reglas_nft(8000, &[]).contains("{ 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16 }"));
    }

    #[test]
    fn sin_huella_no_arranca() {
        if PINNED_SHA256.is_none() {
            assert!(binary_check().unwrap_err().contains("huella"));
        }
    }
}
