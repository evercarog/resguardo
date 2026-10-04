#!/bin/sh
# Instala o actualiza Resguardo Agente en Debian, Ubuntu o un contenedor (CT) de Proxmox.
#
#   Desde la versión publicada (descarga y COMPRUEBA LA FIRMA minisign):
#     curl -fsSL https://github.com/evercarog/resguardo/releases/latest/download/instalar-agente.sh | sudo sh
#
#   Desde un paquete ya copiado al equipo (con su .minisig al lado):
#     sudo sh instalar-agente.sh --paquete ./resguardo-agente-x86_64-linux-musl.tar.gz
#
#   Paquete sin firma (p. ej. el de la integración continua): si hay un
#   SHA256SUMS a su lado, se comprueba; el script muestra además su SHA-256
#   para que la compares con la publicada. Hay que añadir --sin-firma.
#
#   Y vincularlo a la vez con Resguardo Server:
#     --servidor https://192.168.1.20:8443 --codigo ABCD-1234 [--nombre "Equipo"]
#
#   Quitarlo (la configuración y los secretos se quedan, salvo con --purgar):
#     sudo sh /opt/resguardo-agente/instalar-agente.sh --desinstalar [--purgar]
#
# Deja en /opt/resguardo-agente el agente y los restic, rest-server y rclone
# oficiales que trae el paquete (el agente comprueba sus huellas), el enlace
# /usr/local/bin/resguardo-agente y el servicio resguardo-agente en marcha.
# Datos: /var/lib/resguardo-agente. Las copias no se tocan nunca.
set -eu

REPO="evercarog/resguardo"
VERSION="${RESGUARDO_VERSION:-latest}"
# Llave pública de publicación de Resguardo (minisign). PENDIENTE: la genera
# el responsable del proyecto fuera de línea (ver SECURITY.md y docs/plataforma.md, §6).
LLAVE_PUBLICA="${RESGUARDO_LLAVE_PUBLICA:-PENDIENTE}"
PAQUETE_LOCAL="${RESGUARDO_PAQUETE:-}"
SERVIDOR="${RESGUARDO_SERVIDOR:-}"
CODIGO="${RESGUARDO_CODIGO:-}"
NOMBRE="${RESGUARDO_NOMBRE:-}"
DESTINO="/opt/resguardo-agente"
DATOS="/var/lib/resguardo-agente"
UNIDAD="/etc/systemd/system/resguardo-agente.service"
UNIDAD_COPIAS="/etc/systemd/system/resguardo-guarda-copias.service"

fallo() { echo "Error: $*" >&2; exit 1; }

SIN_FIRMA=0
DESINSTALAR=0
PURGAR=0
while [ $# -gt 0 ]; do
  case "$1" in
    --paquete) [ $# -ge 2 ] || fallo "falta el archivo de --paquete."; PAQUETE_LOCAL="$2"; shift ;;
    --sin-firma) SIN_FIRMA=1 ;;
    --servidor) [ $# -ge 2 ] || fallo "falta la dirección de --servidor."; SERVIDOR="$2"; shift ;;
    --codigo) [ $# -ge 2 ] || fallo "falta el código de --codigo."; CODIGO="$2"; shift ;;
    --nombre) [ $# -ge 2 ] || fallo "falta el nombre de --nombre."; NOMBRE="$2"; shift ;;
    --desinstalar) DESINSTALAR=1 ;;
    --purgar) PURGAR=1 ;;
    -h | --ayuda | --help) sed -n '2,24p' "$0" 2>/dev/null | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) fallo "opción desconocida: $1 (ver --ayuda)" ;;
  esac
  shift
done

[ "$(id -u)" -eq 0 ] || fallo "ejecútalo como root (sudo)."
command -v systemctl >/dev/null || fallo "hace falta systemd."
if dpkg-query -W -f='${Status}' resguardo-agente 2>/dev/null | grep -q "install ok installed"; then
  fallo "Resguardo Agente está instalado con el paquete .deb: actualízalo con el .deb nuevo (apt install ./resguardo-agente_….deb) o quítalo con apt remove / apt purge."
fi

# Quita la tabla de nftables del Servidor de copias (si la hay).
quitar_nft() {
  for nft in /usr/sbin/nft /sbin/nft; do
    if [ -x "$nft" ]; then "$nft" delete table inet resguardo >/dev/null 2>&1 || true; break; fi
  done
}

# --- Desinstalar ---
if [ "$DESINSTALAR" = 1 ]; then
  systemctl disable --now resguardo-guarda-copias 2>/dev/null || true
  systemctl disable --now resguardo-agente 2>/dev/null || true
  quitar_nft
  rm -f "$UNIDAD" /usr/local/bin/resguardo-agente
  rm -rf "$DESTINO"
  if [ "$PURGAR" = 1 ]; then
    rm -f "$UNIDAD_COPIAS"
    rm -rf "$DATOS"
  fi
  systemctl daemon-reload
  if [ "$PURGAR" = 1 ]; then
    echo "Resguardo Agente quitado, con su configuración y sus secretos."
    echo "  Las copias (en sus repositorios y en la carpeta del Servidor de copias) no se han tocado."
  else
    echo "Resguardo Agente quitado. La configuración y los secretos se quedan en $DATOS."
    echo "  Para borrarlos también: sudo sh instalar-agente.sh --desinstalar --purgar"
  fi
  exit 0
fi
[ "$PURGAR" = 0 ] || fallo "--purgar va con --desinstalar."
if [ -n "$SERVIDOR" ] || [ -n "$CODIGO" ]; then
  [ -n "$SERVIDOR" ] && [ -n "$CODIGO" ] || fallo "para vincular hacen falta --servidor y --codigo."
fi

case "$(uname -m)" in
  x86_64) ARQ="x86_64" ;;
  aarch64 | arm64) ARQ="aarch64" ;;
  *) fallo "arquitectura no soportada: $(uname -m)" ;;
esac
ARCHIVO="resguardo-agente-${ARQ}-linux-musl.tar.gz"
if [ "$VERSION" = "latest" ]; then BASE="https://github.com/${REPO}/releases/latest/download"; else BASE="https://github.com/${REPO}/releases/download/${VERSION}"; fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# --- El paquete y su comprobación ---
if [ -n "$PAQUETE_LOCAL" ]; then
  [ -f "$PAQUETE_LOCAL" ] || fallo "no existe $PAQUETE_LOCAL."
  cp "$PAQUETE_LOCAL" "$TMP/$ARCHIVO"
  if [ -f "$PAQUETE_LOCAL.minisig" ]; then cp "$PAQUETE_LOCAL.minisig" "$TMP/$ARCHIVO.minisig"; fi
  # El SHA256SUMS de al lado (si lo hay) tiene que coincidir.
  SUMAS="$(dirname "$PAQUETE_LOCAL")/SHA256SUMS"
  if [ -f "$SUMAS" ]; then
    ESPERADA="$(awk -v a="$(basename "$PAQUETE_LOCAL")" '{ n = $2; sub(/^\*/, "", n); if (n == a) { print $1; exit } }' "$SUMAS")"
    if [ -n "$ESPERADA" ]; then
      [ "$(sha256sum "$TMP/$ARCHIVO" | awk '{print $1}')" = "$ESPERADA" ] \
        || fallo "el paquete no coincide con su SHA256SUMS: está dañado o no es el publicado. No se instala."
      echo "El paquete coincide con su SHA256SUMS."
    fi
  fi
else
  [ "$SIN_FIRMA" = 0 ] || fallo "--sin-firma solo vale con --paquete (un archivo que ya has comprobado)."
  [ "$LLAVE_PUBLICA" != "PENDIENTE" ] || fallo "este script aún no tiene la llave pública de publicación: no se descarga nada sin poder comprobar la firma. Instala desde un paquete copiado al equipo (--paquete, ver docs/agente-linux.md)."
  command -v curl >/dev/null || { apt-get update -qq && apt-get install -y -qq curl ca-certificates >/dev/null; }
  echo "Descargando ${ARCHIVO}..."
  curl -fsSL -o "$TMP/$ARCHIVO" "$BASE/$ARCHIVO"
  curl -fsSL -o "$TMP/$ARCHIVO.minisig" "$BASE/$ARCHIVO.minisig"
fi
if [ -f "$TMP/$ARCHIVO.minisig" ] && [ "$LLAVE_PUBLICA" != "PENDIENTE" ]; then
  if ! command -v minisign >/dev/null; then
    echo "Instalando minisign (para comprobar la firma)..."
    apt-get update -qq && apt-get install -y -qq minisign >/dev/null
  fi
  echo "Comprobando la firma..."
  minisign -Vm "$TMP/$ARCHIVO" -P "$LLAVE_PUBLICA" || fallo "la firma no es válida: no se instala."
elif [ "$SIN_FIRMA" = 1 ]; then
  echo "Sin comprobar la firma (--sin-firma). SHA-256 del paquete:"
  echo "  $(sha256sum "$TMP/$ARCHIVO" | awk '{print $1}')"
  echo "  Debe coincidir con la publicada junto al paquete (SHA256SUMS)."
else
  fallo "no se puede comprobar la firma de $PAQUETE_LOCAL (falta su .minisig o la llave pública). Si lo has comprobado por su SHA-256, repite con --sin-firma."
fi

tar -xzf "$TMP/$ARCHIVO" -C "$TMP"
for f in resguardo-agente resguardo-agente.service; do
  [ -f "$TMP/$f" ] || fallo "el paquete no trae $f."
done
if [ "$ARQ" = "x86_64" ]; then
  for f in restic rest-server rclone; do
    [ -f "$TMP/$f" ] || fallo "el paquete no trae $f (es de una versión anterior: usa el de la misma publicación que este script)."
  done
fi

# Versión del paquete y la ya instalada: aviso claro si no es más nueva.
version_de() { "$1" --version 2>/dev/null | awk 'NR == 1 { print $NF }'; }
VERSION_PAQUETE="$(head -n 1 "$TMP/VERSION" 2>/dev/null | tr -d '[:space:]' || true)"
[ -n "$VERSION_PAQUETE" ] || VERSION_PAQUETE="$(version_de "$TMP/resguardo-agente" || true)"
VERSION_INSTALADA=""
if [ -x "$DESTINO/resguardo-agente" ]; then
  VERSION_INSTALADA="$(head -n 1 "$DESTINO/VERSION" 2>/dev/null | tr -d '[:space:]' || true)"
  [ -n "$VERSION_INSTALADA" ] || VERSION_INSTALADA="$(version_de "$DESTINO/resguardo-agente" || true)"
fi
AVISO_VERSION=""
if [ -n "$VERSION_INSTALADA" ] && [ -n "$VERSION_PAQUETE" ]; then
  echo "Versión instalada: $VERSION_INSTALADA · versión del paquete: $VERSION_PAQUETE"
  if [ "$VERSION_INSTALADA" = "$VERSION_PAQUETE" ]; then
    AVISO_VERSION="Misma versión ($VERSION_PAQUETE): se reinstala."
  elif [ "$(printf '%s\n%s\n' "$VERSION_PAQUETE" "$VERSION_INSTALADA" | sort -V | head -n 1)" = "$VERSION_PAQUETE" ]; then
    AVISO_VERSION="AVISO: este paquete es la $VERSION_PAQUETE y ya tienes la $VERSION_INSTALADA (más nueva): se vuelve a una versión ANTERIOR. Si no es lo que querías, instala el paquete de la última publicación."
  fi
  if [ -n "$AVISO_VERSION" ]; then echo "$AVISO_VERSION"; fi
fi

# nftables: el cortafuegos de «Este equipo guarda copias» con «solo redes
# internas» (una tabla propia, solo para su puerto). Si no se puede instalar,
# se sigue: solo hace falta para eso.
if [ ! -x /usr/sbin/nft ] && [ ! -x /sbin/nft ] && command -v apt-get >/dev/null; then
  echo "Instalando nftables (cortafuegos del Servidor de copias)..."
  { apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq nftables >/dev/null; } \
    || echo "Aviso: no se pudo instalar nftables. «Este equipo guarda copias» con «solo redes internas» lo necesita (apt install nftables)."
fi

# --- Instalar ---
# Si ya estaba instalado, se para mientras se cambian los binarios (también
# el Servidor de copias, si estaba en marcha: se vuelve a arrancar al final).
ACTUALIZA=0
if [ -x "$DESTINO/resguardo-agente" ]; then ACTUALIZA=1; fi
GUARDA_COPIAS=0
if systemctl is-active --quiet resguardo-guarda-copias 2>/dev/null; then
  GUARDA_COPIAS=1
  systemctl stop resguardo-guarda-copias 2>/dev/null || true
fi
systemctl stop resguardo-agente 2>/dev/null || true
install -d -m 0755 -o root -g root "$DESTINO"
install -m 0755 -o root -g root "$TMP/resguardo-agente" "$DESTINO/resguardo-agente"
if [ -f "$TMP/VERSION" ]; then install -m 0644 -o root -g root "$TMP/VERSION" "$DESTINO/VERSION"; fi
for f in restic rest-server rclone instalar-agente.sh; do
  if [ -f "$TMP/$f" ]; then install -m 0755 -o root -g root "$TMP/$f" "$DESTINO/$f"; fi
done
for f in "$TMP"/*LICENSE* "$TMP/NOTICE"; do
  if [ -f "$f" ]; then install -m 0644 -o root -g root "$f" "$DESTINO/$(basename "$f")"; fi
done
ln -sf "$DESTINO/resguardo-agente" /usr/local/bin/resguardo-agente
install -m 0644 -o root -g root "$TMP/resguardo-agente.service" "$UNIDAD"
systemctl daemon-reload

if [ -n "$SERVIDOR" ]; then
  echo "Vinculando con ${SERVIDOR}..."
  if [ -n "$NOMBRE" ]; then
    "$DESTINO/resguardo-agente" vincular "$CODIGO" --servidor "$SERVIDOR" --nombre "$NOMBRE" || VINCULO_FALLO=1
  else
    "$DESTINO/resguardo-agente" vincular "$CODIGO" --servidor "$SERVIDOR" || VINCULO_FALLO=1
  fi
fi
systemctl enable resguardo-agente >/dev/null 2>&1
systemctl restart resguardo-agente
if [ "$GUARDA_COPIAS" = 1 ]; then systemctl start resguardo-guarda-copias || true; fi

i=0
while [ $i -lt 10 ] && ! systemctl is-active --quiet resguardo-agente; do
  sleep 1
  i=$((i + 1))
done
if ! systemctl is-active --quiet resguardo-agente; then
  echo
  echo "El servicio no ha arrancado. Su registro:" >&2
  journalctl -u resguardo-agente -n 20 --no-pager >&2 || true
  fallo "revisa el registro (journalctl -u resguardo-agente)."
fi

echo
if [ "$ACTUALIZA" = 1 ]; then echo "Resguardo Agente actualizado y en marcha."; else echo "Resguardo Agente instalado y en marcha."; fi
echo "  $("$DESTINO/resguardo-agente" --version 2>/dev/null || echo "resguardo-agente")"
if [ -x "$DESTINO/restic" ]; then echo "  $("$DESTINO/restic" version 2>/dev/null | head -n 1)"; fi
if [ -x "$DESTINO/rest-server" ]; then echo "  $("$DESTINO/rest-server" --version 2>/dev/null | head -n 1)"; fi
if [ -x "$DESTINO/rclone" ]; then echo "  $("$DESTINO/rclone" version 2>/dev/null | head -n 1)"; fi
if [ -n "$AVISO_VERSION" ]; then echo "  $AVISO_VERSION"; fi
echo
echo "  Estado:   sudo resguardo-agente estado"
echo "  Registro: journalctl -u resguardo-agente -n 30 --no-pager"
if [ "${VINCULO_FALLO:-0}" = 1 ]; then
  echo
  echo "No se pudo vincular (el agente está instalado y hace sus copias en modo local)."
  echo "  Repítelo con: sudo resguardo-agente vincular <código> --servidor https://<servidor>:8443"
  exit 1
fi
if [ -z "$SERVIDOR" ] && ! "$DESTINO/resguardo-agente" estado 2>/dev/null | grep -q "Vinculado con Resguardo Server"; then
  echo
  echo "Siguiente paso: vincularlo con Resguardo Server."
  echo "  1. En la consola: el cliente → «Añadir equipo» → copia el código de vinculación."
  echo "  2. Aquí: sudo resguardo-agente vincular <código> --servidor https://<servidor>:8443"
  echo "     (si Resguardo Server está en este mismo equipo: --servidor https://127.0.0.1:8443)"
  echo "  3. En la consola: comprueba que ve el mismo código de comprobación y confírmalo."
fi
echo
echo "Para que este equipo guarde las copias de otros: en la consola, este equipo →"
echo "  «Este equipo guarda copias» → una carpeta de un disco con espacio y un puerto libre"
echo "  (8000 por defecto; si ya lo usa otro programa aquí, elige otro, p. ej. 8002)."
