#!/bin/sh
# Construye los paquetes de Linux a partir de un binario ya compilado:
#
#   packaging/linux/construir-paquetes.sh [agente] <binario> <versión> <x86_64|aarch64> <carpeta de salida>
#   packaging/linux/construir-paquetes.sh servidor <binario> <versión> <x86_64|aarch64> <carpeta de salida>
#
# Agente: deja en la carpeta de salida
# - resguardo-agente-<arq>-linux-musl.tar.gz: lo que instala instalar-agente.sh
#   (binario, unidad de systemd, el propio instalador, licencias y los restic,
#   rest-server y rclone oficiales que deben estar junto al binario: en x86_64
#   son obligatorios y se comprueban sus huellas; los baja
#   scripts/fetch-binarios-linux.sh). Su firma (.minisig) se hace aparte.
# - resguardo-agente_<versión>_<amd64|arm64>.deb: para Debian y Ubuntu (lleva
#   los mismos binarios: no depende del restic de la distribución).
# - instalar-agente.sh y SHA256SUMS (de todo lo anterior).
#
# Servidor (el binario, compilado con la feature consola-integrada): deja
# - resguardo-server-<arq>-linux-musl.tar.gz: lo que instala instalar-servidor.sh
#   (binario, unidad de systemd, el propio instalador y las licencias), más el
#   kit de la consola en internet: preparar-vm.sh y el filtro de fail2ban
#   (docs/consola-en-linea.md).
# - resguardo-server_<versión>_<amd64|arm64>.deb (el kit en /opt/resguardo-server).
# - instalar-servidor.sh, preparar-vm.sh y SHA256SUMS (de todo lo anterior).
set -eu

QUE="agente"
case "${1:-}" in
  agente | servidor) QUE="$1"; shift ;;
esac
[ $# -eq 4 ] || { echo "Uso: $0 [agente|servidor] <binario> <versión> <x86_64|aarch64> <salida>" >&2; exit 2; }
BINARIO="$1"
VERSION="$2"
ARQ="$3"
SALIDA="$4"
AQUI="$(cd "$(dirname "$0")" && pwd)"
case "$ARQ" in
  x86_64) ARQ_DEB="amd64" ;;
  aarch64) ARQ_DEB="arm64" ;;
  *) echo "Arquitectura no soportada: $ARQ" >&2; exit 2 ;;
esac
mkdir -p "$SALIDA"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
DIR_BIN="$(dirname "$BINARIO")"

if [ "$QUE" = "servidor" ]; then
  # --- tar.gz ---
  mkdir "$TMP/tar"
  install -m 0755 "$BINARIO" "$TMP/tar/resguardo-server"
  install -m 0644 "$AQUI/resguardo-server.service" "$TMP/tar/resguardo-server.service"
  install -m 0755 "$AQUI/instalar-servidor.sh" "$TMP/tar/instalar-servidor.sh"
  # Consola en internet: preparar la máquina virtual y fail2ban.
  install -m 0755 "$AQUI/nube/preparar-vm.sh" "$TMP/tar/preparar-vm.sh"
  install -d "$TMP/tar/fail2ban/filter.d" "$TMP/tar/fail2ban/jail.d"
  install -m 0644 "$AQUI/fail2ban/filter.d/resguardo-server.conf" "$TMP/tar/fail2ban/filter.d/resguardo-server.conf"
  install -m 0644 "$AQUI/fail2ban/jail.d/resguardo-server.conf" "$TMP/tar/fail2ban/jail.d/resguardo-server.conf"
  install -m 0644 "$AQUI/../../LICENSE" "$TMP/tar/LICENSE"
  install -m 0644 "$AQUI/../../NOTICE" "$TMP/tar/NOTICE"
  # La versión del paquete: el instalador avisa si no es más nueva que la instalada.
  echo "$VERSION" > "$TMP/tar/VERSION"
  tar --sort=name --mtime='2000-01-01 00:00Z' --owner=0 --group=0 --numeric-owner \
    -C "$TMP/tar" -czf "$SALIDA/resguardo-server-${ARQ}-linux-musl.tar.gz" .
  install -m 0755 "$AQUI/instalar-servidor.sh" "$SALIDA/instalar-servidor.sh"
  install -m 0755 "$AQUI/nube/preparar-vm.sh" "$SALIDA/preparar-vm.sh"

  # --- .deb ---
  R="$TMP/deb"
  install -d "$R/DEBIAN" "$R/opt/resguardo-server" "$R/usr/bin" "$R/lib/systemd/system" "$R/usr/share/doc/resguardo-server"
  install -m 0755 "$BINARIO" "$R/opt/resguardo-server/resguardo-server"
  ln -s /opt/resguardo-server/resguardo-server "$R/usr/bin/resguardo-server"
  install -m 0644 "$AQUI/resguardo-server.service" "$R/lib/systemd/system/resguardo-server.service"
  install -m 0644 "$AQUI/../../LICENSE" "$R/usr/share/doc/resguardo-server/copyright"
  install -m 0644 "$AQUI/../../NOTICE" "$R/usr/share/doc/resguardo-server/NOTICE"
  install -d "$R/opt/resguardo-server/nube" "$R/opt/resguardo-server/fail2ban/filter.d" "$R/opt/resguardo-server/fail2ban/jail.d"
  install -m 0755 "$AQUI/nube/preparar-vm.sh" "$R/opt/resguardo-server/nube/preparar-vm.sh"
  install -m 0644 "$AQUI/fail2ban/filter.d/resguardo-server.conf" "$R/opt/resguardo-server/fail2ban/filter.d/resguardo-server.conf"
  install -m 0644 "$AQUI/fail2ban/jail.d/resguardo-server.conf" "$R/opt/resguardo-server/fail2ban/jail.d/resguardo-server.conf"
  cat > "$R/DEBIAN/control" <<EOF
Package: resguardo-server
Version: ${VERSION}
Architecture: ${ARQ_DEB}
Maintainer: Resguardo <https://github.com/evercarog/resguardo>
Depends: adduser
Section: admin
Priority: optional
Homepage: https://github.com/evercarog/resguardo
Description: Resguardo Server: consola web de las copias de seguridad gestionadas
 Consola web (HTTPS, puerto 8443) y canal de los agentes de Resguardo: desde
 ella se administran las copias de seguridad de los equipos vinculados. No
 guarda las copias ni puede abrirlas: solo reenvía órdenes selladas.
EOF
  cat > "$R/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
if [ "$1" = "configure" ]; then
  if ! getent passwd resguardo-server >/dev/null; then
    adduser --system --group --home /var/lib/resguardo-server --no-create-home \
      --shell /usr/sbin/nologin --gecos "Resguardo Server" resguardo-server >/dev/null
  fi
  install -d -m 0700 -o resguardo-server -g resguardo-server /var/lib/resguardo-server
  install -d -m 0755 /etc/resguardo-server
  if [ ! -f /etc/resguardo-server/servidor.env ]; then
    cat > /etc/resguardo-server/servidor.env <<'FIN'
# Configuración de Resguardo Server (la lee el servicio al arrancar).
# Tras cambiarla: sudo systemctl restart resguardo-server
#
# Dónde escuchar (por defecto, en todas las interfaces, puerto 8443):
RESGUARDO_ESCUCHAR=0.0.0.0:8443
# Nombres o IP extra para el certificado propio, separados por comas
# (el nombre del equipo y sus IP ya van siempre), p. ej. resguardo.lan,192.168.1.20
#RESGUARDO_NOMBRES=
FIN
  fi
  if [ -d /run/systemd/system ]; then
    systemctl daemon-reload
    systemctl enable resguardo-server.service >/dev/null
    systemctl restart resguardo-server.service
    echo "Resguardo Server en marcha. Dirección, huella TLS y código de primer arranque:"
    echo "  sudo resguardo-server codigo-inicial"
  fi
fi
EOF
  cat > "$R/DEBIAN/prerm" <<'EOF'
#!/bin/sh
set -e
if [ "$1" = "remove" ] && [ -d /run/systemd/system ]; then
  systemctl disable --now resguardo-server.service >/dev/null 2>&1 || true
fi
EOF
  cat > "$R/DEBIAN/postrm" <<'EOF'
#!/bin/sh
set -e
# Solo al purgar: los datos (cuentas, equipos, certificados), la configuración
# y el usuario. Con «remove» se quedan.
if [ "$1" = "purge" ]; then
  rm -rf /var/lib/resguardo-server /etc/resguardo-server
  if getent passwd resguardo-server >/dev/null; then deluser --system resguardo-server >/dev/null 2>&1 || true; fi
fi
if [ -d /run/systemd/system ]; then systemctl daemon-reload || true; fi
EOF
  chmod 0755 "$R/DEBIAN/postinst" "$R/DEBIAN/prerm" "$R/DEBIAN/postrm"
  dpkg-deb --root-owner-group --build "$R" "$SALIDA/resguardo-server_${VERSION}_${ARQ_DEB}.deb" >/dev/null
  (cd "$SALIDA" && sha256sum "resguardo-server-${ARQ}-linux-musl.tar.gz" "resguardo-server_${VERSION}_${ARQ_DEB}.deb" instalar-servidor.sh preparar-vm.sh > SHA256SUMS)
  echo "Paquetes en $SALIDA:"
  ls -l "$SALIDA"
  cat "$SALIDA/SHA256SUMS"
  exit 0
fi

# Binarios oficiales que acompañan al agente (los baja, con sus huellas,
# scripts/fetch-binarios-linux.sh). En x86_64 son obligatorios y tienen que
# ser los de la huella fijada: restic (las copias, sea cual sea el de la
# distribución), rest-server (huella en crates/agente/src/server.rs) y rclone
# (huella en crates/agente/src/nube.rs). En aarch64, opcionales.
RESTIC_SHA256="20d4142678d0d95ec11a4759def1b73fd9190abc9ca19e4b62d067c0b387e639"        # restic 0.19.1
REST_SERVER_SHA256="ec4fa7c3472bdc1cde6bfa994d1ee3344bb6e9dc3d9c10c8ff5316e3e77af6fd"   # rest-server 0.14.0
RCLONE_SHA256="f66d8c1d552ad90296a11bc8b46d56a7fa5da1a7fa05e7ca522d95df92c4a4c0"        # rclone 1.75.1
if [ "$ARQ" = "x86_64" ]; then
  for par in "restic $RESTIC_SHA256" "rest-server $REST_SERVER_SHA256" "rclone $RCLONE_SHA256"; do
    f="${par%% *}"
    h="${par#* }"
    [ -f "$DIR_BIN/$f" ] || { echo "Falta $DIR_BIN/$f (scripts/fetch-binarios-linux.sh $DIR_BIN)." >&2; exit 1; }
    echo "$h  $DIR_BIN/$f" | sha256sum -c - >/dev/null || { echo "$f no es el de la huella fijada." >&2; exit 1; }
  done
elif [ -f "$DIR_BIN/rclone" ]; then
  echo "rclone: solo hay huella fijada para x86_64." >&2
  exit 1
fi
LICENCIAS="$AQUI/../../src-tauri/licenses"

# --- tar.gz ---
mkdir "$TMP/tar"
install -m 0755 "$BINARIO" "$TMP/tar/resguardo-agente"
install -m 0644 "$AQUI/resguardo-agente.service" "$TMP/tar/resguardo-agente.service"
install -m 0755 "$AQUI/instalar-agente.sh" "$TMP/tar/instalar-agente.sh"
install -m 0644 "$AQUI/../../LICENSE" "$TMP/tar/LICENSE"
install -m 0644 "$AQUI/../../NOTICE" "$TMP/tar/NOTICE"
# La versión del paquete: el instalador avisa si no es más nueva que la instalada.
echo "$VERSION" > "$TMP/tar/VERSION"
for f in restic rest-server rclone; do
  if [ -f "$DIR_BIN/$f" ]; then
    install -m 0755 "$DIR_BIN/$f" "$TMP/tar/$f"
    install -m 0644 "$LICENCIAS/$f-LICENSE.txt" "$TMP/tar/$f-LICENSE.txt"
  fi
done
# Fechas y dueños fijos: el mismo binario da el mismo paquete.
tar --sort=name --mtime='2000-01-01 00:00Z' --owner=0 --group=0 --numeric-owner \
  -C "$TMP/tar" -czf "$SALIDA/resguardo-agente-${ARQ}-linux-musl.tar.gz" .
install -m 0755 "$AQUI/instalar-agente.sh" "$SALIDA/instalar-agente.sh"

# --- .deb ---
R="$TMP/deb"
install -d "$R/DEBIAN" "$R/opt/resguardo-agente" "$R/usr/bin" "$R/lib/systemd/system" "$R/usr/share/doc/resguardo-agente"
install -m 0755 "$BINARIO" "$R/opt/resguardo-agente/resguardo-agente"
for f in restic rest-server rclone; do
  if [ -f "$DIR_BIN/$f" ]; then
    install -m 0755 "$DIR_BIN/$f" "$R/opt/resguardo-agente/$f"
    install -m 0644 "$LICENCIAS/$f-LICENSE.txt" "$R/usr/share/doc/resguardo-agente/$f-LICENSE.txt"
  fi
done
ln -s /opt/resguardo-agente/resguardo-agente "$R/usr/bin/resguardo-agente"
install -m 0644 "$AQUI/resguardo-agente.service" "$R/lib/systemd/system/resguardo-agente.service"
install -m 0644 "$AQUI/../../LICENSE" "$R/usr/share/doc/resguardo-agente/copyright"
install -m 0644 "$AQUI/../../NOTICE" "$R/usr/share/doc/resguardo-agente/NOTICE"
cat > "$R/DEBIAN/control" <<EOF
Package: resguardo-agente
Version: ${VERSION}
Architecture: ${ARQ_DEB}
Maintainer: Resguardo <https://github.com/evercarog/resguardo>
Recommends: nftables
Section: admin
Priority: optional
Homepage: https://github.com/evercarog/resguardo
Description: Resguardo Agente: copias de seguridad gestionadas con restic
 Hace las copias de seguridad programadas de este equipo con restic y, si
 se vincula, las administra Resguardo Server. También puede guardar las
 copias de otros equipos (rest-server en modo solo añadir). Lleva dentro
 los restic, rest-server y rclone oficiales, con sus huellas fijadas.
EOF
cat > "$R/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
if [ "$1" = "configure" ] && [ -d /run/systemd/system ]; then
  systemctl daemon-reload
  systemctl enable resguardo-agente.service >/dev/null
  systemctl restart resguardo-agente.service
  # Al actualizar: el Servidor de copias (si estaba en marcha) con el rest-server nuevo.
  systemctl try-restart resguardo-guarda-copias.service >/dev/null 2>&1 || true
fi
EOF
cat > "$R/DEBIAN/prerm" <<'EOF'
#!/bin/sh
set -e
if [ "$1" = "remove" ] && [ -d /run/systemd/system ]; then
  systemctl disable --now resguardo-agente.service >/dev/null 2>&1 || true
  systemctl disable --now resguardo-guarda-copias.service >/dev/null 2>&1 || true
fi
# Las reglas de nftables del Servidor de copias (la tabla propia del agente).
if [ "$1" = "remove" ]; then
  for nft in /usr/sbin/nft /sbin/nft; do
    if [ -x "$nft" ]; then "$nft" delete table inet resguardo >/dev/null 2>&1 || true; break; fi
  done
fi
EOF
cat > "$R/DEBIAN/postrm" <<'EOF'
#!/bin/sh
set -e
# Solo al purgar: la configuración y los secretos del agente. Las copias
# (en sus repositorios) y la carpeta del Servidor de copias no se tocan.
if [ "$1" = "purge" ]; then
  rm -f /etc/systemd/system/resguardo-guarda-copias.service
  rm -rf /var/lib/resguardo-agente
fi
if [ -d /run/systemd/system ]; then systemctl daemon-reload || true; fi
EOF
chmod 0755 "$R/DEBIAN/postinst" "$R/DEBIAN/prerm" "$R/DEBIAN/postrm"
dpkg-deb --root-owner-group --build "$R" "$SALIDA/resguardo-agente_${VERSION}_${ARQ_DEB}.deb" >/dev/null
(cd "$SALIDA" && sha256sum "resguardo-agente-${ARQ}-linux-musl.tar.gz" "resguardo-agente_${VERSION}_${ARQ_DEB}.deb" instalar-agente.sh > SHA256SUMS)
echo "Paquetes en $SALIDA:"
ls -l "$SALIDA"
cat "$SALIDA/SHA256SUMS"
