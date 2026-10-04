//! Ejecución de los ganchos de plantilla de una copia (los tipos y su
//! validación están en el motor: `resguardo_motor::ganchos`).
//!
//! Solo se ejecuta lo que dice la plantilla, con sus parámetros ya validados:
//! - `sqlserver`: `sqlcmd` (de la instalación de SQL Server, en Archivos de
//!   programa; nunca del PATH) con autenticación de Windows, sin shell. Los
//!   nombres de base e instancia solo admiten caracteres seguros, así que no
//!   pueden salirse del T-SQL de la plantilla. Si el agente crea la carpeta
//!   de los volcados, la deja solo para SYSTEM, Administradores y el servicio
//!   de SQL Server. Al terminar la copia se borran los volcados que hizo
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
            p.resultados.push(ResultadoGancho { tipo: g.tipo().into(), estado: "fallo".into(), mensaje: e });
            continue;
        }
        match g {
            Gancho::Sqlserver { instancia, bases, carpeta } => {
                let carpeta = carpeta.trim();
                match preparar_carpeta(Path::new(carpeta), instancia) {
                    Err(e) => p.resultados.push(ResultadoGancho { tipo: g.tipo().into(), estado: "fallo".into(), mensaje: e }),
                    Ok(()) => {
                        p.carpetas.push(carpeta.to_string());
                        for b in bases {
                            let r = volcar(instancia, b, Path::new(carpeta));
                            let (estado, mensaje) = match r {
                                Ok(f) => {
                                    p.volcados.push(f);
                                    ("ok", format!("Volcado de SQL Server de «{b}» hecho."))
                                }
                                Err(e) => ("fallo", format!("No se pudo volcar «{b}» de SQL Server: {e}")),
                            };
                            crate::agent::log(&format!("{}{mensaje}", if estado == "ok" { "" } else { "ERROR: " }));
                            p.resultados.push(ResultadoGancho { tipo: g.tipo().into(), estado: estado.into(), mensaje });
                        }
                    }
                }
            }
            Gancho::CarpetaReciente { carpeta, horas, extension } => {
                let (estado, mensaje) = match reciente(Path::new(carpeta.trim()), *horas, extension.as_deref(), std::time::SystemTime::now()) {
                    Ok(h) => ("ok", format!("Copias de la aplicación al día (la más nueva, de hace {h} h).")),
                    Err(e) => ("aviso", e),
                };
                p.resultados.push(ResultadoGancho { tipo: g.tipo().into(), estado: estado.into(), mensaje });
            }
        }
    }
    p
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

fn volcar(instancia: &str, base: &str, carpeta: &Path) -> Result<PathBuf, String> {
    if !base_valida(base) || !instancia_valida(instancia) {
        return Err("parámetros no válidos.".into());
    }
    let sqlcmd = sqlcmd().ok_or("no se encuentra sqlcmd (las herramientas de línea de órdenes de SQL Server).")?;
    let ruta = carpeta.join(archivo_volcado(base));
    quitar_volcado_previo(&ruta)?;
    let mut c = std::process::Command::new(sqlcmd);
    // Sin `SQLCMDINI` (un guion que sqlcmd ejecuta al conectar), `SQLCMDSERVER`… heredadas.
    resguardo_motor::proceso::entorno_minimo(&mut c);
    c.args(argumentos_sqlcmd(instancia, &tsql_volcado(base, &ruta))).stdin(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let out = c.output().map_err(|e| format!("no se pudo ejecutar sqlcmd: {e}"))?;
    if out.status.success() && ruta.is_file() {
        return Ok(ruta);
    }
    let texto = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    let linea = texto.lines().map(str::trim).find(|l| l.starts_with("Msg") || l.contains("Error") || l.contains("error")).unwrap_or("sqlcmd terminó con error");
    let pista = if texto.contains("Login failed") || texto.contains("Error de inicio de sesión") {
        " (la cuenta del equipo, NT AUTHORITY\\SYSTEM, necesita el rol db_backupoperator en la base)"
    } else if texto.contains("Operating system error 5") || texto.contains("error 5(") {
        " (el servicio de SQL Server no puede escribir en la carpeta de los volcados)"
    } else {
        ""
    };
    Err(format!("{}{pista}", crate::web::public_message(linea)))
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
        let t = tsql_volcado("WO_Altamar", Path::new(r"C:\ResguardoVolcados\resguardo-WO_Altamar.bak"));
        assert_eq!(
            t,
            r"BACKUP DATABASE [WO_Altamar] TO DISK = N'C:\ResguardoVolcados\resguardo-WO_Altamar.bak' WITH COPY_ONLY, INIT, FORMAT, CHECKSUM, NAME = N'Resguardo'"
        );
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
            resultados: vec![ResultadoGancho { tipo: "sqlserver".into(), estado: "fallo".into(), mensaje: "No se pudo volcar «a» de SQL Server: x".into() }],
            ..Default::default()
        };
        aplicar(&mut r, &p);
        assert_eq!((r.result.as_str(), r.message.as_str()), ("error", "Copia hecha, pero no se pudo volcar «a» de SQL Server: x"));
        let _ = std::fs::remove_dir_all(&base);
    }
}
