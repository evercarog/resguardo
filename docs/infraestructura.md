# Infraestructura: dónde están de verdad los datos

Diseño del plan 0.8.1, bloque C4 ([plan-0.8.1.md](plan-0.8.1.md); [plan-mejoras.md](plan-mejoras.md) 15d), escrito el 2026-10-20 (rama `ia/081-c`), y ampliado en el plan 0.8.2, bloque A ([plan-0.8.2.md](plan-0.8.2.md); 15vv, 15ww, 15xx), el 2026-10-21 (rama `ia/082-infra`). Lo dudoso, en [registro-ia.md](registro-ia.md). Relacionados: [regla-3-2-1.md](regla-3-2-1.md), [sedes.md](sedes.md), [consolas-multiples.md](consolas-multiples.md) §6.5, [api-servidor.md](api-servidor.md) («Infraestructura» y «Cambios»).

## Para qué

Saber **dónde están de verdad** los datos y sus copias: en qué servidor físico, en qué discos. Una máquina virtual y el almacén que la copia pueden estar en el mismo anfitrión: dos «copias» que un fallo del servidor se lleva juntas. La regla 3-2-1-1-0 lo tiene en cuenta cuando se anota. Y, con los discos uno a uno, saber **qué disco cambiar** cuando uno falla.

**Para cualquier entorno** (equipo físico con Windows o Linux, Hyper-V, VMware, Proxmox, un NAS, la nube): nada depende de un proveedor. **Sin conectar ningún hipervisor** (15e, aparcada): lo que el agente no ve, lo anota la persona. Resguardo **no gestiona** RAID, discos ni hipervisores: solo sabe dónde está cada cosa y avisa. **Todo es opcional:** sin nada anotado, todo funciona como antes.

## El modelo

Por cliente (`GET/PUT /api/clientes/{c}/infraestructura`):

- **Servidores físicos:** nombre, marca y modelo (opcionales), sede, **hipervisor** (Proxmox VE, Hyper-V, VMware ESXi, Xen, KVM, Nutanix, VirtualBox, otro o ninguno) y notas.
- **Discos físicos** de cada servidor (0.8.2, A2, 15vv), uno a uno: nombre propio («Bahía 2»), tipo (SSD, HDD, NVMe u otro), modelo, **número de serie** (lo escribe la persona: el agente sigue sin leerlo, decidido en 14uu), tamaño, bahía o ranura, **estado anotado** (bien, con avisos, averiado, sin revisar) y notas.
- **Conjuntos de discos** de cada servidor: nombre, tipo (RAID 0/1/5/6/10, ZFS en espejo o raidz1/2/3, Espacios de almacenamiento, NAS, disco suelto, otro), capacidad y estado anotado (bien, degradado, reconstruyendo, sin revisar), y **los discos que lo forman** («Disco virtual 2 = RAID 1 de Bahía 2 + Bahía 3»). Un conjunto de antes, sin discos elegidos, sigue con su «cuántos discos». Resguardo no comprueba el estado: es lo que dice la persona.
- **Equipos** de Resguardo → su servidor y, si se sabe, su conjunto **o** su disco: una máquina virtual o un contenedor en su anfitrión, o el propio servidor si el agente está en él. Desde la 0.8.2 (A3, 15ww), **cada volumen** del equipo (C:, D:, /srv) puede ir en otro disco o conjunto del mismo servidor; los que no se dicen, como el equipo.
- **Destinos** (por su clave del catálogo: `zona:<almacén>:<zona>`, el id de un destino, `nube:…`) → su servidor y su conjunto o disco. Un destino sin anotar toma el servidor del equipo que lo guarda (una zona, el de su almacén), sin conjunto. La nube no se anota: siempre está fuera.

Sin secretos: se rechaza cualquier campo de más, los textos son cortos y las referencias se comprueban.

## Igual en todas las consolas del cliente (0.8.2, A5)

En la 0.8.1 era de cada consola: lo anotado en la local no se veía en la en línea. Desde la 0.8.2 es un **dato común del cliente** ([consolas-multiples.md](consolas-multiples.md) §6.5), como las sedes:

- Una clase nueva, `infra:`, con un dato por cosa: `infra:servidor:<id>` (el servidor con sus discos y conjuntos), `infra:equipo:<id>` (dónde está el equipo y sus volúmenes) e `infra:destino:<clave>`. Validada en `crates/protocolo/src/datos_cliente.rs` (lo mismo en la consola, `lib/infra.ts` `infraValida`, con los vectores de `vectors/datos-cliente.json`, «infra»), en el agente (`admite: "infra_comun"`; a uno anterior no se le manda) y en el servidor.
- **Describir un servidor no pide la clave** (no cambia nada de la regla por sí solo). **Dónde está un equipo o un destino sí** (`datos_cliente_admin`): cuenta en la regla 3-2-1 (soportes reales, servidor compartido), igual que la sede. En la consola de origen se aplica al momento; a las demás llega cuando alguien la reparte con la clave desde el aviso «Lo mismo en todas las consolas».
- El servidor sigue guardando su copia (`infra:<cliente>`) y la usa como «lo de aquí»: la primera vez que llega el mismo servidor anotado distinto en otra consola, **no se pisa**: sale como diferencia para elegir. Lo anotado antes de la 0.8.2 se reparte una vez como **semilla**. Un sitio que llega antes que su servidor se guarda igual y se enseña cuando llega.
- **Tamaño:** el documento de cada equipo tiene un tope de 256 KiB para todos los datos comunes. Cada servidor, como mucho 16 KiB; toda la infraestructura, como mucho **96 KiB** (lo comprueba el servidor al guardar y lo avisa el editor desde el 80 %). Medido: un cliente grande (6 servidores con 12 discos y 4 conjuntos cada uno, 80 equipos y 40 destinos) ocupa unos **58 KiB**; uno normal, 3 a 5 KiB. La consola deja de mandarla a un equipo con el documento por encima de 224 KiB (los comentarios dejan de repartirse antes, a 192 KiB). **Decisión:** se queda como dato común con estos topes (alternativa descartada: repartir solo los servidores y dejar las ubicaciones en cada consola, que deja la regla distinta en cada consola).

## Lo que el agente ve solo

- **Su tipo** (C3, 15c): físico, máquina virtual y de qué tipo, o contenedor. Se corrige a mano con `tipo_equipo`.
- **Sus discos y conjuntos** (`datos_basicos.almacenamiento`): en Linux, los discos de `/sys/block` (tamaño, SSD o no, modelo), los RAID de `/proc/mdstat` y los grupos de ZFS por el origen de sus montajes; en Windows, cada `PhysicalDriveN` con su bus y su modelo: un disco de **Espacios de almacenamiento** o de una **controladora RAID** (vista como un disco) es un conjunto. Nunca números de serie.
- **En qué conjunto o disco está cada volumen** (`discos[].volumen`) y, desde la 0.8.2, **su etiqueta** en Windows («Datos»).

Solo lecturas (sin ejecutar programas), al arrancar y cada 6 horas, como los demás datos básicos.

## La interfaz propone y la persona confirma

**Ajustes → Infraestructura** (0.8.2, A1: se configura una vez, como las sedes; la dirección de antes, «Destinos → Infraestructura», redirige). Arriba, **lo que ven tus equipos**, en una frase:

- «SERVIDOR-01 es un equipo físico (Marca Modelo) con 2 discos y el conjunto «md0». ¿Lo anotas como servidor?» → el servidor ya relleno, **con sus discos** (y cómo los llama su equipo, `visto`) para revisarlo y escribir los números de serie.
- «SERVIDOR-01 ve 1 disco que no está anotado en «SERVIDOR-01» (HDD 1 TB · Modelo). ¿Lo añades?» (0.8.2).
- «VM-01 es una máquina virtual. ¿En qué servidor físico está?» → un desplegable.
- «El almacén ALMACEN-01 guarda en ZFS «tank» (1,1 TB, 65 % usado). ¿En qué servidor está?» → el servidor y su conjunto o disco.
- «Ahora no» la quita (se recuerda por persona en el navegador).

Debajo, cada servidor en una tarjeta (discos físicos con su estado y número de serie, conjuntos con sus discos y **lo que vive aquí**), y dos listas para asignar cada equipo (con «Por volumen») y cada destino. `?equipo=<id>` o `?destino=<clave>` dejan la fila ya elegida: es adonde llevan los «Anotar» de la ficha de cada equipo y de cada destino. En Estado, un aviso suave si con algún servidor anotado quedan equipos o destinos sin anotar (se oculta, por persona, hasta que falte algo más).

**La tarjeta «Equipo»** de la ficha (A3) enseña los discos en tres niveles: discos físicos (con su nombre de Infraestructura; si no, «Disco 2 · HDD 1 TB · modelo»), conjuntos o discos virtuales (con los discos que los forman) y, colgando de cada uno, sus volúmenes con su etiqueta, sistema de archivos, lo libre y lo que guardan («con copias», «zona «Disco F» del almacén», «espejo «USB»», «destino «Copias en D»»). Nunca «disco0» (el número interno de Windows). En una máquina virtual, sus discos virtuales y «Sus discos virtuales están en hv-01 · tank»; un volumen anotado aparte dice «Físicamente en …».

**Una sola línea por destino**, en su página: «Dónde está físicamente: hv-01 · tank, espejo de 2 discos · Oficina central» (o «Sin anotar · Anotar»).

## En el mapa

- Lo anotado se ve en cada tarjeta (servidor · conjunto o disco), también en las de los equipos (capas «Infraestructura» y «Almacenamiento», encendidas por defecto; sin nada anotado no salen).
- **Filtrar por infraestructura** (A4, 15xx): en Estado, en Equipos y en «Todos los clientes» (con un cliente elegido), «Toda la infraestructura» o un servidor físico o un disco o conjunto suyo. Queda lo que está allí y su camino entero: lo que guarda (hacia la derecha) y quién le manda (hacia la izquierda). Se recuerda por persona.
- **En Equipos** (C4), «Agrupar por» puede ser también «Servidor físico» o «Disco o conjunto», y en el mapa cada grupo va en su recuadro, plegable.

## En la regla 3-2-1-1-0

Con infraestructura anotada (`ponerInfra` en `consola/src/lib/regla321.ts`):

- **Soportes reales:** un paso cuyo destino tiene conjunto anotado cuenta con el soporte `conjunto:<servidor>:<conjunto>` (desde la 0.8.2, uno en un disco suelto, `disco:<servidor>:<disco>`); los originales, con el de su equipo si se sabe. Dos destinos en el mismo conjunto cuentan como uno; uno en otro conjunto del mismo servidor, como otro soporte. Lo que una persona marcó como soporte en el catálogo manda. (Los volúmenes anotados aparte aún no cambian el soporte de los originales: se usa el del equipo.)
- **«Servidor físico compartido»** (`servidor_compartido`, vectores compartidos con `protection.rs`): dos copias de los mismos datos en **equipos distintos** (o un destino suelto, como un NAS) del **mismo servidor físico**. Es un aviso, como los demás: nunca bloquea.

## Lo que queda

- Conectar un hipervisor (15e, aparcada): el agente en el anfitrión en modo «solo informa» o su API, para enlazar solo cada equipo con su servidor y leer SMART y scrub.
- Avisos de un conjunto degradado que ve el agente o de un disco anotado como averiado (hoy solo se enseña).
- La capa «Infraestructura» del mapa como columnas propias (servidor → hipervisor → VM); hoy es una línea en cada tarjeta, el filtro y los recuadros.
- Que el soporte de los originales use el disco de cada volumen.
