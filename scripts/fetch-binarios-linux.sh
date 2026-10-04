#!/bin/sh
# Descarga los binarios oficiales que acompañan al agente de Linux (x86_64) y
# comprueba sus huellas SHA-256 fijadas: la del archivo publicado (la de su
# SHA256SUMS) y la del binario de dentro.
#
#   scripts/fetch-binarios-linux.sh <carpeta> [x86_64|aarch64]
#
# x86_64 (por defecto): deja <carpeta>/restic, rest-server y rclone.
# aarch64 (arm64, p. ej. Oracle Cloud Ampere): solo restic, comprobado por la
# huella del archivo publicado (la de su SHA256SUMS). rest-server y rclone no
# tienen todavía huella fijada en el agente para arm64 («Este equipo guarda
# copias» y el espejo en la nube no se usan ahí; ver crates/agente/src/server.rs).
#
# - restic 0.19.1 (el mismo que el instalador de Windows): las copias. El
#   agente usa el que está junto a él, nunca el del PATH.
# - rest-server 0.14.0: «Este equipo guarda copias». El agente comprueba su
#   huella antes de cada arranque (PINNED_SHA256 en crates/agente/src/server.rs).
# - rclone 1.75.1: espejo del Servidor de copias en una nube (RCLONE_SHA256 en
#   crates/agente/src/nube.rs).
#
# Todo desde las publicaciones oficiales de GitHub. Para cambiar de versión:
# actualiza aquí, en server.rs o nube.rs (las del binario) y en
# packaging/linux/construir-paquetes.sh (que vuelve a comprobarlas).
set -eu

[ $# -eq 1 ] || [ $# -eq 2 ] || { echo "Uso: $0 <carpeta> [x86_64|aarch64]" >&2; exit 2; }
DESTINO="$1"
ARQ="${2:-x86_64}"
case "$ARQ" in x86_64 | aarch64) ;; *) echo "Arquitectura no soportada: $ARQ" >&2; exit 2 ;; esac

RESTIC_VERSION="0.19.1"
RESTIC_ARCHIVO_SHA256="f415415624dcc452f2a02b8c33641791a8c6d6d3b65bbb3543fcf9a25151585c"   # restic_0.19.1_linux_amd64.bz2
RESTIC_SHA256="20d4142678d0d95ec11a4759def1b73fd9190abc9ca19e4b62d067c0b387e639"           # restic de dentro
REST_SERVER_VERSION="0.14.0"
REST_SERVER_ARCHIVO_SHA256="4c9c95bc079a0334e81fad379b19dc5c3353c71c2c88d652cafce2081c2b1c66" # rest-server_0.14.0_linux_amd64.tar.gz
REST_SERVER_SHA256="ec4fa7c3472bdc1cde6bfa994d1ee3344bb6e9dc3d9c10c8ff5316e3e77af6fd"         # rest-server de dentro
RCLONE_VERSION="1.75.1"
RCLONE_ARCHIVO_SHA256="982b5aa772841168f8e380f139e9e787b2a105403e32b94da8676a0e1c0a13ab"   # rclone-v1.75.1-linux-amd64.zip
RCLONE_SHA256="f66d8c1d552ad90296a11bc8b46d56a7fa5da1a7fa05e7ca522d95df92c4a4c0"           # rclone de dentro
RESTIC_ARM64_ARCHIVO_SHA256="a5f64aaab53d51e311fa3829124c5b703f2d14cf187d8640b6be3b2b49376465" # restic_0.19.1_linux_arm64.bz2

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# bajar <url> <archivo> <sha256>
bajar() {
  echo "Descargando $1"
  curl -fsSL --retry 3 -o "$TMP/$2" "$1"
  echo "$3  $TMP/$2" | sha256sum -c - >/dev/null || { echo "La huella de $2 no coincide. No se usa." >&2; exit 1; }
}
# comprobar <binario> <sha256>
comprobar() {
  echo "$2  $1" | sha256sum -c - >/dev/null || { echo "La huella de $(basename "$1") no coincide. No se usa." >&2; exit 1; }
}

if [ "$ARQ" = "aarch64" ]; then
  bajar "https://github.com/restic/restic/releases/download/v${RESTIC_VERSION}/restic_${RESTIC_VERSION}_linux_arm64.bz2" restic.bz2 "$RESTIC_ARM64_ARCHIVO_SHA256"
  bzip2 -dc "$TMP/restic.bz2" > "$TMP/restic"
  mkdir -p "$DESTINO"
  install -m 0755 "$TMP/restic" "$DESTINO/restic"
  echo "restic $RESTIC_VERSION (arm64) en $DESTINO; SHA-256 del binario: $(sha256sum "$DESTINO/restic" | awk '{print $1}')"
  exit 0
fi

bajar "https://github.com/restic/restic/releases/download/v${RESTIC_VERSION}/restic_${RESTIC_VERSION}_linux_amd64.bz2" restic.bz2 "$RESTIC_ARCHIVO_SHA256"
bzip2 -dc "$TMP/restic.bz2" > "$TMP/restic"
comprobar "$TMP/restic" "$RESTIC_SHA256"

bajar "https://github.com/restic/rest-server/releases/download/v${REST_SERVER_VERSION}/rest-server_${REST_SERVER_VERSION}_linux_amd64.tar.gz" rest-server.tar.gz "$REST_SERVER_ARCHIVO_SHA256"
tar -xzf "$TMP/rest-server.tar.gz" -C "$TMP" --strip-components=1 "rest-server_${REST_SERVER_VERSION}_linux_amd64/rest-server"
comprobar "$TMP/rest-server" "$REST_SERVER_SHA256"

bajar "https://github.com/rclone/rclone/releases/download/v${RCLONE_VERSION}/rclone-v${RCLONE_VERSION}-linux-amd64.zip" rclone.zip "$RCLONE_ARCHIVO_SHA256"
unzip -q -j "$TMP/rclone.zip" "rclone-v${RCLONE_VERSION}-linux-amd64/rclone" -d "$TMP"
comprobar "$TMP/rclone" "$RCLONE_SHA256"

mkdir -p "$DESTINO"
for f in restic rest-server rclone; do install -m 0755 "$TMP/$f" "$DESTINO/$f"; done
echo "restic $RESTIC_VERSION, rest-server $REST_SERVER_VERSION y rclone $RCLONE_VERSION en $DESTINO"
