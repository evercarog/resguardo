/// SHA-256 de `binaries/restic-x86_64-pc-windows-msvc.exe` (restic 0.19.1
/// oficial, extraído de restic_0.19.1_windows_amd64.zip). Si cambias de
/// versión, actualízalo aquí y en scripts/fetch-restic.ps1.
const RESTIC_EXE_SHA256: &str = "b0dd1fd21eea5d8fe1325f55f7118213c21f36de8a261e04c0624a5ab9fd7830";

/// En las compilaciones de publicación, el restic incluido debe ser
/// exactamente el oficial: el agente lo ejecuta como SYSTEM.
fn check_bundled_restic() {
    use sha2::{Digest, Sha256};
    let path = std::path::Path::new("binaries").join("restic-x86_64-pc-windows-msvc.exe");
    println!("cargo:rerun-if-changed={}", path.display());
    if std::env::var("PROFILE").as_deref() != Ok("release") || std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("Falta {}: {e}. Ejecuta scripts/fetch-restic.ps1.", path.display()));
    let hash: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    if hash != RESTIC_EXE_SHA256 {
        panic!(
            "La huella de {} no es la del restic oficial (esperada {RESTIC_EXE_SHA256}, obtenida {hash}).              Vuelve a descargarlo con scripts/fetch-restic.ps1.",
            path.display()
        );
    }
}

fn main() {
    check_bundled_restic();
    let mut attributes = tauri_build::Attributes::new();

    // En Windows el diálogo nativo necesita Common Controls v6. tauri-build solo
    // incrusta ese manifiesto en el ejecutable de la app; lo incrustamos en todos
    // los binarios para que `cargo test` también funcione.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        attributes = attributes.windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        let manifest = std::env::current_dir().unwrap().join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed=windows-app-manifest.xml");
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }

    tauri_build::try_build(attributes).expect("failed to run build script");
}
