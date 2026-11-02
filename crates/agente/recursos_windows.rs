// Recursos de Windows del ejecutable (16z): datos de versión, fabricante,
// descripción, icono y manifiesto. Un .exe sin nada de esto parece más
// sospechoso a las heurísticas de los antivirus (Defender puso en cuarentena el
// agente 0.8.2). Lo usan los build.rs del agente y del servidor con `include!`.
//
// Solo hace algo al compilar para Windows; en Linux no hace nada.

/// Escribe el .rc y el manifiesto en OUT_DIR y los enlaza en el binario `bin`.
fn recursos_windows(bin: &str, producto: &str, descripcion: &str) {
    use std::path::Path;

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", Path::new(env!("CARGO_MANIFEST_DIR")).join("../agente/recursos_windows.rs").display());
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let manifiesto_crate = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    // El icono es el mismo de los instaladores (no se dibuja otro).
    let icono = Path::new(&manifiesto_crate).join("../../packaging/windows/arte/resguardo.ico");
    println!("cargo:rerun-if-changed={}", icono.display());
    let icono = icono.canonicalize().expect("el icono packaging/windows/arte/resguardo.ico");
    // rc.exe no entiende el prefijo \\?\ que deja canonicalize.
    let icono = icono.display().to_string().trim_start_matches(r"\\?\").replace('\\', "\\\\");

    let num = |v: &str| std::env::var(v).ok().and_then(|x| x.parse::<u16>().ok()).unwrap_or(0);
    let (ma, mi, pa) = (num("CARGO_PKG_VERSION_MAJOR"), num("CARGO_PKG_VERSION_MINOR"), num("CARGO_PKG_VERSION_PATCH"));
    let version = std::env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION");
    let out = std::env::var("OUT_DIR").expect("OUT_DIR");

    // El manifiesto: se ejecuta con los permisos de quien lo abre (el instalador
    // y el servicio ya llevan los suyos), declara Windows 10 y 11 y acepta rutas
    // largas si el sistema las tiene activadas.
    let manifiesto = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <longPathAware xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">true</longPathAware>
    </windowsSettings>
  </application>
</assembly>
"#;
    let ruta_manifiesto = Path::new(&out).join("resguardo.manifest");
    std::fs::write(&ruta_manifiesto, manifiesto).expect("escribir el manifiesto");
    let manifiesto_rc = ruta_manifiesto.display().to_string().replace('\\', "\\\\");

    // 1034 (0x040A) = español, como los datos de los instaladores.
    let rc = format!(
        r#"#pragma code_page(65001)
1 ICON "{icono}"
1 24 "{manifiesto_rc}"
1 VERSIONINFO
FILEVERSION {ma},{mi},{pa},0
PRODUCTVERSION {ma},{mi},{pa},0
FILEFLAGSMASK 0x3f
FILEFLAGS 0x0
FILEOS 0x40004
FILETYPE 0x1
FILESUBTYPE 0x0
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040A04B0"
    BEGIN
      VALUE "CompanyName", "Resguardo"
      VALUE "ProductName", "{producto}"
      VALUE "FileDescription", "{descripcion}"
      VALUE "FileVersion", "{version}"
      VALUE "ProductVersion", "{version}"
      VALUE "InternalName", "{bin}"
      VALUE "OriginalFilename", "{bin}.exe"
      VALUE "LegalCopyright", "© 2026 Ever Caro y colaboradores · AGPL-3.0-or-later"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x040A, 1200
  END
END
"#
    );
    let ruta_rc = Path::new(&out).join("resguardo.rc");
    std::fs::write(&ruta_rc, rc).expect("escribir el .rc");
    embed_resource::compile_for(&ruta_rc, [bin], embed_resource::NONE).manifest_required().expect("compilar los recursos de Windows");
}
