//! Políticas de retención y su vista previa (`restic forget --dry-run`).
//!
//! La vista previa la calcula el propio restic, así coincide exactamente con
//! lo que haría `forget`. Se ejecuta con `--no-lock` y `--dry-run`: no
//! modifica nada ni bloquea el repositorio (funciona en servidores append-only).

use crate::restic::{self, Access, Snapshot};
use serde::{Deserialize, Serialize};

/// Valor de un campo de cantidad que significa «sin límite» (`unlimited` en restic).
pub const UNLIMITED: i64 = -1;

/// Política de retención. Los campos de cantidad son números de versiones
/// (0: no se usa; `UNLIMITED`: todas); los `keep_within*`, duraciones de restic
/// ("15d", "1y6m"). Las políticas guardadas por versiones anteriores (solo
/// cantidades u32 y `keep_within`) se leen igual.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Policy {
    #[serde(default)]
    pub keep_last: i64,
    #[serde(default)]
    pub keep_hourly: i64,
    #[serde(default)]
    pub keep_daily: i64,
    #[serde(default)]
    pub keep_weekly: i64,
    #[serde(default)]
    pub keep_monthly: i64,
    #[serde(default)]
    pub keep_yearly: i64,
    /// Conservar todo lo más reciente que esta duración, p. ej. "30d", "1y6m".
    #[serde(default)]
    pub keep_within: Option<String>,
    /// Por plazos: la última versión de cada hora, día, semana, mes o año
    /// dentro de esa duración (`--keep-within-hourly 15d`…).
    #[serde(default)]
    pub keep_within_hourly: Option<String>,
    #[serde(default)]
    pub keep_within_daily: Option<String>,
    #[serde(default)]
    pub keep_within_weekly: Option<String>,
    #[serde(default)]
    pub keep_within_monthly: Option<String>,
    #[serde(default)]
    pub keep_within_yearly: Option<String>,
    /// Cómo agrupa restic las versiones antes de aplicar la política:
    /// `None`, su predeterminado (por equipo y carpetas); `Some([])`, todas en un
    /// solo grupo (`--group-by ''`); o una combinación de "host", "paths" y
    /// "tags" (`--group-by host,tags`).
    #[serde(default)]
    pub group_by: Option<Vec<String>>,
    /// Aplicar solo a las versiones con alguna de estas etiquetas (un `--tag`
    /// por etiqueta: restic lo interpreta como «alguna»). Las demás no se tocan.
    #[serde(default)]
    pub filter_tags: Vec<String>,
    /// Aplicar solo a las versiones de este equipo (`--host`).
    #[serde(default)]
    pub filter_host: Option<String>,
    /// Aplicar solo a las versiones de estas carpetas (`--path`, una por carpeta).
    #[serde(default)]
    pub filter_paths: Vec<String>,
    /// Nunca borrar las versiones con alguna de estas etiquetas (`--keep-tag`).
    #[serde(default)]
    pub keep_tags: Vec<String>,
}

/// Criterios de agrupación que admite restic.
const GROUP_KEYS: [&str; 3] = ["host", "paths", "tags"];

/// Etiqueta, equipo o carpeta aptos para restic y para los comandos que se
/// muestran para copiar en una terminal: sin comillas, barras invertidas en
/// etiquetas, saltos de línea ni caracteres de control. Las etiquetas, además,
/// sin comas (restic las usaría como separador) ni espacios.
fn check_value(kind: &str, v: &str) -> Result<(), String> {
    let bad = |c: char| c.is_control() || matches!(c, '\'' | '"' | '`' | '$');
    if v.trim().is_empty() || v.chars().count() > 260 || v.chars().any(bad) || v.starts_with('-') {
        return Err(format!("{kind} no válido: «{v}» (sin comillas, $, ` ni saltos de línea)."));
    }
    if kind == "Etiqueta" && (v.contains(',') || v.chars().any(char::is_whitespace)) {
        return Err(format!("Etiqueta no válida: «{v}» (sin comas ni espacios)."));
    }
    Ok(())
}

impl Policy {
    /// Sin ninguna regla de plazos ni de cantidades. Agrupar, filtrar o
    /// proteger etiquetas no son una política por sí solos (con solo
    /// `--keep-tag`, restic borraría todo lo demás).
    pub fn is_empty(&self) -> bool {
        self.keep_args().is_empty()
    }

    /// ¿Se aplica solo a una parte de las versiones?
    pub fn has_filter(&self) -> bool {
        !self.filter_tags.is_empty() || self.filter_host.as_deref().is_some_and(|h| !h.is_empty()) || !self.filter_paths.is_empty()
    }

    /// ¿Esta versión entra en el filtro? (Misma regla que restic: alguna de las
    /// etiquetas, el equipo y alguna de las carpetas.)
    fn matches_filter(&self, s: &Snapshot) -> bool {
        let tags = self.filter_tags.is_empty() || self.filter_tags.iter().any(|t| s.tags.contains(t));
        let host = self.filter_host.as_deref().filter(|h| !h.is_empty()).is_none_or(|h| s.hostname == h);
        let paths = self.filter_paths.is_empty() || self.filter_paths.iter().any(|p| s.paths.contains(p));
        tags && host && paths
    }

    fn durations(&self) -> [(&'static str, Option<&str>); 6] {
        fn d(o: &Option<String>) -> Option<&str> {
            o.as_deref().filter(|w| !w.is_empty())
        }
        [
            ("--keep-within", d(&self.keep_within)),
            ("--keep-within-hourly", d(&self.keep_within_hourly)),
            ("--keep-within-daily", d(&self.keep_within_daily)),
            ("--keep-within-weekly", d(&self.keep_within_weekly)),
            ("--keep-within-monthly", d(&self.keep_within_monthly)),
            ("--keep-within-yearly", d(&self.keep_within_yearly)),
        ]
    }

    fn counts(&self) -> [(&'static str, i64); 6] {
        [
            ("--keep-last", self.keep_last),
            ("--keep-hourly", self.keep_hourly),
            ("--keep-daily", self.keep_daily),
            ("--keep-weekly", self.keep_weekly),
            ("--keep-monthly", self.keep_monthly),
            ("--keep-yearly", self.keep_yearly),
        ]
    }

    /// Solo las reglas `--keep-*`: plazos primero, después cantidades.
    fn keep_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        for (flag, within) in self.durations() {
            if let Some(w) = within {
                args.push(flag.to_string());
                args.push(w.to_string());
            }
        }
        for (flag, n) in self.counts() {
            if n == UNLIMITED {
                args.push(flag.to_string());
                args.push("unlimited".into());
            } else if n > 0 {
                args.push(flag.to_string());
                args.push(n.to_string());
            }
        }
        args
    }

    /// Opciones de restic para esta política: agrupación, filtro, reglas
    /// `--keep-*` y etiquetas protegidas. Cada valor es un argumento propio del
    /// proceso (sin intérprete de órdenes): un solo grupo es `--group-by` con
    /// una cadena vacía.
    pub fn args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if let Some(keys) = &self.group_by {
            args.push("--group-by".into());
            args.push(keys.join(","));
        }
        for t in &self.filter_tags {
            args.push("--tag".into());
            args.push(t.clone());
        }
        if let Some(h) = self.filter_host.as_deref().filter(|h| !h.is_empty()) {
            args.push("--host".into());
            args.push(h.to_string());
        }
        for p in &self.filter_paths {
            args.push("--path".into());
            args.push(p.clone());
        }
        args.extend(self.keep_args());
        for t in &self.keep_tags {
            args.push("--keep-tag".into());
            args.push(t.clone());
        }
        args
    }

    pub fn validate(&self) -> Result<(), String> {
        for (_, within) in self.durations() {
            if let Some(w) = within {
                if !valid_duration(w) {
                    return Err(format!("Duración no válida: {w}. Usa por ejemplo 30d, 6m o 1y."));
                }
            }
        }
        for (flag, n) in self.counts() {
            if !(UNLIMITED..=100_000).contains(&n) {
                return Err(format!("Cantidad no válida en {flag}: {n}."));
            }
        }
        if let Some(keys) = &self.group_by {
            let mut seen = Vec::new();
            for k in keys {
                if !GROUP_KEYS.contains(&k.as_str()) || seen.contains(k) {
                    return Err(format!("Agrupación no válida: {k}."));
                }
                seen.push(k.clone());
            }
        }
        for t in self.filter_tags.iter().chain(&self.keep_tags) {
            check_value("Etiqueta", t)?;
        }
        if let Some(h) = self.filter_host.as_deref().filter(|h| !h.is_empty()) {
            check_value("Equipo", h)?;
        }
        for p in &self.filter_paths {
            check_value("Carpeta", p)?;
        }
        if self.filter_tags.len() + self.keep_tags.len() + self.filter_paths.len() > 50 {
            return Err("Demasiadas etiquetas o carpetas en la política.".into());
        }
        Ok(())
    }
}

/// Duración de restic: uno o más pares número+unidad (y, m, d, h), p. ej. "1y6m".
fn valid_duration(s: &str) -> bool {
    let mut digits = false;
    let mut pairs = 0;
    for c in s.chars() {
        if c.is_ascii_digit() {
            digits = true;
        } else if matches!(c, 'y' | 'm' | 'd' | 'h') && digits {
            digits = false;
            pairs += 1;
        } else {
            return false;
        }
    }
    pairs > 0 && !digits && s.len() <= 16
}

#[derive(Deserialize)]
struct Reason {
    snapshot: Snapshot,
    #[serde(default, deserialize_with = "null_as_empty")]
    matches: Vec<String>,
}

#[derive(Deserialize)]
pub(crate) struct Group {
    #[serde(default, deserialize_with = "null_as_empty")]
    keep: Vec<Snapshot>,
    #[serde(default, deserialize_with = "null_as_empty")]
    remove: Vec<Snapshot>,
    #[serde(default, deserialize_with = "null_as_empty")]
    reasons: Vec<Reason>,
}

/// Los grupos de `restic forget --dry-run --json`: un array (puede venir tras líneas de texto).
pub(crate) fn grupos_de_forget(out: &[u8]) -> Result<Vec<Group>, String> {
    let text = String::from_utf8_lossy(out);
    let json = text.lines().find(|l| l.trim_start().starts_with('[')).unwrap_or("[]");
    serde_json::from_str(json).map_err(|e| format!("Respuesta inesperada de restic: {e}"))
}

/// restic escribe `null` en vez de `[]` cuando una lista está vacía (p. ej.
/// `"remove": null` si la política no quita nada).
fn null_as_empty<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(d)?.unwrap_or_default())
}

/// Motivo de las versiones que la política no toca por no entrar en su filtro
/// (restic no las incluye en la salida de `forget`).
pub const OUTSIDE_FILTER: &str = "outside filter";

#[derive(Serialize)]
pub struct PreviewItem {
    pub snapshot: Snapshot,
    pub keep: bool,
    /// Motivos de restic ("daily snapshot", "weekly snapshot"…, o
    /// `OUTSIDE_FILTER`); vacío si se elimina.
    pub reasons: Vec<String>,
}

#[derive(Serialize)]
pub struct Preview {
    pub items: Vec<PreviewItem>,
    /// Grupos de restic (por equipo y rutas); la política se aplica a cada uno por separado.
    pub groups: usize,
}

pub fn preview(access: &Access, policy: &Policy) -> Result<Preview, String> {
    policy.validate()?;
    if policy.is_empty() {
        return Err("Define al menos una regla para ver qué se conservaría.".into());
    }
    let mut args: Vec<String> = ["forget", "--dry-run", "--json", "--no-lock"].map(String::from).to_vec();
    args.extend(policy.args());
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = restic::run(access, &refs)?;
    let groups = grupos_de_forget(&out)?;

    let count = groups.len();
    let mut items = Vec::new();
    for g in groups {
        let mut reasons = std::collections::HashMap::new();
        for r in g.reasons {
            reasons.insert(r.snapshot.id, r.matches);
        }
        for s in g.keep {
            let why = reasons.remove(&s.id).unwrap_or_default();
            items.push(PreviewItem { snapshot: s, keep: true, reasons: why });
        }
        for s in g.remove {
            items.push(PreviewItem { snapshot: s, keep: false, reasons: vec![] });
        }
    }
    // Con filtro, restic solo lista las versiones que entran en él: las demás se
    // conservan siempre y se añaden para que la vista previa las muestre.
    if policy.has_filter() {
        let listed: std::collections::HashSet<String> = items.iter().map(|i| i.snapshot.id.clone()).collect();
        for s in restic::snapshots(access)? {
            if !listed.contains(&s.id) && !policy.matches_filter(&s) {
                items.push(PreviewItem { snapshot: s, keep: true, reasons: vec![OUTSIDE_FILTER.into()] });
            }
        }
    }
    items.sort_by(|a, b| b.snapshot.time.cmp(&a.snapshot.time));
    Ok(Preview { items, groups: count })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listas_nulas_de_restic() {
        // Así responde restic cuando la política no quita ninguna versión.
        let json = r#"[{"group_key":{"hostname":"","paths":null,"tags":null},"keep":[],"remove":null,"reasons":null}]"#;
        let groups: Vec<Group> = serde_json::from_str(json).unwrap();
        assert!(groups[0].remove.is_empty() && groups[0].reasons.is_empty());
    }

    #[test]
    fn argumentos_de_la_politica() {
        let p = Policy { keep_daily: 7, keep_monthly: 12, keep_within: Some("30d".into()), ..Default::default() };
        assert_eq!(p.args(), ["--keep-within", "30d", "--keep-daily", "7", "--keep-monthly", "12"]);
        assert!(Policy::default().is_empty());
        // Agrupar, filtrar o proteger sin reglas no es una política.
        assert!(Policy { group_by: Some(vec![]), keep_tags: vec!["conservar".into()], ..Default::default() }.is_empty());
        let p = Policy { keep_last: UNLIMITED, keep_within_weekly: Some("".into()), ..Default::default() };
        assert_eq!(p.args(), ["--keep-last", "unlimited"]);
    }

    #[test]
    fn politica_de_siigo() {
        // restic forget --group-by '' --keep-within-hourly 15d --keep-within-daily 1y --keep-monthly unlimited
        let p = Policy {
            keep_within_hourly: Some("15d".into()),
            keep_within_daily: Some("1y".into()),
            keep_monthly: UNLIMITED,
            group_by: Some(vec![]),
            ..Default::default()
        };
        assert!(p.validate().is_ok());
        assert_eq!(p.args(), ["--group-by", "", "--keep-within-hourly", "15d", "--keep-within-daily", "1y", "--keep-monthly", "unlimited"]);
    }

    #[test]
    fn agrupacion_filtro_y_etiquetas_protegidas() {
        let p = Policy {
            keep_within_daily: Some("60d".into()),
            keep_monthly: UNLIMITED,
            group_by: Some(vec!["host".into(), "tags".into()]),
            filter_tags: vec!["siigo".into(), "contabilidad".into()],
            filter_host: Some("SERVIDOR".into()),
            filter_paths: vec![r"D:\Siigo".into(), r"D:\Datos compartidos".into()],
            keep_tags: vec!["conservar".into()],
            ..Default::default()
        };
        assert!(p.validate().is_ok());
        assert_eq!(
            p.args(),
            [
                "--group-by",
                "host,tags",
                "--tag",
                "siigo",
                "--tag",
                "contabilidad",
                "--host",
                "SERVIDOR",
                "--path",
                r"D:\Siigo",
                "--path",
                r"D:\Datos compartidos",
                "--keep-within-daily",
                "60d",
                "--keep-monthly",
                "unlimited",
                "--keep-tag",
                "conservar",
            ]
        );
        assert!(p.has_filter());
        assert_eq!(Policy { group_by: Some(vec!["tags".into()]), keep_last: 3, ..Default::default() }.args(), ["--group-by", "tags", "--keep-last", "3"]);
        // Valores que romperían los comandos para copiar en una terminal.
        let bad = |f: fn(&mut Policy)| {
            let mut p = Policy { keep_last: 1, ..Default::default() };
            f(&mut p);
            p.validate().is_err()
        };
        assert!(bad(|p| p.group_by = Some(vec!["usuario".into()])));
        assert!(bad(|p| p.group_by = Some(vec!["host".into(), "host".into()])));
        assert!(bad(|p| p.filter_tags = vec!["a,b".into()]));
        assert!(bad(|p| p.keep_tags = vec!["con espacio".into()]));
        assert!(bad(|p| p.filter_host = Some("pc'; rm -rf /".into())));
        assert!(bad(|p| p.filter_paths = vec!["/datos\n/otra".into()]));
        assert!(bad(|p| p.filter_paths = vec!["$(whoami)".into()]));
        assert!(bad(|p| p.filter_tags = vec!["--help".into()]));
        assert!(!bad(|p| p.filter_paths = vec!["/srv/datos compartidos".into()]));
    }

    #[test]
    fn filtro_como_restic() {
        let snap = |tags: &[&str], host: &str, path: &str| Snapshot {
            id: "x".into(),
            short_id: "x".into(),
            time: String::new(),
            hostname: host.into(),
            username: None,
            paths: vec![path.into()],
            tags: tags.iter().map(|t| t.to_string()).collect(),
            excludes: vec![],
            parent: None,
            program_version: None,
            summary: None,
            original: None,
        };
        let p = Policy { filter_tags: vec!["siigo".into(), "diaria".into()], filter_host: Some("SRV".into()), ..Default::default() };
        assert!(p.matches_filter(&snap(&["diaria"], "SRV", "/a")), "alguna de las etiquetas");
        assert!(!p.matches_filter(&snap(&["otra"], "SRV", "/a")));
        assert!(!p.matches_filter(&snap(&["siigo"], "PC", "/a")));
        assert!(Policy::default().matches_filter(&snap(&[], "PC", "/a")));
    }

    #[test]
    fn politica_anterior_y_validacion() {
        // Guardada por una versión anterior (u32 y sin los campos nuevos).
        let old: Policy =
            serde_json::from_str(r#"{"keep_last":0,"keep_hourly":0,"keep_daily":7,"keep_weekly":4,"keep_monthly":12,"keep_yearly":3,"keep_within":null}"#)
                .unwrap();
        assert_eq!((old.keep_daily, old.keep_monthly, old.group_by.clone()), (7, 12, None));
        assert!(old.keep_within_daily.is_none());
        assert_eq!(old.args(), ["--keep-daily", "7", "--keep-weekly", "4", "--keep-monthly", "12", "--keep-yearly", "3"]);
        assert!(Policy { keep_within_daily: Some("1 año".into()), ..Default::default() }.validate().is_err());
        assert!(Policy { keep_within_hourly: Some("--help".into()), ..Default::default() }.validate().is_err());
        assert!(Policy { keep_daily: -2, ..Default::default() }.validate().is_err());
    }

    #[test]
    fn duraciones() {
        assert!(valid_duration("30d"));
        assert!(valid_duration("1y6m"));
        assert!(!valid_duration("30"));
        assert!(!valid_duration("d30"));
        assert!(!valid_duration("--help"));
        assert!(!valid_duration(""));
    }

    #[test]
    fn vista_previa_real() {
        let (Ok(repo), Ok(pw)) = (std::env::var("RESGUARDO_TEST_REPO"), std::env::var("RESGUARDO_TEST_PASSWORD")) else {
            return;
        };
        let access = Access::new(repo, pw);
        let total = restic::snapshots(&access).unwrap().len();
        let p = Policy { keep_last: 2, ..Default::default() };
        let preview = preview(&access, &p).unwrap();
        // Otro test puede añadir snapshots en paralelo: como mínimo están los contados antes.
        assert!(preview.items.len() >= total);
        assert_eq!(preview.items.iter().filter(|i| i.keep).count(), total.min(2));
        assert!(preview.items.iter().filter(|i| i.keep).all(|i| i.reasons.iter().any(|r| r.contains("last"))));
        // Un solo grupo (`--group-by ''` como argumento vacío) y reglas por plazos.
        let p = Policy {
            keep_within_hourly: Some("15d".into()),
            keep_within_daily: Some("1y".into()),
            keep_monthly: UNLIMITED,
            group_by: Some(vec![]),
            ..Default::default()
        };
        let grouped = super::preview(&access, &p).unwrap();
        assert_eq!(grouped.groups, 1);
        assert!(grouped.items.iter().any(|i| i.keep && i.reasons.iter().any(|r| r.contains("within"))));
        // Con filtro de etiquetas: lo que no la tiene se conserva y se lista.
        let p = Policy { keep_last: 1, filter_tags: vec!["prueba-plan".into()], ..Default::default() };
        let filtered = super::preview(&access, &p).unwrap();
        assert!(filtered.items.len() >= total);
        assert!(filtered.items.iter().filter(|i| !i.keep).all(|i| i.snapshot.tags.contains(&"prueba-plan".to_string())));
        assert!(filtered.items.iter().any(|i| i.reasons.iter().any(|r| r == OUTSIDE_FILTER)));
        // Que no deja bloqueos se comprueba a mano (`restic list locks`): aquí
        // otros tests crean bloqueos en paralelo sobre el mismo repositorio.
    }
}
