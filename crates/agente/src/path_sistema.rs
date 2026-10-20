//! 15o: la carpeta del programa en el `PATH` del sistema (Windows), para que
//! `resguardo-agente …` funcione en cualquier símbolo del sistema nuevo sin
//! escribir la ruta entera. Lo llama el instalador (`--anadir-al-path`) y el
//! desinstalador (`--quitar-del-path`).
//!
//! Lo hace el agente y no NSIS: NSIS lee las cadenas con un tope de longitud
//! (1024 caracteres en la compilación de siempre) y un `PATH` largo se cortaría
//! al volver a escribirlo. Aquí se lee el valor **sin expandir** (las entradas
//! con `%SystemRoot%` se quedan así), se conserva su tipo y solo se añade o se
//! quita la entrada de esta carpeta; las demás no se tocan, ni su orden.

/// Las entradas son iguales sin distinguir mayúsculas, espacios alrededor ni la
/// barra final (`C:\Program Files\Resguardo Agente\` = `c:\program files\resguardo agente`).
fn misma(entrada: &str, carpeta: &str) -> bool {
    let n = |s: &str| s.trim().trim_end_matches(['\\', '/']).to_lowercase();
    !entrada.trim().is_empty() && n(entrada) == n(carpeta)
}

/// El `PATH` con `carpeta` al final, o `None` si ya estaba (o la carpeta no vale).
pub fn con_carpeta(path: &str, carpeta: &str) -> Option<String> {
    if !valida(carpeta) || path.split(';').any(|e| misma(e, carpeta)) {
        return None;
    }
    let base = path.trim_end_matches(';');
    Some(if base.is_empty() { carpeta.to_string() } else { format!("{base};{carpeta}") })
}

/// El `PATH` sin las entradas de `carpeta`, o `None` si no estaba. Las demás
/// entradas se quedan tal cual (también las vacías que ya hubiera).
pub fn sin_carpeta(path: &str, carpeta: &str) -> Option<String> {
    if !path.split(';').any(|e| misma(e, carpeta)) {
        return None;
    }
    Some(path.split(';').filter(|e| !misma(e, carpeta)).collect::<Vec<_>>().join(";"))
}

/// Una carpeta que se puede poner en el `PATH`: absoluta, sin `;` ni comillas ni saltos.
fn valida(carpeta: &str) -> bool {
    let c = carpeta.trim();
    // Absoluta de Windows (`C:\…` o `\\servidor\…`), también al probarlo en Linux.
    let b = c.as_bytes();
    let absoluta = (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'\\' | b'/')) || c.starts_with(r"\\");
    absoluta && !c.contains([';', '"', '\r', '\n', '\0'])
}

/// La carpeta de este programa.
fn carpeta_programa() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| format!("No se sabe dónde está el programa: {e}"))?;
    let dir = exe.parent().ok_or("No se sabe en qué carpeta está el programa.")?;
    dir.to_str().map(str::to_string).ok_or_else(|| "La carpeta del programa tiene caracteres que no se pueden poner en el PATH.".into())
}

/// `--anadir-al-path`: 0 si quedó (o ya estaba), 1 si no se pudo.
pub fn anadir_main() -> i32 {
    cambiar(true)
}

/// `--quitar-del-path`: 0 si se quitó (o no estaba), 1 si no se pudo.
pub fn quitar_main() -> i32 {
    cambiar(false)
}

fn cambiar(anadir: bool) -> i32 {
    let r = carpeta_programa().and_then(|c| win::cambiar(&c, anadir));
    match r {
        Ok(()) => 0,
        Err(e) => {
            crate::agent::log(&format!("PATH del sistema: {e}"));
            1
        }
    }
}

#[cfg(not(windows))]
mod win {
    pub fn cambiar(_carpeta: &str, _anadir: bool) -> Result<(), String> {
        Err("Solo en Windows: en Linux el programa ya está en /usr/bin.".into())
    }
}

#[cfg(windows)]
mod win {
    use windows_sys::Win32::Foundation::{ERROR_SUCCESS, LPARAM, WPARAM};
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_EXPAND_SZ, REG_SZ,
        REG_VALUE_TYPE,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE};

    const CLAVE: &str = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";

    fn ancho(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    struct Clave(HKEY);
    impl Drop for Clave {
        fn drop(&mut self) {
            // SAFETY: la clave se abrió con RegOpenKeyExW y se cierra una sola vez.
            unsafe {
                RegCloseKey(self.0);
            }
        }
    }

    /// El valor `Path` tal cual (sin expandir) y su tipo.
    fn leer(k: &Clave, nombre: &[u16]) -> Result<(String, REG_VALUE_TYPE), String> {
        let mut tipo: REG_VALUE_TYPE = 0;
        let mut bytes: u32 = 0;
        // SAFETY: primero se pide el tamaño (sin búfer); después se lee en uno de ese tamaño.
        let r = unsafe { RegQueryValueExW(k.0, nombre.as_ptr(), std::ptr::null(), &mut tipo, std::ptr::null_mut(), &mut bytes) };
        if r != ERROR_SUCCESS {
            return Err(format!("no se pudo leer el PATH del sistema (error {r})"));
        }
        let mut buf = vec![0u16; (bytes as usize).div_ceil(2) + 1];
        let mut bytes2 = (buf.len() * 2) as u32;
        // SAFETY: `buf` tiene al menos `bytes2` bytes y vive durante la llamada.
        let r = unsafe { RegQueryValueExW(k.0, nombre.as_ptr(), std::ptr::null(), &mut tipo, buf.as_mut_ptr().cast(), &mut bytes2) };
        if r != ERROR_SUCCESS {
            return Err(format!("no se pudo leer el PATH del sistema (error {r})"));
        }
        if tipo != REG_EXPAND_SZ && tipo != REG_SZ {
            return Err("el PATH del sistema no es un texto: no se toca".into());
        }
        let n = (bytes2 as usize / 2).min(buf.len());
        let texto = &buf[..n];
        let fin = texto.iter().position(|&c| c == 0).unwrap_or(texto.len());
        Ok((String::from_utf16_lossy(&texto[..fin]), tipo))
    }

    pub fn cambiar(carpeta: &str, anadir: bool) -> Result<(), String> {
        let ruta = ancho(CLAVE);
        let nombre = ancho("Path");
        let mut h: HKEY = std::ptr::null_mut();
        // SAFETY: cadenas terminadas en cero; `h` recibe la clave abierta.
        let r = unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, ruta.as_ptr(), 0, KEY_QUERY_VALUE | KEY_SET_VALUE, &mut h) };
        if r != ERROR_SUCCESS {
            return Err(format!("no se pudo abrir el PATH del sistema (error {r}; ¿como administrador?)"));
        }
        let k = Clave(h);
        let (actual, tipo) = leer(&k, &nombre)?;
        let nuevo = if anadir { super::con_carpeta(&actual, carpeta) } else { super::sin_carpeta(&actual, carpeta) };
        let Some(nuevo) = nuevo else {
            return Ok(());
        };
        let datos = ancho(&nuevo);
        // SAFETY: `datos` (con su cero final) vive durante la llamada; el tamaño va en bytes.
        let r = unsafe { RegSetValueExW(k.0, nombre.as_ptr(), 0, tipo, datos.as_ptr().cast(), (datos.len() * 2) as u32) };
        if r != ERROR_SUCCESS {
            return Err(format!("no se pudo escribir el PATH del sistema (error {r})"));
        }
        // Que los programas que escuchan (el Explorador) tomen el PATH nuevo para lo que abran después.
        let entorno = ancho("Environment");
        let mut res: usize = 0;
        // SAFETY: mensaje de difusión estándar con una cadena que vive durante la llamada.
        unsafe {
            SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, 0 as WPARAM, entorno.as_ptr() as LPARAM, SMTO_ABORTIFHUNG, 5000, &mut res);
        }
        crate::agent::log(&if anadir { format!("PATH del sistema: añadida «{carpeta}».") } else { format!("PATH del sistema: quitada «{carpeta}».") });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const C: &str = r"C:\Program Files\Resguardo Agente";

    #[test]
    fn anade_al_final_sin_tocar_lo_demas() {
        assert_eq!(
            con_carpeta(r"%SystemRoot%\system32;%SystemRoot%;C:\Herramientas", C).as_deref(),
            Some(r"%SystemRoot%\system32;%SystemRoot%;C:\Herramientas;C:\Program Files\Resguardo Agente")
        );
        // Con un «;» al final, no queda una entrada vacía en medio.
        assert_eq!(con_carpeta(r"C:\A;", C).as_deref(), Some(r"C:\A;C:\Program Files\Resguardo Agente"));
        assert_eq!(con_carpeta("", C).as_deref(), Some(C));
    }

    #[test]
    fn no_anade_dos_veces() {
        assert_eq!(con_carpeta(r"C:\A;c:\program files\resguardo agente\;C:\B", C), None);
        assert_eq!(con_carpeta(&format!(r"C:\A; {C} "), C), None);
    }

    #[test]
    fn no_anade_carpetas_raras() {
        assert_eq!(con_carpeta(r"C:\A", r"C:\Uno;C:\Dos"), None);
        assert_eq!(con_carpeta(r"C:\A", "relativa"), None);
        assert_eq!(con_carpeta(r"C:\A", r#"C:\Con "comillas""#), None);
    }

    #[test]
    fn quita_solo_la_suya() {
        assert_eq!(
            sin_carpeta(r"%SystemRoot%\system32;C:\Program Files\Resguardo Agente\;C:\Herramientas", C).as_deref(),
            Some(r"%SystemRoot%\system32;C:\Herramientas")
        );
        // Repetida: se quitan todas; las entradas vacías ajenas se quedan.
        assert_eq!(sin_carpeta(&format!(r"C:\A;;{C};C:\B;{C}"), C).as_deref(), Some(r"C:\A;;C:\B"));
        // Una carpeta que solo empieza igual no es la suya.
        assert_eq!(sin_carpeta(r"C:\Program Files\Resguardo Agente 2;C:\A", C), None);
        assert_eq!(sin_carpeta(r"C:\A;C:\B", C), None);
    }

    #[test]
    fn ida_y_vuelta() {
        let path = r"%SystemRoot%\system32;%SystemRoot%;C:\Herramientas";
        let con = con_carpeta(path, C).unwrap();
        assert_eq!(sin_carpeta(&con, C).as_deref(), Some(path));
    }
}
