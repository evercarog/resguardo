# La consola de Resguardo en internet

Resguardo Server puede funcionar como una **consola en línea** para varios
clientes a la vez: una máquina virtual pequeña en la nube, con su nombre
(`consola.ejemplo.com`), un certificado público que se renueva solo, y cada
cliente aislado de los demás. Esta guía explica dónde alojarla, qué pasa (y
qué no) por ella, cómo quedan los certificados y la vinculación de los
equipos, y el paso a paso. **Ahora** se usa una máquina del nivel gratuito
de **Google Cloud (e2-micro)** para compilar y probar, pruebas básicas y
vigilar: es una consola **de pruebas**. La definitiva (Oracle Cloud Always
Free o Hetzner) vendrá después, y pasar a ella es sencillo: ver «Mudarse de la
consola de pruebas a la definitiva».

> **Estado.** Lo de esta guía está en el código y se prueba en la integración
> continua: el certificado contra una autoridad ACME de prueba (firmas, nonces,
> reto HTTP-01), el servidor sirviendo el certificado público en el dominio y
> el propio a los agentes, el aislamiento entre clientes ruta a ruta, las
> cuotas, los paquetes de arm64 (arrancados con qemu) y `preparar-vm.sh` en un
> Ubuntu limpio (la integración continua lo ejecuta de verdad, dos veces). **No** se ha probado todavía en una máquina de Oracle ni de
> Google, ni con Let's Encrypt de verdad (en la integración continua nunca se
> le pide nada). Haz la primera instalación con `--acme-pruebas` (abajo).

## Dónde alojarla

Resguardo Server es **un programa que se queda en marcha**: mantiene abierto
un WebSocket con cada equipo, sirve HTTPS con su propio TLS, guarda sus datos
en SQLite en el disco y, si se pide, hace de relé para restaurar en el
navegador. Eso descarta las plataformas «sin servidor»:

- **Vercel** (y similares): funciones que viven segundos, sin conexiones
  largas ni disco propio. No sirve.
- **Supabase**: base de datos y funciones gestionadas; tampoco ejecuta un
  programa propio en marcha. La web antigua de Supabase se va a retirar: la
  consola nueva es Resguardo Server.

Lo que encaja es **una máquina virtual Linux pequeña**:

| Opción | Cuándo |
|---|---|
| **Google Cloud, nivel gratuito, e2-micro** (x86_64, 1 GB de RAM, solo en `us-west1`, `us-central1` o `us-east1`; ~1 GB de salida al mes gratis) | **Ahora: pruebas.** Compilar y probar, pruebas básicas y vigilar con pocos equipos: el tráfico de la consola es pequeño. Una restauración por el relé se come el mes entero: limita el relé (abajo). Paquetes `x86_64`. |
| **Oracle Cloud Always Free** (Ampere A1, **arm64**, hasta 4 OCPU y 24 GB entre todas las del plan gratuito; 10 TB de salida al mes) | **Para producción, más adelante.** La mejor opción gratuita: sobra para muchos clientes y aguanta restauraciones por el relé. Paquetes `aarch64`. |
| **Hetzner Cloud CX22** (2 vCPU x86_64, 4 GB; 20 TB al mes; unos 4 € al mes) | **Para producción, más adelante**, si no quieres depender de un plan gratuito. (CAX11, arm64, cuesta parecido.) |

Las condiciones de los planes gratuitos cambian: compruébalas al crear la
cuenta (en Oracle, la política de instancias inactivas del plan gratuito; en
Google, qué entra en el nivel gratuito y si la IP externa tiene coste).

### Qué pasa por la consola y qué no

- **Las copias de seguridad nunca pasan por la consola.** Cada equipo copia
  directamente a sus destinos (un Servidor de copias, un disco, una nube) con
  restic, cifrado con la contraseña de su repositorio, que el servidor no
  tiene.
- Por la consola pasa **lo que la administra**: el estado de cada equipo
  (informes, historial, avisos), su configuración cifrada, las órdenes
  selladas (el servidor no puede abrirlas) y sus resultados firmados. Son
  pocos MB al mes por equipo; «Clientes del servidor» enseña cuánto ocupa
  cada cliente.
- **El relé de descargas** (opcional): solo cuando alguien elige «Descargar»
  para restaurar en el navegador, el equipo sube esos archivos (cifrados de
  extremo a extremo) al servidor y el navegador los baja. Ese es el único
  tráfico grande, y tiene su cuota por cliente y mes (`relevo_mb_mes`) y un
  tamaño máximo por descarga (`--max-descarga MB`, 500 por defecto). En un
  servidor con poco tráfico gratis (Google), pon una cuota pequeña o restaura
  en el propio equipo.

## Certificados y vinculación de los equipos

Con `--dominio consola.ejemplo.com` el servidor pide solo un **certificado
público** a Let's Encrypt (ACME, reto HTTP-01 por el puerto 80), lo guarda en
`/var/lib/resguardo-server/acme/` y lo renueva cuando le queda un tercio de su
vigencia (unos 30 días antes de caducar), sin reiniciar. Si falla, lo dice en
el registro y lo vuelve a intentar al cabo de 1 min, 4, 16, 64… (como mucho
cada 6 h), sin pasarse de los límites de Let's Encrypt.

Pero los **agentes no usan ese certificado**:

- Al vincularse, cada agente **fija la autoridad TLS propia del servidor**
  (la de siempre, generada en el primer arranque) y su **identidad Ed25519**.
  El número de comprobación (SAS v3) cubre las dos: si alguien se pusiera en
  medio durante la vinculación, el número no coincidiría con el de la consola.
- El servidor sirve el certificado público **solo a quien pide el dominio**
  (`consola.ejemplo.com`, por SNI). En **`agentes.consola.ejemplo.com`**
  (`--dominio-agentes`) y por IP sigue sirviendo el de su autoridad propia.
  La consola da esa dirección a los equipos (`url_agentes` de
  `GET /api/servidor`): en «Añadir equipo», en el instalador listo y en las
  líneas de Linux y macOS.
- Así una renovación del certificado público, o que Let's Encrypt cambie de
  raíz (lo hace cada pocos años), **no afecta a los equipos**: lo que fijaron
  no cambia nunca. Y nadie con un certificado válido para tu dominio (por
  ejemplo, quien lograra uno de otra autoridad) puede hacerse pasar por el
  servidor ante los equipos: no es la autoridad que fijaron, y además en cada
  conexión el servidor firma un reto con su identidad.
- Los navegadores sí usan el certificado público: la consola se abre sin
  avisos. Lo que el navegador no puede comprobar es la identidad Ed25519; por
  eso, la primera vez, compárala (paso 5).
- Las **otras consolas** que se conectan a esta («Conectar también a otra
  consola») hacen lo mismo que un agente: van a `agentes.<dominio>` y fijan la
  autoridad propia y la identidad.

En la red local nada cambia: sin `--dominio`, el servidor sigue con su
certificado propio para todo, como hasta ahora.

Los dos nombres (`consola.…` y `agentes.…`) tienen que apuntar a la IP de la
máquina. Si no tienes dominio, **DuckDNS** (gratis) da `tu-oficina.duckdns.org`,
y `agentes.tu-oficina.duckdns.org` funciona solo (sus subdominios apuntan a la
misma IP).

### Detrás de Caddy (u otro proxy)

Si en la máquina ya hay un Caddy que pone los certificados, Resguardo Server
puede ir detrás. El servidor sigue con su TLS propio en el 8443 (los agentes
lo necesitan) y Caddy pone el público delante para los navegadores:

```caddyfile
consola.ejemplo.com {
    reverse_proxy https://127.0.0.1:8443 {
        transport http {
            # La autoridad propia de Resguardo Server (copia de /var/lib/resguardo-server/tls/ca.crt).
            tls_trust_pool file /etc/caddy/resguardo-ca.crt
            tls_server_name localhost
        }
    }
}
```

(En Caddy anterior a 2.8: `tls_trusted_ca_certs /etc/caddy/resguardo-ca.crt`.)

Y en `/etc/resguardo-server/servidor.env`:

```sh
RESGUARDO_ESCUCHAR=0.0.0.0:8443
RESGUARDO_DETRAS_DE_PROXY=1
RESGUARDO_PUBLICO=1
RESGUARDO_URL_AGENTES=https://consola.ejemplo.com:8443
RESGUARDO_NOMBRES=consola.ejemplo.com
```

- `RESGUARDO_DETRAS_DE_PROXY=1` (`--detras-de-proxy`): en las conexiones que
  llegan de 127.0.0.1 (Caddy), la IP de quien pide es la última de
  `X-Forwarded-For` (para los límites, la auditoría y fail2ban). Las demás
  conexiones no pueden fingir su IP con esa cabecera.
- **Proxy en otra máquina o contenedor** (Caddy en Docker, en otro CT…):
  `RESGUARDO_PROXY_RED=172.18.0.0/16` (`--proxy-red`, varias redes separadas por
  comas; implica `RESGUARDO_DETRAS_DE_PROXY=1`). De esas redes también se cree
  `X-Forwarded-For`, empezando por la derecha: la IP de quien pide es la primera
  que no es de un proxy de confianza. Sin ella, todas las peticiones tendrían la
  IP del proxy y los límites por IP serían de todos a la vez. Pon la red más
  pequeña posible (mejor la IP del proxy con `/32`). Ver
  [servidor-linux.md](servidor-linux.md), «Proxy en otro CT o contenedor».
- Los agentes van directos al 8443 (ábrelo en el cortafuegos) con la
  dirección de `RESGUARDO_URL_AGENTES`.
- Sin Caddy, `--dominio` hace todo esto solo y basta con los puertos 80 y 443.
- La consola se mantiene al día con un WebSocket propio (`/api/clientes/{c}/vivo`,
  en la misma dirección que la consola). Caddy lo pasa sin configurar nada; con
  otro proxy (nginx…), deja pasar `Upgrade` y `Connection` como para cualquier
  WebSocket. Si no pasa, la consola sigue funcionando y pregunta cada pocos segundos.

## Paso a paso (lo común)

Lo que tienes que hacer tú (Resguardo no crea cuentas ni gasta dinero): la
**cuenta del proveedor**, la **máquina** y el **nombre** (dominio propio o
DuckDNS). Lo demás lo hace `preparar-vm.sh`.

1. **Los paquetes.** De la integración continua: «Paquetes del servidor»
   (artefacto `servidor-linux-x86_64`) para Google e2-micro o Hetzner CX22; o
   «Paquetes del agente y del servidor (Linux arm64, musl)»
   (`servidor-linux-aarch64`) para Oracle o Hetzner CAX11. En tu PC, comprueba
   la SHA-256 con su `SHA256SUMS`:

   ```powershell
   Get-FileHash .\resguardo-server-x86_64-linux-musl.tar.gz -Algorithm SHA256
   ```

2. **El nombre.** Crea dos registros `A` (y `AAAA` si la máquina tiene IPv6)
   con la IP pública de la máquina: `consola.ejemplo.com` y
   `agentes.consola.ejemplo.com`. Con DuckDNS, crea `tu-oficina` y pon la IP;
   el de `agentes.` va solo. Comprueba desde tu PC:

   ```powershell
   Resolve-DnsName consola.ejemplo.com
   Resolve-DnsName agentes.consola.ejemplo.com
   ```

3. **Copiar y preparar.** Desde tu PC:

   ```powershell
   scp resguardo-server-x86_64-linux-musl.tar.gz <usuario>@<IP>:~
   ssh <usuario>@<IP>
   ```

   (En Google, `gcloud compute scp` y `gcloud compute ssh <instancia>`, o el
   botón SSH de la consola web; en Oracle el usuario es `ubuntu`.)

   Y en la máquina:

   ```sh
   mkdir resguardo && tar -xzf resguardo-server-x86_64-linux-musl.tar.gz -C resguardo
   sudo sh resguardo/preparar-vm.sh --dominio consola.ejemplo.com --acme-correo tu@correo \
        --paquete resguardo-server-x86_64-linux-musl.tar.gz --sin-firma --acme-pruebas
   ```

   `--sin-firma` porque aún no hay llave de publicación: ya comprobaste la
   SHA-256 en el paso 1. `--acme-pruebas` pide el certificado al Let's Encrypt
   de pruebas (el navegador avisará: es normal). Mira el registro hasta ver
   «Certificado público de consola.ejemplo.com listo»:

   ```sh
   journalctl -u resguardo-server -f
   ```

   Si funciona, quita las pruebas (se pide el de verdad al momento):

   ```sh
   sudo sed -i '/^RESGUARDO_ACME_PRUEBAS=/d' /etc/resguardo-server/servidor.env
   # (desde 0.7.16 no hace falta: cada autoridad guarda lo suyo en su carpeta)
   sudo systemctl restart resguardo-server
   ```

   El script se puede repetir (por ejemplo, con un paquete nuevo para
   actualizar) y deja todo igual. Hace: actualizaciones de seguridad
   automáticas; cortafuegos con solo 22, 80 y 443 (`--ssh-desde TU_IP/32` para
   limitar SSH a tu IP); Resguardo Server en el 443 como usuario sin
   privilegios (solo con el permiso de usar los puertos 80 y 443); fail2ban
   con su filtro; y la carpeta de las copias de la consola. En máquinas de
   1 GB añade 1 GB de swap.

4. **Primer arranque, por SSH.**

   ```sh
   sudo resguardo-server codigo-inicial
   ```

   Da el código de primer arranque y la **identidad del servidor** (grupos de
   4 cifras). Abre `https://consola.ejemplo.com`, crea la cuenta de
   propietario con el código y da de alta la verificación en dos pasos.

5. **Comprobar la identidad.** En la consola, «Ajustes» → «Identidad»: tiene
   que ser la misma que dio `codigo-inicial` por SSH. Así sabes que la consola
   que abriste es la de tu máquina (y no otra a la que apuntara el nombre).

6. **Copia de la consola.** «Ajustes» → «Copia de la consola»: pon la clave
   de respaldo y guarda el kit. Cada noche queda una copia cifrada en
   `/var/lib/resguardo-server/respaldos/`, y cada hora se copia a
   `/var/backups/resguardo-consola/` (legible por tu usuario, sin sudo). Llévala
   fuera de la máquina:

   ```powershell
   scp "<usuario>@<IP>:/var/backups/resguardo-consola/*.resguardo-consola" F:\copias-consola\
   ```

   o con `rclone` a un almacenamiento de objetos (Oracle y Google tienen un
   poco gratis). Van cifradas con tu clave de respaldo: quien las tenga no
   puede abrirlas. Para restaurar en otra máquina: «Copia de la consola» en
   [servidor-linux.md](servidor-linux.md).

7. **Clientes y personas.** En «Clientes» → «Clientes del servidor»:
   - **Cliente para otra persona**: crea el cliente **sin que tú entres** y te
     da un enlace de invitación de propietario (7 días, un solo uso). Mándalo
     por un canal de confianza: quien lo abra primero se queda con el
     cliente, invita a su gente y vincula sus equipos. Tú solo ves cifras.
   - **Cuotas**: las predeterminadas y las de cada cliente (equipos, historial
     por equipo, MB del relé al mes, órdenes por minuto; vacío = la de arriba,
     0 = sin límite).
   - Para tu propia empresa, «Nuevo cliente» como siempre (ahí sí eres
     miembro).

8. **Vincular los equipos.** Desde el cliente, «Añadir equipo»: la consola da
   la dirección de los agentes (`https://agentes.consola.ejemplo.com`), el
   instalador listo de Windows o la línea de Linux. Los equipos necesitan
   salir a internet por el 443; nada más.
   El paquete de Linux no trae el instalador del agente de Windows: para
   «Descargar instalador listo», sube `Resguardo-Agente-setup.exe` de la
   misma publicación a la máquina y, por SSH,
   `sudo resguardo-server poner-instalador-agente Resguardo-Agente-setup.exe --sha256 <el de SHA256SUMS>`
   (sin reiniciar; ver [servidor-linux.md](servidor-linux.md#el-instalador-listo-de-windows)).
   Sin él, la consola ofrece el instalador normal y un código.

9. **Conectar las consolas locales.** Si ya tienes un Resguardo Server en la
   oficina, en él: «Conectar también a otra consola» con la dirección de los
   agentes de la consola en línea (`https://agentes.consola.ejemplo.com`).
   Comprueba el número de comprobación y la identidad en las dos.

## Google Cloud, e2-micro (la consola de pruebas)

Para compilar y probar, pruebas básicas y vigilar. No es la consola
definitiva: cuando toque, se muda (abajo).

1. **Cuenta y proyecto.** Crea la cuenta (pide tarjeta) y un proyecto. El
   nivel gratuito cubre **una** e2-micro al mes en `us-west1` (Oregón),
   `us-central1` (Iowa) o `us-east1` (Carolina del Sur), con hasta 30 GB de
   disco estándar. Pon una **alerta de presupuesto** (*Billing → Budgets &
   alerts*, p. ej. 1 USD) para enterarte si algo deja de ser gratis.
2. **Crear la instancia**: *Compute Engine → VM instances → Create instance*.
   - Región: una de las tres de arriba (cualquier zona). Serie **E2**, tipo
     **e2-micro** (2 vCPU compartidas, 1 GB).
   - Disco de arranque: **Ubuntu 24.04 LTS** (x86/64), tipo **Standard
     persistent disk** (no «balanced» ni SSD: no entran en el nivel
     gratuito), **30 GB**.
   - Cortafuegos: marca **Allow HTTP traffic** y **Allow HTTPS traffic**
     (crean las reglas de entrada para 80 y 443 con las etiquetas
     `http-server` y `https-server`). El 22 ya lo abre la regla
     `default-allow-ssh` de la red por defecto.
   - *Advanced options → Management → Metadata*: clave `user-data` con el
     contenido de `packaging/linux/nube/cloud-init.yaml` (opcional; el script
     hace lo mismo).
3. **IP fija**: *VPC network → IP addresses → Reserve external static IP*, en
   la misma región, y asígnala a la instancia. Si no, la IP puede cambiar al
   parar la máquina y habría que cambiar el nombre. Mira en la calculadora de
   precios si la IP tiene coste en tu caso.
4. **El nombre** (paso 2 de arriba). Para pruebas basta DuckDNS
   (`tu-prueba.duckdns.org`; `agentes.tu-prueba.duckdns.org` va solo).
5. **Instalar**: pasos 3 a 9 de arriba con el paquete **x86_64**. Empieza con
   `--acme-pruebas`.
6. **Memoria (1 GB).** Resguardo Server usa poca (unas decenas de MB con
   pocos equipos), pero `apt` y fail2ban a la vez pueden apretar: con 1 GB o
   menos, `preparar-vm.sh` añade **1 GB de swap** (`/swapfile`). Compruébalo y
   vigila:

   ```sh
   free -m
   swapon --show
   systemctl status resguardo-server | grep -i memory
   ```

   Si alguna vez falta memoria, el registro del sistema lo dice
   (`journalctl -k | grep -i oom`). No instales nada más pesado en esta
   máquina (ni el agente con su servidor de copias, ni un proxy).
7. **Tráfico**: ~1 GB de salida al mes gratis. Para la consola sobra; para el
   relé, no. Pon en «Clientes del servidor» → «Cuotas predeterminadas» un relé
   pequeño (p. ej. 200 MB al mes) o restaura en los propios equipos. También
   puedes bajar el tamaño máximo de una descarga: `RESGUARDO_MAX_DESCARGA=100`
   (MB) en `/etc/resguardo-server/servidor.env`.
8. **Instantáneas del disco** (antes de probar algo arriesgado, o cada
   semana): *Compute Engine → Disks → (el disco) → Create snapshot*, o

   ```sh
   gcloud compute disks snapshot <disco> --zone <zona> --snapshot-names resguardo-$(date +%Y%m%d)
   ```

   Para volver atrás: crea un disco desde la instantánea y arranca la máquina
   con él. Las instantáneas ocupan espacio facturable (el nivel gratuito
   incluye unos pocos GB): borra las viejas. No sustituyen a la **copia de la
   consola** (paso 6), que es la que sirve para mudarse.
9. **Vigilar**: el panel *Observability* de la instancia (CPU, red, disco) y
   el registro:

   ```sh
   journalctl -u resguardo-server -f
   sudo fail2ban-client status resguardo-server
   ```

   «Clientes del servidor» enseña la última actividad y el uso de cada
   cliente.

## Para producción, más adelante

### Oracle Cloud Always Free

1. Crea la cuenta (pide tarjeta para verificar; el plan Always Free no cobra).
   Elige bien la **región de origen**: las máquinas gratuitas solo se pueden
   crear ahí.
2. **Crear la instancia**: *Compute → Instances → Create instance*.
   - Imagen: **Canonical Ubuntu 24.04** (o 22.04), la de **aarch64**.
   - Forma: *Ampere* → **VM.Standard.A1.Flex**. Con 1 OCPU y 6 GB sobra para
     empezar (el plan gratuito da hasta 4 OCPU y 24 GB en total).
   - Red: la VCN que propone, con **IP pública**.
   - Claves SSH: sube tu clave pública.
   - *Show advanced options → Management → cloud-init*: pega
     `packaging/linux/nube/cloud-init.yaml` (opcional; el script hace lo mismo).
   - Si dice *Out of capacity*, prueba otro dominio de disponibilidad o más
     tarde.
3. **IP fija**: en la VNIC de la instancia, cambia la IP pública a
   **reservada** (gratis en el plan; si no, cambia al reiniciar la máquina).
4. **Abrir 80 y 443** en la red: *Networking → Virtual cloud networks → (tu
   VCN) → Security Lists → Default → Add Ingress Rules*: origen `0.0.0.0/0`,
   TCP, puertos `80` y `443` (dos reglas). El 22 ya está. Dentro de la
   máquina, la imagen de Oracle trae reglas de iptables que lo cierran todo:
   `preparar-vm.sh` abre 80 y 443 antes de su `REJECT` y las guarda.
5. El nombre (paso 2 de arriba) con la IP reservada.
6. Pasos 1 a 9 con el paquete **aarch64**. El usuario es `ubuntu`.

Oracle puede reclamar las instancias Always Free que pasan mucho tiempo casi
sin hacer nada (mira su política actual). Una consola con equipos conectados
trabaja poco; si te avisan, pasar la cuenta a «pago por uso» lo evita y lo
gratuito sigue siendo gratis.

### Hetzner Cloud (de pago)

Crea un servidor **CX22** (o **CAX11**, arm64) con Ubuntu 24.04 y tu clave
SSH, y añade un cortafuegos de Hetzner con 22, 80 y 443. Después, pasos 2 a 9
(paquete x86_64 en CX22, aarch64 en CAX11). La IP ya es fija.

## Mudarse de la consola de pruebas a la definitiva

Todo lo que importa de la consola está en su **carpeta de datos** (cuentas,
clientes, equipos, historial, su **identidad** y su **autoridad TLS**), y la
**copia de la consola** (paso 6) la guarda entera, cifrada. Por eso mudarse es
sencillo. Dos caminos:

**A. La misma consola en otra máquina (recomendado).** Los equipos no notan
nada: la consola nueva tiene la misma identidad y la misma autoridad TLS, que
es lo que fijaron.

1. En la consola de pruebas: «Ajustes» → «Copia de la consola» → «Hacer una
   copia ahora», o `sudo resguardo-server hacer-respaldo`. Tráete el archivo
   `.resguardo-consola` (paso 6).
2. Prepara la máquina nueva con `preparar-vm.sh` (pasos 1 y 3), con los
   **mismos nombres** (`--dominio` y, si lo pusiste, `--dominio-agentes`).
3. Para el servicio nuevo, copia el archivo y restaura con la huella de tu
   kit:

   ```sh
   sudo systemctl stop resguardo-server
   sudo resguardo-server restaurar-respaldo consola-AAAAMMDD-….resguardo-consola \
        --reemplazar --confiar-en <huella del kit>
   sudo systemctl start resguardo-server
   ```

4. Cambia los dos registros del nombre (`consola.…` y `agentes.…`) a la IP
   nueva (en DuckDNS, la IP; con un dominio propio, los `A`). En cuanto el
   nombre apunte a la máquina nueva, pide su certificado público y los
   equipos vuelven solos (reintentan y comprueban la identidad: es la misma).
5. Comprueba en «Clientes del servidor» que los equipos se conectan y, unos
   días después, apaga la máquina de pruebas (y borra su IP reservada y sus
   instantáneas para que no cuesten).

Si quieres que la definitiva tenga **otro nombre** (p. ej. de
`tu-prueba.duckdns.org` a `consola.tuempresa.com`), restaura igual pero arranca
con los dos nombres en el certificado propio (`RESGUARDO_NOMBRES` con el
antiguo) y deja el nombre antiguo apuntando a la nueva un tiempo; después,
cambia los equipos de dirección desde la consola (orden «Cambiar de servidor»)
o con el camino B.

**B. Otra consola, y los equipos pasan a ella.** Para cuando la definitiva ya
existe (con otra identidad): en la definitiva, «Conectar también a otra
consola» con la de pruebas; comprueba el número de comprobación y la
identidad en las dos, deja que los equipos se den de alta en la nueva y,
cuando todos estén, quita la de pruebas (en cada cliente, «Mover cliente» /
«Cambiar de servidor»; ver [api-servidor.md](api-servidor.md), §11). Es más largo
que A: úsalo solo si no puedes conservar la identidad.

## Seguridad de una consola pública

- **Solo por invitación.** No hay alta abierta: la primera cuenta sale del
  código de primer arranque (solo se ve por SSH) y las demás, de invitaciones
  de un solo uso que da el propietario de cada cliente (o el del servidor,
  con «Cliente para otra persona»). Todas con verificación en dos pasos.
- **Cada cliente aislado.** Cada uno en su base de datos; todo lo de
  `/api/clientes/{c}` exige ser miembro antes incluso de leer la petición. El
  propietario del servidor **no** entra en los clientes de los que no es
  miembro: ve cifras en «Clientes del servidor». Lo comprueba
  `crates/servidor/tests/aislamiento.rs`, que recorre todas las rutas.
  (Quien administra la máquina siempre puede más que la consola: por eso las
  claves, las contraseñas de los repositorios y las órdenes van cifradas de
  extremo a extremo y el servidor no las puede abrir.)
- **Límites.** Además de los de siempre (intentos de entrar por correo y por
  IP, códigos, vinculaciones), cada IP tiene como mucho 1800 peticiones a la
  API por minuto y 1000 conexiones abiertas, y cada cuenta 1200 peticiones por
  minuto.
- **fail2ban.** Cada acceso fallido (contraseña, código, invitación, secreto
  de un equipo…) deja en el registro `Acceso fallido desde <IP>: …`;
  `preparar-vm.sh` instala el filtro y la jaula
  (`packaging/linux/fail2ban`): 10 fallos en 10 minutos bloquean esa IP una
  hora. Ojo con las oficinas tras una sola IP (`ignoreip` en la jaula).

  ```sh
  sudo fail2ban-client status resguardo-server
  sudo fail2ban-client set resguardo-server unbanip 203.0.113.5
  ```

## Mantenimiento

- **Actualizar**: copia el paquete nuevo y repite `preparar-vm.sh` con
  `--paquete` (o `sudo sh /opt/resguardo-server/instalar-servidor.sh --paquete …`).
  Antes de abrir la base de datos con una versión nueva, el servidor hace una
  copia de la consola.
- **Registro**: `journalctl -u resguardo-server -n 100 --no-pager`.
- **Certificado**: `sudo ls -l /var/lib/resguardo-server/acme/`. Se renueva
  solo; si el registro dice «no se pudo obtener», comprueba que los dos
  nombres apuntan a la máquina y que el 80 está abierto en el proveedor y en
  la máquina.
- **Opciones** (en `/etc/resguardo-server/servidor.env`, ver
  `resguardo-server --ayuda`): `RESGUARDO_DOMINIO`, `RESGUARDO_ACME_CORREO`,
  `RESGUARDO_DOMINIO_AGENTES`, `RESGUARDO_ACME_PRUEBAS=1`,
  `RESGUARDO_ACME_HTTP` (por defecto `0.0.0.0:80`), `RESGUARDO_URL_AGENTES`,
  `RESGUARDO_DETRAS_DE_PROXY=1`, `RESGUARDO_PROXY_RED`, `RESGUARDO_PUBLICO=1`.

## Lo que falta

- La **llave de publicación** (minisign): hasta que exista, los paquetes se
  copian a mano y se comprueban por su SHA-256.
- El **instalador de Windows dentro del paquete de Linux**: la CI de Linux no
  lo compila (se hace en Windows, con NSIS), así que se pone a mano con
  `poner-instalador-agente` (paso 8).
- El **agente de arm64** lleva restic, pero no rest-server ni rclone con
  huella fijada: en arm64, «Este equipo guarda copias» y el espejo en la nube
  aún no se usan (no hace falta para la consola).
- Probarlo en Oracle y en Google de verdad, y con Let's Encrypt de verdad.
