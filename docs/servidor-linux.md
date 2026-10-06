# Instalar Resguardo Server en Linux

Guía paso a paso para un contenedor (CT) de Proxmox con Debian 12, Ubuntu
22.04 o Ubuntu 24.04, sin escritorio. El servidor se administra desde el
navegador de otro equipo de la red. Vale igual para un Debian o un Ubuntu
normales.

Detalles técnicos (rutas, permisos, cómo se construyen los paquetes): ver
[packaging/linux/README.md](../packaging/linux/README.md).

> **Estado.** El servidor de Linux se compila y se prueba en la integración
> continua: allí se instala en un Ubuntu con systemd, se comprueba que el
> servicio arranca como su usuario, que la consola responde por HTTPS y que
> `codigo-inicial` funciona, y se quita. Todavía no se ha probado en un CT de
> Proxmox real. La instalación de una línea (`curl … | sudo sh`) se niega a
> instalar mientras no exista la llave pública de publicación; hasta
> entonces, instala desde el paquete copiado al CT (abajo).

Para una **consola en internet** (una máquina virtual en Oracle Cloud, Google
Cloud o Hetzner, con certificado público y varios clientes), ver
[consola-en-linea.md](consola-en-linea.md).

## Qué hace falta

- Un CT con **systemd** (las plantillas de Debian y Ubuntu de Proxmox lo
  traen). Sirve un CT **sin privilegios**, con **nesting** (anidamiento)
  activado: *CT → Opciones → Características → Anidamiento*. Las plantillas
  recientes lo traen activado. Sin él, el servicio no arranca (226/NAMESPACE).
- Unos **100 MB** libres en el disco raíz para el programa y sus datos
  (`/var/lib/resguardo-server`: cuentas, equipos y certificados; las
  descargas al navegador pasan por ahí, hasta 500 MB cada una).
- El **puerto 8443** libre (se puede cambiar).
- Los paquetes, de la integración continua (artefacto `servidor-linux-x86_64`
  del trabajo «Paquetes del servidor») o de una versión publicada:
  - `resguardo-server-x86_64-linux-musl.tar.gz`: estático, con la consola web
    dentro; vale para cualquier Linux x86_64 con systemd.
  - `resguardo-server_<versión>_amd64.deb`: lo mismo, como paquete de Debian.
  - `instalar-servidor.sh` y `SHA256SUMS`.

## 1. Copiar el paquete al CT

Los paquetes se publican en GitHub (Releases o los artefactos de la
integración continua). Puedes bajarlos en el propio CT con `wget`, o en tu PC
y copiarlos.

**Con `scp`** (viene con Windows 10 y 11), desde PowerShell en tu PC, si el
CT tiene SSH:

```powershell
cd <carpeta con los paquetes>
scp resguardo-server-x86_64-linux-musl.tar.gz instalar-servidor.sh SHA256SUMS root@192.168.1.30:/root/
```

(Con PuTTY: `pscp` con los mismos argumentos.)

**Sin SSH en el CT:** cópialos al anfitrión de Proxmox y, desde su consola,
mételos en el CT con `pct push` (`<id>` es el número del CT):

```sh
pct push <id> /root/resguardo-server-x86_64-linux-musl.tar.gz /root/resguardo-server-x86_64-linux-musl.tar.gz
pct push <id> /root/instalar-servidor.sh /root/instalar-servidor.sh
pct push <id> /root/SHA256SUMS /root/SHA256SUMS
```

**Cuando haya versiones publicadas** (repositorio público), desde el CT:

```sh
cd /root
wget https://github.com/evercarog/resguardo/releases/latest/download/resguardo-server-x86_64-linux-musl.tar.gz
wget https://github.com/evercarog/resguardo/releases/latest/download/instalar-servidor.sh
wget https://github.com/evercarog/resguardo/releases/latest/download/SHA256SUMS
```

## 2. Comprobar el paquete e instalar

En el CT (consola de Proxmox o SSH), como root:

```sh
cd /root
sha256sum -c --ignore-missing SHA256SUMS
sh instalar-servidor.sh --paquete ./resguardo-server-x86_64-linux-musl.tar.gz --sin-firma
```

- `sha256sum` debe decir `La suma coincide` (o `OK`) en las dos líneas.
  Compara también la SHA-256 con la que aparece en el registro del trabajo de
  la integración continua o en la página de la versión: si no coinciden, no
  instales.
- `--sin-firma`: el paquete de la integración continua no lleva firma
  minisign. Si el paquete trae su `.minisig` al lado y el script tiene la
  llave pública, se comprueba la firma y no hace falta esta opción.
- `--puerto 9443`: para escuchar en otro puerto (por defecto, 8443).

El script:

1. Crea el usuario de sistema `resguardo-server` (sin contraseña ni intérprete).
2. Deja el programa en `/opt/resguardo-server` (y el enlace
   `/usr/local/bin/resguardo-server`).
3. Crea `/var/lib/resguardo-server` (0700, solo para ese usuario) y la
   configuración `/etc/resguardo-server/servidor.env`.
4. Instala, activa y arranca el servicio `resguardo-server`.
5. Muestra la **dirección de la consola**, la **huella de la autoridad TLS** y
   el **código de primer arranque**:

```
Resguardo Server instalado y en marcha.
Consola: https://192.168.1.30:8443/
Huella de la autoridad TLS (la del certificado «Resguardo Server» en el navegador):
  3F:A1:…:9C
Código de primer arranque (para crear la cuenta de propietario):
  ABCD-1234
```

Para verlos otra vez en cualquier momento:

```sh
sudo resguardo-server codigo-inicial
journalctl -u resguardo-server -n 30 --no-pager   # el registro del arranque
```

El código cambia en cada arranque del servicio mientras no exista la cuenta
de propietario; vale el último. Después de crearla, desaparece.

### Con el `.deb` (alternativa)

```sh
apt install ./resguardo-server_<versión>_amd64.deb
resguardo-server codigo-inicial
```

(`<versión>`: la del paquete que descargaste, p. ej. `resguardo-server_0.7.10_amd64.deb`.)

Deja el programa en `/opt/resguardo-server` (enlace en `/usr/bin`), con el
mismo usuario, carpetas y servicio. No mezcles las dos formas en el mismo CT.

## 3. Abrir la consola desde otro equipo

En el navegador de tu PC, abre la dirección que mostró el script, por
ejemplo `https://192.168.1.30:8443/`.

El navegador avisa de que la conexión no es privada: el certificado lo firma
una autoridad propia del servidor («Resguardo Server»), no una pública. Antes
de aceptar, **comprueba la huella**:

- **Chrome / Edge:** pulsa el aviso «No es seguro» de la barra de dirección →
  *El certificado no es válido* → pestaña *Detalles* → en la jerarquía,
  selecciona **Resguardo Server** (la de arriba, no «Resguardo Server
  (servidor)») → *Huella digital SHA-256*.
- **Firefox:** *Avanzado…* → *Ver certificado* → pestaña **Resguardo Server**
  → *Huellas digitales* → *SHA-256*.

Debe coincidir con la que mostró `codigo-inicial` (son 32 pares como
`3F:A1:…`). Si coincide, acepta y continúa (*Avanzado → Acceder a
192.168.1.30 (sitio no seguro)*). Si **no** coincide, no sigas: algo se ha
puesto entre tu PC y el servidor.

Para no ver el aviso más: importa la autoridad en tu PC como «Entidad de
certificación raíz de confianza». Está en `/var/lib/resguardo-server/tls/ca.crt`
(y en `https://<servidor>:8443/api/servidor/ca`); compárala antes con la
huella.

## 4. Crear la cuenta de propietario (con verificación en dos pasos)

La consola abre la página **«Bienvenido a Resguardo Server»**:

1. **Código de primer arranque:** el que mostró `codigo-inicial`.
2. **Tu nombre, correo y contraseña** (al menos 12 caracteres).
3. **Activa la verificación en dos pasos:** escanea el código QR con una app
   de autenticación (Google Authenticator, Microsoft Authenticator, Aegis,
   1Password…) y escribe los 6 dígitos que muestra. Es obligatoria.
4. **Códigos de recuperación:** guárdalos fuera del servidor (en papel o en
   un gestor de contraseñas). Sirven si pierdes el móvil.

Después ya puedes crear clientes y añadir equipos (botón «Añadir equipo»: da
el código de vinculación de cada agente).

## 5. El agente en el mismo CT («Este equipo guarda copias»)

El mismo CT puede tener también **Resguardo Agente**, para que guarde las
copias de los equipos de la oficina (el rest-server de restic en modo solo
añadir). Guía completa: [docs/agente-linux.md](agente-linux.md).

En un CT como el de este ejemplo (Ubuntu 22.04, datos en `/mnt/restic`, otro
rest-server en los puertos 8000 y 8001), los pasos exactos están en
[docs/agente-linux.md, «Ejemplo: Resguardo Server y agente en el mismo CT»](agente-linux.md#ejemplo-resguardo-server-y-agente-en-el-mismo-ct).
En resumen:

- **Se instala igual que el servidor:** se copian
  `resguardo-agente-x86_64-linux-musl.tar.gz`, `instalar-agente.sh` y
  `SHA256SUMS`, y
  `sh instalar-agente.sh --paquete ./resguardo-agente-x86_64-linux-musl.tar.gz --sin-firma`.
  El paquete trae los restic, rest-server y rclone oficiales (con sus huellas
  fijadas): no hace falta el restic del CT ni fijar a mano la huella del
  rest-server, y no se toca el rest-server que ya está en marcha.
- **Vincularlo con el servidor del mismo CT:**
  `resguardo-agente vincular <código> --servidor https://127.0.0.1:8443`.
- **Carpeta de las copias en `/mnt/restic`, nunca en el disco raíz** (10 GB):
  por ejemplo `/mnt/restic/resguardo`. No uses `/mnt/restic` ni
  `/mnt/restic/equipos` directamente: son del rest-server que ya existe.
- **Puerto distinto de 8000 y 8001** (los usa el rest-server que ya existe;
  el agente propone 8000 por defecto y, si está ocupado, no lo activa). Por
  ejemplo, **8002**.

Comprueba después que todos siguen escuchando: `ss -ltnp | grep -E ':(8000|8001|8002|8443) '`.

**Más corto (agente ≥ 0.7.7, consola v1.19):** con `instalar-servidor.sh
--con-agente` (y el paquete del agente e `instalar-agente.sh` junto al
script; con `--sin-firma` si el del servidor también lo lleva) se instala el
agente a la vez, **sin vincular**. Después, en la consola, «Vincular este
servidor» (en Primeros pasos o en «Añadir equipo»): el agente se une solo a
`https://127.0.0.1:<puerto>`, compruebas el número (`sudo resguardo-agente
registro 5`), lo das de alta con la clave de administración y «Usarlo como
almacén» abre «Este equipo guarda copias» con un puerto libre propuesto
(8002 si el 8000 está ocupado) y «Explorar…» para elegir la carpeta.

En un CT sin privilegios, nftables dentro del contenedor puede no estar
permitido: entonces desmarca «Solo redes internas» (o `--toda-la-red`) y
limita el puerto con el cortafuegos de Proxmox (abajo).

## Cortafuegos

Los instaladores del servidor no tocan el cortafuegos. En un CT recién creado
no hay ninguno: el puerto 8443 queda abierto a quien llegue al CT. Lo
recomendable es abrirlo solo a las redes privadas de la oficina (10.0.0.0/8,
172.16.0.0/12 y 192.168.0.0/16), nunca a Internet, y no abrir puertos en el
router.

- **Cortafuegos de Proxmox** (recomendado para un CT): *CT → Cortafuegos →
  Añadir*: dirección *in*, acción *ACCEPT*, protocolo *tcp*, puerto de
  destino *8443*, origen `192.168.1.0/24` (tu red). Actívalo en *Opciones*
  con la política de entrada en *DROP* (y abre también el SSH y los puertos
  8000, 8001 y 8002 si los usas).
- **ufw** (si ya lo usas en el CT):

  ```sh
  ufw allow from 192.168.0.0/16 to any port 8443 proto tcp
  ufw allow from 10.0.0.0/8 to any port 8443 proto tcp
  ufw allow from 172.16.0.0/12 to any port 8443 proto tcp
  ```

- **nftables** (en un CT con privilegios o en una máquina normal), con su
  propia tabla, como hace el agente con su Servidor de copias:

  ```sh
  nft add table inet resguardo-server
  nft add chain inet resguardo-server entrada '{ type filter hook input priority 0; policy accept; }'
  nft add rule inet resguardo-server entrada tcp dport 8443 ip saddr '{ 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 127.0.0.0/8 }' accept
  nft add rule inet resguardo-server entrada tcp dport 8443 ip6 saddr '{ fe80::/10, ::1 }' accept
  nft add rule inet resguardo-server entrada tcp dport 8443 drop
  ```

  Estas reglas no sobreviven a un reinicio: guárdalas en `/etc/nftables.conf`.

## El instalador listo de Windows

En «Añadir equipo», la consola ofrece **Descargar instalador listo** para
Windows (el equipo se vincula solo al instalarlo) si el servidor tiene el
instalador genérico del agente, `Resguardo-Agente-setup.exe`. Resguardo Server
para Windows lo trae; el paquete de Linux no (se compila en Windows). Mientras
no lo tenga, la consola ofrece «Instalador normal y un código».

Para tenerlo también en Linux, copia `Resguardo-Agente-setup.exe` de la misma
publicación al CT, comprueba su SHA-256 con el `SHA256SUMS` de la publicación
y ponlo:

```sh
sha256sum Resguardo-Agente-setup.exe     # compáralo con SHA256SUMS
sudo resguardo-server poner-instalador-agente Resguardo-Agente-setup.exe --sha256 <ese valor>
```

Comprueba que es un ejecutable de Windows y que no es un instalador ya
preparado para un equipo (esos llevan un código dentro), enseña su SHA-256 y,
con `--sha256`, se niega si no es ese. Lo deja en
`/var/lib/resguardo-server/agente/Resguardo-Agente-setup.exe`, solo para el
usuario del servicio (carpeta 0700, archivo 0600). Vale al momento, sin
reiniciar; para cambiarlo por el de una versión nueva, se repite.

Si prefieres dejarlo en otro sitio: `RESGUARDO_INSTALADOR_AGENTE=/ruta/al/setup.exe`
en `servidor.env` (o `--instalador-agente`), y `systemctl restart resguardo-server`.

## Configuración

`/etc/resguardo-server/servidor.env` (después: `systemctl restart resguardo-server`):

| Variable | Para qué |
| --- | --- |
| `RESGUARDO_ESCUCHAR` | Dirección y puerto (por defecto `0.0.0.0:8443`). Para un puerto por debajo de 1024 hace falta además `AmbientCapabilities=CAP_NET_BIND_SERVICE` (con `systemctl edit resguardo-server`). |
| `RESGUARDO_NOMBRES` | Nombres o IP extra para el certificado (separados por comas), p. ej. `resguardo.oficina.lan`. El nombre del CT y sus IP ya van siempre. |
| `RESGUARDO_MAX_DESCARGA` | Tamaño máximo de una descarga al navegador, en MB (por defecto 500). |
| `RESGUARDO_INSTALADOR_AGENTE` | Otro instalador del agente para «Descargar instalador listo» (por defecto, el de `poner-instalador-agente`: ver arriba). |
| `RESGUARDO_DOMINIO`, `RESGUARDO_ACME_CORREO`… | Consola en internet con certificado público: ver [consola-en-linea.md](consola-en-linea.md). |
| `RESGUARDO_DETRAS_DE_PROXY=1` | Hay un proxy con HTTPS (Caddy, nginx) delante **en este mismo CT**: la IP de quien pide es la que el proxy pone en `X-Forwarded-For`. |
| `RESGUARDO_PROXY_RED` | El proxy está **en otro CT, contenedor o máquina**: su red en CIDR (p. ej. `172.18.0.0/16` o `10.0.5.10/32`; varias, separadas por comas). Implica `RESGUARDO_DETRAS_DE_PROXY=1`. Ver abajo. |

El certificado del servidor se vuelve a emitir en cada arranque (por si
cambia la IP), siempre con la misma autoridad: la huella que comprobaste no
cambia y los agentes vinculados siguen confiando en él.

### Proxy en otro CT o contenedor (`RESGUARDO_PROXY_RED`)

Sin decir nada, el servidor solo cree `X-Forwarded-For` en las conexiones
que llegan del propio equipo (127.0.0.1). Si el proxy está en otro CT de
Proxmox o en un contenedor de Docker, todas las peticiones llegarían con la
IP del proxy y los límites por IP (intentos de entrada, fallos de agentes)
serían de todos a la vez: quien fallara contraseñas bloquearía a todos.

Con `RESGUARDO_PROXY_RED=172.18.0.0/16` (o `--proxy-red 172.18.0.0/16`, que
se puede repetir), las conexiones que llegan de esa red también son de un
proxy de confianza. La IP de quien pide es la de `X-Forwarded-For` **más a la
derecha que no es de un proxy de confianza** (lo que añadió cada proxy; lo de
su izquierda lo pudo escribir cualquiera). Pon la red más pequeña que cubra
los proxies (mejor su IP con `/32`): cualquiera que pueda conectar desde esa
red puede decir que es otra IP. No se admite `/0`.

## Copia de la consola

En la consola, como propietario del servidor: **Servidor → Copia de la consola**
→ «Poner la clave de respaldo…». Elige una frase larga (no la de
administración), imprime el kit y guárdalo fuera del CT. Desde entonces, cada
noche (y al arrancar una versión nueva) el servidor deja una copia cifrada de
sí mismo en `/var/lib/resguardo-server/respaldos/` y conserva las más
recientes. El servidor no conoce la clave: puede hacer las copias, pero no
abrirlas. También a mano: `sudo -u resguardo-server resguardo-server hacer-respaldo`.

Para que salgan del CT, añade `/var/lib/resguardo-server/respaldos` a una
copia del agente del mismo CT (sección 5: en su equipo, «Cambiar las copias»
→ «Añadir una copia», por ejemplo «Consola de Resguardo»). Si ese agente es
también el almacén, la copia puede ir a un repositorio de su propio almacén
(«Copiar en este mismo almacén»): así la cubren su espejo y la nube. La
configuración de `/etc/resguardo-server/servidor.env` no va dentro: apunta su
dirección y nombres.

**Restaurar en otro CT** (con la misma IP o nombre):

```sh
sudo sh instalar-servidor.sh                  # el paquete de siempre (paso 2)
sudo systemctl stop resguardo-server
sudo resguardo-server restaurar-respaldo /root/consola-AAAAMMDD-HHMMSS-mmm.resguardo-consola --reemplazar --confiar-en AB:CD:…
sudo systemctl start resguardo-server
```

Antes de nada comprueba la firma de la copia y la huella de la identidad del
servidor que la hizo: con `--confiar-en` (la huella del kit) tiene que ser esa;
sin él, la enseña y pide confirmarla. Las copias de antes de 0.7.11 no llevan
firma: lo avisa. Después pide la clave de respaldo (no se ve al escribir). `--reemplazar` aparta lo que
dejó la instalación nueva en `antes-de-restaurar-…` (no se borra) y los
archivos quedan del usuario `resguardo-server`. Los agentes reconocen la misma
identidad y vuelven solos. Con una clave equivocada no se toca nada.

## Actualizar

Copia el `.tar.gz` nuevo al CT y repite el paso 2. Con la copia de la consola
puesta, el servidor hace una antes de abrir la base de datos con la versión
nueva. Se conservan los datos, la
configuración, la autoridad TLS (la misma huella) y las cuentas. Con el
`.deb`: `apt install ./resguardo-server_<nueva>_amd64.deb`.

## Quitar

```sh
sh /opt/resguardo-server/instalar-servidor.sh --desinstalar            # conserva /var/lib/resguardo-server
sh /opt/resguardo-server/instalar-servidor.sh --desinstalar --purgar   # borra también datos, configuración y usuario
```

Con el `.deb`: `apt remove resguardo-server` (conserva los datos) o
`apt purge resguardo-server` (los borra).

Ojo: borrar los datos borra las cuentas, los equipos vinculados y la
autoridad TLS (y las copias de la consola de `respaldos/`, si no salieron del
CT). Los agentes tendrían que vincularse de nuevo, salvo que restaures una copia
de la consola. Las copias de los equipos no se tocan: están en sus repositorios.

## Si algo falla

| Lo que ves | Qué hacer |
| --- | --- |
| El servicio falla con `226/NAMESPACE` | Activa *Anidamiento* (nesting) en las opciones del CT y reinícialo. |
| `codigo-inicial`: «ejecútalo con sudo» | Los datos son solo del usuario `resguardo-server`: usa `sudo`. |
| `codigo-inicial`: «ya tiene la cuenta de propietario» | Entra con esa cuenta. Si la has perdido del todo, borra los datos (`--desinstalar --purgar`) y empieza de nuevo. |
| `No se pudo escuchar en 0.0.0.0:8443` | Otro programa usa el puerto (`ss -ltnp | grep 8443`): cambia `RESGUARDO_ESCUCHAR`. |
| El navegador no llega | Comprueba la IP (`hostname -I`), el cortafuegos y que el servicio está activo (`systemctl status resguardo-server`). |
| `restaurar-respaldo`: «La clave de respaldo no es la de esta copia» | Es otra clave (o la de otra copia): busca la del kit de la copia de la consola. No se ha tocado nada. |
| `restaurar-respaldo`: «Ya hay un servidor» | Para el servicio y repite con `--reemplazar`. |
| La huella no coincide | No continúes: comprueba que abres la IP correcta y que no hay un proxy en medio. |
