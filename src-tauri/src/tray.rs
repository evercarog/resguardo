//! Icono en la bandeja del sistema y sus ajustes.
//!
//! El escudo se pinta aquí (verde, ámbar o rojo según el estado general) y el
//! menú permite abrir la app, pedir «Copiar ahora» de una copia, pausar las
//! copias automáticas (abre la app: pide administrador y contraseña) y salir.
//! El estado lo calcula la interfaz (la misma lógica que el panel «Estado») y
//! lo manda con `tray_update`; el menú solo devuelve eventos a la interfaz, que
//! hace lo mismo que si se pulsara el botón en la ventana.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

const TRAY_ID: &str = "main";
/// Lado del icono en píxeles (Windows lo reduce a 16 o 20 según el DPI).
const ICON_SIZE: u32 = 32;

/// Ajustes de la bandeja (`bandeja.json`, por usuario). El inicio con
/// Windows no se guarda aquí: manda la entrada del registro (ver `autostart`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Settings {
    /// Al cerrar la ventana, la app sigue en la bandeja.
    #[serde(default = "yes")]
    pub close_to_tray: bool,
    /// Avisos de Windows (ver avisos.rs).
    #[serde(default = "yes")]
    pub notifications: bool,
}

fn yes() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Settings { close_to_tray: true, notifications: true }
    }
}

fn settings_path(dir: &Path) -> PathBuf {
    dir.join("bandeja.json")
}

/// Ajustes guardados; None si aún no hay (primera vez con esta versión).
pub fn load_settings(dir: &Path) -> Option<Settings> {
    let raw = fs::read(settings_path(dir)).ok()?;
    Some(serde_json::from_slice(&raw).unwrap_or_default())
}

pub fn save_settings(dir: &Path, settings: &Settings) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let data = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(settings_path(dir), data).map_err(|e| format!("No se pudo guardar el ajuste: {e}"))
}

/// Estado de la bandeja en esta sesión.
pub struct Tray {
    pub dir: PathBuf,
    pub settings: Mutex<Settings>,
    status: Mutex<Option<TrayStatus>>,
    /// Línea de progreso del menú: se cambia su texto sin rehacer el menú.
    progress_item: Mutex<Option<MenuItem<tauri::Wry>>>,
}

impl Tray {
    pub fn new(dir: PathBuf, settings: Settings) -> Self {
        Tray { dir, settings: Mutex::new(settings), status: Mutex::new(None), progress_item: Mutex::new(None) }
    }
    pub fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }
}

/// Lo que muestra la bandeja (lo manda la interfaz).
#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct TrayStatus {
    /// "ok", "warn", "bad" o "neutral".
    pub tone: String,
    /// «Resguardo: todo protegido», «Resguardo: 2 cosas necesitan atención»…
    pub tooltip: String,
    /// «Copiando «Laboral» · 45 %», «Subiendo a la nube «Disco» · 30 %»…
    #[serde(default)]
    pub progress: Option<String>,
    /// Copias de este equipo, para «Copiar ahora».
    #[serde(default)]
    pub copies: Vec<TrayCopy>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct TrayCopy {
    pub repo_id: String,
    pub plan_id: String,
    pub label: String,
}

/// Máximo de copias en el submenú (el resto, desde la app).
const MAX_COPIES: usize = 20;

/// Recorta lo que llega de la interfaz: textos cortos y tonos conocidos.
fn clean(mut status: TrayStatus) -> TrayStatus {
    let cut = |s: &str, n: usize| -> String {
        let s: String = s.chars().filter(|c| !c.is_control()).collect();
        if s.chars().count() > n {
            format!("{}…", s.chars().take(n - 1).collect::<String>())
        } else {
            s
        }
    };
    if !matches!(status.tone.as_str(), "ok" | "warn" | "bad" | "neutral") {
        status.tone = "neutral".into();
    }
    // Windows corta la descripción del icono a 127 caracteres.
    status.tooltip = cut(&status.tooltip, 120);
    status.progress = status.progress.as_deref().map(|p| cut(p, 80)).filter(|p| !p.is_empty());
    status.copies.truncate(MAX_COPIES);
    for c in &mut status.copies {
        c.label = cut(&c.label, 60);
    }
    status
}

// ---------- Icono ----------

/// Colores del escudo: los mismos tonos de la app, legibles sobre una barra de
/// tareas clara u oscura.
fn tone_color(tone: &str) -> [u8; 3] {
    match tone {
        "ok" => [0x16, 0xa3, 0x4a],
        "warn" => [0xe8, 0x8a, 0x0c],
        "bad" => [0xdc, 0x26, 0x26],
        _ => [0x7a, 0x7a, 0x86],
    }
}

/// ¿El punto (x, y), en [0, 1], cae dentro del escudo?
fn in_shield(x: f32, y: f32) -> bool {
    const TOP: f32 = 0.06;
    const MID: f32 = 0.52;
    const TIP: f32 = 0.96;
    const HALF: f32 = 0.40;
    if !(TOP..=TIP).contains(&y) {
        return false;
    }
    // Arriba, recto (con las esquinas un poco recortadas); abajo, en curva hasta la punta.
    let half = if y <= MID {
        let corner = 0.06 - (y - TOP);
        if corner > 0.0 {
            HALF - corner
        } else {
            HALF
        }
    } else {
        let t = (y - MID) / (TIP - MID);
        HALF * (1.0 - t * t)
    };
    (x - 0.5).abs() <= half
}

/// Distancia del punto p al segmento a-b.
fn dist_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0) };
    let (qx, qy) = (a.0 + t * dx, a.1 + t * dy);
    ((p.0 - qx).powi(2) + (p.1 - qy).powi(2)).sqrt()
}

/// ¿El punto cae en el símbolo blanco? Una marca de verificación si todo va
/// bien, una exclamación si algo necesita atención y una raya si aún no se sabe.
fn in_glyph(tone: &str, x: f32, y: f32) -> bool {
    match tone {
        "ok" => dist_segment((x, y), (0.31, 0.47), (0.45, 0.62)) < 0.065 || dist_segment((x, y), (0.45, 0.62), (0.70, 0.34)) < 0.065,
        "warn" | "bad" => dist_segment((x, y), (0.5, 0.24), (0.5, 0.50)) < 0.07 || ((x - 0.5).powi(2) + (y - 0.66).powi(2)).sqrt() < 0.075,
        _ => dist_segment((x, y), (0.34, 0.47), (0.66, 0.47)) < 0.06,
    }
}

/// Píxeles RGBA del escudo (con suavizado: 4×4 muestras por píxel).
pub fn icon_rgba(tone: &str, size: u32) -> Vec<u8> {
    const SS: u32 = 4;
    let [r, g, b] = tone_color(tone);
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for py in 0..size {
        for px in 0..size {
            let (mut shield, mut glyph) = (0u32, 0u32);
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = (px as f32 + (sx as f32 + 0.5) / SS as f32) / size as f32;
                    let y = (py as f32 + (sy as f32 + 0.5) / SS as f32) / size as f32;
                    if in_shield(x, y) {
                        shield += 1;
                        if in_glyph(tone, x, y) {
                            glyph += 1;
                        }
                    }
                }
            }
            let n = (SS * SS) as f32;
            let alpha = shield as f32 / n;
            // Mezcla del color del tono con blanco según cuánto ocupa el símbolo.
            let w = if shield == 0 { 0.0 } else { glyph as f32 / shield as f32 };
            let mix = |c: u8| (c as f32 * (1.0 - w) + 255.0 * w).round() as u8;
            out.extend_from_slice(&[mix(r), mix(g), mix(b), (alpha * 255.0).round() as u8]);
        }
    }
    out
}

fn icon(tone: &str) -> Image<'static> {
    Image::new_owned(icon_rgba(tone, ICON_SIZE), ICON_SIZE, ICON_SIZE)
}

// ---------- Menú ----------

type Built = (Menu<tauri::Wry>, Option<MenuItem<tauri::Wry>>);

fn menu(app: &AppHandle, status: Option<&TrayStatus>) -> tauri::Result<Built> {
    let menu = Menu::new(app)?;
    let mut progress = None;
    if let Some(s) = status {
        // Primera línea: el estado; debajo, lo que está en marcha. Solo informan.
        let headline = s.tooltip.strip_prefix("Resguardo: ").unwrap_or(&s.tooltip);
        let mut first: String = headline.chars().take(1).flat_map(char::to_uppercase).collect();
        first.extend(headline.chars().skip(1));
        menu.append(&MenuItem::with_id(app, "estado", first, false, None::<&str>)?)?;
        if let Some(p) = &s.progress {
            let item = MenuItem::with_id(app, "progreso", p, false, None::<&str>)?;
            menu.append(&item)?;
            progress = Some(item);
        }
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }
    menu.append(&MenuItem::with_id(app, "abrir", "Abrir Resguardo", true, None::<&str>)?)?;
    let copies = status.map(|s| s.copies.as_slice()).unwrap_or_default();
    let submenu = Submenu::with_id(app, "copiar", "Copiar ahora", !copies.is_empty())?;
    for (i, c) in copies.iter().enumerate() {
        submenu.append(&MenuItem::with_id(app, format!("copiar:{i}"), &c.label, true, None::<&str>)?)?;
    }
    menu.append(&submenu)?;
    menu.append(&MenuItem::with_id(app, "pausar", "Pausar copias automáticas 1 hora…", !copies.is_empty(), None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "salir", "Salir", true, None::<&str>)?)?;
    Ok((menu, progress))
}

/// Crea el icono de la bandeja (al arrancar la app).
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon("neutral"))
        .tooltip("Resguardo")
        .menu(&menu(app, None)?.0)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| on_menu(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Actualiza el icono, la descripción y el menú (solo si algo cambió).
pub fn update(app: &AppHandle, status: TrayStatus) -> Result<(), String> {
    let status = clean(status);
    let tray_state = app.state::<Tray>();
    let mut current = tray_state.status.lock().unwrap();
    if current.as_ref() == Some(&status) {
        return Ok(());
    }
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return Ok(()) };
    let e = |e: tauri::Error| e.to_string();
    if current.as_ref().map(|c| c.tone.as_str()) != Some(status.tone.as_str()) {
        tray.set_icon(Some(icon(&status.tone))).map_err(e)?;
    }
    let tooltip = match &status.progress {
        Some(p) => format!("{}\n{p}", status.tooltip),
        None => status.tooltip.clone(),
    };
    tray.set_tooltip(Some(tooltip)).map_err(e)?;
    // Si solo avanza el progreso, se cambia esa línea: rehacer el menú cada
    // segundo lo cerraría mientras se mira.
    let mut item = tray_state.progress_item.lock().unwrap();
    let only_progress = current.as_ref().is_some_and(|c| TrayStatus { progress: status.progress.clone(), ..c.clone() } == status);
    match (&*item, &status.progress) {
        (Some(line), Some(p)) if only_progress => line.set_text(p).map_err(e)?,
        _ => {
            let (built, progress) = menu(app, Some(&status)).map_err(e)?;
            tray.set_menu(Some(built)).map_err(e)?;
            *item = progress;
        }
    }
    *current = Some(status);
    Ok(())
}

/// Muestra y trae al frente la ventana principal.
pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn on_menu(app: &AppHandle, id: &str) {
    match id {
        "abrir" => show_main(app),
        "salir" => {
            crate::request_quit(app);
        }
        // Pausar pide administrador y la contraseña del destino: se hace en la app.
        "pausar" => crate::avisos::request_view(app, crate::avisos::Target::Pause),
        _ => {
            let Some(i) = id.strip_prefix("copiar:").and_then(|i| i.parse::<usize>().ok()) else { return };
            let copy = app.state::<Tray>().status.lock().unwrap().as_ref().and_then(|s| s.copies.get(i).cloned());
            let Some(copy) = copy else { return };
            // Con la app bloqueada no se hace nada sin desbloquearla antes.
            if app.state::<crate::applock::AppLock>().is_locked() {
                show_main(app);
                return;
            }
            let _ = app.emit("tray-copy", copy);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ajustes_por_defecto_y_guardados() {
        let dir = std::env::temp_dir().join(format!("resguardo-bandeja-{}", uuid::Uuid::new_v4()));
        assert!(load_settings(&dir).is_none());
        save_settings(&dir, &Settings { close_to_tray: false, notifications: true }).unwrap();
        assert_eq!(load_settings(&dir), Some(Settings { close_to_tray: false, notifications: true }));
        // Campos que faltan: activados.
        fs::write(settings_path(&dir), "{}").unwrap();
        assert_eq!(load_settings(&dir), Some(Settings::default()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_icono_tiene_escudo_y_simbolo() {
        for tone in ["ok", "warn", "bad", "neutral"] {
            let px = icon_rgba(tone, 32);
            assert_eq!(px.len(), 32 * 32 * 4);
            let at = |x: usize, y: usize| &px[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4];
            // Esquinas transparentes; el centro, opaco.
            assert_eq!(at(0, 0)[3], 0);
            assert_eq!(at(0, 31)[3], 0);
            assert_eq!(at(16, 8)[3], 255);
            // Color del tono en el borde del escudo.
            assert_eq!(&at(7, 10)[..3], &tone_color(tone)[..]);
        }
        // El símbolo es blanco.
        let ok = icon_rgba("ok", 32);
        assert!(ok.chunks(4).any(|p| p == [255, 255, 255, 255]));
    }

    #[test]
    fn limpia_lo_que_llega_de_la_interfaz() {
        let s = clean(TrayStatus {
            tone: "rosa".into(),
            tooltip: "x".repeat(300),
            progress: Some(String::new()),
            copies: (0..30).map(|i| TrayCopy { repo_id: "r".into(), plan_id: format!("p{i}"), label: "a\nb".into() }).collect(),
        });
        assert_eq!(s.tone, "neutral");
        assert_eq!(s.tooltip.chars().count(), 120);
        assert_eq!(s.progress, None);
        assert_eq!(s.copies.len(), MAX_COPIES);
        assert_eq!(s.copies[0].label, "ab");
    }
}
