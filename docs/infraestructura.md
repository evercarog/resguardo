# Infraestructura: dónde están de verdad los datos (fase 1)

Diseño del plan 0.8.1, bloque C4 ([plan-0.8.1.md](plan-0.8.1.md); [plan-mejoras.md](plan-mejoras.md) 15d). Escrito el 2026-10-20 con el responsable ausente (rama `ia/081-c`); lo dudoso, en [registro-ia.md](registro-ia.md). Relacionados: [regla-3-2-1.md](regla-3-2-1.md), [sedes.md](sedes.md), [api-servidor.md](api-servidor.md) («Infraestructura» y «Cambios»).

## Para qué

Saber **dónde están de verdad** los datos y sus copias: en qué servidor físico, en qué discos. Una máquina virtual y el almacén que la copia pueden estar en el mismo anfitrión: dos «copias» que un fallo del servidor se lleva juntas. La regla 3-2-1-1-0 lo tiene en cuenta cuando se anota.

**Para cualquier entorno** (equipo físico con Windows o Linux, Hyper-V, VMware, Proxmox, un NAS, la nube): nada depende de un proveedor. **Sin conectar ningún hipervisor** (15e, aparcada): lo que el agente no ve, lo anota la persona. Resguardo **no gestiona** RAID, discos ni hipervisores: solo sabe dónde está cada cosa y avisa. **Todo es opcional:** sin nada anotado, todo funciona como antes.

## El modelo

Por cliente, en el servidor de la consola (`infra:<cliente>`, `GET/PUT /api/clientes/{c}/infraestructura`):

- **Servidores físicos:** nombre, marca y modelo (opcionales), sede, **hipervisor** (Proxmox VE, Hyper-V, VMware ESXi, Xen, KVM, Nutanix, VirtualBox, otro o ninguno) y notas.
- **Conjuntos de discos** de cada servidor: nombre, tipo (RAID 0/1/5/6/10, ZFS en espejo o raidz1/2/3, Espacios de almacenamiento, NAS, disco suelto, otro), cuántos discos, capacidad y **estado anotado** (bien, degradado, reconstruyendo, sin revisar). Resguardo no comprueba el estado: es lo que dice la persona.
- **Equipos** de Resguardo → su servidor (y, si se sabe, el conjunto donde viven sus discos): una máquina virtual o un contenedor en su anfitrión, o el propio servidor si el agente está en él.
- **Destinos** (por su clave del catálogo: `zona:<almacén>:<zona>`, el id de un destino, `nube:…`) → su servidor y su conjunto. Un destino sin anotar toma el servidor del equipo que lo guarda (una zona, el de su almacén), sin conjunto. La nube no se anota: siempre está fuera.

Sin secretos: el servidor rechaza cualquier campo de más, los textos son cortos y las referencias se comprueban. En la fase 1 es **de cada consola** (no viaja por los equipos como los datos comunes de [consolas-multiples.md](consolas-multiples.md) §6.5): llevarlo allí es una clase nueva de dato común con su validación en `crates/protocolo/src/datos_cliente.rs`, que se deja para cuando esté hecho el bloque B5 (nombres y observaciones comunes), para no pisarse.

## Lo que el agente ve solo

- **Su tipo** (C3, 15c): físico, máquina virtual y de qué tipo, o contenedor (`entorno`: el fabricante y el modelo del firmware, `/run/systemd/container`, el entorno del primer proceso). Se corrige a mano con `tipo_equipo` (lo guarda el equipo, como el nombre).
- **Sus discos y conjuntos** (`datos_basicos.almacenamiento`): en Linux, los discos de `/sys/block` (tamaño, SSD o no, modelo), los RAID de `/proc/mdstat` (nivel, discos y si está degradado o reconstruyendo) y los grupos de ZFS por el origen de sus montajes (con su estado de `/proc/spl/kstat/zfs`, si el equipo es el anfitrión); en Windows, cada `PhysicalDriveN` con su bus y su modelo: un disco de **Espacios de almacenamiento** o de una **controladora RAID** (vista como un disco) es un conjunto. Nunca números de serie.
- **En qué conjunto está cada disco** (`discos[].volumen`): así la consola sabe que la carpeta de una zona está en ZFS `tank`, aunque el agente esté **dentro** de un contenedor y no vea los discos del anfitrión.

Solo lecturas (sin ejecutar programas), al arrancar y cada 6 horas, como los demás datos básicos.

## La interfaz propone y la persona confirma

Página «Infraestructura» del cliente (en el menú, de momento junto a «Repositorios y destinos»; con el menú nuevo del bloque B, dentro de «Destinos»). Arriba, **lo que ven tus equipos**, en una frase y con pocos campos:

- «SERVIDOR-01 es un equipo físico (Marca Modelo) con 2 discos y el conjunto «md0». ¿Lo anotas como servidor?» → abre el servidor ya relleno (nombre, marca, modelo, conjuntos con su forma y discos) para revisarlo. Solo se propone un equipo físico que **parece un servidor** (ve conjuntos, varios discos o guarda copias); un puesto se puede anotar a mano.
- «VM-01 es una máquina virtual. ¿En qué servidor físico está?» → un desplegable.
- «El almacén ALMACEN-01 guarda en ZFS «tank» (1,1 TB, 65 % usado). ¿En qué servidor está?» → el servidor y, si tiene conjuntos, cuál.
- «Ahora no» la quita (se recuerda por persona en el navegador).

Debajo, cada servidor en una tarjeta (sus discos con su estado y **lo que vive aquí**), y dos listas para asignar cada equipo y cada destino.

**Una sola línea por destino**, en su página: «Dónde está físicamente: hv-01 · tank, espejo de 2 discos · Oficina central» (o «Sin anotar · Anotarlo»); el detalle, en la página «Infraestructura». La ficha de cada equipo dice su tipo y dónde está físicamente; las capas del mapa (C2, 15l) **Infraestructura** y **Almacenamiento** lo enseñan en cada tarjeta.

## En la regla 3-2-1-1-0

Con infraestructura anotada (`ponerInfra` en `consola/src/lib/regla321.ts`):

- **Soportes reales:** un paso cuyo destino tiene conjunto anotado cuenta con el soporte `conjunto:<servidor>:<conjunto>`; los originales, con el del conjunto de su equipo si se sabe. Dos destinos en el mismo conjunto cuentan como uno; uno en otro conjunto del mismo servidor, como otro soporte. Lo que una persona marcó como soporte en el catálogo manda.
- **«Servidor físico compartido»** (`servidor_compartido`, vectores compartidos con `protection.rs`): dos copias de los mismos datos en **equipos distintos** (o un destino suelto, como un NAS) del **mismo servidor físico**. En el mismo equipo ya lo dice `mismo_equipo`. Es un aviso, como los demás: nunca bloquea.

## Lo que queda (fase 2 y más)

- Datos comunes a todas las consolas del cliente (ver arriba).
- Conectar un hipervisor (15e, aparcada): el agente en el anfitrión en modo «solo informa» o su API, para enlazar solo cada equipo con su servidor y leer SMART y scrub.
- Avisos de un conjunto degradado que ve el agente (hoy solo se enseña en la ficha del equipo).
- La capa «Infraestructura» del mapa como columnas propias (servidor → hipervisor → VM); hoy es una línea en cada tarjeta.
