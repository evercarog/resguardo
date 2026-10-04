# Descarga el restic oficial que se incluye en el instalador y verifica su huella.
# Uso (desde la raíz del proyecto):  powershell -ExecutionPolicy Bypass -File scripts\fetch-restic.ps1
#
# Para cambiar de versión: actualiza $Version, $Sha256 (y $ExeSha256 aquí y en src-tauri/build.rs) con los valores de
# https://github.com/restic/restic/releases (archivo SHA256SUMS de esa versión).

$ErrorActionPreference = "Stop"
$Version = "0.19.1"
$Sha256 = "DA948AD707ED690426473AABA2046CD61F8F90F6F0E7DAB6BE0D5796531DE67D"  # restic_0.19.1_windows_amd64.zip
# Huella del .exe de dentro del zip (la comprueba también src-tauri/build.rs al compilar para publicar).
$ExeSha256 = "B0DD1FD21EEA5D8FE1325F55F7118213C21F36DE8A261E04C0624A5AB9FD7830"

$root = Split-Path -Parent $PSScriptRoot
$dest = Join-Path $root "src-tauri\binaries\restic-x86_64-pc-windows-msvc.exe"
$zip = Join-Path $env:TEMP "restic_${Version}_windows_amd64.zip"
$url = "https://github.com/restic/restic/releases/download/v$Version/restic_${Version}_windows_amd64.zip"

Write-Host "Descargando $url"
Invoke-WebRequest $url -OutFile $zip -UseBasicParsing

$hash = (Get-FileHash $zip -Algorithm SHA256).Hash
if ($hash -ne $Sha256) {
    Remove-Item $zip
    throw "La huella no coincide (esperada $Sha256, obtenida $hash). No se usa el archivo."
}

$tmp = Join-Path $env:TEMP "restic-$Version"
Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
Expand-Archive $zip -DestinationPath $tmp
New-Item -ItemType Directory -Force (Split-Path $dest) | Out-Null
$exe = (Get-ChildItem $tmp -Filter "*.exe" | Select-Object -First 1).FullName
$exeHash = (Get-FileHash $exe -Algorithm SHA256).Hash
if ($exeHash -ne $ExeSha256) {
    Remove-Item -Recurse -Force $tmp, $zip
    throw "La huella del ejecutable no coincide (esperada $ExeSha256, obtenida $exeHash). No se usa el archivo."
}
Copy-Item $exe $dest -Force
Remove-Item -Recurse -Force $tmp, $zip

& $dest version
Write-Host "Listo: $dest"
