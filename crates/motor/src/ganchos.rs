//! Ganchos de una copia: **plantillas cerradas** con parámetros validados
//! (docs/plataforma.md §4). Nunca hay órdenes libres: la consola elige una
//! plantilla y rellena sus campos; el agente solo sabe ejecutar estas.
//!
//! - `sqlserver`: antes de la copia, un volcado `BACKUP DATABASE … WITH
//!   COPY_ONLY` de cada base a una carpeta local que entra en la copia; al
//!   terminar, se borran los volcados que hizo el gancho (solo esos archivos).
//! - `carpeta_reciente`: comprueba que la carpeta de copias propias de una
//!   aplicación tiene algún archivo de las últimas `horas`; si no, la copia
//!   termina con aviso (no cambia lo que se copia).
//!
//! La ejecución está en el agente (`crates/agente/src/ganchos.rs`).

use serde::{Deserialize, Serialize};

/// Máximo de ganchos por copia y de bases por gancho.
pub const MAX_GANCHOS: usize = 4;
pub const MAX_BASES: usize = 20;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case", deny_unknown_fields)]
pub enum Gancho {
    /// Volcado de SQL Server (autenticación de Windows: la cuenta del equipo).
    Sqlserver {
        /// `.`, `localhost`, `EQUIPO\INSTANCIA` o `EQUIPO,puerto`. Por defecto, `.` (la instancia predeterminada).
        #[serde(default = "instancia_local")]
        instancia: String,
        /// Bases a volcar (al menos una).
        bases: Vec<String>,
        /// Carpeta local (ruta completa) donde se dejan los volcados. El
        /// servicio de SQL Server tiene que poder escribir en ella (la carpeta
        /// `Backup` de la instancia ya tiene ese permiso).
        carpeta: String,
    },
    /// Aviso si la carpeta de copias de una aplicación no tiene nada reciente.
    CarpetaReciente {
        carpeta: String,
        /// Antigüedad máxima del archivo más nuevo (1 a 720 horas).
        horas: u32,
        /// Solo cuentan los archivos que terminan así (p. ej. `.bak`), sin distinguir mayúsculas.
        #[serde(default)]
        extension: Option<String>,
    },
}

fn instancia_local() -> String {
    ".".into()
}

/// Ruta local completa, sin caracteres de control ni comillas (va dentro de T-SQL).
fn ruta_valida(p: &str) -> bool {
    let p = p.trim();
    let completa = {
        let b = p.as_bytes();
        // «X:\…» (no la raíz) y, fuera de Windows, «/…» (no «//», que puede ser de red).
        let disco = b.len() > 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\' && !matches!(b[3], b'\\' | b'/');
        disco || (!cfg!(windows) && p.starts_with('/') && !p.starts_with("//") && p.trim_end_matches('/').len() > 1)
    };
    completa
        && p.len() <= 240
        && !p.chars().any(|c| c.is_control() || matches!(c, '\'' | '"' | ';' | '*' | '?' | '<' | '>' | '|'))
        && !p.split(['\\', '/']).any(|s| s == "..")
}

/// Nombre del archivo del volcado de una base (solo caracteres seguros).
pub fn archivo_volcado(base: &str) -> String {
    let limpio: String = base.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    format!("resguardo-{limpio}.bak")
}

/// Nombre de base de SQL Server admitido: letras, números, espacio, `_`, `-`, `.` y `$`.
pub fn base_valida(b: &str) -> bool {
    !b.is_empty() && b.len() <= 128 && b.trim() == b && b.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | ' ' | '.' | '$'))
}

/// Instancia: nombre de equipo (o `.`), `\INSTANCIA` opcional y `,puerto` opcional.
pub fn instancia_valida(i: &str) -> bool {
    let (resto, puerto) = i.split_once(',').map_or((i, None), |(a, b)| (a, Some(b)));
    if puerto.is_some_and(|p| p.parse::<u16>().map_or(true, |p| p == 0)) {
        return false;
    }
    let (equipo, inst) = resto.split_once('\\').map_or((resto, None), |(a, b)| (a, Some(b)));
    // Sin «-» al principio: sqlcmd lo tomaría por una opción.
    let nombre =
        |s: &str| !s.is_empty() && !s.starts_with('-') && s.len() <= 63 && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    nombre(equipo) && inst.is_none_or(|n| nombre(n) && !n.contains('.'))
}

impl Gancho {
    pub fn tipo(&self) -> &'static str {
        match self {
            Gancho::Sqlserver { .. } => "sqlserver",
            Gancho::CarpetaReciente { .. } => "carpeta_reciente",
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Gancho::Sqlserver { instancia, bases, carpeta } => {
                if !instancia_valida(instancia) {
                    return Err(format!("Instancia de SQL Server no válida: «{instancia}» (p. ej. «.» o «EQUIPO\\SQLEXPRESS»)."));
                }
                if bases.is_empty() || bases.len() > MAX_BASES {
                    return Err(format!("Indica de 1 a {MAX_BASES} bases de datos para volcar."));
                }
                if let Some(b) = bases.iter().find(|b| !base_valida(b)) {
                    return Err(format!("Nombre de base de datos no admitido: «{b}»."));
                }
                let mut vistas = std::collections::HashSet::new();
                // Repetidas, o que darían el mismo archivo de volcado («a b» y «a_b»).
                if bases.iter().any(|b| !vistas.insert(archivo_volcado(b).to_lowercase())) {
                    return Err("Hay una base de datos repetida (o dos que darían el mismo archivo de volcado).".into());
                }
                if !ruta_valida(carpeta) {
                    return Err("La carpeta de los volcados tiene que ser una ruta local completa (p. ej. C:\\ResguardoVolcados).".into());
                }
            }
            Gancho::CarpetaReciente { carpeta, horas, extension } => {
                if !ruta_valida(carpeta) && !carpeta.starts_with(r"\\") {
                    return Err("La carpeta a vigilar tiene que ser una ruta completa.".into());
                }
                if !(1..=720).contains(horas) {
                    return Err("Las horas tienen que estar entre 1 y 720.".into());
                }
                if extension.as_deref().is_some_and(|e| e.is_empty() || e.len() > 10 || !e.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')) {
                    return Err("Extensión no válida (p. ej. .bak).".into());
                }
            }
        }
        Ok(())
    }

    /// Carpetas que el gancho añade a la copia (los volcados).
    pub fn carpetas_extra(&self) -> Vec<String> {
        match self {
            Gancho::Sqlserver { carpeta, .. } => vec![carpeta.trim().to_string()],
            Gancho::CarpetaReciente { .. } => vec![],
        }
    }
}

/// Valida la lista de ganchos de una copia.
pub fn validar(ganchos: &[Gancho]) -> Result<(), String> {
    if ganchos.len() > MAX_GANCHOS {
        return Err(format!("Como mucho {MAX_GANCHOS} ganchos por copia."));
    }
    ganchos.iter().try_for_each(Gancho::validate)
}

/// `gancho` de la configuración v1: `null`, un gancho o una lista de ganchos.
pub fn de_config(v: &serde_json::Value) -> Result<Vec<Gancho>, String> {
    let lista: Vec<Gancho> = match v {
        serde_json::Value::Null => vec![],
        serde_json::Value::Array(a) => {
            a.iter().map(|g| serde_json::from_value(g.clone())).collect::<Result<_, _>>().map_err(|e| format!("Gancho no válido: {e}"))?
        }
        g => vec![serde_json::from_value(g.clone()).map_err(|e| format!("Gancho no válido: {e}"))?],
    };
    validar(&lista)?;
    Ok(lista)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn plantillas_y_validaciones() {
        let g = de_config(
            &json!({"tipo": "sqlserver", "instancia": "SERVIDOR-01\\SQLEXPRESS", "bases": ["WO_Altamar", "Contab 2026"], "carpeta": "C:\\ResguardoVolcados"}),
        )
        .unwrap();
        assert_eq!(g[0].tipo(), "sqlserver");
        assert_eq!(g[0].carpetas_extra(), vec!["C:\\ResguardoVolcados".to_string()]);
        let d = de_config(&json!({"tipo": "sqlserver", "bases": ["a"], "carpeta": "D:\\v"})).unwrap();
        assert!(matches!(&d[0], Gancho::Sqlserver { instancia, .. } if instancia == "."));
        assert!(de_config(&json!(null)).unwrap().is_empty());
        let dos = de_config(&json!([{"tipo": "carpeta_reciente", "carpeta": "D:\\WO\\Copias", "horas": 26, "extension": ".bak"}, {"tipo": "carpeta_reciente", "carpeta": "\\\\nas\\copias", "horas": 48}])).unwrap();
        assert_eq!(dos.len(), 2);
        // Lo que no es una plantilla, o se sale de ella, no entra.
        for mal in [
            json!({"tipo": "orden", "linea": "del C:\\"}),
            json!({"tipo": "sqlserver", "bases": ["x]; DROP DATABASE y; --"], "carpeta": "C:\\v"}),
            json!({"tipo": "sqlserver", "bases": ["a"], "carpeta": "C:\\v' ; --"}),
            json!({"tipo": "sqlserver", "bases": ["a"], "carpeta": "volcados"}),
            json!({"tipo": "sqlserver", "bases": ["a"], "carpeta": "C:\\a\\..\\b"}),
            json!({"tipo": "sqlserver", "instancia": "a;b", "bases": ["a"], "carpeta": "C:\\v"}),
            json!({"tipo": "sqlserver", "instancia": "a,0", "bases": ["a"], "carpeta": "C:\\v"}),
            json!({"tipo": "sqlserver", "bases": [], "carpeta": "C:\\v"}),
            json!({"tipo": "sqlserver", "bases": ["a", "A"], "carpeta": "C:\\v"}),
            json!({"tipo": "sqlserver", "bases": ["a"], "carpeta": "C:\\v", "despues": "cmd"}),
            json!({"tipo": "carpeta_reciente", "carpeta": "D:\\x", "horas": 0}),
            json!({"tipo": "carpeta_reciente", "carpeta": "D:\\x", "horas": 5, "extension": "*"}),
            // Raíz de un disco, rutas de red, instancia que parece una opción, dos bases con el mismo volcado.
            json!({"tipo": "sqlserver", "bases": ["a"], "carpeta": "C:\\"}),
            json!({"tipo": "sqlserver", "bases": ["a"], "carpeta": "//servidor/compartida"}),
            json!({"tipo": "sqlserver", "bases": ["a"], "carpeta": "C:\\\\servidor"}),
            json!({"tipo": "sqlserver", "instancia": "-Q", "bases": ["a"], "carpeta": "C:\\v"}),
            json!({"tipo": "sqlserver", "bases": ["a b", "a_b"], "carpeta": "C:\\v"}),
        ] {
            assert!(de_config(&mal).is_err(), "{mal}");
        }
        assert!(instancia_valida("localhost,1433") && instancia_valida(".") && !instancia_valida("a\\b\\c"));
        let cinco = serde_json::Value::Array(vec![json!({"tipo": "carpeta_reciente", "carpeta": "D:\\x", "horas": 5}); 5]);
        assert!(de_config(&cinco).is_err());
    }
}
