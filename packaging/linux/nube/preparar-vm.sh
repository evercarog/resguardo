#!/bin/sh
# Prepara una máquina virtual nueva de Ubuntu (22.04 o 24.04; Oracle Cloud,
# Google Cloud, Hetzner…) como consola de Resguardo Server en internet.
# Guía paso a paso: docs/consola-en-linea.md.
#
#   sudo sh preparar-vm.sh --dominio consola.ejemplo.com --acme-correo tu@correo \
#        --paquete ./resguardo-server-aarch64-linux-musl.tar.gz [--sin-firma]
#
# Qué hace (se puede repetir: deja todo igual):
#   1. Actualizaciones de seguridad automáticas (unattended-upgrades).
#   2. Cortafuegos: solo entra SSH (22), HTTP (80, para el certificado) y
#      HTTPS (443). Con ufw o, en las imágenes de Oracle Cloud, con sus reglas
#      de iptables (las guarda netfilter-persistent).
#   3. Resguardo Server con instalar-servidor.sh (el de al lado): su usuario de
#      sistema sin privilegios, la unidad de systemd endurecida y los datos en
#      /var/lib/resguardo-server.
#   4. La configuración de una consola en internet en
#      /etc/resguardo-server/servidor.env: puerto 443, --dominio (certificado
#      de Let's Encrypt automático) y el permiso para usar los puertos 80 y 443
#      (CAP_NET_BIND_SERVICE, nada más).
#   5. fail2ban con el filtro de Resguardo Server (bloquea una hora la IP que
#      falla 10 accesos en 10 minutos).
#   6. Una carpeta con las copias de la consola para llevarlas fuera
#      (/var/backups/resguardo-consola, la rellena cada hora un temporizador):
#      ya van cifradas con tu clave de respaldo.
#
# Opciones:
#   --dominio NOMBRE         El nombre de la consola (tiene que apuntar ya a esta máquina).
#   --acme-correo CORREO     Para los avisos de Let's Encrypt.
#   --dominio-agentes NOMBRE Por defecto agentes.<dominio> (también tiene que apuntar aquí).
#   --paquete ARCHIVO        El .tar.gz de Resguardo Server (con su .minisig al lado, o --sin-firma).
#   --sin-firma              Ver instalar-servidor.sh: solo con un paquete que has comprobado.
#   --acme-pruebas           Let's Encrypt de pruebas (para ensayar sin gastar sus límites).
#   --acme-directorio URL    Otra autoridad ACME (pruebas).
#   --ssh-desde RED          Solo deja entrar por SSH desde esa IP o red (p. ej. 203.0.113.7/32).
#   --sin-cortafuegos        No toca el cortafuegos (si ya lo gestionas tú).
#   --usuario-copias USUARIO Quién puede leer las copias de la consola (por defecto, quien usó sudo).
set -eu

fallo() { echo "Error: $*" >&2; exit 1; }

# ¿Está apt/dpkg ocupado? (p. ej. las actualizaciones automáticas del sistema
# justo después de arrancar una máquina nueva). Con fuser, por sus cerrojos;
# sin él (contenedores mínimos), por los procesos.
apt_ocupado() {
  if command -v fuser >/dev/null 2>&1; then
    fuser /var/lib/dpkg/lock-frontend /var/lib/dpkg/lock /var/lib/apt/lists/lock /var/cache/apt/archives/lock >/dev/null 2>&1
  else
    pgrep -x 'apt|apt-get|dpkg|unattended-upgr' >/dev/null 2>&1
  fi
}

# Espera (hasta 10 min, avisando) a que apt quede libre en vez de fallar.
esperar_apt() {
  espera=0
  while apt_ocupado; do
    if [ "$espera" -eq 0 ]; then echo "Esperando a que terminen las actualizaciones automáticas del sistema (apt está ocupado; hasta 10 min)..."; fi
    [ "$espera" -lt 600 ] || fallo "apt sigue ocupado después de 10 minutos. Espera a que terminen las actualizaciones del sistema y repite."
    sleep 5
    espera=$((espera + 5))
  done
}

# apt-get, después de esperar a que quede libre (y, por si acaso, que también
# espere él: DPkg::Lock::Timeout, apt 1.9.11 o posterior; los anteriores lo ignoran).
apt_get() {
  esperar_apt
  DEBIAN_FRONTEND=noninteractive apt-get -o DPkg::Lock::Timeout=600 "$@"
}

AQUI="$(cd "$(dirname "$0")" && pwd)"
DOMINIO=""
CORREO=""
DOMINIO_AGENTES=""
PAQUETE=""
SIN_FIRMA=0
PRUEBAS=0
DIRECTORIO=""
SSH_DESDE=""
CORTAFUEGOS=1
USUARIO_COPIAS="${SUDO_USER:-}"
while [ $# -gt 0 ]; do
  case "$1" in
    --dominio) [ $# -ge 2 ] || fallo "falta el nombre de --dominio."; DOMINIO="$2"; shift ;;
    --acme-correo) [ $# -ge 2 ] || fallo "falta el correo de --acme-correo."; CORREO="$2"; shift ;;
    --dominio-agentes) [ $# -ge 2 ] || fallo "falta el nombre de --dominio-agentes."; DOMINIO_AGENTES="$2"; shift ;;
    --paquete) [ $# -ge 2 ] || fallo "falta el archivo de --paquete."; PAQUETE="$2"; shift ;;
    --sin-firma) SIN_FIRMA=1 ;;
    --acme-pruebas) PRUEBAS=1 ;;
    --acme-directorio) [ $# -ge 2 ] || fallo "falta la dirección de --acme-directorio."; DIRECTORIO="$2"; shift ;;
    --ssh-desde) [ $# -ge 2 ] || fallo "falta la red de --ssh-desde."; SSH_DESDE="$2"; shift ;;
    --sin-cortafuegos) CORTAFUEGOS=0 ;;
    --usuario-copias) [ $# -ge 2 ] || fallo "falta el usuario de --usuario-copias."; USUARIO_COPIAS="$2"; shift ;;
    -h | --ayuda | --help) sed -n '2,40p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) fallo "opción desconocida: $1 (ver --ayuda)" ;;
  esac
  shift
done

[ "$(id -u)" -eq 0 ] || fallo "ejecútalo como root (sudo)."
[ -n "$DOMINIO" ] || fallo "falta --dominio (el nombre de la consola, p. ej. consola.ejemplo.com o tu-oficina.duckdns.org)."
case "$DOMINIO$DOMINIO_AGENTES" in *[!A-Za-z0-9.-]*) fallo "nombre no válido: $DOMINIO $DOMINIO_AGENTES" ;; esac
case "$CORREO" in *[!A-Za-z0-9.@_+-]*) fallo "correo no válido: $CORREO" ;; esac
case "$SSH_DESDE" in *[!0-9a-fA-F.:/]*) fallo "red no válida para --ssh-desde: $SSH_DESDE" ;; esac
command -v systemctl >/dev/null || fallo "hace falta systemd."
INSTALAR="$AQUI/instalar-servidor.sh"
[ -f "$INSTALAR" ] || INSTALAR="$AQUI/../instalar-servidor.sh"
if [ ! -f "$INSTALAR" ] && { [ -n "$PAQUETE" ] || [ ! -x /opt/resguardo-server/resguardo-server ]; }; then
  fallo "falta instalar-servidor.sh junto a este script (viene en el .tar.gz de Resguardo Server)."
fi
[ -z "$DOMINIO_AGENTES" ] && DOMINIO_AGENTES="agentes.$DOMINIO"

export DEBIAN_FRONTEND=noninteractive

# --- 0. Memoria: con 1 GB o menos (Google e2-micro), 1 GB de swap para apt y compañía ---
MEM_MB="$(awk '/^MemTotal:/ {print int($2 / 1024)}' /proc/meminfo)"
if [ "${MEM_MB:-0}" -lt 1536 ] && [ -z "$(swapon --noheadings 2>/dev/null)" ] && [ ! -e /swapfile ]; then
  echo "== Memoria: ${MEM_MB} MB, se añade 1 GB de swap (/swapfile)"
  fallocate -l 1G /swapfile 2>/dev/null || dd if=/dev/zero of=/swapfile bs=1M count=1024 status=none
  chmod 600 /swapfile
  mkswap /swapfile >/dev/null
  swapon /swapfile
  grep -q '^/swapfile ' /etc/fstab || echo '/swapfile none swap sw 0 0' >> /etc/fstab
fi

# --- 1. Actualizaciones de seguridad automáticas ---
echo "== Actualizaciones de seguridad automáticas"
apt_get update -qq
apt_get install -y -qq unattended-upgrades fail2ban python3-systemd curl ca-certificates >/dev/null
cat > /etc/apt/apt.conf.d/20auto-upgrades <<'EOF'
// Resguardo (preparar-vm.sh): listas cada día y actualizaciones de seguridad
// automáticas (lo que dice 50unattended-upgrades: en Ubuntu, «-security»).
APT::Periodic::Update-Package-Lists "1";
APT::Periodic::Unattended-Upgrade "1";
APT::Periodic::AutocleanInterval "7";
EOF
systemctl enable --now unattended-upgrades >/dev/null 2>&1 || true

# --- 2. Cortafuegos: 22, 80 y 443 ---
abrir_iptables() {
  # Imágenes de Oracle Cloud: INPUT acaba en un REJECT; las reglas van antes.
  for p in 80 443; do
    iptables -C INPUT -p tcp -m state --state NEW --dport "$p" -j ACCEPT 2>/dev/null \
      || iptables -I INPUT "$(iptables -L INPUT --line-numbers | awk '/REJECT/ {print $1; exit}')" -p tcp -m state --state NEW --dport "$p" -j ACCEPT
    if command -v ip6tables >/dev/null && ip6tables -L INPUT 2>/dev/null | grep -q REJECT; then
      ip6tables -C INPUT -p tcp -m state --state NEW --dport "$p" -j ACCEPT 2>/dev/null \
        || ip6tables -I INPUT "$(ip6tables -L INPUT --line-numbers | awk '/REJECT/ {print $1; exit}')" -p tcp -m state --state NEW --dport "$p" -j ACCEPT
    fi
  done
  netfilter-persistent save >/dev/null 2>&1 || true
}
if [ "$CORTAFUEGOS" = 1 ]; then
  echo "== Cortafuegos: SSH, 80 y 443"
  if command -v netfilter-persistent >/dev/null && [ -f /etc/iptables/rules.v4 ] && grep -q REJECT /etc/iptables/rules.v4; then
    # Oracle Cloud: no se mezcla con ufw (las dos tocan las mismas cadenas).
    abrir_iptables
    if [ -n "$SSH_DESDE" ]; then echo "  (--ssh-desde: en Oracle Cloud, limítalo en la «Security List» de la red virtual.)"; fi
  else
    apt_get install -y -qq ufw >/dev/null
    ufw --force default deny incoming >/dev/null
    ufw --force default allow outgoing >/dev/null
    if [ -n "$SSH_DESDE" ]; then
      ufw allow from "$SSH_DESDE" to any port 22 proto tcp >/dev/null
      ufw delete allow OpenSSH >/dev/null 2>&1 || true
      ufw delete allow 22/tcp >/dev/null 2>&1 || true
    else
      ufw allow 22/tcp >/dev/null
    fi
    ufw allow 80/tcp >/dev/null
    ufw allow 443/tcp >/dev/null
    ufw --force enable >/dev/null
  fi
  echo "  Abre también 80 y 443 en el cortafuegos del proveedor (Oracle: «Security List»; Google: «VPC firewall»)."
fi

# --- 3. Consola en internet (antes de instalar: el primer arranque ya usa el 443) ---
echo "== Consola en internet: https://$DOMINIO (agentes: https://$DOMINIO_AGENTES)"
install -d -m 0755 /etc/resguardo-server
ENV=/etc/resguardo-server/servidor.env
[ -f "$ENV" ] || printf '# Configuración de Resguardo Server (la lee el servicio al arrancar).\n# Tras cambiarla: sudo systemctl restart resguardo-server\n' > "$ENV"
chmod 0644 "$ENV"
poner() {
  if grep -q "^#\{0,1\}$1=" "$ENV"; then sed -i "s|^#\{0,1\}$1=.*|$1=$2|" "$ENV"; else echo "$1=$2" >> "$ENV"; fi
}
quitar() { sed -i "/^$1=/d" "$ENV"; }
grep -q "Consola en internet (preparar-vm.sh)" "$ENV" || printf '\n# Consola en internet (preparar-vm.sh, docs/consola-en-linea.md)\n' >> "$ENV"
poner RESGUARDO_ESCUCHAR "0.0.0.0:443"
poner RESGUARDO_DOMINIO "$DOMINIO"
poner RESGUARDO_DOMINIO_AGENTES "$DOMINIO_AGENTES"
if [ -n "$CORREO" ]; then poner RESGUARDO_ACME_CORREO "$CORREO"; else quitar RESGUARDO_ACME_CORREO; fi
if [ "$PRUEBAS" = 1 ]; then poner RESGUARDO_ACME_PRUEBAS 1; else quitar RESGUARDO_ACME_PRUEBAS; fi
if [ -n "$DIRECTORIO" ]; then poner RESGUARDO_ACME_DIRECTORIO "$DIRECTORIO"; else quitar RESGUARDO_ACME_DIRECTORIO; fi
install -d -m 0755 /etc/systemd/system/resguardo-server.service.d
cat > /etc/systemd/system/resguardo-server.service.d/consola-en-linea.conf <<'EOF'
# Resguardo (preparar-vm.sh): los puertos 80 y 443 sin ser root. Solo este permiso.
[Service]
AmbientCapabilities=CAP_NET_BIND_SERVICE
CapabilityBoundingSet=CAP_NET_BIND_SERVICE
EOF

# --- 4. Resguardo Server ---
echo "== Resguardo Server"
if [ -n "$PAQUETE" ]; then
  if [ "$SIN_FIRMA" = 1 ]; then sh "$INSTALAR" --paquete "$PAQUETE" --sin-firma --puerto 443; else sh "$INSTALAR" --paquete "$PAQUETE" --puerto 443; fi
elif [ -x /opt/resguardo-server/resguardo-server ]; then
  # Ya instalado (con instalar-servidor.sh o con el .deb): solo la configuración.
  echo "  Ya instalado: se deja como está (para actualizar, --paquete con el nuevo)."
else
  sh "$INSTALAR" --puerto 443
fi
systemctl daemon-reload
systemctl restart resguardo-server
i=0
while [ $i -lt 20 ] && ! systemctl is-active --quiet resguardo-server; do sleep 1; i=$((i + 1)); done
systemctl is-active --quiet resguardo-server || { journalctl -u resguardo-server -n 30 --no-pager >&2; fallo "el servicio no arrancó (registro arriba)."; }

# --- 5. fail2ban ---
echo "== fail2ban"
F2B="$AQUI/../fail2ban"
[ -d "$F2B" ] || F2B="$AQUI/fail2ban"
if [ -f "$F2B/filter.d/resguardo-server.conf" ]; then
  install -m 0644 "$F2B/filter.d/resguardo-server.conf" /etc/fail2ban/filter.d/resguardo-server.conf
  install -m 0644 "$F2B/jail.d/resguardo-server.conf" /etc/fail2ban/jail.d/resguardo-server.conf
  systemctl enable fail2ban >/dev/null 2>&1 || true
  systemctl restart fail2ban
else
  echo "  Falta packaging/linux/fail2ban junto a este script: fail2ban sin el filtro de Resguardo Server."
fi

# --- 6. Copias de la consola para llevarlas fuera ---
echo "== Copias de la consola: /var/backups/resguardo-consola"
GRUPO="root"
if [ -n "$USUARIO_COPIAS" ] && id "$USUARIO_COPIAS" >/dev/null 2>&1; then GRUPO="$(id -gn "$USUARIO_COPIAS")"; fi
install -d -m 0750 -o root -g "$GRUPO" /var/backups/resguardo-consola
cat > /etc/systemd/system/resguardo-consola-fuera.service <<EOF
# Resguardo (preparar-vm.sh): deja las copias de la consola (ya cifradas con la
# clave de respaldo) donde se pueden llevar fuera sin sudo. Ver docs/consola-en-linea.md.
[Unit]
Description=Copias de Resguardo Server para llevar fuera
ConditionPathIsDirectory=/var/lib/resguardo-server/respaldos

[Service]
Type=oneshot
ExecStart=/bin/sh -c 'for f in /var/lib/resguardo-server/respaldos/*.resguardo-consola; do [ -f "\$\$f" ] && install -m 0640 -o root -g $GRUPO "\$\$f" /var/backups/resguardo-consola/; done; find /var/backups/resguardo-consola -name "*.resguardo-consola" -mtime +30 -delete'
EOF
cat > /etc/systemd/system/resguardo-consola-fuera.timer <<'EOF'
[Unit]
Description=Copias de Resguardo Server para llevar fuera (cada hora)

[Timer]
OnCalendar=hourly
Persistent=true

[Install]
WantedBy=timers.target
EOF
systemctl daemon-reload
systemctl enable --now resguardo-consola-fuera.timer >/dev/null

echo
echo "Listo. Lo que queda (docs/consola-en-linea.md):"
echo "  - Que $DOMINIO y $DOMINIO_AGENTES apunten a la IP pública de esta máquina."
echo "  - El certificado se pide solo (journalctl -u resguardo-server -f)."
echo "  - Entrar por primera vez: sudo resguardo-server codigo-inicial"
echo "  - En la consola, «Ajustes» → «Copia de la consola»: pon la clave de respaldo. Las copias"
echo "    aparecen en /var/backups/resguardo-consola; llévatelas fuera con scp o rclone."
