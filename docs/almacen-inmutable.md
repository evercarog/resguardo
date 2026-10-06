# Un almacén que un ransomware no pueda vaciar

Guía de la tarea 8e de [plan-mejoras.md](plan-mejoras.md), con [regla-3-2-1.md](regla-3-2-1.md). Para quien instala el almacén de una oficina.

El almacén (el equipo con «Este equipo guarda copias») ya protege mucho: cada equipo copia en un rest-server de **solo añadir** con su propio usuario, así que un equipo comprometido no puede borrar sus copias. Pero el almacén mismo es un equipo: si alguien se hace con él (o con su administrador), puede borrar sus discos. Esta guía es para que **ni siquiera así** se pierdan las copias.

La consola **no comprueba** nada de esto (el almacén no puede ver lo que hace su anfitrión): cuando lo tengas, márcalo tú en el destino («Repositorios y destinos» → el destino → «Regla 3-2-1» → «Con instantáneas inmutables fuera de su alcance» o «Desconectado»). La regla 3-2-1-1-0 lo cuenta como inmutable y te recuerda que es **local**: no protege de un incendio o un robo de la oficina. Mantén también una copia fuera (la copia externa a la nube, con bloqueo de objetos si puedes).

El sistema de archivos de cada carpeta y si el almacén corre en un contenedor o una máquina virtual se enseñan en la consola **solo como dato**: nunca restan en la regla ni en la salud de la protección.

## Lo recomendado: Proxmox con ZFS e instantáneas del anfitrión

1. **Un anfitrión Proxmox VE** con los discos en **ZFS en espejo** (dos discos iguales; `mirror` al crear el pool). Un disco que falla no se lleva nada.
2. **El almacén, en un contenedor (CT) o una máquina virtual Debian** dentro de ese anfitrión, con el agente de Linux ([agente-linux.md](agente-linux.md)) y «Este equipo guarda copias». La carpeta del almacén vive en un *dataset* de ZFS del anfitrión (en un CT, un punto de montaje `mp0`; en una VM, su disco).
3. **Instantáneas en el anfitrión**, no en el almacén: el almacén no las ve ni las puede borrar. Con `sanoid` (o `zfs-auto-snapshot`), por ejemplo, en `/etc/sanoid/sanoid.conf`:

   ```ini
   [rpool/data/subvol-105-disk-1]
           use_template = copias

   [template_copias]
           hourly = 48
           daily = 30
           monthly = 6
           autosnap = yes
           autoprune = yes
   ```

   (El nombre del *dataset* es el de tu almacén: `zfs list` en el anfitrión.) Cada hora, una instantánea; se guardan 2 días de horas, 30 días y 6 meses.
4. **El anfitrión, fuera de la red de la oficina y con otras credenciales**: su interfaz web solo desde una red de administración (o una VPN), con una contraseña que no se use en ningún otro sitio y segundo factor. Si alguien entra en un equipo de la oficina, no debe poder llegar al anfitrión.
5. **`scrub` periódico** del pool (Proxmox ya programa uno al mes; compruébalo con `zpool status`) y avisos por correo de ZFS (`zed`).
6. **Recuperar** algo borrado en el almacén: en el anfitrión, `zfs rollback` de la instantánea (todo el *dataset*) o copia la carpeta desde `.zfs/snapshot/<nombre>/`. Después, en la consola, el repositorio sigue como estaba (las copias son archivos cifrados de restic).

Con esto, marca la zona del almacén como «Con instantáneas inmutables fuera de su alcance».

## Otras formas

- **Linux endurecido, sin anfitrión**: un Debian mínimo solo para el almacén, sin acceso remoto (ni SSH ni escritorio remoto; se administra en local), actualizaciones automáticas de seguridad, cortafuegos que solo deja entrar el puerto del almacén desde la red de la oficina. Para lo antiguo, `chattr +i` con un plazo (una tarea de `root` que marca inmutables las carpetas de `data/` de más de N días y se las quita antes de podar): complica la retención del almacén, así que solo si sabes lo que haces. No cuenta como «instantáneas»: márcalo solo si de verdad nadie del almacén puede deshacerlo.
- **Discos USB que se rotan**: dos o tres discos para la copia externa (o el espejo), uno conectado y los otros guardados fuera de la oficina, desconectados. Un disco desconectado no lo cifra nadie. Márcalos como «Desconectado» y, si son el mismo juego de discos, con el mismo **soporte** («Discos USB rotados») para que cuenten como uno.

## Un almacén con Windows

Se puede, y funciona igual, pero **las instantáneas de Windows (VSS, «versiones anteriores») las borra cualquier administrador** del equipo: no cuentan como inmutables. Para que sea lo más difícil posible:

- **Fuera del dominio** de la oficina: un administrador del dominio comprometido no debe ser administrador del almacén.
- **Una cuenta de administrador propia**, con una contraseña que no se use en ningún otro sitio; nadie trabaja con ella a diario.
- **Sin escritorio remoto expuesto** (ni a internet ni a toda la oficina); si hace falta, solo desde un equipo concreto.
- **Una copia fuera de su alcance**: la copia externa a la nube con bloqueo de objetos (Backblaze B2 o S3 con Object Lock), o discos USB rotados. Eso es lo que hace que cumpla el «1 inmutable» de verdad.

## Comprobar

En la consola, en la copia: la tira «3 · 2 · 1 · 1 · 0» dice qué destinos cuentan como inmutables y si son locales. En «Repositorios y destinos», cada zona enseña su sistema de archivos (`zfs`, `NTFS`, `ReFS`…) y si el almacén está en un contenedor o una máquina virtual. Prueba de vez en cuando a recuperar un archivo desde una instantánea del anfitrión: una instantánea que nadie ha probado es como una copia que nadie ha restaurado.
