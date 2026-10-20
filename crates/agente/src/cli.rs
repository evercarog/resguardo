//! Línea de órdenes del agente, en español (docs/plataforma.md, §3.5):
//! para administrar el equipo en local, también sin servidor.
//!
//! ```text
//! resguardo-agente estado
//! resguardo-agente copias
//! resguardo-agente copiar-ahora <repo> <copia>
//! resguardo-agente versiones <repo>
//! resguardo-agente restaurar <repo> <versión> <ruta> --destino <carpeta>
//! resguardo-agente registro [n]
//! resguardo-agente kit <repo>
//! resguardo-agente vincular <código> --servidor <url> [--nombre <nombre>] [--empezar-de-cero]
//! resguardo-agente desvincular
//! resguardo-agente guardar-copias estado|activar|anadir|quitar|desactivar …
//! resguardo-agente servidores
//! resguardo-agente consolas [quitar <n.º | dirección | huella>]
//! resguardo-agente ordenes [aprobar <n.º>]
//! resguardo-agente parar [<n.º> [--hasta HH:MM | --en <horas> | --a-mano]]
//! resguardo-agente config exportar|importar <archivo>
//! resguardo-agente repositorio crear <id> --nombre … --tipo … --donde … [--usuario …]
//! ```

use std::time::Duration;

pub const ORDENES: &[&str] = &[
    "estado",
    "copias",
    "copiar-ahora",
    "versiones",
    "restaurar",
    "registro",
    "kit",
    "vincular",
    "leer-instalador",
    "desvincular",
    "guardar-copias",
    "nube",
    "servidores",
    "consolas",
    "ordenes",
    "parar",
    "config",
    "repositorio",
    "ayuda",
    "version",
    "actualizaciones",
];

/// Otras formas de pedir la ayuda o la versión (`--help`, `-h`, `--version`…).
pub fn alias(a: &str) -> Option<&'static str> {
    match a {
        "--help" | "-h" | "help" | "/?" | "--ayuda" => Some("ayuda"),
        "--version" | "-V" | "versión" => Some("version"),
        "--leer-instalador" => Some("leer-instalador"),
        _ => None,
    }
}

// G4: «Consolas y órdenes» aparte del modo local; «almacén», no «Servidor de copias» ni
// «rest-server»; sin mayúsculas de énfasis; el horario del espejo dicho como en la consola.
const AYUDA: &str = "Resguardo Agente: órdenes (ejecútalas como administrador; en Linux, con sudo)

  estado                                  Resumen: consola, copias (última y próxima), almacén,
                                          espejo, nubes y últimos errores
  version                                 La versión del agente
  actualizaciones                         La actualización automática: versión disponible, estado y motivo
  actualizaciones github si|no            Buscar también en las publicaciones de GitHub (por defecto, sí)
  copias                                  Todos los repositorios del equipo (también los vacíos), sus
                                          copias y sus copias derivadas
  copiar-ahora <repo> <copia>             Lanza una copia ya
  versiones <repo>                        Versiones guardadas en un repositorio
  restaurar <repo> <versión> <ruta> --destino <carpeta>
                                          Restaura una carpeta o archivo de una versión (nunca sobrescribe)
  registro [n]                            Las últimas n líneas del registro (50 por defecto)
  kit <repo> [<derivada>]                 Datos para recuperar las copias (incluye la contraseña) de un
                                          repositorio o de una de sus copias derivadas

  Consolas y órdenes:
  vincular <código> --servidor <url> [--nombre <nombre>]
                                          Vincula el equipo con una consola (Resguardo Server). Si ya tenía
                                          clave de administración (p. ej. se perdió la consola), conserva
                                          todo y espera el alta de la consola nueva con esa misma clave.
                                          Con --huella-ca AB:CD:…, solo si la autoridad TLS es esa
  vincular <código> --servidor <url> --empezar-de-cero
                                          Olvida la consola, la clave y las copias que gestionaba
                                          (lo copiado se queda) y lo vincula como equipo nuevo
  vincular --instalador <ruta>            Vincula con los datos de un instalador «listo»
  leer-instalador <ruta>                  ¿Trae ese instalador datos para vincular?
  desvincular                             Deja la consola y sigue copiando en modo local
  servidores                              Consola actual, cambio en curso y consolas de respaldo
  consolas                                Las consolas que gestionan este equipo (pueden ser varias a la vez)
  consolas quitar <n.º | dirección | huella>
                                          Deja de conectarse a una consola (p. ej. una que ya no existe);
                                          las demás siguen igual
  ordenes                                 Órdenes que esperan su turno en este equipo (de todas sus consolas)
  ordenes aprobar <n.º>                   La aplica ya, sin esperar a su hora: pide la clave de administración
                                          (sin que se vea al escribirla) y queda en el historial de todas
                                          las consolas, con un aviso en todas
  parar                                   Lo que está en marcha ahora (copias, verificaciones, espejos…)
  parar <n.º>                             Lo para ya: pide la clave de administración (sin que se vea)
  parar <n.º> --hasta 22:00 | --en 2 | --a-mano
                                          Pausa una copia: se para ya y sigue sola a esa hora (o dentro
                                          de esas horas); con --a-mano, cuando se pida «Copiar ahora»

  Sin consola (modo local):
  config exportar <archivo>               La configuración del equipo, sin contraseñas (JSON)
  config importar <archivo>               Aplica las copias de un archivo (solo en modo local)
  repositorio crear <id> --nombre <n> --tipo local|rest|s3|b2|sftp --donde <dónde>
                    [--usuario <u>] [--nombre-destino <n>]
                                          Crea un repositorio (solo en modo local). --nombre-destino es el
                                          nombre de su destino (por defecto, «Destino»). Pide la contraseña
                                          (y el secreto del destino, si hay usuario) por la entrada estándar

  Guardar las copias de otros equipos (almacén, de solo añadir):
  guardar-copias estado
  guardar-copias activar --carpeta <ruta> [--puerto 8000] [--toda-la-red]
                                          Por defecto solo acepta la red local
  guardar-copias anadir <equipo>          Usuario y contraseña para un equipo que copia aquí
  guardar-copias quitar <equipo>          Ya no puede entrar (sus copias se quedan)
  guardar-copias desactivar               Deja de guardar copias (las copias se quedan)
  guardar-copias espejo [--carpeta <ruta>]... [--nube <nombre> --carpeta-nube <carpeta>]...
                        [--hora 02:00] [--limite-kib <KiB/s>]
                                          Cada día a las 02:00 (o a la hora de --hora), copia todo lo
                                          guardado a otra carpeta (mejor en otro disco) y/o a una nube
                                          conectada; nunca borra allí
  guardar-copias espejo --quitar          Deja de hacer el espejo (lo copiado se queda)
  nube conectar dropbox|drive --nombre <nombre>
                                          Conecta una nube para el espejo (abre el navegador; el permiso
                                          se guarda protegido solo en este equipo)
  nube lista                              Nubes conectadas en este equipo
  nube quitar <nombre>                    Olvida una nube (revoca también el permiso en su web)
";

/// `actualizaciones [github si|no]` (docs/actualizaciones.md).
fn actualizaciones(args: &[String]) -> Result<(), String> {
    use crate::actualizacion as a;
    if args.get(1).map(String::as_str) == Some("github") {
        crate::agent::require_admin()?;
        let github = match args.get(2).map(String::as_str) {
            Some("si" | "sí") => true,
            Some("no") => false,
            _ => return Err("Usa «actualizaciones github si» o «actualizaciones github no».".into()),
        };
        a::poner_ajustes(&a::Ajustes { github })?;
        println!("{}", if github { "Buscará también en GitHub." } else { "Solo buscará en sus consolas." });
        return Ok(());
    }
    print!("{}", texto_actualizaciones(&a::informe(), crate::version_programa(), a::ajustes().github));
    Ok(())
}

/// El estado de la actualización automática, en palabras.
pub fn texto_actualizaciones(i: &serde_json::Value, version: &str, github: bool) -> String {
    let t = |k: &str| i[k].as_str().unwrap_or("").to_string();
    let mut s = format!("Versión instalada: {version}\n");
    let estado = match t("estado").as_str() {
        "desactivada" if t("motivo") == "sin_actualizaciones" => "Sin actualización automática (este programa se compiló así).".to_string(),
        "desactivada" => "Sin actualización automática (este programa no tiene llave de publicación).".to_string(),
        "al_dia" => "Al día.".to_string(),
        "pausada" => format!("En pausa desde la consola (disponible: {}).", t("version_disponible")),
        "pendiente" => format!(
            "Pendiente: la {} ({}{}).",
            t("version_disponible"),
            motivo_actualizacion(&t("motivo")),
            if t("hasta").is_empty() { String::new() } else { format!(", hasta {}", t("hasta")) }
        ),
        "descargando" => format!("Descargando la {}.", t("version_objetivo")),
        "actualizando" => format!("Actualizando a la {}.", t("version_objetivo")),
        "actualizada" => format!("Actualizada a la {}.", t("version_objetivo")),
        "vuelta_atras" => format!("La {} no estuvo sana y volvió a la anterior. {}", t("version_fallida"), t("mensaje")),
        "fallida" => format!("No se pudo actualizar a la {}. {}", t("version_objetivo"), t("mensaje")),
        otro => otro.to_string(),
    };
    s.push_str(&format!("Estado: {}\n", estado.trim()));
    if !t("anillo").is_empty() {
        s.push_str(&format!("Anillo: {} · modo: {}\n", t("anillo"), t("modo")));
    }
    if !t("ultima_busqueda").is_empty() {
        s.push_str(&format!("Última búsqueda: {} ({})\n", t("ultima_busqueda"), t("origen")));
    }
    s.push_str(&format!("GitHub: {}\n", if github { "sí (si las consolas no la tienen)" } else { "no" }));
    s
}

/// Por qué espera, en palabras (lo mismo que dice la consola).
pub fn motivo_actualizacion(m: &str) -> &'static str {
    match m {
        "sin_paquete" => "no hay paquete para este sistema",
        "necesita_intermedia" => "antes hay que instalar una versión intermedia",
        "espera_aprobacion" => "esperando a que la apruebes en la consola",
        "retenida" => "retenida: falló en algún equipo",
        "espera_anillo" => "espera su turno (anillo general)",
        "espera_ventana" => "fuera de la ventana de mantenimiento",
        "ventanas_sin_coincidir" => "las ventanas de sus consolas no coinciden nunca",
        "en_marcha" => "hay una copia o una restauración en marcha",
        "almacen_sin_ventana" => "es un almacén: necesita una ventana de mantenimiento o «Actualizar ahora»",
        "sin_consola" => "ninguna consola respondió",
        _ => "esperando",
    }
}

/// `vincular`. Con `--result <archivo>` (el instalador) deja **siempre** el resultado en ese
/// archivo, pase lo que pase: «ok», el código de comprobación y el servidor, o «error» y el
/// motivo (una cosa por línea), y el motivo también en el registro del agente (14ee).
fn vincular(args: &[String]) -> Result<(), String> {
    let Some(archivo) = opcion(args, "--result") else { return vincular_y_contar(args) };
    // 14ee: la carpeta del agente, preparada aquí con sus permisos (como el servicio). El
    // instalador lanza `vincular` justo después de arrancar el servicio, que aún puede no
    // haberla creado: sin ella, el motivo de un fallo no quedaba en ningún registro.
    preparar_carpeta(Duration::from_secs(20));
    let r = vincular_con_resultado(args);
    let texto = match &r {
        Ok(v) => format!("ok\r\n{}\r\n{}\r\n", v.sas, v.servidor),
        Err(e) => {
            crate::agent::log(&format!("No se pudo vincular: {e}"));
            format!("error\r\n{}\r\n", e.replace(['\r', '\n'], " "))
        }
    };
    std::fs::write(&archivo, &texto).map_err(|e| {
        crate::agent::log(&format!("Vincular: no se pudo dejar el resultado para el instalador: {e}"));
        e.to_string()
    })?;
    r.map(|_| ()).map_err(|_| "No se pudo vincular.".into())
}

/// La carpeta del agente lista (con sus permisos) antes de vincular, con unos reintentos: el
/// servicio recién arrancado puede estar preparándola a la vez. Sin administrador no se toca
/// (`vincular` lo dirá después).
fn preparar_carpeta(plazo: Duration) {
    if crate::agent::require_admin().is_err() {
        return;
    }
    let hasta = std::time::Instant::now() + plazo;
    loop {
        match crate::agent::prepare_dir() {
            Ok(()) => return,
            Err(e) if std::time::Instant::now() >= hasta => {
                // Puede que el registro aún no se pueda escribir: el motivo va en el resultado.
                crate::agent::log(&format!("Vincular: no se pudo preparar la carpeta del agente: {e}"));
                return;
            }
            Err(_) => std::thread::sleep(Duration::from_millis(500)),
        }
    }
}

/// Los datos de `vincular` (del instalador «listo» o de la línea) y la vinculación.
fn vincular_con_resultado(args: &[String]) -> Result<crate::servidor_v2::Vinculado, String> {
    // v1.17: con --instalador <ruta>, todo sale de la cola del instalador (servidor,
    // huella de su autoridad TLS, cliente, nombre y código de un solo uso).
    let preparado = match opcion(args, "--instalador") {
        Some(r) => Some(leer_instalador(&r)?.ok_or("Este instalador no trae datos para vincular: usa un código de la consola.")?),
        None => None,
    };
    let (codigo, url, nombre, huella, cliente) = match &preparado {
        Some(d) => (d.codigo.clone(), d.servidor.clone(), d.nombre.clone(), Some(d.huella_ca.clone()), Some(d.cliente.clone())),
        None => (
            arg(args, 1)?.to_string(),
            opcion(args, "--servidor").ok_or("Falta --servidor <url> (p. ej. https://192.168.1.20:8443).")?,
            opcion(args, "--nombre").unwrap_or_else(crate::web::default_device_name),
            opcion(args, "--huella-ca"),
            None,
        ),
    };
    // 14ee: --esperar <segundos> (el instalador): seguir intentando llegar al servidor.
    let esperar = match opcion(args, "--esperar") {
        Some(s) => Duration::from_secs(s.parse::<u64>().map_err(|_| "--esperar va en segundos (p. ej. 60).")?.min(300)),
        None => Duration::ZERO,
    };
    let esperado = crate::servidor_v2::Esperado { huella_ca: huella, cliente, esperar };
    if args.iter().any(|a| a == "--empezar-de-cero") {
        crate::servidor_v2::empezar_de_cero()?;
    }
    crate::agent::log(&format!("Vinculando este equipo con {url}…"));
    crate::servidor_v2::vincular_con(&url, &codigo, &nombre, &esperado)
}

/// `vincular` en la terminal: lo que hay que comprobar en la consola.
fn vincular_y_contar(args: &[String]) -> Result<(), String> {
    let v = vincular_con_resultado(args)?;
    println!("Vinculado con {}.", v.servidor);
    println!("Huella de la autoridad TLS del servidor: {}", v.huella_ca);
    println!();
    println!("  Número de comprobación: {}", v.sas);
    println!();
    println!("Comprueba en la consola que ve el mismo número antes de confirmar.");
    if !v.sas_v3 {
        println!("La consola es antigua y el número no cubre su certificado: comprueba también que la consola");
        println!("enseña esta misma huella de la autoridad TLS. Si no es la misma, no confirmes.");
    }
    if v.espera_alta {
        println!();
        println!("Este equipo ya tenía clave de administración: conserva su configuración y sus copias.");
        println!("La consola nueva tiene que darlo de alta con esa misma clave; hasta entonces no recibe nada.");
        println!("Si es otro cliente (otra clave), repite con --empezar-de-cero.");
    }
    Ok(())
}

/// ¿Se enganchó a la consola de la terminal que lo lanzó? (El programa es de ventana: `cmd` y
/// PowerShell no lo esperan y ya enseñan su aviso mientras escribe.)
static ADJUNTADA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Las ventanas de Windows sin consola: la de la terminal desde la que se lanza.
fn adjuntar_consola() {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
        if AttachConsole(ATTACH_PARENT_PROCESS) != 0 {
            ADJUNTADA.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// 15q: al terminar, que la terminal vuelva a enseñar su aviso **detrás** de lo escrito. `cmd`
/// no espera a un programa de ventana: su aviso sale antes que la salida. Si se escribió en esa
/// consola (sin redirigir), se le manda una pulsación de Intro, que pinta el aviso de nuevo
/// debajo, y se suelta la consola. (La forma limpia sería un `resguardo-agente-cli.exe` de
/// consola, que el instalador tendría que llevar: anotado.)
fn soltar_consola() {
    use std::io::Write;
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    #[cfg(windows)]
    if ADJUNTADA.load(std::sync::atomic::Ordering::Relaxed) {
        use windows_sys::Win32::System::Console::{
            FreeConsole, GetConsoleMode, GetStdHandle, WriteConsoleInputW, INPUT_RECORD, INPUT_RECORD_0, KEY_EVENT, KEY_EVENT_RECORD, KEY_EVENT_RECORD_0,
            STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
        };
        // SAFETY: handles de la consola propia; estructuras locales.
        unsafe {
            let (entrada, salida) = (GetStdHandle(STD_INPUT_HANDLE), GetStdHandle(STD_OUTPUT_HANDLE));
            let mut m = 0u32;
            let en_consola = !salida.is_null() && GetConsoleMode(salida, &mut m) != 0 && !entrada.is_null() && GetConsoleMode(entrada, &mut m) != 0;
            if en_consola {
                const VK_RETURN: u16 = 0x0D;
                let tecla = |abajo: bool| INPUT_RECORD {
                    EventType: KEY_EVENT as u16,
                    Event: INPUT_RECORD_0 {
                        KeyEvent: KEY_EVENT_RECORD {
                            bKeyDown: i32::from(abajo),
                            wRepeatCount: 1,
                            wVirtualKeyCode: VK_RETURN,
                            wVirtualScanCode: 0x1C,
                            uChar: KEY_EVENT_RECORD_0 { UnicodeChar: u16::from(b'\r') },
                            dwControlKeyState: 0,
                        },
                    },
                };
                let teclas = [tecla(true), tecla(false)];
                let mut escritas = 0u32;
                WriteConsoleInputW(entrada, teclas.as_ptr(), 2, &mut escritas);
            }
            FreeConsole();
        }
    }
}

fn arg(args: &[String], i: usize) -> Result<&str, String> {
    args.get(i).map(String::as_str).ok_or_else(|| "Faltan datos. Usa «resguardo-agente ayuda».".to_string())
}

/// La cola «listo para vincular» del final de un instalador (v1.17), si la trae.
fn leer_instalador(ruta: &str) -> Result<Option<resguardo_protocolo::instalador::DatosInstalador>, String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(ruta).map_err(|e| format!("No se pudo abrir el instalador: {e}"))?;
    let largo = f.metadata().map_err(|e| e.to_string())?.len();
    let leer = largo.min(resguardo_protocolo::instalador::LEER_DEL_FINAL as u64);
    f.seek(SeekFrom::Start(largo - leer)).map_err(|e| e.to_string())?;
    let mut fin = Vec::with_capacity(leer as usize);
    f.take(leer).read_to_end(&mut fin).map_err(|e| e.to_string())?;
    resguardo_protocolo::instalador::leer_cola(&fin)
}

fn opcion(args: &[String], nombre: &str) -> Option<String> {
    args.iter().position(|a| a == nombre).and_then(|i| args.get(i + 1)).cloned()
}

/// Todos los valores de una opción que se puede repetir.
fn opciones(args: &[String], nombre: &str) -> Vec<String> {
    args.windows(2).filter(|w| w[0] == nombre).map(|w| w[1].clone()).collect()
}

/// `nube conectar|lista|quitar`: las nubes para el espejo (nube.rs).
fn nube(args: &[String]) -> Result<(), String> {
    match args.get(1).map(String::as_str) {
        None | Some("lista") => {
            let l = crate::nube::lista();
            if l.is_empty() {
                println!("No hay ninguna nube conectada en este equipo.");
            }
            for n in l {
                let tipo = crate::nube::TIPOS.iter().find(|(t, _)| *t == n.tipo).map_or(n.tipo.as_str(), |(_, nombre)| nombre);
                println!("  {} · {tipo}", n.nombre);
            }
            Ok(())
        }
        Some("conectar") => {
            let nombre = opcion(args, "--nombre").ok_or("Falta --nombre <nombre> (por ejemplo, --nombre \"Dropbox Altamar\").")?;
            println!("{}", crate::nube::conectar(arg(args, 2)?, &nombre)?);
            Ok(())
        }
        Some("quitar") => {
            println!("{}", crate::nube::quitar(arg(args, 2)?)?);
            Ok(())
        }
        Some(otra) => Err(format!("Orden desconocida: nube {otra}. Usa «resguardo-agente ayuda».")),
    }
}

fn acceso(repo_id: &str) -> Result<(crate::agent::AgentRepo, crate::restic::Access), String> {
    let config = crate::agent::load_config();
    let repo =
        config.repos.into_iter().find(|r| r.id == repo_id).ok_or_else(|| format!("No hay ningún repositorio «{repo_id}» en este equipo (mira «copias»)."))?;
    let secretos = crate::agent::load_secrets()?;
    let s = secretos.get(repo_id).ok_or("Falta la contraseña de ese repositorio en el agente.")?;
    let access = crate::restic::Access {
        location: repo.location.clone(),
        password: s.password.clone(),
        rest_auth: repo.rest_username.clone().zip(s.rest_password.clone()),
        cacert: repo.cacert.clone(),
        env: s.env.clone(),
    };
    Ok((repo, access))
}

/// 15q: lo que hay en el equipo, para `copias`: **todos** los repositorios (también los vacíos,
/// los de solo lectura y los que no tienen copias), las copias y las copias derivadas, por separado.
#[derive(Debug, Default, PartialEq)]
pub struct Listado {
    pub repos: Vec<RepoListado>,
    pub copias: Vec<CopiaListada>,
    pub derivadas: Vec<DerivadaListada>,
}

#[derive(Debug, Default, PartialEq)]
pub struct RepoListado {
    pub id: String,
    pub nombre: String,
    /// El destino (su nombre) o la ubicación sin credenciales.
    pub donde: String,
    pub versiones: Option<u64>,
    pub solo_lectura: bool,
}

#[derive(Debug, Default, PartialEq)]
pub struct CopiaListada {
    pub id: String,
    pub nombre: String,
    pub repo: String,
    pub carpetas: usize,
}

#[derive(Debug, Default, PartialEq)]
pub struct DerivadaListada {
    pub repo: String,
    /// Su id (`externa`: la de cada día de antes).
    pub id: String,
    pub destino: String,
}

/// Junta lo de la configuración del agente y lo de la consola (repositorios gestionados).
fn listado() -> Listado {
    let config = crate::agent::load_config();
    let v = crate::servidor_v2::cargar();
    let versiones = |id: &str| crate::informe_v2::resumen_repo(id)["versiones"].as_u64();
    let mut l = Listado::default();
    if let Some(v) = &v {
        let destino =
            |id: &str| v.destinos.iter().find(|d| d.id == id).map(|d| format!("«{}»", d.nombre)).unwrap_or_else(|| "un destino que ya no está".into());
        for r in &v.repos_v2 {
            l.repos.push(RepoListado {
                id: r.id.clone(),
                nombre: r.nombre.clone(),
                donde: destino(&r.destino),
                versiones: versiones(&r.id),
                solo_lectura: r.solo_lectura,
            });
            if let Some(e) = &r.externa {
                l.derivadas.push(DerivadaListada { repo: r.id.clone(), id: "externa".into(), destino: destino(e["destino"].as_str().unwrap_or("")) });
            }
            for d in &r.derivadas {
                l.derivadas.push(DerivadaListada {
                    repo: r.id.clone(),
                    id: d["id"].as_str().unwrap_or("?").to_string(),
                    destino: destino(d["destino"].as_str().unwrap_or("")),
                });
            }
        }
    }
    for r in &config.repos {
        if !l.repos.iter().any(|x| x.id == r.id) {
            l.repos.push(RepoListado {
                id: r.id.clone(),
                nombre: r.name.clone(),
                donde: crate::kit::public_location(&r.location),
                versiones: versiones(&r.id),
                solo_lectura: false,
            });
            for d in &r.derived {
                let destino = d.offsite.target_name.clone().map(|n| format!("«{n}»")).unwrap_or_else(|| crate::kit::public_location(&d.offsite.location));
                l.derivadas.push(DerivadaListada { repo: r.id.clone(), id: d.id.clone(), destino });
            }
        }
        for p in &r.plans {
            l.copias.push(CopiaListada { id: p.id.clone(), nombre: p.name.clone(), repo: r.id.clone(), carpetas: p.paths.len() });
        }
    }
    l
}

/// El listado de `copias`, en palabras.
pub fn texto_copias(l: &Listado) -> String {
    let mut s = String::new();
    let nombre_repo = |id: &str| l.repos.iter().find(|r| r.id == id).map(|r| format!("«{}» ({id})", r.nombre)).unwrap_or_else(|| id.to_string());
    if l.repos.is_empty() {
        s.push_str("Este equipo aún no tiene ningún repositorio.\n");
        return s;
    }
    s.push_str(&format!("Repositorios ({}):\n", l.repos.len()));
    for r in &l.repos {
        let versiones = match r.versiones {
            Some(0) => " · sin versiones".to_string(),
            Some(1) => " · 1 versión".to_string(),
            Some(n) => format!(" · {n} versiones"),
            None => String::new(),
        };
        let lectura = if r.solo_lectura { " · solo lectura (importado de otro equipo)" } else { "" };
        s.push_str(&format!("  {} · id del repositorio: {} · en {}{versiones}{lectura}\n", r.nombre, r.id, r.donde));
    }
    s.push_str(&format!("\nCopias ({}):\n", l.copias.len()));
    if l.copias.is_empty() {
        s.push_str("  Ninguna.\n");
    }
    for k in &l.copias {
        let carpetas = if k.carpetas == 1 { "1 carpeta".to_string() } else { format!("{} carpetas", k.carpetas) };
        s.push_str(&format!("  {} · id de la copia: {} · guarda en {} · {carpetas}\n", k.nombre, k.id, nombre_repo(&k.repo)));
    }
    if !l.derivadas.is_empty() {
        s.push_str(&format!("\nCopias derivadas ({}):\n", l.derivadas.len()));
        for d in &l.derivadas {
            let de_cada_dia = if d.id == "externa" { " (la de cada día)" } else { "" };
            s.push_str(&format!("  de {} · id de la derivada: {}{de_cada_dia} · a {}\n", nombre_repo(&d.repo), d.id, d.destino));
        }
    }
    s.push_str("\nKit de recuperación (como administrador): resguardo-agente kit <id del repositorio> [<id de la derivada>]\n");
    s
}

/// 15q: el acceso para el kit de cualquier repositorio del equipo (con copias o sin ellas,
/// gestionado por una consola o no) o de una de sus copias derivadas.
fn acceso_kit(repo_id: &str, derivada: Option<&str>) -> Result<(String, crate::restic::Access), String> {
    if let Some(d) = derivada {
        let (acc, destino) = crate::tasks::acceso_copia(repo_id, d).map_err(|e| format!("{e} (mira «resguardo-agente copias»)."))?;
        let destino = destino.map(|n| format!(" a «{n}»")).unwrap_or_default();
        return Ok((format!("copia derivada «{d}»{destino} de {repo_id}"), acc));
    }
    if crate::agent::load_config().repos.iter().any(|r| r.id == repo_id) {
        let (repo, acc) = acceso(repo_id)?;
        return Ok((format!("{} ({})", repo.name, repo.id), acc));
    }
    let v = crate::servidor_v2::cargar().ok_or_else(|| format!("No hay ningún repositorio «{repo_id}» en este equipo (mira «copias»)."))?;
    let r = v.repos_v2.iter().find(|r| r.id == repo_id).ok_or_else(|| format!("No hay ningún repositorio «{repo_id}» en este equipo (mira «copias»)."))?;
    Ok((format!("{} ({})", r.nombre, r.id), crate::gestion_v2::acceso(&v, repo_id)?))
}

/// Ejecuta una orden de la línea de órdenes. `args[0]` es la orden.
pub fn ejecutar(args: &[String]) -> i32 {
    adjuntar_consola();
    let r = match orden(args) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error: {e}");
            1
        }
    };
    soltar_consola();
    r
}

fn orden(args: &[String]) -> Result<(), String> {
    match args[0].as_str() {
        "ayuda" => {
            print!("{AYUDA}");
            Ok(())
        }
        "estado" => {
            print!("{}", estado(chrono::Local::now()));
            Ok(())
        }
        "version" => {
            println!("Resguardo Agente {}", crate::version_programa());
            Ok(())
        }
        "actualizaciones" => actualizaciones(args),
        "copias" => {
            print!("{}", texto_copias(&listado()));
            Ok(())
        }
        "copiar-ahora" => {
            crate::agent::request_backup(arg(args, 1)?, arg(args, 2)?)?;
            println!("Copia pedida: el agente la empieza en unos segundos.");
            Ok(())
        }
        "versiones" => {
            crate::agent::require_admin()?;
            let (_, access) = acceso(arg(args, 1)?)?;
            for s in crate::restic::snapshots(&access)? {
                println!("{}  {}  {}", s.short_id, s.time, s.paths.join(", "));
            }
            Ok(())
        }
        "restaurar" => {
            crate::agent::require_admin()?;
            let (_, access) = acceso(arg(args, 1)?)?;
            let version = arg(args, 2)?;
            if !crate::restic::valid_snapshot_id(version) {
                return Err("Versión no válida (usa el id corto de «versiones»).".into());
            }
            let ruta = arg(args, 3)?;
            let destino = opcion(args, "--destino").ok_or("Falta --destino <carpeta>.")?;
            let objetivo = format!("{version}:{}", crate::restic::snapshot_dir(&crate::restic::ruta_en_version(ruta))?);
            let out = crate::restic::run_raw(
                &access,
                &["restore", &objetivo, "--target", &destino, "--overwrite", "never"],
                std::time::Duration::from_secs(24 * 3600),
            )?;
            if out.code != Some(0) {
                return Err(crate::restic::exit_error(out.code, &out.stderr));
            }
            println!("Restaurado en {destino}.");
            Ok(())
        }
        "registro" => {
            let n: usize = args.get(1).and_then(|n| n.parse().ok()).unwrap_or(50);
            for l in crate::agent::log_tail(n) {
                println!("{l}");
            }
            Ok(())
        }
        "kit" => {
            crate::agent::require_admin()?;
            let (nombre, access) = acceso_kit(arg(args, 1)?, args.get(2).map(String::as_str))?;
            println!("Repositorio: {nombre}");
            println!("Ubicación:   {}", crate::kit::public_location(&access.location));
            if let Some((usuario, _)) = &access.rest_auth {
                println!("Usuario del servidor: {usuario} (su contraseña está en el equipo; si la necesitas, pídela en la consola)");
            }
            println!("Contraseña:  {}", access.password);
            // Decisión (G4): el kit es para recuperar sin Resguardo, así que aquí sí se nombra
            // el programa, una vez («el programa de copias (restic)»).
            println!("\nGuárdalo en un lugar seguro: con esto se pueden abrir las copias con el programa de copias (restic) en cualquier equipo.");
            Ok(())
        }
        "leer-instalador" => {
            // v1.17: ¿trae este instalador una cola «listo para vincular»? Con --result,
            // «ok», el servidor, el nombre y el código (una cosa por línea), o «no».
            let ruta = arg(args, 1)?;
            let datos = leer_instalador(ruta)?;
            let texto = match &datos {
                Some(d) => format!("ok\r\n{}\r\n{}\r\n{}\r\n", d.servidor, d.nombre, d.codigo),
                None => "no\r\n".to_string(),
            };
            match opcion(args, "--result") {
                Some(archivo) => std::fs::write(&archivo, &texto).map_err(|e| e.to_string())?,
                None => match &datos {
                    Some(d) => println!("Instalador listo para vincular con {} como «{}» (cliente {}).", d.servidor, d.nombre, d.cliente),
                    None => println!("Instalador genérico: no trae datos para vincular."),
                },
            }
            if datos.is_some() {
                Ok(())
            } else {
                Err("Sin datos para vincular.".into())
            }
        }
        "vincular" => vincular(args),
        "desvincular" => {
            crate::servidor_v2::desvincular_local()?;
            println!("El equipo ha dejado el servidor y sigue con sus copias en modo local.");
            Ok(())
        }
        "guardar-copias" => guardar_copias(args),
        "nube" => {
            let r = nube(args);
            // El resumen de la consola al día (lista de nubes).
            if r.is_ok() && args.get(1).is_some_and(|a| a != "lista") {
                crate::servidor_v2::subir_config_desde_fuera();
            }
            r
        }
        "servidores" => {
            let v = crate::servidor_v2::cargar().ok_or("Este equipo no está vinculado a ningún servidor.")?;
            println!("Servidor: {} · modo {} · equipo {}", if v.url.is_empty() { "(ninguno)" } else { &v.url }, v.modo, v.equipo_id);
            if v.ultimo_ok > 0 {
                let t = chrono::DateTime::from_timestamp(v.ultimo_ok, 0).map(|d| d.with_timezone(&chrono::Local).to_rfc3339()).unwrap_or_default();
                println!("Última respuesta del servidor: {t}");
            }
            if let Some(c) = &v.cambio {
                println!(
                    "Cambio de servidor en curso hacia {} (hasta {}).",
                    c.destino.url,
                    chrono::DateTime::from_timestamp(c.hasta, 0).map(|d| d.to_rfc3339()).unwrap_or_default()
                );
            }
            if let Some(a) = &v.adopcion {
                println!(
                    "Pendiente del alta en {} (con la misma clave de administración; hasta {}).",
                    a.url,
                    chrono::DateTime::from_timestamp(a.hasta, 0).map(|d| d.to_rfc3339()).unwrap_or_default()
                );
            }
            if v.respaldo.is_empty() {
                println!("Sin servidores de respaldo.");
            }
            for r in &v.respaldo {
                println!("Respaldo: {} (si el principal no responde en {} días)", r.url, v.respaldo_dias);
            }
            Ok(())
        }
        "consolas" => {
            let v = crate::servidor_v2::cargar().filter(|v| v.modo == "gestionado" && !v.secreto.is_empty());
            match args.get(1).map(String::as_str) {
                None | Some("lista") => {
                    let v = v.ok_or("Este equipo no está gestionado por ninguna consola.")?;
                    for l in crate::consolas_v2::listar(&v) {
                        println!("{l}");
                    }
                    if v.otras.is_empty() {
                        println!("(Para gestionarlo también desde otra consola: en la consola, «Conectar también a otra consola…».)");
                    }
                    Ok(())
                }
                Some("quitar") => {
                    let que = arg(args, 2)?;
                    let quitada = crate::consolas_v2::quitar_local(que)?;
                    println!("Este equipo ya no se conecta a {quitada}. Las demás consolas siguen igual.");
                    Ok(())
                }
                Some(otro) => Err(format!("«consolas {otro}» no existe. Usa «consolas» o «consolas quitar <n.º | dirección | huella>».")),
            }
        }
        "ordenes" => ordenes(args),
        "parar" => parar(args, chrono::Local::now()),
        "config" => {
            crate::agent::require_admin()?;
            let archivo = arg(args, 2)?;
            match arg(args, 1)? {
                "exportar" => {
                    let v = vinculo_local(false)?;
                    let doc = serde_json::to_string_pretty(&crate::gestion_v2::documento(&v)).map_err(|e| e.to_string())?;
                    std::fs::write(archivo, doc).map_err(|e| format!("No se pudo escribir {archivo}: {e}"))?;
                    println!("Configuración exportada en {archivo} (sin contraseñas).");
                    Ok(())
                }
                "importar" => {
                    let mut v = vinculo_local(true)?;
                    let doc: serde_json::Value = serde_json::from_slice(&std::fs::read(archivo).map_err(|e| format!("No se pudo leer {archivo}: {e}"))?)
                        .map_err(|_| format!("{archivo} no es una configuración que se pueda leer: usa un archivo de «config exportar»."))?;
                    let m = crate::gestion_v2::aplicar_config(&mut v, &serde_json::json!({ "config": doc }))?;
                    crate::servidor_v2::guardar(&v)?;
                    println!("{m}");
                    Ok(())
                }
                otra => Err(format!("Orden desconocida: config {otra}.")),
            }
        }
        "repositorio" => {
            crate::agent::require_admin()?;
            if arg(args, 1)? != "crear" {
                return Err("Uso: repositorio crear <id> --nombre … --tipo … --donde …".into());
            }
            let id = arg(args, 2)?.to_string();
            let mut v = vinculo_local(true)?;
            let usuario = opcion(args, "--usuario");
            let contrasena = leer_linea("Contraseña del repositorio: ")?;
            let secreto = if usuario.is_some() { Some(leer_linea("Secreto del destino: ")?) } else { None };
            let destino_id = format!("destino-{id}");
            let m = crate::gestion_v2::crear_repositorio(
                &mut v,
                &serde_json::json!({
                    "id": id, "nombre": opcion(args, "--nombre").unwrap_or_else(|| id.clone()), "contrasena": contrasena,
                    "destino": { "id": destino_id, "nombre": opcion(args, "--nombre-destino").unwrap_or_else(|| "Destino".into()),
                                 "tipo": opcion(args, "--tipo").ok_or("Falta --tipo.")?, "donde": opcion(args, "--donde").ok_or("Falta --donde.")?,
                                 "usuario": usuario, "secreto": secreto },
                }),
            )?;
            crate::servidor_v2::guardar(&v)?;
            println!("{m}");
            Ok(())
        }
        otra => Err(format!("Orden desconocida: {otra}. Usa «resguardo-agente ayuda».")),
    }
}

/// 14a: `ordenes` (las que esperan su turno) y `ordenes aprobar <n.º>`.
fn ordenes(args: &[String]) -> Result<(), String> {
    crate::agent::require_admin()?;
    let v = crate::servidor_v2::cargar()
        .filter(|v| v.modo == "gestionado" && !v.secreto.is_empty())
        .ok_or("Este equipo no está gestionado por ninguna consola.")?;
    let lista = crate::espera_v2::lista_local(&v);
    let ahora = chrono::Local::now();
    match args.get(1).map(String::as_str) {
        None | Some("lista") => {
            if lista.is_empty() {
                println!("Ninguna orden espera su turno en este equipo.");
            }
            for o in &lista {
                println!("{}", texto_orden(o, ahora));
            }
            Ok(())
        }
        Some("aprobar") => {
            let n: usize = arg(args, 2)?.parse().map_err(|_| "Di el número de la orden (el de «resguardo-agente ordenes»).")?;
            let o = lista.iter().find(|o| o["n"].as_u64() == Some(n as u64)).ok_or(format!("No hay ninguna orden esperando con el número {n}."))?;
            println!("{}", texto_orden(o, ahora));
            if o["aprobable"] != true {
                return Err("El cliente de la consola que la mandó no deja aprobar órdenes en el equipo.".into());
            }
            println!("Se aplicará ya, sin esperar a su hora. Quedará en el historial de todas las consolas, con un aviso en todas.");
            let hola = crate::ipc_local::pedir(&serde_json::json!({ "op": "hola" }))?;
            let sal = hola["sal_equipo"].as_str().ok_or("Este equipo aún no tiene clave de administración.")?.to_string();
            let clave = leer_clave("Clave de administración (Intro para no aprobarla): ")?;
            if clave.is_empty() {
                println!("No se aprobó: sigue esperando su turno.");
                return Ok(());
            }
            let prueba = crate::ipc_local::prueba(&clave, &sal)?;
            drop(clave);
            // Quién lo pidió con sudo (solo informativo; el servicio sabe que es root).
            let sudo = std::env::var("SUDO_USER").ok().filter(|_| cfg!(unix));
            let r = crate::ipc_local::pedir(&serde_json::json!({
                "op": "aprobar_espera", "reto": hola["reto"], "prueba": prueba, "id": o["id"], "sudo_usuario": sudo,
            }))?;
            println!("{}", r["mensaje"].as_str().unwrap_or("Aprobada."));
            if !matches!(r["estado"].as_str(), Some("hecha" | "en_marcha")) {
                return Err("La orden se aprobó, pero no se pudo aplicar.".into());
            }
            Ok(())
        }
        Some(otro) => Err(format!("«ordenes {otro}» no existe. Usa «ordenes» o «ordenes aprobar <n.º>».")),
    }
}

/// 15hh: `parar` (lo que está en marcha, numerado) y `parar <n.º> [--hasta | --en | --a-mano]`.
fn parar(args: &[String], ahora: chrono::DateTime<chrono::Local>) -> Result<(), String> {
    crate::agent::require_admin()?;
    let lista: Vec<serde_json::Value> =
        crate::progreso_v2::tareas(crate::servidor_v2::cargar().as_ref()).into_iter().filter(|t| t["parar"].is_string() && t["fase"] != "en_cola").collect();
    let Some(n) = args.get(1) else {
        if lista.is_empty() {
            println!("No hay nada en marcha en este equipo.");
        }
        for (i, t) in lista.iter().enumerate() {
            println!("{}", texto_en_marcha(i + 1, t));
        }
        if !lista.is_empty() {
            println!("\nPara pararlo: resguardo-agente parar <n.º> (pide la clave de administración).");
        }
        return Ok(());
    };
    let n: usize = n.parse().map_err(|_| "Di el número (el de «resguardo-agente parar»).")?;
    let t = n.checked_sub(1).and_then(|i| lista.get(i)).ok_or(format!("No hay nada en marcha con el número {n}."))?;
    let pausa = pausa_de(args, ahora)?;
    if pausa.is_some() && t["pausable"] != true {
        return Err("Esto se puede parar, pero no pausar (solo las copias).".into());
    }
    println!("{}", texto_en_marcha(n, t));
    println!(
        "{}",
        match &pausa {
            Some(p) if p["a_mano"] == true => "Se parará ya y seguirá cuando se pida «Copiar ahora».".to_string(),
            Some(p) => format!("Se parará ya y seguirá sola {}.", cuando_rfc3339(p["hasta"].as_str().unwrap_or(""), ahora)),
            None => "Se parará ya (lo copiado hasta ahora se aprovecha la próxima vez).".to_string(),
        }
    );
    let hola = crate::ipc_local::pedir(&serde_json::json!({ "op": "hola" }))?;
    let sal = hola["sal_equipo"].as_str().ok_or("Este equipo aún no tiene clave de administración.")?.to_string();
    let clave = leer_clave("Clave de administración (Intro para no pararlo): ")?;
    if clave.is_empty() {
        println!("No se paró: sigue en marcha.");
        return Ok(());
    }
    let prueba = crate::ipc_local::prueba(&clave, &sal)?;
    drop(clave);
    let sudo = std::env::var("SUDO_USER").ok().filter(|_| cfg!(unix));
    let r = crate::ipc_local::pedir(&serde_json::json!({
        "op": "parar", "reto": hola["reto"], "prueba": prueba, "sudo_usuario": sudo,
        "parar": { "clave": t["parar"], "pausa": pausa },
    }))?;
    println!("{}", r["mensaje"].as_str().unwrap_or("Parando."));
    Ok(())
}

/// `--hasta HH:MM` (hoy o, si ya pasó, mañana), `--en <horas>` o `--a-mano`: la pausa (o nada).
fn pausa_de(args: &[String], ahora: chrono::DateTime<chrono::Local>) -> Result<Option<serde_json::Value>, String> {
    if args.iter().any(|a| a == "--a-mano") {
        return Ok(Some(serde_json::json!({ "a_mano": true })));
    }
    if let Some(h) = opcion(args, "--hasta") {
        let t = chrono::NaiveTime::parse_from_str(&h, "%H:%M").map_err(|_| "--hasta va en HH:MM (p. ej. 22:00).")?;
        let hoy = ahora.date_naive().and_time(t).and_local_timezone(chrono::Local).earliest().ok_or("Esa hora no existe hoy.")?;
        let hasta = if hoy > ahora { hoy } else { hoy + chrono::Duration::days(1) };
        return Ok(Some(serde_json::json!({ "hasta": hasta.to_rfc3339() })));
    }
    if let Some(h) = opcion(args, "--en") {
        let horas: f64 = h.replace(',', ".").parse().map_err(|_| "--en va en horas (p. ej. 2).")?;
        if !(0.05..=168.0).contains(&horas) {
            return Err("--en va de unos minutos a 168 horas.".into());
        }
        let hasta = ahora + chrono::Duration::minutes((horas * 60.0).round() as i64);
        return Ok(Some(serde_json::json!({ "hasta": hasta.to_rfc3339() })));
    }
    Ok(None)
}

/// Algo en marcha, para la terminal: «1. Copia «Documentos» · subiendo 45 %».
pub fn texto_en_marcha(n: usize, t: &serde_json::Value) -> String {
    let tipo = match t["tipo"].as_str().unwrap_or("") {
        "copia" => "Copia",
        "verificar" => "Verificación",
        "verificar_externa" => "Verificación de una copia derivada",
        "prueba_restauracion" => "Prueba de restauración",
        "copia_externa" => "Copia derivada",
        "espejo" => "Espejo",
        "restauracion" => "Restauración",
        otro => otro,
    };
    let nombre = t["nombre"].as_str().or(t["repo"].as_str()).map(|x| format!(" «{x}»")).unwrap_or_default();
    let pct = t["porcentaje"].as_f64().map(|p| format!(" · {} %", (p * 100.0).round())).unwrap_or_default();
    let pausa = if t["pausable"] == true { " (se puede pausar)" } else { "" };
    format!("{n}. {tipo}{nombre}{pct}{pausa}")
}

/// Una orden en espera, para la terminal.
pub fn texto_orden(o: &serde_json::Value, ahora: chrono::DateTime<chrono::Local>) -> String {
    let t = |k: &str| o[k].as_str().unwrap_or("").to_string();
    let desde = match o["consola"].as_str() {
        Some(c) if !c.is_empty() => format!("desde la consola «{c}»"),
        _ => "desde otra consola".into(),
    };
    let por = o["por"].as_str().map(|p| format!(", pedida por {p}")).unwrap_or_default();
    let repo = o["repositorio"].as_str().map(|r| format!(" (repositorio «{r}»)")).unwrap_or_default();
    let mut s = format!("{}. {}{repo}\n   {desde}{por} · se aplica {}", o["n"].as_u64().unwrap_or(0), t("descripcion"), cuando_rfc3339(&t("aplica"), ahora));
    if o["aprobable"] == true {
        s.push_str(&format!("\n   Aprobarla ya: resguardo-agente ordenes aprobar {}", o["n"].as_u64().unwrap_or(0)));
    } else {
        s.push_str("\n   Su cliente no deja aprobarla en el equipo: se aplica a su hora si nadie la cancela.");
    }
    s
}

/// Lee una clave de la terminal **sin que se vea** al escribirla (si la entrada no es una
/// terminal, p. ej. redirigida, la lee tal cual). Nunca va en los argumentos.
fn leer_clave(pregunta: &str) -> Result<String, String> {
    use std::io::Write;
    // 15q: en Windows, desde `cmd` o PowerShell la terminal no espera a este programa (es de
    // ventana) y lee el teclado a la vez: la clave podría acabar escrita como una orden de la
    // terminal (y en su historial). Se pide en una ventana de consola propia.
    #[cfg(windows)]
    if ADJUNTADA.load(std::sync::atomic::Ordering::Relaxed) && entrada_es_consola() {
        eprintln!("Se abre una ventana para escribir la clave (así la terminal no la ve).");
        let _ = std::io::stderr().flush();
        return leer_clave_en_ventana(pregunta);
    }
    eprint!("{pregunta}");
    let _ = std::io::stderr().flush();
    let _eco = SinEco::poner();
    let mut s = String::new();
    let r = std::io::stdin().read_line(&mut s);
    if _eco.activo() {
        eprintln!();
    }
    r.map_err(|e| e.to_string())?;
    Ok(s.trim_end_matches(['\r', '\n']).to_string())
}

/// ¿La entrada estándar es una consola (no redirigida)?
#[cfg(windows)]
fn entrada_es_consola() -> bool {
    use windows_sys::Win32::System::Console::{GetConsoleMode, GetStdHandle, STD_INPUT_HANDLE};
    let mut m = 0u32;
    // SAFETY: el handle de la entrada estándar; si no es una consola, falla y ya.
    unsafe {
        let h = GetStdHandle(STD_INPUT_HANDLE);
        !h.is_null() && GetConsoleMode(h, &mut m) != 0
    }
}

/// 15q: pide la clave en una consola propia (sin eco) y vuelve a la de la terminal.
#[cfg(windows)]
fn leer_clave_en_ventana(pregunta: &str) -> Result<String, String> {
    use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
    use windows_sys::Win32::System::Console::{
        AllocConsole, AttachConsole, FreeConsole, GetConsoleMode, ReadConsoleW, SetConsoleMode, SetConsoleTitleW, WriteConsoleW, ATTACH_PARENT_PROCESS,
        ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT,
    };
    let ancho = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    // SAFETY: llamadas a la consola con cadenas terminadas en 0 y búferes propios; los handles
    // abiertos aquí se cierran aquí. Al final se vuelve a la consola de la terminal.
    unsafe {
        FreeConsole();
        if AllocConsole() == 0 {
            AttachConsole(ATTACH_PARENT_PROCESS);
            return Err("No se pudo abrir la ventana para escribir la clave.".into());
        }
        SetConsoleTitleW(ancho("Resguardo Agente: clave de administración").as_ptr());
        let abrir = |n: &str| {
            CreateFileW(
                ancho(n).as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        let (entrada, salida) = (abrir("CONIN$"), abrir("CONOUT$"));
        let mut r = Err("No se pudo leer la clave.".to_string());
        if entrada != INVALID_HANDLE_VALUE && salida != INVALID_HANDLE_VALUE {
            let texto: Vec<u16> = pregunta.encode_utf16().collect();
            let mut n = 0u32;
            WriteConsoleW(salida, texto.as_ptr().cast(), texto.len() as u32, &mut n, std::ptr::null());
            let mut modo = 0u32;
            GetConsoleMode(entrada, &mut modo);
            SetConsoleMode(entrada, (modo | ENABLE_LINE_INPUT | ENABLE_PROCESSED_INPUT) & !ENABLE_ECHO_INPUT);
            let mut buf = [0u16; 512];
            let mut leidos = 0u32;
            if ReadConsoleW(entrada, buf.as_mut_ptr().cast(), buf.len() as u32, &mut leidos, std::ptr::null()) != 0 {
                let s = String::from_utf16_lossy(&buf[..(leidos as usize).min(buf.len())]);
                r = Ok(s.trim_end_matches(['\r', '\n']).to_string());
            }
            // El búfer tenía la clave: fuera.
            buf.iter_mut().for_each(|c| *c = 0);
        }
        for h in [entrada, salida] {
            if h != INVALID_HANDLE_VALUE {
                CloseHandle(h);
            }
        }
        FreeConsole();
        AttachConsole(ATTACH_PARENT_PROCESS);
        r
    }
}

/// Quita el eco de la terminal mientras vive (y lo devuelve como estaba al soltarlo).
struct SinEco {
    #[cfg(unix)]
    antes: Option<libc::termios>,
    #[cfg(windows)]
    antes: Option<(windows_sys::Win32::Foundation::HANDLE, u32)>,
}

impl SinEco {
    #[cfg(unix)]
    fn poner() -> SinEco {
        // SAFETY: fd 0; estructuras locales.
        unsafe {
            if libc::isatty(0) != 1 {
                return SinEco { antes: None };
            }
            let mut t: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(0, &mut t) != 0 {
                return SinEco { antes: None };
            }
            let antes = t;
            t.c_lflag &= !libc::ECHO;
            if libc::tcsetattr(0, libc::TCSANOW, &t) != 0 {
                return SinEco { antes: None };
            }
            SinEco { antes: Some(antes) }
        }
    }

    #[cfg(windows)]
    fn poner() -> SinEco {
        use windows_sys::Win32::System::Console::{GetConsoleMode, GetStdHandle, SetConsoleMode, ENABLE_ECHO_INPUT, STD_INPUT_HANDLE};
        // SAFETY: el handle de la entrada estándar; si no es una consola, GetConsoleMode falla y no se toca nada.
        unsafe {
            let h = GetStdHandle(STD_INPUT_HANDLE);
            let mut modo = 0u32;
            if h.is_null() || GetConsoleMode(h, &mut modo) == 0 {
                return SinEco { antes: None };
            }
            if SetConsoleMode(h, modo & !ENABLE_ECHO_INPUT) == 0 {
                return SinEco { antes: None };
            }
            SinEco { antes: Some((h, modo)) }
        }
    }

    #[cfg(not(any(unix, windows)))]
    fn poner() -> SinEco {
        SinEco {}
    }

    fn activo(&self) -> bool {
        #[cfg(any(unix, windows))]
        return self.antes.is_some();
        #[cfg(not(any(unix, windows)))]
        false
    }
}

impl Drop for SinEco {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(t) = self.antes {
            // SAFETY: devuelve el modo que tenía la terminal.
            unsafe {
                libc::tcsetattr(0, libc::TCSANOW, &t);
            }
        }
        #[cfg(windows)]
        if let Some((h, modo)) = self.antes {
            // SAFETY: el mismo handle y el modo que tenía.
            unsafe {
                windows_sys::Win32::System::Console::SetConsoleMode(h, modo);
            }
        }
    }
}

/// Fecha y hora para leer de un vistazo: «hoy 02:00», «ayer 23:10» o «lun 28/09 14:00».
fn cuando(t: chrono::DateTime<chrono::Local>, now: chrono::DateTime<chrono::Local>) -> String {
    use chrono::Datelike;
    const DIAS: [&str; 7] = ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"];
    let dias = t.date_naive().signed_duration_since(now.date_naive()).num_days();
    let hora = t.format("%H:%M");
    match dias {
        0 => format!("hoy {hora}"),
        -1 => format!("ayer {hora}"),
        1 => format!("mañana {hora}"),
        _ => format!("{} {} {hora}", DIAS[t.weekday().num_days_from_monday() as usize], t.format("%d/%m")),
    }
}

fn cuando_rfc3339(t: &str, now: chrono::DateTime<chrono::Local>) -> String {
    chrono::DateTime::parse_from_rfc3339(t).map(|t| cuando(t.with_timezone(&chrono::Local), now)).unwrap_or_else(|_| t.to_string())
}

/// «bien», «con avisos» o «error» (lo que guarda el agente: ok, warning, error).
fn resultado(r: &str) -> &str {
    match r {
        "ok" => "bien",
        "warning" => "con avisos",
        "error" => "falló",
        "info" => "aviso",
        "stopped" => "parada",
        otro => otro,
    }
}

/// Una línea por copia: la última vez, cómo fue y la próxima.
fn linea_copia(
    nombre: &str,
    ultima: Option<&crate::agent::RunRecord>,
    proxima: Option<chrono::DateTime<chrono::Local>>,
    pausa: Option<String>,
    now: chrono::DateTime<chrono::Local>,
) -> String {
    let ultima = match ultima {
        Some(r) => format!("última {} ({})", cuando_rfc3339(&r.finished, now), resultado(&r.result)),
        None => "todavía ninguna".into(),
    };
    let proxima = match (pausa, proxima) {
        (Some(p), _) => format!("en pausa {p}"),
        (None, Some(t)) => format!("próxima {}", cuando(t, now)),
        (None, None) => "sin horario".into(),
    };
    format!("    {nombre} · {ultima} · {proxima}")
}

/// `resguardo-agente estado`: todo lo importante del equipo en una pantalla.
fn estado(now: chrono::DateTime<chrono::Local>) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let admin = crate::agent::require_admin().is_ok();
    let _ = writeln!(s, "Resguardo Agente {}", crate::version_programa());
    if !admin {
        let _ = writeln!(s, "(Sin permisos de administrador: el servidor, las nubes y algunos datos no se pueden leer. Repite la orden como administrador.)");
    }

    // La consola (Resguardo Server), la consola de Windows y Resguardo Web.
    let _ = writeln!(s, "\nConsola");
    match crate::servidor_v2::cargar() {
        Some(v) if v.modo == "gestionado" && !v.secreto.is_empty() => {
            let _ = writeln!(s, "  Vinculado con la consola {}", v.url);
            if v.ultimo_ok > 0 {
                if let Some(t) = chrono::DateTime::from_timestamp(v.ultimo_ok, 0) {
                    let _ = writeln!(s, "  Última respuesta de la consola: {}", cuando(t.with_timezone(&chrono::Local), now));
                }
            }
            if let Some(c) = &v.cambio {
                let _ = writeln!(s, "  Cambio de consola en curso hacia {}", c.destino.url);
            }
            // v1.35: varias consolas a la vez.
            for e in &v.otras {
                let _ = writeln!(s, "  También lo gestiona: {} ({})", crate::consolas_v2::nombre_de(&e.nombre, &e.url), e.url);
            }
        }
        Some(_) => {
            let _ = writeln!(s, "  Sin consola (modo local): el equipo hace sus copias solo.");
        }
        None => {
            let _ = writeln!(s, "  Sin vincular: el equipo hace sus copias solo. Para vincularlo: resguardo-agente vincular <código> --servidor <url>");
        }
    }
    if let Some(a) = crate::servidor_v2::cargar().and_then(|v| v.adopcion) {
        let _ = writeln!(s, "  Pendiente del alta en la consola {}: el equipo pasa cuando allí lo den de alta con su misma clave de administración.", a.url);
    }
    if let Some(e) = crate::endpoint::load() {
        let estado = if e.stopped { "ya no lo gestiona" } else { "emparejado" };
        let _ = writeln!(s, "  Consola de Resguardo para Windows: «{}» ({estado})", e.console_name);
    }
    if let Some(l) = crate::web::load_link().filter(|l| !l.revoked) {
        let _ = writeln!(s, "  Resguardo Web: vinculado como «{}»", l.device_name);
    }

    // Copias.
    let config = crate::agent::load_config();
    let estado_agente = crate::agent::load_state();
    let _ = writeln!(s, "\nCopias");
    if config.repos.is_empty() {
        let _ = writeln!(s, "  No hay copias programadas en este equipo.");
    }
    for r in &config.repos {
        let _ = writeln!(s, "  {} ({})", r.name, r.id);
        let pausa = r.active_pause(now).map(|p| p.until_words());
        for p in &r.plans {
            let ultima = estado_agente.runs.get(&crate::plans::plan_key(&r.id, &p.id));
            let _ = writeln!(s, "{}", linea_copia(&p.name, ultima, p.schedule.next_slot(now), pausa.clone(), now));
        }
    }
    if let Some(c) = &estado_agente.running {
        let nombre = config.repos.iter().find(|r| r.id == c.repo_id).map_or(c.repo_id.as_str(), |r| r.name.as_str());
        let avance = c.percent.map(|p| format!(" · {:.0} %", p * 100.0)).unwrap_or_default();
        let _ = writeln!(s, "  Ahora mismo: copiando «{nombre}»{avance} (desde {})", cuando_rfc3339(&c.started, now));
    }

    // El almacén (guarda copias de otros equipos) y su espejo.
    let srv = crate::server::load();
    let _ = writeln!(s, "\nAlmacén");
    if srv.enabled {
        let marcha = if crate::server::listening(srv.port) { "en marcha" } else { "sin responder" };
        let equipos = crate::server::repos_by_user(&srv).len();
        let _ = writeln!(s, "  {marcha} en el puerto {} · {equipos} equipo(s) · más detalle: resguardo-agente guardar-copias estado", srv.port);
        match &srv.espejo {
            Some(e) => {
                let _ = writeln!(s, "  Espejo: cada día a las {}", e.hora);
                for d in e.destinos() {
                    let ultima = d.ultima.as_deref().map_or("todavía no".to_string(), |u| cuando_rfc3339(u, now));
                    let _ = writeln!(
                        s,
                        "    {}{} · última {ultima}{}",
                        d.texto(),
                        crate::espejo::cuando_texto(&d),
                        d.resultado.as_deref().map(|r| format!(" · {r}")).unwrap_or_default()
                    );
                }
            }
            None => {
                let _ = writeln!(s, "  Sin espejo.");
            }
        }
    } else {
        let _ = writeln!(s, "  Este equipo no guarda copias de otros.");
    }

    // Nubes.
    let nubes = crate::nube::lista();
    let _ = writeln!(s, "\nNubes");
    if nubes.is_empty() {
        let _ = writeln!(s, "  Ninguna conectada.");
    }
    for n in nubes {
        let tipo = crate::nube::TIPOS.iter().find(|(t, _)| *t == n.tipo).map_or(n.tipo.as_str(), |(_, nombre)| nombre);
        let _ = writeln!(s, "  {} · {tipo}", n.nombre);
    }

    // Últimos errores (de las copias y tareas que fallaron la última vez).
    let mut errores: Vec<(&String, &crate::agent::RunRecord)> = estado_agente.runs.iter().filter(|(_, r)| r.result == "error").collect();
    errores.sort_by(|a, b| b.1.finished.cmp(&a.1.finished));
    let _ = writeln!(s, "\nÚltimos errores");
    if errores.is_empty() {
        let _ = writeln!(s, "  Ninguno: la última vez, todo salió bien.");
    }
    for (clave, r) in errores.into_iter().take(5) {
        let _ = writeln!(s, "  {} · {} · {}", cuando_rfc3339(&r.finished, now), nombre_de_clave(&config, clave), r.message.lines().next().unwrap_or(""));
    }
    let _ = writeln!(s, "\nMás detalle: resguardo-agente registro 50");
    s
}

/// «Oficina / Diario» a partir de la clave del estado («repo#plan»); otras tareas, tal cual.
fn nombre_de_clave(config: &crate::agent::AgentConfig, clave: &str) -> String {
    let nombre_repo = |id: &str| config.repos.iter().find(|r| r.id == id).map_or(id.to_string(), |r| r.name.clone());
    if let Some((tarea, repo)) = clave.split_once(':') {
        let que = match tarea {
            "verify" => "Verificación",
            "offsite" => "Copia derivada",
            "verify_offsite" => "Verificación de la copia derivada",
            "restore_test" => "Prueba de restauración",
            _ => return clave.to_string(),
        };
        return format!("{que} de {}", nombre_repo(repo));
    }
    let Some((repo, plan)) = clave.split_once('#') else { return clave.to_string() };
    match config.repos.iter().find(|r| r.id == repo) {
        Some(r) => match r.plans.iter().find(|p| p.id == plan) {
            Some(p) => format!("{} / {}", r.name, p.name),
            None => format!("{} ({plan})", r.name),
        },
        None => clave.to_string(),
    }
}

/// `guardar-copias …`: el almacén de este equipo (server.rs).
fn guardar_copias(args: &[String]) -> Result<(), String> {
    let r = guardar_copias_sin_avisar(args);
    // Si el equipo está vinculado, el resumen de la consola al día (guarda copias, espejo).
    if r.is_ok() && args.get(1).is_some_and(|a| a != "estado") {
        crate::servidor_v2::subir_config_desde_fuera();
    }
    r
}

fn guardar_copias_sin_avisar(args: &[String]) -> Result<(), String> {
    use crate::server;
    let mostrar_estado = || {
        let c = server::load();
        if !c.enabled {
            println!("Este equipo no guarda copias de otros (el almacén está desactivado).");
            return;
        }
        let en_marcha = if server::listening(c.port) { "en marcha" } else { "sin responder" };
        println!("Almacén {en_marcha} en el puerto {} · carpeta {}", c.port, c.path);
        println!("Acepta: {}", if c.local_subnet_only { "solo la red local" } else { "cualquier red" });
        if let Some(h) = &c.tls_sha256 {
            println!("Huella de su autoridad TLS: {h}");
        }
        println!("Certificado de su autoridad para los equipos que copian aquí: {}", server::cert_file().display());
        if let Some(e) = &c.espejo {
            let limite = e.limite_kib.map(|k| format!(" · subida a la nube hasta {k} KiB/s")).unwrap_or_default();
            println!("Espejo: cada día a las {}{limite}", e.hora);
            for d in e.destinos() {
                println!(
                    "  {}{} · última: {} · {}",
                    d.texto(),
                    crate::espejo::cuando_texto(&d),
                    d.ultima.as_deref().unwrap_or("todavía no"),
                    d.resultado.as_deref().unwrap_or("")
                );
            }
        }
        for (usuario, repos) in server::repos_by_user(&c) {
            println!("  {usuario} · {} repositorio(s)", repos.len());
        }
    };
    match args.get(1).map(String::as_str) {
        None | Some("estado") => {
            mostrar_estado();
            Ok(())
        }
        Some("activar") => {
            let carpeta = opcion(args, "--carpeta").ok_or("Falta --carpeta <ruta> (dónde se guardan las copias de los demás).")?;
            let puerto = match opcion(args, "--puerto") {
                Some(p) => p.parse::<u16>().map_err(|_| "Puerto no válido: escribe un número entre 1024 y 65535 (por ejemplo, 8000).".to_string())?,
                None => 8000,
            };
            server::activar(&carpeta, puerto, !args.iter().any(|a| a == "--toda-la-red"))?;
            mostrar_estado();
            Ok(())
        }
        Some("anadir") => {
            let (usuario, contrasena, ubicacion) = server::anadir_equipo(arg(args, 2)?)?;
            println!("Equipo «{usuario}» añadido. Datos para que copie aquí (la contraseña no se vuelve a mostrar):");
            println!();
            println!("  Ubicación:   {ubicacion}");
            println!("  Usuario:     {usuario}");
            println!("  Contraseña:  {contrasena}");
            if let Some(h) = server::load().tls_sha256 {
                println!("  Huella TLS:  {h}");
            }
            println!("  Certificado: {}", server::cert_file().display());
            Ok(())
        }
        Some("quitar") => {
            let nombre = arg(args, 2)?;
            server::quitar_equipo(nombre)?;
            println!("«{nombre}» ya no puede entrar. Sus copias siguen en la carpeta del almacén.");
            Ok(())
        }
        Some("espejo") => {
            if args.iter().any(|a| a == "--quitar") {
                println!("{}", server::poner_espejo(None)?);
                return Ok(());
            }
            let (carpetas, nubes, en_nube) = (opciones(args, "--carpeta"), opciones(args, "--nube"), opciones(args, "--carpeta-nube"));
            if nubes.len() != en_nube.len() {
                return Err("Cada --nube <nombre> va con su --carpeta-nube <carpeta> (por ejemplo, Resguardo/Almacen).".into());
            }
            let mut destinos: Vec<_> = carpetas.into_iter().map(|c| serde_json::json!({ "tipo": "carpeta", "carpeta": c })).collect();
            destinos.extend(nubes.into_iter().zip(en_nube).map(|(n, c)| serde_json::json!({ "tipo": "nube", "nube": n, "carpeta": c })));
            if destinos.is_empty() {
                return Err("Falta --carpeta <ruta> o --nube <nombre> --carpeta-nube <carpeta> (o --quitar).".into());
            }
            let limite = match opcion(args, "--limite-kib") {
                Some(k) => Some(k.parse::<u32>().map_err(|_| "Límite no válido (KiB/s).".to_string())?),
                None => None,
            };
            let pedido = serde_json::json!({
                "destinos": destinos, "hora": opcion(args, "--hora").unwrap_or_else(|| "02:00".into()), "limite_kib": limite,
            });
            println!("{}", server::poner_espejo(crate::espejo::pedido(&pedido)?)?);
            Ok(())
        }
        Some("desactivar") => {
            server::desactivar()?;
            println!("Almacén desactivado. Las copias guardadas se quedan en su carpeta.");
            Ok(())
        }
        Some(otra) => Err(format!("Orden desconocida: guardar-copias {otra}. Usa «resguardo-agente ayuda».")),
    }
}

/// El vínculo para administrar en local: el que hay (si no lo gestiona un
/// servidor) o uno nuevo sin servidor. `cambiar`: la orden modifica algo.
fn vinculo_local(cambiar: bool) -> Result<crate::servidor_v2::Vinculo, String> {
    match crate::servidor_v2::cargar() {
        Some(v) if cambiar && v.modo == "gestionado" && !v.secreto.is_empty() => {
            Err("Este equipo lo gestiona una consola: cámbialo desde ella (o «desvincular» antes).".into())
        }
        Some(v) => Ok(v),
        None => Ok(crate::servidor_v2::Vinculo { modo: "local".into(), espera_min_horas: 24, ..Default::default() }),
    }
}

/// Una línea de la entrada estándar (contraseñas: no van en la línea de órdenes).
fn leer_linea(pregunta: &str) -> Result<String, String> {
    use std::io::Write;
    eprint!("{pregunta}");
    let _ = std::io::stderr().flush();
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).map_err(|e| e.to_string())?;
    Ok(s.trim_end_matches(['\r', '\n']).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, TimeZone};

    fn at(s: &str) -> chrono::DateTime<Local> {
        Local.from_local_datetime(&chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()).unwrap()
    }

    /// 15q: `copias` enseña todos los repositorios (vacíos, de solo lectura, sin copias),
    /// las copias y las derivadas por separado, y cómo sacar el kit.
    #[test]
    fn copias_con_todos_los_repositorios() {
        let l = Listado {
            repos: vec![
                RepoListado { id: "r1".into(), nombre: "Documentos".into(), donde: "«Almacén»".into(), versiones: Some(12), solo_lectura: false },
                RepoListado { id: "r2".into(), nombre: "Acceso".into(), donde: "«Almacén»".into(), versiones: Some(0), solo_lectura: false },
                RepoListado { id: "r3".into(), nombre: "Fotos".into(), donde: "«Disco E»".into(), versiones: None, solo_lectura: true },
            ],
            copias: vec![CopiaListada { id: "k1".into(), nombre: "Diaria".into(), repo: "r1".into(), carpetas: 3 }],
            derivadas: vec![DerivadaListada { repo: "r1".into(), id: "d1".into(), destino: "«Nube»".into() }],
        };
        let t = texto_copias(&l);
        assert!(t.starts_with("Repositorios (3):"), "{t}");
        assert!(t.contains("Acceso · id del repositorio: r2 · en «Almacén» · sin versiones"), "{t}");
        assert!(t.contains("solo lectura"), "{t}");
        assert!(t.contains("Diaria · id de la copia: k1 · guarda en «Documentos» (r1) · 3 carpetas"), "{t}");
        assert!(t.contains("de «Documentos» (r1) · id de la derivada: d1 · a «Nube»"), "{t}");
        assert!(t.contains("resguardo-agente kit <id del repositorio>"), "{t}");
        assert_eq!(texto_copias(&Listado::default()), "Este equipo aún no tiene ningún repositorio.\n");
    }

    /// 15hh: `parar <n.º> --hasta | --en | --a-mano`.
    #[test]
    fn pausa_de_la_terminal() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let ahora = at("2026-10-20 18:00");
        assert_eq!(pausa_de(&a(&["parar", "1"]), ahora).unwrap(), None);
        assert_eq!(pausa_de(&a(&["parar", "1", "--a-mano"]), ahora).unwrap().unwrap()["a_mano"], true);
        let h = pausa_de(&a(&["parar", "1", "--hasta", "22:00"]), ahora).unwrap().unwrap();
        assert_eq!(h["hasta"], at("2026-10-20 22:00").to_rfc3339());
        // Si ya pasó hoy, mañana.
        let m = pausa_de(&a(&["parar", "1", "--hasta", "08:00"]), ahora).unwrap().unwrap();
        assert_eq!(m["hasta"], at("2026-10-21 08:00").to_rfc3339());
        let e = pausa_de(&a(&["parar", "1", "--en", "2"]), ahora).unwrap().unwrap();
        assert_eq!(e["hasta"], at("2026-10-20 20:00").to_rfc3339());
        assert!(pausa_de(&a(&["parar", "1", "--hasta", "25:00"]), ahora).is_err());
        assert!(pausa_de(&a(&["parar", "1", "--en", "999"]), ahora).is_err());
        let t = serde_json::json!({ "tipo": "copia", "nombre": "Diaria", "porcentaje": 0.45, "pausable": true });
        assert_eq!(texto_en_marcha(1, &t), "1. Copia «Diaria» · 45 % (se puede pausar)");
    }

    /// 14a: `ordenes`, una por bloque, con cómo aprobarla (o por qué no se puede).
    #[test]
    fn texto_de_una_orden_en_espera() {
        let ahora = at("2026-10-13 10:00");
        let aplica = at("2026-10-14 10:30").to_rfc3339();
        let o = serde_json::json!({ "n": 2, "id": "x", "descripcion": "Pausar las copias", "consola": "Oficina", "por": "Ana", "aplica": aplica, "aprobable": true });
        let t = texto_orden(&o, ahora);
        assert!(t.starts_with("2. Pausar las copias\n   desde la consola «Oficina», pedida por Ana · se aplica mañana 10:30"), "{t}");
        assert!(t.ends_with("resguardo-agente ordenes aprobar 2"), "{t}");
        let no = serde_json::json!({ "n": 1, "descripcion": "Quitar el repositorio «Fotos»", "consola": null, "aplica": aplica, "aprobable": false, "repositorio": "Fotos" });
        let t = texto_orden(&no, ahora);
        assert!(t.contains("desde otra consola") && t.contains("(repositorio «Fotos»)") && t.contains("no deja aprobarla"), "{t}");
    }

    #[test]
    fn instalador_listo_o_generico() {
        use resguardo_protocolo::instalador::{cola, DatosInstalador};
        let dir = std::env::temp_dir().join(format!("resguardo-inst-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let generico = dir.join("generico.exe");
        // Más grande que lo que se lee del final, para que se lea solo la cola.
        std::fs::write(&generico, vec![b'M'; 20_000]).unwrap();
        assert_eq!(leer_instalador(&generico.display().to_string()).unwrap(), None);
        let d = DatosInstalador {
            v: 1,
            servidor: "https://192.168.1.20:8443".into(),
            huella_ca: vec!["0F"; 32].join(":"),
            cliente: "c1".into(),
            nombre: "SERVIDOR-01".into(),
            codigo: "ABCD-EFGH-JK".into(),
        };
        let listo = dir.join("listo.exe");
        let mut b = vec![b'M'; 20_000];
        b.extend(cola(&d).unwrap());
        std::fs::write(&listo, b).unwrap();
        assert_eq!(leer_instalador(&listo.display().to_string()).unwrap(), Some(d));
        assert!(leer_instalador(&dir.join("no-existe.exe").display().to_string()).is_err());
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(alias("--leer-instalador"), Some("leer-instalador"));
    }

    #[test]
    fn ayuda_y_version_con_otros_nombres() {
        assert_eq!(alias("--help"), Some("ayuda"));
        assert_eq!(alias("-h"), Some("ayuda"));
        assert_eq!(alias("--version"), Some("version"));
        assert_eq!(alias("--service"), None);
        assert!(ORDENES.contains(&"version"));
        // Las órdenes de siempre siguen ahí (las usan los instaladores y la documentación).
        for o in ["estado", "vincular", "desvincular", "guardar-copias", "registro", "ayuda"] {
            assert!(ORDENES.contains(&o), "{o}");
        }
    }

    #[test]
    fn fechas_para_leer_de_un_vistazo() {
        let now = at("2026-09-30 10:00"); // miércoles
        assert_eq!(cuando(at("2026-09-30 02:00"), now), "hoy 02:00");
        assert_eq!(cuando(at("2026-09-29 23:10"), now), "ayer 23:10");
        assert_eq!(cuando(at("2026-10-01 02:00"), now), "mañana 02:00");
        assert_eq!(cuando(at("2026-09-28 14:00"), now), "lun 28/09 14:00");
    }

    #[test]
    fn una_linea_por_copia() {
        let now = at("2026-09-30 10:00");
        let r = crate::agent::RunRecord { finished: at("2026-09-30 02:03").to_rfc3339(), result: "ok".into(), ..Default::default() };
        assert_eq!(linea_copia("Diario", Some(&r), Some(at("2026-10-01 02:00")), None, now), "    Diario · última hoy 02:03 (bien) · próxima mañana 02:00");
        assert_eq!(
            linea_copia("Diario", None, Some(at("2026-10-01 02:00")), Some("hasta que las reanudes".into()), now),
            "    Diario · todavía ninguna · en pausa hasta que las reanudes"
        );
    }

    #[test]
    fn nombres_de_las_tareas() {
        let config = crate::agent::AgentConfig::default();
        assert_eq!(nombre_de_clave(&config, "verify:nas"), "Verificación de nas");
        assert_eq!(nombre_de_clave(&config, "otra"), "otra");
        assert_eq!(nombre_de_clave(&config, "nas#diario"), "nas#diario");
    }
}
