# Descarga el rclone oficial que se incluye en el instalador del agente (espejo en la nube del
# Servidor de copias) y verifica sus huellas: la del zip (SHA256SUMS del proyecto) y la del .exe.
# Uso (desde la raíz del proyecto):  powershell -ExecutionPolicy Bypass -File scripts\fetch-rclone.ps1
#
# Para cambiar de versión: actualiza $Version, $Sha256, $ExeSha256 aquí y RCLONE_SHA256 en
# crates/agente/src/nube.rs (y las de Linux para los paquetes de Linux).

$ErrorActionPreference = "Stop"
$Version = "1.75.1"
$Sha256 = "200EB602C126D82AA38B51E0F6B9AE837473FF99B51278D3F6F837574C494D6E"     # rclone-v1.75.1-windows-amd64.zip
$ExeSha256 = "033EEE51C9AD47C2DE2624B6674D355274BCD6CF0027A5F85DB4437BA24AE81C"  # rclone.exe de dentro
# Linux (paquetes del agente): zip 982B5AA772841168F8E380F139E9E787B2A105403E32B94DA8676A0E1C0A13AB,
# rclone F66D8C1D552AD90296A11BC8B46D56A7FA5DA1A7FA05E7CA522D95DF92C4A4C0.

$root = Split-Path -Parent $PSScriptRoot
$dest = Join-Path $root "src-tauri\binaries\rclone-x86_64-pc-windows-msvc.exe"
$zip = Join-Path $env:TEMP "rclone-v$Version-windows-amd64.zip"
$url = "https://downloads.rclone.org/v$Version/rclone-v$Version-windows-amd64.zip"

Write-Host "Descargando $url"
Invoke-WebRequest $url -OutFile $zip -UseBasicParsing

$hash = (Get-FileHash $zip -Algorithm SHA256).Hash
if ($hash -ne $Sha256) {
    Remove-Item $zip
    throw "La huella no coincide (esperada $Sha256, obtenida $hash). No se usa el archivo."
}

$tmp = Join-Path $env:TEMP "rclone-$Version"
Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
Expand-Archive $zip -DestinationPath $tmp
New-Item -ItemType Directory -Force (Split-Path $dest) | Out-Null
$exe = (Get-ChildItem $tmp -Recurse -Filter "rclone.exe" | Select-Object -First 1).FullName
$exeHash = (Get-FileHash $exe -Algorithm SHA256).Hash
if ($exeHash -ne $ExeSha256) {
    Remove-Item -Recurse -Force $tmp, $zip
    throw "La huella del ejecutable no coincide (esperada $ExeSha256, obtenida $exeHash). No se usa el archivo."
}
Copy-Item $exe $dest -Force
Remove-Item -Recurse -Force $tmp, $zip

& $dest version
Write-Host "Listo: $dest"
