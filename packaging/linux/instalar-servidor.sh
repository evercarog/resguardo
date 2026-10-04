#!/bin/sh
# Instala o actualiza Resguardo Server en Debian, Ubuntu o un contenedor (CT) de Proxmox.
#
#   Desde la versión publicada (descarga y COMPRUEBA LA FIRMA minisign):
#     curl -fsSL https://github.com/evercarog/resguardo/releases/latest/download/instalar-servidor.sh | sudo sh
#
#   Desde un paquete ya copiado al equipo (con su .minisig al lado):
#     sudo sh instalar-servidor.sh --paquete ./resguardo-server-x86_64-linux-musl.tar.gz
#
#   Paquete sin firma (p. ej. el de la integración continua): compara antes su
#   SHA-256 con la publicada (el script la muestra) y añade --sin-firma.
#
#   Quitarlo (los datos se quedan, salvo con --purgar):
#     sudo sh /opt/resguardo-server/instalar-servidor.sh --desinstalar [--purgar]
#
# Otras opciones: --puerto N (por defecto 8443; solo al instalar por primera
# vez o si se indica). Deja el binario en /opt/resguardo-server (enlace en
# /usr/local/bin), el usuario de sistema resguardo-server, los datos en
# /var/lib/resguardo-server (0700), la configuración en
# /etc/resguardo-server/servidor.env y el servicio resguardo-server en marcha.
# No toca el cortafuegos (ver docs/servidor-linux.md).
#
# --con-agente [paquete]: este equipo también guarda copias. Instala además
# Resguardo Agente (con instalar-agente.sh y el paquete del agente que haya al
# lado, o el indicado), sin vincular: la consola lo ofrece después como
# «Vincular este servidor» (con la clave de administración).
set -eu

REPO="evercarog/resguardo"
VERSION="${RESGUARDO_VERSION:-latest}"
# Llave pública de publicación de Resguardo (minisign). PENDIENTE: la genera
# el responsable del proyecto fuera de línea (ver SECURITY.md y docs/plataforma.md, §6).
LLAVE_PUBLICA="${RESGUARDO_LLAVE_PUBLICA:-PENDIENTE}"
PAQUETE_LOCAL="${RESGUARDO_PAQUETE:-}"
DESTINO="/opt/resguardo-server"
DATOS="/var/lib/resguardo-server"
CONF="/etc/resguardo-server"
USUARIO="resguardo-server"
UNIDAD="/etc/systemd/system/resguardo-server.service"

fallo() { echo "Error: $*" >&2; exit 1; }

SIN_FIRMA=0
DESINSTALAR=0
PURGAR=0
PUERTO=""
CON_AGENTE=0
PAQUETE_AGENTE=""
while [ $# -gt 0 ]; do
  case "$1" in
    --paquete) [ $# -ge 2 ] || fallo "falta el archivo de --paquete."; PAQUETE_LOCAL="$2"; shift ;;
    --sin-firma) SIN_FIRMA=1 ;;
    --puerto) [ $# -ge 2 ] || fallo "falta el número de --puerto."; PUERTO="$2"; shift ;;
    --desinstalar) DESINSTALAR=1 ;;
    --purgar) PURGAR=1 ;;
    --con-agente) CON_AGENTE=1; if [ $# -ge 2 ] && [ "${2#-}" = "$2" ]; then PAQUETE_AGENTE="$2"; shift; fi ;;
    -h | --ayuda | --help) sed -n '2,28p' "$0" 2>/dev/null | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) fallo "opción desconocida: $1 (ver --ayuda)" ;;
  esac
  shift
done

[ "$(id -u)" -eq 0 ] || fallo "ejecútalo como root (sudo)."
command -v systemctl >/dev/null || fallo "hace falta systemd."
if dpkg-query -W -f='${Status}' resguardo-server 2>/dev/null | grep -q "install ok installed"; then
  fallo "Resguardo Server está instalado con el paquete .deb: actualízalo con el .deb nuevo (apt install ./resguardo-server_….deb) o quítalo con apt remove / apt purge."
fi

# --- Desinstalar ---
if [ "$DESINSTALAR" = 1 ]; then
  systemctl disable --now resguardo-server 2>/dev/null || true
  rm -f "$UNIDAD" /usr/local/bin/resguardo-server
  rm -rf "$DESTINO"
  systemctl daemon-reload
  if [ "$PURGAR" = 1 ]; then
    rm -rf "$DATOS" "$CONF"
    if id "$USUARIO" >/dev/null 2>&1; then userdel "$USUARIO" 2>/dev/null || true; fi
    echo "Resguardo Server quitado, con sus datos, su configuración y su usuario."
  else
    echo "Resguardo Server quitado. Los datos se quedan en $DATOS (cuentas, equipos, certificados)."
    echo "  Para borrarlos también: sudo sh instalar-servidor.sh --desinstalar --purgar"
  fi
  exit 0
fi
[ "$PURGAR" = 0 ] || fallo "--purgar va con --desinstalar."
if [ -n "$PUERTO" ]; then
  case "$PUERTO" in *[!0-9]*) fallo "puerto no válido: $PUERTO" ;; esac
  [ "$PUERTO" -ge 1 ] && [ "$PUERTO" -le 65535 ] || fallo "puerto no válido: $PUERTO"
fi

case "$(uname -m)" in
  x86_64) ARQ="x86_64" ;;
  aarch64 | arm64) ARQ="aarch64" ;;
  *) fallo "arquitectura no soportada: $(uname -m)" ;;
esac
ARCHIVO="resguardo-server-${ARQ}-linux-musl.tar.gz"
if [ "$VERSION" = "latest" ]; then BASE="https://github.com/${REPO}/releases/latest/download"; else BASE="https://github.com/${REPO}/releases/download/${VERSION}"; fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# --- El paquete y su comprobación ---
if [ -n "$PAQUETE_LOCAL" ]; then
  [ -f "$PAQUETE_LOCAL" ] || fallo "no existe $PAQUETE_LOCAL."
  cp "$PAQUETE_LOCAL" "$TMP/$ARCHIVO"
  if [ -f "$PAQUETE_LOCAL.minisig" ]; then cp "$PAQUETE_LOCAL.minisig" "$TMP/$ARCHIVO.minisig"; fi
else
  [ "$SIN_FIRMA" = 0 ] || fallo "--sin-firma solo vale con --paquete (un archivo que ya has comprobado)."
  [ "$LLAVE_PUBLICA" != "PENDIENTE" ] || fallo "este script aún no tiene la llave pública de publicación: no se descarga nada sin poder comprobar la firma. Instala desde un paquete copiado al equipo (--paquete, ver docs/servidor-linux.md)."
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
for f in resguardo-server resguardo-server.service; do
  [ -f "$TMP/$f" ] || fallo "el paquete no trae $f."
done

# Versión del paquete y la ya instalada: aviso claro si no es más nueva
# (las instalaciones anteriores no guardaban su versión: VERSION en el paquete).
VERSION_PAQUETE="$(head -n 1 "$TMP/VERSION" 2>/dev/null | tr -d '[:space:]' || true)"
VERSION_INSTALADA=""
if [ -x "$DESTINO/resguardo-server" ]; then VERSION_INSTALADA="$(head -n 1 "$DESTINO/VERSION" 2>/dev/null | tr -d '[:space:]' || true)"; fi
AVISO_VERSION=""
if [ -x "$DESTINO/resguardo-server" ] && [ -n "$VERSION_PAQUETE" ]; then
  echo "Versión instalada: ${VERSION_INSTALADA:-desconocida (una instalación anterior no la guardaba)} · versión del paquete: $VERSION_PAQUETE"
  if [ "$VERSION_INSTALADA" = "$VERSION_PAQUETE" ]; then
    AVISO_VERSION="Misma versión ($VERSION_PAQUETE): se reinstala."
  elif [ -n "$VERSION_INSTALADA" ] && [ "$(printf '%s\n%s\n' "$VERSION_PAQUETE" "$VERSION_INSTALADA" | sort -V | head -n 1)" = "$VERSION_PAQUETE" ]; then
    AVISO_VERSION="AVISO: este paquete es la $VERSION_PAQUETE y ya tienes la $VERSION_INSTALADA (más nueva): se vuelve a una versión ANTERIOR. Si no es lo que querías, instala el paquete de la última publicación."
  fi
  if [ -n "$AVISO_VERSION" ]; then echo "$AVISO_VERSION"; fi
fi

# --- Usuario, carpetas y archivos ---
if ! id "$USUARIO" >/dev/null 2>&1; then
  useradd --system --user-group --home-dir "$DATOS" --no-create-home --shell /usr/sbin/nologin \
    --comment "Resguardo Server" "$USUARIO"
fi
ACTUALIZA=0
if systemctl is-active --quiet resguardo-server 2>/dev/null; then ACTUALIZA=1; fi
systemctl stop resguardo-server 2>/dev/null || true
install -d -m 0755 -o root -g root "$DESTINO"
install -m 0755 -o root -g root "$TMP/resguardo-server" "$DESTINO/resguardo-server"
if [ -f "$TMP/VERSION" ]; then install -m 0644 -o root -g root "$TMP/VERSION" "$DESTINO/VERSION"; fi
if [ -f "$TMP/instalar-servidor.sh" ]; then install -m 0755 -o root -g root "$TMP/instalar-servidor.sh" "$DESTINO/instalar-servidor.sh"; fi
# El kit de la consola en internet (docs/consola-en-linea.md), si el paquete lo trae.
if [ -f "$TMP/preparar-vm.sh" ]; then install -m 0755 -o root -g root "$TMP/preparar-vm.sh" "$DESTINO/preparar-vm.sh"; fi
if [ -d "$TMP/fail2ban" ]; then
  install -d -m 0755 "$DESTINO/fail2ban/filter.d" "$DESTINO/fail2ban/jail.d"
  install -m 0644 -o root -g root "$TMP/fail2ban/filter.d/resguardo-server.conf" "$DESTINO/fail2ban/filter.d/resguardo-server.conf"
  install -m 0644 -o root -g root "$TMP/fail2ban/jail.d/resguardo-server.conf" "$DESTINO/fail2ban/jail.d/resguardo-server.conf"
fi
for f in LICENSE NOTICE; do
  if [ -f "$TMP/$f" ]; then install -m 0644 -o root -g root "$TMP/$f" "$DESTINO/$f"; fi
done
ln -sf "$DESTINO/resguardo-server" /usr/local/bin/resguardo-server
install -d -m 0700 -o "$USUARIO" -g "$USUARIO" "$DATOS"
chown "$USUARIO:$USUARIO" "$DATOS"
chmod 0700 "$DATOS"
install -d -m 0755 -o root -g root "$CONF"
if [ ! -f "$CONF/servidor.env" ]; then
  cat > "$CONF/servidor.env" <<EOF
# Configuración de Resguardo Server (la lee el servicio al arrancar).
# Tras cambiarla: sudo systemctl restart resguardo-server
#
# Dónde escuchar (por defecto, en todas las interfaces, puerto 8443):
RESGUARDO_ESCUCHAR=0.0.0.0:${PUERTO:-8443}
# Nombres o IP extra para el certificado propio, separados por comas
# (el nombre del equipo y sus IP ya van siempre), p. ej. resguardo.lan,192.168.1.20
#RESGUARDO_NOMBRES=
EOF
  chmod 0644 "$CONF/servidor.env"
elif [ -n "$PUERTO" ]; then
  if grep -q '^RESGUARDO_ESCUCHAR=' "$CONF/servidor.env"; then
    sed -i "s/^RESGUARDO_ESCUCHAR=.*/RESGUARDO_ESCUCHAR=0.0.0.0:${PUERTO}/" "$CONF/servidor.env"
  else
    echo "RESGUARDO_ESCUCHAR=0.0.0.0:${PUERTO}" >> "$CONF/servidor.env"
  fi
fi
install -m 0644 -o root -g root "$TMP/resguardo-server.service" "$UNIDAD"
systemctl daemon-reload
systemctl enable resguardo-server >/dev/null 2>&1
systemctl restart resguardo-server

# --- Esperar al arranque y decir dónde entrar ---
i=0
while [ $i -lt 30 ]; do
  if ! systemctl is-active --quiet resguardo-server && [ $i -ge 3 ]; then break; fi
  if [ -f "$DATOS/codigo-arranque.txt" ] || { [ -f "$DATOS/tls/ca.huella" ] && [ -f "$DATOS/control.db" ] && [ $i -ge 4 ]; }; then break; fi
  sleep 1
  i=$((i + 1))
done
if ! systemctl is-active --quiet resguardo-server; then
  echo
  echo "El servicio no ha arrancado. Su registro:" >&2
  journalctl -u resguardo-server -n 20 --no-pager >&2 || true
  fallo "revisa el registro (journalctl -u resguardo-server). En un CT de Proxmox sin privilegios, activa «nesting» (anidamiento) en sus opciones."
fi

echo
if [ "$ACTUALIZA" = 1 ]; then echo "Resguardo Server actualizado y en marcha."; else echo "Resguardo Server instalado y en marcha."; fi
if [ -n "$VERSION_PAQUETE" ]; then echo "  Versión: $VERSION_PAQUETE"; fi
if [ -n "$AVISO_VERSION" ]; then echo "  $AVISO_VERSION"; fi
# shellcheck disable=SC1091
( set -a; . "$CONF/servidor.env"; set +a; "$DESTINO/resguardo-server" codigo-inicial --datos "$DATOS" ) || true
echo
echo "  Ábrela desde otro equipo de la red. El navegador avisará del certificado"
echo "  (es propio): comprueba que la huella de la autoridad coincide con la de arriba."
echo "  Código y huella otra vez:  sudo resguardo-server codigo-inicial"
echo "  Registro:                  journalctl -u resguardo-server -n 30 --no-pager"
echo "  Configuración (puerto):    $CONF/servidor.env"
echo "  Cortafuegos: este script no lo toca (ver docs/servidor-linux.md)."

# --- Este equipo también guarda copias (--con-agente) ---
if [ "$CON_AGENTE" = 1 ]; then
  echo
  AQUI="$(dirname "$0")"
  if [ -z "$PAQUETE_AGENTE" ]; then
    for p in "$AQUI"/resguardo-agente-*.tar.gz; do
      if [ -f "$p" ]; then PAQUETE_AGENTE="$p"; break; fi
    done
  fi
  if [ ! -f "$AQUI/instalar-agente.sh" ] || [ -z "$PAQUETE_AGENTE" ]; then
    echo "Para que este equipo guarde copias falta el agente: pon instalar-agente.sh y el paquete"
    echo "resguardo-agente-….tar.gz junto a este script (o usa --con-agente <paquete>) y repite."
  elif if [ "$SIN_FIRMA" = 1 ]; then sh "$AQUI/instalar-agente.sh" --paquete "$PAQUETE_AGENTE" --sin-firma; else sh "$AQUI/instalar-agente.sh" --paquete "$PAQUETE_AGENTE"; fi; then
    echo "Resguardo Agente instalado (sin vincular). En la consola, «Vincular este servidor» lo da"
    echo "de alta con la clave de administración y eliges la carpeta donde guardar las copias."
  else
    echo "No se pudo instalar Resguardo Agente: mira el mensaje de arriba (docs/agente-linux.md)."
  fi
fi
