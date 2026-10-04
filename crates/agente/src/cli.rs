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
//! resguardo-agente config exportar|importar <archivo>
//! resguardo-agente repositorio crear <id> --nombre … --tipo … --donde … [--usuario …]
//! ```

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
    "config",
    "repositorio",
    "ayuda",
    "version",
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

const AYUDA: &str = "Resguardo Agente: órdenes (ejecútalas como administrador; en Linux, con sudo)

  estado                                  Resumen: servidor, copias (última y próxima), Servidor de copias,
                                          espejo, nubes y últimos errores
  version                                 La versión del agente
  copias                                  Repositorios y copias que hace este equipo
  copiar-ahora <repo> <copia>             Lanza una copia ya
  versiones <repo>                        Versiones guardadas en un repositorio
  restaurar <repo> <versión> <ruta> --destino <carpeta>
                                          Restaura una carpeta o archivo de una versión (nunca sobrescribe)
  registro [n]                            Las últimas n líneas del registro (50 por defecto)
  kit <repo>                              Datos para recuperar las copias (incluye la contraseña)
  vincular <código> --servidor <url> [--nombre <nombre>]
                                          Une el equipo a Resguardo Server. Si ya tenía clave de
                                          administración (p. ej. se perdió la consola), conserva todo
                                          y espera el alta del servidor nuevo con esa misma clave.
                                          Con --huella-ca AB:CD:…, solo si la autoridad TLS es esa
  vincular <código> --servidor <url> --empezar-de-cero
                                          Olvida el servidor, la clave y las copias que gestionaba
                                          (lo copiado se queda) y lo vincula como equipo nuevo
  vincular --instalador <ruta>            Une el equipo con los datos de un instalador «listo»
  leer-instalador <ruta>                  ¿Trae ese instalador datos para vincular?
  desvincular                             Deja el servidor y sigue copiando en modo local

  Guardar las copias de otros equipos (Servidor de copias, rest-server solo añadir):
  guardar-copias estado
  guardar-copias activar --carpeta <ruta> [--puerto 8000] [--toda-la-red]
                                          Por defecto solo acepta la red local
  guardar-copias anadir <equipo>          Usuario y contraseña para un equipo cliente
  guardar-copias quitar <equipo>          Ya no puede entrar (sus copias se quedan)
  guardar-copias desactivar               Para el servidor (las copias se quedan)
  guardar-copias espejo [--carpeta <ruta>]... [--nube <nombre> --carpeta-nube <carpeta>]...
                        [--hora 02:00] [--limite-kib <KiB/s>]
                                          Cada noche, copia todo lo guardado a otra carpeta (otro disco)
                                          y/o a una nube conectada (solo añade: nunca borra)
  guardar-copias espejo --quitar          Deja de hacer el espejo (lo copiado se queda)
  nube conectar dropbox|drive --nombre <nombre>
                                          Conecta una nube para el espejo (abre el navegador; el permiso
                                          se guarda protegido solo en este equipo)
  nube lista                              Nubes conectadas en este equipo
  nube quitar <nombre>                    Olvida una nube (revoca también el permiso en su web)

  Sin servidor (modo local) o para moverse:
  servidores                              Servidor actual, cambio en curso y servidores de respaldo
  consolas                                Las consolas que gestionan este equipo (pueden ser varias a la vez)
  consolas quitar <n.º | dirección | huella>
                                          Deja de conectarse a una consola (p. ej. una que ya no existe);
                                          las demás siguen igual
  config exportar <archivo>               La configuración del equipo, sin contraseñas (JSON)
  config importar <archivo>               Aplica las copias de un archivo (solo en modo local)
  repositorio crear <id> --nombre <n> --tipo local|rest|s3|b2|sftp --donde <dónde> [--usuario <u>]
                                          Crea un repositorio (solo en modo local). Pide la contraseña
                                          (y el secreto del destino, si hay usuario) por la entrada estándar
";

/// Las ventanas de Windows sin consola: la de la terminal desde la que se lanza.
fn adjuntar_consola() {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
        AttachConsole(ATTACH_PARENT_PROCESS);
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

/// Ejecuta una orden de la línea de órdenes. `args[0]` es la orden.
pub fn ejecutar(args: &[String]) -> i32 {
    adjuntar_consola();
    match orden(args) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error: {e}");
            1
        }
    }
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
        "copias" => {
            for r in crate::agent::load_config().repos {
                println!("{} ({}) · {}", r.name, r.id, r.location);
                for p in &r.plans {
                    println!("  {} ({}) · {} carpeta(s)", p.name, p.id, p.paths.len());
                }
            }
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
            let (repo, access) = acceso(arg(args, 1)?)?;
            println!("Repositorio: {} ({})", repo.name, repo.id);
            println!("Ubicación:   {}", access.location);
            println!("Contraseña:  {}", access.password);
            println!("\nGuárdalo en un lugar seguro: con esto se pueden abrir las copias con restic en cualquier equipo.");
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
        "vincular" => {
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
            let codigo = codigo.as_str();
            let esperado = crate::servidor_v2::Esperado { huella_ca: huella, cliente };
            if args.iter().any(|a| a == "--empezar-de-cero") {
                crate::servidor_v2::empezar_de_cero()?;
            }
            // Con --result <archivo> (el instalador): «ok», el código de comprobación y el
            // servidor, o «error» y el motivo, una cosa por línea.
            if let Some(archivo) = opcion(args, "--result") {
                let texto = match crate::servidor_v2::vincular_con(&url, codigo, &nombre, &esperado) {
                    Ok(v) => format!("ok\r\n{}\r\n{}\r\n", v.sas, v.servidor),
                    Err(e) => {
                        crate::agent::log(&format!("No se pudo vincular con {url}: {e}"));
                        format!("error\r\n{}\r\n", e.replace(['\r', '\n'], " "))
                    }
                };
                std::fs::write(&archivo, &texto).map_err(|e| e.to_string())?;
                return if texto.starts_with("ok") { Ok(()) } else { Err("No se pudo vincular.".into()) };
            }
            let v = crate::servidor_v2::vincular_con(&url, codigo, &nombre, &esperado)?;
            println!("Vinculado con {}.", v.servidor);
            println!("Huella de la autoridad TLS del servidor: {}", v.huella_ca);
            println!();
            println!("  Código de comprobación: {}", v.sas);
            println!();
            println!("Comprueba en la consola que ve el mismo código antes de confirmar.");
            if !v.sas_v3 {
                println!("El servidor es antiguo y el código no cubre su certificado: comprueba también que la consola");
                println!("enseña esta misma huella de la autoridad TLS. Si no es la misma, no confirmes.");
            }
            if v.espera_alta {
                println!();
                println!("Este equipo ya tenía clave de administración: conserva su configuración y sus copias.");
                println!("El servidor nuevo tiene que darlo de alta con esa MISMA clave; hasta entonces no recibe nada.");
                println!("Si es otro cliente (otra clave), repite con --empezar-de-cero.");
            }
            Ok(())
        }
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
                        .map_err(|e| format!("{archivo} no es JSON: {e}"))?;
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
        "error" => "ERROR",
        "info" => "aviso",
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

    // Servidor (Resguardo Server), consola de Windows y Resguardo Web.
    let _ = writeln!(s, "\nServidor");
    match crate::servidor_v2::cargar() {
        Some(v) if v.modo == "gestionado" && !v.secreto.is_empty() => {
            let _ = writeln!(s, "  Vinculado con Resguardo Server: {}", v.url);
            if v.ultimo_ok > 0 {
                if let Some(t) = chrono::DateTime::from_timestamp(v.ultimo_ok, 0) {
                    let _ = writeln!(s, "  Última respuesta del servidor: {}", cuando(t.with_timezone(&chrono::Local), now));
                }
            }
            if let Some(c) = &v.cambio {
                let _ = writeln!(s, "  Cambio de servidor en curso hacia {}", c.destino.url);
            }
            // v1.3x: varias consolas a la vez.
            for e in &v.otras {
                let _ = writeln!(s, "  También lo gestiona: {} ({})", crate::consolas_v2::nombre_de(&e.nombre, &e.url), e.url);
            }
        }
        Some(_) => {
            let _ = writeln!(s, "  Sin servidor (modo local): el equipo hace sus copias solo.");
        }
        None => {
            let _ = writeln!(s, "  Sin vincular: el equipo hace sus copias solo. Para vincularlo: resguardo-agente vincular <código> --servidor <url>");
        }
    }
    if let Some(a) = crate::servidor_v2::cargar().and_then(|v| v.adopcion) {
        let _ = writeln!(s, "  Pendiente del alta en {}: el equipo pasa cuando allí lo den de alta con su misma clave de administración.", a.url);
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

    // Servidor de copias y espejo.
    let srv = crate::server::load();
    let _ = writeln!(s, "\nServidor de copias");
    if srv.enabled {
        let marcha = if crate::server::listening(srv.port) { "en marcha" } else { "SIN RESPONDER" };
        let equipos = crate::server::repos_by_user(&srv).len();
        let _ = writeln!(s, "  {marcha} en el puerto {} · {equipos} equipo(s) · más detalle: resguardo-agente guardar-copias estado", srv.port);
        match &srv.espejo {
            Some(e) => {
                let _ = writeln!(s, "  Espejo cada día a las {}", e.hora);
                for d in e.destinos() {
                    let ultima = d.ultima.as_deref().map_or("todavía no".to_string(), |u| cuando_rfc3339(u, now));
                    let _ = writeln!(s, "    {} · última {ultima}{}", d.texto(), d.resultado.as_deref().map(|r| format!(" · {r}")).unwrap_or_default());
                }
            }
            None => {
                let _ = writeln!(s, "  Sin espejo.");
            }
        }
    } else {
        let _ = writeln!(s, "  Desactivado (este equipo no guarda copias de otros).");
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
            "offsite" => "Copia externa",
            "verify_offsite" => "Verificación de la copia externa",
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

/// `guardar-copias …`: el Servidor de copias de este equipo (server.rs).
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
            println!("El Servidor de copias está desactivado.");
            return;
        }
        let en_marcha = if server::listening(c.port) { "en marcha" } else { "sin responder" };
        println!("Servidor de copias {en_marcha} en el puerto {} · carpeta {}", c.port, c.path);
        println!("Acepta: {}", if c.local_subnet_only { "solo la red local" } else { "cualquier red" });
        if let Some(h) = &c.tls_sha256 {
            println!("Huella de su autoridad TLS: {h}");
        }
        println!("Certificado para los clientes (--cacert): {}", server::cert_file().display());
        if let Some(e) = &c.espejo {
            let limite = e.limite_kib.map(|k| format!(" · subida a la nube hasta {k} KiB/s")).unwrap_or_default();
            println!("Espejo cada día a las {}{limite}", e.hora);
            for d in e.destinos() {
                println!("  {} · última: {} · {}", d.texto(), d.ultima.as_deref().unwrap_or("todavía no"), d.resultado.as_deref().unwrap_or(""));
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
            println!("Equipo cliente «{usuario}» añadido. Datos para configurarlo (la contraseña no se vuelve a mostrar):");
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
            println!("«{nombre}» ya no puede entrar. Sus copias siguen en la carpeta del servidor.");
            Ok(())
        }
        Some("espejo") => {
            if args.iter().any(|a| a == "--quitar") {
                println!("{}", server::poner_espejo(None)?);
                return Ok(());
            }
            let (carpetas, nubes, en_nube) = (opciones(args, "--carpeta"), opciones(args, "--nube"), opciones(args, "--carpeta-nube"));
            if nubes.len() != en_nube.len() {
                return Err("Cada --nube <nombre> va con su --carpeta-nube <carpeta> (por ejemplo, Resguardo/Sur).".into());
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
            println!("Servidor de copias desactivado. Las copias guardadas se quedan en su carpeta.");
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
            Err("Este equipo lo gestiona un servidor: cámbialo desde la consola (o «desvincular» antes).".into())
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
