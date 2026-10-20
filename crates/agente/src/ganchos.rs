//! Ejecución de los ganchos de plantilla de una copia (los tipos y su
//! validación están en el motor: `resguardo_motor::ganchos`).
//!
//! Solo se ejecuta lo que dice la plantilla, con sus parámetros ya validados:
//! - `sqlserver`: `sqlcmd` (de la instalación de SQL Server, en Archivos de
//!   programa; nunca del PATH) con autenticación de Windows, sin shell. Los
//!   nombres de base e instancia solo admiten caracteres seguros, así que no
//!   pueden salirse del T-SQL de la plantilla. Si el agente crea la carpeta
//!   de los volcados, la deja solo para SYSTEM, Administradores y el servicio
//!   de SQL Server. Cada volcado se comprueba enseguida con `RESTORE
//!   VERIFYONLY … WITH CHECKSUM` (14ccc; desactivable con `verificar: false`):
//!   si no pasa, la copia sale fallida; si solo falta el permiso para
//!   verificar (`CREATE ANY DATABASE`), sale con aviso y dice cómo darlo (15f).
//!   Antes de volcar se leen las bases de la instancia (`sys.databases`): las
//!   que no están en línea no se intentan volcar (aviso), con `todas` se
//!   vuelcan todas las de usuario y, con una lista fija, se avisa de las que
//!   no entran (15u). Al terminar la copia se borran los volcados que hizo
//!   (y solo esos archivos).
//! - `carpeta_reciente`: solo lee fechas de archivos.

pub use resguardo_motor::ganchos::*;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Lo que pasó con un gancho (va en el registro de la copia y en el informe, sin rutas).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ResultadoGancho {
    pub tipo: String,
    /// "ok", "aviso" o "fallo".
    pub estado: String,
    pub mensaje: String,
    /// v1.6x (0.8.0, 14ccc): en un volcado de SQL Server,
    /// si se comprobó con `RESTORE VERIFYONLY` (true: bien; false: no pasó).
    /// Sin el campo: no se verificó (desactivado o agente anterior).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verificado: Option<bool>,
}

/// Lo preparado antes de la copia.
#[derive(Debug, Default)]
pub struct Preparado {
    pub resultados: Vec<ResultadoGancho>,
    /// Carpetas que se añaden a la copia.
    pub carpetas: Vec<String>,
    /// Volcados hechos (se borran al terminar).
    pub volcados: Vec<PathBuf>,
}

impl Preparado {
    pub fn fallos(&self) -> impl Iterator<Item = &ResultadoGancho> {
        self.resultados.iter().filter(|r| r.estado == "fallo")
    }
    pub fn avisos(&self) -> impl Iterator<Item = &ResultadoGancho> {
        self.resultados.iter().filter(|r| r.estado == "aviso")
    }
}

/// Antes de la copia: ejecuta los ganchos en orden. Nunca impide la copia:
/// lo que falle se anota (y la copia termina con error o aviso).
pub fn antes(ganchos: &[Gancho]) -> Preparado {
    let mut p = Preparado::default();
    for g in ganchos {
        if let Err(e) = g.validate() {
            p.resultados.push(ResultadoGancho { tipo: g.tipo().into(), estado: "fallo".into(), mensaje: e, verificado: None });
            continue;
        }
        match g {
            Gancho::Sqlserver { instancia, bases, carpeta, todas, excluir, sistema, .. } => {
                let carpeta = carpeta.trim();
                let verificar = g.verifica_volcados();
                match preparar_carpeta(Path::new(carpeta), instancia) {
                    Err(e) => p.resultados.push(ResultadoGancho { tipo: g.tipo().into(), estado: "fallo".into(), mensaje: e, verificado: None }),
                    Ok(()) => {
                        p.carpetas.push(carpeta.to_string());
                        // 15u: las bases de la instancia (con una lista fija, si no se pueden leer, se
                        // vuelca la lista como antes; con «todas», es un fallo).
                        let lista = listar_bases(instancia);
                        if let (true, Err(e)) = (*todas, &lista) {
                            let mensaje = format!("No se pudo leer la lista de bases de la instancia: {e}");
                            p.resultados.push(ResultadoGancho { tipo: g.tipo().into(), estado: "fallo".into(), mensaje, verificado: None });
                        }
                        let eleccion = elegir_bases(bases, *todas, excluir, *sistema, lista.as_deref().ok());
                        for m in eleccion.avisos {
                            crate::agent::log(&format!("AVISO: {m}"));
                            p.resultados.push(ResultadoGancho { tipo: g.tipo().into(), estado: "aviso".into(), mensaje: m, verificado: None });
                        }
                        // 15f: si falta el permiso para verificar, no se vuelve a intentar con cada base.
                        let mut sin_permiso: Option<String> = None;
                        for b in &eleccion.volcar {
                            let volcado = volcar(instancia, b, Path::new(carpeta));
                            if let Ok(f) = &volcado {
                                p.volcados.push(f.clone());
                            }
                            let comprobado = match (&volcado, &sin_permiso) {
                                (Ok(_), Some(e)) if verificar => Some(Err(e.clone())),
                                (Ok(f), None) if verificar => {
                                    let r = verificar_volcado(instancia, f);
                                    if let Err(e) = &r {
                                        if e.starts_with(FALTA_PERMISO_VERIFICAR) {
                                            sin_permiso = Some(e.clone());
                                        }
                                    }
                                    Some(r)
                                }
                                _ => None,
                            };
                            let r = resultado_volcado(
                                b,
                                volcado.as_ref().map(|_| ()).map_err(String::as_str),
                                comprobado.as_ref().map(|c| c.as_ref().map(|_| ()).map_err(String::as_str)),
                            );
                            crate::agent::log(&format!("{}{}", if r.estado == "ok" { "" } else { "ERROR: " }, r.mensaje));
                            p.resultados.push(r);
                        }
                    }
                }
            }
            Gancho::CarpetaReciente { carpeta, horas, extension } => {
                let (estado, mensaje) = match reciente(Path::new(carpeta.trim()), *horas, extension.as_deref(), std::time::SystemTime::now()) {
                    Ok(h) => ("ok", format!("Copias de la aplicación al día (la más nueva, de hace {h} h).")),
                    Err(e) => ("aviso", e),
                };
                p.resultados.push(ResultadoGancho { tipo: g.tipo().into(), estado: estado.into(), mensaje, verificado: None });
            }
        }
    }
    p
}

/// 14ccc: el resultado de volcar una base y, si se pidió, de verificar el volcado.
/// `verificacion`: `None` si no se verificó (desactivado o el volcado falló).
/// 15f: si lo único que falla es el permiso para verificar, la copia no sale
/// fallida (el volcado está hecho): aviso, con la orden para dar el permiso.
pub fn resultado_volcado(base: &str, volcado: Result<(), &str>, verificacion: Option<Result<(), &str>>) -> ResultadoGancho {
    let (estado, mensaje, verificado) = match (volcado, verificacion) {
        (Err(e), _) => ("fallo", format!("No se pudo volcar «{base}» de SQL Server: {e}"), None),
        (Ok(()), None) => ("ok", format!("Volcado de SQL Server de «{base}» hecho (sin verificar)."), None),
        (Ok(()), Some(Ok(()))) => ("ok", format!("Volcado de SQL Server de «{base}» hecho y verificado."), Some(true)),
        (Ok(()), Some(Err(e))) if e.starts_with(FALTA_PERMISO_VERIFICAR) => {
            ("aviso", format!("Volcado de SQL Server de «{base}» hecho, sin verificar: falta permiso. {}", ORDEN_PERMISO_VERIFICAR), None)
        }
        (Ok(()), Some(Err(e))) => ("fallo", format!("El volcado de «{base}» no se pudo verificar: {e}"), Some(false)),
    };
    ResultadoGancho { tipo: "sqlserver".into(), estado: estado.into(), mensaje, verificado }
}

/// 15f: el principio del error de sqlcmd cuando a la cuenta del equipo le falta el
/// permiso que pide `RESTORE VERIFYONLY` (crear bases, a nivel del servidor).
pub const FALTA_PERMISO_VERIFICAR: &str = "falta el permiso para verificar los volcados";
/// 15f: cómo darlo (lo mismo que dice la ayuda del gancho).
pub const ORDEN_PERMISO_VERIFICAR: &str =
    "Para verificarlo, da a la cuenta del equipo el permiso CREATE ANY DATABASE en SQL Server: GRANT CREATE ANY DATABASE TO [NT AUTHORITY\\SYSTEM];";

/// 15u: una base de la instancia (de `sys.databases`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BaseSql {
    pub nombre: String,
    /// `ONLINE`, `RECOVERY_PENDING`, `OFFLINE`, `RESTORING`, `SUSPECT`…
    pub estado: String,
    /// `master`, `tempdb`, `model` o `msdb`.
    pub sistema: bool,
}

/// 15u: qué bases se vuelcan y de qué se avisa (sin rutas).
#[derive(Debug, Default, PartialEq)]
pub struct Eleccion {
    pub volcar: Vec<String>,
    pub avisos: Vec<String>,
}

/// Hasta 3 nombres y «y N más».
fn nombres_cortos(xs: &[String]) -> String {
    let n = xs.len();
    let primeros: Vec<String> = xs.iter().take(3).map(|x| format!("«{x}»")).collect();
    match n {
        0 => String::new(),
        1 => primeros[0].clone(),
        2 | 3 => format!("{} y {}", primeros[..n - 1].join(", "), primeros[n - 1]),
        _ => format!("{} y {} más", primeros.join(", "), n - 3),
    }
}

/// 15u: las bases que se vuelcan en esta copia. Con `todas`, las de usuario en línea
/// de la instancia (y `master` y `msdb` con `sistema`; nunca `tempdb` ni `model`), sin las
/// excluidas. Con una lista fija, la lista, avisando de las bases en línea que no
/// entran. En los dos casos, una base que no está en línea no se intenta volcar: aviso.
/// `instancia`: lo que hay (None si no se pudo leer: con lista fija, se vuelca la lista).
pub fn elegir_bases(bases: &[String], todas: bool, excluir: &[String], sistema: bool, instancia: Option<&[BaseSql]>) -> Eleccion {
    let mut e = Eleccion::default();
    let es = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    let fuera_de_linea = |b: &BaseSql| format!("La base «{}» no está en línea ({}): no se volcó.", b.nombre, b.estado.to_lowercase().replace('_', " "));
    if todas {
        let Some(lista) = instancia else { return e };
        let mut archivos = std::collections::HashSet::new();
        for b in lista {
            let del_sistema_volcable = b.sistema && sistema && (es(&b.nombre, "master") || es(&b.nombre, "msdb"));
            if (b.sistema && !del_sistema_volcable) || excluir.iter().any(|x| es(x, &b.nombre)) {
                continue;
            }
            if !es(&b.estado, "ONLINE") {
                e.avisos.push(fuera_de_linea(b));
            } else if !base_valida(&b.nombre) || !archivos.insert(archivo_volcado(&b.nombre).to_lowercase()) {
                e.avisos.push(format!("La base «{}» tiene un nombre que Resguardo no admite: no se volcó.", crate::web::public_message(&b.nombre)));
            } else if e.volcar.len() >= MAX_BASES_TODAS {
                e.avisos.push(format!("La instancia tiene más de {MAX_BASES_TODAS} bases: se volcaron las {MAX_BASES_TODAS} primeras."));
                break;
            } else {
                e.volcar.push(b.nombre.clone());
            }
        }
        if e.volcar.is_empty() && e.avisos.is_empty() {
            e.avisos.push("La instancia no tiene bases de usuario en línea que volcar.".into());
        }
        return e;
    }
    for b in bases {
        match instancia.and_then(|l| l.iter().find(|x| es(&x.nombre, b))) {
            Some(x) if !es(&x.estado, "ONLINE") => e.avisos.push(fuera_de_linea(x)),
            _ => e.volcar.push(b.clone()),
        }
    }
    if let Some(lista) = instancia {
        let sin: Vec<String> =
            lista.iter().filter(|x| !x.sistema && es(&x.estado, "ONLINE") && !bases.iter().any(|b| es(b, &x.nombre))).map(|x| x.nombre.clone()).collect();
        if !sin.is_empty() {
            e.avisos.push(format!(
                "La instancia tiene {} que no se {}: {}. Añádelas a la lista o elige «Todas las bases».",
                if sin.len() == 1 { "una base" } else { "bases" },
                if sin.len() == 1 { "vuelca" } else { "vuelcan" },
                nombres_cortos(&sin)
            ));
        }
    }
    e
}

/// 15u: las bases de la instancia: «nombre|estado|sistema» por línea, sin cabeceras.
pub const TSQL_BASES: &str =
    "SET NOCOUNT ON; SELECT name + N'|' + state_desc + N'|' + CASE WHEN database_id <= 4 THEN N'1' ELSE N'0' END FROM sys.databases ORDER BY name";

/// 15u: lee la salida de [`TSQL_BASES`] (las líneas que no tienen esa forma no cuentan).
pub fn leer_bases(salida: &str) -> Vec<BaseSql> {
    salida
        .lines()
        .filter_map(|l| {
            let mut p = l.trim().split('|');
            let (nombre, estado, sistema) = (p.next()?.trim(), p.next()?.trim(), p.next()?.trim());
            if p.next().is_some() || nombre.is_empty() || nombre.len() > 128 || estado.is_empty() || !estado.chars().all(|c| c.is_ascii_uppercase() || c == '_')
            {
                return None;
            }
            Some(BaseSql { nombre: nombre.to_string(), estado: estado.to_string(), sistema: sistema == "1" })
        })
        .take(1000)
        .collect()
}

/// 15u: las bases de la instancia, con su estado (también para «Ver las bases de la
/// instancia» en la consola: la orden `bases_sqlserver`, que no cambia nada).
pub fn listar_bases(instancia: &str) -> Result<Vec<BaseSql>, String> {
    if !instancia_valida(instancia) {
        return Err("parámetros no válidos.".into());
    }
    let mut args = argumentos_sqlcmd(instancia, TSQL_BASES);
    // Sin cabeceras, sin espacios de relleno y en UTF-8 (nombres con tildes).
    let consulta = args.split_off(args.len() - 2);
    args.extend(["-h", "-1", "-W", "-f", "65001"].map(String::from));
    args.extend(consulta);
    leer_bases_de(&ejecutar_sqlcmd_args(&args)?)
}

fn leer_bases_de(salida: &str) -> Result<Vec<BaseSql>, String> {
    let l = leer_bases(salida);
    if l.is_empty() {
        return Err("SQL Server no devolvió ninguna base.".into());
    }
    Ok(l)
}

/// Después de la copia (haya ido bien o no): borra los volcados que hizo.
pub fn despues(p: &Preparado) {
    for f in &p.volcados {
        if let Err(e) = std::fs::remove_file(f) {
            crate::agent::log(&format!("AVISO: no se pudo borrar el volcado {}: {e}", f.display()));
        }
    }
}

/// Junta el resultado de los ganchos con el de la copia: un volcado fallido
/// es un error (la base no quedó copiada); una carpeta sin copias recientes, un aviso.
pub fn aplicar(record: &mut crate::agent::RunRecord, p: &Preparado) {
    record.ganchos = p.resultados.clone();
    if let Some(f) = p.fallos().next() {
        if record.result != "error" {
            record.result = "error".into();
            record.message = format!("Copia hecha, pero {}", minuscula(&f.mensaje));
        } else {
            record.message = format!("{} {}", record.message, f.mensaje);
        }
    } else if let Some(a) = p.avisos().next() {
        if record.result == "ok" {
            record.result = "warning".into();
            record.message = format!("Copia hecha. {}", a.mensaje);
            record.unchanged = false;
        }
    }
}

fn minuscula(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|p| p.to_lowercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// Nombre del servicio de SQL Server de una instancia (para el permiso de la carpeta).
pub fn servicio_sql(instancia: &str) -> String {
    let sin_puerto = instancia.split(',').next().unwrap_or(instancia);
    match sin_puerto.split_once('\\') {
        Some((_, nombre)) if !nombre.eq_ignore_ascii_case("MSSQLSERVER") => format!("MSSQL${}", nombre.to_uppercase()),
        _ => "MSSQLSERVER".into(),
    }
}

/// La carpeta de los volcados (solo para ellos): en cada copia se deja solo
/// para SYSTEM, Administradores y el servicio de SQL Server, sea nueva o no,
/// para que ningún usuario pueda leer los volcados ni desviar su borrado.
/// Nunca la raíz de un disco, una carpeta de red ni un camino con enlaces.
fn preparar_carpeta(carpeta: &Path, instancia: &str) -> Result<(), String> {
    crate::platform::carpeta_privada(carpeta)?;
    #[cfg(windows)]
    {
        let cuenta = format!(r"NT SERVICE\{}:(OI)(CI)M", servicio_sql(instancia));
        let (ok, salida) = crate::platform::tool("icacls.exe", &[&carpeta.to_string_lossy(), "/grant", &cuenta])?;
        if !ok {
            return Err(format!(
                "No se pudo dar permiso al servicio de SQL Server en la carpeta de los volcados ({}). Si SQL Server usa otra cuenta, dale permiso de escritura a mano.",
                salida.trim()
            ));
        }
    }
    let _ = instancia;
    Ok(())
}

/// `sqlcmd` de la instalación de SQL Server (nunca del PATH, que podría
/// apuntar a una carpeta que un usuario puede escribir).
pub fn sqlcmd() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        // Las carpetas conocidas del sistema (no %ProgramFiles%, que se hereda del entorno).
        for r in crate::platform::program_files_dirs().into_iter().map(PathBuf::from) {
            // go-sqlcmd (instalación nueva).
            let nuevo = r.join("SqlCmd").join("sqlcmd.exe");
            if nuevo.is_file() {
                return Some(nuevo);
            }
            // Herramientas clásicas: …\Microsoft SQL Server\Client SDK\ODBC\<v>\Tools\Binn y …\<v>\Tools\Binn.
            let base = r.join("Microsoft SQL Server");
            let mut candidatos = Vec::new();
            for d in [base.join("Client SDK").join("ODBC"), base.clone()] {
                if let Ok(it) = std::fs::read_dir(&d) {
                    for e in it.flatten() {
                        let f = e.path().join("Tools").join("Binn").join("SQLCMD.EXE");
                        if f.is_file() {
                            candidatos.push(f);
                        }
                    }
                }
            }
            // La versión más alta (los nombres son números: 170, 160, 130…).
            candidatos.sort();
            if let Some(f) = candidatos.pop() {
                return Some(f);
            }
        }
        None
    }
    #[cfg(not(windows))]
    {
        ["/opt/mssql-tools18/bin/sqlcmd", "/opt/mssql-tools/bin/sqlcmd"].iter().map(PathBuf::from).find(|p| p.is_file())
    }
}

/// Argumentos de sqlcmd. -E: autenticación de Windows (la cuenta del equipo);
/// -b: código de error si falla; -l: espera para conectar; -t: límite de la
/// consulta (6 h); -x: sin sustituir `$(variables)` en la consulta (una ruta
/// con `$(…)` metería variables de entorno en el T-SQL).
pub fn argumentos_sqlcmd(instancia: &str, tsql: &str) -> Vec<String> {
    ["-S", instancia, "-E", "-b", "-x", "-l", "30", "-t", "21600", "-Q", tsql].map(String::from).to_vec()
}

/// Un volcado anterior con el mismo nombre se quita antes de volcar: con
/// `WITH INIT` SQL Server lo reescribiría conservando su dueño y sus permisos
/// (o, si es un enlace, escribiría donde apunta).
fn quitar_volcado_previo(ruta: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(ruta) {
        Err(_) => Ok(()),
        Ok(m) if m.is_dir() && !m.file_type().is_symlink() => Err("en la carpeta de los volcados hay una carpeta con el nombre del volcado.".into()),
        Ok(_) => std::fs::remove_file(ruta).or_else(|_| std::fs::remove_dir(ruta)).map_err(|e| format!("no se pudo quitar el volcado anterior: {e}")),
    }
}

/// El T-SQL de la plantilla (base y ruta ya validadas: sin `]` ni `'`).
pub fn tsql_volcado(base: &str, ruta: &Path) -> String {
    format!("BACKUP DATABASE [{base}] TO DISK = N'{}' WITH COPY_ONLY, INIT, FORMAT, CHECKSUM, NAME = N'Resguardo'", ruta.display())
}

/// 14ccc: comprobar el volcado recién hecho (ruta ya validada: sin `'`). Lee el
/// archivo entero y comprueba la suma de cada página, sin restaurar nada.
pub fn tsql_verificar(ruta: &Path) -> String {
    format!("RESTORE VERIFYONLY FROM DISK = N'{}' WITH CHECKSUM", ruta.display())
}

/// La ruta va entre comillas simples en el T-SQL: nunca con `'` ni `]`, aunque la carpeta ya esté validada.
fn ruta_segura(ruta: &Path) -> bool {
    !ruta.to_string_lossy().contains(['\'', ']'])
}

fn volcar(instancia: &str, base: &str, carpeta: &Path) -> Result<PathBuf, String> {
    if !base_valida(base) || !instancia_valida(instancia) {
        return Err("parámetros no válidos.".into());
    }
    let ruta = carpeta.join(archivo_volcado(base));
    if !ruta_segura(&ruta) {
        return Err("parámetros no válidos.".into());
    }
    quitar_volcado_previo(&ruta)?;
    ejecutar_sqlcmd(instancia, &tsql_volcado(base, &ruta))?;
    if ruta.is_file() {
        Ok(ruta)
    } else {
        Err("sqlcmd terminó sin dejar el volcado.".into())
    }
}

/// 14ccc: `RESTORE VERIFYONLY … WITH CHECKSUM` del volcado, en la misma instancia.
fn verificar_volcado(instancia: &str, ruta: &Path) -> Result<(), String> {
    if !instancia_valida(instancia) || !ruta_segura(ruta) {
        return Err("parámetros no válidos.".into());
    }
    ejecutar_sqlcmd(instancia, &tsql_verificar(ruta))
}

/// Ejecuta un T-SQL de la plantilla con sqlcmd. El error, en palabras y sin rutas.
fn ejecutar_sqlcmd(instancia: &str, tsql: &str) -> Result<(), String> {
    ejecutar_sqlcmd_args(&argumentos_sqlcmd(instancia, tsql)).map(|_| ())
}

/// Ejecuta sqlcmd con esos argumentos (ya validados) y devuelve lo que escribió.
fn ejecutar_sqlcmd_args(args: &[String]) -> Result<String, String> {
    let sqlcmd = sqlcmd().ok_or("no se encuentra sqlcmd (las herramientas de línea de órdenes de SQL Server).")?;
    let mut c = std::process::Command::new(sqlcmd);
    // Sin `SQLCMDINI` (un guion que sqlcmd ejecuta al conectar), `SQLCMDSERVER`… heredadas.
    resguardo_motor::proceso::entorno_minimo(&mut c);
    c.args(args).stdin(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let out = c.output().map_err(|e| format!("no se pudo ejecutar sqlcmd: {e}"))?;
    if out.status.success() {
        return Ok(String::from_utf8_lossy(&out.stdout).to_string());
    }
    let texto = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    Err(error_sqlcmd(&texto))
}

/// La línea útil de la salida de sqlcmd, con una pista de qué hacer si se reconoce.
/// 15f: también con SQL Server en español («Mens. 262, Nivel 14…»), y el permiso que
/// falta para verificar dicho con la orden para darlo.
pub fn error_sqlcmd(texto: &str) -> String {
    let lineas: Vec<&str> = texto.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let min = texto.to_lowercase();
    // 15f: `RESTORE VERIFYONLY` pide poder crear bases (Msg 262 en master).
    if min.contains("create database") && (min.contains("denied") || min.contains("deneg")) {
        return format!("{FALTA_PERMISO_VERIFICAR} (CREATE ANY DATABASE).");
    }
    // «Msg 3241, Level 16, State 0…» (o «Mens. 3241, Nivel 16…») va seguida del texto del error: mejor ese texto.
    let linea = lineas
        .iter()
        .position(|l| l.starts_with("Msg") || l.starts_with("Mens"))
        .and_then(|i| lineas.get(i + 1).copied())
        .or_else(|| lineas.iter().copied().find(|l| l.contains("Error") || l.contains("error")))
        .unwrap_or("SQL Server terminó con un error sin decir el motivo");
    let pista = if min.contains("backup database permission denied") || (min.contains("backup database") && min.contains("deneg")) {
        " (la cuenta del equipo necesita el rol db_backupoperator en esa base: en SQL Server, ALTER ROLE db_backupoperator ADD MEMBER [NT AUTHORITY\\SYSTEM]; dentro de la base)"
    } else if texto.contains("Login failed") || texto.contains("Error de inicio de sesión") {
        " (la cuenta del equipo, NT AUTHORITY\\SYSTEM, necesita el rol db_backupoperator en la base)"
    } else if texto.contains("Operating system error 5") || texto.contains("error 5(") {
        " (el servicio de SQL Server no puede escribir en la carpeta de los volcados)"
    } else if ["damaged", "checksum", "dañad", "suma de comprobación"].iter().any(|x| texto.contains(x)) {
        " (el volcado está dañado: revisa el disco de la carpeta de los volcados y la base con DBCC CHECKDB)"
    } else {
        ""
    };
    format!("{}{pista}", crate::web::public_message(linea))
}

/// Horas desde el archivo más nuevo de la carpeta (hasta 3 niveles y 20 000
/// entradas). Error (texto del aviso) si no hay ninguno o es más viejo que `horas`.
pub fn reciente(carpeta: &Path, horas: u32, extension: Option<&str>, ahora: std::time::SystemTime) -> Result<u64, String> {
    if !carpeta.is_dir() {
        return Err("El agente no encuentra la carpeta de copias de la aplicación.".into());
    }
    let ext = extension.map(|e| e.trim_start_matches('.').to_lowercase());
    let mut mas_nuevo: Option<std::time::SystemTime> = None;
    let mut pendientes = vec![(carpeta.to_path_buf(), 0u8)];
    let mut vistas = 0usize;
    while let Some((dir, nivel)) = pendientes.pop() {
        let Ok(it) = std::fs::read_dir(&dir) else { continue };
        for e in it.flatten() {
            vistas += 1;
            if vistas > 20_000 {
                break;
            }
            let Ok(m) = std::fs::symlink_metadata(e.path()) else { continue };
            if m.is_dir() && !m.file_type().is_symlink() {
                if nivel < 3 {
                    pendientes.push((e.path(), nivel + 1));
                }
                continue;
            }
            if !m.is_file() {
                continue;
            }
            if let Some(x) = &ext {
                if !e.file_name().to_string_lossy().to_lowercase().ends_with(&format!(".{x}")) {
                    continue;
                }
            }
            if let Ok(t) = m.modified() {
                mas_nuevo = Some(mas_nuevo.map_or(t, |n| n.max(t)));
            }
        }
    }
    let Some(t) = mas_nuevo else {
        return Err("La carpeta de copias de la aplicación no tiene ninguna copia.".into());
    };
    let h = ahora.duration_since(t).map(|d| d.as_secs() / 3600).unwrap_or(0);
    if h > u64::from(horas) {
        return Err(format!("La copia propia de la aplicación más nueva es de hace {h} horas (se esperaba en las últimas {horas})."));
    }
    Ok(h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    #[test]
    fn sqlcmd_sin_variables_y_sin_volcado_previo() {
        let a = argumentos_sqlcmd(".", r"BACKUP DATABASE [x] TO DISK = N'C:\a$(PATH).bak'");
        assert!(a.iter().any(|x| x == "-x"));
        assert_eq!(a.last().map(String::as_str), Some(r"BACKUP DATABASE [x] TO DISK = N'C:\a$(PATH).bak'"));
        let d = std::env::temp_dir().join(format!("resguardo-volcado-previo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("resguardo-x.bak");
        quitar_volcado_previo(&f).unwrap();
        std::fs::write(&f, b"viejo").unwrap();
        quitar_volcado_previo(&f).unwrap();
        assert!(!f.exists());
        std::fs::create_dir(&f).unwrap();
        assert!(quitar_volcado_previo(&f).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn nombres_y_tsql() {
        assert_eq!(servicio_sql("."), "MSSQLSERVER");
        assert_eq!(servicio_sql("SERVIDOR-01\\sqlexpress"), "MSSQL$SQLEXPRESS");
        assert_eq!(servicio_sql("localhost\\MSSQLSERVER,1433"), "MSSQLSERVER");
        assert_eq!(archivo_volcado("Contab 2026$"), "resguardo-Contab_2026_.bak");
        let t = tsql_volcado("Ventas", Path::new(r"C:\ResguardoVolcados\resguardo-Ventas.bak"));
        assert_eq!(
            t,
            r"BACKUP DATABASE [Ventas] TO DISK = N'C:\ResguardoVolcados\resguardo-Ventas.bak' WITH COPY_ONLY, INIT, FORMAT, CHECKSUM, NAME = N'Resguardo'"
        );
        // 14ccc: la verificación del mismo archivo, sin restaurar nada.
        assert_eq!(
            tsql_verificar(Path::new(r"C:\ResguardoVolcados\resguardo-Ventas.bak")),
            r"RESTORE VERIFYONLY FROM DISK = N'C:\ResguardoVolcados\resguardo-Ventas.bak' WITH CHECKSUM"
        );
        let a = argumentos_sqlcmd(".", &tsql_verificar(Path::new(r"C:\v\resguardo-x.bak")));
        assert_eq!(a.last().map(String::as_str), Some(r"RESTORE VERIFYONLY FROM DISK = N'C:\v\resguardo-x.bak' WITH CHECKSUM"));
        // Una ruta con comilla o corchete nunca llega al T-SQL.
        assert!(ruta_segura(Path::new(r"C:\v\resguardo-x.bak")));
        assert!(!ruta_segura(Path::new(r"C:\v'; DROP DATABASE x; --\a.bak")));
        assert!(!ruta_segura(Path::new(r"C:\v]\a.bak")));
    }

    #[test]
    fn volcado_verificado_o_no() {
        let r = resultado_volcado("Ventas", Ok(()), Some(Ok(())));
        assert_eq!((r.estado.as_str(), r.mensaje.as_str(), r.verificado), ("ok", "Volcado de SQL Server de «Ventas» hecho y verificado.", Some(true)));
        let r = resultado_volcado("Ventas", Ok(()), None);
        assert_eq!((r.estado.as_str(), r.mensaje.as_str(), r.verificado), ("ok", "Volcado de SQL Server de «Ventas» hecho (sin verificar).", None));
        let r = resultado_volcado("Ventas", Ok(()), Some(Err("el volcado está dañado")));
        assert_eq!(
            (r.estado.as_str(), r.mensaje.as_str(), r.verificado),
            ("fallo", "El volcado de «Ventas» no se pudo verificar: el volcado está dañado", Some(false))
        );
        let r = resultado_volcado("Ventas", Err("sin permiso"), None);
        assert_eq!((r.estado.as_str(), r.mensaje.as_str(), r.verificado), ("fallo", "No se pudo volcar «Ventas» de SQL Server: sin permiso", None));
        // Sin `verificado`, el resultado se escribe como antes (los agentes y consolas anteriores no lo conocen).
        assert!(serde_json::to_value(resultado_volcado("a", Ok(()), None)).unwrap().get("verificado").is_none());
        assert_eq!(serde_json::to_value(resultado_volcado("a", Ok(()), Some(Ok(())))).unwrap()["verificado"], serde_json::json!(true));
        // La copia sale fallida si un volcado no pasa la verificación.
        let p = Preparado { resultados: vec![resultado_volcado("Ventas", Ok(()), Some(Err("x")))], ..Default::default() };
        let mut r = crate::agent::RunRecord { result: "ok".into(), ..Default::default() };
        aplicar(&mut r, &p);
        assert_eq!((r.result.as_str(), r.message.as_str()), ("error", "Copia hecha, pero el volcado de «Ventas» no se pudo verificar: x"));
    }

    #[test]
    fn errores_de_sqlcmd_en_palabras() {
        let danado = "Msg 3189, Level 16, State 1, Server S, Line 1\nDamage to the backup set was detected.\nMsg 3013, Level 16, State 1\nVERIFY DATABASE is terminating abnormally.";
        assert_eq!(error_sqlcmd(danado), "Damage to the backup set was detected.");
        let suma = "Msg 3183, Level 16, State 1\nRESTORE detected an error on page (1:80) in database \"x\" as read from the backup set. checksum mismatch";
        assert!(error_sqlcmd(suma).ends_with("(el volcado está dañado: revisa el disco de la carpeta de los volcados y la base con DBCC CHECKDB)"));
        assert!(error_sqlcmd("Msg 18456, Level 14\nLogin failed for user 'NT AUTHORITY\\SYSTEM'.").contains("db_backupoperator"));
        assert_eq!(error_sqlcmd(""), "SQL Server terminó con un error sin decir el motivo");
        // 15f: el permiso para verificar, en inglés y en español.
        let en = "Msg 262, Level 14, State 1, Server S, Line 1\nCREATE DATABASE permission denied in database 'master'.\nMsg 3013, Level 16, State 1\nVERIFY DATABASE is terminating abnormally.";
        assert!(error_sqlcmd(en).starts_with(FALTA_PERMISO_VERIFICAR));
        let es = "Mens. 262, Nivel 14, Estado 1, Servidor S, Línea 1\nSe denegó el permiso CREATE DATABASE en la base de datos 'master'.";
        assert!(error_sqlcmd(es).starts_with(FALTA_PERMISO_VERIFICAR));
        let es_otro = "Mens. 3201, Nivel 16, Estado 2\nNo se puede abrir el dispositivo de copia de seguridad.";
        assert_eq!(error_sqlcmd(es_otro), "No se puede abrir el dispositivo de copia de seguridad.");
        assert!(error_sqlcmd("Msg 262, Level 14\nBACKUP DATABASE permission denied in database 'Ventas'.").contains("ALTER ROLE db_backupoperator"));
    }

    /// 15f: si solo falta el permiso para verificar, la copia sale con aviso (no fallida) y dice cómo darlo.
    #[test]
    fn sin_permiso_para_verificar() {
        let e = error_sqlcmd("Msg 262, Level 14\nCREATE DATABASE permission denied in database 'master'.");
        let r = resultado_volcado("Ventas", Ok(()), Some(Err(&e)));
        assert_eq!((r.estado.as_str(), r.verificado), ("aviso", None));
        assert!(
            r.mensaje.starts_with("Volcado de SQL Server de «Ventas» hecho, sin verificar: falta permiso.") && r.mensaje.contains("GRANT CREATE ANY DATABASE"),
            "{}",
            r.mensaje
        );
        let p = Preparado { resultados: vec![r], ..Default::default() };
        let mut rec = crate::agent::RunRecord { result: "ok".into(), ..Default::default() };
        aplicar(&mut rec, &p);
        assert_eq!(rec.result, "warning");
    }

    /// 15u: qué bases se vuelcan, de cuáles se avisa y cómo se lee `sys.databases`.
    #[test]
    fn elegir_las_bases() {
        let b = |n: &str, e: &str, s: bool| BaseSql { nombre: n.into(), estado: e.into(), sistema: s };
        let inst = vec![
            b("master", "ONLINE", true),
            b("model", "ONLINE", true),
            b("msdb", "ONLINE", true),
            b("tempdb", "ONLINE", true),
            b("Ventas", "ONLINE", false),
            b("Nominas", "ONLINE", false),
            b("Empresa 3", "RECOVERY_PENDING", false),
            b("Pruebas", "ONLINE", false),
        ];
        let s = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        // Todas: las de usuario en línea, sin las excluidas; la que no está en línea, con aviso.
        let e = elegir_bases(&[], true, &s(&["pruebas"]), false, Some(&inst));
        assert_eq!(e.volcar, s(&["Ventas", "Nominas"]));
        assert_eq!(e.avisos, s(&["La base «Empresa 3» no está en línea (recovery pending): no se volcó."]));
        // Con las del sistema: master y msdb (nunca tempdb ni model).
        let e = elegir_bases(&[], true, &[], true, Some(&inst));
        assert_eq!(e.volcar, s(&["master", "msdb", "Ventas", "Nominas", "Pruebas"]));
        // Lista fija: aviso de las que no entran y de la que no está en línea.
        let e = elegir_bases(&s(&["Ventas", "Empresa 3"]), false, &[], false, Some(&inst));
        assert_eq!(e.volcar, s(&["Ventas"]));
        assert_eq!(e.avisos[0], "La base «Empresa 3» no está en línea (recovery pending): no se volcó.");
        assert_eq!(e.avisos[1], "La instancia tiene bases que no se vuelcan: «Nominas» y «Pruebas». Añádelas a la lista o elige «Todas las bases».");
        // Sin poder leer la instancia, la lista como antes y sin avisos.
        assert_eq!(elegir_bases(&s(&["Ventas"]), false, &[], false, None), Eleccion { volcar: s(&["Ventas"]), avisos: vec![] });
        assert!(elegir_bases(&[], true, &[], false, Some(&inst[..4])).avisos[0].contains("no tiene bases de usuario"));
        // La salida de sqlcmd (sin cabeceras): lo que no tiene la forma no cuenta.
        let l = leer_bases("master|ONLINE|1\r\nVentas|ONLINE|0\n\nEmpresa 3|RECOVERY_PENDING|0\nraro\nx|y|z|w\n");
        assert_eq!(l, vec![b("master", "ONLINE", true), b("Ventas", "ONLINE", false), b("Empresa 3", "RECOVERY_PENDING", false)]);
        assert!(TSQL_BASES.contains("sys.databases"));
    }

    #[test]
    fn carpeta_reciente_y_resultado() {
        let base = std::env::temp_dir().join(format!("resguardo-gancho-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("2026")).unwrap();
        let ahora = SystemTime::now();
        assert!(reciente(&base, 24, None, ahora).unwrap_err().contains("ninguna"));
        let f = base.join("2026").join("empresa.BAK");
        std::fs::write(&f, b"x").unwrap();
        std::fs::File::options().write(true).open(&f).unwrap().set_modified(ahora - Duration::from_secs(30 * 3600)).unwrap();
        std::fs::write(base.join("nota.txt"), b"x").unwrap();
        // El .txt de ahora no cuenta con extensión .bak; el .BAK es de hace 30 h.
        assert!(reciente(&base, 24, Some(".bak"), ahora).unwrap_err().contains("30 horas"));
        assert_eq!(reciente(&base, 48, Some("bak"), ahora).unwrap(), 30);
        assert_eq!(reciente(&base, 24, None, ahora).unwrap(), 0);
        assert!(reciente(&base.join("no"), 24, None, ahora).is_err());

        let g = Gancho::CarpetaReciente { carpeta: base.display().to_string(), horas: 24, extension: Some(".bak".into()) };
        let p = antes(&[g]);
        assert_eq!(p.resultados[0].estado, "aviso");
        let mut r = crate::agent::RunRecord { result: "ok".into(), message: "Copia completada.".into(), ..Default::default() };
        aplicar(&mut r, &p);
        assert_eq!(r.result, "warning");
        assert!(r.message.starts_with("Copia hecha. La copia propia"));
        assert_eq!(r.ganchos.len(), 1);
        // Un volcado fallido es un error aunque restic haya ido bien.
        let mut r = crate::agent::RunRecord { result: "ok".into(), ..Default::default() };
        let p = Preparado {
            resultados: vec![ResultadoGancho {
                tipo: "sqlserver".into(),
                estado: "fallo".into(),
                mensaje: "No se pudo volcar «a» de SQL Server: x".into(),
                verificado: None,
            }],
            ..Default::default()
        };
        aplicar(&mut r, &p);
        assert_eq!((r.result.as_str(), r.message.as_str()), ("error", "Copia hecha, pero no se pudo volcar «a» de SQL Server: x"));
        let _ = std::fs::remove_dir_all(&base);
    }
}
