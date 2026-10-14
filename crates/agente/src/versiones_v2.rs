//! Plan 0.8.0, F2: comentarios en las versiones (14ww, parte 2) y versiones que se
//! conservan para siempre (14cc).
//!
//! **Dónde se guardan.** En el equipo, como el nombre y la observación del equipo
//! (datos_equipo.rs): llegan con una orden de cualquiera de sus consolas, el equipo
//! los guarda (en `servidor.bin`, junto a lo demás del equipo), los anota en su
//! historial (el que reciben todas sus consolas) y los dice en su resumen. Nunca en
//! el repositorio: no se toca nada de lo copiado (en un almacén de solo añadir, el
//! equipo ni siquiera podría).
//!
//! - **Comentarios** (`comentario_version`, inofensiva): quién, cuándo y texto
//!   (hasta [`MAX_TEXTO`] caracteres, sin caracteres de control y sin nada que
//!   parezca una contraseña o una clave). Cambiarlo o borrarlo, solo quien lo
//!   escribió (la misma persona desde la misma consola).
//! - **Conservar para siempre** (`conservar_version`, clave de administración, con
//!   **motivo obligatorio** también al quitarla). Marcarla aumenta la protección;
//!   quitarla la reduce: con la espera de lo destructivo. La retención del equipo
//!   (`aplicar_retencion`) nunca quita una versión marcada.
//! - **En el almacén.** Si el repositorio está en un almacén de solo añadir, la
//!   retención la aplica el almacén con su propia clave (retencion_almacen.rs), y
//!   el equipo no puede tocar el repositorio. La consola manda la misma orden al
//!   almacén (`{usuario, repo, version, motivo, conservar}`) y el almacén guarda la
//!   lista de ids protegidos (`privado/almacen-conservadas.json`), que su
//!   planificador respeta. Se eligió una lista de ids y no una etiqueta de restic
//!   (`restic tag --add`): etiquetar reescribe la versión con **otro id** (y borra
//!   la de antes, que en un almacén de solo añadir el equipo no puede), así que los
//!   comentarios, los informes y las fichas que la nombran dejarían de encontrarla.

use crate::servidor_v2::Vinculo;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Largo máximo de un comentario.
pub const MAX_TEXTO: usize = 280;
/// Comentarios como mucho por equipo (van en el resumen, que no pasa de 64 KiB).
pub const MAX_COMENTARIOS: usize = 40;
/// Comentarios como mucho por versión.
pub const MAX_POR_VERSION: usize = 5;
/// Versiones conservadas como mucho por equipo (y por repositorio en el almacén).
pub const MAX_CONSERVADAS: usize = 100;
/// Largo del motivo de «Conservar» (y de quitarlo).
pub const MIN_MOTIVO: usize = 3;
pub const MAX_MOTIVO: usize = 200;
/// Lo que ocupan como mucho en el resumen (en JSON); lo que no cabe, no va (lo más antiguo).
const MAX_RESUMEN_BYTES: usize = 24 * 1024;

/// Un comentario en una versión.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Comentario {
    /// Id propio (lo pone el equipo).
    pub id: String,
    pub repo: String,
    /// El id de la versión (8 a 64 caracteres hexadecimales; se compara por el principio).
    pub version: String,
    pub texto: String,
    pub cuando: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editado: Option<String>,
    /// El nombre que esa consola tiene en el equipo.
    #[serde(default)]
    pub consola: String,
    /// La identidad de la consola (para saber quién puede cambiarlo).
    #[serde(default)]
    pub identidad: String,
    /// Quién lo escribió (lo dice la consola).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub por: Option<String>,
    /// La cuenta de quien lo escribió en esa consola (para cambiarlo o borrarlo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autor: Option<String>,
}

/// Una versión que la retención nunca quita.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Conservada {
    pub repo: String,
    pub version: String,
    pub motivo: String,
    pub cuando: String,
    #[serde(default)]
    pub consola: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub por: Option<String>,
}

fn corto(t: &str, max: usize) -> String {
    t.chars().take(max).collect()
}

/// Un id de versión de restic: 8 a 64 caracteres hexadecimales (minúsculas).
pub fn version_valida(v: &str) -> bool {
    (8..=64).contains(&v.len()) && v.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// ¿Es `id` (completo o corto) la versión `marcada` (completa o corta)?
pub fn misma_version(marcada: &str, id: &str) -> bool {
    let n = marcada.len().min(id.len());
    n >= 8 && marcada[..n] == id[..n]
}

/// ¿Parece una contraseña o una clave? Una palabra larga que mezcla letras y
/// números sin espacios, un bloque PEM o «contraseña: …». No es infalible: el
/// aviso de la consola lo dice antes de escribir.
pub fn parece_secreto(t: &str) -> bool {
    let bajo = t.to_lowercase();
    if bajo.contains("-----begin") {
        return true;
    }
    for p in ["contraseña", "contrasena", "password", "passwd", "clave", "token", "secret", "api key", "apikey"] {
        if let Some(i) = bajo.find(p) {
            let resto = bajo[i + p.len()..].trim_start();
            if resto.starts_with(':') || resto.starts_with('=') || resto.starts_with("es ") {
                return true;
            }
        }
    }
    t.split_whitespace().any(|w| {
        let w = w.trim_matches(|c: char| ".,;:()[]{}«»\"'".contains(c));
        let letras = w.chars().filter(|c| c.is_alphabetic()).count();
        let cifras = w.chars().filter(char::is_ascii_digit).count();
        w.chars().count() >= 20 && letras > 0 && cifras > 0 && !w.contains("://")
    })
}

/// El texto limpio: sin espacios en los extremos, `\n` como salto de línea, hasta
/// [`MAX_TEXTO`] caracteres, sin caracteres de control (salvo el salto de línea)
/// y sin nada que parezca un secreto. Vacío: borrar.
pub fn limpiar_texto(t: &str) -> Result<String, String> {
    let t = t.replace("\r\n", "\n").replace('\r', "\n");
    let t = t.trim().to_string();
    if t.chars().count() > MAX_TEXTO {
        return Err(format!("Como mucho {MAX_TEXTO} caracteres."));
    }
    if t.chars().any(|c| c.is_control() && c != '\n') {
        return Err("El texto lleva caracteres no válidos.".into());
    }
    if parece_secreto(&t) {
        return Err("Parece una contraseña o una clave: no la pongas en un comentario.".into());
    }
    Ok(t)
}

/// El motivo de «Conservar» (y de quitarlo): de 3 a 200 caracteres, una línea, sin secretos.
pub fn limpiar_motivo(t: &str) -> Result<String, String> {
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    let n = t.chars().count();
    if !(MIN_MOTIVO..=MAX_MOTIVO).contains(&n) {
        return Err(format!("Escribe el motivo (de {MIN_MOTIVO} a {MAX_MOTIVO} caracteres)."));
    }
    if t.chars().any(char::is_control) {
        return Err("El motivo lleva caracteres no válidos.".into());
    }
    if parece_secreto(&t) {
        return Err("Parece una contraseña o una clave: no la pongas en el motivo.".into());
    }
    Ok(t)
}

fn version_de(c: &Value) -> Result<String, String> {
    let v = c["version"].as_str().unwrap_or("").trim().to_lowercase();
    if !version_valida(&v) {
        return Err("Versión no válida.".into());
    }
    Ok(v)
}

fn repo_de(v: &Vinculo, c: &Value) -> Result<String, String> {
    let r = c["repo"].as_str().unwrap_or("");
    v.repos_v2.iter().find(|x| x.id == r).map(|x| x.id.clone()).ok_or_else(|| "Ese repositorio no lo gestiona este servidor.".to_string())
}

fn consola_de(v: &Vinculo) -> String {
    corto(&crate::consolas_v2::nombre_de(&v.nombre_consola, ""), 60)
}

fn autor_de(c: &Value) -> Option<String> {
    c["autor"].as_str().map(|a| corto(a.trim(), 64)).filter(|a| !a.is_empty() && !a.chars().any(char::is_control))
}

/// `comentario_version { repo, version, texto, id?, autor? }`: sin `id`, uno nuevo; con
/// `id`, lo cambia (o lo borra, con el texto vacío), solo si lo escribió la misma
/// persona desde la misma consola.
pub fn comentar(v: &mut Vinculo, c: &Value, por: Option<&str>) -> Result<String, String> {
    let repo = repo_de(v, c)?;
    let version = version_de(c)?;
    let texto = limpiar_texto(c["texto"].as_str().ok_or("Falta el texto (vacío para borrarlo).")?)?;
    let autor = autor_de(c);
    let por = por.map(|p| corto(p.trim(), 60)).filter(|p| !p.is_empty());
    let ahora = chrono::Local::now().to_rfc3339();
    match c["id"].as_str().filter(|s| !s.is_empty()) {
        None => {
            if texto.is_empty() {
                return Err("Escribe el comentario.".into());
            }
            if v.comentarios.len() >= MAX_COMENTARIOS {
                return Err(format!("Este equipo ya tiene {MAX_COMENTARIOS} comentarios: borra alguno que ya no haga falta."));
            }
            if v.comentarios.iter().filter(|x| x.repo == repo && misma_version(&x.version, &version)).count() >= MAX_POR_VERSION {
                return Err(format!("Esta versión ya tiene {MAX_POR_VERSION} comentarios."));
            }
            let id = uuid::Uuid::new_v4().simple().to_string()[..12].to_string();
            v.comentarios.push(Comentario {
                id,
                repo,
                version,
                texto,
                cuando: ahora,
                editado: None,
                consola: consola_de(v),
                identidad: v.identidad.clone(),
                por,
                autor,
            });
            Ok("Comentario guardado: lo verán todas las consolas del equipo.".into())
        }
        Some(id) => {
            let i = v.comentarios.iter().position(|x| x.id == id && x.repo == repo).ok_or("Ese comentario ya no existe.")?;
            let x = &v.comentarios[i];
            let mismo = x.identidad == v.identidad && x.autor.is_some() && x.autor == autor;
            if !mismo {
                return Err("Solo quien escribió el comentario puede cambiarlo o borrarlo.".into());
            }
            if texto.is_empty() {
                v.comentarios.remove(i);
                return Ok("Comentario borrado.".into());
            }
            let x = &mut v.comentarios[i];
            x.texto = texto;
            x.editado = Some(ahora);
            Ok("Comentario cambiado.".into())
        }
    }
}

/// `conservar_version { repo, version, motivo, conservar }` en el equipo dueño (clave de
/// administración; quitarla, con la espera de lo destructivo).
pub fn conservar(v: &mut Vinculo, c: &Value, por: Option<&str>) -> Result<String, String> {
    let repo = repo_de(v, c)?;
    let version = version_de(c)?;
    let motivo = limpiar_motivo(c["motivo"].as_str().unwrap_or(""))?;
    let si = c["conservar"].as_bool().ok_or("Falta si se conserva o no.")?;
    let ya = v.conservadas.iter().position(|x| x.repo == repo && misma_version(&x.version, &version));
    let nombre = v.repos_v2.iter().find(|r| r.id == repo).map(|r| r.nombre.clone()).unwrap_or_default();
    if si {
        if let Some(i) = ya {
            v.conservadas[i].motivo = motivo;
            return Ok("La versión ya se conservaba para siempre: motivo cambiado.".into());
        }
        if v.conservadas.len() >= MAX_CONSERVADAS {
            return Err(format!("Este equipo ya conserva {MAX_CONSERVADAS} versiones para siempre: quita alguna antes."));
        }
        v.conservadas.push(Conservada {
            repo,
            version,
            motivo,
            cuando: chrono::Local::now().to_rfc3339(),
            consola: consola_de(v),
            por: por.map(|p| corto(p.trim(), 60)).filter(|p| !p.is_empty()),
        });
        Ok(format!("Versión de «{nombre}» conservada para siempre: la retención nunca la quitará."))
    } else {
        let Some(i) = ya else { return Ok("Esa versión no estaba marcada para conservarla.".into()) };
        v.conservadas.remove(i);
        Ok(format!("La versión de «{nombre}» ya no se conserva para siempre: la retención podrá quitarla."))
    }
}

/// Los ids conservados de un repositorio del equipo.
pub fn conservadas_de(v: &Vinculo, repo: &str) -> Vec<String> {
    v.conservadas.iter().filter(|x| x.repo == repo).map(|x| x.version.clone()).collect()
}

/// Lo que quita la retención menos lo conservado.
pub fn sin_conservadas(quitar: Vec<String>, conservadas: &[String]) -> (Vec<String>, usize) {
    let antes = quitar.len();
    let quedan: Vec<String> = quitar.into_iter().filter(|id| !conservadas.iter().any(|c| misma_version(c, id))).collect();
    let salvadas = antes - quedan.len();
    (quedan, salvadas)
}

/// La descripción para el historial común (con el motivo: lo pide el historial).
pub fn descripcion(tipo: &str, v: &Vinculo, c: &Value) -> Option<String> {
    let version: String = c["version"].as_str().unwrap_or("").chars().take(8).collect();
    let repo = c["repo"].as_str().unwrap_or("");
    let nombre = v.repos_v2.iter().find(|r| r.id == repo).map(|r| r.nombre.clone()).unwrap_or_else(|| repo.to_string());
    let donde = match c["usuario"].as_str() {
        Some(u) => format!("{u}/{repo} (en este almacén)"),
        None => format!("«{nombre}»"),
    };
    let motivo = corto(c["motivo"].as_str().unwrap_or("").trim(), 100);
    Some(match tipo {
        "comentario_version" => match (c["id"].as_str().filter(|s| !s.is_empty()), c["texto"].as_str().unwrap_or("").trim().is_empty()) {
            (None, _) => format!("Comentar la versión {version} de {donde}"),
            (Some(_), true) => format!("Borrar un comentario de la versión {version} de {donde}"),
            (Some(_), false) => format!("Cambiar un comentario de la versión {version} de {donde}"),
        },
        "conservar_version" if c["conservar"] == false => format!("Dejar de conservar para siempre la versión {version} de {donde}: «{motivo}»"),
        "conservar_version" => format!("Conservar para siempre la versión {version} de {donde}: «{motivo}»"),
        _ => return None,
    })
}

/// `resumen.comentarios` (`null` si no hay): los más recientes que caben. `esta: true`
/// si lo escribió la consola que recibe el resumen (para ofrecer «Cambiar»).
pub fn resumen_comentarios(v: &Vinculo) -> Value {
    if v.comentarios.is_empty() {
        return Value::Null;
    }
    let mut lista: Vec<&Comentario> = v.comentarios.iter().collect();
    lista.sort_by(|a, b| b.cuando.cmp(&a.cuando));
    let mut out = Vec::new();
    let mut bytes = 0;
    for c in lista {
        let x = json!({
            "id": c.id, "repo": c.repo, "version": c.version, "texto": c.texto, "cuando": c.cuando, "editado": c.editado,
            "consola": (!c.consola.is_empty()).then_some(&c.consola), "por": c.por, "autor": c.autor,
            "esta": !c.identidad.is_empty() && c.identidad == v.identidad,
        });
        bytes += x.to_string().len();
        if bytes > MAX_RESUMEN_BYTES {
            break;
        }
        out.push(x);
    }
    json!(out)
}

/// `resumen.conservadas` (`null` si no hay).
pub fn resumen_conservadas(v: &Vinculo) -> Value {
    if v.conservadas.is_empty() {
        return Value::Null;
    }
    json!(v
        .conservadas
        .iter()
        .map(|c| json!({ "repo": c.repo, "version": c.version, "motivo": c.motivo, "cuando": c.cuando, "consola": (!c.consola.is_empty()).then_some(&c.consola), "por": c.por }))
        .collect::<Vec<_>>())
}

/// Al quitar un repositorio, sus comentarios y marcas se van con él.
pub fn olvidar_repo(v: &mut Vinculo, repo: &str) {
    v.comentarios.retain(|c| c.repo != repo);
    v.conservadas.retain(|c| c.repo != repo);
}

// ---------- En el almacén ----------

const ARCHIVO_ALMACEN: &str = "almacen-conservadas.json";

/// Las versiones conservadas de un repositorio del almacén (`usuario/repo`).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ConservadasAlmacen {
    pub usuario: String,
    pub repo: String,
    #[serde(default)]
    pub versiones: Vec<Conservada>,
}

static CANDADO: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn archivo_almacen() -> std::path::PathBuf {
    crate::agent::private_dir().join(ARCHIVO_ALMACEN)
}

pub fn cargar_almacen() -> Vec<ConservadasAlmacen> {
    let _g = CANDADO.lock();
    std::fs::read(archivo_almacen()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Los ids conservados de `usuario/repo` en este almacén.
pub fn conservadas_almacen(usuario: &str, repo: &str) -> Vec<String> {
    cargar_almacen()
        .into_iter()
        .find(|e| e.usuario == usuario && e.repo == repo)
        .map(|e| e.versiones.into_iter().map(|c| c.version).collect())
        .unwrap_or_default()
}

/// `conservar_version { usuario, repo, version, motivo, conservar }` en el almacén.
pub fn conservar_en_almacen(v: &Vinculo, c: &Value, por: Option<&str>) -> Result<String, String> {
    let usuario = c["usuario"].as_str().unwrap_or("").trim().to_string();
    let repo = c["repo"].as_str().unwrap_or("").trim().to_string();
    if !crate::retencion_almacen::segmento_valido(&usuario) || repo.is_empty() || repo.split('/').any(|s| !crate::retencion_almacen::segmento_valido(s)) {
        return Err("Repositorio del almacén no válido.".into());
    }
    if !crate::server::load().enabled {
        return Err("Este equipo no guarda copias de otros.".into());
    }
    let version = version_de(c)?;
    let motivo = limpiar_motivo(c["motivo"].as_str().unwrap_or(""))?;
    let si = c["conservar"].as_bool().ok_or("Falta si se conserva o no.")?;
    crate::agent::prepare_dir()?;
    let _g = CANDADO.lock().map_err(|_| "Candado roto.".to_string())?;
    let mut lista: Vec<ConservadasAlmacen> = std::fs::read(archivo_almacen()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let pos = match lista.iter().position(|e| e.usuario == usuario && e.repo == repo) {
        Some(p) => p,
        None => {
            lista.push(ConservadasAlmacen { usuario: usuario.clone(), repo: repo.clone(), versiones: vec![] });
            lista.len() - 1
        }
    };
    let e = &mut lista[pos];
    let ya = e.versiones.iter().position(|x| misma_version(&x.version, &version));
    let m = match (si, ya) {
        (true, Some(i)) => {
            e.versiones[i].motivo = motivo;
            "La versión ya se conservaba para siempre en este almacén: motivo cambiado.".to_string()
        }
        (true, None) => {
            if e.versiones.len() >= MAX_CONSERVADAS {
                return Err(format!("Este repositorio ya tiene {MAX_CONSERVADAS} versiones conservadas en el almacén."));
            }
            e.versiones.push(Conservada {
                repo: repo.clone(),
                version,
                motivo,
                cuando: chrono::Local::now().to_rfc3339(),
                consola: consola_de(v),
                por: por.map(|p| corto(p.trim(), 60)).filter(|p| !p.is_empty()),
            });
            format!("La retención de este almacén nunca quitará esa versión de «{usuario}/{repo}».")
        }
        (false, Some(i)) => {
            e.versiones.remove(i);
            format!("La retención de este almacén ya puede quitar esa versión de «{usuario}/{repo}».")
        }
        (false, None) => "Esa versión no estaba marcada en este almacén.".to_string(),
    };
    lista.retain(|e| !e.versiones.is_empty());
    let datos = serde_json::to_vec(&lista).map_err(|e| e.to_string())?;
    crate::agent::write_new(&archivo_almacen(), &datos).map_err(|e| format!("No se pudo guardar: {e}"))?;
    Ok(m)
}

/// `guarda_copias.conservadas` del resumen del almacén: `[{usuario, repo, versiones: [{version, motivo, cuando}]}]`.
pub fn resumen_almacen() -> Value {
    let l = cargar_almacen();
    if l.is_empty() {
        return Value::Null;
    }
    json!(l
        .iter()
        .map(|e| json!({ "usuario": e.usuario, "repo": e.repo, "versiones": e.versiones.iter().map(|c| json!({ "version": c.version, "motivo": c.motivo, "cuando": c.cuando })).collect::<Vec<_>>() }))
        .collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vinculo() -> Vinculo {
        let mut v: Vinculo = serde_json::from_value(json!({
            "url": "https://consola.ejemplo", "ca_pem": "", "identidad": "consola-a", "cliente_id": "c", "equipo_id": "e",
            "secreto": "", "box_secret": "", "sign_seed": "", "sal_equipo": "",
        }))
        .unwrap();
        v.repos_v2.push(crate::gestion_v2::RepoV2 { id: "r1".into(), nombre: "Contabilidad".into(), ..Default::default() });
        v
    }

    #[test]
    fn textos_y_secretos() {
        assert_eq!(limpiar_texto("  Antes del cierre\r\n de mes ").unwrap(), "Antes del cierre\n de mes");
        assert!(limpiar_texto(&"a".repeat(MAX_TEXTO + 1)).is_err());
        assert!(limpiar_texto("con\u{7}campana").is_err());
        for s in ["contraseña: Verano2026", "la clave es 1234", "token=abc", "Xk9vQ2mZ7pL4rT8wY1nB5cD3", "-----BEGIN PRIVATE KEY-----"] {
            assert!(limpiar_texto(s).is_err(), "{s}");
        }
        for s in ["Versión buena antes de migrar el programa", "Ver https://ejemplo.com/una/ruta/bastante/larga/123", "Cierre 2025"] {
            assert!(limpiar_texto(s).is_ok(), "{s}");
        }
        assert!(limpiar_motivo("ab").is_err());
        assert_eq!(limpiar_motivo("  Cierre   contable 2025 ").unwrap(), "Cierre contable 2025");
        assert!(version_valida("3f2a9c1b") && !version_valida("3F2A9C1B") && !version_valida("3f2a") && !version_valida("3f2a9c1z"));
        assert!(misma_version("3f2a9c1b", "3f2a9c1bdeadbeef") && !misma_version("3f2a9c1b", "3f2a9c1c00") && !misma_version("3f2a", "3f2a"));
    }

    #[test]
    fn comentarios_de_quien_los_escribe() {
        let mut v = vinculo();
        let c = json!({ "repo": "r1", "version": "3f2a9c1b", "texto": "Antes del cierre", "autor": "cuenta-ana" });
        comentar(&mut v, &c, Some("Ana")).unwrap();
        assert_eq!(v.comentarios.len(), 1);
        let id = v.comentarios[0].id.clone();
        // Otra persona (u otra consola) no puede cambiarlo ni borrarlo.
        let otro = json!({ "repo": "r1", "version": "3f2a9c1b", "texto": "", "id": id, "autor": "cuenta-luis" });
        assert!(comentar(&mut v, &otro, Some("Luis")).is_err());
        let mut desde_otra = v.clone();
        desde_otra.identidad = "consola-b".into();
        assert!(comentar(&mut desde_otra, &json!({ "repo": "r1", "version": "3f2a9c1b", "texto": "x", "id": id, "autor": "cuenta-ana" }), None).is_err());
        // Quien lo escribió, sí.
        comentar(&mut v, &json!({ "repo": "r1", "version": "3f2a9c1b", "texto": "Antes del cierre de mes", "id": id, "autor": "cuenta-ana" }), Some("Ana"))
            .unwrap();
        assert_eq!(v.comentarios[0].texto, "Antes del cierre de mes");
        assert!(v.comentarios[0].editado.is_some());
        let r = resumen_comentarios(&v);
        assert_eq!(r[0]["esta"], true);
        assert_eq!(r[0]["por"], "Ana");
        comentar(&mut v, &json!({ "repo": "r1", "version": "3f2a9c1b", "texto": "", "id": id, "autor": "cuenta-ana" }), None).unwrap();
        assert!(v.comentarios.is_empty() && resumen_comentarios(&v).is_null());
        // Límites y lo que no vale.
        assert!(comentar(&mut v, &json!({ "repo": "otro", "version": "3f2a9c1b", "texto": "x" }), None).is_err());
        assert!(comentar(&mut v, &json!({ "repo": "r1", "version": "zz", "texto": "x" }), None).is_err());
        assert!(comentar(&mut v, &json!({ "repo": "r1", "version": "3f2a9c1b", "texto": "" }), None).is_err());
        for _ in 0..MAX_POR_VERSION {
            comentar(&mut v, &json!({ "repo": "r1", "version": "3f2a9c1b", "texto": "otro" }), None).unwrap();
        }
        assert!(comentar(&mut v, &json!({ "repo": "r1", "version": "3f2a9c1b", "texto": "uno más" }), None).is_err());
        assert_eq!(
            descripcion("comentario_version", &v, &json!({ "repo": "r1", "version": "3f2a9c1bdead", "texto": "x" })).unwrap(),
            "Comentar la versión 3f2a9c1b de «Contabilidad»"
        );
    }

    #[test]
    fn conservar_con_motivo_y_retencion() {
        let mut v = vinculo();
        assert!(conservar(&mut v, &json!({ "repo": "r1", "version": "3f2a9c1b", "conservar": true }), None).is_err(), "sin motivo, no");
        conservar(&mut v, &json!({ "repo": "r1", "version": "3f2a9c1b", "motivo": "Cierre contable 2025", "conservar": true }), Some("Ana")).unwrap();
        assert_eq!(conservadas_de(&v, "r1"), vec!["3f2a9c1b".to_string()]);
        assert_eq!(resumen_conservadas(&v)[0]["motivo"], "Cierre contable 2025");
        // La retención nunca la quita (por el id completo o el corto).
        let (quedan, salvadas) = sin_conservadas(vec!["3f2a9c1bdeadbeef".into(), "aaaaaaaa11".into()], &conservadas_de(&v, "r1"));
        assert_eq!((quedan, salvadas), (vec!["aaaaaaaa11".to_string()], 1));
        // Quitarla también pide motivo, y queda en el historial con él.
        assert!(conservar(&mut v, &json!({ "repo": "r1", "version": "3f2a9c1b", "conservar": false }), None).is_err());
        let c = json!({ "repo": "r1", "version": "3f2a9c1b", "motivo": "Ya no hace falta", "conservar": false });
        assert!(descripcion("conservar_version", &v, &c).unwrap().contains("Ya no hace falta"));
        conservar(&mut v, &c, None).unwrap();
        assert!(conservadas_de(&v, "r1").is_empty());
    }
}
