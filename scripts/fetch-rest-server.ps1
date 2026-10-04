# Descarga el rest-server oficial que se incluye en el instalador (servidor de copias) y verifica su huella.
# Uso (desde la raíz del proyecto):  powershell -ExecutionPolicy Bypass -File scripts\fetch-rest-server.ps1
#
# Para cambiar de versión: actualiza $Version, $Sha256 (del archivo SHA256SUMS de
# https://github.com/restic/rest-server/releases) y $ExeSha256 aquí y en src-tauri/src/server.rs.

$ErrorActionPreference = "Stop"
$Version = "0.14.0"
$Sha256 = "98E31E33816861722E0F04C7F56811A3FF2F37E006DEF217AD6EDA8E438C9D82"  # rest-server_0.14.0_windows_amd64.zip
# Huella del .exe de dentro del zip (la comprueba también la app antes de cada arranque).
$ExeSha256 = "66E185D33B4777BCC3B939085E48FC487DE597F7C8607727F82B6BE5A01D15F4"

$root = Split-Path -Parent $PSScriptRoot
$dest = Join-Path $root "src-tauri\binaries\rest-server-x86_64-pc-windows-msvc.exe"
$zip = Join-Path $env:TEMP "rest-server_${Version}_windows_amd64.zip"
$url = "https://github.com/restic/rest-server/releases/download/v$Version/rest-server_${Version}_windows_amd64.zip"

Write-Host "Descargando $url"
Invoke-WebRequest $url -OutFile $zip -UseBasicParsing

$hash = (Get-FileHash $zip -Algorithm SHA256).Hash
if ($hash -ne $Sha256) {
    Remove-Item $zip
    throw "La huella no coincide (esperada $Sha256, obtenida $hash). No se usa el archivo."
}

$tmp = Join-Path $env:TEMP "rest-server-$Version"
Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
Expand-Archive $zip -DestinationPath $tmp
New-Item -ItemType Directory -Force (Split-Path $dest) | Out-Null
$exe = (Get-ChildItem $tmp -Recurse -Filter "rest-server.exe" | Select-Object -First 1).FullName
$exeHash = (Get-FileHash $exe -Algorithm SHA256).Hash
if ($exeHash -ne $ExeSha256) {
    Remove-Item -Recurse -Force $tmp, $zip
    throw "La huella del ejecutable no coincide (esperada $ExeSha256, obtenida $exeHash). No se usa el archivo."
}
Copy-Item $exe $dest -Force
Remove-Item -Recurse -Force $tmp, $zip

& $dest --version
Write-Host "Listo: $dest"
