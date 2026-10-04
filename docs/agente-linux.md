# Instalar Resguardo Agente en Linux

Guía paso a paso para Debian 12 o posterior, Ubuntu 22.04 o posterior y un
contenedor (CT) de Proxmox. El agente hace las copias de este equipo con
restic y, si lo vinculas, lo administras desde la consola de Resguardo Server.

Detalles técnicos del empaquetado (rutas, permisos, cortafuegos): ver
[packaging/linux/README.md](../packaging/linux/README.md).
Resguardo Server en el mismo equipo o CT: ver
[docs/servidor-linux.md](servidor-linux.md).

> **Estado.** El agente de Linux se compila y se prueba en la integración
> continua, que también lo instala con `instalar-agente.sh` en Ubuntu, activa
> «Este equipo guarda copias» y lo quita. La instalación de una línea
> (descarga desde GitHub) se niega a instalar mientras no exista la llave
> pública de publicación: no se descarga nada sin comprobar la firma. Hasta
> entonces, copia al equipo el `.tar.gz` (o el `.deb`) y comprueba su SHA-256
> (abajo).

## Qué necesitas

- **Debian 12 o posterior, Ubuntu 22.04 o posterior** (o un CT de Proxmox con
  ellos), en x86_64, con **systemd**.
- **root** (o `sudo`): el agente tiene que poder leer todo lo que copia.
- **Nada más.** El paquete trae los programas oficiales que usa el agente,
  con sus huellas comprobadas y sus licencias. No importa el restic de la
  distribución (Ubuntu 22.04 trae la 0.12, demasiado antigua):
  - **restic 0.19.1:** las copias. El agente usa el que está junto a él
    (`/opt/resguardo-agente/restic`), nunca el del `PATH`.
  - **rest-server 0.14.0:** «Este equipo guarda copias». El agente comprueba
    su huella antes de cada arranque.
  - **rclone 1.75.1:** el espejo de esas copias en una nube.
- Para vincularlo: **el código de vinculación** que da la consola de
  Resguardo Server (página del cliente, botón «Añadir equipo») y la dirección
  del servidor (por ejemplo, `https://192.168.1.20:8443`).

## Instalar con el `.tar.gz` (recomendado)

Copia al equipo `resguardo-agente-x86_64-linux-musl.tar.gz`,
`instalar-agente.sh` y `SHA256SUMS` (los tres en la misma carpeta) y:

```sh
sudo sh instalar-agente.sh --paquete ./resguardo-agente-x86_64-linux-musl.tar.gz --sin-firma
```

El script:

1. Comprueba el paquete con el `SHA256SUMS` de al lado (si no coincide, no
   instala nada) y muestra su SHA-256. Sin `.minisig` (firma) hay que poner
   `--sin-firma` a propósito.
2. Lo instala en `/opt/resguardo-agente` (agente, restic, rest-server, rclone
   y licencias), con el enlace `/usr/local/bin/resguardo-agente`.
3. Instala nftables si falta (el cortafuegos de «Este equipo guarda copias»).
4. Pone el servicio `resguardo-agente` de systemd, lo activa y lo arranca.
5. Muestra las versiones y el siguiente paso: vincularlo.

Para vincularlo en el mismo paso, añade
`--servidor https://192.168.1.20:8443 --codigo ABCD-1234` (y, si quieres,
`--nombre "Servidor de la oficina"`).

- **Actualizar:** lo mismo con el paquete nuevo. La configuración, los
  secretos y el Servidor de copias se conservan (se para y se vuelve a
  arrancar).
- **Quitar:** `sudo sh /opt/resguardo-agente/instalar-agente.sh --desinstalar`
  (la configuración y los secretos se quedan en `/var/lib/resguardo-agente`).
- **Quitar del todo:** añade `--purgar`. Las copias, que están en sus
  repositorios, y la carpeta del Servidor de copias no se tocan nunca.

Cuando haya llave de publicación, en una línea (descarga y comprueba la firma):

```sh
curl -fsSL https://github.com/evercarog/resguardo/releases/latest/download/instalar-agente.sh \
  | sudo sh -s -- --servidor https://192.168.1.20:8443 --codigo ABCD-1234
```

## Instalar con el `.deb`

```sh
sudo apt install ./resguardo-agente_<versión>_amd64.deb   # p. ej. resguardo-agente_0.7.10_amd64.deb
sudo resguardo-agente vincular ABCD-1234 --servidor https://192.168.1.20:8443
```

Lleva lo mismo que el `.tar.gz` (no depende del restic de la distribución).

- **Actualizar:** instala el `.deb` nuevo igual. El Servidor de copias, si
  estaba en marcha, se reinicia con el `rest-server` nuevo.
- **Quitar:** `sudo apt remove resguardo-agente`. **Quitar del todo:**
  `sudo apt purge resguardo-agente`.
- No mezcles: si lo instalaste con el `.deb`, el script se niega a instalar
  encima (y al revés, quítalo con `--desinstalar` antes de usar el `.deb`).

## Vincularlo con Resguardo Server

```sh
sudo resguardo-agente vincular ABCD-1234 --servidor https://192.168.1.20:8443
```

- `ABCD-1234` es el código de la consola (cliente → «Añadir equipo»): sirve
  una vez y caduca.
- Al vincular, el agente muestra la huella de la autoridad TLS del servidor y
  un **código de comprobación**. Comprueba en la consola que ve el mismo y
  confírmalo allí.
- **Si Resguardo Server está en el mismo equipo o CT,** usa
  `https://127.0.0.1:8443`: el certificado del servidor cubre siempre
  `127.0.0.1` y `localhost`, la autoridad se fija igual que con la IP, el
  tráfico no sale del equipo y no depende de que cambie su IP.

## Comprobar que funciona

```sh
sudo resguardo-agente estado         # versión, servidor, copias y últimos errores
sudo resguardo-agente registro 30    # las últimas líneas del registro
journalctl -u resguardo-agente -n 30 --no-pager
```

## En un contenedor (CT) de Proxmox

Igual que en Debian, con estas diferencias:

1. **Plantilla:** Debian 12 o Ubuntu 24.04. Sirve un CT sin privilegios.
2. **Qué copiar:** el agente ve lo que ve el CT. Para copiar datos del
   anfitrión o de otros CT, móntalos en el CT como puntos de montaje
   (`mp0: /ruta/en/el/anfitrión,mp=/datos`, en *Recursos → Añadir → Punto de
   montaje*). En un CT sin privilegios, los archivos del anfitrión pueden
   aparecer como de `nobody`: si el agente no puede leerlos, la copia termina
   con «algunos archivos no se pudieron leer».
3. **Sin instantáneas de disco:** en un CT no hay VSS ni instantáneas. Para
   bases de datos, usa los ganchos de la consola («Antes de copiar»).
4. **Servidor de copias en el CT:** en un CT sin privilegios, nftables dentro
   del contenedor puede no estar permitido. Actívalo con `--toda-la-red` y
   limita el acceso con el cortafuegos de Proxmox (en el CT, *Cortafuegos →
   Añadir*: el puerto, solo desde la red de la oficina).
5. **Hora:** el CT usa el reloj del anfitrión. Si la hora del anfitrión está
   mal, las copias programadas se adelantan o se retrasan; el agente se
   recupera solo cuando se corrige.

## Guardar las copias de otros equipos

Desde la consola: *el equipo → «Este equipo guarda copias»* → la carpeta
(en un disco con espacio) y el puerto. O en el equipo:

```sh
sudo resguardo-agente guardar-copias activar --carpeta /srv/copias [--puerto 8000] [--toda-la-red]
sudo resguardo-agente guardar-copias anadir pc-recepcion
sudo resguardo-agente guardar-copias estado
```

- **Puerto:** 8000 por defecto (el habitual de rest-server). Si ya lo usa
  otro programa en ese equipo, el agente no lo activa («El puerto 8000 ya está
  en uso en este equipo: elige otro») y no toca a ese programa: elige otro,
  por ejemplo 8002.
- **El rest-server** es el oficial del paquete
  (`/opt/resguardo-agente/rest-server`). El agente comprueba su huella SHA-256
  antes de cada arranque (ver
  [packaging/linux/README.md](../packaging/linux/README.md)).
- **«Solo redes internas»** (lo predeterminado) pone una tabla propia de
  nftables solo para ese puerto. Si el equipo tiene otro cortafuegos que lo
  bloquea todo, abre el puerto también ahí.

## Ejemplo: Resguardo Server y agente en el mismo CT

Un CT de ejemplo: Ubuntu 22.04, IP `192.168.1.30`, Resguardo Server ya
instalado ([docs/servidor-linux.md](servidor-linux.md)), datos en
`/mnt/restic` y **otro rest-server en los puertos 8000 y 8001 que no se
toca**. El agente guardará las copias en `/mnt/restic/resguardo`, puerto
**8002**.

**1. Copiar los archivos desde tu PC** (PowerShell):

```powershell
cd <carpeta con los paquetes>
Get-FileHash resguardo-agente-x86_64-linux-musl.tar.gz -Algorithm SHA256   # compárala con SHA256SUMS
scp resguardo-agente-x86_64-linux-musl.tar.gz instalar-agente.sh SHA256SUMS root@192.168.1.30:/root/
```

**2. Instalar** (en el CT, como root):

```sh
cd /root
sh instalar-agente.sh --paquete ./resguardo-agente-x86_64-linux-musl.tar.gz --sin-firma
```

Debe decir «El paquete coincide con su SHA256SUMS», mostrar la misma SHA-256
que viste en tu PC y terminar con «Resguardo Agente instalado y en marcha»
y las versiones de restic 0.19.1, rest-server 0.14.0 y rclone 1.75.1. No
cambia nada del restic ni del rest-server que ya tenía el CT.

**3. Vincularlo con Resguardo Server del mismo CT.** En la consola
(`https://192.168.1.30:8443`): el cliente → «Añadir equipo» → copia el
código. En el CT:

```sh
resguardo-agente vincular ABCD-1234 --servidor https://127.0.0.1:8443 --nombre ALMACEN-01
```

Comprueba en la consola el código de comprobación y confirma el equipo.

**4. «Este equipo guarda copias»** (en la consola): en la ficha del equipo
`ALMACEN-01`, la tarjeta «Este equipo puede guardar copias» → botón
**«Este equipo guarda copias»**:

- **Carpeta:** `/mnt/restic/resguardo` (no `/mnt/restic` ni
  `/mnt/restic/equipos`, que son del rest-server que ya existe; nunca en el
  disco raíz del CT).
- **Puerto:** `8002` (no 8000 ni 8001). Si te equivocas y pones uno ocupado,
  el agente no lo activa y no toca al que lo usa.
- **Solo redes internas:** marcado. Si falla con un error de nftables (CT sin
  privilegios que no lo permite), repítelo desmarcado y limita el puerto 8002
  con el cortafuegos de Proxmox (*CT → Cortafuegos → Añadir*: TCP 8002, origen
  la red de la oficina).

O, en el CT, lo mismo con:
`resguardo-agente guardar-copias activar --carpeta /mnt/restic/resguardo --puerto 8002`.

**5. Comprobar** (en el CT): los tres siguen escuchando, cada uno en su puerto.

```sh
ss -ltnp | grep -E ':(8000|8001|8002|8443) '
resguardo-agente guardar-copias estado
```

Después, en la consola, en cada equipo que tenga que copiar aquí: «Copiar en
ALMACEN-01».

## Compilar el agente

```sh
rustup target add x86_64-unknown-linux-musl
sudo apt install musl-tools
cargo build --release -p resguardo-agente --bin resguardo-agente --target x86_64-unknown-linux-musl
sh packaging/linux/construir-paquetes.sh \
  src-tauri/target/x86_64-unknown-linux-musl/release/resguardo-agente <versión> x86_64 paquetes
```

Antes, `sh scripts/fetch-binarios-linux.sh src-tauri/target/x86_64-unknown-linux-musl/release`
deja junto al binario los restic, rest-server y rclone oficiales (sin ellos no
se hace el paquete de x86_64). En `paquetes/` quedan el `.deb`, el `.tar.gz`
(sin firmar), `instalar-agente.sh` y `SHA256SUMS`.

## Si algo falla

| Lo que ves | Qué hacer |
| --- | --- |
| `vincular`: «No se pudo conectar con el servidor» | Comprueba la dirección y el puerto, y que el equipo llega al servidor (`curl -k https://servidor:8443/`). |
| `vincular`: «El código no es válido o ha caducado» | Pide un código nuevo en la consola: cada código sirve una vez y caduca. |
| `vincular`: «El certificado del servidor ha cambiado…» | Vuelve a intentarlo. Si se repite, puede haber algo entre este equipo y el servidor: compruébalo antes de seguir. |
| «No se encontró restic» | Vuelve a instalar el agente: el paquete trae restic. |
| «El puerto N ya está en uso en este equipo» | Otro programa usa ese puerto: elige otro en «Este equipo guarda copias» (`ss -ltnp` muestra los ocupados). |
| «Falta nftables» o «No se pudieron aplicar las reglas de nftables» | `apt install nftables`; en un CT sin privilegios que no lo permite, desmarca «Solo redes internas» y usa el cortafuegos de Proxmox. |
| `instalar-agente.sh`: «el paquete no coincide con su SHA256SUMS» | El archivo se copió mal o no es el publicado: vuelve a copiarlo. |
