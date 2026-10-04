//! Bloqueo de la app con Windows Hello o la contraseña de Windows.
//!
//! En un equipo compartido, quien se siente delante podría explorar y
//! restaurar archivos de otros con Resguardo. Con el bloqueo activado, la app
//! se abre bloqueada y solo se desbloquea tras pasar la comprobación de
//! Windows (`UserConsentVerifier`: huella, cara, PIN o la contraseña, según lo
//! que el usuario tenga configurado en Windows Hello). No hay PIN propio.
//!
//! Solo afecta a la ventana: el agente (SYSTEM) no lee este ajuste.
//! Mientras está bloqueada, el backend rechaza cualquier comando salvo los de
//! esta lista (ver `allowed_while_locked`).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// Minutos sin usar la app tras los que se puede volver a bloquear.
pub const IDLE_CHOICES: [u32; 5] = [5, 10, 15, 30, 60];

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct LockConfig {
    #[serde(default)]
    pub enabled: bool,
    /// Volver a pedirla tras N minutos sin usar la app (None: solo al abrir).
    #[serde(default)]
    pub idle_minutes: Option<u32>,
}

/// Estado del bloqueo en esta ventana.
pub struct AppLock {
    dir: PathBuf,
    config: Mutex<LockConfig>,
    locked: AtomicBool,
}

impl AppLock {
    /// Se abre bloqueada si el bloqueo está activado.
    pub fn new(dir: PathBuf) -> Self {
        let config = load(&dir);
        let locked = AtomicBool::new(config.enabled);
        AppLock { dir, config: Mutex::new(config), locked }
    }
    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::SeqCst)
    }
    pub fn config(&self) -> LockConfig {
        self.config.lock().unwrap().clone()
    }
    /// Bloquea ya (tras un rato sin usarla), si el bloqueo está activado.
    pub fn lock(&self) {
        if self.config().enabled {
            self.locked.store(true, Ordering::SeqCst);
        }
    }
    pub fn unlock(&self) {
        self.locked.store(false, Ordering::SeqCst);
    }
    pub fn set(&self, config: LockConfig) -> Result<(), String> {
        save(&self.dir, &config)?;
        *self.config.lock().unwrap() = config;
        Ok(())
    }
}

fn path(dir: &Path) -> PathBuf {
    dir.join("bloqueo.json")
}

pub fn load(dir: &Path) -> LockConfig {
    std::fs::read(path(dir)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save(dir: &Path, config: &LockConfig) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = path(dir).with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(config).unwrap()).map_err(|e| format!("No se pudo guardar el ajuste: {e}"))?;
    std::fs::rename(&tmp, path(dir)).map_err(|e| format!("No se pudo guardar el ajuste: {e}"))
}

pub fn validate(config: &LockConfig) -> Result<(), String> {
    match config.idle_minutes {
        Some(m) if !IDLE_CHOICES.contains(&m) => Err("Ese tiempo sin usar la app no es válido.".into()),
        _ => Ok(()),
    }
}

/// Comandos que se pueden usar con la app bloqueada (no muestran ni cambian nada).
pub fn allowed_while_locked(command: &str) -> bool {
    matches!(command, "app_lock_status" | "app_lock_unlock" | "restic_version")
}

/// ¿Se puede pedir la comprobación de Windows en este equipo y usuario?
#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    Available,
    /// Windows Hello no está configurado para este usuario.
    NotConfigured,
    DisabledByPolicy,
    DeviceNotPresent,
    DeviceBusy,
    /// Otro sistema, o Windows no respondió.
    Unsupported,
}

impl Availability {
    pub fn explain(self) -> &'static str {
        match self {
            Availability::Available => "",
            Availability::NotConfigured => {
                "Windows Hello no está configurado para tu usuario. Configura un PIN, huella o reconocimiento facial en Configuración de Windows → Cuentas → Opciones de inicio de sesión."
            }
            Availability::DisabledByPolicy => "Windows Hello está desactivado por una directiva de este equipo.",
            Availability::DeviceNotPresent => "No hay ningún método de Windows Hello disponible en este equipo.",
            Availability::DeviceBusy => "Windows Hello está ocupado. Inténtalo de nuevo en un momento.",
            Availability::Unsupported => "Este equipo no permite pedir Windows Hello.",
        }
    }
}

/// Resultado de pedir la comprobación.
#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Verification {
    Verified,
    Canceled,
    RetriesExhausted,
    /// No se pudo pedir (ver `Availability`).
    Unavailable,
}

#[cfg(windows)]
mod win {
    use super::{Availability, Verification};
    use windows::core::{factory, HSTRING};
    use windows::Security::Credentials::UI::{UserConsentVerificationResult as R, UserConsentVerifier, UserConsentVerifierAvailability as A};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::WinRT::IUserConsentVerifierInterop;
    use windows_future::IAsyncOperation;

    pub fn availability() -> Availability {
        match UserConsentVerifier::CheckAvailabilityAsync().and_then(|op| op.join()) {
            Ok(a) if a == A::Available => Availability::Available,
            Ok(a) if a == A::NotConfiguredForUser => Availability::NotConfigured,
            Ok(a) if a == A::DisabledByPolicy => Availability::DisabledByPolicy,
            Ok(a) if a == A::DeviceNotPresent => Availability::DeviceNotPresent,
            Ok(a) if a == A::DeviceBusy => Availability::DeviceBusy,
            _ => Availability::Unsupported,
        }
    }

    /// Pide la comprobación con el diálogo de Windows delante de `hwnd`.
    pub fn verify(hwnd: isize, message: &str) -> Result<Verification, String> {
        let interop = factory::<UserConsentVerifier, IUserConsentVerifierInterop>().map_err(|e| e.message())?;
        let op: IAsyncOperation<R> = unsafe { interop.RequestVerificationForWindowAsync(HWND(hwnd as _), &HSTRING::from(message)) }.map_err(|e| e.message())?;
        let r = op.join().map_err(|e| e.message())?;
        Ok(if r == R::Verified {
            Verification::Verified
        } else if r == R::Canceled {
            Verification::Canceled
        } else if r == R::RetriesExhausted {
            Verification::RetriesExhausted
        } else {
            Verification::Unavailable
        })
    }
}

#[cfg(windows)]
pub use win::{availability, verify};

#[cfg(not(windows))]
pub fn availability() -> Availability {
    Availability::Unsupported
}

#[cfg(not(windows))]
pub fn verify(_hwnd: isize, _message: &str) -> Result<Verification, String> {
    Ok(Verification::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ajuste_por_defecto_y_guardado() {
        let dir = std::env::temp_dir().join(format!("resguardo-bloqueo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // Sin archivo: desactivado y la app no se abre bloqueada.
        let lock = AppLock::new(dir.clone());
        assert_eq!(lock.config(), LockConfig::default());
        assert!(!lock.is_locked());
        lock.lock();
        assert!(!lock.is_locked(), "sin bloqueo activado, nunca se bloquea");

        let on = LockConfig { enabled: true, idle_minutes: Some(15) };
        lock.set(on.clone()).unwrap();
        // Al volver a abrir, se abre bloqueada.
        let again = AppLock::new(dir.clone());
        assert_eq!(again.config(), on);
        assert!(again.is_locked());
        again.unlock();
        assert!(!again.is_locked());
        again.lock();
        assert!(again.is_locked());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validacion_y_comandos_permitidos() {
        assert!(validate(&LockConfig { enabled: true, idle_minutes: Some(15) }).is_ok());
        assert!(validate(&LockConfig { enabled: true, idle_minutes: None }).is_ok());
        assert!(validate(&LockConfig { enabled: true, idle_minutes: Some(0) }).is_err());
        assert!(validate(&LockConfig { enabled: true, idle_minutes: Some(7) }).is_err());
        assert!(allowed_while_locked("app_lock_unlock"));
        for cmd in ["list_repos", "list_snapshots", "list_snapshot_dir", "run_restore", "search_files", "app_lock_set", "app_lock_lock"] {
            assert!(!allowed_while_locked(cmd), "{cmd}");
        }
    }

    /// Solo comprueba que la consulta a Windows no falla ni se queda colgada.
    #[test]
    fn disponibilidad_responde() {
        eprintln!("Windows Hello: {:?}", availability());
    }
}
