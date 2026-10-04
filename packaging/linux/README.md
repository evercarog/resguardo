# Resguardo Agente en Linux

Agente de Linux para Debian 12 o Ubuntu 22.04 y posteriores. También vale
para un contenedor (CT) de Proxmox. Diseño: [docs/plataforma.md](../../docs/plataforma.md), §2.1 y §4.
Guía de instalación paso a paso: [docs/agente-linux.md](../../docs/agente-linux.md).

| Archivo | Para qué |
| --- | --- |
| `instalar-agente.sh` | Instala, actualiza o quita Resguardo Agente. Con la versión publicada comprueba la firma minisign; con `--paquete X.tar.gz --sin-firma`, el `SHA256SUMS` de al lado (si lo hay) y muestra la SHA-256 del paquete. |
| `resguardo-agente.service` | Unidad de systemd del agente. Corre como root y con prioridad baja de CPU y disco. |
| `construir-paquetes.sh` | Crea el `.tar.gz` (el que instala el script) y el `.deb` a partir de un binario compilado (`servidor` como primer argumento: los de Resguardo Server). |
| `instalar-servidor.sh` | Instala, actualiza o quita Resguardo Server (ver abajo). |
| `resguardo-server.service` | Unidad de systemd de Resguardo Server: usuario propio sin privilegios y endurecida. |
| `nube/preparar-vm.sh` | Consola en internet: prepara una VM nueva de Ubuntu (actualizaciones automáticas, cortafuegos 22/80/443, el servidor en el 443 con `--dominio`, fail2ban y las copias de la consola para llevarlas fuera). Va dentro del `.tar.gz` y del `.deb` del servidor. Ver [docs/consola-en-linea.md](../../docs/consola-en-linea.md). |
| `nube/cloud-init.yaml` | Lo básico para pegar al crear la VM (actualizaciones y paquetes). |
| `fail2ban/` | Filtro y jaula de fail2ban para las líneas «Acceso fallido desde <IP>» del servidor. |

## Dónde queda cada cosa

- **Binarios:** `/opt/resguardo-agente/`. Contiene `resguardo-agente` y, en x86_64, los binarios oficiales que trae el paquete (`.tar.gz` y `.deb`), con sus licencias:
  - `restic` 0.19.1 (el mismo que el instalador de Windows): las copias.
  - `rest-server` 0.14.0: «Este equipo guarda copias». Huella fijada en `crates/agente/src/server.rs`.
  - `rclone` 1.75.1: espejo del Servidor de copias en una nube. Huella fijada en `crates/agente/src/nube.rs`.
  - Los baja de las publicaciones oficiales de GitHub, comprobando la huella del archivo y la del binario, `scripts/fetch-binarios-linux.sh`. `construir-paquetes.sh` vuelve a comprobarlas y no hace el paquete de x86_64 sin ellos.
  - También hay un enlace en `/usr/local/bin` (script) o en `/usr/bin` (`.deb`).
- **restic:** el que está junto al agente (el del paquete). Si no hay ninguno (aarch64), `/usr/bin/restic`, el de la distribución. El agente nunca busca restic en el `PATH`. El `.deb` ya no depende del paquete `restic` de la distribución (Ubuntu 22.04 trae la 0.12, demasiado antigua).
- **Datos:** `/var/lib/resguardo-agente/` (0755, de root).
  - Los secretos (contraseñas de los repositorios y llaves del vínculo) van en `privado/` (0700).
  - Las solicitudes de copia de los usuarios van en `solicitudes/` (1733, como `/tmp`).
- **Secretos en reposo:** en Linux no hay DPAPI. Los secretos quedan protegidos por los permisos de root, como los de cualquier servicio del sistema. Pendiente: `systemd-creds` con TPM2 donde lo haya. El almacén de credenciales del usuario (secret-service) no se usa: el agente no tiene sesión de usuario.
- **Registro:** `journalctl -u resguardo-agente` y `resguardo-agente registro`.

## Órdenes

```sh
resguardo-agente ayuda
resguardo-agente vincular <código> --servidor https://<servidor>:8443
resguardo-agente estado
resguardo-agente --install-service      # crea y arranca la unidad (si no la puso el paquete)
resguardo-agente --uninstall-service
```

## Guardar las copias de otros equipos (nodo de almacenamiento)

El agente puede hacer de **Servidor de copias**: el rest-server oficial de restic.

- **Modo:** siempre `--append-only --private-repos`. Cada equipo cliente solo puede añadir a su carpeta.
- **TLS:** con una autoridad propia.
- **Arranque:** su propio servicio, `resguardo-guarda-copias.service`. Lo genera el agente; solo puede escribir en la carpeta de las copias y en la del agente.

```sh
resguardo-agente guardar-copias activar --carpeta /srv/copias [--puerto 8000] [--toda-la-red]   # puerto ocupado: no lo activa
resguardo-agente guardar-copias anadir pc-recepcion      # usuario, contraseña, ubicación y huella TLS
resguardo-agente guardar-copias estado
resguardo-agente guardar-copias quitar pc-recepcion
resguardo-agente guardar-copias desactivar
```

- **Huella del rest-server.** Antes de cada arranque, el agente comprueba la huella SHA-256 de `/opt/resguardo-agente/rest-server`.
  - En x86_64 va fijada en el agente: la del rest-server 0.14.0 que trae el paquete. Se puede sustituir al compilar (`RESGUARDO_REST_SERVER_SHA256`).
  - Si no la llevan (aarch64), la fija el administrador en `/etc/resguardo-agente/rest-server.sha256`: la huella, o la línea correspondiente del `SHA256SUMS` del proyecto rest-server. Ese archivo debe ser de root y sin escritura para nadie más.
  - Sin huella válida, el servidor de copias no arranca.
- **Cortafuegos (nftables).** Con «solo la red local», que es lo predeterminado, el agente crea su propia tabla `inet resguardo`.
  - Esa tabla acepta el puerto desde las redes IPv4 del equipo (las que da `ip -o -4 addr`), desde `fe80::/10` y desde `lo`, y descarta todo lo demás.
  - Las reglas se vuelven a poner en cada arranque del servicio.
  - Con `--toda-la-red` no se pone ninguna regla.
  - Si el equipo tiene otro cortafuegos que lo bloquea todo (ufw, firewalld o el de Proxmox), también hay que abrir el puerto en ese cortafuegos. En nftables, aceptar en una tabla no anula lo que descarta otra.
  - Nunca se abren puertos en el router.

## Qué falta probar en un CT real

No hay Linux en el equipo de desarrollo: lo de Linux lo compila y prueba la CI (`cargo clippy` y `cargo test -p resguardo-agente`). Falta comprobar en un CT de Proxmox (Debian 12):

1. **Servicio del agente.**
   - `systemctl enable --now resguardo-agente` y `systemctl stop`: debe registrar «detenido por un administrador».
   - Un reinicio del CT no debe registrar eso.
2. **Copia programada completa.** Comprobar el bloqueo con `flock` y que solo corre un proceso de tareas a la vez.
3. **Permisos.** Comprobar los permisos de `/var/lib/resguardo-agente`, `privado/` y `solicitudes/` tras `prepare_dir`.
4. **Vinculación con Resguardo Server.** `resguardo-agente vincular …` y órdenes desde la consola.
5. **Servidor de copias.**
   - `guardar-copias activar` y `anadir`.
   - Una copia desde un Windows de la red.
   - Que con nftables el puerto quede cerrado desde fuera de la red local.
   - En un CT **sin privilegios**, nftables dentro del contenedor puede no estar permitido. Entonces hay que usar el cortafuegos de Proxmox y `--toda-la-red`.
6. **Paquetes.**
   - Instalar el `.deb` (`apt install ./resguardo-agente_….deb`), desinstalarlo y purgarlo.
   - `instalar-agente.sh --paquete … --sin-firma`, actualizar encima, `--desinstalar` y `--purgar`: lo prueba la CI en Ubuntu (también «guardar copias» con el rest-server del paquete). Falta en un CT real y con una llave minisign de prueba.

## Resguardo Server en Linux

Guía paso a paso (CT de Proxmox con Debian 12): [docs/servidor-linux.md](../../docs/servidor-linux.md).

- **Paquetes:** `resguardo-server-x86_64-linux-musl.tar.gz` (estático, con la
  consola web dentro) y `resguardo-server_<versión>_amd64.deb`. Los genera la
  integración continua (trabajo «Paquetes del servidor», artefacto
  `servidor-linux-x86_64`, con `SHA256SUMS`), que además los instala en el
  ejecutor de Ubuntu y comprueba el servicio, la consola por HTTPS y
  `codigo-inicial`.
- **Binario:** `/opt/resguardo-server/resguardo-server`, con enlace en
  `/usr/local/bin` (script) o `/usr/bin` (`.deb`).
- **Usuario:** `resguardo-server`, de sistema y sin intérprete de órdenes.
- **Datos:** `/var/lib/resguardo-server/` (0700, del usuario `resguardo-server`):
  base de datos, identidad del servidor, autoridad TLS propia (`tls/`) y el
  código de primer arranque mientras no haya cuenta de propietario.
- **Configuración:** `/etc/resguardo-server/servidor.env`
  (`RESGUARDO_ESCUCHAR`, por defecto `0.0.0.0:8443`; `RESGUARDO_NOMBRES`).
  Cambiar la carpeta de datos exige ampliar `ReadWritePaths` de la unidad.
- **Registro:** `journalctl -u resguardo-server` (salida estándar del servicio).
- **Primer arranque:** `sudo resguardo-server codigo-inicial` muestra la
  dirección, la huella de la autoridad TLS y el código.
- **Cortafuegos:** los instaladores no lo tocan. Ver la guía.

```sh
sudo sh instalar-servidor.sh --paquete ./resguardo-server-x86_64-linux-musl.tar.gz [--sin-firma] [--puerto 8443]
sudo sh /opt/resguardo-server/instalar-servidor.sh --desinstalar            # conserva los datos
sudo sh /opt/resguardo-server/instalar-servidor.sh --desinstalar --purgar   # y los borra
```
