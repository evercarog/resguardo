//! Espejo del Servidor de copias (copia externa): cada noche, lo que guarda
//! el rest-server de este equipo se copia, archivo a archivo, a uno o varios
//! destinos: otra carpeta (p. ej. el segundo disco del almacén) o una nube
//! conectada con rclone (Dropbox, Google Drive; nube.rs, que sube con
//! `rclone copy --immutable`: nunca `sync`, nunca reescribe ni borra).
//!
//! Es seguro con repositorios de restic en «solo añadir»: sus archivos no
//! cambian una vez escritos (su nombre es su hash). Así que:
//! - nunca se borra nada en el espejo (lo borrado en el origen, que en solo
//!   añadir no debería pasar, sigue en el espejo);
//! - no se copian los `locks` ni lo modificado en los últimos 10 minutos (una
//!   subida a medias: entrará en la vuelta siguiente);
//! - cada archivo se copia a un nombre temporal y se renombra al final;
//! - un archivo que ya está con el mismo tamaño no se vuelve a copiar; si
//!   está con otro tamaño, tampoco se reemplaza (igual que en la nube con
//!   `--immutable`): alguien ha cambiado una copia ya escrita, y el espejo
//!   termina con error para que se revise.
//!
//! No necesita las contraseñas de los repositorios: no abre nada, solo copia
//! archivos cifrados.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Lo que se ha modificado hace menos de esto se deja para la próxima vuelta.
const RECIENTE: Duration = Duration::from_secs(10 * 60);

/// Un destino del espejo.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Destino {
    /// "carpeta" (ruta local completa) o "nube" (carpeta dentro de la nube `nube`).
    pub tipo: String,
    pub carpeta: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nube: Option<String>,
    #[serde(default)]
    pub ultima: Option<String>,
    #[serde(default)]
    pub resultado: Option<String>,
    /// v1.31: el espacio de la nube (`rclone about`) tras el último espejo a
    /// ella, y cuándo se leyó. Las carpetas se miden al hacer el resumen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cuota: Option<(crate::espacio::Espacio, String)>,
}

impl Destino {
    pub fn mismo(&self, o: &Destino) -> bool {
        self.tipo == o.tipo && self.carpeta == o.carpeta && self.nube == o.nube
    }
    pub fn texto(&self) -> String {
        match &self.nube {
            Some(n) => format!("{n}:{}", self.carpeta),
            None => self.carpeta.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Espejo {
    /// Forma de 0.7.0 (una sola carpeta): se lee y pasa a `destinos`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub carpeta: String,
    /// «HH:MM», hora local.
    pub hora: String,
    /// Cuándo terminó la última vuelta (RFC 3339; versiones anteriores: solo AAAA-MM-DD).
    #[serde(default)]
    pub ultima: Option<String>,
    #[serde(default)]
    pub resultado: Option<String>,
    #[serde(default)]
    pub destinos: Vec<Destino>,
    /// Límite de subida a la nube, en KiB/s (no se aplica a las carpetas).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limite_kib: Option<u32>,
}

impl Espejo {
    /// Los destinos, contando la forma antigua de una sola carpeta.
    pub fn destinos(&self) -> Vec<Destino> {
        let mut d = self.destinos.clone();
        if d.is_empty() && !self.carpeta.is_empty() {
            d.push(Destino {
                tipo: "carpeta".into(),
                carpeta: self.carpeta.clone(),
                nube: None,
                ultima: self.ultima.clone(),
                resultado: self.resultado.clone(),
                cuota: None,
            });
        }
        d
    }

    /// Pasa la forma antigua a `destinos`.
    pub fn normalizar(&mut self) {
        if self.destinos.is_empty() && !self.carpeta.is_empty() {
            self.destinos = self.destinos();
        }
        self.carpeta.clear();
    }

    /// ¿Quita `nuevo` alguno de los destinos de este espejo? (orden destructiva)
    pub fn quita_destinos(&self, nuevo: Option<&Espejo>) -> bool {
        let nuevos = nuevo.map(Espejo::destinos).unwrap_or_default();
        self.destinos().iter().any(|d| !nuevos.iter().any(|n| n.mismo(d)))
    }

    /// Lo que se ve en el resumen de la consola (sin secretos).
    pub fn resumen(&self) -> serde_json::Value {
        let destinos: Vec<_> = self
            .destinos()
            .iter()
            .map(|d| {
                // v1.31: libre y total (carpeta: su volumen, ahora; nube: la cuenta, tras el último espejo).
                let espacio = match (&d.tipo[..], &d.cuota) {
                    ("carpeta", _) => crate::espacio::json_de(&d.carpeta),
                    (_, Some((e, leido))) => e.json(leido),
                    _ => serde_json::Value::Null,
                };
                serde_json::json!({ "tipo": d.tipo, "carpeta": d.carpeta, "nube": d.nube, "ultima": d.ultima, "resultado": d.resultado, "espacio": espacio })
            })
            .collect();
        serde_json::json!({
            "hora": self.hora, "ultima": self.ultima, "resultado": self.resultado, "limite_kib": self.limite_kib, "destinos": destinos,
        })
    }
}

/// El espejo pedido en una orden `guarda_copias {espejo: …}`:
/// - `null` (o sin carpeta ni destinos): quitarlo;
/// - `{carpeta, hora}`: la forma de 0.7.0, una carpeta;
/// - `{destinos: [{tipo:"carpeta", carpeta} | {tipo:"nube", nube, carpeta}], hora, limite_kib?}`.
pub fn pedido(v: &serde_json::Value) -> Result<Option<Espejo>, String> {
    if v.is_null() {
        return Ok(None);
    }
    let hora = v["hora"].as_str().unwrap_or("02:00").to_string();
    let limite_kib = match &v["limite_kib"] {
        serde_json::Value::Null => None,
        l => Some(l.as_u64().and_then(|k| u32::try_from(k).ok()).ok_or("Límite de subida no válido (KiB/s).")?),
    }
    .filter(|k| *k > 0);
    let mut destinos = Vec::new();
    if let Some(c) = v["carpeta"].as_str() {
        destinos.push(Destino { tipo: "carpeta".into(), carpeta: c.into(), ..Default::default() });
    }
    if let Some(lista) = v.get("destinos").filter(|l| !l.is_null()) {
        for d in lista.as_array().ok_or("«destinos» tiene que ser una lista.")? {
            let carpeta = d["carpeta"].as_str().ok_or("A un destino del espejo le falta la carpeta.")?.to_string();
            match d["tipo"].as_str() {
                Some("carpeta") => destinos.push(Destino { tipo: "carpeta".into(), carpeta, ..Default::default() }),
                Some("nube") => {
                    let nube = d["nube"].as_str().ok_or("A un destino en la nube le falta el nombre de la nube.")?.to_string();
                    destinos.push(Destino { tipo: "nube".into(), carpeta, nube: Some(nube), ..Default::default() });
                }
                _ => return Err("Tipo de destino del espejo no válido (carpeta o nube).".into()),
            }
        }
    }
    if destinos.is_empty() {
        return Ok(None);
    }
    Ok(Some(Espejo { hora, destinos, limite_kib, ..Default::default() }))
}

#[derive(Debug, Default, PartialEq)]
pub struct Resumen {
    pub copiados: u64,
    pub bytes: u64,
    pub iguales: u64,
    pub recientes: u64,
    /// Ya estaban en el espejo con otro tamaño: no se reemplazan.
    pub distintos: u64,
}

/// Copia `origen` en `destino` con las reglas de arriba.
pub fn copiar(origen: &Path, destino: &Path) -> Result<Resumen, String> {
    copiar_con(origen, destino, &mut |_| {})
}

/// Lo mismo, diciendo los bytes copiados hasta ahora tras cada archivo
/// (la ventana del equipo saca de ahí el ritmo).
pub fn copiar_con(origen: &Path, destino: &Path, avance: &mut dyn FnMut(u64)) -> Result<Resumen, String> {
    let mut r = Resumen::default();
    let ahora = SystemTime::now();
    let mut pendientes: Vec<PathBuf> = vec![PathBuf::new()];
    while let Some(rel) = pendientes.pop() {
        let dir = origen.join(&rel);
        for e in std::fs::read_dir(&dir).map_err(|e| format!("No se pudo leer {}: {e}", dir.display()))?.flatten() {
            let nombre = e.file_name();
            if nombre == "locks" || nombre.to_string_lossy().ends_with(".tmp-espejo") {
                continue;
            }
            let Ok(m) = std::fs::symlink_metadata(e.path()) else { continue };
            let rel_e = rel.join(&nombre);
            if m.file_type().is_symlink() || crate::platform::is_reparse_point(&e.path()) {
                continue;
            }
            if m.is_dir() {
                // En el destino, nunca a través de un enlace (desviaría lo que escribe SYSTEM).
                if crate::platform::is_reparse_point(&destino.join(&rel_e)) {
                    return Err(format!("{} es un enlace: el espejo no escribe a través de enlaces.", destino.join(&rel_e).display()));
                }
                pendientes.push(rel_e);
                continue;
            }
            if m.modified().ok().and_then(|t| ahora.duration_since(t).ok()).is_none_or(|d| d < RECIENTE) {
                r.recientes += 1;
                continue;
            }
            let dest = destino.join(&rel_e);
            if let Ok(d) = std::fs::symlink_metadata(&dest) {
                // Nunca se reescribe lo que ya está: los archivos de restic no
                // cambian, así que otro tamaño es un daño (en el origen o aquí).
                if d.len() == m.len() && d.is_file() {
                    r.iguales += 1;
                } else {
                    r.distintos += 1;
                }
                continue;
            }
            if let Some(p) = dest.parent() {
                std::fs::create_dir_all(p).map_err(|e| format!("No se pudo crear {}: {e}", p.display()))?;
            }
            if crate::platform::is_reparse_point(&dest) {
                return Err(format!("{} es un enlace: el espejo no escribe a través de enlaces.", dest.display()));
            }
            let tmp = dest.with_file_name(format!("{}.tmp-espejo", nombre.to_string_lossy()));
            // Un temporal que ya estuviera (o un enlace con su nombre) se quita antes.
            if std::fs::symlink_metadata(&tmp).is_ok() {
                std::fs::remove_file(&tmp).map_err(|e| format!("No se pudo quitar {}: {e}", tmp.display()))?;
            }
            if let Err(err) = std::fs::copy(e.path(), &tmp) {
                // Lo copiado a medias no se queda (en un disco lleno, ocuparía lo poco que queda).
                let _ = std::fs::remove_file(&tmp);
                return Err(error_al_copiar(&err, destino));
            }
            std::fs::rename(&tmp, &dest).map_err(|e| format!("No se pudo terminar {}: {e}", dest.display()))?;
            r.copiados += 1;
            r.bytes += m.len();
            avance(r.bytes);
        }
    }
    Ok(r)
}

/// El motivo de un archivo que no se pudo copiar al espejo, con qué hacer.
fn error_al_copiar(e: &std::io::Error, destino: &Path) -> String {
    let lleno = e.kind() == std::io::ErrorKind::StorageFull
        || e.raw_os_error() == Some(if cfg!(windows) { 112 } else { 28 })
        || resguardo_motor::restic::sin_espacio(&e.to_string().to_lowercase());
    if lleno {
        format!(
            "no queda espacio en el disco del espejo ({}). Libera espacio en él o elige otra carpeta con más sitio; \
             lo que ya está en el espejo se conserva y lo que falta se copiará en la próxima vuelta.",
            destino.display()
        )
    } else {
        format!("no se pudo copiar a {}: {e}", destino.display())
    }
}

/// ¿Toca hoy? (pasada la hora y aún sin hacer hoy).
pub fn toca(e: &Espejo, ahora: chrono::DateTime<chrono::Local>) -> bool {
    let hoy = ahora.format("%Y-%m-%d").to_string();
    let Ok(h) = chrono::NaiveTime::parse_from_str(&e.hora, "%H:%M") else { return false };
    ahora.time() >= h && !e.ultima.as_deref().is_some_and(|u| u.starts_with(&hoy))
}

/// Copia a un destino, contando cómo va en `guarda` (la ventana del equipo: los
/// bytes copiados a una carpeta; lo que lee y sube rclone a una nube). Devuelve
/// el texto del resultado.
fn copiar_a(origen: &Path, d: &Destino, limite_kib: Option<u32>, guarda: &crate::escritorio::en_marcha::Guarda) -> Result<String, String> {
    if d.tipo == "nube" {
        let nombre = d.nube.as_deref().unwrap_or_default();
        let n = crate::nube::buscar(nombre).ok_or_else(|| format!("la nube «{nombre}» ya no está conectada en este equipo."))?;
        return crate::nube::copiar(&n, origen, &d.carpeta, limite_kib, &mut |l, s| guarda.ritmos(l, s));
    }
    // La carpeta de destino: local, sin enlaces en el camino y de Administradores.
    crate::platform::carpeta_local_valida(&d.carpeta)?;
    let destino = Path::new(&d.carpeta);
    // En pruebas (RESGUARDO_AGENT_DIR, sin administrador) la carpeta es de quien
    // corre la prueba, como en `carpeta_privada`.
    if destino.exists() && !crate::agent::test_mode() && !crate::platform::owned_by_admins(destino) {
        return Err("la carpeta del espejo no es de Administradores (vuelve a poner el espejo para corregirla).".into());
    }
    let r = copiar_con(origen, destino, &mut |b| guarda.progreso(Some(b), None))?;
    let texto =
        format!("{} archivos nuevos ({} MB), {} ya estaban, {} se dejan para la próxima vez.", r.copiados, r.bytes / (1024 * 1024), r.iguales, r.recientes);
    if r.distintos > 0 {
        return Err(format!(
            "{texto} Pero {} ya estaban en el espejo con otro tamaño y no se han reemplazado: alguien ha cambiado copias ya escritas (en el Servidor de copias o en el espejo). Revísalo.",
            r.distintos
        ));
    }
    Ok(texto)
}

/// Lo llama el servicio en cada vuelta: si toca, hace el espejo (en otro hilo) y anota el resultado.
pub fn si_toca() {
    let c = crate::server::load();
    let Some(e) = c.espejo.clone().filter(|_| c.enabled) else { return };
    let ahora = chrono::Local::now();
    if !toca(&e, ahora) {
        return;
    }
    static EN_MARCHA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if EN_MARCHA.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        let mut hechos: Vec<(Destino, String, Option<crate::espacio::Espacio>)> = Vec::new();
        let mut errores = 0;
        for (i, d) in e.destinos().into_iter().enumerate() {
            // Para la ventana y los avisos del escritorio: sin la carpeta (es una ruta).
            let (tipo, nombre) = match d.nube.as_deref().filter(|_| d.tipo == "nube") {
                Some(n) => ("nube", n.to_string()),
                None => ("espejo", "Disco o carpeta del equipo".to_string()),
            };
            let guarda = crate::escritorio::en_marcha::empezar(tipo, &i.to_string(), &nombre);
            let texto = match copiar_a(Path::new(&c.path), &d, e.limite_kib, &guarda) {
                Ok(t) => {
                    guarda.terminar("ok");
                    format!("Espejo hecho en {}: {t}", d.texto())
                }
                Err(m) => {
                    errores += 1;
                    drop(guarda);
                    format!("ERROR: espejo del Servidor de copias en {}: {m}", d.texto())
                }
            };
            crate::agent::log(&texto);
            // v1.31: el espacio de la nube, una vez por noche (para «¿Cuándo se llena?»).
            let cuota = if d.tipo == "nube" { d.nube.as_deref().and_then(crate::nube::buscar).and_then(|n| crate::nube::cuota(&n)) } else { None };
            hechos.push((d, texto, cuota));
        }
        // Se vuelve a leer: la configuración pudo cambiar mientras se copiaba.
        let mut c = crate::server::load();
        if let Some(esp) = c.espejo.as_mut() {
            esp.normalizar();
            let fin = chrono::Local::now().to_rfc3339();
            for (d, texto, cuota) in &hechos {
                if let Some(x) = esp.destinos.iter_mut().find(|x| x.mismo(d)) {
                    x.ultima = Some(fin.clone());
                    x.resultado = Some(texto.clone());
                    if let Some(q) = cuota {
                        x.cuota = Some((*q, fin.clone()));
                    }
                }
            }
            esp.ultima = Some(fin);
            esp.resultado = Some(match (hechos.len(), errores) {
                (1, _) => hechos[0].1.clone(),
                (n, 0) => format!("Espejo hecho en los {n} destinos."),
                (n, f) => format!("ERROR: espejo del Servidor de copias: {f} de {n} destinos con error."),
            });
            // También en la bitácora del equipo (para una consola nueva), sin rutas.
            if let (Some(u), Some(r)) = (&esp.ultima, &esp.resultado) {
                crate::bitacora::espejo(u, r);
            }
        }
        let _ = crate::server::save(&c);
        EN_MARCHA.store(false, std::sync::atomic::Ordering::SeqCst);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copia_sin_locks_ni_recientes_y_sin_borrar() {
        let base = std::env::temp_dir().join(format!("resguardo-espejo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (o, d) = (base.join("origen"), base.join("destino"));
        std::fs::create_dir_all(o.join("ana/repo/data/ab")).unwrap();
        std::fs::create_dir_all(o.join("ana/repo/locks")).unwrap();
        std::fs::write(o.join("ana/repo/config"), b"config").unwrap();
        std::fs::write(o.join("ana/repo/data/ab/abcdef"), b"datos").unwrap();
        std::fs::write(o.join("ana/repo/locks/l1"), b"lock").unwrap();
        // Todo «reciente»: nada se copia todavía.
        let r = copiar(&o, &d).unwrap();
        assert_eq!((r.copiados, r.recientes), (0, 2));
        // Con fecha antigua, sí (menos el lock).
        let viejo = SystemTime::now() - Duration::from_secs(3600);
        for f in ["ana/repo/config", "ana/repo/data/ab/abcdef"] {
            std::fs::File::options().write(true).open(o.join(f)).unwrap().set_modified(viejo).unwrap();
        }
        let mut avances = Vec::new();
        let r = copiar_con(&o, &d, &mut |b| avances.push(b)).unwrap();
        assert_eq!((r.copiados, r.bytes), (2, 11));
        // Los bytes copiados, tras cada archivo (de ahí sale el ritmo en la ventana).
        assert_eq!(avances.len(), 2);
        assert_eq!(avances.last(), Some(&11));
        assert!(!d.join("ana/repo/locks").exists());
        assert_eq!(std::fs::read(d.join("ana/repo/data/ab/abcdef")).unwrap(), b"datos");
        // Otra vuelta: nada nuevo. Y lo borrado en el origen sigue en el espejo.
        std::fs::remove_file(o.join("ana/repo/config")).unwrap();
        let r = copiar(&o, &d).unwrap();
        assert_eq!((r.copiados, r.iguales), (0, 1));
        assert!(d.join("ana/repo/config").is_file());
        // Un archivo cambiado en el origen (otro tamaño) no reemplaza al del espejo.
        let f = o.join("ana/repo/data/ab/abcdef");
        std::fs::write(&f, b"datos cambiados").unwrap();
        std::fs::File::options().write(true).open(&f).unwrap().set_modified(viejo).unwrap();
        let r = copiar(&o, &d).unwrap();
        assert_eq!((r.copiados, r.distintos), (0, 1));
        assert_eq!(std::fs::read(d.join("ana/repo/data/ab/abcdef")).unwrap(), b"datos");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Un enlace puesto en el destino (por un usuario) no se sigue: el espejo se para con error.
    #[cfg(windows)]
    #[test]
    fn no_escribe_a_traves_de_enlaces() {
        let base = std::env::temp_dir().join(format!("resguardo-espejo-enlace-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (o, d, otra) = (base.join("origen"), base.join("destino"), base.join("otra"));
        std::fs::create_dir_all(o.join("ana/repo")).unwrap();
        std::fs::create_dir_all(&d).unwrap();
        std::fs::create_dir_all(&otra).unwrap();
        let f = o.join("ana/repo/config");
        std::fs::write(&f, b"config").unwrap();
        std::fs::File::options().write(true).open(&f).unwrap().set_modified(SystemTime::now() - Duration::from_secs(3600)).unwrap();
        let ok = std::process::Command::new(crate::platform::system_tool("cmd.exe"))
            .args(["/c", "mklink", "/J"])
            .arg(d.join("ana"))
            .arg(&otra)
            .output()
            .is_ok_and(|o| o.status.success());
        if ok {
            assert!(copiar(&o, &d).unwrap_err().contains("enlace"));
            assert!(std::fs::read_dir(&otra).unwrap().next().is_none(), "nada escrito a través del enlace");
        }
        let _ = std::fs::remove_dir(d.join("ana"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn hora_del_espejo() {
        use chrono::TimeZone;
        let e = Espejo { carpeta: "x".into(), hora: "02:00".into(), ..Default::default() };
        assert_eq!(e.destinos().len(), 1, "forma de 0.7.0");
        let t = chrono::Local.with_ymd_and_hms(2026, 10, 2, 1, 59, 0).unwrap();
        assert!(!toca(&e, t));
        let t = chrono::Local.with_ymd_and_hms(2026, 10, 2, 2, 0, 0).unwrap();
        assert!(toca(&e, t));
        let hecha = Espejo { ultima: Some("2026-10-02T02:03:00-05:00".into()), ..e };
        assert!(!toca(&hecha, t));
    }

    #[test]
    fn pedidos_y_destinos_quitados() {
        use serde_json::json;
        assert_eq!(pedido(&json!(null)).unwrap(), None);
        assert_eq!(pedido(&json!({"carpeta": null})).unwrap(), None);
        let viejo = pedido(&json!({"carpeta": "E:\\espejo", "hora": "03:00"})).unwrap().unwrap();
        assert_eq!((viejo.hora.as_str(), viejo.destinos[0].tipo.as_str()), ("03:00", "carpeta"));
        let dos = pedido(&json!({"destinos": [{"tipo": "carpeta", "carpeta": "E:\\espejo"}, {"tipo": "nube", "nube": "Dropbox Altamar", "carpeta": "Resguardo/Sur"}], "limite_kib": 512}))
            .unwrap()
            .unwrap();
        assert_eq!((dos.destinos.len(), dos.limite_kib, dos.hora.as_str()), (2, Some(512), "02:00"));
        assert!(pedido(&json!({"destinos": [{"tipo": "ftp", "carpeta": "x"}]})).is_err());
        assert!(pedido(&json!({"destinos": [{"tipo": "nube", "carpeta": "x"}]})).is_err());
        // Añadir un destino no quita nada; quitar uno, o el espejo entero, sí.
        let antiguo = Espejo { carpeta: "E:\\espejo".into(), hora: "02:00".into(), ..Default::default() };
        assert!(!antiguo.quita_destinos(Some(&dos)));
        assert!(dos.quita_destinos(Some(&viejo)));
        assert!(antiguo.quita_destinos(None));
        let mut n = antiguo.clone();
        n.normalizar();
        assert_eq!((n.carpeta.as_str(), n.destinos.len()), ("", 1));
        let r = dos.resumen();
        assert_eq!(r["destinos"][1]["nube"], "Dropbox Altamar");
        assert_eq!(r["limite_kib"], 512);
    }
}
