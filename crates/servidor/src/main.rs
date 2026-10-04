//! `resguardo-server`: el programa. Ver `resguardo-server --ayuda`.

use resguardo_servidor::estado::Opciones;
use resguardo_servidor::{identidad, preparar, servir, Tls};
use std::net::SocketAddr;
use std::path::PathBuf;

#[cfg(windows)]
mod servicio;

const AYUDA: &str = "Resguardo Server: consola web y canal de los agentes de Resguardo.

Uso: resguardo-server [opciones]
     resguardo-server codigo-inicial [--datos DIR]
     resguardo-server hacer-respaldo [--datos DIR]
     resguardo-server restaurar-respaldo ARCHIVO [--datos DIR] [--reemplazar] [--confiar-en HUELLA]

  --datos DIR          Carpeta de datos (por defecto /var/lib/resguardo-server; en Windows,
                       C:\\ProgramData\\Resguardo Server)
  --escuchar DIR:PUERTO  Dónde escuchar (por defecto 0.0.0.0:8443)
  --nombre NOMBRE      Nombre o IP extra para el certificado propio (se puede repetir)
  --cert ARCHIVO --clave ARCHIVO   Certificado y clave propios (PEM) en vez de los generados
  --sin-tls            Sin HTTPS: solo detrás de un proxy con HTTPS (exige --detras-de-proxy)
                       o escuchando en 127.0.0.1
  --detras-de-proxy    Hay un proxy con HTTPS delante en este mismo equipo (Caddy, nginx): la
                       cookie sigue siendo Secure y la IP de quien pide es la última de
                       X-Forwarded-For (solo en conexiones desde 127.0.0.1)
  --consola DIR        Carpeta con la consola web compilada (tiene prioridad sobre la que
                       lleva dentro el binario compilado con la feature consola-integrada)
  --max-descarga MB    Tamaño máximo de una descarga al navegador (por defecto 500)
  --instalador-agente ARCHIVO  Instalador del agente para «Descargar instalador listo» (por
                       defecto, agente/Resguardo-Agente-setup.exe junto al programa)
  --ayuda              Esta ayuda

  Consola en internet (docs/consola-en-linea.md):
  --dominio NOMBRE     Certificado público automático (Let's Encrypt, ACME) para ese nombre,
                       que tiene que apuntar a este servidor; el reto HTTP-01 usa el puerto 80
                       (que además redirige a HTTPS). Para el puerto 443, --escuchar 0.0.0.0:443.
                       Al usarlo aceptas las condiciones de Let's Encrypt.
  --acme-correo CORREO Correo para los avisos de Let's Encrypt (recomendado)
  --acme-pruebas       Let's Encrypt de pruebas (sin sus límites; el navegador no confía en él)
  --acme-directorio URL  Otra autoridad ACME (en vez de Let's Encrypt)
  --acme-http DIR:PUERTO  Dónde escuchar los retos HTTP-01 (por defecto 0.0.0.0:80)
  --dominio-agentes NOMBRE  El nombre para los agentes y las otras consolas (por defecto
                       agentes.<dominio>; tiene que apuntar también a este servidor). Ahí se
                       sirve el certificado de la autoridad propia, el que fijan al vincularse.
  --url-agentes URL    La dirección que la consola da a los agentes, si no es la suya (p. ej.
                       detrás de un proxy: https://consola.ejemplo.com:8443)
  --publico            Consola en internet sin --dominio (p. ej. detrás de un proxy): cuotas
                       por cliente más estrictas por defecto. --dominio ya lo implica.

  codigo-inicial       Muestra la dirección de la consola, la huella de la autoridad TLS y el
                       código de primer arranque (para crear la cuenta de propietario), sin
                       parar el servidor. Como root (sudo) o como el usuario del servicio.

  hacer-respaldo       Hace ahora una copia de la consola (cifrada con la clave de respaldo
                       que se puso en la consola, «Servidor» → «Copia de la consola») en
                       la carpeta respaldos/ de los datos. El servidor puede seguir en marcha.
  restaurar-respaldo ARCHIVO
                       Restaura una copia de la consola (.resguardo-consola) en la carpeta
                       de datos: pide la clave de respaldo y deja el servidor con su misma
                       identidad (los equipos vuelven solos, sin vincularse otra vez). Con
                       el servicio parado. Si ya hay un servidor en esa carpeta, hace falta
                       --reemplazar (lo que había se aparta en antes-de-restaurar-…, no se
                       borra). La clave también se puede dar en RESGUARDO_CLAVE_RESPALDO.
                       Antes de pedir la clave comprueba la firma de la copia y enseña la
                       huella de la identidad del servidor que la hizo: tiene que ser la del
                       kit. Se confirma en la terminal o con --confiar-en HUELLA (la del kit,
                       AB:CD:…); si no coincide, no se restaura. Las copias anteriores a
                       0.7.11 no llevan firma: se avisa y se pide lo mismo.

Variables de entorno (las opciones tienen prioridad; en Linux, el servicio de systemd
las lee de /etc/resguardo-server/servidor.env):
  RESGUARDO_DATOS      Como --datos
  RESGUARDO_ESCUCHAR   Como --escuchar (p. ej. 0.0.0.0:8443)
  RESGUARDO_NOMBRES    Como --nombre: varios, separados por comas o espacios
  RESGUARDO_DOMINIO, RESGUARDO_ACME_CORREO, RESGUARDO_DOMINIO_AGENTES, RESGUARDO_URL_AGENTES,
  RESGUARDO_ACME_DIRECTORIO, RESGUARDO_ACME_HTTP, RESGUARDO_MAX_DESCARGA
                       Como las opciones del mismo nombre
  RESGUARDO_ACME_PRUEBAS=1, RESGUARDO_DETRAS_DE_PROXY=1, RESGUARDO_PUBLICO=1
                       Como --acme-pruebas, --detras-de-proxy y --publico

Registro: la salida estándar (en Linux, journalctl -u resguardo-server).

Solo en Windows (como administrador):
  --instalar-servicio [--escuchar …] [--toda-la-red]
                       Instala y arranca el servicio «ResguardoServer» (arranque automático,
                       se reinicia si falla), con su regla del cortafuegos para el puerto
                       (solo la red local, salvo --toda-la-red). Datos en ProgramData.
  --desinstalar-servicio
                       Para y quita el servicio y su regla. Los datos se quedan.
";

/// Lo que se decide en la línea de órdenes.
pub struct Config {
    pub datos: PathBuf,
    pub escuchar: SocketAddr,
    pub nombres: Vec<String>,
    pub cert: Option<PathBuf>,
    pub clave: Option<PathBuf>,
    pub sin_tls: bool,
    pub proxy: bool,
    pub consola: Option<PathBuf>,
    pub max_mb: u64,
    /// Instalador genérico del agente (v1.17); por defecto, agente/Resguardo-Agente-setup.exe junto al programa.
    pub instalador_agente: Option<PathBuf>,
    /// Consola en internet: el certificado público (ACME) para este nombre.
    pub dominio: Option<String>,
    pub acme_correo: Option<String>,
    pub acme_pruebas: bool,
    pub acme_directorio: Option<String>,
    pub acme_http: SocketAddr,
    /// El nombre para los agentes (por defecto `agentes.<dominio>`).
    pub dominio_agentes: Option<String>,
    pub url_agentes: Option<String>,
    pub publico: bool,
}

/// Carpeta de datos por defecto.
pub fn datos_por_defecto() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into())).join("Resguardo Server")
    } else {
        PathBuf::from("/var/lib/resguardo-server")
    }
}

fn main() {
    if let Err(e) = principal() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

enum Modo {
    Servir,
    Servicio,
    Instalar {
        // Solo lo usa el servicio de Windows.
        #[cfg_attr(not(windows), allow(dead_code))]
        toda_la_red: bool,
    },
    Desinstalar,
    CodigoInicial,
    HacerRespaldo,
    RestaurarRespaldo {
        archivo: PathBuf,
        reemplazar: bool,
        /// `--confiar-en`: la huella (o la identidad) que se espera, la del kit.
        confiar_en: Option<String>,
    },
}

/// Valor de una variable de entorno `RESGUARDO_*` (vacía = sin poner).
fn entorno(nombre: &str) -> Option<String> {
    std::env::var(nombre).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// ¿Está puesta (1, si, true…) una variable de entorno de sí o no?
fn entorno_si(nombre: &str) -> bool {
    entorno(nombre).is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "si" | "sí" | "true" | "yes"))
}

fn leer_args(args: &[String]) -> Result<Option<(Config, Modo)>, String> {
    // Por defecto, lo del entorno (la unidad de systemd); las opciones mandan.
    let escuchar = match entorno("RESGUARDO_ESCUCHAR") {
        Some(v) => v.parse().map_err(|_| format!("RESGUARDO_ESCUCHAR no es una dirección válida: {v} (p. ej. 0.0.0.0:8443)."))?,
        None => "0.0.0.0:8443".parse().expect("dirección fija"),
    };
    let nombres = entorno("RESGUARDO_NOMBRES").map(|v| v.split([',', ' ']).filter(|n| !n.is_empty()).map(str::to_string).collect()).unwrap_or_default();
    let mut c = Config {
        datos: entorno("RESGUARDO_DATOS").map(PathBuf::from).unwrap_or_else(datos_por_defecto),
        escuchar,
        nombres,
        cert: None,
        clave: None,
        sin_tls: false,
        proxy: false,
        consola: None,
        max_mb: match entorno("RESGUARDO_MAX_DESCARGA") {
            Some(v) => v.parse().map_err(|_| format!("RESGUARDO_MAX_DESCARGA no es un número de MB: {v}."))?,
            None => 500,
        },
        instalador_agente: None,
        dominio: entorno("RESGUARDO_DOMINIO"),
        acme_correo: entorno("RESGUARDO_ACME_CORREO"),
        acme_pruebas: entorno_si("RESGUARDO_ACME_PRUEBAS"),
        acme_directorio: entorno("RESGUARDO_ACME_DIRECTORIO"),
        acme_http: match entorno("RESGUARDO_ACME_HTTP") {
            Some(v) => v.parse().map_err(|_| format!("RESGUARDO_ACME_HTTP no es una dirección válida: {v} (p. ej. 0.0.0.0:80)."))?,
            None => "0.0.0.0:80".parse().expect("dirección fija"),
        },
        dominio_agentes: entorno("RESGUARDO_DOMINIO_AGENTES"),
        url_agentes: entorno("RESGUARDO_URL_AGENTES"),
        publico: entorno_si("RESGUARDO_PUBLICO"),
    };
    c.proxy = entorno_si("RESGUARDO_DETRAS_DE_PROXY");
    let (mut modo, mut toda_la_red, mut reemplazar, mut confiar_en) = (Modo::Servir, false, false, None);
    let mut i = 0;
    let valor = |i: &mut usize| -> Result<String, String> {
        *i += 1;
        args.get(*i).cloned().ok_or_else(|| format!("Falta el valor de {}", args[*i - 1]))
    };
    while i < args.len() {
        match args[i].as_str() {
            "--datos" => c.datos = PathBuf::from(valor(&mut i)?),
            "--escuchar" => c.escuchar = valor(&mut i)?.parse().map_err(|_| "Dirección no válida (p. ej. 0.0.0.0:8443).")?,
            "--nombre" => c.nombres.push(valor(&mut i)?),
            "--cert" => c.cert = Some(PathBuf::from(valor(&mut i)?)),
            "--clave" => c.clave = Some(PathBuf::from(valor(&mut i)?)),
            "--sin-tls" => c.sin_tls = true,
            "--detras-de-proxy" => c.proxy = true,
            "--consola" => c.consola = Some(PathBuf::from(valor(&mut i)?)),
            "--max-descarga" => c.max_mb = valor(&mut i)?.parse().map_err(|_| "Tamaño no válido.")?,
            "--instalador-agente" => c.instalador_agente = Some(PathBuf::from(valor(&mut i)?)),
            "--dominio" => c.dominio = Some(valor(&mut i)?),
            "--acme-correo" => c.acme_correo = Some(valor(&mut i)?),
            "--acme-pruebas" => c.acme_pruebas = true,
            "--acme-directorio" => c.acme_directorio = Some(valor(&mut i)?),
            "--acme-http" => c.acme_http = valor(&mut i)?.parse().map_err(|_| "Dirección no válida para --acme-http (p. ej. 0.0.0.0:80).")?,
            "--dominio-agentes" => c.dominio_agentes = Some(valor(&mut i)?),
            "--url-agentes" => c.url_agentes = Some(valor(&mut i)?),
            "--publico" => c.publico = true,
            "--servicio" => modo = Modo::Servicio,
            "--instalar-servicio" => modo = Modo::Instalar { toda_la_red: false },
            "--desinstalar-servicio" => modo = Modo::Desinstalar,
            "--toda-la-red" => toda_la_red = true,
            "codigo-inicial" | "--codigo-inicial" => modo = Modo::CodigoInicial,
            "hacer-respaldo" => modo = Modo::HacerRespaldo,
            "restaurar-respaldo" => modo = Modo::RestaurarRespaldo { archivo: PathBuf::from(valor(&mut i)?), reemplazar: false, confiar_en: None },
            "--reemplazar" => reemplazar = true,
            "--confiar-en" => confiar_en = Some(valor(&mut i)?),
            "--ayuda" | "--help" | "-h" => {
                print!("{AYUDA}");
                return Ok(None);
            }
            otro => return Err(format!("Opción desconocida: {otro} (ver --ayuda)")),
        }
        i += 1;
    }
    if let Modo::Instalar { .. } = modo {
        modo = Modo::Instalar { toda_la_red };
    }
    if let Modo::RestaurarRespaldo { archivo, .. } = modo {
        modo = Modo::RestaurarRespaldo { archivo, reemplazar, confiar_en };
    }
    if c.sin_tls && !c.proxy && !c.escuchar.ip().is_loopback() {
        return Err("Sin TLS solo se permite escuchando en 127.0.0.1 o con --detras-de-proxy.".into());
    }
    if c.cert.is_some() != c.clave.is_some() {
        return Err("--cert y --clave van juntos.".into());
    }
    preparar_dominio(&mut c)?;
    Ok(Some((c, modo)))
}

/// Con `--dominio`: valida los nombres, decide la dirección de los agentes y
/// comprueba que no choca con otras opciones.
fn preparar_dominio(c: &mut Config) -> Result<(), String> {
    use resguardo_servidor::acme;
    if let Some(u) = &c.url_agentes {
        let u = u.trim().trim_end_matches('/');
        if !u.starts_with("https://") || u.len() <= "https://".len() || u.contains(char::is_whitespace) {
            return Err(format!("--url-agentes tiene que ser una dirección https:// ({u})."));
        }
        c.url_agentes = Some(u.to_string());
    }
    if let Some(correo) = &c.acme_correo {
        c.acme_correo = Some(acme::valida_correo(correo)?);
    }
    let Some(d) = c.dominio.clone() else {
        if c.acme_correo.is_some() || c.acme_pruebas || c.acme_directorio.is_some() || c.dominio_agentes.is_some() {
            return Err("Las opciones --acme-… y --dominio-agentes van con --dominio.".into());
        }
        return Ok(());
    };
    if c.sin_tls {
        return Err("--dominio y --sin-tls no van juntos (detrás de un proxy, el certificado público lo pone el proxy).".into());
    }
    if c.cert.is_some() {
        return Err("--dominio y --cert no van juntos: con --dominio el certificado público se pide solo.".into());
    }
    let d = acme::valida_dominio(&d)?;
    let agentes = acme::valida_dominio(c.dominio_agentes.as_deref().unwrap_or(&format!("agentes.{d}")))?;
    if agentes == d {
        return Err("--dominio-agentes tiene que ser otro nombre que --dominio (ahí se sirve el certificado propio que fijan los agentes).".into());
    }
    let mismo_ip = c.acme_http.ip() == c.escuchar.ip() || c.acme_http.ip().is_unspecified() || c.escuchar.ip().is_unspecified();
    if c.acme_http.port() == c.escuchar.port() && mismo_ip {
        return Err(format!("--acme-http ({}) no puede ser el mismo puerto que --escuchar ({}).", c.acme_http, c.escuchar));
    }
    if c.url_agentes.is_none() {
        let puerto = c.escuchar.port();
        c.url_agentes = Some(if puerto == 443 { format!("https://{agentes}") } else { format!("https://{agentes}:{puerto}") });
    }
    // El certificado propio cubre también los dos nombres.
    for n in [&d, &agentes] {
        if !c.nombres.contains(n) {
            c.nombres.push(n.clone());
        }
    }
    c.dominio = Some(d);
    c.dominio_agentes = Some(agentes);
    c.publico = true;
    Ok(())
}

fn principal() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((c, modo)) = leer_args(&args)? else { return Ok(()) };
    match modo {
        Modo::Servir => arrancar(c, &|l| println!("{l}")),
        Modo::CodigoInicial => codigo_inicial(&c),
        Modo::HacerRespaldo => hacer_respaldo(&c),
        Modo::RestaurarRespaldo { archivo, reemplazar, confiar_en } => restaurar_respaldo(&c, &archivo, reemplazar, confiar_en.as_deref()),
        #[cfg(windows)]
        Modo::Servicio => servicio::ejecutar(c),
        #[cfg(windows)]
        Modo::Instalar { toda_la_red } => servicio::instalar(&c, toda_la_red),
        #[cfg(windows)]
        Modo::Desinstalar => servicio::desinstalar(),
        #[cfg(not(windows))]
        _ => Err("Esa opción solo existe en Windows (en Linux, el servicio de systemd: packaging/linux/instalar-servidor.sh).".into()),
    }
}

/// Archivo con el código de primer arranque (solo administradores): lo lee
/// quien instala el servidor como servicio, que no ve su salida.
pub fn archivo_codigo(datos: &std::path::Path) -> PathBuf {
    datos.join("codigo-arranque.txt")
}

/// `codigo-inicial`: lo que necesita quien instala el servidor sin pantalla
/// (p. ej. en un CT de Proxmox) para entrar en la consola por primera vez.
fn codigo_inicial(c: &Config) -> Result<(), String> {
    let huella = std::fs::read_to_string(c.datos.join("tls").join("ca.huella"));
    let codigo = std::fs::read_to_string(archivo_codigo(&c.datos));
    let sin_permiso = |r: &std::io::Result<String>| matches!(r, Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied);
    if sin_permiso(&huella) || sin_permiso(&codigo) {
        return Err(format!("No se puede leer {}: ejecútalo con sudo.", c.datos.display()));
    }
    let Ok(huella) = huella else {
        return Err(format!(
            "No hay datos del servidor en {}: ¿está en marcha? (systemctl status resguardo-server; registro: journalctl -u resguardo-server)",
            c.datos.display()
        ));
    };
    match dominio_guardado(&c.datos).or(c.dominio.clone()) {
        Some(d) => {
            println!("Consola: https://{d}/ (certificado público; en la red local también https://{}:{}/)", host_consola(c), c.escuchar.port());
            println!("Huella de la autoridad TLS propia (la que fijan los agentes, en agentes.{d}):");
        }
        None => {
            println!("Consola: https://{}:{}/", host_consola(c), c.escuchar.port());
            println!("Huella de la autoridad TLS (la del certificado «{}» en el navegador):", identidad::NOMBRE_CA);
        }
    }
    println!("  {}", huella.trim());
    if c.datos.join("identidad.key").is_file() {
        if let Ok(k) = identidad::identidad(&c.datos) {
            println!("Identidad del servidor (compárala con la de la consola: «Ajustes», «Identidad»):");
            println!("  {}", identidad_legible(&identidad::publica(&k)));
        }
    }
    match codigo.map(|s| s.trim().to_string()) {
        Ok(codigo) if !codigo.is_empty() => {
            println!("Código de primer arranque (para crear la cuenta de propietario):");
            println!("  {codigo}");
        }
        _ => println!("No hay código de primer arranque: este servidor ya tiene la cuenta de propietario."),
    }
    Ok(())
}

/// El dominio del certificado público que ya tiene el servidor (`<datos>/acme/<dominio>.crt`).
fn dominio_guardado(datos: &std::path::Path) -> Option<String> {
    let mut nombres: Vec<String> = std::fs::read_dir(resguardo_servidor::acme::carpeta(datos))
        .ok()?
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.strip_suffix(".crt").map(str::to_string))
        .collect();
    nombres.sort();
    nombres.pop()
}

/// La identidad (Ed25519, base64) como la enseña la consola: hex en grupos de 4.
fn identidad_legible(b64: &str) -> String {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).unwrap_or_default();
    let hex: String = bytes.iter().map(|b| format!("{b:02X}")).collect();
    hex.as_bytes().chunks(4).map(|c| String::from_utf8_lossy(c).to_string()).collect::<Vec<_>>().join(" ")
}

/// `hacer-respaldo`: una copia de la consola ahora (con el servidor en marcha o no).
fn hacer_respaldo(c: &Config) -> Result<(), String> {
    if !c.datos.join("control.db").is_file() {
        return Err(format!("No hay datos del servidor en {}.", c.datos.display()));
    }
    let u = resguardo_servidor::respaldo::hacer(&c.datos, &identidad::identidad(&c.datos)?, "manual")?;
    println!("{} {}", u.mensaje, resguardo_servidor::respaldo::carpeta(&c.datos).join(u.archivo.unwrap_or_default()).display());
    Ok(())
}

/// La clave de respaldo: de RESGUARDO_CLAVE_RESPALDO o preguntada (sin verla en Linux).
fn pedir_clave_respaldo() -> Result<String, String> {
    if let Some(c) = entorno("RESGUARDO_CLAVE_RESPALDO") {
        return Ok(c);
    }
    use std::io::Write;
    print!("Clave de respaldo de la consola (la del kit «Copia de la consola»): ");
    let _ = std::io::stdout().flush();
    #[cfg(unix)]
    let oculta = std::process::Command::new(stty()).arg("-echo").stdin(std::process::Stdio::inherit()).status().is_ok_and(|s| s.success());
    let mut linea = String::new();
    let r = std::io::stdin().read_line(&mut linea);
    #[cfg(unix)]
    if oculta {
        let _ = std::process::Command::new(stty()).arg("echo").stdin(std::process::Stdio::inherit()).status();
        println!();
    }
    r.map_err(|e| e.to_string())?;
    let clave = linea.trim_end_matches(['\r', '\n']).to_string();
    if clave.is_empty() {
        return Err("Sin clave no se puede restaurar.".into());
    }
    Ok(clave)
}

/// `stty` por su ruta (nunca del PATH: se ejecuta como root).
#[cfg(unix)]
fn stty() -> &'static str {
    ["/usr/bin/stty", "/bin/stty"].into_iter().find(|p| std::path::Path::new(p).is_file()).unwrap_or("/bin/stty")
}

/// ¿Se restaura una copia con esta identidad? Con `--confiar-en`, solo si es esa
/// (la huella del kit). Sin él, si es la del servidor que ya hay en la carpeta de
/// datos; si no, lo confirma la persona (`preguntar`). Nunca sin confirmar.
fn confiar_en_identidad(
    identidad: &str,
    confiar_en: Option<&str>,
    actual: Option<&str>,
    preguntar: impl FnOnce() -> Result<bool, String>,
) -> Result<(), String> {
    use resguardo_protocolo::respaldo_consola::{coincide_huella, huella};
    if let Some(h) = confiar_en {
        return if coincide_huella(identidad, h) {
            Ok(())
        } else {
            Err(format!("La copia es de otro servidor: su identidad es {}, no {h}. No se restaura.", huella(identidad)))
        };
    }
    if actual == Some(identidad) {
        println!("Es la identidad del servidor que ya hay en la carpeta de datos.");
        return Ok(());
    }
    if let Some(a) = actual {
        println!("AVISO: el servidor que hay ahora en la carpeta de datos tiene otra identidad ({}).", huella(a));
    }
    if preguntar()? {
        Ok(())
    } else {
        Err("No se restaura: la identidad no es la del kit.".into())
    }
}

/// Pregunta en la terminal si la identidad es la del kit (sin terminal, no).
fn preguntar_identidad() -> Result<bool, String> {
    use std::io::{IsTerminal, Write};
    if !std::io::stdin().is_terminal() {
        return Err("Sin terminal no se puede confirmar la identidad: repite con --confiar-en <huella del kit>.".into());
    }
    print!("¿Es la identidad de tu kit «Copia de la consola»? Escribe «si» para seguir: ");
    let _ = std::io::stdout().flush();
    let mut linea = String::new();
    std::io::stdin().read_line(&mut linea).map_err(|e| e.to_string())?;
    Ok(matches!(linea.trim().to_lowercase().as_str(), "si" | "sí" | "s"))
}

/// `restaurar-respaldo`: deja la carpeta de datos como estaba en la copia, con su identidad.
fn restaurar_respaldo(c: &Config, archivo: &std::path::Path, reemplazar: bool, confiar_en: Option<&str>) -> Result<(), String> {
    use resguardo_protocolo::respaldo_consola as rc;
    // Primero, sin la clave: la estructura, la firma y de qué servidor es.
    let f = std::fs::File::open(archivo).map_err(|e| format!("No se pudo abrir {}: {e}", archivo.display()))?;
    let comprobada = rc::comprobar(std::io::BufReader::new(f))?;
    let cab = &comprobada.cabecera;
    println!("Copia del {} (Resguardo Server {}).", cab.creado, cab.version);
    println!("Identidad del servidor que la hizo: {}", rc::huella(&cab.identidad));
    if comprobada.firmada {
        println!("Firma de la copia: correcta (la hizo ese servidor).");
    } else {
        println!("AVISO: esta copia no lleva firma (la hizo un Resguardo Server anterior a 0.7.11): no se puede comprobar qué servidor la hizo.");
        println!("       Compara la identidad con la de tu kit antes de seguir.");
    }
    let actual = c.datos.join("identidad.key").is_file().then(|| identidad::identidad(&c.datos).ok().map(|k| identidad::publica(&k))).flatten();
    confiar_en_identidad(&cab.identidad, confiar_en, actual.as_deref(), preguntar_identidad)?;
    let clave = pedir_clave_respaldo()?;
    println!("Descifrando (tarda unos segundos)…");
    let r = resguardo_servidor::respaldo::restaurar(archivo, &clave, &c.datos, reemplazar, &cab.identidad)?;
    println!("Consola restaurada en {} ({} archivos, copia del {}, Resguardo Server {}).", c.datos.display(), r.archivos, r.creado, r.version);
    println!("Identidad del servidor (la que reconocen los equipos): {} ({})", rc::huella(&r.identidad), r.identidad);
    if let Some(a) = r.antes {
        println!("Lo que había antes quedó en {}.", a.display());
    }
    println!("Arranca el servicio con la misma dirección de siempre: los equipos vuelven solos.");
    if cfg!(unix) {
        println!("  sudo systemctl start resguardo-server");
    } else {
        println!("  sc start ResguardoServer   (o reinicia el servicio «Resguardo Server»)");
    }
    Ok(())
}

/// El nombre o la IP con que abrir la consola desde otro equipo.
fn host_consola(c: &Config) -> String {
    if !c.escuchar.ip().is_unspecified() {
        return c.escuchar.ip().to_string();
    }
    if cfg!(windows) {
        if let Ok(n) = std::env::var("COMPUTERNAME") {
            return n.to_lowercase();
        }
    }
    // En Linux (servicio sin pantalla, a menudo sin DNS en la red local): su IP.
    identidad::ip_principal().map(|ip| ip.to_string()).or_else(identidad::nombre_equipo).unwrap_or_else(|| "localhost".into())
}

/// Prepara y sirve (hasta que se pare el proceso). `salida`: cada línea del registro.
pub fn arrancar(c: Config, salida: &dyn Fn(&str)) -> Result<(), String> {
    // Una versión nueva: copia de la consola antes de abrir (y quizá cambiar) la base de datos.
    match resguardo_servidor::respaldo::antes_de_actualizar(&c.datos) {
        Some(Ok(u)) => salida(&format!("Versión nueva: {}", u.mensaje)),
        Some(Err(e)) => salida(&format!("Versión nueva: no se pudo hacer la copia de la consola ({e}).")),
        None => {}
    }
    let huella = identidad::preparar_tls(&c.datos, &c.nombres)?;
    // El instalador del agente que trae Resguardo Server para Windows (agente/Resguardo-Agente-setup.exe).
    let instalador_agente = c.instalador_agente.clone().or_else(|| {
        let p = std::env::current_exe().ok()?.parent()?.join("agente").join("Resguardo-Agente-setup.exe");
        p.is_file().then_some(p)
    });
    let opciones = Opciones {
        max_relevo: c.max_mb * 1024 * 1024,
        https: !c.sin_tls || c.proxy,
        consola: c.consola.clone(),
        instalador_agente,
        puerto: Some(c.escuchar.port()),
        proxy: c.proxy,
        url_agentes: c.url_agentes.clone(),
        publico: c.publico,
        ..Default::default()
    };
    let st = preparar(&c.datos, opciones)?;
    salida(&format!("Resguardo Server {} · datos en {}", env!("CARGO_PKG_VERSION"), c.datos.display()));
    salida(&format!("Identidad del servidor (la fijan los agentes): {}", st.identidad_pub));
    salida(&format!("Huella de su autoridad TLS: {huella}"));
    let archivo = archivo_codigo(&c.datos);
    match resguardo_servidor::api::preparar_codigo_arranque(&st)? {
        Some(codigo) => {
            salida("");
            let esquema = if c.sin_tls { "http" } else { "https" };
            let url = match &c.dominio {
                Some(d) if c.escuchar.port() == 443 => format!("https://{d}/"),
                Some(d) => format!("https://{d}:{}/", c.escuchar.port()),
                None => format!("{esquema}://{}:{}/", host_consola(&c), c.escuchar.port()),
            };
            salida(&format!("  Primer arranque: abre {url} y usa este código para crear la cuenta de propietario:"));
            salida(&format!("      {codigo}"));
            if cfg!(unix) {
                salida("  (también lo muestra «sudo resguardo-server codigo-inicial»)");
            }
            salida("");
            let fin = if cfg!(windows) { "\r\n" } else { "\n" };
            let _ = identidad::escribir_privado(&archivo, format!("{codigo}{fin}").as_bytes());
        }
        None => {
            let _ = std::fs::remove_file(&archivo);
        }
    }
    let dir = c.datos.join("tls");
    let tls = match (&c.dominio, c.sin_tls) {
        (_, true) => Tls::Ninguno,
        (Some(d), false) => {
            let mut acme = resguardo_servidor::acme::ConfigAcme::nueva(d, c.acme_correo.clone(), c.acme_pruebas);
            if let Some(url) = &c.acme_directorio {
                acme.directorio = url.clone();
            }
            let autoridad = if c.acme_pruebas {
                "Let's Encrypt de pruebas"
            } else if c.acme_directorio.is_some() {
                "otra autoridad ACME"
            } else {
                "Let's Encrypt"
            };
            salida(&format!(
                "Consola en internet: https://{d} con certificado público ({autoridad}); agentes y otras consolas: {}. Retos HTTP-01 en {}.",
                c.url_agentes.as_deref().unwrap_or(""),
                c.acme_http
            ));
            Tls::Publico { cert: dir.join("servidor.crt"), clave: dir.join("servidor.key"), acme, http: c.acme_http }
        }
        (None, false) => Tls::Propio { cert: c.cert.unwrap_or_else(|| dir.join("servidor.crt")), clave: c.clave.unwrap_or_else(|| dir.join("servidor.key")) },
    };
    if c.proxy {
        salida("Detrás de un proxy: la IP de cada petición desde 127.0.0.1 es la última de X-Forwarded-For.");
    }
    salida(&format!("Escuchando en {}://{}", if c.sin_tls { "http" } else { "https" }, c.escuchar));
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().map_err(|e| e.to_string())?;
    rt.block_on(servir(st, c.escuchar, tls))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn restaurar_respaldo_con_sus_opciones() {
        let (_, modo) = leer_args(&args(&["restaurar-respaldo", "copia.resguardo-consola", "--reemplazar"])).unwrap().unwrap();
        assert!(
            matches!(modo, Modo::RestaurarRespaldo { ref archivo, reemplazar: true, confiar_en: None } if archivo == &PathBuf::from("copia.resguardo-consola"))
        );
        let (_, modo) = leer_args(&args(&["--datos", "/srv/rs", "restaurar-respaldo", "x", "--confiar-en", "AB:CD"])).unwrap().unwrap();
        assert!(matches!(modo, Modo::RestaurarRespaldo { reemplazar: false, confiar_en: Some(ref h), .. } if h == "AB:CD"));
        assert!(leer_args(&args(&["restaurar-respaldo", "x", "--confiar-en"])).is_err(), "falta la huella");
        assert!(leer_args(&args(&["restaurar-respaldo"])).is_err(), "falta el archivo");
        assert!(matches!(leer_args(&args(&["hacer-respaldo"])).unwrap().unwrap().1, Modo::HacerRespaldo));
    }

    #[test]
    fn la_identidad_de_la_copia_se_confirma_siempre() {
        use resguardo_protocolo::respaldo_consola::huella;
        let id = identidad::publica(&ed25519_dalek::SigningKey::from_bytes(&[3u8; 32]));
        let otra = identidad::publica(&ed25519_dalek::SigningKey::from_bytes(&[4u8; 32]));
        let nunca = || -> Result<bool, String> { panic!("no se pregunta") };
        // Con la huella del kit: solo si coincide (aunque en la carpeta haya otra).
        assert!(confiar_en_identidad(&id, Some(&huella(&id)), Some(&otra), nunca).is_ok());
        assert!(confiar_en_identidad(&id, Some(&huella(&otra)), Some(&id), nunca).unwrap_err().contains("otro servidor"));
        // Sin ella: la del servidor de la carpeta vale; otra cosa, se pregunta.
        assert!(confiar_en_identidad(&id, None, Some(&id), nunca).is_ok());
        assert!(confiar_en_identidad(&id, None, Some(&otra), || Ok(false)).is_err());
        assert!(confiar_en_identidad(&id, None, None, || Ok(true)).is_ok());
        assert!(confiar_en_identidad(&id, None, None, || Err("sin terminal".into())).is_err());
    }

    #[test]
    fn opciones_de_la_consola_en_internet() {
        let (c, _) = leer_args(&args(&["--dominio", "Consola.Ejemplo.com", "--escuchar", "0.0.0.0:443", "--acme-correo", "ana@ejemplo.com"])).unwrap().unwrap();
        assert_eq!(c.dominio.as_deref(), Some("consola.ejemplo.com"));
        assert_eq!(c.dominio_agentes.as_deref(), Some("agentes.consola.ejemplo.com"));
        assert_eq!(c.url_agentes.as_deref(), Some("https://agentes.consola.ejemplo.com"));
        assert!(c.publico);
        assert!(c.nombres.contains(&"agentes.consola.ejemplo.com".to_string()) && c.nombres.contains(&"consola.ejemplo.com".to_string()));
        // En otro puerto, con él; y con su propio nombre para los agentes.
        let (c, _) = leer_args(&args(&["--dominio", "x.duckdns.org", "--dominio-agentes", "ag.x.duckdns.org"])).unwrap().unwrap();
        assert_eq!(c.url_agentes.as_deref(), Some("https://ag.x.duckdns.org:8443"));
        // Lo que no tiene sentido, no.
        let malos: [&[&str]; 8] = [
            &["--dominio", "203.0.113.5"],
            &["--dominio", "x.ejemplo.com", "--sin-tls", "--detras-de-proxy"],
            &["--dominio", "x.ejemplo.com", "--cert", "a", "--clave", "b"],
            &["--dominio", "x.ejemplo.com", "--dominio-agentes", "x.ejemplo.com"],
            &["--dominio", "x.ejemplo.com", "--escuchar", "0.0.0.0:80"],
            &["--acme-correo", "ana@ejemplo.com"],
            &["--dominio", "x.ejemplo.com", "--acme-correo", "no es un correo"],
            &["--url-agentes", "http://x.ejemplo.com"],
        ];
        for malo in malos {
            assert!(leer_args(&args(malo)).is_err(), "{malo:?}");
        }
        // Detrás de un proxy: la dirección de los agentes a mano y las cuotas de internet.
        let a = args(&["--sin-tls", "--detras-de-proxy", "--escuchar", "127.0.0.1:8080", "--url-agentes", "https://consola.ejemplo.com:8443/", "--publico"]);
        let (c, _) = leer_args(&a).unwrap().unwrap();
        assert!(c.proxy && c.publico && c.dominio.is_none());
        assert_eq!(c.url_agentes.as_deref(), Some("https://consola.ejemplo.com:8443"));
        assert_eq!(identidad_legible("AAECAw=="), "0001 0203");
    }

    #[test]
    fn codigo_inicial_con_su_carpeta() {
        let (c, modo) = leer_args(&args(&["codigo-inicial", "--datos", "/srv/rs"])).unwrap().unwrap();
        assert!(matches!(modo, Modo::CodigoInicial));
        assert_eq!(c.datos, PathBuf::from("/srv/rs"));
    }

    #[test]
    fn codigo_inicial_lee_lo_del_servidor() {
        let dir = tempfile::tempdir().unwrap();
        let (mut c, _) = leer_args(&args(&["codigo-inicial"])).unwrap().unwrap();
        c.datos = dir.path().to_path_buf();
        // Sin datos: no hay servidor que consultar.
        assert!(codigo_inicial(&c).is_err());
        std::fs::create_dir_all(dir.path().join("tls")).unwrap();
        std::fs::write(dir.path().join("tls").join("ca.huella"), "AA:BB\n").unwrap();
        assert!(codigo_inicial(&c).is_ok());
        identidad::escribir_privado(&archivo_codigo(&c.datos), b"ABCD-1234\n").unwrap();
        assert!(codigo_inicial(&c).is_ok());
    }
}
